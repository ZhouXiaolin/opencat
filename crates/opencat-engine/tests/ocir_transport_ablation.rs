//! Ablation: raw vs. compressed OCIR transport, on real k3-promo frames.
//!
//! Measures, per sampled frame:
//!   encode   — RenderFrame → raw OCIR bytes (Rust, core)
//!   compress — raw → container (Rust, core; raw DEFLATE at several levels)
//!   inflate  — container → raw (Rust, core; mirrors the browser's
//!              `DecompressionStream('deflate-raw')` cost)
//! and reports sizes.
//!
//! Ignored by default (renders a real composition through the engine). Run:
//!   cargo test -p opencat-engine --test ocir_transport_ablation -- --ignored --nocapture

use std::path::PathBuf;
use std::time::{Duration, Instant};

use opencat_core::ir::draw_encoding::encode_ir_envelope;
use opencat_core::ir::draw_frame::DrawFrameScratch;
use opencat_core::ir::transport::{compress_ir_envelope, decompress_ir_envelope};
use opencat_core::pipeline::Pipeline;
use opencat_core::script::js_context::JsContext;

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

#[test]
#[ignore = "renders k3-promo through the engine; run with --ignored --nocapture"]
fn ocir_transport_ablation() {
    let path =
        std::env::var("OCIR_ABLATION_INPUT").unwrap_or_else(|_| "examples/k3-promo.xml".to_string());
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let full = repo.join(&path);
    let src = std::fs::read_to_string(&full).expect("read input");
    let base = full.parent().unwrap().to_path_buf();

    let cache_base = dirs::home_dir().unwrap();
    let cache_dir = cache_base.join(".opencat").join("assets");
    let loader = opencat_engine::resource::loader::EngineLoader::new(base, cache_dir).unwrap();
    let ctx = opencat_engine::js_context::RqJsContext::new().unwrap();
    let mut pipeline = opencat_engine::pipeline::open(&src, loader, ctx).unwrap();
    let info = pipeline.info().clone();
    let frame_count = ((info.duration * info.fps as f64) as u32).max(1);

    // Coarse sweep + a contiguous window (to show frame-to-frame redundancy).
    let step = (frame_count / 8).max(1);
    let mid = frame_count / 2;
    let mut sample: Vec<u32> = (0..frame_count).step_by(step as usize).collect();
    sample.extend(mid..(mid + 6).min(frame_count));
    sample.sort_unstable();
    sample.dedup();

    // miniz levels to compare: 1 (fastest), 6 (default), 10 (best).
    let levels: [u8; 3] = [1, 6, 10];

    let mut scratch = DrawFrameScratch::default();

    let mut raw_total = 0u64;
    // accumulators over *warm* frames (skip frame 0: first-touch allocation
    // noise inflates the mean otherwise).
    let mut enc_ms: Vec<f64> = Vec::new();
    let mut comp_ms: [Vec<f64>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let mut infl_ms: [Vec<f64>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let mut size_total = [0u64; 3];

    println!(
        "\n{:<6} {:>10} {:>9} {:>9} {:>9} {:>7} {:>8} {:>9} {:>9}",
        "frame", "raw B", "z1 B", "z6 B", "z10 B", "z6 x", "enc ms", "comp6 ms", "infl6 ms"
    );

    for (idx, &f) in sample.iter().enumerate() {
        let frame = pipeline.render_frame(f).unwrap();

        // encode
        scratch.clear();
        let t = Instant::now();
        let raw = encode_ir_envelope(&frame, &mut scratch).unwrap();
        let e_ms = ms(t.elapsed());

        let mut comp = [0usize; 3];
        let mut c_ms = [0.0f64; 3];
        let mut i_ms = [0.0f64; 3];
        for (i, &lv) in levels.iter().enumerate() {
            let t = Instant::now();
            let packed = compress_ir_envelope(&raw, lv);
            c_ms[i] = ms(t.elapsed());
            comp[i] = packed.len();
            // inflate (mirrors browser DecompressionStream cost)
            let t = Instant::now();
            let back = decompress_ir_envelope(&packed).unwrap();
            i_ms[i] = ms(t.elapsed());
            debug_assert_eq!(back.len(), raw.len());
        }

        raw_total += raw.len() as u64;
        for i in 0..3 {
            size_total[i] += comp[i] as u64;
        }
        // skip frame index 0 from the timing means (warm-up)
        if idx > 0 {
            enc_ms.push(e_ms);
            for i in 0..3 {
                comp_ms[i].push(c_ms[i]);
                infl_ms[i].push(i_ms[i]);
            }
        }

        println!(
            "{:<6} {:>10} {:>9} {:>9} {:>9} {:>7.2} {:>8.3} {:>9.3} {:>9.3}",
            f,
            raw.len(),
            comp[0],
            comp[1],
            comp[2],
            raw.len() as f64 / comp[1] as f64,
            e_ms,
            c_ms[1],
            i_ms[1],
        );
    }

    let n = sample.len() as f64;
    let m = enc_ms.len() as f64;
    let mean = |v: &[f64]| v.iter().sum::<f64>() / (v.len() as f64).max(1.0);
    let max_of = |v: &[f64]| v.iter().cloned().fold(0.0f64, f64::max);
    println!("\n=== AGGREGATE over {n} frames (timing mean over {} warm frames) ===", enc_ms.len());
    println!("raw OCIR total              = {raw_total} B");
    println!(
        "encode:        mean {:.3} ms  max {:.3} ms",
        mean(&enc_ms),
        max_of(&enc_ms)
    );
    for (i, &lv) in levels.iter().enumerate() {
        println!(
            "compress z{lv:<2}:  mean {:.3} ms  max {:.3} ms   total {} B  ({:.2}x vs {raw_total} B raw)",
            mean(&comp_ms[i]),
            max_of(&comp_ms[i]),
            size_total[i],
            raw_total as f64 / size_total[i] as f64,
        );
        println!(
            "inflate  z{lv:<2}:  mean {:.3} ms  max {:.3} ms",
            mean(&infl_ms[i]),
            max_of(&infl_ms[i]),
        );
    }
    let _ = m;
    println!(
        "\nnote: 'inflate' is core's Rust miniz decode — a close proxy for the browser\n\
         DecompressionStream('deflate-raw') cost on the same stream. 'encode' is the\n\
         raw OCIR pack the wasm bridge already does today (compression is a separate,\n\
         optional hop cost)."
    );
}
