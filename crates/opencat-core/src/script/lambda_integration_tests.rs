//! surface_* binding 通路上的 lambda 集成测试（语言本体在
//! `opencat-liangyi`，这里测 core 侧接线：dispatch → surface → bake）。

#![cfg(test)]

use serde_json::Value;

use opencat_liangyi::program::ScanKind;

use crate::ir::GeneratedImageId;
use crate::script::dispatch::dispatch_binding;
use crate::script::recorder::MutationStore;
use crate::text::surface;

/// 与 k3-promo.jsx 场构建 lambda 完全一致的 JS lambda 源（打包：R=mask、
/// G/B = 距离低/高字节）。
const FIELD_SEED: &str = "(uv, src) => { const c = src.eval(uv); const m = byte(c.a * 255 + 0.5); const d = m > 120 ? 0 : 3000; return [m / 255, d % 256 / 255, floor(d / 256) / 255, 1]; }";
const FIELD_FWD: &str = "(get) => { const c = get(0, 0); const d = byte(c.g * 255 + 0.5) + byte(c.b * 255 + 0.5) * 256; const l = get(-1, 0); const t = get(0, -1); const tl = get(-1, -1); const tr = get(1, -1); const m = min(min(d, byte(l.g * 255 + 0.5) + byte(l.b * 255 + 0.5) * 256 + 3), min(byte(t.g * 255 + 0.5) + byte(t.b * 255 + 0.5) * 256 + 3, min(byte(tl.g * 255 + 0.5) + byte(tl.b * 255 + 0.5) * 256 + 4, byte(tr.g * 255 + 0.5) + byte(tr.b * 255 + 0.5) * 256 + 4))); return [c.r, m % 256 / 255, floor(m / 256) / 255, 1]; }";
const FIELD_BWD: &str = "(get) => { const c = get(0, 0); const d = byte(c.g * 255 + 0.5) + byte(c.b * 255 + 0.5) * 256; const r = get(1, 0); const b = get(0, 1); const br = get(1, 1); const bl = get(-1, 1); const m = min(min(d, byte(r.g * 255 + 0.5) + byte(r.b * 255 + 0.5) * 256 + 3), min(byte(b.g * 255 + 0.5) + byte(b.b * 255 + 0.5) * 256 + 3, min(byte(br.g * 255 + 0.5) + byte(br.b * 255 + 0.5) * 256 + 4, byte(bl.g * 255 + 0.5) + byte(bl.b * 255 + 0.5) * 256 + 4))); return [c.r, m % 256 / 255, floor(m / 256) / 255, 1]; }";

/// oracle：删除前效果模块中 3-4 chamfer 双遍扫描的逐位拷贝（单位 px*3）。
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

/// render target 通路端到端（bindings 层）：surface_apply_effect（seed，
/// surface child 采样）→ surface_scan_pass 双遍 → surface_bake 注册帧级
/// 生成图像。场语义与 oracle 全等；bake 幂等 id 由 key 决定。
#[test]
fn render_target_bindings_end_to_end() {
    let (w, h) = (4u32, 4u32);
    let n = (w * h) as usize;
    let mut store = MutationStore::default();
    let call = |store: &mut MutationStore, name: &str, args: Vec<Value>| {
        dispatch_binding(store, name, &args).unwrap_or_else(|e| panic!("{name}: {e}"))
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
    surface::surface_write_rgba("src", w, h, src_rgba).unwrap();

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
    for (dir, src) in [("forward", FIELD_FWD), ("backward", FIELD_BWD)] {
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
    let got = surface::surface_get_rgba("tgt", 0.0, 0.0, 4.0, 4.0).unwrap();
    assert_eq!(got, want);

    // bake：注册帧级生成图像（id 由 key 决定，像素 = 目标当前内容）
    call(&mut store, "surface_bake", vec!["tgt".into(), Value::from("e2e-key")]);
    let pending = store.take_pending_generated_images();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, GeneratedImageId::from_key("e2e-key"));
    assert_eq!((pending[0].width, pending[0].height), (w, h));
    assert_eq!(pending[0].rgba.as_ref(), want.as_slice());
}

/// pixel 类 lambda（runEffect 通路）拒绝 scan 类 spec；scan 类拒绝
/// pixel spec（互斥由 binding 与 parse 双重把守）。
#[test]
fn surface_target_ops_reject_wrong_kind() {
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
    assert!(err.to_string().contains("pixel 类 lambda"), "{err}");

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
    assert!(err.to_string().contains("scan 类 lambda"), "{err}");

    // spec.kind 双向枚举仍与语言本体一致
    assert_eq!(ScanKind::default(), ScanKind::Pixel);
}
