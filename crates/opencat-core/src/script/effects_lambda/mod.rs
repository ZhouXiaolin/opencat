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

/// JS 侧 spec（`CK.Effect.fromLambda(fn, spec)` 的第二参）反序列化后的形态。
/// uniforms 保持声明序——顺序即 f32 打包顺序，必须与 JS 传参一致。
#[derive(Debug, Clone, Default)]
pub struct EffectSpec {
    pub uniforms: Vec<(String, program::Ty)>,
    pub backend_override: Option<Backend>,
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

    #[test]
    fn interpreter_gradient_pixels() {
        let mut s = spec(&[("w", "float"), ("h", "float")]);
        s.backend_override = Some(Backend::Cpu);
        let c = compile_effect(GRADIENT, &s).unwrap();
        let ctx = interp::InterpCtx {
            uniforms: &[program::Val::F(4.0), program::Val::F(2.0)],
            rect: [0.0, 0.0, 4.0, 2.0],
            children: &[],
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
        let ctx = interp::InterpCtx { uniforms: &[], rect: [0.0; 4], children: &[] };
        let px = interp::eval_pixel(&c.program, 0.5, 0.5, &ctx);
        // mix → 0.5；clamp(…, 0.25, 0.75) → 0.5 → 127.5 → round 128
        assert_eq!(px, [0.5, 0.5, 0.5, 0.5]);
        let rgba = interp::render(&c.program, 1, 1, &ctx);
        assert_eq!(&rgba[..], &[128, 128, 128, 128]);
    }
}
