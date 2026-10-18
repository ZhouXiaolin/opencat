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
    builder.push(DrawOp::ClipPath { anti_alias: true });
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

fn render_group(
    builder: &mut DrawOpBuilder,
    state: &mut EmitState,
    group: &usvg::Group,
) {
    let outer_ts = state.ts;
    if std::env::var("OC_SVG_DEBUG").is_ok() {
        eprintln!("[svg] group ts={:?} isolate={} children={}", (group.transform().sx, group.transform().kx, group.transform().ky, group.transform().sy, group.transform().tx, group.transform().ty), group.should_isolate(), group.children().len());
    }
    // Save 先于组矩阵落盘：执行端 Restore 回滚到进组前的 CTM。若 Save 晚于
    // Concat，Restore 只能恢复到组矩阵生效后，矩阵会泄漏给后续兄弟
    // （use x=60 的兄弟叠加成 y=120→180 出界的实例教训）。隔离组的
    // SaveLayer 也一样隐含 save，必须吃在 Concat 前面。
    builder.push(DrawOp::Save);
    concat(builder, state, group.transform());

    if !group.should_isolate() {
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
        // P1 折算：矢量形状 mask 近似为 clip；luminance/图片 mask 是 P3。
        emit_clip_shapes(builder, state, mask.root());
    }
    render_nodes(builder, state, group);
    builder.push(DrawOp::Restore);
    state.ts = outer_ts;
}

// ---------------------------------------------------------------------------
// clip（形状遮罩 → ClipPath）
// ---------------------------------------------------------------------------

/// 把 clipPath 的形状逐个 `ClipPath`（求交）。子形状保留各自 fill-rule；
/// clip 内 stroke 无效、paint 颜色无关（resvg clip.rs 同语义）。
///
/// ClipPath 是活动 canvas 状态（Save/Restore 会把它一起弹掉），所以这里
/// 不用 Save 保护矩阵：先 concat clip.transform 供形状定标，ClipPath 落盘
/// （执行端按设备空间记录）后再 concat 逆矩阵把 CTM 抵消回去。
fn emit_clip(builder: &mut DrawOpBuilder, state: &mut EmitState, clip: &usvg::ClipPath) {
    let t = clip.transform();
    concat(builder, state, t);
    emit_clip_shapes(builder, state, clip.root());
    match t.invert() {
        Some(inv) => concat(builder, state, inv),
        None => return, // 退化矩阵：clip 区域已不可信，别再叠加内层
    }
    if let Some(inner) = clip.clip_path() {
        emit_clip(builder, state, inner);
    }
}

/// clip 内容只消费形状：走执行端 current_path（BeginPath+Path+ClipPath）。
fn emit_clip_shapes(builder: &mut DrawOpBuilder, state: &mut EmitState, root: &usvg::Group) {
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
                builder.push(DrawOp::BeginPath);
                for op in ops {
                    builder.push(DrawOp::Path(op));
                }
                builder.push(DrawOp::ClipPath { anti_alias: true });
            }
            usvg::Node::Group(group) => {
                let outer = state.ts;
                concat(builder, state, group.transform());
                emit_clip_shapes(builder, state, group);
                state.ts = outer;
            }
            usvg::Node::Text(text) => {
                emit_clip_shapes(builder, state, text.flattened());
            }
            usvg::Node::Image(_) => {}
        }
    }
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
    if fill.is_none() && stroke.is_none() {
        return;
    }
    // 水平/垂直线段无法填充（resvg 同规则跳过）。
    let can_fill = fill.is_some()
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

    match path.paint_order() {
        usvg::PaintOrder::FillAndStroke => {
            if can_fill {
                emit(builder, fill.as_ref().unwrap());
            }
            if let Some(ref s) = stroke {
                emit(builder, s);
            }
        }
        usvg::PaintOrder::StrokeAndFill => {
            if let Some(ref s) = stroke {
                emit(builder, s);
            }
            if can_fill {
                emit(builder, fill.as_ref().unwrap());
            }
        }
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
            }))
        }
        usvg::Paint::RadialGradient(rg) => {
            let (stops, colors) = gradient_stops(rg.stops(), opacity)?;
            // 焦点（fx/fy/fr）本引擎 ShaderSpec 暂缺，退化为主圆（P2）。
            Some(FillSpec::Shader(ShaderSpec::RadialGradient {
                center: [rg.cx(), rg.cy()],
                radius: rg.r().get(),
                stops,
                colors,
                tile_mode: convert_spread(rg.spread_method()),
                local_matrix: ts_matrix(&rg.transform()),
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
