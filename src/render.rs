use std::path::Path;

use anyhow::{Result, anyhow};

use crate::{
    codec::decode::AudioTrack,
    runtime::{
        audio::build_audio_track as build_runtime_audio_track,
        preflight::ensure_assets_preloaded,
        render_registry,
        target::RenderTargetHandle,
    },
    scene::composition::Composition,
};

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

impl Composition {
    pub fn render(&self, output_path: impl AsRef<Path>, config: &EncodingConfig) -> Result<()> {
        self.render_with_backend(
            output_path,
            config,
            render_registry::default_render_backend(),
        )
    }

    pub fn render_with_backend(
        &self,
        output_path: impl AsRef<Path>,
        config: &EncodingConfig,
        backend: RenderBackend,
    ) -> Result<()> {
        match &config.format {
            OutputFormat::Mp4(mp4_config) => render_mp4(self, output_path, mp4_config, backend),
            OutputFormat::Png => render_png(self, output_path, backend),
        }
    }

    pub fn render_frame_with_target(
        &self,
        frame_index: u32,
        session: &mut RenderSession,
        target: &mut RenderTargetHandle,
    ) -> Result<()> {
        render_frame_to_target(self, frame_index, session, target)
    }
}

pub fn build_audio_track(
    composition: &Composition,
    session: &mut RenderSession,
) -> Result<Option<AudioTrack>> {
    ensure_assets_preloaded(composition, session)?;
    build_runtime_audio_track(composition, &mut session.assets)
}

fn render_png(
    composition: &Composition,
    output_path: impl AsRef<Path>,
    backend: RenderBackend,
) -> Result<()> {
    let engine = render_registry::render_engine_for_backend(backend)?;
    let mut session = RenderSession::with_render_engine(engine.clone());
    let rgba = engine.render_frame_rgba(composition, 0, &mut session)?;
    let image =
        image::RgbaImage::from_raw(composition.width as u32, composition.height as u32, rgba)
            .ok_or_else(|| anyhow!("failed to build PNG image from RGBA frame"))?;
    image.save(output_path)?;
    session.profiler.print_summary();
    Ok(())
}

fn render_mp4(
    composition: &Composition,
    output_path: impl AsRef<Path>,
    config: &Mp4Config,
    backend: RenderBackend,
) -> Result<()> {
    let composition = composition.aligned_for_video_encoding();
    let engine = render_registry::render_engine_for_backend(backend)?;
    let mut session = RenderSession::with_render_engine(engine.clone());
    let audio_track = build_audio_track(&composition, &mut session)?;
    crate::codec::encode::encode_rgba_frames(
        output_path,
        composition.width as u32,
        composition.height as u32,
        composition.fps,
        composition.frames,
        config,
        audio_track.as_ref(),
        |frame_index| {
            let rgba = engine.render_frame_rgba(&composition, frame_index, &mut session)?;
            Ok(rgba)
        },
    )?;
    session.profiler.print_summary();
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
    use crate::{
        Composition, FrameCtx,
        scene::primitives::{canvas, div},
        style::ColorToken,
    };

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
                    CK.XYWHRect(0, 0, 1, 1),
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
}
