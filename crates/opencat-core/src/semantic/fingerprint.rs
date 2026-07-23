use std::hash::{Hash, Hasher};

use crate::{
    resolve::{
        style::{ComputedLayoutStyle, ComputedVisualStyle},
        tree::{ElementKind, ElementNode},
    },
    style::ComputedTextStyle,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ElementInputFingerprints {
    pub structure_local: u64,
    pub layout_input_local: u64,
    pub paint_input_local: u64,
    /// composite/apply 维度的 local 哈希：opacity / transforms / backdrop_blur_sigma。
    /// 不进任何 paint cache key，仅供 L3 (DisplayBuildSession) 的子树复用判断使用。
    pub apply_input_local: u64,
    pub structure_subtree: u64,
    pub layout_input_subtree: u64,
    pub paint_input_subtree: u64,
    pub apply_input_subtree: u64,
    pub node_count: usize,
}

#[derive(Clone, Copy)]
struct F32Hash(f32);

impl Hash for F32Hash {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

pub fn compute_element_input_fingerprints(root: &mut ElementNode) -> ElementInputFingerprints {
    let fingerprints = compute_node(root);
    root.fingerprints = fingerprints;
    fingerprints
}

fn compute_node(node: &mut ElementNode) -> ElementInputFingerprints {
    let mut child_fps = Vec::with_capacity(node.children.len());
    for child in &mut node.children {
        child_fps.push(compute_node(child));
    }

    let structure_local = hash_value(&StructureLocal(node));
    let layout_local = hash_value(&LayoutInputLocal(node));
    let paint_local = hash_value(&PaintInputLocal(node));
    let apply_local = hash_value(&ApplyInputLocal(node));
    let node_count = 1 + child_fps.iter().map(|fp| fp.node_count).sum::<usize>();

    let fingerprints = ElementInputFingerprints {
        structure_local,
        layout_input_local: layout_local,
        paint_input_local: paint_local,
        apply_input_local: apply_local,
        structure_subtree: hash_subtree(
            structure_local,
            child_fps.iter().map(|fp| fp.structure_subtree),
        ),
        layout_input_subtree: hash_subtree(
            layout_local,
            child_fps.iter().map(|fp| fp.layout_input_subtree),
        ),
        paint_input_subtree: hash_subtree(
            paint_local,
            child_fps.iter().map(|fp| fp.paint_input_subtree),
        ),
        apply_input_subtree: hash_subtree(
            apply_local,
            child_fps.iter().map(|fp| fp.apply_input_subtree),
        ),
        node_count,
    };
    node.fingerprints = fingerprints;
    fingerprints
}

fn hash_value(value: &impl Hash) -> u64 {
    let mut hasher = ahash::AHasher::default();
    value.hash(&mut hasher);
    hasher.finish()
}

fn hash_subtree(local: u64, children: impl Iterator<Item = u64>) -> u64 {
    let mut hasher = ahash::AHasher::default();
    local.hash(&mut hasher);
    let mut count = 0_usize;
    for child in children {
        count += 1;
        child.hash(&mut hasher);
    }
    count.hash(&mut hasher);
    hasher.finish()
}

struct StructureLocal<'a>(&'a ElementNode);

impl Hash for StructureLocal<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        element_kind_tag(&self.0.kind).hash(state);
        self.0.style.id.hash(state);
        self.0.children.len().hash(state);
    }
}

struct LayoutInputLocal<'a>(&'a ElementNode);

impl Hash for LayoutInputLocal<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        LayoutStyleInput(&self.0.style.layout).hash(state);
        self.0.style.visual.border_width.map(F32Hash).hash(state);
        self.0
            .style
            .visual
            .border_top_width
            .map(F32Hash)
            .hash(state);
        self.0
            .style
            .visual
            .border_right_width
            .map(F32Hash)
            .hash(state);
        self.0
            .style
            .visual
            .border_bottom_width
            .map(F32Hash)
            .hash(state);
        self.0
            .style
            .visual
            .border_left_width
            .map(F32Hash)
            .hash(state);

        match &self.0.kind {
            ElementKind::Div(_) | ElementKind::Timeline(_) | ElementKind::Canvas(_) => {}
            ElementKind::Text(text) => {
                text.text.hash(state);
                TextLayoutInput(&text.text_style).hash(state);
            }
            ElementKind::Bitmap(bitmap) => {
                bitmap.width.hash(state);
                bitmap.height.hash(state);
            }
            ElementKind::Lottie(lottie) => {
                lottie.width.hash(state);
                lottie.height.hash(state);
            }
            ElementKind::SvgPath(svg) => {
                svg.intrinsic_size
                    .map(|(w, h)| (F32Hash(w), F32Hash(h)))
                    .hash(state);
            }
        }
    }
}

struct PaintInputLocal<'a>(&'a ElementNode);

impl Hash for PaintInputLocal<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        PaintStyleInput(&self.0.style.visual).hash(state);

        match &self.0.kind {
            ElementKind::Div(_) => {}
            ElementKind::Timeline(tl) => {
                if let Some(transition) = &tl.transition {
                    F32Hash(transition.progress).hash(state);
                    std::mem::discriminant(&transition.kind).hash(state);
                }
            }
            ElementKind::Text(text) => {
                text.text.hash(state);
                TextPaintInput(&text.text_style).hash(state);
                text.text_unit_overrides.is_some().hash(state);
                if let Some(batch) = &text.text_unit_overrides {
                    std::mem::discriminant(&batch.granularity).hash(state);
                    for unit in &batch.overrides {
                        unit.opacity.map(f32::to_bits).hash(state);
                        unit.translate_x.map(f32::to_bits).hash(state);
                        unit.translate_y.map(f32::to_bits).hash(state);
                        unit.scale.map(f32::to_bits).hash(state);
                        unit.rotation_deg.map(f32::to_bits).hash(state);
                        unit.color.hash(state);
                    }
                }
            }
            ElementKind::Bitmap(bitmap) => {
                bitmap.asset_id.hash(state);
                bitmap.width.hash(state);
                bitmap.height.hash(state);
                bitmap.video_timing.hash(state);
            }
            ElementKind::Lottie(lottie) => {
                lottie.bundle_id.hash(state);
                lottie.width.hash(state);
                lottie.height.hash(state);
                lottie.fps.to_bits().hash(state);
                lottie.in_frame.to_bits().hash(state);
                lottie.out_frame.to_bits().hash(state);
                lottie.timing.hash(state);
            }
            ElementKind::Canvas(canvas) => {
                canvas.commands.hash(state);
            }
            ElementKind::SvgPath(svg) => {
                svg.path_data.hash(state);
                for value in svg.view_box {
                    F32Hash(value).hash(state);
                }
            }
        }

        self.0.draw_slot.commands.hash(state);
    }
}

/// composite/apply 三字段：opacity / transforms / backdrop_blur_sigma。
///
/// 不参与 paint 维度的任何缓存键，仅供 L3 (DisplayBuildSession) 判定
/// "DisplayNode 子树可整段复用" 时使用 —— DisplayNode 把这三字段烘到自身上,
/// 改了就必须刷新对应 DisplayNode。
struct ApplyInputLocal<'a>(&'a ElementNode);

impl Hash for ApplyInputLocal<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let visual = &self.0.style.visual;
        F32Hash(visual.opacity).hash(state);
        visual.transforms.hash(state);
        visual.css_filter.hash(state);
        visual.backdrop_blur_sigma.map(F32Hash).hash(state);
    }
}

struct LayoutStyleInput<'a>(&'a ComputedLayoutStyle);

impl Hash for LayoutStyleInput<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let style = self.0;
        style.position.hash(state);
        style.inset_left.hash(state);
        style.inset_top.hash(state);
        style.inset_right.hash(state);
        style.inset_bottom.hash(state);
        style.width.map(F32Hash).hash(state);
        style.width_percent.map(F32Hash).hash(state);
        style.height.map(F32Hash).hash(state);
        style.max_width.map(F32Hash).hash(state);
        style.width_full.hash(state);
        style.height_full.hash(state);
        F32Hash(style.padding_top).hash(state);
        F32Hash(style.padding_right).hash(state);
        F32Hash(style.padding_bottom).hash(state);
        F32Hash(style.padding_left).hash(state);
        style.margin_top.hash(state);
        style.margin_right.hash(state);
        style.margin_bottom.hash(state);
        style.margin_left.hash(state);
        style.min_height.hash(state);
        style.is_flex.hash(state);
        style.is_grid.hash(state);
        style.grid_template_columns.hash(state);
        style.grid_template_rows.hash(state);
        style.grid_auto_flow.hash(state);
        style.grid_auto_rows.hash(state);
        style.col_start.hash(state);
        style.col_end.hash(state);
        style.row_start.hash(state);
        style.row_end.hash(state);
        style.auto_size.hash(state);
        style.flex_direction.hash(state);
        style.justify_content.hash(state);
        style.align_items.hash(state);
        style.flex_wrap.hash(state);
        style.align_content.hash(state);
        style.align_self.hash(state);
        style.justify_items.hash(state);
        style.justify_self.hash(state);
        F32Hash(style.gap).hash(state);
        style.gap_x.map(F32Hash).hash(state);
        style.gap_y.map(F32Hash).hash(state);
        style.order.hash(state);
        style.aspect_ratio.map(F32Hash).hash(state);
        style.flex_basis.hash(state);
        F32Hash(style.flex_grow).hash(state);
        style.flex_shrink.map(F32Hash).hash(state);
        style.z_index.hash(state);
        style.truncate.hash(state);
    }
}

struct PaintStyleInput<'a>(&'a ComputedVisualStyle);

impl Hash for PaintStyleInput<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let style = self.0;
        style.background.hash(state);
        style.fill.hash(state);
        style.border_radius.hash(state);
        style.border_width.map(F32Hash).hash(state);
        style.border_top_width.map(F32Hash).hash(state);
        style.border_right_width.map(F32Hash).hash(state);
        style.border_bottom_width.map(F32Hash).hash(state);
        style.border_left_width.map(F32Hash).hash(state);
        style.border_color.hash(state);
        style.stroke_color.hash(state);
        style.stroke_width.map(F32Hash).hash(state);
        style.stroke_dasharray.map(F32Hash).hash(state);
        style.stroke_dashoffset.map(F32Hash).hash(state);
        style.border_style.hash(state);
        style.object_fit.hash(state);
        style.clip_contents.hash(state);
        style.clip_path.hash(state);
        style.box_shadow.hash(state);
        style.inset_shadow.hash(state);
        style.drop_shadow.hash(state);
        style.svg_path.hash(state);
    }
}

struct TextLayoutInput<'a>(&'a ComputedTextStyle);

impl Hash for TextLayoutInput<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let style = self.0;
        F32Hash(style.text_px).hash(state);
        style.font_weight.hash(state);
        F32Hash(style.letter_spacing).hash(state);
        F32Hash(style.line_height).hash(state);
        style.line_height_px.map(F32Hash).hash(state);
        style.text_transform.hash(state);
        style.wrap_text.hash(state);
    }
}

struct TextPaintInput<'a>(&'a ComputedTextStyle);

impl Hash for TextPaintInput<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let style = self.0;
        style.color.hash(state);
        F32Hash(style.text_px).hash(state);
        style.font_weight.hash(state);
        F32Hash(style.letter_spacing).hash(state);
        style.text_align.hash(state);
        F32Hash(style.line_height).hash(state);
        style.line_height_px.map(F32Hash).hash(state);
        style.text_transform.hash(state);
        style.wrap_text.hash(state);
        style.line_through.hash(state);
    }
}

fn element_kind_tag(kind: &ElementKind) -> u8 {
    match kind {
        ElementKind::Div(_) => 0,
        ElementKind::Timeline(_) => 1,
        ElementKind::Text(_) => 2,
        ElementKind::Bitmap(_) => 3,
        ElementKind::Canvas(_) => 4,
        ElementKind::SvgPath(_) => 5,
        ElementKind::Lottie(_) => 6,
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        FrameCtx,
        parse::{
            easing::Easing,
            primitives::{canvas, div, image, text},
            transition::{fade, slide, timeline},
        },
        probe::catalog::PreparedResourceCatalog,
        resolve::resolve::resolve_ui_tree,
        test_support::MockScriptHost,
    };

    fn frame_ctx() -> FrameCtx {
        FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 1,
        }
    }

    fn resolve(node: crate::Node) -> crate::resolve::tree::ElementNode {
        let mut assets = PreparedResourceCatalog::default();
        resolve_ui_tree(
            &node,
            &frame_ctx(),
            &mut assets,
            None,
            &mut MockScriptHost::default(),
        )
        .expect("tree should resolve")
    }

    fn resolve_at_frame(node: crate::Node, frame: u32) -> crate::resolve::tree::ElementNode {
        let mut assets = PreparedResourceCatalog::default();
        resolve_ui_tree(
            &node,
            &FrameCtx {
                frame,
                fps: 30,
                width: 320,
                height: 180,
                frames: 10,
            },
            &mut assets,
            None,
            &mut MockScriptHost::default(),
        )
        .expect("tree should resolve")
    }

    #[test]
    fn child_paint_change_only_changes_ancestor_paint_subtree() {
        let first = resolve(
            div()
                .id("root")
                .child(text("A").id("label").text_red())
                .into(),
        );
        let second = resolve(
            div()
                .id("root")
                .child(text("A").id("label").text_blue())
                .into(),
        );

        assert_eq!(
            first.fingerprints.layout_input_subtree,
            second.fingerprints.layout_input_subtree
        );
        assert_eq!(
            first.fingerprints.paint_input_local, second.fingerprints.paint_input_local,
            "parent local paint semantics should ignore child paint changes"
        );
        assert_ne!(
            first.fingerprints.paint_input_subtree, second.fingerprints.paint_input_subtree,
            "parent paint subtree should include child paint changes"
        );
        assert_ne!(
            first.children[0].fingerprints.paint_input_local,
            second.children[0].fingerprints.paint_input_local
        );
    }

    #[test]
    fn bitmap_asset_change_dirties_paint_but_not_layout_when_dimensions_match() {
        let first = resolve(
            div()
                .id("root")
                .child(image().id("photo").path("/tmp/a.png").w(20.0).h(20.0))
                .into(),
        );
        let second = resolve(
            div()
                .id("root")
                .child(image().id("photo").path("/tmp/b.png").w(20.0).h(20.0))
                .into(),
        );

        assert_eq!(
            first.fingerprints.layout_input_subtree,
            second.fingerprints.layout_input_subtree
        );
        assert_ne!(
            first.fingerprints.paint_input_subtree,
            second.fingerprints.paint_input_subtree
        );
    }

    #[test]
    fn canvas_hidden_child_paint_change_flows_into_canvas_paint_subtree() {
        let first = resolve(
            canvas()
                .id("canvas")
                .hidden_child(text("A").id("hidden").text_red().into())
                .into(),
        );
        let second = resolve(
            canvas()
                .id("canvas")
                .hidden_child(text("A").id("hidden").text_blue().into())
                .into(),
        );

        assert_eq!(first.fingerprints.node_count, 2);
        assert_eq!(
            first.fingerprints.layout_input_subtree,
            second.fingerprints.layout_input_subtree
        );
        assert_ne!(
            first.fingerprints.paint_input_subtree,
            second.fingerprints.paint_input_subtree
        );
    }

    #[test]
    fn opacity_change_affects_apply_input_only() {
        let first = resolve(
            div()
                .id("root")
                .child(text("A").id("label").opacity(1.0))
                .into(),
        );
        let second = resolve(
            div()
                .id("root")
                .child(text("A").id("label").opacity(0.5))
                .into(),
        );

        assert_eq!(
            first.fingerprints.paint_input_subtree, second.fingerprints.paint_input_subtree,
            "opacity must not leak into paint_input"
        );
        assert_eq!(
            first.fingerprints.layout_input_subtree, second.fingerprints.layout_input_subtree,
            "opacity must not leak into layout_input"
        );
        assert_ne!(
            first.fingerprints.apply_input_subtree, second.fingerprints.apply_input_subtree,
            "opacity change must move apply_input_subtree"
        );
        assert_ne!(
            first.children[0].fingerprints.apply_input_local,
            second.children[0].fingerprints.apply_input_local,
            "opacity change must move apply_input_local on the affected node"
        );
    }

    #[test]
    fn css_filter_change_affects_apply_input_only() {
        let first = resolve(
            div()
                .id("root")
                .child(text("A").id("label").filter_brightness(1.0))
                .into(),
        );
        let second = resolve(
            div()
                .id("root")
                .child(text("A").id("label").filter_brightness(0.5))
                .into(),
        );

        assert_eq!(
            first.fingerprints.paint_input_subtree, second.fingerprints.paint_input_subtree,
            "CSS filter must not be baked into paint_input"
        );
        assert_eq!(
            first.fingerprints.layout_input_subtree, second.fingerprints.layout_input_subtree,
            "CSS filter must not affect layout_input"
        );
        assert_ne!(
            first.fingerprints.apply_input_subtree, second.fingerprints.apply_input_subtree,
            "CSS filter changes must move apply_input_subtree"
        );
        assert_ne!(
            first.children[0].fingerprints.apply_input_local,
            second.children[0].fingerprints.apply_input_local,
            "CSS filter changes must move apply_input_local on the affected node"
        );
    }

    #[test]
    fn child_apply_change_propagates_to_ancestor_apply_subtree_only() {
        let first = resolve(
            div()
                .id("root")
                .child(text("A").id("label").opacity(1.0))
                .into(),
        );
        let second = resolve(
            div()
                .id("root")
                .child(text("A").id("label").opacity(0.2))
                .into(),
        );

        assert_eq!(
            first.fingerprints.apply_input_local, second.fingerprints.apply_input_local,
            "parent's own apply must not move when only child apply changes"
        );
        assert_ne!(
            first.fingerprints.apply_input_subtree, second.fingerprints.apply_input_subtree,
            "parent's apply_subtree must include child apply"
        );
    }

    #[test]
    fn transforms_change_only_moves_apply_dimension() {
        use crate::style::Transform;
        let first = resolve(
            div()
                .id("root")
                .child(
                    text("A")
                        .id("label")
                        .transform(Transform::RotateDeg { value: 0.0 }),
                )
                .into(),
        );
        let second = resolve(
            div()
                .id("root")
                .child(
                    text("A")
                        .id("label")
                        .transform(Transform::RotateDeg { value: 45.0 }),
                )
                .into(),
        );

        assert_eq!(
            first.fingerprints.paint_input_subtree,
            second.fingerprints.paint_input_subtree
        );
        assert_eq!(
            first.fingerprints.layout_input_subtree,
            second.fingerprints.layout_input_subtree
        );
        assert_ne!(
            first.fingerprints.apply_input_subtree,
            second.fingerprints.apply_input_subtree
        );
    }

    #[test]
    fn transition_progress_change_moves_paint_input_subtree() {
        let node: crate::Node = timeline()
            .sequence(3.0 / 30.0, div().id("scene_a").w(100.0).h(100.0).into())
            .transition(fade().timing(Easing::Linear, 4.0 / 30.0))
            .sequence(3.0 / 30.0, div().id("scene_b").w(100.0).h(100.0).into())
            .into();

        let at_frame_3 = resolve_at_frame(node.clone(), 3);
        let at_frame_5 = resolve_at_frame(node, 5);

        assert_ne!(
            at_frame_3.fingerprints.paint_input_subtree,
            at_frame_5.fingerprints.paint_input_subtree,
            "transition progress change must move paint_input_subtree so DisplayBuildSession cache invalidates"
        );
    }
}
