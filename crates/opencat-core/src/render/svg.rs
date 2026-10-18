//! SVG 后端：`usvg::Tree` → DrawOp 序列。
//!
//! 解析交给 usvg（本地 resvg 检出，锚点 054ccf39），绘制侧把树遍历翻译成
//! DrawOp（PLAN §2/§3）。resvg 的 tiny-skia renderer 是移植参考：
//! 组语义按 `should_isolate` 分离，隔离组走 `SaveLayer`；形状遮罩走
//! `ClipPath`；滤镜/图案是 P2/P4 范围，遇到时降级（正常画内容，不套效果）。
//!
//! `EmitState.ts` 是执行端 CTM 的镜像：SaveLayer 的层界、渐变 user-space
//! 坐标推导等需要绝对矩阵时从它读，op 流里的 Concat 与它同步推进。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use tiny_skia_path::{PathSegment, Transform};
use usvg::ImageKind;

use crate::canvas::paint::{
    BlendMode, FillSpec, PaintSpec, PaintStyle, PathEffectSpec, ShaderSpec, StrokeCap, StrokeJoin,
    StrokeSpec, TileMode,
};
use crate::ir::draw_op::{DrawOp, Rect4};
use crate::ir::draw_types::{EncodedPath, FillType, ImageRef, PathOp};
use crate::render::builder::DrawOpBuilder;

// ---------------------------------------------------------------------------
// Tree 加载与缓存
// ---------------------------------------------------------------------------

/// 进程级树缓存：一个 svg 路径解析一次。渲染进程是短命的（一条命令一组
/// 帧），Mutex 争用可以忽略。
static TREE_CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<usvg::Tree>>>> = OnceLock::new();

/// 解析并缓存一棵树。`src` 是 svg 文件路径；相对引用（`<image href>`）相对
/// 它自身目录解析（PLAN §4），由 `resources_dir` 保证。
pub fn load_tree(src: &Path) -> Option<Arc<usvg::Tree>> {
    let cache = TREE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(tree) = cache.lock().ok()?.get(src) {
        return Some(tree.clone());
    }
    let text = std::fs::read_to_string(src).ok()?;
    let mut opt = usvg::Options::default();
    opt.resources_dir = src.parent().map(|d| d.to_path_buf());
    // resvg CLI 默认加载系统字体；text→glyph path 的对齐质量归 P5。
    opt.fontdb = Arc::new({
        let mut db = usvg::fontdb::Database::new();
        db.load_system_fonts();
        db
    });
    let tree = usvg::Tree::from_str(&text, &opt).ok()?;
    let tree = Arc::new(tree);
    cache.lock().ok()?.insert(src.to_path_buf(), tree.clone());
    Some(tree)
}

/// svg 的 intrinsic 渲染尺寸（`tree.size()`，width/height/viewBox 归一后的
/// 画布尺寸；resvg CLI 输出 PNG 用同一尺寸）。
pub fn tree_size(tree: &usvg::Tree) -> (f32, f32) {
    let s = tree.size();
    (s.width(), s.height())
}

// ---------------------------------------------------------------------------
// 发射入口
// ---------------------------------------------------------------------------

/// 把整棵树画进 `builder`，视口映射到 `dst`（元素盒子，画布坐标）。
///
/// usvg 已把根 viewBox→viewport 的映射（含 preserveAspectRatio）编码进根
/// group 的 `abs_transform`，所以这里只做 tree.size()→dst 的盒子缩放。
pub fn emit_svg(
    builder: &mut DrawOpBuilder,
    src: &Path,
    tree: &usvg::Tree,
    dst: Rect4,
) {
    let (tw, th) = tree_size(tree);
    if tw <= 0.0 || th <= 0.0 || dst.width <= 0.0 || dst.height <= 0.0 {
        return;
    }
    let mut state = EmitState {
        ts: Transform::identity(),
    };
    builder.push(DrawOp::Save);
    // svg 元素自身是视口：内容裁在盒子里（overflow:hidden 语义）。
    builder.push(DrawOp::BeginPath);
    builder.push(DrawOp::Path(PathOp::AddRect {
        x: dst.x,
        y: dst.y,
        width: dst.width,
        height: dst.height,
    }));
    builder.push(DrawOp::ClipPath { anti_alias: true, fill_type: Default::default() });
    concat(builder, &mut state, {
        Transform::from_scale(dst.width / tw, dst.height / th)
            .pre_translate(dst.x, dst.y)
    });
    render_group(builder, &mut state, tree.root());
    builder.push(DrawOp::Restore);
    if std::env::var("OC_SVG_DEBUG").is_ok() {
        let ops = builder.ops_debug();
        eprintln!("[svg] emit done, {} ops total:", ops.len());
        for op in ops {
            eprintln!("  {:?}", op);
        }
        eprintln!("[svg] paints:");
        for (i, p) in builder.paints_debug().iter().enumerate() {
            eprintln!("  [{}] {:?}", i, p);
        }
    }
}

struct EmitState {
    ts: Transform,
}

/// CTM 前乘 + 同步发出 Concat。DrawOp::Concat 的执行端按**列主序**读
/// （replay: `new_all(m0, m3, m6, m1, m4, m7, m2, m5, m8)`），与
/// ShaderSpec local_matrix 的行主序约定不同，见 executor/paint.rs。
fn concat(builder: &mut DrawOpBuilder, state: &mut EmitState, t: Transform) {
    state.ts = state.ts.pre_concat(t);
    builder.push(DrawOp::Concat {
        matrix: [t.sx, t.ky, 0.0, t.kx, t.sy, 0.0, t.tx, t.ty, 1.0],
    });
}

// ---------------------------------------------------------------------------
// 组与节点遍历（参考 resvg render.rs）
// ---------------------------------------------------------------------------

fn render_nodes(
    builder: &mut DrawOpBuilder,
    state: &mut EmitState,
    parent: &usvg::Group,
) {
    for node in parent.children() {
        render_node(builder, state, node);
    }
}

fn render_node(
    builder: &mut DrawOpBuilder,
    state: &mut EmitState,
    node: &usvg::Node,
) {
    match node {
        usvg::Node::Group(group) => {
            render_group(builder, state, group);
        }
        usvg::Node::Path(path) => {
            render_path(builder, path);
        }
        usvg::Node::Image(image) => {
            render_image(builder, state, image);
        }
        usvg::Node::Text(text) => {
            // usvg 已把 text 排版成 glyph path 组（PLAN §3：按 path 走）。
            render_group(builder, state, text.flattened());
        }
    }
}

/// clipPath 的有效子形状数（resvg clip.rs draw_children 的同类集合：
/// Path/Text/Group；Image 等忽略）。Text 按展开后的 glyph 数计——多字形
/// 是 union 语义，必须走 mask 化路径（逐个 ClipPath 求交会剪成空集）。
fn clip_shape_count(group: &usvg::Group) -> usize {
    group
        .children()
        .iter()
        .map(|n| match n {
            usvg::Node::Image(_) => 0,
            usvg::Node::Group(g) => clip_shape_count(g),
            usvg::Node::Text(t) => clip_shape_count(t.flattened()),
            usvg::Node::Path(_) => 1,
        })
        .sum()
}

fn render_group(
    builder: &mut DrawOpBuilder,
    state: &mut EmitState,
    group: &usvg::Group,
) {
    let outer_ts = state.ts;
    if std::env::var("OC_SVG_DEBUG").is_ok() {
        eprintln!("[svg] group ts={:?} isolate={} children={}", (group.transform().sx, group.transform().kx, group.transform().ky, group.transform().sy, group.transform().tx, group.transform().ty), group.should_isolate(), group.children().len());
    }
    // 多子形状的 clipPath 转 mask（见 emit_clip）需要层载体（DstIn 挂在
    // SaveLayer 上），非隔离组此时也要走隔离路径。
    let mask_clip = group
        .clip_path()
        .is_some_and(|c| clip_shape_count(c.root()) > 1);
    // Save 先于组矩阵落盘：执行端 Restore 回滚到进组前的 CTM。若 Save 晚于
    // Concat，Restore 只能恢复到组矩阵生效后，矩阵会泄漏给后续兄弟
    // （use x=60 的兄弟叠加成 y=120→180 出界的实例教训）。隔离组的
    // SaveLayer 也一样隐含 save，必须吃在 Concat 前面。
    builder.push(DrawOp::Save);
    concat(builder, state, group.transform());

    if !group.should_isolate() && !mask_clip {
        // 非隔离组：clip 是唯一需要落盘的状态（几何裁剪，与 resvg 的
        // 隔离层路径相比少一次离屏，结果一致）。
        if let Some(clip) = group.clip_path() {
            emit_clip(builder, state, clip);
        }
        render_nodes(builder, state, group);
        builder.push(DrawOp::Restore);
        state.ts = outer_ts;
        return;
    }

    // 隔离组：SaveLayer{alpha, blend} → clip → mask → 内容 → Restore。
    // resvg 的层序是 内容→filter→clip→mask 后合成；本引擎 filter 挂层
    // paint（P4），形状 mask 折算为 clip（PLAN §3），因此 clip 先于内容。
    // 载体 paint 的颜色 alpha 必须是 1：执行端把它当层合成 alpha 用，
    // 组透明度走独立的 `alpha` 字段（SaveLayer 语义见 replay.rs）。
    let mut paint = PaintSpec {
        fill: FillSpec::Solid([0.0, 0.0, 0.0, 1.0]),
        ..PaintSpec::default()
    };
    paint.blend_mode = convert_blend_mode(group.blend_mode());
    let paint_id = builder.intern_paint(paint);
    builder.push(DrawOp::SaveLayer {
        bounds: None,
        paint: Some(paint_id),
        alpha: group.opacity().get(),
        backdrop: false,
    });
    if let Some(clip) = group.clip_path() {
        emit_clip(builder, state, clip);
    }
    if let Some(mask) = group.mask() {
        emit_mask(builder, state, mask);
    }
    render_nodes(builder, state, group);
    // 两个栈帧各弹一次：SaveLayer 隐含 save，组自身的 Save 也要配平——
    // 少发一个会让组矩阵与 clip 泄漏给后续兄弟（nested-svg stroke 框被
    // 平移 (50,50) 的实例教训）。
    builder.push(DrawOp::Restore);
    builder.push(DrawOp::Restore);
    state.ts = outer_ts;
}

// ---------------------------------------------------------------------------
// clip（形状遮罩 → ClipPath）
// ---------------------------------------------------------------------------

/// `<mask>` 的 emit：内容子树（user space，与使用处同系）+ kind + region
/// 随 `MaskClip` 下发；嵌套 mask（mask.mask()）展开为串联的多个 MaskClip
/// （resvg mask.rs：region 剪内容 → 内层 mask → type 混合，串联求交等价）。
fn emit_mask(builder: &mut DrawOpBuilder, state: &mut EmitState, mask: &usvg::Mask) {
    // mask 有内容才有遮罩意义；空 root 是"整元素遮没"（resvg 同），
    // 用空 clip 表达。空剪在 clip 坐标系之外也成立。
    if mask.root().children().is_empty() {
        emit_empty_clip(builder);
        return;
    }
    let subtree = builder
        .record_subtree(|b| {
            // mask 内容是 user space 坐标：子树从恒等开始，执行端离屏时
            // 会套当时的组 CTM（与 DrawPath 同一坐标系约定）。
            let mut sub_state = EmitState {
                ts: Transform::default(),
            };
            render_nodes(b, &mut sub_state, mask.root());
            Ok::<(), ()>(())
        })
        .expect("subtree recording cannot fail");
    let rect = mask.rect();
    builder.push(DrawOp::MaskClip {
        subtree,
        kind: match mask.kind() {
            usvg::MaskType::Luminance => 0,
            usvg::MaskType::Alpha => 1,
        },
        region: Rect4 {
            x: rect.x(),
            y: rect.y(),
            width: rect.width(),
            height: rect.height(),
        },
    });
    if let Some(inner) = mask.mask() {
        emit_mask(builder, state, inner);
    }
}

/// 把 clipPath 的形状逐个 `ClipPath`（求交）。子形状保留各自 fill-rule；
/// clip 内 stroke 无效、paint 颜色无关（resvg clip.rs 同语义）。
///
/// ClipPath 是活动 canvas 状态（Save/Restore 会把它一起弹掉），所以这里
/// 不用 Save 保护矩阵：先 concat clip.transform 供形状定标，ClipPath 落盘
/// （执行端按设备空间记录）后再 concat 逆矩阵把 CTM 抵消回去。
fn emit_clip(builder: &mut DrawOpBuilder, state: &mut EmitState, clip: &usvg::ClipPath) {
    let t = clip.transform();
    concat(builder, state, t);
    // 多子形状：SVG 语义是子形状 union（clip-rule 只管单个形状内部的
    // 自交），ClipPath op 串联是求交，表达不了——转 MaskClip(kind=alpha)。
    // resvg clip.rs 正是子形状以 Clear 擦同一张 coverage 后 apply_mask，
    // 与 mask 的 DstIn 管线同源位对齐。子形状 fill 默认黑（alpha=1），
    // alpha 直取即 coverage。单子形状仍走 ClipPath op（shapes/structure
    // 已对齐的边缘量化不受影响）。
    if clip_shape_count(clip.root()) > 1 {
        emit_clip_as_mask(builder, clip);
    } else if emit_clip_shapes(builder, state, clip.root()) == 0 {
        // clipPath 无有效子形状（invisible/空）：剪掉一切（resvg 同语义）。
        emit_empty_clip(builder);
    }
    match t.invert() {
        Some(inv) => concat(builder, state, inv),
        None => return, // 退化矩阵：clip 区域已不可信，别再叠加内层
    }
    if let Some(inner) = clip.clip_path() {
        emit_clip(builder, state, inner);
    }
}

/// 多子形状 clipPath → MaskClip：内容子树（user space）+ alpha 折算 +
/// region（子树 bbox 外扩 AA 裙边；coverage 实际形状由子树渲染决定）。
fn emit_clip_as_mask(builder: &mut DrawOpBuilder, clip: &usvg::ClipPath) {
    let subtree = builder
        .record_subtree(|b| {
            let mut sub_state = EmitState {
                ts: Transform::default(),
            };
            render_nodes(b, &mut sub_state, clip.root());
            Ok::<(), ()>(())
        })
        .expect("subtree recording cannot fail");
    // 空 bbox = 全剪（mask_push_tiny 的退化 region → 全 0 coverage）。
    let r = clip.root().bounding_box();
    let (x, y, w, h) = (r.x() - 1.0, r.y() - 1.0, r.width() + 2.0, r.height() + 2.0);
    builder.push(DrawOp::MaskClip {
        subtree,
        kind: 1,
        region: Rect4 { x, y, width: w, height: h },
    });
}

/// clip 内容只消费形状：走执行端 current_path（BeginPath+Path+ClipPath）。
/// 返回发出的形状数：0 表示 clipPath 无有效子形状（invisible/空），
/// 语义是剪掉一切（resvg 同：空 clip 覆盖图为全透明），调用方须补空剪。
fn emit_clip_shapes(
    builder: &mut DrawOpBuilder,
    state: &mut EmitState,
    root: &usvg::Group,
) -> usize {
    let mut emitted = 0usize;
    for node in root.children() {
        match node {
            usvg::Node::Path(path) => {
                if !path.is_visible() {
                    continue;
                }
                let ops = path_ops(path.data());
                if ops.is_empty() {
                    continue;
                }
                emitted += 1;
                builder.push(DrawOp::BeginPath);
                for op in ops {
                    builder.push(DrawOp::Path(op));
                }
                // usvg 已把 clip-rule 折进子形状的 fill.rule（resvg 同源）。
                builder.push(DrawOp::ClipPath {
                    anti_alias: true,
                    fill_type: fill_type(path.fill().map(|f| f.rule())),
                });
            }
            usvg::Node::Group(group) => {
                let outer = state.ts;
                let gt = group.transform();
                concat(builder, state, gt);
                // 带组属性的 clip 子形状（如 clip-path）被 usvg 包成组
                // （converter.rs convert_group）：先发内层 clip 再递归形状
                // ——ClipPath 全是 Intersect，与 resvg 的「子内容离屏套
                // 内层 clip 再合回」等价。
                if let Some(inner) = group.clip_path() {
                    emit_clip(builder, state, inner);
                }
                emitted += emit_clip_shapes(builder, state, group);
                // 逆矩阵必须落盘：ClipPath 是设备空间记录，组变换泄漏进
                // CTM 会缩放/旋转后续子形状的 clip（with-transform 实例）。
                if let Some(inv) = gt.invert() {
                    concat(builder, state, inv);
                }
                state.ts = outer;
            }
            usvg::Node::Text(text) => {
                emitted += emit_clip_shapes(builder, state, text.flattened());
            }
            usvg::Node::Image(_) => {}
        }
    }
    emitted
}

/// 全剪 clip：0×0 矩形（任何区域与它求交为空）。tiny 覆盖图路径与 skia
/// 空路径 clip 都符合「剪掉一切」语义。
fn emit_empty_clip(builder: &mut DrawOpBuilder) {
    builder.push(DrawOp::BeginPath);
    builder.push(DrawOp::Path(PathOp::AddRect {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    }));
    builder.push(DrawOp::ClipPath {
        anti_alias: false,
        fill_type: Default::default(),
    });
}

// ---------------------------------------------------------------------------
// path 绘制（参考 resvg path.rs）
// ---------------------------------------------------------------------------

fn render_path(builder: &mut DrawOpBuilder, path: &usvg::Path) {
    if !path.is_visible() {
        return;
    }
    if std::env::var("OC_SVG_DEBUG").is_ok() {
        let t = path.abs_transform();
        eprintln!(
            "[svg] path visible={} fill?={} stroke_w={:?} abs=({},{},{},{},{},{}) bounds={:?}",
            path.is_visible(),
            path.fill().is_some(),
            path.stroke().map(|s| s.width().get()),
            t.sx, t.kx, t.ky, t.sy, t.tx, t.ty,
            path.data().bounds()
        );
    }
    let aa = path.rendering_mode().use_shape_antialiasing();
    let fill = fill_paint(path, aa);
    let stroke = stroke_paint(path, aa);
    // `<pattern>` 填充不走 PaintSpec（paint 是 intern 表，塞不下子树引用），
    // 用专用 FillPattern/StrokePattern op：tile 内容录成 picture recording。
    let pattern_fill = path
        .fill()
        .filter(|f| matches!(f.paint(), usvg::Paint::Pattern(_)));
    let pattern_stroke = path
        .stroke()
        .filter(|s| s.width().get() > 0.0)
        .filter(|s| matches!(s.paint(), usvg::Paint::Pattern(_)));
    if fill.is_none() && stroke.is_none() && pattern_fill.is_none() && pattern_stroke.is_none() {
        return;
    }
    // 水平/垂直线段无法填充（resvg 同规则跳过）。
    let can_fill = (fill.is_some() || pattern_fill.is_some())
        && path.data().bounds().width() > 0.0
        && path.data().bounds().height() > 0.0;
    let ops = path_ops(path.data());
    if ops.is_empty() {
        return;
    }
    let fill_type = fill_type(path.fill().map(|f| f.rule()));

    let emit = |builder: &mut DrawOpBuilder, spec: &PaintSpec| {
        let path_id = builder.intern_path(EncodedPath {
            fill_type,
            ops: ops.clone(),
        });
        let paint_id = builder.intern_paint(spec.clone());
        builder.push(DrawOp::DrawPath {
            path: path_id,
            paint: paint_id,
        });
    };
    // stroke pattern 的描边样式（width/cap/join/miter），与 stroke_paint
    // 同规则；opacity 取 stroke 不透明度（与 resvg stroke pattern 同源）。
    let pattern_stroke_spec = pattern_stroke.map(|s| StrokeSpec {
        width: s.width().get(),
        cap: convert_cap(s.linecap()),
        join: convert_join(s.linejoin()),
        miter_limit: s.miterlimit().get(),
    });

    match path.paint_order() {
        usvg::PaintOrder::FillAndStroke => {
            if can_fill {
                if let Some(f) = pattern_fill {
                    emit_pattern(builder, path, f.paint(), f.opacity(), fill_type, None);
                } else {
                    emit(builder, fill.as_ref().unwrap());
                }
            }
            if let Some(ps) = pattern_stroke {
                emit_pattern(builder, path, ps.paint(), ps.opacity(), fill_type, pattern_stroke_spec);
            } else if let Some(ref s) = stroke {
                emit(builder, s);
            }
        }
        usvg::PaintOrder::StrokeAndFill => {
            if let Some(ps) = pattern_stroke {
                emit_pattern(builder, path, ps.paint(), ps.opacity(), fill_type, pattern_stroke_spec);
            } else if let Some(ref s) = stroke {
                emit(builder, s);
            }
            if can_fill {
                if let Some(f) = pattern_fill {
                    emit_pattern(builder, path, f.paint(), f.opacity(), fill_type, None);
                } else {
                    emit(builder, fill.as_ref().unwrap());
                }
            }
        }
    }
}

/// `<pattern>` 填充/描边的 emit：tile 内容录成 picture recording（subtree），
/// 元数据（user space 归一后的 tile rect / patternTransform（含
/// transform-origin）/ opacity）随 op 下发。usvg 已把 units /
/// contentUnits / viewBox 全部折算进 rect 与 root（user space），引擎侧
/// 只需补运行时的 CTM 缩放（resvg render_pattern_pixmap 同式）。
/// `stroke` 为 `Some` 时发 StrokePattern（描边样式随 op 内联）。
fn emit_pattern(
    builder: &mut DrawOpBuilder,
    path: &usvg::Path,
    paint: &usvg::Paint,
    opacity: usvg::Opacity,
    fill_type: FillType,
    stroke: Option<StrokeSpec>,
) {
    let usvg::Paint::Pattern(patt) = paint else {
        return;
    };
    let subtree = builder
        .record_subtree(|b| {
            // tile 内容从 pattern 内容空间原点开始画（引擎离屏时再叠
            // scale(sx,sy)），子树 emit 不继承外层 CTM。
            let mut sub_state = EmitState {
                ts: Transform::default(),
            };
            render_nodes(b, &mut sub_state, patt.root());
            Ok::<(), ()>(())
        })
        .expect("subtree recording cannot fail");
    let t = patt.transform();
    let rect = patt.rect();
    let path_id = builder.intern_path(EncodedPath {
        fill_type,
        ops: path_ops(path.data()),
    });
    // asAffine 序（tiny 0.12 from_row 对接序）：[sx ky kx sy tx ty]。
    let pattern_transform = [t.sx, t.ky, t.kx, t.sy, t.tx, t.ty];
    let tile = Rect4 {
        x: rect.x(),
        y: rect.y(),
        width: rect.width(),
        height: rect.height(),
    };
    let opacity = opacity.get();
    if let Some(stroke) = stroke {
            builder.push(DrawOp::StrokePattern {
                path: path_id,
                subtree,
                tile,
                pattern_transform,
                opacity,
                stroke,
            });
        } else {
            builder.push(DrawOp::FillPattern {
                path: path_id,
                subtree,
                tile,
                pattern_transform,
                opacity,
            });
        }
    }

/// tiny-skia path 段 → PathOp。所有段都是绝对坐标（usvg 归一）。
fn path_ops(data: &tiny_skia_path::Path) -> Vec<PathOp> {
    let mut ops = Vec::new();
    for seg in data.segments() {
        match seg {
            PathSegment::MoveTo(p) => ops.push(PathOp::MoveTo { x: p.x, y: p.y }),
            PathSegment::LineTo(p) => ops.push(PathOp::LineTo { x: p.x, y: p.y }),
            PathSegment::QuadTo(c, p) => ops.push(PathOp::QuadTo {
                cx: c.x,
                cy: c.y,
                x: p.x,
                y: p.y,
            }),
            PathSegment::CubicTo(c1, c2, p) => ops.push(PathOp::CubicTo {
                c1x: c1.x,
                c1y: c1.y,
                c2x: c2.x,
                c2y: c2.y,
                x: p.x,
                y: p.y,
            }),
            PathSegment::Close => ops.push(PathOp::Close),
        }
    }
    ops
}

fn fill_type(rule: Option<usvg::FillRule>) -> FillType {
    match rule {
        Some(usvg::FillRule::EvenOdd) => FillType::EvenOdd,
        _ => FillType::Winding,
    }
}

/// 填充 paint；无 fill 或渐变缺 stop 时返回 None。
fn fill_paint(path: &usvg::Path, aa: bool) -> Option<PaintSpec> {
    let fill = path.fill()?;
    let fill_spec = paint_to_fill(fill.paint(), fill.opacity())?;
    Some(PaintSpec {
        fill: fill_spec,
        style: PaintStyle::Fill,
        stroke: None,
        anti_alias: aa,
        blend_mode: BlendMode::SrcOver,
        image_filter: None,
        color_filter: None,
        mask_filter: None,
        path_effect: None,
    })
}

fn stroke_paint(path: &usvg::Path, aa: bool) -> Option<PaintSpec> {
    let stroke = path.stroke()?;
    if stroke.width().get() <= 0.0 {
        return None;
    }
    let fill_spec = paint_to_fill(stroke.paint(), stroke.opacity())?;
    let mut spec = PaintSpec {
        fill: fill_spec,
        style: PaintStyle::Stroke,
        stroke: Some(StrokeSpec {
            width: stroke.width().get(),
            cap: convert_cap(stroke.linecap()),
            join: convert_join(stroke.linejoin()),
            miter_limit: stroke.miterlimit().get(),
        }),
        anti_alias: aa,
        blend_mode: BlendMode::SrcOver,
        image_filter: None,
        color_filter: None,
        mask_filter: None,
        path_effect: None,
    };
    if let Some(dash) = stroke.dasharray() {
        spec.path_effect = Some(PathEffectSpec::Dash {
            intervals: dash.to_vec(),
            phase: stroke.dashoffset(),
        });
    }
    Some(spec)
}

/// paint → FillSpec。Pattern 是 P2 缺口（PLAN §3），返回 None 跳过该 paint。
fn paint_to_fill(paint: &usvg::Paint, opacity: usvg::Opacity) -> Option<FillSpec> {
    match paint {
        usvg::Paint::Color(c) => Some(FillSpec::Solid([
            c.red as f32 / 255.0,
            c.green as f32 / 255.0,
            c.blue as f32 / 255.0,
            opacity.get(),
        ])),
        usvg::Paint::LinearGradient(lg) => {
            let (stops, colors) = gradient_stops(lg.stops(), opacity)?;
            Some(FillSpec::Shader(ShaderSpec::LinearGradient {
                from: [lg.x1(), lg.y1()],
                to: [lg.x2(), lg.y2()],
                stops,
                colors,
                tile_mode: convert_spread(lg.spread_method()),
                local_matrix: ts_matrix(&lg.transform()),
                // SVG/resvg 插值在 unpremul 空间（与浏览器 CSS 渐变相反）。
                interpolate_premul: false,
            }))
        }
        usvg::Paint::RadialGradient(rg) => {
            let (stops, colors) = gradient_stops(rg.stops(), opacity)?;
            // 焦点圆 (fx, fy, fr)：usvg 已把缺省 fx/fy 归一化为 cx/cy、
            // fr 归一化为 0，与 cx/cy/r 同值时按普通 radial 处理。
            let focal = [rg.fx(), rg.fy(), rg.fr().get()];
            let is_trivial = focal[0] == rg.cx()
                && focal[1] == rg.cy()
                && focal[2] == 0.0;
            Some(FillSpec::Shader(ShaderSpec::RadialGradient {
                center: [rg.cx(), rg.cy()],
                radius: rg.r().get(),
                stops,
                colors,
                tile_mode: convert_spread(rg.spread_method()),
                local_matrix: ts_matrix(&rg.transform()),
                interpolate_premul: false,
                focal: if is_trivial { None } else { Some(focal) },
            }))
        }
        usvg::Paint::Pattern(_) => None,
    }
}

fn gradient_stops(
    stops: &[usvg::Stop],
    opacity: usvg::Opacity,
) -> Option<(Vec<f32>, Vec<[f32; 4]>)> {
    if stops.is_empty() {
        return None;
    }
    let mut offsets = Vec::with_capacity(stops.len());
    let mut colors = Vec::with_capacity(stops.len());
    for stop in stops {
        offsets.push(stop.offset().get());
        let c = stop.color();
        colors.push([
            c.red as f32 / 255.0,
            c.green as f32 / 255.0,
            c.blue as f32 / 255.0,
            stop.opacity().get() * opacity.get(),
        ]);
    }
    Some((offsets, colors))
}

fn convert_spread(spread: usvg::SpreadMethod) -> TileMode {
    match spread {
        usvg::SpreadMethod::Pad => TileMode::Clamp,
        usvg::SpreadMethod::Reflect => TileMode::Mirror,
        usvg::SpreadMethod::Repeat => TileMode::Repeat,
    }
}

fn convert_cap(cap: usvg::LineCap) -> StrokeCap {
    match cap {
        usvg::LineCap::Butt => StrokeCap::Butt,
        usvg::LineCap::Round => StrokeCap::Round,
        usvg::LineCap::Square => StrokeCap::Square,
    }
}

fn convert_join(join: usvg::LineJoin) -> StrokeJoin {
    match join {
        usvg::LineJoin::Miter | usvg::LineJoin::MiterClip => StrokeJoin::Miter,
        usvg::LineJoin::Round => StrokeJoin::Round,
        usvg::LineJoin::Bevel => StrokeJoin::Bevel,
    }
}

fn convert_blend_mode(mode: usvg::BlendMode) -> BlendMode {
    match mode {
        usvg::BlendMode::Normal => BlendMode::SrcOver,
        usvg::BlendMode::Multiply => BlendMode::Multiply,
        usvg::BlendMode::Screen => BlendMode::Screen,
        usvg::BlendMode::Overlay => BlendMode::Overlay,
        usvg::BlendMode::Darken => BlendMode::Darken,
        usvg::BlendMode::Lighten => BlendMode::Lighten,
        usvg::BlendMode::ColorDodge => BlendMode::ColorDodge,
        usvg::BlendMode::ColorBurn => BlendMode::ColorBurn,
        usvg::BlendMode::HardLight => BlendMode::HardLight,
        usvg::BlendMode::SoftLight => BlendMode::SoftLight,
        usvg::BlendMode::Difference => BlendMode::Difference,
        usvg::BlendMode::Exclusion => BlendMode::Exclusion,
        usvg::BlendMode::Hue => BlendMode::Hue,
        usvg::BlendMode::Saturation => BlendMode::Saturation,
        usvg::BlendMode::Color => BlendMode::Color,
        usvg::BlendMode::Luminosity => BlendMode::Luminosity,
        usvg::BlendMode::PlusLighter => BlendMode::Plus,
    }
}

/// tiny-skia Transform → ShaderSpec local_matrix（行主序直通，
/// executor/paint.rs 的 skia_matrix_from_row_major 按参数序展开）。
/// 单位阵返回 None 省一次换算。
fn ts_matrix(t: &Transform) -> Option<[f32; 9]> {
    let ident = Transform::identity();
    if t.sx == ident.sx
        && t.kx == ident.kx
        && t.ky == ident.ky
        && t.sy == ident.sy
        && t.tx == ident.tx
        && t.ty == ident.ty
    {
        return None;
    }
    Some([t.sx, t.kx, t.tx, t.ky, t.sy, t.ty, 0.0, 0.0, 1.0])
}

// ---------------------------------------------------------------------------
// image（参考 resvg image.rs）
// ---------------------------------------------------------------------------

fn render_image(builder: &mut DrawOpBuilder, state: &mut EmitState, image: &usvg::Image) {
    if !image.is_visible() {
        return;
    }
    match image.kind() {
        ImageKind::SVG(tree) => {
            // 嵌套 svg：root transform 已含摆放，直接递归。
            render_group(builder, state, tree.root());
        }
        ImageKind::JPEG(data) | ImageKind::PNG(data) | ImageKind::GIF(data)
        | ImageKind::WEBP(data) => {
            // Core 是纯协议层：编码字节原样进 bytes 表，解码与重采样归
            // engine（与 Static 通道同一分层）。resvg 端同构于
            // render_raster 的 fill_rect((0,0,natural_w,natural_h))，自然
            // 尺寸矩形铺在 image_ts 之后的局部空间，摆位全在 CTM 里。
            let data = builder.intern_bytes(data);
            let size = image.size();
            builder.push(DrawOp::ImageRect {
                image: ImageRef::Encoded { data },
                src: None,
                dst: Rect4 {
                    x: 0.0,
                    y: 0.0,
                    width: size.width() as f32,
                    height: size.height() as f32,
                },
                paint: None,
            });
        }
    }
}
