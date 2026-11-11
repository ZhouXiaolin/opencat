//! HTML 行内格式化上下文（IFC）：文本 run 与原子行内盒折行、基线对齐。
//!
//! taffy 没有行内布局。行内流容器在 taffy 里按单行 flex row 求出子盒尺寸
//! （原子盒 shrink-to-fit），这里按容器最终内容宽重排成行：
//! - 文本按 UAX#14 断点切段，贪心填行；行尾可折叠空格悬挂、不计宽；
//! - 行高按 CSS 行盒模型：strut 与各 run/原子盒在基线上下的伸展取最大；
//! - 跨行的文本 run 产出逐行片段，渲染端逐片段单行绘制。

use std::ops::Range;

use hashbrown::HashMap;
use unicode_linebreak::BreakOpportunity;

use crate::style::{ComputedTextStyle, TextAlign, VerticalAlign};
use crate::text::{LineMeasure, compute_line_x_shift, measure_line};

/// 断行容差：分段量宽与整段量宽的舍入差不应把恰好放下的内容挤到下一行。
const FIT_EPSILON: f32 = 0.5;

/// 原子盒在 UAX#14 里的替身（OBJECT REPLACEMENT CHARACTER，前后可断）。
const OBJECT_CHAR: char = '\u{FFFC}';

/// 行内项（容器在流子项的文档序）。
pub(crate) enum InlineItem<'a> {
    /// 可断行文本 run。
    Text {
        text: &'a str,
        style: &'a ComputedTextStyle,
        /// `white-space: nowrap`：run 内部不断行。
        nowrap: bool,
    },
    /// 原子行内盒：margin box 宽高、基线距 margin box 顶的距离。
    Atomic {
        width: f32,
        height: f32,
        baseline: f32,
        align: VerticalAlign,
    },
    /// 脱流子项（绝对定位）：不参与排行。
    Skip,
}

/// 文本 run 在一行上的片段（容器内容区坐标）。
pub(crate) struct Fragment {
    pub range: Range<usize>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// 基线距片段顶。
    pub baseline: f32,
}

pub(crate) enum Placement {
    Text(Vec<Fragment>),
    /// 原子盒 margin box 左上角（容器内容区坐标）。
    Atomic { x: f32, y: f32 },
    Skip,
}

pub(crate) struct InlineLayout {
    pub placements: Vec<Placement>,
    /// 各行行盒高之和。
    pub height: f32,
    /// 末行基线（内容区坐标）。
    pub last_baseline: f32,
}

/// 一行里的一段：某 item 的字节区间（原子盒为 `0..0`）。
struct Part {
    item: usize,
    range: Range<usize>,
}

/// 断点之间的不可断段。
struct Segment {
    parts: Vec<Part>,
    /// 前进宽（含段尾空格）。
    width: f32,
    /// 段尾可折叠空格悬挂后的宽（放在行尾时的占宽）。
    trimmed: f32,
    /// 段后是强制换行。
    forced: bool,
}

/// 文本量宽缓存：同一 run 的同一区间在一次排版里反复量（断行、片段、对齐）。
struct Measurer<'a, 'b> {
    items: &'b [InlineItem<'a>],
    font_db: &'b fontdb::Database,
    cache: HashMap<(usize, usize, usize), LineMeasure>,
}

impl Measurer<'_, '_> {
    fn text(&mut self, item: usize, range: Range<usize>) -> LineMeasure {
        let items = self.items;
        let InlineItem::Text { text, style, .. } = &items[item] else {
            unreachable!("text measure on non-text item");
        };
        let font_db = self.font_db;
        *self
            .cache
            .entry((item, range.start, range.end))
            .or_insert_with(|| measure_line(&text[range], style, font_db))
    }

    /// 片段放在行尾时的占宽：可折叠尾空格悬挂。
    fn trimmed_width(&mut self, item: usize, range: Range<usize>) -> f32 {
        let items = self.items;
        let InlineItem::Text { text, .. } = &items[item] else {
            return self.part_width(item, range);
        };
        let trimmed =
            text[range.clone()].trim_end_matches(|c: char| c.is_whitespace() && c != '\u{a0}');
        if trimmed.len() == range.end - range.start {
            return self.text(item, range).width;
        }
        if trimmed.is_empty() {
            return 0.0;
        }
        self.text(item, range.start..range.start + trimmed.len()).width
    }

    fn part_width(&mut self, item: usize, range: Range<usize>) -> f32 {
        let items = self.items;
        match &items[item] {
            InlineItem::Text { .. } => self.text(item, range).width,
            InlineItem::Atomic { width, .. } => *width,
            InlineItem::Skip => 0.0,
        }
    }
}

/// 行内排版。`wrap_width` 为 `INFINITY` 时不折行（nowrap 容器）；
/// `align_width` 是 text-align 的对齐基准（容器内容宽）。
pub(crate) fn layout_inline(
    items: &[InlineItem<'_>],
    wrap_width: f32,
    align_width: f32,
    strut: &ComputedTextStyle,
    text_align: TextAlign,
    font_db: &fontdb::Database,
) -> InlineLayout {
    let mut m = Measurer {
        items,
        font_db,
        cache: HashMap::default(),
    };
    let segments = segments(items, &mut m);
    let lines = fill_lines(&segments, wrap_width);

    let strut_m = measure_line("x", strut, font_db);
    let mut placements: Vec<Placement> = items
        .iter()
        .map(|item| match item {
            InlineItem::Text { .. } => Placement::Text(Vec::new()),
            InlineItem::Atomic { .. } => Placement::Atomic { x: 0.0, y: 0.0 },
            InlineItem::Skip => Placement::Skip,
        })
        .collect();

    let mut y = 0.0f32;
    let mut last_baseline = strut_m.baseline;
    for line in &lines {
        // 同一 item 的相邻部分并成一个片段（跨段的同一 run 在行内连续）。
        let mut parts: Vec<Part> = Vec::new();
        for part in line.iter().flat_map(|&seg| segments[seg].parts.iter()) {
            match parts.last_mut() {
                Some(last) if last.item == part.item && last.range.end == part.range.start => {
                    last.range.end = part.range.end;
                }
                _ => parts.push(Part {
                    item: part.item,
                    range: part.range.clone(),
                }),
            }
        }
        // 水平：逐片段前进；行尾片段的尾空格悬挂（只影响对齐宽）。
        let mut xs = Vec::with_capacity(parts.len());
        let mut cursor = 0.0f32;
        for part in &parts {
            xs.push(cursor);
            cursor += m.part_width(part.item, part.range.clone());
        }
        let line_w = match parts.last() {
            Some(last) => {
                cursor - m.part_width(last.item, last.range.clone())
                    + m.trimmed_width(last.item, last.range.clone())
            }
            None => 0.0,
        };
        let shift = compute_line_x_shift(line_w, align_width, text_align);

        // 垂直：strut 与基线对齐项的上下伸展；top/bottom 对齐项最后撑行高。
        let mut above = strut_m.baseline;
        let mut below = strut_m.line_height - strut_m.baseline;
        let mut edge_aligned_max = 0.0f32;
        for part in &parts {
            match &items[part.item] {
                InlineItem::Text { .. } => {
                    let lm = m.text(part.item, part.range.clone());
                    above = above.max(lm.baseline);
                    below = below.max(lm.line_height - lm.baseline);
                }
                InlineItem::Atomic {
                    height,
                    baseline,
                    align,
                    ..
                } => match atomic_extent(*height, *baseline, *align, strut) {
                    Some((a, b)) => {
                        above = above.max(a);
                        below = below.max(b);
                    }
                    None => edge_aligned_max = edge_aligned_max.max(*height),
                },
                InlineItem::Skip => {}
            }
        }
        // CSS：top/bottom 对齐盒高于行时，行盒向下长高（基线不动）。
        let line_h = (above + below).max(edge_aligned_max);
        let baseline_y = y + above;

        for (part, x) in parts.iter().zip(xs) {
            let x = x + shift;
            match (&items[part.item], &mut placements[part.item]) {
                (InlineItem::Text { .. }, Placement::Text(frags)) => {
                    let lm = m.text(part.item, part.range.clone());
                    frags.push(Fragment {
                        range: part.range.clone(),
                        x,
                        y: baseline_y - lm.baseline,
                        width: lm.width,
                        height: lm.line_height,
                        baseline: lm.baseline,
                    });
                }
                (
                    InlineItem::Atomic {
                        height,
                        baseline,
                        align,
                        ..
                    },
                    Placement::Atomic { x: px, y: py },
                ) => {
                    *px = x;
                    *py = match align {
                        VerticalAlign::Top => y,
                        VerticalAlign::Bottom => y + line_h - *height,
                        _ => match atomic_extent(*height, *baseline, *align, strut) {
                            Some((a, _)) => baseline_y - a,
                            None => y,
                        },
                    };
                }
                _ => {}
            }
        }
        last_baseline = baseline_y;
        y += line_h;
    }

    InlineLayout {
        placements,
        height: y,
        last_baseline,
    }
}

/// 原子盒在基线上/下的伸展。top/bottom 对齐（与行盒边对齐）返回 `None`。
fn atomic_extent(
    height: f32,
    baseline: f32,
    align: VerticalAlign,
    strut: &ComputedTextStyle,
) -> Option<(f32, f32)> {
    let above = match align {
        VerticalAlign::Top | VerticalAlign::Bottom => return None,
        VerticalAlign::Baseline => baseline,
        // 盒中线对齐父基线上方半个 x-height（x-height 近似 0.5em）。
        VerticalAlign::Middle => height * 0.5 + strut.text_px * 0.25,
        // text-top/text-bottom 近似为盒顶/盒底贴父内容区（ascent≈0.8em，descent≈0.2em）。
        VerticalAlign::TextTop => strut.text_px * 0.8,
        VerticalAlign::TextBottom => height - strut.text_px * 0.2,
        VerticalAlign::Shift(px) => baseline + px,
    };
    Some((above, height - above))
}

/// 拼接全部在流项（原子盒用 U+FFFC 占位）→ UAX#14 断点 → 不可断段。
fn segments(items: &[InlineItem<'_>], m: &mut Measurer<'_, '_>) -> Vec<Segment> {
    // 拼接串上每个 item 的字节区间。
    let mut joined = String::new();
    let mut spans: Vec<(usize, Range<usize>)> = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let start = joined.len();
        match item {
            InlineItem::Text { text, .. } => joined.push_str(text),
            InlineItem::Atomic { .. } => joined.push(OBJECT_CHAR),
            InlineItem::Skip => continue,
        }
        spans.push((index, start..joined.len()));
    }
    let span_at = |pos: usize| spans.iter().position(|(_, r)| r.start <= pos && pos < r.end);

    let mut out = Vec::new();
    let mut prev = 0usize;
    for (pos, kind) in unicode_linebreak::linebreaks(&joined) {
        let forced = kind == BreakOpportunity::Mandatory && pos < joined.len();
        // nowrap 侧的可选断点作废（段继续延伸）。white-space 可继承：断点两
        // 侧任一 run 是 nowrap（如 `.hl{white-space:nowrap}` 内的 stem span 与
        // 兄弟直接文本）都不可断——figma act2 的高亮短语正是跨界空格断行。
        if !forced
            && pos > 0
            && pos < joined.len()
            && let (Some(before), Some(after)) = (span_at(pos - 1), span_at(pos))
            && [before, after].into_iter().any(|span| {
                matches!(items[spans[span].0], InlineItem::Text { nowrap: true, .. })
            })
        {
            continue;
        }
        let mut parts = Vec::new();
        for (index, span) in &spans {
            let start = span.start.max(prev);
            let end = span.end.min(pos);
            if start >= end {
                continue;
            }
            let range = match items[*index] {
                InlineItem::Text { .. } => start - span.start..end - span.start,
                _ => 0..0,
            };
            parts.push(Part {
                item: *index,
                range,
            });
        }
        prev = pos;
        if parts.is_empty() {
            continue;
        }
        let width: f32 = parts
            .iter()
            .map(|p| m.part_width(p.item, p.range.clone()))
            .sum();
        let last = parts.last().expect("non-empty");
        let trimmed = width - m.part_width(last.item, last.range.clone())
            + m.trimmed_width(last.item, last.range.clone());
        out.push(Segment {
            parts,
            width,
            trimmed,
            forced,
        });
    }
    out
}

/// 贪心填行：段放不下（行尾悬挂空格不计）就换行；行首段超宽照放（溢出）。
fn fill_lines(segments: &[Segment], wrap_width: f32) -> Vec<Vec<usize>> {
    let mut lines: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut used = 0.0f32;
    for (index, seg) in segments.iter().enumerate() {
        if !current.is_empty() && used + seg.trimmed > wrap_width + FIT_EPSILON {
            lines.push(std::mem::take(&mut current));
            used = 0.0;
        }
        current.push(index);
        used += seg.width;
        if seg.forced {
            lines.push(std::mem::take(&mut current));
            used = 0.0;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}
