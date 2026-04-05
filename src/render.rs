use std::{collections::HashSet, path::Path, sync::Arc, time::Instant};

use anyhow::{Result, anyhow};
use skia_safe::{
    AlphaType, ColorType, EncodedImageFormat, ImageInfo, Picture, image::CachingHint, surfaces,
};

use crate::{
    Composition, FrameCtx, Node,
    assets::AssetsMap,
    backend::{
        skia::{
            SkiaBackend, draw_layout_tree_with_subtree_cache, record_display_list_picture,
            record_layout_tree_picture_with_subtree_cache,
        },
        skia_transition,
    },
    cache_policy::{display_list_contains_video, scene_cache_scope},
    display::{build::build_display_list, list::DisplayList},
    element::resolve::resolve_ui_tree_with_script_cache,
    layout::LayoutSession,
    media::MediaContext,
    nodes::ImageSource,
    profile::{BackendProfile, FrameProfile, RenderProfiler, SceneBuildStats},
    render_cache::{RenderCacheState, SceneSlot},
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
    caches: RenderCacheState,
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
            caches: RenderCacheState::new(),
            script_runtime: ScriptRuntimeCache::default(),
            scene_layout: LayoutSession::new(),
            transition_from_layout: LayoutSession::new(),
            transition_to_layout: LayoutSession::new(),
            profiler: RenderProfiler::default(),
            prepared_root_ptr: None,
        }
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
    crate::codec::encode::encode_rgba_frames(
        output_path,
        composition.width as u32,
        composition.height as u32,
        composition.fps,
        composition.frames,
        config,
        |frame_index| render_frame_rgba(composition, frame_index, &mut session),
    )?;
    session.profiler.print_summary();
    Ok(())
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
            let (layout_tree, display_list, scene_stats) = build_scene_display_list_with_slot(
                &scene,
                &frame_ctx,
                session,
                mutations.as_ref(),
                SceneSlot::Scene,
            )?;
            frame_profile.merge_scene_stats(&scene_stats);
            let cache_scope =
                scene_cache_scope(&scene_stats.layout_pass, scene_stats.contains_video);

            let backend_started = Instant::now();
            let mut backend_profile = BackendProfile::default();

            if let Some(picture) = picture_for_slot(
                session,
                SceneSlot::Scene,
                &layout_tree,
                &display_list,
                &scene_stats,
                composition.width,
                composition.height,
                &frame_ctx,
                &mut backend_profile,
                false,
            )? {
                let picture_draw_started = Instant::now();
                canvas.draw_picture(&picture, None, None);
                backend_profile.picture_draw_ms +=
                    picture_draw_started.elapsed().as_secs_f64() * 1000.0;
            } else if scene_stats.contains_video || cache_scope.prefers_subtree_cache() {
                draw_layout_tree_with_subtree_cache(
                    &layout_tree,
                    canvas,
                    &session.assets,
                    session.caches.image_cache(),
                    session.caches.text_picture_cache(),
                    session.caches.subtree_picture_cache(),
                    Some(&mut session.media_ctx),
                    &frame_ctx,
                    Some(&mut backend_profile),
                )?;
            } else {
                let mut backend = SkiaBackend::new_with_cache_and_profile(
                    canvas,
                    composition.width,
                    composition.height,
                    &session.assets,
                    session.caches.image_cache(),
                    session.caches.text_picture_cache(),
                    None,
                    Some(&mut session.media_ctx),
                    &frame_ctx,
                    Some(&mut backend_profile),
                );
                backend.execute(&display_list)?;
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
            let (from_layout, from_display, from_stats) = build_scene_display_list_with_slot(
                &from,
                &frame_ctx,
                session,
                mutations.as_ref(),
                SceneSlot::TransitionFrom,
            )?;
            let (to_layout, to_display, to_stats) = build_scene_display_list_with_slot(
                &to,
                &frame_ctx,
                session,
                mutations.as_ref(),
                SceneSlot::TransitionTo,
            )?;
            frame_profile.merge_scene_stats(&from_stats);
            frame_profile.merge_scene_stats(&to_stats);

            let backend_started = Instant::now();
            let mut backend_profile = BackendProfile::default();
            let from_picture = picture_for_slot(
                session,
                SceneSlot::TransitionFrom,
                &from_layout,
                &from_display,
                &from_stats,
                composition.width,
                composition.height,
                &frame_ctx,
                &mut backend_profile,
                true,
            )?
            .expect("transition source picture should exist");
            let to_picture = picture_for_slot(
                session,
                SceneSlot::TransitionTo,
                &to_layout,
                &to_display,
                &to_stats,
                composition.width,
                composition.height,
                &frame_ctx,
                &mut backend_profile,
                true,
            )?
            .expect("transition target picture should exist");
            frame_profile.backend_ms = backend_started.elapsed().as_secs_f64() * 1000.0;
            frame_profile.merge_backend_profile(&backend_profile);

            let transition_started = Instant::now();
            skia_transition::draw_transition(
                canvas,
                &from_picture,
                &to_picture,
                progress,
                kind,
                composition.width,
                composition.height,
            )?;
            let transition_ms = transition_started.elapsed().as_secs_f64() * 1000.0;
            frame_profile.transition_ms = transition_ms;
            match kind {
                crate::transitions::TransitionKind::Slide => {
                    frame_profile.slide_transition_ms = transition_ms;
                    frame_profile.slide_transition_frames = 1;
                }
                crate::transitions::TransitionKind::LightLeak(_) => {
                    frame_profile.light_leak_transition_ms = transition_ms;
                    frame_profile.light_leak_transition_frames = 1;
                }
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
        NodeKind::Text(_) | NodeKind::Video(_) => {}
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
) -> Result<(
    crate::layout::tree::LayoutTree,
    DisplayList,
    SceneBuildStats,
)> {
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
    let display_list = build_display_list(&layout_tree)?;
    stats.display_ms = display_started.elapsed().as_secs_f64() * 1000.0;
    stats.contains_video = display_list_contains_video(&display_list, &session.assets);

    Ok((layout_tree, display_list, stats))
}

fn picture_for_slot(
    session: &mut RenderSession,
    slot: SceneSlot,
    layout_tree: &crate::layout::tree::LayoutTree,
    display_list: &DisplayList,
    scene_stats: &SceneBuildStats,
    width: i32,
    height: i32,
    frame_ctx: &FrameCtx,
    backend_profile: &mut BackendProfile,
    require_picture: bool,
) -> Result<Option<Picture>> {
    let cache_scope = scene_cache_scope(&scene_stats.layout_pass, scene_stats.contains_video);

    if scene_stats.contains_video {
        session.caches.store_picture(slot, None);
        if !require_picture {
            return Ok(None);
        }
        let picture = record_layout_tree_picture_with_subtree_cache(
            layout_tree,
            width,
            height,
            &session.assets,
            session.caches.image_cache(),
            session.caches.text_picture_cache(),
            session.caches.subtree_picture_cache(),
            Some(&mut session.media_ctx),
            frame_ctx,
            Some(backend_profile),
        )?;
        return Ok(Some(picture));
    }

    if cache_scope.allows_picture_reuse() {
        if let Some(picture) = session.caches.picture(slot) {
            backend_profile.picture_cache_hits += 1;
            return Ok(Some(picture));
        }

        let picture = record_display_list_picture(
            display_list,
            width,
            height,
            &session.assets,
            session.caches.image_cache(),
            session.caches.text_picture_cache(),
            Some(&mut session.media_ctx),
            frame_ctx,
            Some(backend_profile),
        )?;
        backend_profile.picture_cache_misses += 1;
        session.caches.store_picture(slot, Some(picture.clone()));
        return Ok(Some(picture));
    }

    session.caches.store_picture(slot, None);
    if !require_picture {
        return Ok(None);
    }

    let picture = if cache_scope.prefers_subtree_cache() {
        record_layout_tree_picture_with_subtree_cache(
            layout_tree,
            width,
            height,
            &session.assets,
            session.caches.image_cache(),
            session.caches.text_picture_cache(),
            session.caches.subtree_picture_cache(),
            Some(&mut session.media_ctx),
            frame_ctx,
            Some(backend_profile),
        )?
    } else {
        record_display_list_picture(
            display_list,
            width,
            height,
            &session.assets,
            session.caches.image_cache(),
            session.caches.text_picture_cache(),
            Some(&mut session.media_ctx),
            frame_ctx,
            Some(backend_profile),
        )?
    };
    Ok(Some(picture))
}

#[cfg(test)]
mod tests {
    use super::{RenderSession, render_frame_rgba};
    use crate::{Composition, EncodingConfig, FrameCtx, nodes::div};

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
}
