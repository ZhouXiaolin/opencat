use std::path::Path;

use anyhow::{Result, anyhow};
use skia_safe::{AlphaType, ColorType, ImageInfo, image::CachingHint, surfaces};

pub use crate::media::Mp4Config;

use crate::media::{AudioTrack, MediaContext, decode_audio_to_f32_stereo, encode_rgba_frames};
use opencat_core::frame_ctx::duration_secs_to_frames;
use opencat_core::ir::GeneratedImageId;
use skia_safe::Image;
use std::collections::HashMap;

pub enum OutputFormat {
    Mp4(Mp4Config),
    Png,
}

pub struct EncodingConfig {
    pub format: OutputFormat,
}

/// Render one pipeline frame to RGBA.
///
/// `surface_w/h` is the surface + `read_pixels` size (MP4 may pass even-aligned
/// dims). Creates a fresh surface per call (acceptable cost for the offline path).
/// Generated Skia images are cached by [`GeneratedImageId`] across frames.
fn render_pipeline_frame_to_rgba(
    pipeline: &mut crate::EnginePipeline,
    media_ctx: &mut MediaContext,
    executor: &mut crate::executor::EngineDrawExecutor,
    generated_cache: &mut HashMap<GeneratedImageId, Image>,
    surface_w: u32,
    surface_h: u32,
    frame_index: u32,
) -> Result<Vec<u8>> {
    let render = pipeline.render_frame(frame_index)?;
    let mut frame = render.draw;
    let media_plan = render.media;

    let mut surface = surfaces::raster_n32_premul((surface_w as i32, surface_h as i32))
        .ok_or_else(|| anyhow!("failed to create skia raster surface"))?;

    // SAFETY: skia_safe::Canvas wraps a C++ ref-counted object with interior mutability.
    // All draw methods take &self at the Rust level while mutating internal C++ state.
    // The surface owns the canvas and no other references exist at this point.
    #[allow(invalid_reference_casting)]
    let canvas: &mut skia_safe::Canvas =
        unsafe { &mut *(surface.canvas() as *const skia_safe::Canvas as *mut skia_safe::Canvas) };

    crate::consumer::execute_render_frame(
        &mut frame,
        &media_plan,
        executor,
        pipeline.loader(),
        media_ctx,
        generated_cache,
        canvas,
    )?;

    let image = surface.image_snapshot();
    let image_info = ImageInfo::new(
        (surface_w as i32, surface_h as i32),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );
    let mut rgba = vec![0u8; (surface_w as usize) * (surface_h as usize) * 4];
    let read_ok = image.read_pixels(
        &image_info,
        rgba.as_mut_slice(),
        surface_w as usize * 4,
        (0, 0),
        CachingHint::Allow,
    );
    if !read_ok {
        return Err(anyhow!("failed to read pixels from skia surface"));
    }
    Ok(rgba)
}

/// Premix the whole composition audio track.
/// Shared by `render_from_jsonl` and `opencat-see`.
///
/// Segment timing comes exclusively from the canonical core
/// [`opencat_core::AudioPlan`] on `CompositionInfo::audio_plan` (issue #18 /
/// issue #47). The engine only decodes and mixes — it never re-walks the
/// composition tree to calculate its own segment offsets.
///
/// The plan is already anchored in microsecond precision; the engine converts
/// `start_micros` / `end_micros` directly to sample frames for the mix
/// buffer, so no second derivation of timeline/scene/transition geometry is
/// possible.
pub fn build_audio_track_from_pipeline(
    pipeline: &crate::EnginePipeline,
) -> Result<Option<AudioTrack>> {
    let info = pipeline.info();
    let plan = &info.audio_plan;
    if plan.segments.is_empty() {
        return Ok(None);
    }

    let mut mixed_samples = Vec::new();
    let sample_rate: u32 = 48_000;
    let channels: u16 = 2;
    let frame_count = duration_secs_to_frames(info.duration, info.fps);
    let total_sample_frames = ((frame_count as u64 * sample_rate as u64) / info.fps as u64) as usize;
    mixed_samples.resize(total_sample_frames * channels as usize, 0.0f32);

    for seg in &plan.segments {
        let handle = pipeline
            .loader()
            .handle(&seg.asset)
            .ok_or_else(|| anyhow!("audio asset {:?} not found in loader", seg.asset))?;
        let path = handle
            .local_path()
            .ok_or_else(|| anyhow!("audio {:?}: local_path required", seg.asset))?;
        let clip = decode_audio_to_f32_stereo(path, sample_rate)?;
        let start_us = seg.start_micros().0;
        let end_us = seg.end_micros().0;
        let start_sample = ((start_us * sample_rate as u64) / 1_000_000) as usize;
        let end_sample = ((end_us * sample_rate as u64) / 1_000_000) as usize;
        let copy_frames = end_sample
            .saturating_sub(start_sample)
            .min(clip.sample_frames());
        for i in 0..copy_frames {
            let dst = (start_sample + i) * channels as usize;
            let src = i * channels as usize;
            if dst + 1 < mixed_samples.len() && src + 1 < clip.samples.len() {
                mixed_samples[dst] += clip.samples[src];
                mixed_samples[dst + 1] += clip.samples[src + 1];
            }
        }
    }

    for s in &mut mixed_samples {
        *s = s.clamp(-1.0, 1.0);
    }

    Ok(Some(AudioTrack::new(sample_rate, channels, mixed_samples)))
}

/// Primary render entry: parse JSONL/XML → EnginePipeline → encode frames.
pub fn render_from_jsonl(
    jsonl: &str,
    output_path: impl AsRef<Path>,
    config: &EncodingConfig,
) -> Result<()> {
    render_from_jsonl_with_base(jsonl, None, output_path, config)
}

pub fn render_from_jsonl_with_base(
    input: &str,
    base_dir: Option<&Path>,
    output_path: impl AsRef<Path>,
    config: &EncodingConfig,
) -> Result<()> {
    use crate::js_context::RqJsContext;
    use opencat_core::pipeline::Pipeline;
    use opencat_core::script::js_context::JsContext;

    let output_path = output_path.as_ref();
    let cache_base =
        dirs::home_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let cache_dir = cache_base.join(".opencat").join("assets");

    let loader = crate::resource::loader::EngineLoader::new(
        base_dir.unwrap_or(&cache_base).to_path_buf(),
        cache_dir,
    )?;
    let ctx = RqJsContext::new()?;
    let mut pipeline = crate::pipeline::open(input, loader, ctx)?;
    let info = pipeline.info().clone();
    let frame_count = duration_secs_to_frames(info.duration, info.fps);

    let mut media_ctx = MediaContext::new();
    media_ctx.set_composition_fps(info.fps);
    let mut generated_cache: HashMap<GeneratedImageId, Image> = HashMap::new();

    let audio_track = build_audio_track_from_pipeline(&pipeline)?;

    match &config.format {
        OutputFormat::Png => {
            let mut executor = crate::executor::EngineDrawExecutor::new();
            for i in 0..frame_count {
                let rgba = render_pipeline_frame_to_rgba(
                    &mut pipeline,
                    &mut media_ctx,
                    &mut executor,
                    &mut generated_cache,
                    info.width,
                    info.height,
                    i,
                )?;
                let img = image::RgbaImage::from_raw(info.width, info.height, rgba)
                    .ok_or_else(|| anyhow!("failed to build PNG image from RGBA frame"))?;
                let filename = output_path.join(format!("frame_{:04}.png", i));
                img.save(&filename)?;
            }
        }
        OutputFormat::Mp4(mp4_config) => {
            let aligned_info = if info.width % 2 != 0 || info.height % 2 != 0 {
                let aw = (info.width + 1) & !1;
                let ah = (info.height + 1) & !1;
                (aw, ah)
            } else {
                (info.width, info.height)
            };

            let mut executor = crate::executor::EngineDrawExecutor::new();
            encode_rgba_frames(
                output_path,
                aligned_info.0,
                aligned_info.1,
                info.fps,
                frame_count,
                mp4_config,
                audio_track.as_ref(),
                |_, _| {},
                |frame_index| {
                    render_pipeline_frame_to_rgba(
                    &mut pipeline,
                    &mut media_ctx,
                    &mut executor,
                    &mut generated_cache,
                    aligned_info.0,
                    aligned_info.1,
                    frame_index,
                )
                },
            )?;
        }
    }

    Ok(())
}

/// Render a single frame from JSONL via EnginePipeline, returning raw RGBA bytes.
pub fn render_single_frame_from_jsonl(
    jsonl: &str,
    frame_index: u32,
) -> Result<(Vec<u8>, u32, u32)> {
    render_single_frame_from_jsonl_with_base(jsonl, None, frame_index)
}

/// Render a single frame with explicit base directory for resolving relative asset paths.
pub fn render_single_frame_from_jsonl_with_base(
    input: &str,
    base_dir: Option<&Path>,
    frame_index: u32,
) -> Result<(Vec<u8>, u32, u32)> {
    use crate::js_context::RqJsContext;
    use opencat_core::pipeline::Pipeline;
    use opencat_core::script::js_context::JsContext;

    let cache_base =
        dirs::home_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let cache_dir = cache_base.join(".opencat").join("assets");

    let loader = crate::resource::loader::EngineLoader::new(
        base_dir.unwrap_or(&cache_base).to_path_buf(),
        cache_dir,
    )?;
    let ctx = RqJsContext::new()?;
    let mut pipeline = crate::pipeline::open(input, loader, ctx)?;
    let info = pipeline.info().clone();
    let frame_count = duration_secs_to_frames(info.duration, info.fps);

    if frame_index >= frame_count {
        anyhow::bail!(
            "frame_index {} out of range (composition has {} frames)",
            frame_index,
            frame_count
        );
    }

    let mut media_ctx = MediaContext::new();
    media_ctx.set_composition_fps(info.fps);
    let mut executor = crate::executor::EngineDrawExecutor::new();
    let mut generated_cache: HashMap<GeneratedImageId, Image> = HashMap::new();
    let rgba = render_pipeline_frame_to_rgba(
                    &mut pipeline,
                    &mut media_ctx,
                    &mut executor,
                    &mut generated_cache,
                    info.width,
                    info.height,
                    frame_index,
                )?;

    Ok((rgba, info.width, info.height))
}

pub fn render_single_frame_png_with_base(
    input: &str,
    base_dir: Option<&Path>,
    output_path: impl AsRef<Path>,
    frame_index: u32,
) -> Result<()> {
    let output_path = output_path.as_ref();
    let (rgba, width, height) =
        render_single_frame_from_jsonl_with_base(input, base_dir, frame_index)?;
    write_rgba_png(output_path, width, height, rgba)
}

pub fn render_single_frame_png(
    input: &str,
    output_path: impl AsRef<Path>,
    frame_index: u32,
) -> Result<()> {
    render_single_frame_png_with_base(input, None, output_path, frame_index)
}

fn write_rgba_png(output_path: &Path, width: u32, height: u32, rgba: Vec<u8>) -> Result<()> {
    if let Some(parent) = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }

    let image = image::RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| anyhow!("failed to build PNG image from RGBA frame"))?;
    image.save(output_path)?;
    Ok(())
}

impl EncodingConfig {
    pub fn mp4() -> Self {
        Self {
            format: OutputFormat::Mp4(Mp4Config::default()),
        }
    }

    pub fn mp4_with(config: Mp4Config) -> Self {
        Self {
            format: OutputFormat::Mp4(config),
        }
    }

    pub fn png() -> Self {
        Self {
            format: OutputFormat::Png,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::render_pipeline_frame_to_rgba;
    use crate::media::MediaContext;
    use opencat_core::parse::composition::Composition;
    use crate::EnginePipeline;
    use opencat_core::frame_ctx::duration_secs_to_frames;
    use opencat_core::ir::GeneratedImageId;
    use opencat_core::parse::{ParsedComposition, node::Node};
    use opencat_core::pipeline::Pipeline;
    use opencat_core::fonts::FontManifest;
    use opencat_core::script::js_context::JsContext;
    use skia_safe::Image;
    use std::collections::HashMap;

    struct TestPipeline {
        pipeline: EnginePipeline,
        media_ctx: MediaContext,
        executor: crate::executor::EngineDrawExecutor,
        generated_cache: HashMap<GeneratedImageId, Image>,
        width: u32,
        height: u32,
        _fixture_dir: std::path::PathBuf,
    }

    impl TestPipeline {
        fn render(&mut self, frame_index: u32) -> anyhow::Result<Vec<u8>> {
            render_pipeline_frame_to_rgba(
                    &mut self.pipeline,
                    &mut self.media_ctx,
                    &mut self.executor,
                    &mut self.generated_cache,
                    self.width,
                    self.height,
                    frame_index,
                )
        }
    }

    impl Drop for TestPipeline {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self._fixture_dir);
        }
    }

    fn frames_at_30fps(frames: u32) -> f64 {
        frames as f64 / 30.0
    }

    fn make_test_pipeline_from_scene(
        scene: impl Into<Node>,
        width: i32,
        height: i32,
        fps: u32,
        duration: f64,
    ) -> TestPipeline {
        make_test_pipeline_from_node(scene.into(), width, height, fps, duration)
    }

    fn make_test_pipeline_from_node(
        root: Node,
        width: i32,
        height: i32,
        fps: u32,
        duration: f64,
    ) -> TestPipeline {
        let parsed = ParsedComposition {
            width,
            height,
            fps: fps as i32,
            duration,
            root,
            script: None,
            audio_sources: vec![],
            font_manifest: FontManifest::default(),
        };
        open_test_pipeline(parsed, width as u32, height as u32, fps, duration)
    }

    fn open_test_pipeline(
        parsed: ParsedComposition,
        width: u32,
        height: u32,
        fps: u32,
        _duration: f64,
    ) -> TestPipeline {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let fixture_dir = std::path::PathBuf::from(format!(
            "target/opencat-render-test-{}-{}",
            std::process::id(),
            nanos
        ));
        let cache = fixture_dir.join("cache");
        std::fs::create_dir_all(&cache).expect("test cache dir");
        // Caption nodes with path "sub.srt" still trigger loader preload even when
        // entries are inlined; seed a placeholder so the host-owned open's
        // `load_all` succeeds and `hydrate_captions` parses it.
        std::fs::write(
            fixture_dir.join("sub.srt"),
            "1\n00:00:00,000 --> 00:00:01,000\n\n",
        )
        .expect("seed sub.srt");
        let loader = crate::resource::loader::EngineLoader::new(fixture_dir.clone(), cache)
            .expect("loader");
        let ctx = crate::js_context::RqJsContext::new().expect("js context");
        // Open through the host-owned chain (fetch/cache → build_catalog →
        // hydrate captions → open_pipeline), the same path the
        // engine uses in production. No open_parsed / loader_mut here.
        let pipeline = crate::pipeline::open_parsed_host_owned(
            parsed,
            loader,
            ctx,
            crate::fonts::engine_default_font_faces(),
        )
        .expect("pipeline");
        let mut media_ctx = MediaContext::new();
        media_ctx.set_composition_fps(fps);
        TestPipeline {
            pipeline,
            media_ctx,
            executor: crate::executor::EngineDrawExecutor::new(),
            generated_cache: HashMap::new(),
            width,
            height,
            _fixture_dir: fixture_dir,
        }
    }

    fn write_test_png(path: &std::path::Path) {
        let mut image = image::RgbaImage::new(2, 1);
        image.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        image.put_pixel(1, 0, image::Rgba([0, 255, 0, 255]));
        image.save(path).expect("test image should save");
    }

    fn pixel_rgba(frame: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
        let index = (y * width + x) * 4;
        [
            frame[index],
            frame[index + 1],
            frame[index + 2],
            frame[index + 3],
        ]
    }

    fn has_bright_pixel_in_rect(
        frame: &[u8],
        width: usize,
        left: usize,
        top: usize,
        rect_width: usize,
        rect_height: usize,
    ) -> bool {
        for y in top..top + rect_height {
            for x in left..left + rect_width {
                let px = pixel_rgba(frame, width, x, y);
                if px[0] > 180 && px[1] > 180 && px[2] > 180 {
                    return true;
                }
            }
        }
        false
    }

    fn dark_pixel_count_in_rect(
        frame: &[u8],
        width: usize,
        left: usize,
        top: usize,
        rect_width: usize,
        rect_height: usize,
    ) -> usize {
        let mut count = 0;
        for y in top..top + rect_height {
            for x in left..left + rect_width {
                let px = pixel_rgba(frame, width, x, y);
                if px[0] < 80 && px[1] < 80 && px[2] < 80 && px[3] > 200 {
                    count += 1;
                }
            }
        }
        count
    }

    #[test]
    fn bold_amount_text_renders_every_ascii_glyph() {
        let amount = "¥12,846.53";
        let text_style = opencat_core::style::ComputedTextStyle {
            text_px: 22.0,
            font_weight: opencat_core::style::FontWeight::BOLD,
            ..Default::default()
        };
        let scene = crate::div().id("root").w_full().h_full().bg_white().child(
            crate::text(amount)
                .id("amount")
                .absolute()
                .left(12.0)
                .top(8.0)
                .w(220.0)
                .h(42.0)
                .text_px(22.0)
                .font_weight(opencat_core::style::FontWeight::BOLD)
                .text_color(crate::ColorToken::Black),
        );

        let mut pipeline = make_test_pipeline_from_scene(scene, 260, 70, 30, frames_at_30fps(1));
        let frame = pipeline.render(0).expect("frame should render");

        let font_db = crate::fonts::engine_default_font_db();
        let raster = opencat_core::text::rasterize_glyphs(
            amount,
            &text_style,
            f32::INFINITY,
            false,
            false,
            font_db.as_ref(),
        );
        for line in &raster.lines {
            let mut missing = Vec::new();
            for (index, pos) in line.positions.iter().enumerate() {
                let label = &amount[pos.byte_range.clone()];
                let left = (12.0 + pos.x).floor().max(0.0) as usize;
                let next_x = line
                    .positions
                    .get(index + 1)
                    .map(|next| 12.0 + next.x)
                    .unwrap_or(line.width + 12.0);
                let window_width = (next_x.ceil().max(left as f32 + 4.0) as usize - left).max(4);
                let count = dark_pixel_count_in_rect(&frame, 260, left, 12, window_width, 28);
                if count <= 4 {
                    missing.push(format!("{label}({count})"));
                }
            }
            assert!(
                missing.is_empty(),
                "glyphs should contribute visible dark pixels: {}",
                missing.join(", ")
            );
        }
    }

    /// Issue #9: color-emoji glyphs must flow through the generated-image
    /// path, not be dropped. Before #9, `render_text` discarded the RGBA and
    /// emitted a synthetic `glyph:*` `ImageRef::Static` that the loader could
    /// never resolve — so emoji rendered as nothing. This test proves the table
    /// is populated AND the engine resolved the generated image to visible
    /// colorful pixels (emoji are multi-colored, unlike grayscale text).
    #[test]
    fn color_emoji_glyphs_render_via_generated_image_table() {
        let emoji = "😀";
        let scene = crate::div().id("root").w_full().h_full().bg_white().child(
            crate::text(emoji)
                .id("face")
                .absolute()
                .left(8.0)
                .top(4.0)
                .w(64.0)
                .h(64.0)
                .text_px(48.0)
                .text_color(crate::ColorToken::Black),
        );

        let mut pipeline =
            make_test_pipeline_from_scene(scene, 96, 80, 30, frames_at_30fps(1));

        // Sanity: emoji rasterizes to a ColorImage glyph with the engine font db.
        let font_db = crate::fonts::engine_default_font_db();
        let raster = opencat_core::text::rasterize_glyphs(
            emoji,
            &opencat_core::style::ComputedTextStyle {
                text_px: 48.0,
                ..Default::default()
            },
            f32::INFINITY,
            false,
            false,
            font_db.as_ref(),
        );
        let has_color_glyph = raster
            .glyphs
            .values()
            .any(|d| matches!(d, opencat_core::text::GlyphData::ColorImage { .. }));
        assert!(
            has_color_glyph,
            "test precondition: NotoColorEmoji must rasterize 😀 as ColorImage"
        );

        let frame = pipeline.render(0).expect("frame should render");

        // AC: RenderFrame media plan carries full generated-image RGBA (id/size/bytes).
        {
            let render = pipeline
                .pipeline
                .render_frame(0)
                .expect("render_frame for generated-image plan");
            assert!(
                !render.media.generated_images.is_empty(),
                "FrameMediaPlan must carry the emoji glyph RGBA on RenderFrame"
            );
            for g in &render.media.generated_images {
                assert!(g.width > 0 && g.height > 0, "generated image must have size");
                assert_eq!(
                    g.rgba.len(),
                    g.width as usize * g.height as usize * 4,
                    "RGBA length must match width*height*4"
                );
            }
        }

        // AC#6: the engine resolved the generated image to a Skia image and
        // drew it — emoji produces colorful (saturated) pixels, unlike grayscale
        // outline text. Scan the whole frame for any saturated, opaque pixel.
        let colorful = (0..80)
            .flat_map(|y| (0..96).map(move |x| (x, y)))
            .filter(|(x, y)| is_colorful_opaque(&frame, 96, *x, *y))
            .count();
        assert!(
            colorful > 8,
            "emoji should contribute colorful pixels to the frame, found {colorful}"
        );
    }

    fn is_colorful_opaque(frame: &[u8], width: usize, x: usize, y: usize) -> bool {
        let px = pixel_rgba(frame, width, x, y);
        if px[3] < 200 {
            return false;
        }
        let max = px[0].max(px[1]).max(px[2]);
        let min = px[0].min(px[1]).min(px[2]);
        // Saturated color: emoji yellows/greens/reds have a wide channel spread;
        // grayscale text (even anti-aliased) stays narrow.
        max.saturating_sub(min) > 40
    }

    #[test]
    fn subtree_cache_does_not_apply_node_opacity_twice() {
        let scene = crate::div()
            .id("root")
            .w_full()
            .h_full()
            .bg_black()
            .script_source(r#"ctx.getNode("box").opacity(ctx.frame === 0 ? 1 : 0.5);"#)
            .expect("script should compile")
            .child(
                crate::div()
                    .id("box")
                    .absolute()
                    .left(0.0)
                    .top(0.0)
                    .w(10.0)
                    .h(10.0)
                    .bg_white(),
            );

        let mut pipeline = make_test_pipeline_from_scene(scene, 20, 20, 30, frames_at_30fps(2));
        let first = pipeline.render(0).expect("frame 0 should render");
        let second = pipeline.render(1).expect("frame 1 should render");

        let first_pixel = pixel_rgba(&first, 20, 5, 5);
        let second_pixel = pixel_rgba(&second, 20, 5, 5);

        assert!(first_pixel[0] >= 250, "frame 0 should stay fully white");
        assert!(
            (120..=136).contains(&second_pixel[0]),
            "frame 1 should be roughly 50% white, got {:?}",
            second_pixel
        );
    }

    #[test]
    fn split_text_gsap_api_renders_text_property_layer() {
        let jsonl_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("examples/split_text_demo.jsonl");
        let jsonl_text = std::fs::read_to_string(&jsonl_path).expect("read jsonl");
        let base_dir = jsonl_path.parent().unwrap().to_path_buf();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let fixture_dir = std::path::PathBuf::from(format!(
            "target/opencat-render-test-split-{}-{}",
            std::process::id(),
            nanos
        ));
        let cache = fixture_dir.join("cache");
        std::fs::create_dir_all(&cache).expect("cache");
        // Prefer the example's directory as loader base so relative assets resolve.
        let loader = crate::resource::loader::EngineLoader::new(base_dir.clone(), cache)
            .expect("loader");
        let ctx = crate::js_context::RqJsContext::new().expect("js");
        let mut pipeline = crate::pipeline::open(&jsonl_text, loader, ctx).expect("pipeline");
        let info = pipeline.info().clone();
        let frames = duration_secs_to_frames(info.duration, info.fps);
        let mut media_ctx = MediaContext::new();
        media_ctx.set_composition_fps(info.fps);
        let mut executor = crate::executor::EngineDrawExecutor::new();
        let mut generated_cache: HashMap<GeneratedImageId, Image> = HashMap::new();
        let frame = render_pipeline_frame_to_rgba(
                    &mut pipeline,
                    &mut media_ctx,
                    &mut executor,
                    &mut generated_cache,
                    info.width,
                    info.height,
                    100,
                )
        .expect("frame should render");
        let _ = std::fs::remove_dir_all(&fixture_dir);

        assert!(
            has_bright_pixel_in_rect(&frame, info.width as usize, 120, 330, 420, 130),
            "chars text should be visible after splitText property-layer animation settles"
        );
        assert!(
            has_bright_pixel_in_rect(&frame, info.width as usize, 760, 330, 420, 130),
            "words text should be visible after splitText property-layer animation settles"
        );
    }

    #[test]
    fn subtree_cache_preserves_shadow_outside_node_bounds_during_opacity_animation() {
        let scene = crate::div()
            .id("root")
            .w_full()
            .h_full()
            .bg_white()
            .script_source(r#"ctx.getNode("box").opacity(ctx.frame === 0 ? 1 : 0.5);"#)
            .expect("script should compile")
            .child(
                crate::div()
                    .id("box")
                    .absolute()
                    .left(10.0)
                    .top(10.0)
                    .w(20.0)
                    .h(8.0)
                    .rounded_full()
                    .bg(crate::ColorToken::Red500)
                    .shadow_lg(),
            );

        let mut pipeline = make_test_pipeline_from_scene(scene, 40, 40, 30, frames_at_30fps(2));
        let first = pipeline.render(0).expect("frame 0 should render");
        let second = pipeline.render(1).expect("frame 1 should render");

        let first_shadow = pixel_rgba(&first, 40, 20, 22);
        let second_shadow = pixel_rgba(&second, 40, 20, 22);

        assert!(
            first_shadow[0] < 245 || first_shadow[1] < 245 || first_shadow[2] < 245,
            "frame 0 should contain shadow outside the node bounds, got {:?}",
            first_shadow
        );
        assert!(
            second_shadow[0] < 250 || second_shadow[1] < 250 || second_shadow[2] < 250,
            "frame 1 should keep the shadow visible instead of clipping back to background, got {:?}",
            second_shadow
        );
    }

    #[test]
    fn display_list_and_subtree_cache_both_preserve_overflow_clipping() {
        let scene = crate::div()
            .id("root")
            .w_full()
            .h_full()
            .bg(crate::ColorToken::Black)
            .script_source(r#"ctx.getNode("mover").translateY(ctx.frame);"#)
            .expect("script should compile")
            .child(
                crate::div()
                    .id("card")
                    .absolute()
                    .left(4.0)
                    .top(4.0)
                    .w(12.0)
                    .h(12.0)
                    .rounded(6.0)
                    .overflow_hidden()
                    .child(
                        crate::div()
                            .id("card-fill")
                            .w_full()
                            .h_full()
                            .bg(crate::ColorToken::White),
                    ),
            )
            .child(
                crate::div()
                    .id("mover")
                    .absolute()
                    .left(0.0)
                    .top(0.0)
                    .w(2.0)
                    .h(2.0)
                    .bg(crate::ColorToken::Red500),
            );

        let mut pipeline = make_test_pipeline_from_scene(scene, 24, 24, 30, frames_at_30fps(2));
        let first = pipeline.render(0).expect("frame 0 should render");
        let second = pipeline.render(1).expect("frame 1 should render");

        assert_eq!(
            pixel_rgba(&first, 24, 4, 4),
            [0, 0, 0, 255],
            "frame 0 should keep the clipped corner transparent to the black background"
        );
        assert_eq!(
            pixel_rgba(&second, 24, 4, 4),
            [0, 0, 0, 255],
            "frame 1 should match frame 0 after subtree caching kicks in"
        );
    }

    #[test]
    fn canvas_node_draw_image_uses_asset_alias_in_backend() {
        let image_path =
            std::env::temp_dir().join(format!("opencat-canvas-test-{}.png", std::process::id()));
        write_test_png(&image_path);

        let scene = crate::canvas()
            .id("canvas")
            .size(2.0, 1.0)
            .asset_path("hero", image_path.to_string_lossy().into_owned())
            .script_source(
                r#"
                const CK = ctx.CanvasKit;
                const image = ctx.getImage("hero");
                ctx.getCanvasById('canvas').drawImageRect(
                    image,
                    CK.XYWHRect(0, 0, 2, 1),
                    CK.XYWHRect(0, 0, 2, 1),
                );
                "#,
            )
            .expect("script should compile");

        let mut pipeline = make_test_pipeline_from_scene(scene, 2, 1, 30, frames_at_30fps(1));
        let frame = pipeline.render(0).expect("frame should render");
        let _ = std::fs::remove_file(&image_path);

        assert_eq!(pixel_rgba(&frame, 2, 0, 0), [255, 0, 0, 255]);
        assert_eq!(pixel_rgba(&frame, 2, 1, 0), [0, 255, 0, 255]);
    }

    #[test]
    fn composition_alignment_for_video_encoding_rounds_up_to_even_dimensions() {
        let composition = Composition::new("align")
            .size(3, 5)
            .fps(30)
            .duration(frames_at_30fps(1))
            .root(|_ctx| crate::div().id("root").into())
            .build()
            .expect("composition should build");

        let aligned = composition.aligned_for_video_encoding();
        assert_eq!((aligned.width, aligned.height), (4, 6));

        let even = Composition::new("align-even")
            .size(1280, 720)
            .fps(30)
            .duration(frames_at_30fps(1))
            .root(|_ctx| crate::div().id("root").into())
            .build()
            .expect("composition should build");
        let even_aligned = even.aligned_for_video_encoding();
        assert_eq!((even_aligned.width, even_aligned.height), (1280, 720));
    }

    #[test]
    fn subtree_cache_preserves_rust_driven_scale_animation() {
        // open_parsed freezes the root node; drive per-frame scale via script instead of
        // Composition::root(|ctx| ...).
        let scene = crate::div()
            .id("root")
            .w_full()
            .h_full()
            .bg_black()
            .script_source(
                r#"ctx.getNode("dot").scale(ctx.frame === 0 ? 1 : 2);"#,
            )
            .expect("script should compile")
            .child(
                crate::div()
                    .id("dot")
                    .absolute()
                    .left(8.0)
                    .top(8.0)
                    .w(8.0)
                    .h(8.0)
                    .rounded_full()
                    .bg_white()
                    .scale(1.0),
            );

        let mut pipeline = make_test_pipeline_from_scene(scene, 24, 24, 30, frames_at_30fps(2));
        let first = pipeline.render(0).expect("frame 0 should render");
        let second = pipeline.render(1).expect("frame 1 should render");

        assert_eq!(
            pixel_rgba(&first, 24, 5, 12),
            [0, 0, 0, 255],
            "frame 0 should keep pixels outside the original dot bounds black"
        );
        assert_eq!(
            pixel_rgba(&second, 24, 5, 12),
            [255, 255, 255, 255],
            "frame 1 should expand the dot when scale changes under subtree caching"
        );
    }

    #[test]
    fn subtree_cache_invalidation_tracks_descendant_transform_changes() {
        let scene = crate::div()
            .id("root")
            .w_full()
            .h_full()
            .bg_black()
            .script_source(
                r#"
                ctx.getNode("dot").scale(ctx.frame === 0 ? 1 : 2);
                ctx.getNode("ticker-fill").bg(ctx.frame === 0 ? '#ef4444' : '#3b82f6');
                "#,
            )
            .expect("script should compile")
            .child(
                crate::div()
                    .id("group")
                    .absolute()
                    .left(8.0)
                    .top(8.0)
                    .h(8.0)
                    .w(8.0)
                    .child(
                        crate::div()
                            .id("dot")
                            .w_full()
                            .h_full()
                            .rounded_full()
                            .bg_white()
                            .scale(1.0),
                    ),
            )
            .child(
                crate::div()
                    .id("ticker-fill")
                    .absolute()
                    .left(0.0)
                    .top(0.0)
                    .w(1.0)
                    .h(1.0)
                    .bg(crate::ColorToken::Red500),
            );

        let mut pipeline = make_test_pipeline_from_scene(scene, 24, 24, 30, frames_at_30fps(2));
        let first = pipeline.render(0).expect("frame 0 should render");
        let second = pipeline.render(1).expect("frame 1 should render");

        assert_eq!(
            pixel_rgba(&first, 24, 5, 12),
            [0, 0, 0, 255],
            "frame 0 should keep pixels outside the original dot bounds black"
        );
        assert_eq!(
            pixel_rgba(&second, 24, 5, 12),
            [255, 255, 255, 255],
            "frame 1 should redraw the parent subtree when a descendant transform changes"
        );
    }

    #[test]
    fn layered_caption_renders_above_timeline_transition() {
        use opencat_core::parse::easing::Easing;
        use opencat_core::parse::primitives::{SrtEntry, caption, text};
        use opencat_core::parse::transition::{fade, timeline};

        let root = crate::div()
            .id("root")
            .child(
                timeline()
                    .sequence(
                        frames_at_30fps(10),
                        crate::div()
                            .id("scene-a")
                            .bg(crate::ColorToken::Black)
                            .child(text("A").id("a"))
                            .into(),
                    )
                    .transition(fade().timing(Easing::Linear, frames_at_30fps(5)))
                    .sequence(
                        frames_at_30fps(10),
                        crate::div()
                            .id("scene-b")
                            .bg(crate::ColorToken::Black)
                            .child(text("B").id("b"))
                            .into(),
                    ),
            )
            .child(
                crate::div().id("overlay-root").child(
                    caption()
                        .id("subs")
                        .path("sub.srt")
                        .entries(vec![SrtEntry {
                            index: 1,
                            start_frame: 0,
                            end_frame: 25,
                            text: "Subtitle".into(),
                        }])
                        .text_color(crate::ColorToken::White),
                ),
            );

        let mut pipeline = make_test_pipeline_from_scene(root, 320, 180, 30, frames_at_30fps(25));
        let pixels = pipeline.render(12).expect("frame should render");

        assert!(
            pixels.iter().any(|&byte| byte > 0),
            "transition frame with caption overlay should not be blank"
        );
    }

    #[test]
    fn layered_single_scene_renders_bottom_scene_before_caption_overlay() {
        use opencat_core::parse::primitives::{SrtEntry, caption};

        let root = crate::div()
            .id("root")
            .child(
                crate::div()
                    .id("scene")
                    .w_full()
                    .h_full()
                    .bg(crate::ColorToken::Blue500),
            )
            .child(
                caption()
                    .id("subs")
                    .path("sub.srt")
                    .entries(vec![SrtEntry {
                        index: 1,
                        start_frame: 0,
                        end_frame: 1,
                        text: "Caption".into(),
                    }])
                    .absolute()
                    .left(8.0)
                    .top(8.0)
                    .text_color(crate::ColorToken::White),
            );

        let mut pipeline = make_test_pipeline_from_scene(root, 64, 64, 30, frames_at_30fps(1));
        let pixels = pipeline.render(0).expect("frame should render");

        assert_eq!(
            pixel_rgba(&pixels, 64, 32, 32),
            [43, 127, 255, 255],
            "bottom scene layer should remain visible beneath the caption overlay"
        );
    }

    #[test]
    fn layered_root_caption_without_active_entry_does_not_fail_rendering() {
        use opencat_core::parse::primitives::{SrtEntry, caption};

        let root = crate::div()
            .id("root")
            .child(
                crate::div()
                    .id("scene")
                    .w_full()
                    .h_full()
                    .bg(crate::ColorToken::Blue500),
            )
            .child(
                caption()
                    .id("subs")
                    .path("sub.srt")
                    .entries(vec![SrtEntry {
                        index: 1,
                        start_frame: 30,
                        end_frame: 60,
                        text: "Later".into(),
                    }])
                    .absolute()
                    .left(8.0)
                    .top(8.0)
                    .text_color(crate::ColorToken::White),
            );

        let mut pipeline = make_test_pipeline_from_scene(root, 64, 64, 30, frames_at_30fps(60));
        let pixels = pipeline.render(0).expect("frame should render");

        assert_eq!(
            pixel_rgba(&pixels, 64, 32, 32),
            [43, 127, 255, 255],
            "inactive root caption layer should be skipped while the bottom scene still renders"
        );
    }

    #[test]
    fn timeline_caption_sibling_renders_above_transition() {
        use opencat_core::parse::easing::Easing;
        use opencat_core::parse::primitives::{SrtEntry, caption, text};
        use opencat_core::parse::transition::{fade, timeline};

        let root = crate::div()
            .id("root")
            .child(
                timeline()
                    .sequence(
                        frames_at_30fps(10),
                        crate::div()
                            .id("scene-a")
                            .bg(crate::ColorToken::Black)
                            .child(text("A").id("a"))
                            .into(),
                    )
                    .transition(fade().timing(Easing::Linear, frames_at_30fps(5)))
                    .sequence(
                        frames_at_30fps(10),
                        crate::div()
                            .id("scene-b")
                            .bg(crate::ColorToken::Black)
                            .child(text("B").id("b"))
                            .into(),
                    ),
            )
            .child(
                caption()
                    .id("subs")
                    .path("sub.srt")
                    .entries(vec![SrtEntry {
                        index: 1,
                        start_frame: 0,
                        end_frame: 25,
                        text: "Subtitle".into(),
                    }])
                    .text_color(crate::ColorToken::White),
            );

        let mut pipeline = make_test_pipeline_from_scene(root, 320, 180, 30, frames_at_30fps(25));
        let pixels = pipeline.render(12).expect("frame should render");

        assert!(pixels.iter().any(|&byte| byte > 0));
    }

    fn make_timeline_root_node(
        with_wrapper: bool,
        transition: opencat_core::parse::transition::Transition,
    ) -> Node {
        use opencat_core::parse::transition::timeline;
        use opencat_core::style::{LengthPercentageAuto, Position};

        let mut tl_kind = Node::from(
            timeline()
                .sequence(
                    frames_at_30fps(10),
                    crate::div().id("scene-a").w_full().h_full().bg_red().into(),
                )
                .transition(transition)
                .sequence(
                    frames_at_30fps(10),
                    crate::div()
                        .id("scene-b")
                        .w_full()
                        .h_full()
                        .bg_blue()
                        .into(),
                ),
        )
        .kind()
        .clone();
        let tl_style = tl_kind.style_mut();
        tl_style.id = "tl".into();
        tl_style.overflow_hidden = true;
        if with_wrapper {
            tl_style.position = Some(Position::Absolute);
            tl_style.inset_left = Some(LengthPercentageAuto::Length(0.0));
            tl_style.inset_top = Some(LengthPercentageAuto::Length(0.0));
            tl_style.width = Some(80.0);
            tl_style.height = Some(80.0);
            crate::div()
                .id("root")
                .w_full()
                .h_full()
                .bg_black()
                .child(Node::new(tl_kind))
                .into()
        } else {
            tl_style.width_full = true;
            tl_style.height_full = true;
            Node::new(tl_kind)
        }
    }

    #[test]
    fn nested_timeline_transition_renders_real_composite() {
        use opencat_core::parse::easing::Easing;
        use opencat_core::parse::transition::fade;
        let root = make_timeline_root_node(true, fade().timing(Easing::Linear, frames_at_30fps(10)));
        let mut pipeline = make_test_pipeline_from_node(root, 80, 80, 30, frames_at_30fps(30));
        let pixels = pipeline.render(15).expect("frame should render");
        let pixel = pixel_rgba(&pixels, 80, 40, 40);

        assert!(
            pixel[0] > 0 && pixel[2] > 0,
            "transition pixel should contain both from/to colors, got {:?}",
            pixel
        );
    }

    #[test]
    fn root_timeline_renders_without_root_transition_special_case() {
        use opencat_core::parse::easing::Easing;
        use opencat_core::parse::transition::fade;
        let root =
            make_timeline_root_node(false, fade().timing(Easing::Linear, frames_at_30fps(10)));
        let mut pipeline = make_test_pipeline_from_node(root, 80, 80, 30, frames_at_30fps(30));
        let pixels = pipeline.render(15).expect("frame should render");
        let pixel = pixel_rgba(&pixels, 80, 40, 40);

        assert!(
            pixel[0] > 0 && pixel[2] > 0,
            "root timeline transition should be composited by the tl node itself, got {:?}",
            pixel
        );
    }

    #[test]
    fn gltransition_runtime_effect_samples_timeline_children() {
        use opencat_core::parse::easing::Easing;
        use opencat_core::parse::transition::gl_transition;
        let root = make_timeline_root_node(
            false,
            gl_transition("fade").timing(Easing::Linear, frames_at_30fps(10)),
        );
        let mut pipeline = make_test_pipeline_from_node(root, 80, 80, 30, frames_at_30fps(30));
        let pixels = pipeline.render(15).expect("frame should render");
        let pixel = pixel_rgba(&pixels, 80, 40, 40);

        assert!(
            pixel[0] > 80 && pixel[0] < 220 && pixel[2] > 80 && pixel[2] < 220 && pixel[3] == 255,
            "GLTransition fade should render a real mid-transition blend, got {:?}",
            pixel
        );
    }

    #[test]
    fn light_leak_runtime_effect_samples_timeline_children() {
        use opencat_core::parse::easing::Easing;
        use opencat_core::parse::transition::light_leak;
        let root =
            make_timeline_root_node(false, light_leak().timing(Easing::Linear, frames_at_30fps(10)));
        let mut pipeline = make_test_pipeline_from_node(root, 80, 80, 30, frames_at_30fps(30));
        let pixels = pipeline.render(15).expect("frame should render");
        let pixel = pixel_rgba(&pixels, 80, 40, 40);

        assert!(
            pixel[0] > 120 && pixel[1] > 80 && pixel[3] == 255,
            "light leak transition should produce an opaque warm RuntimeEffect composite, got {:?}",
            pixel
        );
    }

    #[test]
    fn script_can_target_hidden_canvas_descendant() {
        use opencat_core::parse::node::Node;

        let scene = crate::canvas()
            .id("stage")
            .size(32.0, 32.0)
            .hidden_child(Node::new(
                crate::div().id("hidden").w_full().h_full().bg_red(),
            ))
            .script_source(
                r#"
                ctx.getNode('hidden').bg('#00ff00');
                var c = ctx.getCanvasById('stage');
                c.drawPicture(c.getSubTree(), 0, 0);
                "#,
            )
            .expect("script should compile");

        let mut pipeline = make_test_pipeline_from_scene(scene, 32, 32, 30, frames_at_30fps(1));
        let rgba = pipeline.render(0).expect("frame should render");
        assert_eq!(pixel_rgba(&rgba, 32, 16, 16), [0, 255, 0, 255]);
    }

    #[test]
    fn nested_canvas_hidden_children_not_visible_without_explicit_draw_picture() {
        use opencat_core::parse::node::Node;

        let scene = crate::canvas()
            .id("outer")
            .size(32.0, 32.0)
            .hidden_child(Node::new(
                crate::canvas()
                    .id("inner")
                    .size(32.0, 32.0)
                    .hidden_child(Node::new(
                        crate::div().id("inner-child").w_full().h_full().bg_green(),
                    )),
            ))
            .script_source(
                r#"
                var c = ctx.getCanvasById('outer');
                c.drawPicture(c.getSubTree(), 0, 0);
                "#,
            )
            .expect("script should compile");

        let mut pipeline = make_test_pipeline_from_scene(scene, 32, 32, 30, frames_at_30fps(1));
        let rgba = pipeline.render(0).expect("frame should render");
        assert_eq!(pixel_rgba(&rgba, 32, 16, 16), [0, 0, 0, 0]);
    }

    #[test]
    fn indirect_canvas_recursion_returns_error() {
        use opencat_core::parse::node::Node;

        let inner_script = r#"
            ctx.getCanvasById('inner').drawPicture(
                ctx.getCanvasById('outer').getSubTree(), 0, 0
            );
        "#;
        let outer_script = r#"
            var c = ctx.getCanvasById('outer');
            c.drawPicture(c.getSubTree(), 0, 0);
        "#;

        let inner_canvas = crate::canvas()
            .id("inner")
            .size(32.0, 32.0)
            .script_source(inner_script)
            .expect("inner script should compile");

        let scene = crate::canvas()
            .id("outer")
            .size(32.0, 32.0)
            .hidden_child(Node::new(inner_canvas))
            .script_source(outer_script)
            .expect("outer script should compile");

        let mut pipeline = make_test_pipeline_from_scene(scene, 32, 32, 30, frames_at_30fps(1));
        let result = pipeline.render(0);
        assert!(
            result.is_err(),
            "indirect canvas recursion should return an error"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("recursive hidden canvas picture"),
            "error should mention recursive hidden canvas picture, got: {err}"
        );
    }
}
