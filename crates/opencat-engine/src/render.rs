use std::path::Path;

use anyhow::{Result, anyhow};

use crate::{
    codec::decode::AudioTrack,
    resource::media::VideoPreviewQuality,
    runtime::{
        audio::{
            AudioBuffer, build_audio_track as build_runtime_audio_track,
            render_audio_chunk as render_runtime_audio_chunk,
        },
        preflight::ensure_assets_preloaded,
        render_registry,
        target::RenderTargetHandle,
    },
};
use opencat_core::scene::composition::Composition;

pub use crate::codec::encode::Mp4Config;
pub use crate::runtime::session::RenderSession;

pub enum OutputFormat {
    Mp4(Mp4Config),
    Png,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderBackend {
    Software,
    Accelerated,
}

pub struct EncodingConfig {
    pub format: OutputFormat,
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

pub fn render(composition: &Composition, output_path: impl AsRef<Path>, config: &EncodingConfig) -> Result<()> {
    render_with_progress(composition, output_path, config, |_, _| {})
}

pub fn render_with_progress(
    composition: &Composition,
    output_path: impl AsRef<Path>,
    config: &EncodingConfig,
    on_video_frame_encoded: impl FnMut(u32, u32),
) -> Result<()> {
    render_with_backend_progress(
        composition,
        output_path,
        config,
        render_registry::default_render_backend(),
        on_video_frame_encoded,
    )
}

pub fn render_with_backend(
    composition: &Composition,
    output_path: impl AsRef<Path>,
    config: &EncodingConfig,
    backend: RenderBackend,
) -> Result<()> {
    render_with_backend_progress(composition, output_path, config, backend, |_, _| {})
}

pub fn render_with_backend_progress(
    composition: &Composition,
    output_path: impl AsRef<Path>,
    config: &EncodingConfig,
    backend: RenderBackend,
    on_video_frame_encoded: impl FnMut(u32, u32),
) -> Result<()> {
    match &config.format {
        OutputFormat::Mp4(mp4_config) => render_mp4(
            composition,
            output_path,
            mp4_config,
            backend,
            on_video_frame_encoded,
        ),
        OutputFormat::Png => render_png(composition, output_path, backend),
    }
}

pub fn render_frame_with_target(
    composition: &Composition,
    frame_index: u32,
    session: &mut RenderSession,
    target: &mut RenderTargetHandle,
) -> Result<()> {
    render_frame_to_target(composition, frame_index, session, target)
}

pub fn build_audio_track(
    composition: &Composition,
    session: &mut RenderSession,
) -> Result<Option<AudioTrack>> {
    ensure_assets_preloaded(composition, session)?;
    build_runtime_audio_track(
        composition,
        &mut session.assets,
        &mut session.audio_decode_cache,
        &mut session.audio_interval_cache,
    )
}

pub fn render_audio_chunk(
    composition: &Composition,
    session: &mut RenderSession,
    start_time_secs: f64,
    sample_frames: usize,
) -> Result<Option<AudioBuffer>> {
    ensure_assets_preloaded(composition, session)?;
    render_runtime_audio_chunk(
        composition,
        &mut session.assets,
        &mut session.audio_decode_cache,
        &mut session.audio_interval_cache,
        start_time_secs,
        sample_frames,
    )
}

fn render_png(
    composition: &Composition,
    output_path: impl AsRef<Path>,
    backend: RenderBackend,
) -> Result<()> {
    let profile_config = crate::runtime::profile::ProfileConfig::from_env();
    let (_, summary) = crate::runtime::profile::profile_render(&profile_config, || {
        let engine = render_registry::render_engine_for_backend(backend)?;
        let mut session = RenderSession::with_render_engine(engine.clone());
        let rgba = engine.render_frame_rgba(composition, 0, &mut session)?;
        let image =
            image::RgbaImage::from_raw(composition.width as u32, composition.height as u32, rgba)
                .ok_or_else(|| anyhow!("failed to build PNG image from RGBA frame"))?;
        image.save(&output_path)?;
        Ok::<_, anyhow::Error>(())
    })?;
    if let Some(summary) = summary {
        crate::runtime::profile::print_profile_summary(&summary, &profile_config)?;
    }
    Ok(())
}

fn render_mp4(
    composition: &Composition,
    output_path: impl AsRef<Path>,
    config: &Mp4Config,
    backend: RenderBackend,
    on_video_frame_encoded: impl FnMut(u32, u32),
) -> Result<()> {
    let composition = composition.aligned_for_video_encoding();
    let profile_config = crate::runtime::profile::ProfileConfig::from_env();
    let (_, summary) = crate::runtime::profile::profile_render(&profile_config, move || {
        let engine = render_registry::render_engine_for_backend(backend)?;
        let mut session = RenderSession::with_render_engine(engine.clone());
        session
            .media_ctx
            .set_video_preview_quality(VideoPreviewQuality::Exact);

        let audio_track = build_audio_track(&composition, &mut session)?;
        crate::codec::encode::encode_rgba_frames(
            output_path.as_ref(),
            composition.width as u32,
            composition.height as u32,
            composition.fps,
            composition.frames,
            config,
            audio_track.as_ref(),
            on_video_frame_encoded,
            |frame_index| {
                let rgba = engine.render_frame_rgba(&composition, frame_index, &mut session)?;
                Ok(rgba)
            },
        )?;
        Ok::<_, anyhow::Error>(())
    })?;
    if let Some(summary) = summary {
        crate::runtime::profile::print_profile_summary(&summary, &profile_config)?;
    }
    Ok(())
}

pub fn render_frame_to_target(
    composition: &Composition,
    frame_index: u32,
    session: &mut RenderSession,
    target: &mut RenderTargetHandle,
) -> Result<()> {
    render_registry::render_engine_for_frame_view_kind(target.frame_view_kind())?
        .render_frame_to_target(composition, frame_index, session, target)
}

pub fn render_frame_rgba(
    composition: &Composition,
    frame_index: u32,
    session: &mut RenderSession,
) -> Result<Vec<u8>> {
    render_registry::default_render_engine().render_frame_rgba(composition, frame_index, session)
}

pub fn render_frame_rgb(
    composition: &Composition,
    frame_index: u32,
    session: &mut RenderSession,
) -> Result<Vec<u8>> {
    let rgba = render_frame_rgba(composition, frame_index, session)?;
    let mut rgb = vec![0_u8; (composition.width as usize) * (composition.height as usize) * 3];
    for (src, dst) in rgba.chunks_exact(4).zip(rgb.chunks_exact_mut(3)) {
        dst[0] = src[0];
        dst[1] = src[1];
        dst[2] = src[2];
    }
    Ok(rgb)
}

#[cfg(test)]
mod tests {
    use super::{RenderSession, render_frame_rgba};
    use crate::{Composition, FrameCtx, text};
    use opencat_core::scene::primitives::{canvas, div, image};
    use opencat_core::style::ColorToken;

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

    #[test]
    fn subtree_cache_does_not_apply_node_opacity_twice() {
        let scene = div()
            .id("root")
            .w_full()
            .h_full()
            .bg_black()
            .script_source(r#"ctx.getNode("box").opacity(ctx.frame === 0 ? 1 : 0.5);"#)
            .expect("script should compile")
            .child(
                div()
                    .id("box")
                    .absolute()
                    .left(0.0)
                    .top(0.0)
                    .w(10.0)
                    .h(10.0)
                    .bg_white(),
            );

        let composition = Composition::new("opacity_cache")
            .size(20, 20)
            .fps(30)
            .frames(2)
            .root(move |_ctx: &FrameCtx| scene.clone().into())
            .build()
            .expect("composition should build");

        let mut session = RenderSession::new();
        let first =
            render_frame_rgba(&composition, 0, &mut session).expect("frame 0 should render");
        let second =
            render_frame_rgba(&composition, 1, &mut session).expect("frame 1 should render");

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
        let parsed = crate::parse_file("json/split_text_demo.jsonl").expect("parse");
        let root = if let Some(script) = parsed.script.as_deref() {
            if script.trim().is_empty() {
                parsed.root
            } else {
                let driver = crate::ScriptDriver::from_source(script).expect("script");
                parsed.root.script_driver(driver)
            }
        } else {
            parsed.root
        };
        let composition = Composition::new("split_text_property_layer")
            .size(parsed.width, parsed.height)
            .fps(parsed.fps as u32)
            .frames(parsed.frames as u32)
            .root(move |_ctx| root.clone())
            .build()
            .expect("composition");

        let mut session = RenderSession::new();
        let frame =
            render_frame_rgba(&composition, 100, &mut session).expect("frame should render");

        assert!(
            has_bright_pixel_in_rect(&frame, parsed.width as usize, 120, 330, 420, 130),
            "chars text should be visible after splitText property-layer animation settles"
        );
        assert!(
            has_bright_pixel_in_rect(&frame, parsed.width as usize, 760, 330, 420, 130),
            "words text should be visible after splitText property-layer animation settles"
        );
    }

    #[test]
    fn subtree_cache_preserves_shadow_outside_node_bounds_during_opacity_animation() {
        let scene = div()
            .id("root")
            .w_full()
            .h_full()
            .bg_white()
            .script_source(r#"ctx.getNode("box").opacity(ctx.frame === 0 ? 1 : 0.5);"#)
            .expect("script should compile")
            .child(
                div()
                    .id("box")
                    .absolute()
                    .left(10.0)
                    .top(10.0)
                    .w(20.0)
                    .h(8.0)
                    .rounded_full()
                    .bg(ColorToken::Red500)
                    .shadow_lg(),
            );

        let composition = Composition::new("shadow_clip_consistency")
            .size(40, 40)
            .fps(30)
            .frames(2)
            .root(move |_ctx: &FrameCtx| scene.clone().into())
            .build()
            .expect("composition should build");

        let mut session = RenderSession::new();
        let first =
            render_frame_rgba(&composition, 0, &mut session).expect("frame 0 should render");
        let second =
            render_frame_rgba(&composition, 1, &mut session).expect("frame 1 should render");

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
        let scene = div()
            .id("root")
            .w_full()
            .h_full()
            .bg(ColorToken::Black)
            .script_source(r#"ctx.getNode("mover").translateY(ctx.frame);"#)
            .expect("script should compile")
            .child(
                div()
                    .id("card")
                    .absolute()
                    .left(4.0)
                    .top(4.0)
                    .w(12.0)
                    .h(12.0)
                    .rounded(6.0)
                    .overflow_hidden()
                    .child(
                        div()
                            .id("card-fill")
                            .w_full()
                            .h_full()
                            .bg(ColorToken::White),
                    ),
            )
            .child(
                div()
                    .id("mover")
                    .absolute()
                    .left(0.0)
                    .top(0.0)
                    .w(2.0)
                    .h(2.0)
                    .bg(ColorToken::Red500),
            );

        let composition = Composition::new("clip_consistency")
            .size(24, 24)
            .fps(30)
            .frames(2)
            .root(move |_ctx: &FrameCtx| scene.clone().into())
            .build()
            .expect("composition should build");

        let mut session = RenderSession::new();
        let first =
            render_frame_rgba(&composition, 0, &mut session).expect("frame 0 should render");
        let second =
            render_frame_rgba(&composition, 1, &mut session).expect("frame 1 should render");

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

        let scene = canvas()
            .id("canvas")
            .size(2.0, 1.0)
            .asset_path("hero", &image_path)
            .script_source(
                r#"
                const CK = ctx.CanvasKit;
                const image = ctx.getImage("hero");
                ctx.getCanvas().drawImageRect(
                    image,
                    CK.XYWHRect(0, 0, 2, 1),
                    CK.XYWHRect(0, 0, 2, 1),
                );
                "#,
            )
            .expect("script should compile");

        let composition = Composition::new("canvas_asset_alias")
            .size(2, 1)
            .fps(30)
            .frames(1)
            .root(move |_ctx: &FrameCtx| scene.clone().into())
            .build()
            .expect("composition should build");

        let mut session = RenderSession::new();
        let frame = render_frame_rgba(&composition, 0, &mut session).expect("frame should render");

        let _ = std::fs::remove_file(&image_path);

        assert_eq!(pixel_rgba(&frame, 2, 0, 0), [255, 0, 0, 255]);
        assert_eq!(pixel_rgba(&frame, 2, 1, 0), [0, 255, 0, 255]);
    }

    #[test]
    fn composition_alignment_for_video_encoding_rounds_up_to_even_dimensions() {
        let composition = Composition::new("align")
            .size(3, 5)
            .fps(30)
            .frames(1)
            .root(|_ctx| div().id("root").into())
            .build()
            .expect("composition should build");

        let aligned = composition.aligned_for_video_encoding();
        assert_eq!((aligned.width, aligned.height), (4, 6));

        let even = Composition::new("align-even")
            .size(1280, 720)
            .fps(30)
            .frames(1)
            .root(|_ctx| div().id("root").into())
            .build()
            .expect("composition should build");
        let even_aligned = even.aligned_for_video_encoding();
        assert_eq!((even_aligned.width, even_aligned.height), (1280, 720));
    }

    #[test]
    fn subtree_cache_preserves_rust_driven_scale_animation() {
        let composition = Composition::new("rust_scale_cache")
            .size(24, 24)
            .fps(30)
            .frames(2)
            .root(|ctx: &FrameCtx| {
                let scale = if ctx.frame == 0 { 1.0 } else { 2.0 };
                div()
                    .id("root")
                    .w_full()
                    .h_full()
                    .bg_black()
                    .child(
                        div()
                            .id("dot")
                            .absolute()
                            .left(8.0)
                            .top(8.0)
                            .w(8.0)
                            .h(8.0)
                            .rounded_full()
                            .bg_white()
                            .scale(scale),
                    )
                    .into()
            })
            .build()
            .expect("composition should build");

        let mut session = RenderSession::new();
        let first =
            render_frame_rgba(&composition, 0, &mut session).expect("frame 0 should render");
        let second =
            render_frame_rgba(&composition, 1, &mut session).expect("frame 1 should render");

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
        let composition = Composition::new("nested_transform_cache")
            .size(24, 24)
            .fps(30)
            .frames(2)
            .root(|ctx: &FrameCtx| {
                let scale = if ctx.frame == 0 { 1.0 } else { 2.0 };
                let ticker_color = if ctx.frame == 0 {
                    ColorToken::Red500
                } else {
                    ColorToken::Blue500
                };
                div()
                    .id("root")
                    .w_full()
                    .h_full()
                    .bg_black()
                    .child(
                        div()
                            .id("group")
                            .absolute()
                            .left(8.0)
                            .top(8.0)
                            .h(8.0)
                            .w(8.0)
                            .child(
                                div()
                                    .id("dot")
                                    .w_full()
                                    .h_full()
                                    .rounded_full()
                                    .bg_white()
                                    .scale(scale),
                            ),
                    )
                    .child(
                        div()
                            .id("ticker-fill")
                            .absolute()
                            .left(0.0)
                            .top(0.0)
                            .w(1.0)
                            .h(1.0)
                            .bg(ticker_color),
                    )
                    .into()
            })
            .build()
            .expect("composition should build");

        let mut session = RenderSession::new();
        let first =
            render_frame_rgba(&composition, 0, &mut session).expect("frame 0 should render");
        let second =
            render_frame_rgba(&composition, 1, &mut session).expect("frame 1 should render");

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
    fn non_video_bitmap_populates_item_picture_cache() {
        let image_path =
            std::env::temp_dir().join(format!("opencat-item-cache-{}.png", std::process::id()));
        write_test_png(&image_path);

        let composition = Composition::new("bitmap_item_cache")
            .size(24, 24)
            .fps(30)
            .frames(2)
            .root({
                let image_path = image_path.clone();
                move |ctx: &FrameCtx| {
                    let ticker_color = if ctx.frame == 0 {
                        ColorToken::Red500
                    } else {
                        ColorToken::Blue500
                    };
                    div()
                        .id("root")
                        .w_full()
                        .h_full()
                        .bg_black()
                        .child(
                            image()
                                .path(&image_path)
                                .id("bitmap")
                                .absolute()
                                .left(8.0)
                                .top(8.0)
                                .w(8.0)
                                .h(8.0),
                        )
                        .child(
                            div()
                                .id("ticker")
                                .absolute()
                                .left(0.0)
                                .top(0.0)
                                .w(1.0)
                                .h(1.0)
                                .bg(ticker_color),
                        )
                        .into()
                }
            })
            .build()
            .expect("composition should build");

        let mut session = RenderSession::new();
        let _ = render_frame_rgba(&composition, 0, &mut session).expect("frame 0 should render");
        let _ = render_frame_rgba(&composition, 1, &mut session).expect("frame 1 should render");

        assert_eq!(
            session.cache_registry.item_picture_cache().borrow().len(),
            1
        );

        let _ = std::fs::remove_file(&image_path);
    }

    #[test]
    fn layered_caption_renders_above_timeline_transition() {
        use crate::{Easing, SrtEntry, caption, div, fade, timeline};

        let composition = Composition::new("layered_caption")
            .size(320, 180)
            .fps(30)
            .frames(25)
            .root(move |_| {
                div()
                    .id("root")
                    .child(
                        timeline()
                            .sequence(
                                10,
                                div()
                                    .id("scene-a")
                                    .bg(ColorToken::Black)
                                    .child(text("A").id("a"))
                                    .into(),
                            )
                            .transition(fade().timing(Easing::Linear, 5))
                            .sequence(
                                10,
                                div()
                                    .id("scene-b")
                                    .bg(ColorToken::Black)
                                    .child(text("B").id("b"))
                                    .into(),
                            ),
                    )
                    .child(
                        div().id("overlay-root").child(
                            caption()
                                .id("subs")
                                .path("sub.srt")
                                .entries(vec![SrtEntry {
                                    index: 1,
                                    start_frame: 0,
                                    end_frame: 25,
                                    text: "Subtitle".into(),
                                }])
                                .text_color(ColorToken::White),
                        ),
                    )
                    .into()
            })
            .build()
            .expect("composition should build");

        let mut session = RenderSession::new();
        let pixels =
            render_frame_rgba(&composition, 12, &mut session).expect("frame should render");

        assert!(
            pixels.iter().any(|&byte| byte > 0),
            "transition frame with caption overlay should not be blank"
        );
    }

    #[test]
    fn layered_single_scene_renders_bottom_scene_before_caption_overlay() {
        use crate::{SrtEntry, caption, div};

        let composition = Composition::new("layered_single_scene_with_caption")
            .size(64, 64)
            .fps(30)
            .frames(1)
            .root(move |_| {
                div()
                    .id("root")
                    .child(div().id("scene").w_full().h_full().bg(ColorToken::Blue500))
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
                            .text_color(ColorToken::White),
                    )
                    .into()
            })
            .build()
            .expect("composition should build");

        let mut session = RenderSession::new();
        let pixels = render_frame_rgba(&composition, 0, &mut session).expect("frame should render");

        assert_eq!(
            pixel_rgba(&pixels, 64, 32, 32),
            [43, 127, 255, 255],
            "bottom scene layer should remain visible beneath the caption overlay"
        );
    }

    #[test]
    fn layered_root_caption_without_active_entry_does_not_fail_rendering() {
        use crate::{SrtEntry, caption, div};

        let composition = Composition::new("layered_inactive_root_caption")
            .size(64, 64)
            .fps(30)
            .frames(60)
            .root(move |_| {
                div()
                    .id("root")
                    .child(div().id("scene").w_full().h_full().bg(ColorToken::Blue500))
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
                            .text_color(ColorToken::White),
                    )
                    .into()
            })
            .build()
            .expect("composition should build");

        let mut session = RenderSession::new();
        let pixels = render_frame_rgba(&composition, 0, &mut session).expect("frame should render");

        assert_eq!(
            pixel_rgba(&pixels, 64, 32, 32),
            [43, 127, 255, 255],
            "inactive root caption layer should be skipped while the bottom scene still renders"
        );
    }

    #[test]
    fn timeline_caption_sibling_renders_above_transition() {
        use crate::{Easing, SrtEntry, caption, fade, timeline};

        let composition = Composition::new("timeline_caption")
            .size(320, 180)
            .fps(30)
            .frames(25)
            .root(move |_| {
                div()
                    .id("root")
                    .child(
                        timeline()
                            .sequence(
                                10,
                                div()
                                    .id("scene-a")
                                    .bg(ColorToken::Black)
                                    .child(text("A").id("a"))
                                    .into(),
                            )
                            .transition(fade().timing(Easing::Linear, 5))
                            .sequence(
                                10,
                                div()
                                    .id("scene-b")
                                    .bg(ColorToken::Black)
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
                            .text_color(ColorToken::White),
                    )
                    .into()
            })
            .build()
            .expect("composition should build");

        let mut session = RenderSession::new();
        let pixels =
            render_frame_rgba(&composition, 12, &mut session).expect("frame should render");

        assert!(pixels.iter().any(|&byte| byte > 0));
    }

    #[test]
    fn nested_timeline_transition_renders_real_composite() {
        use opencat_core::scene::node::Node;
        use opencat_core::style::{LengthPercentageAuto, Position};
        use crate::{Easing, fade, timeline};

        let composition = Composition::new("nested_timeline_transition")
            .size(80, 80)
            .fps(30)
            .frames(30)
            .root(move |_| {
                let mut tl_kind = Node::from(
                    timeline()
                        .sequence(10, div().id("scene-a").w_full().h_full().bg_red().into())
                        .transition(fade().timing(Easing::Linear, 10))
                        .sequence(10, div().id("scene-b").w_full().h_full().bg_blue().into()),
                )
                .kind()
                .clone();
                let tl_style = tl_kind.style_mut();
                tl_style.id = "tl".into();
                tl_style.position = Some(Position::Absolute);
                tl_style.inset_left = Some(LengthPercentageAuto::Length(0.0));
                tl_style.inset_top = Some(LengthPercentageAuto::Length(0.0));
                tl_style.width = Some(80.0);
                tl_style.height = Some(80.0);
                tl_style.overflow_hidden = true;

                div()
                    .id("root")
                    .w_full()
                    .h_full()
                    .bg_black()
                    .child(Node::new(tl_kind))
                    .into()
            })
            .build()
            .expect("composition should build");

        let mut session = RenderSession::new();
        let pixels =
            render_frame_rgba(&composition, 15, &mut session).expect("frame should render");
        let pixel = pixel_rgba(&pixels, 80, 40, 40);

        assert!(
            pixel[0] > 0 && pixel[2] > 0,
            "transition pixel should contain both from/to colors, got {:?}",
            pixel
        );
    }

    #[test]
    fn root_timeline_renders_without_root_transition_special_case() {
        use opencat_core::scene::node::Node;
        use crate::{Easing, fade, timeline};

        let composition = Composition::new("root_timeline_transition")
            .size(80, 80)
            .fps(30)
            .frames(30)
            .root(move |_| {
                let mut tl_kind = Node::from(
                    timeline()
                        .sequence(10, div().id("scene-a").w_full().h_full().bg_red().into())
                        .transition(fade().timing(Easing::Linear, 10))
                        .sequence(10, div().id("scene-b").w_full().h_full().bg_blue().into()),
                )
                .kind()
                .clone();
                let tl_style = tl_kind.style_mut();
                tl_style.id = "tl".into();
                tl_style.width_full = true;
                tl_style.height_full = true;
                tl_style.overflow_hidden = true;
                Node::new(tl_kind)
            })
            .build()
            .expect("composition should build");

        let mut session = RenderSession::new();
        let pixels =
            render_frame_rgba(&composition, 15, &mut session).expect("frame should render");
        let pixel = pixel_rgba(&pixels, 80, 40, 40);

        assert!(
            pixel[0] > 0 && pixel[2] > 0,
            "root timeline transition should be composited by the tl node itself, got {:?}",
            pixel
        );
    }

    #[test]
    fn transition_subtree_snapshots_are_reused_across_transition_frames() {
        use opencat_core::scene::node::Node;
        use crate::{Easing, fade, timeline};

        let composition = Composition::new("transition_subtree_cache_reuse")
            .size(32, 32)
            .fps(30)
            .frames(30)
            .root(move |_| {
                let mut tl_kind = Node::from(
                    timeline()
                        .sequence(
                            10,
                            div()
                                .id("scene-a")
                                .w_full()
                                .h_full()
                                .bg_red()
                                .child(
                                    div()
                                        .id("inner-a")
                                        .absolute()
                                        .left(4.0)
                                        .top(4.0)
                                        .w(16.0)
                                        .h(16.0)
                                        .bg_white(),
                                )
                                .into(),
                        )
                        .transition(fade().timing(Easing::Linear, 10))
                        .sequence(
                            10,
                            div()
                                .id("scene-b")
                                .w_full()
                                .h_full()
                                .bg_blue()
                                .child(
                                    div()
                                        .id("inner-b")
                                        .absolute()
                                        .left(4.0)
                                        .top(4.0)
                                        .w(16.0)
                                        .h(16.0)
                                        .bg_black(),
                                )
                                .into(),
                        ),
                )
                .kind()
                .clone();
                let tl_style = tl_kind.style_mut();
                tl_style.id = "tl".into();
                tl_style.width_full = true;
                tl_style.height_full = true;
                tl_style.overflow_hidden = true;
                Node::new(tl_kind)
            })
            .build()
            .expect("composition should build");

        let mut session = RenderSession::new();
        let _ = render_frame_rgba(&composition, 12, &mut session)
            .expect("first transition frame should render");
        let size_after_first = session
            .cache_registry
            .subtree_snapshot_cache()
            .borrow()
            .len();

        let _ = render_frame_rgba(&composition, 13, &mut session)
            .expect("second transition frame should render");
        let size_after_second = session
            .cache_registry
            .subtree_snapshot_cache()
            .borrow()
            .len();

        assert!(
            size_after_first >= 2,
            "first transition frame should populate cache for from and to scenes, got {size_after_first}"
        );
        assert_eq!(
            size_after_first, size_after_second,
            "consecutive transition frames should hit cache, not grow it"
        );
    }
}
