use std::sync::Arc;

use crate::css::{VarLink, VarSource};
use crate::frame_ctx::FrameCtx;
use crate::media::VideoFrameRequest;
use crate::parse::composition::Composition;
use crate::parse::document::ParsedComposition;
use crate::parse::node::{Node, NodeKind};
use crate::parse::primitives::{ImageSource, Video};
use crate::parse::time::{FrameState, TimelineSegment, frame_state_for_root};
use crate::probe::catalog::ResourceRequests;

pub fn collect_resource_requests(composition: &Composition) -> ResourceRequests {
    let mut req = ResourceRequests::default();
    req.audios
        .extend(composition.audio_sources().iter().map(|a| a.source.clone()));

    for frame in 0..composition.frames {
        let frame_ctx = FrameCtx {
            frame,
            fps: composition.fps,
            width: composition.width,
            height: composition.height,
            frames: composition.frames,
        };
        let root = composition.root_node(&frame_ctx);
        collect_sources_from_frame_state(
            &frame_state_for_root(&root, &frame_ctx),
            &frame_ctx,
            None,
            &mut req,
        );
    }
    req
}

/// Collect [`ResourceRequests`] directly from a [`ParsedComposition`].
///
/// This is the canonical, host-facing entry point: it walks the *static* node
/// tree once and declares every referenced source. It does **not** iterate
/// composition frames, does **not** evaluate script-driven root closures, and
/// does **not** filter by per-frame visibility -- a declared asset is collected
/// regardless of when (or whether) it is on screen. Collection order is not
/// part of the protocol: callers may fetch/cache the resulting set in any
/// order.
pub fn collect_resource_requests_from_parsed(parsed: &ParsedComposition) -> ResourceRequests {
    let mut req = ResourceRequests::default();
    req.audios
        .extend(parsed.audio_sources.iter().map(|a| a.source.clone()));
    collect_sources_static(&parsed.root, None, &mut req);
    req
}

/// Static counterpart of [`collect_sources`]: walks a node tree collecting
/// declared sources without a [`FrameCtx`]. Timeline segments are fully
/// expanded (every scene / transition endpoint is visited), so sources that
/// only appear on a single frame are still declared.
fn collect_sources_static(node: &Node, vars: Option<&Arc<VarLink>>, req: &mut ResourceRequests) {
    collect_background_images(node, vars, req);
    let child_vars = own_var_link(node, vars);
    match node.kind() {
        NodeKind::Div(div) => {
            for child in div.children_ref() {
                collect_sources_static(child, child_vars.as_ref(), req);
            }
        }
        NodeKind::Canvas(canvas) => {
            for asset in canvas.assets_ref() {
                if !matches!(asset.source, ImageSource::Unset) {
                    req.images.insert(asset.source.clone());
                }
            }
            for child in canvas.hidden_children_ref() {
                collect_sources_static(child, child_vars.as_ref(), req);
            }
        }
        NodeKind::Image(image) => {
            if !matches!(image.source(), ImageSource::Unset) {
                req.images.insert(image.source().clone());
            }
        }
        NodeKind::Lottie(lottie) => {
            if !matches!(
                lottie.source(),
                crate::parse::primitives::LottieSource::Unset
            ) {
                req.lotties.insert(crate::probe::catalog::LottieRequest {
                    source: lottie.source().clone(),
                });
            }
        }
        NodeKind::Video(video) => {
            req.videos.insert(video.source().clone());
            for child in video.children_ref() {
                collect_sources_static(child, child_vars.as_ref(), req);
            }
        }
        NodeKind::Timeline(timeline) => {
            for segment in timeline.segments() {
                match segment {
                    TimelineSegment::Scene { scene, .. } => {
                        collect_sources_static(scene, child_vars.as_ref(), req);
                    }
                    TimelineSegment::Transition { from, to, .. } => {
                        collect_sources_static(from, child_vars.as_ref(), req);
                        collect_sources_static(to, child_vars.as_ref(), req);
                    }
                }
            }
        }
        NodeKind::Text(_) | NodeKind::Path(_) | NodeKind::Svg(_) => {}
    }
}

pub(crate) fn collect_sources_from_frame_state(
    state: &FrameState,
    frame_ctx: &FrameCtx,
    vars: Option<&Arc<VarLink>>,
    req: &mut ResourceRequests,
) {
    match state {
        FrameState::Scene { scene, .. } => {
            collect_sources(scene, frame_ctx, vars, req);
        }
        FrameState::Transition { from, to, .. } => {
            collect_sources(from, frame_ctx, vars, req);
            collect_sources(to, frame_ctx, vars, req);
        }
    }
}

pub(crate) fn collect_sources(
    node: &Node,
    frame_ctx: &FrameCtx,
    vars: Option<&Arc<VarLink>>,
    req: &mut ResourceRequests,
) {
    collect_background_images(node, vars, req);
    let child_vars = own_var_link(node, vars);
    match node.kind() {
        NodeKind::Div(div) => {
            for child in div.children_ref() {
                collect_sources(child, frame_ctx, child_vars.as_ref(), req);
            }
        }
        NodeKind::Canvas(canvas) => {
            for asset in canvas.assets_ref() {
                if !matches!(asset.source, ImageSource::Unset) {
                    req.images.insert(asset.source.clone());
                }
            }
            for child in canvas.hidden_children_ref() {
                collect_sources(child, frame_ctx, child_vars.as_ref(), req);
            }
        }
        NodeKind::Image(image) => {
            if !matches!(image.source(), ImageSource::Unset) {
                req.images.insert(image.source().clone());
            }
        }
        NodeKind::Lottie(lottie) => {
            if !lottie_visible_at_frame(lottie, frame_ctx) {
                return;
            }
            if !matches!(
                lottie.source(),
                crate::parse::primitives::LottieSource::Unset
            ) {
                req.lotties.insert(crate::probe::catalog::LottieRequest {
                    source: lottie.source().clone(),
                });
            }
        }
        NodeKind::Video(video) => {
            if !video_visible_at_frame(video, frame_ctx) {
                return;
            }
            req.videos.insert(video.source().clone());
            for child in video.children_ref() {
                collect_sources(child, frame_ctx, child_vars.as_ref(), req);
            }
        }
        NodeKind::Timeline(_) => {
            collect_sources_from_frame_state(
                &frame_state_for_root(node, frame_ctx),
                frame_ctx,
                child_vars.as_ref(),
                req,
            )
        }
        NodeKind::Text(_) | NodeKind::Path(_) | NodeKind::Svg(_) => {}
    }
}

/// CSS `background-image: url(...)` 层声明的位图（任何节点种类都可能带）。
/// var() 间接引用（`background-image: var(--tex)`）在层叠期落不成图层，url
/// 要到 resolve 期替换后才出现；这里用节点自有变量叠继承链重放一次
/// background-image，把这类运行期才出现的定位串提前报给宿主——资产探测只
/// 跑一次，声明不全 = 渲染时空层。
fn collect_background_images(node: &Node, vars: Option<&Arc<VarLink>>, req: &mut ResourceRequests) {
    collect_image_layers(node.style_ref(), req);
    let Some(deps) = node.style_ref().var_deps.as_ref() else {
        return;
    };
    let mut replayed = node.style_ref().clone();
    crate::css::replay_var_deps(&mut replayed, deps, &ChainVars(vars.map(Arc::as_ref)));
    collect_image_layers(&replayed, req);
}

fn collect_image_layers(style: &crate::style::NodeStyle, req: &mut ResourceRequests) {
    for layer in &style.background_layers {
        if let crate::style::BackgroundFill::Image { image } = layer {
            req.images.insert(image.source());
        }
    }
}

/// 节点自身声明的自定义属性叠上继承链，构成子树查找用链（同 CSS 继承，
/// 与 resolve 期的 [`crate::css::VarLink`] 结构一致）；自身无声明的节点
/// 原样传递父链，后代仍查得到祖先定义。
fn own_var_link(node: &Node, parent: Option<&Arc<VarLink>>) -> Option<Arc<VarLink>> {
    match node.style_ref().var_deps.as_ref() {
        Some(deps) => Some(VarLink::push(parent.cloned(), deps.own.clone(), false)),
        None => parent.cloned(),
    }
}

/// 继承链可能缺席（根节点无父链、root vars 挂在根自身），包一层当 var 源。
struct ChainVars<'a>(Option<&'a VarLink>);

impl VarSource for ChainVars<'_> {
    fn var(&self, key: &str) -> Option<&lightningcss::properties::custom::TokenList<'static>> {
        self.0.and_then(|link| link.var(key))
    }
}

fn lottie_visible_at_frame(
    lottie: &crate::parse::primitives::Lottie,
    frame_ctx: &FrameCtx,
) -> bool {
    VideoFrameRequest {
        composition_time_secs: frame_ctx.frame as f64 / frame_ctx.fps.max(1) as f64,
        timing: lottie.timing(),
    }
    .is_visible()
}

fn video_visible_at_frame(video: &Video, frame_ctx: &FrameCtx) -> bool {
    VideoFrameRequest {
        composition_time_secs: frame_ctx.frame as f64 / frame_ctx.fps.max(1) as f64,
        timing: video.timing(),
    }
    .is_visible()
}


