pub(crate) mod invalidation;

use std::{collections::HashSet, path::Path, sync::Arc, time::Instant};

use anyhow::{Result, anyhow};
use skia_safe::{
    AlphaType, ColorType, EncodedImageFormat, ImageInfo, image::CachingHint, surfaces,
};

use crate::{
    Composition, FrameCtx, Node,
    assets::AssetsMap,
    backend::{resource_cache::BackendResourceCache, skia_transition},
    display::{
        analysis::display_list_contains_video,
        build::{build_display_list_from_tree, build_display_tree},
        list::DisplayList,
        tree::DisplayTree,
    },
    element::resolve::resolve_ui_tree_with_script_cache,
    layout::LayoutSession,
    media::MediaContext,
    nodes::ImageSource,
    profile::{BackendProfile, FrameProfile, RenderProfiler, SceneBuildStats},
    render_cache::{SceneSlot, SceneSnapshotCache},
    scene_snapshot::{SceneSnapshotRuntime, plan_for_scene, render_scene_slot},
    script::{ScriptRuntimeCache, StyleMutations},
    timeline::{FrameState, frame_state_for_root},
    view::NodeKind,
};

pub use crate::codec::encode::Mp4Config;

pub enum OutputFormat {
    Mp4(Mp4Config),
    Png,
}

pub struct EncodingConfig {
    pub format: OutputFormat,
}

pub struct RenderSession {
    media_ctx: MediaContext,
    assets: AssetsMap,
    scene_snapshots: SceneSnapshotCache,
    backend_resources: BackendResourceCache,
    script_runtime: ScriptRuntimeCache,
    scene_layout: LayoutSession,
    transition_from_layout: LayoutSession,
    transition_to_layout: LayoutSession,
    profiler: RenderProfiler,
    prepared_root_ptr: Option<usize>,
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

impl RenderSession {
    pub fn new() -> Self {
        Self {
            media_ctx: MediaContext::new(),
            assets: AssetsMap::new(),
            scene_snapshots: SceneSnapshotCache::new(),
            backend_resources: BackendResourceCache::new(),
            script_runtime: ScriptRuntimeCache::default(),
            scene_layout: LayoutSession::new(),
            transition_from_layout: LayoutSession::new(),
            transition_to_layout: LayoutSession::new(),
            profiler: RenderProfiler::default(),
            prepared_root_ptr: None,
        }
    }

    pub fn print_profile_summary(&self) {
        self.profiler.print_summary();
    }

    fn layout_session_mut(&mut self, slot: SceneSlot) -> &mut LayoutSession {
        match slot {
            SceneSlot::Scene => &mut self.scene_layout,
            SceneSlot::TransitionFrom => &mut self.transition_from_layout,
            SceneSlot::TransitionTo => &mut self.transition_to_layout,
        }
    }
}

impl Composition {
    pub fn render(&self, output_path: impl AsRef<Path>, config: &EncodingConfig) -> Result<()> {
        match &config.format {
            OutputFormat::Mp4(mp4_config) => render_mp4(self, output_path, mp4_config),
            OutputFormat::Png => render_png(self, output_path),
        }
    }
}

fn render_png(composition: &Composition, output_path: impl AsRef<Path>) -> Result<()> {
    let mut session = RenderSession::new();
    let mut surface = render_frame_surface(composition, 0, &mut session)?;
    let image = surface.image_snapshot();
    let data = image
        .encode(None, EncodedImageFormat::PNG, 100)
        .ok_or_else(|| anyhow!("failed to encode PNG"))?;
    std::fs::write(output_path, &*data)?;
    session.profiler.print_summary();
    Ok(())
}

fn render_mp4(
    composition: &Composition,
    output_path: impl AsRef<Path>,
    config: &Mp4Config,
) -> Result<()> {
    let mut session = RenderSession::new();
    let source_width = composition.width as u32;
    let source_height = composition.height as u32;
    let encoded_width = source_width + source_width % 2;
    let encoded_height = source_height + source_height % 2;
    crate::codec::encode::encode_rgba_frames(
        output_path,
        encoded_width,
        encoded_height,
        composition.fps,
        composition.frames,
        config,
        |frame_index| {
            let rgba = render_frame_rgba(composition, frame_index, &mut session)?;
            Ok(pad_rgba_frame(
                &rgba,
                source_width,
                source_height,
                encoded_width,
                encoded_height,
            ))
        },
    )?;
    session.profiler.print_summary();
    Ok(())
}

fn pad_rgba_frame(
    rgba: &[u8],
    source_width: u32,
    source_height: u32,
    target_width: u32,
    target_height: u32,
) -> Vec<u8> {
    if source_width == target_width && source_height == target_height {
        return rgba.to_vec();
    }

    let source_width = source_width as usize;
    let source_height = source_height as usize;
    let target_width = target_width as usize;
    let target_height = target_height as usize;
    let mut padded = vec![0_u8; target_width * target_height * 4];

    for y in 0..target_height {
        let source_y = y.min(source_height.saturating_sub(1));
        for x in 0..target_width {
            let source_x = x.min(source_width.saturating_sub(1));
            let source_index = (source_y * source_width + source_x) * 4;
            let target_index = (y * target_width + x) * 4;
            padded[target_index..target_index + 4]
                .copy_from_slice(&rgba[source_index..source_index + 4]);
        }
    }

    padded
}

fn render_frame_surface(
    composition: &Composition,
    frame_index: u32,
    session: &mut RenderSession,
) -> Result<skia_safe::Surface> {
    ensure_assets_preloaded(composition, session)?;

    let mut frame_profile = FrameProfile::default();
    let frame_ctx = FrameCtx {
        frame: frame_index,
        fps: composition.fps,
        width: composition.width,
        height: composition.height,
        frames: composition.frames,
    };

    let script_started = Instant::now();
    let mutations: Option<StyleMutations> = None;
    frame_profile.script_ms = script_started.elapsed().as_secs_f64() * 1000.0;

    let mut surface = surfaces::raster_n32_premul((composition.width, composition.height))
        .ok_or_else(|| anyhow!("failed to create skia raster surface"))?;
    let canvas = surface.canvas();
    let root = composition.root_node(&frame_ctx);

    let frame_state_started = Instant::now();
    let frame_state = frame_state_for_root(&root, &frame_ctx);
    frame_profile.frame_state_ms = frame_state_started.elapsed().as_secs_f64() * 1000.0;

    match frame_state {
        FrameState::Scene { scene } => {
            let (display_tree, display_list, scene_stats) = build_scene_display_list_with_slot(
                &scene,
                &frame_ctx,
                session,
                mutations.as_ref(),
                SceneSlot::Scene,
            )?;
            frame_profile.merge_scene_stats(&scene_stats);
            let snapshot_plan = plan_for_scene(&scene_stats);

            let backend_started = Instant::now();
            let mut backend_profile = BackendProfile::default();
            {
                let mut snapshot_runtime = SceneSnapshotRuntime {
                    assets: &session.assets,
                    scene_snapshots: &mut session.scene_snapshots,
                    backend_resources: &session.backend_resources,
                    media_ctx: &mut session.media_ctx,
                    frame_ctx: &frame_ctx,
                    backend_profile: &mut backend_profile,
                    width: composition.width,
                    height: composition.height,
                };
                render_scene_slot(
                    &mut snapshot_runtime,
                    SceneSlot::Scene,
                    &display_tree,
                    &display_list,
                    snapshot_plan,
                    false,
                    Some(canvas),
                )?;
            }

            frame_profile.backend_ms = backend_started.elapsed().as_secs_f64() * 1000.0;
            frame_profile.merge_backend_profile(&backend_profile);
        }
        FrameState::Transition {
            from,
            to,
            progress,
            kind,
        } => {
            let (from_tree, from_display, from_stats) = build_scene_display_list_with_slot(
                &from,
                &frame_ctx,
                session,
                mutations.as_ref(),
                SceneSlot::TransitionFrom,
            )?;
            let (to_tree, to_display, to_stats) = build_scene_display_list_with_slot(
                &to,
                &frame_ctx,
                session,
                mutations.as_ref(),
                SceneSlot::TransitionTo,
            )?;
            frame_profile.merge_scene_stats(&from_stats);
            frame_profile.merge_scene_stats(&to_stats);
            let from_plan = plan_for_scene(&from_stats);
            let to_plan = plan_for_scene(&to_stats);

            let backend_started = Instant::now();
            let mut backend_profile = BackendProfile::default();
            let (from_snapshot, to_snapshot) = {
                let mut snapshot_runtime = SceneSnapshotRuntime {
                    assets: &session.assets,
                    scene_snapshots: &mut session.scene_snapshots,
                    backend_resources: &session.backend_resources,
                    media_ctx: &mut session.media_ctx,
                    frame_ctx: &frame_ctx,
                    backend_profile: &mut backend_profile,
                    width: composition.width,
                    height: composition.height,
                };
                let from_snapshot = render_scene_slot(
                    &mut snapshot_runtime,
                    SceneSlot::TransitionFrom,
                    &from_tree,
                    &from_display,
                    from_plan,
                    true,
                    None,
                )?
                .expect("transition source scene snapshot should exist");
                let to_snapshot = render_scene_slot(
                    &mut snapshot_runtime,
                    SceneSlot::TransitionTo,
                    &to_tree,
                    &to_display,
                    to_plan,
                    true,
                    None,
                )?
                .expect("transition target scene snapshot should exist");
                (from_snapshot, to_snapshot)
            };
            frame_profile.backend_ms = backend_started.elapsed().as_secs_f64() * 1000.0;

            let transition_started = Instant::now();
            skia_transition::draw_transition(
                canvas,
                &from_snapshot,
                &to_snapshot,
                progress,
                kind,
                composition.width,
                composition.height,
                Some(&mut backend_profile),
            )?;
            let transition_ms = transition_started.elapsed().as_secs_f64() * 1000.0;
            frame_profile.transition_ms = transition_ms;
            frame_profile.merge_backend_profile(&backend_profile);
            match kind {
                crate::transitions::TransitionKind::Slide(_) => {
                    frame_profile.slide_transition_ms = transition_ms;
                    frame_profile.slide_transition_frames = 1;
                }
                crate::transitions::TransitionKind::LightLeak(_) => {
                    frame_profile.light_leak_transition_ms = transition_ms;
                    frame_profile.light_leak_transition_frames = 1;
                }
                _ => {}
            }
        }
    }

    session.profiler.push(frame_profile);
    Ok(surface)
}

fn ensure_assets_preloaded(composition: &Composition, session: &mut RenderSession) -> Result<()> {
    let root_ptr = Arc::as_ptr(&composition.root) as *const () as usize;
    if session.prepared_root_ptr == Some(root_ptr) {
        return Ok(());
    }

    let mut sources = HashSet::new();
    for frame in 0..composition.frames {
        let frame_ctx = FrameCtx {
            frame,
            fps: composition.fps,
            width: composition.width,
            height: composition.height,
            frames: composition.frames,
        };
        let root = composition.root_node(&frame_ctx);
        match frame_state_for_root(&root, &frame_ctx) {
            FrameState::Scene { scene } => {
                collect_image_sources(&scene, &frame_ctx, &mut sources);
            }
            FrameState::Transition { from, to, .. } => {
                collect_image_sources(&from, &frame_ctx, &mut sources);
                collect_image_sources(&to, &frame_ctx, &mut sources);
            }
        }
    }

    session.assets.preload_image_sources(sources)?;
    session.prepared_root_ptr = Some(root_ptr);
    Ok(())
}

fn collect_image_sources(node: &Node, frame_ctx: &FrameCtx, sources: &mut HashSet<ImageSource>) {
    match node.kind() {
        NodeKind::Component(component) => {
            let rendered = component.render(frame_ctx);
            collect_image_sources(&rendered, frame_ctx, sources);
        }
        NodeKind::Div(div) => {
            for child in div.children_ref() {
                collect_image_sources(child, frame_ctx, sources);
            }
        }
        NodeKind::Canvas(canvas) => {
            for asset in canvas.assets_ref() {
                if !matches!(asset.source, ImageSource::Unset) {
                    sources.insert(asset.source.clone());
                }
            }
        }
        NodeKind::Image(image) => {
            if !matches!(image.source(), ImageSource::Unset) {
                sources.insert(image.source().clone());
            }
        }
        NodeKind::Timeline(_) => match frame_state_for_root(node, frame_ctx) {
            FrameState::Scene { scene } => collect_image_sources(&scene, frame_ctx, sources),
            FrameState::Transition { from, to, .. } => {
                collect_image_sources(&from, frame_ctx, sources);
                collect_image_sources(&to, frame_ctx, sources);
            }
        },
        NodeKind::Text(_) | NodeKind::Video(_) | NodeKind::Lucide(_) => {}
    }
}

pub fn render_frame_rgba(
    composition: &Composition,
    frame_index: u32,
    session: &mut RenderSession,
) -> Result<Vec<u8>> {
    let mut surface = render_frame_surface(composition, frame_index, session)?;
    let image = surface.image_snapshot();
    let image_info = ImageInfo::new(
        (composition.width, composition.height),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );

    let mut rgba = vec![0_u8; (composition.width as usize) * (composition.height as usize) * 4];
    let read_ok = image.read_pixels(
        &image_info,
        rgba.as_mut_slice(),
        (composition.width as usize) * 4,
        (0, 0),
        CachingHint::Allow,
    );

    if !read_ok {
        return Err(anyhow!("failed to read pixels from skia surface"));
    }

    Ok(rgba)
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

impl Default for RenderSession {
    fn default() -> Self {
        Self::new()
    }
}

fn build_scene_display_list_with_slot(
    scene: &Node,
    frame_ctx: &FrameCtx,
    session: &mut RenderSession,
    mutations: Option<&StyleMutations>,
    slot: SceneSlot,
) -> Result<(DisplayTree, DisplayList, SceneBuildStats)> {
    let mut stats = SceneBuildStats::default();

    let resolve_started = Instant::now();
    let element_root = resolve_ui_tree_with_script_cache(
        scene,
        frame_ctx,
        &mut session.media_ctx,
        &mut session.assets,
        mutations,
        &mut session.script_runtime,
    )?;
    stats.resolve_ms = resolve_started.elapsed().as_secs_f64() * 1000.0;

    let layout_started = Instant::now();
    let (layout_tree, layout_pass) = session
        .layout_session_mut(slot)
        .compute_layout(&element_root, frame_ctx)?;
    stats.layout_ms = layout_started.elapsed().as_secs_f64() * 1000.0;
    stats.layout_pass = layout_pass;

    let display_started = Instant::now();
    let display_tree = build_display_tree(&element_root, &layout_tree)?;
    let display_list = build_display_list_from_tree(&display_tree);
    stats.display_ms = display_started.elapsed().as_secs_f64() * 1000.0;
    stats.contains_video = display_list_contains_video(&display_list, &session.assets);

    Ok((display_tree, display_list, stats))
}

#[cfg(test)]
mod tests {
    use super::{RenderSession, pad_rgba_frame, render_frame_rgba};
    use crate::{
        Composition, FrameCtx,
        nodes::{canvas, div},
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
            .script_source(r#"ctx.getCanvas().drawImage("hero", 0, 0, 2, 1, "fill");"#)
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
    fn pad_rgba_frame_extends_edge_pixels_for_even_dimensions() {
        let rgba = vec![
            1, 0, 0, 255, 2, 0, 0, 255, 3, 0, 0, 255, 4, 0, 0, 255, 5, 0, 0, 255, 6, 0, 0, 255, 7,
            0, 0, 255, 8, 0, 0, 255, 9, 0, 0, 255,
        ];

        let padded = pad_rgba_frame(&rgba, 3, 3, 4, 4);

        assert_eq!(pixel_rgba(&padded, 4, 0, 0), [1, 0, 0, 255]);
        assert_eq!(pixel_rgba(&padded, 4, 2, 2), [9, 0, 0, 255]);
        assert_eq!(pixel_rgba(&padded, 4, 3, 0), [3, 0, 0, 255]);
        assert_eq!(pixel_rgba(&padded, 4, 0, 3), [7, 0, 0, 255]);
        assert_eq!(pixel_rgba(&padded, 4, 3, 3), [9, 0, 0, 255]);
    }
}
