//! 离屏 canvas 的脚本期 Skia 光栅化。
//!
//! `document.createElement('canvas')` 的 2D 命令流由 core
//! `compile_offscreen_commands` 折叠成规范 `DrawOpFrame` 后到这里：Skia
//! `surfaces::raster` 离屏重放（与 `fill_pattern_skia` / mask 同一条
//! `replay_op` 执行链），`read_pixels` 回读 premul RGBA，折回 unpremul
//! （store 的 `GeneratedImageEntry.rgba` 契约）。
//!
//! 媒体面：离屏流里可能引用 generated image（putImageData 产物）与
//! encoded bytes（drawImage(Image) 的 ScriptImage 展开）。generated 从
//! store 的 pending 表按 id 取；encoded 用 `MediaContext::encoded_image`
//! 同款解码。重放完毕的像素经 `register_frame_generated_image` 注册，
//! 主帧用标准 ImageRect{Generated} 消费——零新 IR op。

use std::collections::HashMap;

use skia_safe::{AlphaType, ColorType, Data, ImageInfo, surfaces};

use opencat_core::ir::draw_frame::DrawOpFrame;
use opencat_core::ir::draw_op::DrawOp;
use opencat_core::ir::draw_types::{BytesRangeId, ImageRef};
use opencat_core::ir::GeneratedImageId;

use crate::media::video::MediaContext;
use crate::render::executor::EngineDrawExecutor;
use crate::render::executor::replay::replay_frame;
use crate::render::executor::EnginePreparedFrameMedia;

/// 离屏光栅化错误（engine 绑定层转成 JS 异常文案）。
pub type OffscreenError = String;

/// 一次离屏重放的媒体准备：generated id → Skia image 的窄索引。
struct OffscreenMedia {
    inner: EnginePreparedFrameMedia,
}

impl OffscreenMedia {
    fn build(frame: &DrawOpFrame, store: &mut opencat_core::script::recorder::MutationStore) -> Self {
        let mut video = MediaContext::default();
        let mut images = Vec::new();
        let mut index = HashMap::new();
        let mut scales = Vec::new();

        // byte table（ScriptImage 内联时 intern 进来）→ Encoded 解码。
        for (start, range) in frame.byte_ranges.iter().enumerate() {
            let begin = range.start as usize;
            let end = begin + range.len as usize;
            let bytes = frame
                .bytes
                .get(begin..end)
                .unwrap_or(&frame.bytes[start..start]);
            let key = opencat_core::ir::draw_types::BytesRangeId(start as u32);
            let image_ref = ImageRef::Encoded { data: key };
            if index.contains_key(&image_ref) {
                continue;
            }
            if let Ok(image) = video.encoded_image(key, bytes) {
                let idx = images.len();
                images.push(image);
                scales.push((1.0, 1.0));
                index.insert(image_ref, idx);
            }
        }

        // 流里引用的 generated image（putImageData 产物等）：id 从 ops 收集，
        // 像素从 store 的 pending 表取（record 时刚注册，必定还在）。
        let mut needed: Vec<GeneratedImageId> = Vec::new();
        collect_generated_ids(&frame.ops, &mut needed);
        for subtree in &frame.subtrees {
            collect_generated_ids(subtree, &mut needed);
        }
        for id in needed {
            let image_ref = ImageRef::Generated { id };
            if index.contains_key(&image_ref) {
                continue;
            }
            let Some((w, h, rgba)) = store.pending_generated_image(&id) else {
                continue;
            };
            let info = ImageInfo::new(
                (w as i32, h as i32),
                ColorType::RGBA8888,
                AlphaType::Unpremul,
                None,
            );
            let Some(image) = skia_safe::images::raster_from_data(
                &info,
                Data::new_copy(&*rgba),
                w as usize * 4,
            ) else {
                continue;
            };
            let idx = images.len();
            images.push(image);
            scales.push((1.0, 1.0));
            index.insert(image_ref, idx);
        }

        OffscreenMedia {
            inner: EnginePreparedFrameMedia {
                images,
                image_index: index,
                runtime_effects: Vec::new(),
                image_scales: scales,
            },
        }
    }
}

fn collect_generated_ids(ops: &[DrawOp], out: &mut Vec<GeneratedImageId>) {
    for op in ops {
        match op {
            DrawOp::Image { image, .. } | DrawOp::ImageRect { image, .. } => {
                if let ImageRef::Generated { id } = image {
                    out.push(*id);
                }
            }
            _ => {}
        }
    }
}

/// 折叠后的 DrawOpFrame 重放进 (w, h) 离屏面，回读 unpremul RGBA。
pub fn rasterize_offscreen(
    frame: &DrawOpFrame,
    width: i32,
    height: i32,
    store: &mut opencat_core::script::recorder::MutationStore,
) -> Result<Vec<u8>, OffscreenError> {
    if width <= 0 || height <= 0 {
        return Err(format!("offscreen rasterize: bad size {width}x{height}"));
    }
    let media = OffscreenMedia::build(frame, store);

    let info = ImageInfo::new(
        (width, height),
        skia_safe::ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );
    let Some(mut surface) = surfaces::raster(&info, None, None) else {
        return Err("offscreen rasterize: failed to allocate raster surface".into());
    };
    let mut exec = EngineDrawExecutor::new();
    // `new()` 的 current_stroke_paint 是 Paint::default()（Fill 样式）；
    // 描边样式只在 begin_frame 里初始化。不调它，StrokePath 会按 Fill
    // 重放——开放折线面积为零，整条描边静默消失（pattern 斜线纹理全丢）。
    exec.begin_frame();
    {
        let canvas = surface.canvas();
        canvas.clear(skia_safe::Color4f::new(0.0, 0.0, 0.0, 0.0));
        crate::render::executor::replay::replay_frame(
            &mut exec,
            canvas,
            frame,
            &media.inner,
        )
        .map_err(|e| format!("offscreen rasterize: {e}"))?;
    }
    let mut pix = vec![0_u8; info.compute_min_byte_size()];
    if !surface.read_pixels(&info, &mut pix, info.min_row_bytes(), (0, 0)) {
        return Err("offscreen rasterize: read_pixels failed".into());
    }
    // read_pixels 拿到的是 premul（与主帧一致）；GeneratedImageEntry.rgba
    // 契约是 unpremul，折回去。
    for px in pix.chunks_exact_mut(4) {
        let a = px[3] as u32;
        if a == 0 {
            px[..3].fill(0);
        } else if a != 255 {
            for ch in &mut px[..3] {
                *ch = ((*ch as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    Ok(pix)
}

/// 离屏光栅化结果的内容寻址缓存（最近 [`RASTER_MEMO_CAP`] 条）。逐帧重放
/// 的页面每帧重画同一张画布再读回（three `CanvasTexture` 上传），命令流
/// 不变时像素必然不变。
const RASTER_MEMO_CAP: usize = 4;

thread_local! {
    static RASTER_MEMO: std::cell::RefCell<std::collections::VecDeque<(u64, std::sync::Arc<Vec<u8>>)>> =
        Default::default();
}

/// 便捷封装：store 里的离屏命令流 → 折叠 → 光栅化 → RGBA 字节。
pub fn rasterize_canvas_commands(
    canvas: &opencat_core::script::mutations::CanvasMutations,
    width: f32,
    height: f32,
    store: &mut opencat_core::script::recorder::MutationStore,
) -> Result<(u32, u32, Vec<u8>), OffscreenError> {
    use std::hash::{Hash, Hasher};
    let w = width.max(1.0) as i32;
    let h = height.max(1.0) as i32;
    let frame =
        opencat_core::render_offscreen::compile_offscreen_commands(canvas, w as f32, h as f32)
            .map_err(|e| format!("offscreen compile: {e}"))?;
    // 键 = 命令流 + 尺寸 + 引用的 generated 像素（generated 按 id 引用，
    // id 不担保内容，像素本身进键）。
    let mut hasher = std::hash::DefaultHasher::new();
    canvas.commands.hash(&mut hasher);
    (w, h).hash(&mut hasher);
    let mut needed = Vec::new();
    collect_generated_ids(&frame.ops, &mut needed);
    for subtree in &frame.subtrees {
        collect_generated_ids(subtree, &mut needed);
    }
    for id in &needed {
        if let Some((iw, ih, rgba)) = store.pending_generated_image(id) {
            (iw, ih, &rgba[..]).hash(&mut hasher);
        }
    }
    let key = hasher.finish();
    let hit = RASTER_MEMO.with(|memo| {
        let mut memo = memo.borrow_mut();
        let pos = memo.iter().position(|(k, _)| *k == key)?;
        let entry = memo.remove(pos)?;
        memo.push_front(entry.clone());
        Some(entry.1)
    });
    if let Some(pix) = hit {
        return Ok((w as u32, h as u32, pix.to_vec()));
    }
    let pix = rasterize_offscreen(&frame, w, h, store)?;
    RASTER_MEMO.with(|memo| {
        let mut memo = memo.borrow_mut();
        memo.push_front((key, std::sync::Arc::new(pix.clone())));
        memo.truncate(RASTER_MEMO_CAP);
    });
    Ok((w as u32, h as u32, pix))
}

/// unpremul RGBA → PNG 字节（内存编码，`write_rgba_png` 的 to_writer 变体：
/// 先 premul 化——image crate 的 PNG 按 straight alpha 写，这里输入已是
/// straight/unpremul，直接 from_raw 编码即可）。
pub fn encode_rgba_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, OffscreenError> {
    use image::ImageEncoder;
    let expected = width as usize * height as usize * 4;
    if rgba.len() != expected {
        return Err(format!(
            "png encode: byte mismatch {expected} expected, got {}",
            rgba.len()
        ));
    }
    let mut out = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(std::io::Cursor::new(&mut out));
    encoder
        .write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|e| format!("png encode: {e}"))?;
    Ok(out)
}

/// 便捷封装：store 离屏命令流 → 光栅化 → PNG 字节。
pub fn rasterize_canvas_commands_png(
    canvas: &opencat_core::script::mutations::CanvasMutations,
    width: f32,
    height: f32,
    store: &mut opencat_core::script::recorder::MutationStore,
) -> Result<(u32, u32, Vec<u8>), OffscreenError> {
    let (w, h, pix) = rasterize_canvas_commands(canvas, width, height, store)?;
    let png = encode_rgba_png(w, h, &pix)?;
    Ok((w, h, png))
}
