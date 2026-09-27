//! # Effect Lambda DSL
//!
//! 效果（原手写 SKSL、原 Rust 手写逐像素逻辑如 k3 溶解）统一以 JS lambda
//! 表达。lambda **只被编译、从不被执行**：Rust 侧解析其源码（oxc），白名单
//! 校验 + 类型推断后落到自有 IR（[`program`]），再派发后端：
//!
//! - [`Backend::Sksl`]：codegen SKSL（[`lower_sksl`]），走现有
//!   `RuntimeEffect`（CPU raster 管线）绘制——镜像"原手写 SKSL 字符串"世界；
//! - [`Backend::Cpu`]：f64 AST 解释器逐像素求值（[`interp`]），产出
//!   RGBA 走 `DrawOp::Image { Generated }`——镜像"Rust 手写逐像素循环"
//!   世界，语义逐位对齐手写 Rust。
//!
//! 派发由 stdlib 能力标记自动决定（用到 `h01`/`imul` 等整数精确语义 → CPU），
//! spec 可显式覆盖。像素循环永不落在 JS，像素缓冲永不跨进 JS。

pub mod error;
pub mod interp;
pub mod lower_sksl;
pub mod parse;
pub mod program;
pub mod stdlib;

use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use ahash::AHasher;
use hashbrown::HashMap;

pub use error::{LambdaError, LambdaResult};
pub use program::{Backend, Program};

/// JS 侧 spec（`CK.Effect.fromLambda(fn, spec)` / `surface.runEffect` 的
/// 第二参）反序列化后的形态。uniforms 保持声明序——顺序即 f32 打包顺序，
/// 必须与 JS 传参一致。
#[derive(Debug, Clone, Default)]
pub struct EffectSpec {
    pub uniforms: Vec<(String, program::Ty)>,
    pub backend_override: Option<Backend>,
    pub kind: program::ScanKind,
}

impl EffectSpec {
    /// 从 JSON spec 构建。格式（数组保序，禁用对象映射避免键序漂移）：
    /// `{"uniforms": [["t","float"], ["origin","float2"]], "backend": "auto"}`
    pub fn from_json(value: &serde_json::Value) -> LambdaResult<Self> {
        use program::Ty;
        let mut spec = Self::default();
        let Some(obj) = value.as_object() else {
            if value.is_null() {
                return Ok(spec);
            }
            return Err(LambdaError::msg("spec 必须是对象"));
        };
        if let Some(uniforms) = obj.get("uniforms") {
            let Some(items) = uniforms.as_array() else {
                return Err(LambdaError::msg(
                    "spec.uniforms 必须是 [[name, type], ...] 数组（保序）",
                ));
            };
            for item in items {
                let Some(pair) = item.as_array() else {
                    return Err(LambdaError::msg("spec.uniforms 项必须是 [name, type] 对"));
                };
                let Some(name) = pair.first().and_then(|v| v.as_str()) else {
                    return Err(LambdaError::msg("spec.uniforms 项缺少名字字符串"));
                };
                let ty = match pair.get(1).and_then(|v| v.as_str()) {
                    Some("float") => Ty::Float,
                    Some("float2") => Ty::Vec2,
                    Some("float3") => Ty::Vec3,
                    Some("float4") => Ty::Vec4,
                    other => {
                        return Err(LambdaError::msg(format!(
                            "spec.uniforms.{name}: 不支持的类型 {other:?}（仅 float/float2/float3/float4）"
                        )))
                    }
                };
                spec.uniforms.push((name.to_string(), ty));
            }
        }
        match obj.get("backend").and_then(|b| b.as_str()) {
            None | Some("auto") => {}
            Some("sksl") => spec.backend_override = Some(Backend::Sksl),
            Some("cpu") => spec.backend_override = Some(Backend::Cpu),
            Some(other) => {
                return Err(LambdaError::msg(format!(
                    "spec.backend: 不支持的值 {other:?}（仅 auto/sksl/cpu）"
                )))
            }
        }
        match obj.get("kind").and_then(|k| k.as_str()) {
            None => {}
            Some("pixel") => spec.kind = program::ScanKind::Pixel,
            Some("scan") => spec.kind = program::ScanKind::Scan,
            Some(other) => {
                return Err(LambdaError::msg(format!(
                    "spec.kind: 不支持的值 {other:?}（仅 pixel/scan）"
                )))
            }
        }
        Ok(spec)
    }

    fn hash_key(&self, source: &str) -> u64 {
        let mut h = AHasher::default();
        source.hash(&mut h);
        for (name, ty) in &self.uniforms {
            name.hash(&mut h);
            ty.to_discriminant().hash(&mut h);
        }
        self.backend_override.map(|b| b as u8).hash(&mut h);
        self.kind.hash(&mut h);
        h.finish()
    }
}

/// 编译产物。
#[derive(Debug)]
pub struct CompiledEffect {
    pub program: Arc<Program>,
    pub backend: Backend,
    /// 仅 [`Backend::Sksl`]；CPU 后端为 None。
    pub sksl: Option<String>,
    /// ahash(source + spec)，缓存/日志用。
    pub hash: u64,
}

thread_local! {
    static COMPILE_CACHE: RefCell<HashMap<u64, Arc<LambdaResult<Program>>>> =
        RefCell::new(HashMap::new());
}

/// 编译（缓存命中免解析）。脚本每帧重跑，同一 lambda 源码整段渲染期只解析一次。
pub fn compile(source: &str, spec: &EffectSpec) -> LambdaResult<Arc<Program>> {
    let key = spec.hash_key(source);
    if let Some(entry) = COMPILE_CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return match &*entry {
            Ok(program) => Ok(Arc::new(program.clone())),
            Err(e) => Err(e.clone()),
        };
    }
    let result = compile_uncached(source, spec);
    let arc = Arc::new(match &result {
        Ok(program) => Ok((**program).clone()),
        Err(e) => Err(e.clone()),
    });
    COMPILE_CACHE.with(|c| c.borrow_mut().insert(key, arc));
    result
}

fn compile_uncached(source: &str, spec: &EffectSpec) -> LambdaResult<Arc<Program>> {
    let mut program = parse::parse_lambda(source, spec)?;
    program.backend_override = spec.backend_override;
    if program.backend_override == Some(Backend::Sksl) && program.uses_cpu_only {
        return Err(LambdaError::at(
            "该 lambda 使用了仅 CPU 后端支持的内建（h01/imul 等），不能强制 sksl 后端",
            (0, source.len() as u32),
            source,
        ));
    }
    if program.backend_override == Some(Backend::Sksl) && program.kind == program::ScanKind::Scan {
        return Err(LambdaError::at(
            "scan 类 lambda 只支持 CPU 解释器（顺序扫描无 SKSL 形态），不能强制 sksl 后端",
            (0, source.len() as u32),
            source,
        ));
    }
    Ok(Arc::new(program))
}

/// 编译 + 派发 + （SKSL 时）codegen，binding 层入口。
pub fn compile_effect(source: &str, spec: &EffectSpec) -> LambdaResult<CompiledEffect> {
    let hash = spec.hash_key(source);
    let program = compile(source, spec)?;
    let backend = program.backend();
    let sksl = if backend == Backend::Sksl {
        Some(lower_sksl::lower(&program)?)
    } else {
        None
    };
    Ok(CompiledEffect { program, backend, sksl, hash })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(uniforms: &[(&str, &str)]) -> EffectSpec {
        let mut s = EffectSpec::default();
        for (name, ty) in uniforms {
            let ty = match *ty {
                "float" => program::Ty::Float,
                "float2" => program::Ty::Vec2,
                "float3" => program::Ty::Vec3,
                "float4" => program::Ty::Vec4,
                _ => panic!("bad test ty"),
            };
            s.uniforms.push((name.to_string(), ty));
        }
        s
    }

    const GRADIENT: &str = "(uv, u) => { return [uv.x / u.w, uv.y / u.h, 0, 1]; }";

    #[test]
    fn dispatch_float_only_to_sksl() {
        let c = compile_effect(GRADIENT, &spec(&[("w", "float"), ("h", "float")])).unwrap();
        assert_eq!(c.backend, Backend::Sksl);
        let sksl = c.sksl.unwrap();
        assert!(sksl.contains("uniform float u_w;"), "{sksl}");
        assert!(sksl.contains("uniform float u_oc_rect_0;"), "{sksl}");
        assert!(sksl.contains("float2 oc_uv = xy - u_oc_rect.xy;"), "{sksl}");
        assert!(sksl.contains("half4 main(float2 xy)"), "{sksl}");
    }

    const HASHED: &str =
        "(uv, u) => { const h = h01(byte(uv.x), byte(uv.y), u.f); return [h, h, h, 1]; }";

    #[test]
    fn dispatch_h01_to_cpu_and_sksl_override_rejected() {
        let c = compile_effect(HASHED, &spec(&[("f", "float")])).unwrap();
        assert_eq!(c.backend, Backend::Cpu);
        assert!(c.sksl.is_none());

        let mut forced = spec(&[("f", "float")]);
        forced.backend_override = Some(Backend::Sksl);
        let err = compile_effect(HASHED, &forced).unwrap_err();
        assert!(err.to_string().contains("仅 CPU"), "{err}");
    }

    #[test]
    fn cache_hits_same_hash() {
        let a = compile_effect(GRADIENT, &spec(&[("w", "float"), ("h", "float")])).unwrap();
        let b = compile_effect(GRADIENT, &spec(&[("w", "float"), ("h", "float")])).unwrap();
        assert_eq!(a.hash, b.hash);
        assert_eq!(a.sksl, b.sksl);
    }

    /// README "HTML in Canvas" 示例（subtree 纹理 + lambda 折射/色散）——
    /// 保持 README 代码可编译、可派发到 SKSL。
    #[test]
    fn readme_subtree_ripple_lambda_lowers_to_sksl() {
        const RIPPLE: &str = r#"(uv, image, u) => {
  const d = uv - [180.0, 240.0];
  const dist = length(d);
  const dir = dist < 1.0 ? [0.0, 0.0] : d / dist;
  const tang = [-dir.y, dir.x];
  const wave = sin(dist * u.frequency - u.progress * u.speed);
  const base = uv + dir * (wave * u.amplitude * exp(-dist * u.decay));
  const r = image.eval(base + tang * u.split);
  const g = image.eval(base);
  const b = image.eval(base - tang * u.split);
  const a = max(max(r.a, g.a), b.a);
  return [r.r, g.g, b.b, a];
}"#;
        let c = compile_effect(
            RIPPLE,
            &spec(&[
                ("progress", "float"),
                ("amplitude", "float"),
                ("frequency", "float"),
                ("speed", "float"),
                ("decay", "float"),
                ("split", "float"),
            ]),
        )
        .unwrap();
        assert_eq!(c.backend, Backend::Sksl);
        let sksl = c.sksl.unwrap();
        assert!(sksl.contains("u_oc_c0.eval("), "{sksl}");
        assert!(sksl.contains("half4 main(float2 xy)"), "{sksl}");
    }

    /// skill/references/templates.md §7 示例（subtree 纹理 + lambda 波形位移）。
    #[test]
    fn templates_wave_lambda_lowers_to_sksl() {
        const WAVE: &str =
            "(uv, image, u) => { const src = uv + [sin(uv.y * 0.04 + u.t * 4.0) * 6.0, 0.0]; return image.eval(src); }";
        let c = compile_effect(WAVE, &spec(&[("t", "float")])).unwrap();
        assert_eq!(c.backend, Backend::Sksl);
        assert!(c.sksl.unwrap().contains("u_oc_c0.eval("));
    }

    #[test]
    fn interpreter_gradient_pixels() {
        let mut s = spec(&[("w", "float"), ("h", "float")]);
        s.backend_override = Some(Backend::Cpu);
        let c = compile_effect(GRADIENT, &s).unwrap();
        let ctx = interp::InterpCtx {
            uniforms: &[program::Val::F(4.0), program::Val::F(2.0)],
            rect: [0.0, 0.0, 4.0, 2.0],
            children: &[],
            scan: None,
        };
        let rgba = interp::render(&c.program, 4, 2, &ctx);
        // uv = px = col/row + 0.5；4x2 图前 4 像素都是第 0 行
        // gx*255 round：col0→32, col1→96, col2→159, col3→223
        // gy*255 round：row0→64, row1→191
        let assert_px = |i: usize, want: [u8; 4]| {
            assert_eq!(&rgba[i * 4..i * 4 + 4], &want, "px {i}");
        };
        assert_px(0, [32, 64, 0, 255]);
        assert_px(1, [96, 64, 0, 255]);
        assert_px(2, [159, 64, 0, 255]);
        assert_px(3, [223, 64, 0, 255]);
        assert_px(4, [32, 191, 0, 255]);
        assert_px(7, [223, 191, 0, 255]);
    }

    #[test]
    fn interpreter_child_sampling_and_byte_roundtrip() {
        // child R 通道存整数 m，lambda 还原 byte(c.r*255+0.5) == m
        let mut s = EffectSpec::default();
        s.backend_override = Some(Backend::Cpu);
        let c = compile_effect(
            "(uv, tex, u) => { const c = tex.eval(uv); const m = byte(c.r * 255 + 0.5); return [m / 255, m / 255, m / 255, 1]; }",
            &s,
        )
        .unwrap();
        let child = interp::ChildImage {
            width: 2,
            height: 1,
            rgba: std::sync::Arc::from(vec![0u8, 255, 0, 255, 128, 0, 0, 255]),
        };
        let ctx = interp::InterpCtx {
            uniforms: &[],
            rect: [0.0, 0.0, 2.0, 1.0],
            children: &[child],
            scan: None,
        };
        let rgba = interp::render(&c.program, 2, 1, &ctx);
        assert_eq!(&rgba[0..4], &[0, 0, 0, 255]);
        assert_eq!(&rgba[4..8], &[128, 128, 128, 255]);
    }

    #[test]
    fn whitelist_rejections_carry_position() {
        let cases: [(&str, &str); 8] = [
            ("(uv, u) => { for (let i = 0; i < 4; i++) { } return [0,0,0,0]; }", "循环"),
            ("(uv, u) => { const f = () => 1; return [0,0,0,0]; }", "嵌套函数"),
            ("(uv, u) => { return [1 & 2, 0, 0, 0]; }", "位运算"),
            ("(uv, u) => { return ['a', 0, 0, 0]; }", "字符串"),
            ("(uv, u) => { return [u.zz, 0, 0, 0]; }", "未知 uniform"),
            ("(uv, u) => { return [Math.random(), 0, 0, 0]; }", "Math.random"),
            ("(uv, u) => { const a = 1; a = 2; return [0,0,0,0]; }", "const"),
            ("(uv, u) => { return [2 ** 3, 0, 0, 0]; }", "pow"),
        ];
        for (src, hint) in cases {
            let err = compile_effect(src, &EffectSpec::default()).unwrap_err();
            let msg = err.to_string();
            assert!(msg.contains(hint) || msg.contains("lambda 源码第"), "{src} → {msg}");
            assert!(err.line_col().is_some(), "{src} 缺位置：{msg}");
        }
    }

    #[test]
    fn vec_builtins_in_interpreter() {
        let mut s = EffectSpec::default();
        s.backend_override = Some(Backend::Cpu);
        let c = compile_effect(
            "(uv, u) => { const m = mix([0, 0, 0, 0], [1, 1, 1, 1], 0.5); return clamp(m, 0.25, 0.75); }",
            &s,
        )
        .unwrap();
        let ctx = interp::InterpCtx { uniforms: &[], rect: [0.0; 4], children: &[], scan: None };
        let px = interp::eval_pixel(&c.program, 0.5, 0.5, &ctx);
        // mix → 0.5；clamp(…, 0.25, 0.75) → 0.5 → 127.5 → round 128
        assert_eq!(px, [0.5, 0.5, 0.5, 0.5]);
        let rgba = interp::render(&c.program, 1, 1, &ctx);
        assert_eq!(&rgba[..], &[128, 128, 128, 128]);
    }

    // ── render target（scan 类 / surface 目标）：与删除前的效果专属
    //    扫描模块（chamfer_3_4）逐位对齐的 oracle 锚定 ────────────────────

    /// oracle：删除前效果模块中 3-4 chamfer 双遍扫描的逐位拷贝
    /// （单位 px*3；只作测试基准，不在任何生产路径上）。
    fn oracle_chamfer_3_4(dist: &mut [i16], w: usize, h: usize) {
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let mut d = dist[i];
                if x > 0 && dist[i - 1] + 3 < d {
                    d = dist[i - 1] + 3;
                }
                if y > 0 {
                    if dist[i - w] + 3 < d {
                        d = dist[i - w] + 3;
                    }
                    if x > 0 && dist[i - w - 1] + 4 < d {
                        d = dist[i - w - 1] + 4;
                    }
                    if x < w - 1 && dist[i - w + 1] + 4 < d {
                        d = dist[i - w + 1] + 4;
                    }
                }
                dist[i] = d;
            }
        }
        for y in (0..h).rev() {
            for x in (0..w).rev() {
                let i = y * w + x;
                let mut d = dist[i];
                if x < w - 1 && dist[i + 1] + 3 < d {
                    d = dist[i + 1] + 3;
                }
                if y < h - 1 {
                    if dist[i + w] + 3 < d {
                        d = dist[i + w] + 3;
                    }
                    if x < w - 1 && dist[i + w + 1] + 4 < d {
                        d = dist[i + w + 1] + 4;
                    }
                    if x > 0 && dist[i + w - 1] + 4 < d {
                        d = dist[i + w - 1] + 4;
                    }
                }
                dist[i] = d;
            }
        }
    }

    /// oracle：删除前效果模块测试用的合成 mask（含边界子阈值裙、
    /// 圆环、对角条纹等结构）。
    fn oracle_synth_mask(w: u32, h: u32) -> Vec<u8> {
        let (w, h) = (w as i64, h as i64);
        let mut mask = vec![0u8; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let mut a = 0u8;
                if (30..120).contains(&y) && (100..180).contains(&x) {
                    a = 255;
                }
                if (10..50).contains(&y) && (1000..1040).contains(&x) {
                    a = 118;
                }
                let d2 = (x - 700) * (x - 700) + (y - 111) * (y - 111);
                if d2 <= 60 * 60 {
                    a = 255;
                }
                if d2 > 61 * 61 && d2 <= 64 * 64 {
                    a = 140;
                }
                if (x + y) % 50 < 6 && y >= 150 {
                    a = 180;
                }
                mask[(y * w + x) as usize] = a;
            }
        }
        mask
    }

    /// 与 k3-promo.xml 场构建 lambda 完全一致的 JS lambda 源（打包：R=mask、
    /// G/B = 距离低/高字节）。
    const FIELD_SEED: &str = "(uv, src) => { const c = src.eval(uv); const m = byte(c.a * 255 + 0.5); const d = m > 120 ? 0 : 3000; return [m / 255, d % 256 / 255, floor(d / 256) / 255, 1]; }";
    const FIELD_FWD: &str = "(get) => { const c = get(0, 0); const d = byte(c.g * 255 + 0.5) + byte(c.b * 255 + 0.5) * 256; const l = get(-1, 0); const t = get(0, -1); const tl = get(-1, -1); const tr = get(1, -1); const m = min(min(d, byte(l.g * 255 + 0.5) + byte(l.b * 255 + 0.5) * 256 + 3), min(byte(t.g * 255 + 0.5) + byte(t.b * 255 + 0.5) * 256 + 3, min(byte(tl.g * 255 + 0.5) + byte(tl.b * 255 + 0.5) * 256 + 4, byte(tr.g * 255 + 0.5) + byte(tr.b * 255 + 0.5) * 256 + 4))); return [c.r, m % 256 / 255, floor(m / 256) / 255, 1]; }";
    const FIELD_BWD: &str = "(get) => { const c = get(0, 0); const d = byte(c.g * 255 + 0.5) + byte(c.b * 255 + 0.5) * 256; const r = get(1, 0); const b = get(0, 1); const br = get(1, 1); const bl = get(-1, 1); const m = min(min(d, byte(r.g * 255 + 0.5) + byte(r.b * 255 + 0.5) * 256 + 3), min(byte(b.g * 255 + 0.5) + byte(b.b * 255 + 0.5) * 256 + 3, min(byte(br.g * 255 + 0.5) + byte(br.b * 255 + 0.5) * 256 + 4, byte(bl.g * 255 + 0.5) + byte(bl.b * 255 + 0.5) * 256 + 4))); return [c.r, m % 256 / 255, floor(m / 256) / 255, 1]; }";

    #[test]
    fn scan_chain_reproduces_chamfer_oracle_byte_exact() {
        let (w, h) = (208u32, 144u32);
        let n = (w * h) as usize;
        let mask = oracle_synth_mask(w, h);

        // oracle 场：seed + chamfer + bake_rgba 打包
        let mut dist = vec![3000i16; n];
        for i in 0..n {
            if mask[i] > 120 {
                dist[i] = 0;
            }
        }
        oracle_chamfer_3_4(&mut dist, w as usize, h as usize);
        let mut want = vec![0u8; n * 4];
        for i in 0..n {
            let d = dist[i] as u16;
            want[i * 4] = mask[i];
            want[i * 4 + 1] = (d & 0xFF) as u8;
            want[i * 4 + 2] = (d >> 8) as u8;
            want[i * 4 + 3] = 255;
        }

        // JS 链：seed（pixel 类，surface child 采样）→ forward → backward
        let mut seed_spec = EffectSpec::default();
        seed_spec.backend_override = Some(Backend::Cpu);
        let seed = compile_effect(FIELD_SEED, &seed_spec).unwrap();
        let src_rgba: Vec<u8> =
            (0..n).flat_map(|i| [0u8, 0, 0, mask[i]]).collect();
        let child = interp::ChildImage {
            width: w,
            height: h,
            rgba: std::sync::Arc::from(src_rgba),
        };
        let ctx = interp::InterpCtx {
            uniforms: &[],
            rect: [0.0, 0.0, w as f64, h as f64],
            children: &[child],
            scan: None,
        };
        let mut buf = interp::render(&seed.program, w, h, &ctx);
        // seed 单独就应等于 chamfer 前的打包（mask 直通 + sentinel/0）
        for i in 0..n {
            let d: u16 = if mask[i] > 120 { 0 } else { 3000 };
            assert_eq!(buf[i * 4], mask[i], "seed mask[{i}]");
            assert_eq!(
                (buf[i * 4 + 1] as u16) | ((buf[i * 4 + 2] as u16) << 8),
                d,
                "seed dist[{i}]"
            );
        }

        let mut scan_spec = EffectSpec::default();
        scan_spec.kind = program::ScanKind::Scan;
        let fwd = compile_effect(FIELD_FWD, &scan_spec).unwrap();
        assert_eq!(fwd.backend, Backend::Cpu, "scan 强制 CPU 后端");
        interp::render_scan(
            &fwd.program,
            w,
            h,
            &[],
            interp::ScanDirection::Forward,
            &mut buf,
        );
        let bwd = compile_effect(FIELD_BWD, &scan_spec).unwrap();
        interp::render_scan(
            &bwd.program,
            w,
            h,
            &[],
            interp::ScanDirection::Backward,
            &mut buf,
        );
        // 全缓冲逐位相等（含四条边界与 sentinel 3000 的钳制邻域读）
        assert_eq!(buf, want);
    }

    #[test]
    fn scan_whitelist_rejections() {
        // scan 类禁止强制 sksl（顺序扫描只有 CPU 解释器形态）
        let mut s = EffectSpec::default();
        s.kind = program::ScanKind::Scan;
        s.backend_override = Some(Backend::Sksl);
        let err = compile_effect("(get) => { return [0, 0, 0, 1]; }", &s).unwrap_err();
        assert!(err.to_string().contains("CPU"), "{err}");

        // scan 类禁止 child 参数（采样只能 get(dx, dy)）
        let mut s = EffectSpec::default();
        s.kind = program::ScanKind::Scan;
        let err = compile_effect("(get, tex) => { return [0, 0, 0, 1]; }", &s).unwrap_err();
        assert!(err.to_string().contains("child"), "{err}");
    }

    /// render target 通路端到端（bindings 层）：surface_apply_effect（seed，
    /// surface child 采样）→ surface_scan_pass 双遍 → surface_bake 注册帧级
    /// 生成图像。场语义与 oracle 全等；bake 幂等 id 由 key 决定。
    #[test]
    fn render_target_bindings_end_to_end() {
        use crate::script::dispatch::dispatch_binding;
        use crate::script::recorder::MutationStore;
        use serde_json::Value;

        let (w, h) = (4u32, 4u32);
        let n = (w * h) as usize;
        let mut store = MutationStore::default();
        let call = |store: &mut MutationStore, name: &str, args: Vec<Value>| {
            dispatch_binding(store, name, &args)
                .unwrap_or_else(|e| panic!("{name}: {e}"))
        };

        // 源 surface：alpha 列纹（x==1 → 255，否则 0）
        call(
            &mut store,
            "surface_create",
            vec!["src".into(), Value::from(4.0), Value::from(4.0)],
        );
        let src_rgba: Vec<u8> = (0..n)
            .flat_map(|i| {
                let a = if i % 4 == 1 { 255u8 } else { 0 };
                [0u8, 0, 0, a]
            })
            .collect();
        crate::text::surface::surface_write_rgba("src", w, h, src_rgba).unwrap();

        call(
            &mut store,
            "surface_create",
            vec!["tgt".into(), Value::from(4.0), Value::from(4.0)],
        );
        call(
            &mut store,
            "surface_apply_effect",
            vec![
                "tgt".into(),
                Value::from(FIELD_SEED),
                Value::from("{}"),
                Value::Array(vec![]),
                Value::from(r#"[{"__opencatShader":"surface","id":"src"}]"#),
            ],
        );
        for (dir, src) in [
            ("forward", FIELD_FWD),
            ("backward", FIELD_BWD),
        ] {
            call(
                &mut store,
                "surface_scan_pass",
                vec![
                    "tgt".into(),
                    Value::from(src),
                    Value::from("{}"),
                    Value::Array(vec![]),
                    Value::from(dir),
                ],
            );
        }

        // 场语义 oracle 全等
        let mask: Vec<u8> = (0..n).map(|i| if i % 4 == 1 { 255 } else { 0 }).collect();
        let mut dist = vec![3000i16; n];
        for i in 0..n {
            if mask[i] > 120 {
                dist[i] = 0;
            }
        }
        oracle_chamfer_3_4(&mut dist, w as usize, h as usize);
        let mut want = vec![0u8; n * 4];
        for i in 0..n {
            let d = dist[i] as u16;
            want[i * 4] = mask[i];
            want[i * 4 + 1] = (d & 0xFF) as u8;
            want[i * 4 + 2] = (d >> 8) as u8;
            want[i * 4 + 3] = 255;
        }
        let got = crate::text::surface::surface_get_rgba("tgt", 0.0, 0.0, 4.0, 4.0).unwrap();
        assert_eq!(got, want);

        // bake：注册帧级生成图像（id 由 key 决定，像素 = 目标当前内容）
        call(
            &mut store,
            "surface_bake",
            vec!["tgt".into(), Value::from("e2e-key")],
        );
        let pending = store.take_pending_generated_images();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, crate::ir::GeneratedImageId::from_key("e2e-key"));
        assert_eq!((pending[0].width, pending[0].height), (w, h));
        assert_eq!(pending[0].rgba.as_ref(), want.as_slice());
    }

    /// pixel 类 lambda（runEffect 通路）拒绝 scan 类 spec；scan 类拒绝
    /// pixel spec（互斥由 binding 与 parse 双重把守）。
    #[test]
    fn surface_target_ops_reject_wrong_kind() {
        use crate::script::dispatch::dispatch_binding;
        use crate::script::recorder::MutationStore;
        use serde_json::Value;

        let mut store = MutationStore::default();
        let err = dispatch_binding(
            &mut store,
            "surface_apply_effect",
            &[
                "t".into(),
                Value::from(FIELD_FWD),
                Value::from(r#"{"kind":"scan"}"#),
                Value::Array(vec![]),
                Value::from("[]"),
            ],
        )
        .unwrap_err();
        assert!(err.to_string().contains("runEffect"), "{err}");

        let err = dispatch_binding(
            &mut store,
            "surface_scan_pass",
            &[
                "t".into(),
                Value::from(FIELD_SEED),
                Value::from(r#"{"kind":"pixel"}"#),
                Value::Array(vec![]),
                Value::from("forward"),
            ],
        )
        .unwrap_err();
        assert!(err.to_string().contains("scanPass"), "{err}");
    }
}
