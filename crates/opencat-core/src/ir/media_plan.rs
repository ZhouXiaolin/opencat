use std::sync::Arc;

use super::draw_types::{EffectRef, ImageRef};
use super::generated_image::GeneratedImageId;

/// Host-facing payload for one core-generated image needed by the current frame.
///
/// Carries the stable id plus full RGBA so hosts never need to open the
/// pipeline's internal generated-image table. Hosts may cache platform images
/// by [`GeneratedImageId`]; cache epoch is a host concern and is not part of
/// this contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameGeneratedImage {
    pub id: GeneratedImageId,
    pub width: u32,
    pub height: u32,
    /// RGBA_8888, unpremultiplied. Length is `width * height * 4`.
    ///
    /// Empty when [`Self::gpu`] is set.
    pub rgba: Arc<[u8]>,
    /// GPU-resident pixels (opaque host handle). When set, hosts resolve the
    /// texture instead of uploading `rgba`. See
    /// [`GpuImageHandle`](crate::ir::generated_image::GpuImageHandle).
    pub gpu: Option<crate::ir::generated_image::GpuImageHandle>,
}

/// Host-facing payload for one encoded image (`ImageRef::Encoded`): the raw
/// encoded bytes plus the natural pixel size declared by the producer (usvg
/// for `<image>`, so hosts never decode just to learn the draw size).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameEncodedImage {
    pub data: crate::ir::draw_types::BytesRangeId,
    pub width: u32,
    pub height: u32,
    /// Encoded image bytes (PNG/JPEG/GIF/WebP as produced by the source).
    pub bytes: Arc<[u8]>,
}

/// Per-frame media preparation plan describing what a host must prepare for the
/// current frame, derived deterministically from the `DrawOpFrame` plus the
/// pipeline's generated-image table. Each bucket is deduplicated so a given
/// image / video / Lottie / effect / generated image appears at most once.
///
/// Core only describes the current frame's needs; it never predicts future
/// frames or dictates decoder/seek/prefetch strategy. Hosts build their own
/// decoder cache, seek, and prefetch windows on top of this.
///
/// `generated_images` includes full RGBA content — `RenderFrame` is the sole
/// core→host current-frame contract; hosts do not read pipeline-internal
/// tables for glyph bitmaps.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FrameMediaPlan {
    /// External (static asset) image references, deduplicated.
    pub images: Vec<ImageRef>,
    /// Video frame references (canonical `AssetId` + authoritative
    /// `time_micros`), deduplicated.
    pub video_frames: Vec<ImageRef>,
    /// Lottie bundle ids referenced by `DrawOp::LottieRect`, deduplicated.
    pub lottie_bundles: Vec<String>,
    /// Runtime shader effects, deduplicated by effect id.
    pub runtime_effects: Vec<EffectRef>,
    /// Core-generated images (color-emoji bitmap glyphs) referenced by
    /// `ImageRef::Generated`, deduplicated by id, each with full RGBA.
    pub generated_images: Vec<FrameGeneratedImage>,
    /// Encoded images referenced by `ImageRef::Encoded`, deduplicated by
    /// bytes-range id, each with the raw encoded bytes. Decoding (and any
    /// resampling) is host/engine territory.
    pub encoded_images: Vec<FrameEncodedImage>,
    /// Author-asserted YUV→RGB conversion matrices (`data-color-matrix`) for
    /// the video assets this frame references, keyed by canonical asset id.
    /// Hosts must decode the named assets with the declared matrix instead of
    /// trusting stream metadata (capture tools often tag BT.709 over BT.601
    /// encodes). Assets absent from this map decode automatically.
    pub video_color_matrices: std::collections::BTreeMap<String, crate::media::VideoColorMatrix>,
    /// Effective on-device draw size per `ImageRect`-drawn reference (destination
    /// size × accumulated ancestor `Scale`, ceiled, max-merged across all draw
    /// sites). Hosts may decode down to this size — never up — without visible
    /// change; assets absent from this map decode at native resolution.
    ///
    /// Only `DrawOp::ImageRect` contributes: `DrawOp::Image` has no destination
    /// (whole-image semantics), generated glyphs ship exact-size RGBA, and
    /// runtime-effect child textures are sampled at shader-controlled UVs.
    pub image_draw_sizes: std::collections::HashMap<ImageRef, (u32, u32)>,
}

// ---------------------------------------------------------------------------
// wire JSON 契约（M3-④ 生成化）
// ---------------------------------------------------------------------------

/// media plan wire JSON 的键清单与 TS 类型文本（`build_frame_ir` /
/// `get_frame_plan` 返回的 `mediaPlan` 字符串）。单一事实源：[`FrameMediaPlan::to_wire_json`]
/// 按本契约生产，schema_gen 按本表生成 TS 接口（media-plan.schema.generated.ts），
/// 下方测试验证生产键与本表逐键一致——三处漂移任何一处都会在构建期/测试期炸出。
pub mod wire_spec {
    /// `(键, TS 类型文本)`。契约是键集（JSON 对象键无序——to_wire_json
    /// 的实际键序由 serde_json 决定）；TS 类型文本逐键给出，schema_gen
    /// 原样拷入生成接口。
    pub const FIELDS: &[(&str, &str)] = &[
        ("videoFrames", "{ assetId: string; timeMicros: number }[]"),
        ("images", "{ assetId: string }[]"),
        ("lottieBundles", "{ bundleId: string }[]"),
        ("generatedImages", "{ id: number; width: number; height: number }[]"),
        ("videoColorMatrices", "Record<string, 'bt601' | 'bt709'>"),
    ];
}

impl FrameMediaPlan {
    /// wire JSON（host 侧 `mediaPlan` 字符串）。只描述当前帧的准备工作；
    /// generated 图的完整 RGBA 与 encoded 字节走 OCIR envelope 段 12，
    /// 这里只留分配提示需要的 id/尺寸。
    ///
    /// （原 wasm_bridge.rs::media_plan_to_json，随 wire 契约一起收进 core
    /// 与 [`wire_spec`] 同居一文件。）
    pub fn to_wire_json(&self) -> String {
        use serde_json::{Value, json};

        let video_frames: Vec<Value> = self
            .video_frames
            .iter()
            .filter_map(|r| match r {
                ImageRef::VideoFrame {
                    asset_id,
                    time_micros,
                } => Some(json!({
                    "assetId": asset_id,
                    "timeMicros": time_micros,
                })),
                _ => None,
            })
            .collect();
        let images: Vec<Value> = self
            .images
            .iter()
            .filter_map(|r| match r {
                ImageRef::Static { asset_id } => Some(json!({ "assetId": asset_id })),
                _ => None,
            })
            .collect();
        let lottie_bundles: Vec<Value> = self
            .lottie_bundles
            .iter()
            .map(|id| json!({ "bundleId": id }))
            .collect();
        let generated_images: Vec<Value> = self
            .generated_images
            .iter()
            .map(|g| {
                json!({
                    "id": g.id.0,
                    "width": g.width,
                    "height": g.height,
                })
            })
            .collect();
        let video_color_matrices: std::collections::BTreeMap<&str, &str> = self
            .video_color_matrices
            .iter()
            .map(|(id, m)| {
                (
                    id.as_str(),
                    match m {
                        crate::media::VideoColorMatrix::Bt601 => "bt601",
                        crate::media::VideoColorMatrix::Bt709 => "bt709",
                    },
                )
            })
            .collect();
        json!({
            "videoFrames": video_frames,
            "images": images,
            "lottieBundles": lottie_bundles,
            "generatedImages": generated_images,
            "videoColorMatrices": video_color_matrices,
        })
        .to_string()
    }
}

#[test]
fn wire_json_matches_spec() {
    let mut plan = FrameMediaPlan::default();
    plan.images.push(ImageRef::Static {
        asset_id: "img".into(),
    });
    plan.video_frames.push(ImageRef::VideoFrame {
        asset_id: "vid".into(),
        time_micros: 1234,
    });
    plan.lottie_bundles.push("lot".into());
    plan.generated_images.push(FrameGeneratedImage {
        id: GeneratedImageId(7),
        width: 2,
        height: 3,
        rgba: Arc::from([] as [u8; 0]),
        gpu: None,
    });
    plan.video_color_matrices.insert(
        "vid".into(),
        crate::media::VideoColorMatrix::Bt709,
    );

    let v: serde_json::Value = serde_json::from_str(&plan.to_wire_json()).unwrap();
    let obj = v.as_object().expect("wire JSON 顶层是对象");
    let mut keys: Vec<_> = obj.keys().map(|k| k.as_str()).collect();
    let mut want: Vec<_> = wire_spec::FIELDS.iter().map(|(k, _)| *k).collect();
    keys.sort_unstable();
    want.sort_unstable();
    assert_eq!(keys, want, "顶层键集与 wire_spec 漂移");
    // 内层键抽查：与 spec 的 TS 文本对齐（assetId/timeMicros 命名）。
    assert_eq!(obj["videoFrames"][0]["assetId"], "vid");
    assert_eq!(obj["videoFrames"][0]["timeMicros"], 1234);
    assert_eq!(obj["images"][0]["assetId"], "img");
    assert_eq!(obj["lottieBundles"][0]["bundleId"], "lot");
    assert_eq!(obj["generatedImages"][0]["id"], 7);
    assert_eq!(obj["videoColorMatrices"]["vid"], "bt709");
}
