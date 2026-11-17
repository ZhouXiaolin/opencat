use std::hash::{Hash, Hasher};

use crate::{
    fp::{FpDim, FpF32},
    resolve::tree::{ElementKind, ElementNode},
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
        // ComputedLayoutStyle 的 Layout 维度字段清单由 fp_struct! 声明序派生。
        self.0.style.layout.fp_hash_dim(FpDim::Layout, state);
        // CSS 直通腿是布局消费端的第一优先输入；taffy Style 无 Hash，
        // Debug 文本足够稳定（同一份代码产出的字段集合固定）。
        if let Some(css) = &self.0.style.css_layout {
            format!("{:?}", css.0).hash(state);
        }
        // 边框宽在布局与绘制两个维度都参与（历史行为）；visual 侧的
        // Paint 标注管绘制维度，这里的布局维度消费跨结构体手写。
        let visual = &self.0.style.visual;
        visual.border_width.map(FpF32).hash(state);
        visual.border_top_width.map(FpF32).hash(state);
        visual.border_right_width.map(FpF32).hash(state);
        visual.border_bottom_width.map(FpF32).hash(state);
        visual.border_left_width.map(FpF32).hash(state);

        match &self.0.kind {
            ElementKind::Div(_) | ElementKind::Timeline(_) | ElementKind::Canvas(_) => {}
            ElementKind::Text(text) => {
                text.text.hash(state);
                text.text_style.fp_hash_dim(FpDim::TextLayout, state);
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
                    .map(|(w, h)| (FpF32(w), FpF32(h)))
                    .hash(state);
            }
            ElementKind::Svg(svg) => {
                svg.intrinsic_size
                    .map(|(w, h)| (FpF32(w), FpF32(h)))
                    .hash(state);
                svg.src.hash(state);
                svg.inline.hash(state);
            }
        }
    }
}

struct PaintInputLocal<'a>(&'a ElementNode);

impl Hash for PaintInputLocal<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // ComputedVisualStyle 的 Paint 维度字段清单由 fp_struct! 声明序派生。
        self.0.style.visual.fp_hash_dim(FpDim::Paint, state);

        match &self.0.kind {
            ElementKind::Div(_) => {}
            ElementKind::Timeline(tl) => {
                if let Some(transition) = &tl.transition {
                    FpF32(transition.progress).hash(state);
                    std::mem::discriminant(&transition.kind).hash(state);
                }
            }
            ElementKind::Text(text) => {
                text.text.hash(state);
                text.text_style.fp_hash_dim(FpDim::TextPaint, state);
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
                    FpF32(value).hash(state);
                }
            }
            ElementKind::Svg(svg) => {
                svg.preserve_aspect_ratio.hash(state);
                svg.inline.hash(state);
            }
        }

        self.0.draw_slot.commands.hash(state);
    }
}

/// composite/apply 维度：opacity / transforms / origin / perspective /
/// preserve-3d / backface / filter / backdrop / mask（字段清单派生自
/// ComputedVisualStyle 的 `[Apply]` 标注）。
///
/// 不参与 paint 维度的任何缓存键，仅供 L3 (DisplayBuildSession) 判定
/// "DisplayNode 子树可整段复用" 时使用 —— DisplayNode 把这些字段烘到自身上,
/// 改了就必须刷新对应 DisplayNode。
struct ApplyInputLocal<'a>(&'a ElementNode);

impl Hash for ApplyInputLocal<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Apply 维度（opacity / transforms / origin / perspective / filter /
        // backdrop / mask）的 13 项字段清单由 fp_struct! 声明序派生。
        // 注意：字段序与旧手写序不同 → 哈希值变化，但键只做同输入等值
        // 比较（纯内存、不序列化），行为不变。
        self.0.style.visual.fp_hash_dim(FpDim::Apply, state);
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
        ElementKind::Svg(_) => 7,
        ElementKind::Lottie(_) => 6,
    }
}
