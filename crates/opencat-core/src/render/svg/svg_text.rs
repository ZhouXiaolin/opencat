//! SVG `<text>` 布局（Linear 流）。
//!
//! usvg 只做解析：x 列表展开成 chunk、span 样式继承求值、字符级
//! dx/dy/rotate 列表全在解析产物里（`tree.text.rs`）。shaping 与字形轮廓
//! 换 core 文本栈（cosmic-text + fontdb，与 JSX 文本共用 swash 字形缓存），
//! SVG 定位语义（text-anchor、dx/dy、rotate、textLength）在此实现。
//!
//! 竖排（writing-mode: tb）与 textPath（`TextFlow::Path`）尚未接管：出现
//! 时整元素回退 usvg 自带布局（`flattened()` glyph path 组），行为与退役
//! 前逐位一致；能力补齐后移除该分支。
//!
//! 坐标基准：text 元素的 transform 属性由 usvg 转成父 Group 承载，本模块
//! 只在当前 CTM（父 group 局部系）下排 glyph，不碰矩阵。

use std::sync::OnceLock;

use cosmic_text::Command;

use crate::canvas::paint::PaintSpec;
use crate::ir::draw_op::DrawOp;
use crate::ir::draw_types::{EncodedPath, FillType, PathOp};
use crate::render::builder::DrawOpBuilder;
use crate::style::{ComputedTextStyle, FontStyle, FontWeight};
use crate::text::{GlyphData, rasterize_glyphs};

use super::{fill_paint_of, render_group, stroke_paint_of};
use crate::text::FontTextMetrics;

/// baseline-shift + dominant/alignment-baseline 的合成 y 偏移
/// （usvg resolve_baseline 的等价实现，Chrome 硬编码公式同款）。
/// sub/super 取 os2 真值（FontTextMetrics 供货，缺表回退同 usvg）。
fn baseline_shift(m: &FontTextMetrics, span: &usvg::TextSpan) -> f32 {
    let mut shift = 0.0f32;
    for b in span.baseline_shift().iter().rev() {
        match b {
            usvg::BaselineShift::Baseline => {}
            usvg::BaselineShift::Subscript => shift -= m.subscript_offset,
            usvg::BaselineShift::Superscript => shift += m.superscript_offset,
            usvg::BaselineShift::Number(n) => shift += *n,
        }
    }
    let mut shift = -shift;
    shift += match span.alignment_baseline() {
        usvg::AlignmentBaseline::Auto | usvg::AlignmentBaseline::Baseline => {
            match span.dominant_baseline() {
                usvg::DominantBaseline::Central => {
                    m.ascent - (m.ascent - m.descent) * 0.5
                }
                usvg::DominantBaseline::Middle => m.x_height * 0.5,
                usvg::DominantBaseline::Hanging => m.ascent * 0.8,
                usvg::DominantBaseline::Mathematical => m.ascent * 0.5,
                usvg::DominantBaseline::Ideographic => m.descent,
                usvg::DominantBaseline::TextAfterEdge => m.descent,
                usvg::DominantBaseline::TextBeforeEdge => m.ascent,
                _ => 0.0,
            }
        }
        usvg::AlignmentBaseline::BeforeEdge | usvg::AlignmentBaseline::TextBeforeEdge => m.ascent,
        usvg::AlignmentBaseline::Middle => m.x_height * 0.5,
        usvg::AlignmentBaseline::Central => m.ascent - (m.ascent - m.descent) * 0.5,
        usvg::AlignmentBaseline::AfterEdge
        | usvg::AlignmentBaseline::TextAfterEdge
        | usvg::AlignmentBaseline::Ideographic => m.descent,
        usvg::AlignmentBaseline::Hanging => m.ascent * 0.8,
        usvg::AlignmentBaseline::Mathematical => m.ascent * 0.5,
        usvg::AlignmentBaseline::Alphabetic => 0.0,
    };
    shift
}

/// 进程级 SVG 字形字体库：系统字体扫描一次。与 usvg 解析侧（parse_tree
/// 的 fontdb）同一来源，family 匹配结果一致。
fn svg_font_db() -> &'static fontdb::Database {
    static DB: OnceLock<fontdb::Database> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        db
    })
}

pub(super) fn render_text(
    builder: &mut DrawOpBuilder,
    state: &mut super::EmitState,
    text: &usvg::Text,
) {
    // 竖排 / textPath：回退 usvg 布局（glyph path 组，含全部定位）。
    if text.writing_mode() != usvg::WritingMode::LeftToRight
        || text
            .chunks()
            .iter()
            .any(|c| matches!(c.text_flow(), usvg::TextFlow::Path(_)))
    {
        render_group(builder, state, text.flattened());
        return;
    }

    let db = svg_font_db();
    // shape 全部 chunk（纯函数，一次），顺带探测 color 字形（COLR/emoji
    // 位图）。color 字形回退 usvg 布局：flatten 时由 usvg 的 skrifa
    // painter 嵌成 SVG 子树，与 resvg 逐位同源；swash 的 ColorImage
    // 位图化在大字形上有可测的着色差（color-font 案 p8 2-7%）。
    let mut shapes = Vec::with_capacity(text.chunks().len());
    let has_color = {
        let mut found = false;
        let mut char_base = 0usize;
        for chunk in text.chunks() {
            let shape = shape_chunk(text, chunk, char_base, db);
            found |= shape.has_color;
            shapes.push(shape);
            char_base += chunk.text().chars().count();
        }
        found
    };
    if has_color {
        render_group(builder, state, text.flattened());
        return;
    }

    // 流式延续点：chunk 无 x/y 时从上一 chunk 可见终点续排（chunk 链语义）。
    let mut cursor: Option<(f32, f32)> = None;
    let mut char_base = 0usize;
    for (chunk, shape) in text.chunks().iter().zip(&shapes) {
        cursor = Some(layout_chunk(builder, text, chunk, shape, char_base, cursor));
        char_base += chunk.text().chars().count();
    }
}

/// 一个 span 切段 shape 后的字形。
struct SegGlyph {
    rel_x: f32,
    /// 视觉 advance（按 x 排序取相邻差；RTL 的 emission 序非单调）。
    adv: f32,
    /// 逻辑字符序号（chunk 内，供 text 级 dx/dy/rotate 列表查 index）。
    char_index: usize,
    /// swash 轮廓，px 域、y 向上、相对 baseline。
    outline: Vec<Command>,
}

struct Seg {
    glyphs: Vec<SegGlyph>,
    width: f32,
    span: usize,
    shift: f32,
    metrics: Option<FontTextMetrics>,
}

/// 一个 chunk 的 shape 产物：切段字形 + 字符级 dx/dy 累积偏移表。
struct ChunkShape {
    segs: Vec<Seg>,
    dxs: Vec<f32>,
    dys: Vec<f32>,
    has_color: bool,
}

/// span 切段 shaping + 字符级偏移表（纯函数，不碰 builder）。
fn shape_chunk(
    text: &usvg::Text,
    chunk: &usvg::TextChunk,
    char_base: usize,
    db: &fontdb::Database,
) -> ChunkShape {
    // dx/dy 列表是累积语义（usvg resolve_relative_shifts：字符 i 的偏移 =
    // 自 chunk 首字符起 dx/dy 逐项累加；x/y 列表已在解析期展开成 chunk，
    // 所以累加不跨 chunk）。rotate 不累积，逐字符独立。
    let n_chars = chunk.text().chars().count();
    let mut dxs = Vec::with_capacity(n_chars);
    let mut dys = Vec::with_capacity(n_chars);
    let (mut acc_x, mut acc_y) = (0.0f32, 0.0f32);
    for ci in 0..n_chars {
        acc_x += text.dx().get(char_base + ci).copied().unwrap_or(0.0);
        acc_y += text.dy().get(char_base + ci).copied().unwrap_or(0.0);
        dxs.push(acc_x);
        dys.push(acc_y);
    }

    let chunk_text = chunk.text();
    // chunk 文本的字节 → 字符序号表（cluster 首字节向下取整；1:1 字形即字符序号）。
    let byte_starts: Vec<(usize, usize)> = chunk_text
        .char_indices()
        .enumerate()
        .map(|(ci, (b, _))| (b, ci))
        .collect();
    let mut segs = Vec::new();
    let mut has_color = false;
    for (si, span) in chunk.spans().iter().enumerate() {
        let seg_text = chunk_text.get(span.start()..span.end()).unwrap_or("");
        if seg_text.is_empty() {
            continue;
        }
        let style = span_style(span, db);
        let r = rasterize_glyphs(seg_text, &style, f32::INFINITY, false, false, db);
        let metrics = r.metrics;
        let shift = metrics.as_ref().map_or(0.0, |m| baseline_shift(m, span));
        let Some(line) = r.lines.first() else {
            continue;
        };
        let mut glyphs = line
            .positions
            .iter()
            .filter_map(|pos| {
                let GlyphData::Outline(commands, _) = r.glyphs.get(&pos.cache_key)? else {
                    // COLR/emoji 位图：整元素回退 usvg 布局（见 render_text）。
                    has_color = true;
                    return None;
                };
                let char_index = byte_starts
                    .iter()
                    .rev()
                    .find(|(b, _)| *b <= pos.byte_range.start)
                    .map_or(0, |(_, c)| *c);
                Some(SegGlyph {
                    rel_x: pos.x_precise,
                    adv: 0.0,
                    char_index,
                    outline: commands.clone(),
                })
            })
            .collect::<Vec<_>>();
        // 字形 advance 按视觉 x 排序取相邻差（RTL 的 emission 序非单调，
        // 直接相邻差会把 advance 算成负值）；最右字形由段宽收尾。
        {
            let mut by_x: Vec<usize> = (0..glyphs.len()).collect();
            by_x.sort_by(|&i, &j| glyphs[i].rel_x.total_cmp(&glyphs[j].rel_x));
            let mut advs = vec![0.0f32; glyphs.len()];
            for (w, &i) in by_x.iter().enumerate() {
                advs[i] = match by_x.get(w + 1) {
                    Some(&next) => glyphs[next].rel_x - glyphs[i].rel_x,
                    None => line.width - glyphs[i].rel_x,
                };
            }
            for (g, adv) in glyphs.iter_mut().zip(advs) {
                g.adv = adv;
            }
        }
        segs.push(Seg {
            glyphs,
            width: line.width,
            span: si,
            shift,
            metrics,
        });
    }
    ChunkShape {
        segs,
        dxs,
        dys,
        has_color,
    }
}

/// 排一个 chunk（shape 已就绪）：流内布点（dx/dy/rotate/textLength）
/// → anchor 平移 → 装饰线与 glyph 轮廓落 DrawOp。返回续排点（可见终点）。
fn layout_chunk(
    builder: &mut DrawOpBuilder,
    text: &usvg::Text,
    chunk: &usvg::TextChunk,
    shape: &ChunkShape,
    char_base: usize,
    cursor: Option<(f32, f32)>,
) -> (f32, f32) {
    let start_x = chunk.x().unwrap_or(cursor.map_or(0.0, |c| c.0));
    let start_y = chunk.y().unwrap_or(cursor.map_or(0.0, |c| c.1));
    let (segs, dxs, dys) = (&shape.segs, &shape.dxs, &shape.dys);

    // 流内布点。f 是流内 x（自然流 + rel 排版 + dx 位移，不含 chunk 起点与
    // anchor）；adv 是流内 advance（dx 是位移不计入）。lengthAdjust=spacing
    // 的 textLength 在此拉伸段内位置与段宽。
    struct Placed {
        f: f32,
        adv: f32,
        y: f32,
        rotate: f32,
        outline: Vec<Command>,
        span: usize,
    }
    let mut placed: Vec<Placed> = Vec::new();
    // 各段的流内起点与拉伸系数（装饰线按段取宽）。
    let mut seg_geo: Vec<(f32, f32)> = Vec::with_capacity(segs.len());
    let mut pen = 0.0f32;
    let mut sga_k = 1.0f32; // SpacingAndGlyphs 的字形 x 向缩放
    for seg in segs {
        let span = &chunk.spans()[seg.span];
        let tl = span.text_length();
        let stretch = tl.map(|tl| tl / seg.width.max(1e-6)).unwrap_or(1.0);
        seg_geo.push((pen, stretch));
        for g in &seg.glyphs {
            let ci = g.char_index;
            placed.push(Placed {
                f: pen + g.rel_x * stretch + dxs.get(ci).copied().unwrap_or(0.0),
                adv: g.adv * stretch,
                y: start_y + seg.shift + dys.get(ci).copied().unwrap_or(0.0),
                rotate: text.rotate().get(char_base + ci).copied().unwrap_or(0.0),
                outline: g.outline.clone(),
                span: seg.span,
            });
        }
        pen += seg.width * stretch;
    }
    // SpacingAndGlyphs（单 span chunk）：位置与字形一并缩放到 textLength。
    // spacing 模式已在上面 stretch 处理；多 span 的 textLength 各段独立生效。
    if segs.len() == 1
        && chunk.spans()[segs[0].span].length_adjust() == usvg::LengthAdjust::SpacingAndGlyphs
        && let Some(tl) = chunk.spans()[segs[0].span].text_length()
        && pen > 1e-6
    {
        sga_k = tl / pen;
        for p in &mut placed {
            p.f *= sga_k;
            p.adv *= sga_k;
        }
    }

    // anchor：完全照抄 usvg layout 的种子式——绝对 x = start + base + f，
    // base = shift(首逻辑字符) - left + anchor，left/right 是含 shift 的
    // 可视极值（usvg resolve_clusters_positions_horizontal:607）。
    let (mut left, mut right) = (f32::INFINITY, f32::NEG_INFINITY);
    for p in &placed {
        left = left.min(p.f);
        right = right.max(p.f + p.adv);
    }
    if !left.is_finite() {
        (left, right) = (0.0, 0.0);
    }
    let width = (right - left).max(0.0);
    let anchor_off = match chunk.anchor() {
        usvg::TextAnchor::Start => 0.0,
        usvg::TextAnchor::Middle => -width / 2.0,
        usvg::TextAnchor::End => -width,
    };
    let base = dxs.first().copied().unwrap_or(0.0) - left + anchor_off;

    // 装饰线（underline/overline/line-through）：矩形挂字符基线（usvg
    // offset 公式：underline=-post、overline=-ascent、line-through=-os2
    // strikeout，厚度同 post）。字符级 dx/dy/rotate 全零的平段画一整条；
    // 含位移/旋转的段逐字符画（usvg 把线画在 cluster 变换系内，线随字符
    // 走）。先线后字。
    let mut pi = 0usize;
    for (si, seg) in segs.iter().enumerate() {
        let Some(m) = &seg.metrics else { continue };
        let deco = chunk.spans()[seg.span].decoration();
        let n = seg.glyphs.len();
        let run = &placed[pi..pi + n];
        pi += n;
        let (pen_start, stretch) = seg_geo[si];
        let lines = [
            deco.underline().map(|s| (-m.underline_offset, s)),
            deco.overline().map(|s| (-m.ascent, s)),
            deco.line_through().map(|s| (-m.strikeout_offset, s)),
        ];
        let thick = m.stroke_size;
        if thick <= 0.0 {
            continue;
        }
        let flat = run.iter().all(|p| p.rotate == 0.0 && p.y == start_y + seg.shift);
        for (dy_dec, style) in lines.into_iter().flatten() {
            if flat {
                let w = seg.width * stretch;
                if w <= 0.0 {
                    continue;
                }
                emit_deco_line(
                    builder,
                    start_x + pen_start + base,
                    start_y + seg.shift,
                    0.0,
                    w,
                    dy_dec,
                    thick,
                    style,
                );
            } else {
                for p in run.iter() {
                    if p.adv <= 0.0 {
                        continue;
                    }
                    emit_deco_line(
                        builder,
                        start_x + p.f + base,
                        p.y,
                        p.rotate,
                        p.adv,
                        dy_dec,
                        thick,
                        style,
                    );
                }
            }
        }
    }

    // glyph 轮廓 → path。swash 轮廓已是 px 域（size 缩放后）、y 相对
    // baseline（core HTML 侧同款用法：乘 upem/px 还原设计单位再重缩，
    // 这里跳过两步直接用 px 域）。同一 span 共享 paint（intern 去重），
    // fill/stroke 按 paint_order 各画一遍。
    let aa = true;
    for p in &placed {
        let ops = glyph_ops(&p.outline, p.rotate, sga_k, start_x + p.f + base, p.y);
        if ops.is_empty() {
            continue;
        }
        let path_id = builder.intern_path(EncodedPath {
            fill_type: FillType::Winding,
            ops,
        });
        let span = &chunk.spans()[p.span];
        let emit = |builder: &mut DrawOpBuilder, spec: PaintSpec| {
            let paint_id = builder.intern_paint(spec);
            builder.push(DrawOp::DrawPath {
                path: path_id,
                paint: paint_id,
            });
        };
        let fill = span.fill().and_then(|f| fill_paint_of(f, aa));
        let stroke = span.stroke().and_then(|s| stroke_paint_of(s, aa));
        match span.paint_order() {
            usvg::PaintOrder::StrokeAndFill => {
                if let Some(spec) = stroke {
                    emit(builder, spec);
                }
                if let Some(spec) = fill {
                    emit(builder, spec);
                }
            }
            _ => {
                if let Some(spec) = fill {
                    emit(builder, spec);
                }
                if let Some(spec) = stroke {
                    emit(builder, spec);
                }
            }
        }
    }

    (start_x + base + pen + dxs.last().copied().unwrap_or(0.0), start_y)
}

/// 一段装饰线矩形：基线原点 (cx, cy)，随 rot 旋转（同 glyph_ops 的旋转
/// 约定），矩形纵向 [dy-thick/2, dy+thick/2]（y 向下系）。
fn emit_deco_line(
    builder: &mut DrawOpBuilder,
    cx: f32,
    cy: f32,
    rot_deg: f32,
    w: f32,
    dy: f32,
    thick: f32,
    style: &usvg::TextDecorationStyle,
) {
    let (sin, cos) = if rot_deg == 0.0 {
        (0.0, 1.0)
    } else {
        let r = rot_deg.to_radians();
        (r.sin(), r.cos())
    };
    let map = |lx: f32, ly: f32| (cx + lx * cos - ly * sin, cy + lx * sin + ly * cos);
    let y0 = dy - thick / 2.0;
    let y1 = dy + thick / 2.0;
    let (x0, yy0) = map(0.0, y0);
    let (x1, yy1) = map(w, y0);
    let (x2, yy2) = map(w, y1);
    let (x3, yy3) = map(0.0, y1);
    let path_id = builder.intern_path(EncodedPath {
        fill_type: FillType::Winding,
        ops: vec![
            PathOp::MoveTo { x: x0, y: yy0 },
            PathOp::LineTo { x: x1, y: yy1 },
            PathOp::LineTo { x: x2, y: yy2 },
            PathOp::LineTo { x: x3, y: yy3 },
            PathOp::Close,
        ],
    });
    if let Some(fill) = style.fill()
        && let Some(spec) = fill_paint_of(fill, true)
    {
        let paint_id = builder.intern_paint(spec);
        builder.push(DrawOp::DrawPath {
            path: path_id,
            paint: paint_id,
        });
    }
    if let Some(stroke) = style.stroke()
        && let Some(spec) = stroke_paint_of(stroke, true)
    {
        let paint_id = builder.intern_paint(spec);
        builder.push(DrawOp::DrawPath {
            path: path_id,
            paint: paint_id,
        });
    }
}

/// usvg span 样式 → core 文本样式（shaping 输入）。family 解析按 usvg
/// 语义：name 栈逐个 fontdb query，全部 miss 落 Serif 兜底（usvg
/// default_font_selector 同款），把**命中的 face family 名**交给 shaping——
/// cosmic-text 的 script fallback 会选别的 face，字形与度量都会漂。
/// fill/stroke 颜色不走这里（paint 直接从 usvg::Fill/Stroke 转）。
fn span_style(span: &usvg::TextSpan, db: &fontdb::Database) -> ComputedTextStyle {
    let font = span.font();
    let mut families: Vec<fontdb::Family> = font
        .families()
        .iter()
        .map(|f| match f {
            usvg::FontFamily::Named(n) => fontdb::Family::Name(n),
            usvg::FontFamily::Serif => fontdb::Family::Serif,
            usvg::FontFamily::SansSerif => fontdb::Family::SansSerif,
            usvg::FontFamily::Monospace => fontdb::Family::Monospace,
            usvg::FontFamily::Cursive => fontdb::Family::Cursive,
            usvg::FontFamily::Fantasy => fontdb::Family::Fantasy,
        })
        .collect();
    families.push(fontdb::Family::Serif);
    let style = match font.style() {
        usvg::FontStyle::Normal => fontdb::Style::Normal,
        usvg::FontStyle::Italic | usvg::FontStyle::Oblique => fontdb::Style::Italic,
    };
    let query = fontdb::Query {
        families: &families,
        weight: fontdb::Weight(font.weight()),
        stretch: match font.stretch() {
            usvg::FontStretch::Condensed => fontdb::Stretch::Condensed,
            usvg::FontStretch::SemiCondensed => fontdb::Stretch::SemiCondensed,
            usvg::FontStretch::ExtraCondensed => fontdb::Stretch::ExtraCondensed,
            usvg::FontStretch::UltraCondensed => fontdb::Stretch::UltraCondensed,
            usvg::FontStretch::SemiExpanded => fontdb::Stretch::SemiExpanded,
            usvg::FontStretch::Expanded => fontdb::Stretch::Expanded,
            usvg::FontStretch::ExtraExpanded => fontdb::Stretch::ExtraExpanded,
            usvg::FontStretch::UltraExpanded => fontdb::Stretch::UltraExpanded,
            usvg::FontStretch::Normal => fontdb::Stretch::Normal,
        },
        style,
    };
    let family = db
        .query(&query)
        .and_then(|id| db.face(id))
        .and_then(|face| face.families.first().map(|(name, _)| name.clone()));
    ComputedTextStyle {
        text_px: span.font_size().get(),
        font_family: family,
        font_generic: None,
        font_weight: FontWeight(font.weight()),
        font_style: match font.style() {
            usvg::FontStyle::Normal => FontStyle::Normal,
            usvg::FontStyle::Italic | usvg::FontStyle::Oblique => FontStyle::Italic,
        },
        letter_spacing: span.letter_spacing(),
        // 单行 shaping，行高不参与定位。
        line_height: 1.2,
        ..ComputedTextStyle::default()
    }
}

/// 轮廓（px 域，y 向上，相对 baseline）→ 旋转/平移后的 PathOp（y 向下系）。
/// 旋转绕 baseline 原点，正角顺时针；`xscale` 供 SpacingAndGlyphs 拉伸。
fn glyph_ops(
    outline: &[Command],
    rotate_deg: f32,
    xscale: f32,
    tx: f32,
    ty: f32,
) -> Vec<PathOp> {
    let (sin, cos) = if rotate_deg == 0.0 {
        (0.0, 1.0)
    } else {
        let r = rotate_deg.to_radians();
        (r.sin(), r.cos())
    };
    let map = |p: swash::zeno::Point| -> (f32, f32) {
        let x = p.x * xscale;
        let y = -p.y;
        (tx + x * cos - y * sin, ty + x * sin + y * cos)
    };
    outline
        .iter()
        .map(|cmd| match *cmd {
            Command::MoveTo(p) => {
                let (x, y) = map(p);
                PathOp::MoveTo { x, y }
            }
            Command::LineTo(p) => {
                let (x, y) = map(p);
                PathOp::LineTo { x, y }
            }
            Command::QuadTo(c, p) => {
                let (cx, cy) = map(c);
                let (x, y) = map(p);
                PathOp::QuadTo { cx, cy, x, y }
            }
            Command::CurveTo(c1, c2, p) => {
                let (c1x, c1y) = map(c1);
                let (c2x, c2y) = map(c2);
                let (x, y) = map(p);
                PathOp::CubicTo {
                    c1x,
                    c1y,
                    c2x,
                    c2y,
                    x,
                    y,
                }
            }
            Command::Close => PathOp::Close,
        })
        .collect()
}
