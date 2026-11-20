//! core 的 pattern-tile 可见区整理 pass（唯一生产 hook，engine/web 消费同一裁减结果）。
//!
//! 范围（窄版，宁缺毋滥）：仅处理主 frame 直接引用、且引用计数恰为 1
//! （唯一引用者即本 op）的 [`DrawOp::FillPattern`]{ `rasterize_at: Some` }，
//! 剔除其 subtree 顶层（非滤镜层内）[`DrawOp::FillOvalBatch`] 中与可见窗口
//! （外扩 halo 后）完全不相交的椭圆。其余 op、状态命令、坐标、颜色、alpha、
//! 保序一概不动；frame.ops / paints / paths / children / Arc 共享数据只读。
//!
//! 窗口推导与 engine 的 pattern shader 矩阵同构：
//! forward = ctm ∘ pt ∘ T(tile) ∘ S(1/s)，整体 invert 得 device→texel，把
//! 合成视口 (0,0,w,h) 映成 texel AABB，外扩 `2 + 采样脚印` 后必须完整落在
//! [0,tile_w]×[0,tile_h] 内才剔（跨 repeat / outside-tile 自然失效）。合成
//! 视口忽略普通 clip（保守高估可见域）；宿主在 DrawOp 之外叠加的 base 仿射
//! （如 render_scale）与"device clip = base∘视口"同时成立时在数学上相互抵消，
//! 窗口不变。
//!
//! 主 frame 级保守回退（通常按 pattern 粒度；main ReplayRange 整帧禁裁）：
//! 1. pattern 处于活动的 source image-filter SaveLayer 内 → skip。滤镜层把
//!    设备域外扩到滤镜输入需求（blur/drop-shadow 等空间核），层内绘制会采样
//!    窗口外 texel，回读可把差异漏进视口（不能用"全 surface 不变"替代）。
//!    仅放行已证逐像素、bounds 恒等的 ColorFilter（feColorMatrix /
//!    BlendColor / gamma——output(p)=f(input(p))，无空间扩散）及其纯颜色
//!    `Compose` 链（两侧递归均为 ColorFilter；CSS hue/saturate 折叠形）；
//!    blur（含 sigma 0）/DropShadow/Chain 一律视为空间核。
//! 2. 更晚存在 `SaveBackdropLayer` 且 paint 带 image_filter → skip。engine
//!    两遍法（`build_backdrop_pictures`）用 ±1e6 cull（非 clip）把该 op 之前
//!    的全部前缀录成 picture 并经 `draw_picture`+filter 重放——pattern 在前缀
//!    内被以超大采样域二次执行，core 侧 baked 裁减会改 backdrop 滤镜输入。
//!    无 filter 的 SaveBackdropLayer 不录用（`use_snapshot` 要求
//!    `backdrop.is_some()`）、`SaveLayer{backdrop:true}` 走 Skia 原生 backdrop
//!    （读主面已绘内容 ⊆ 视口），均不阻塞。
//! 3. pattern 索引落在任何间接执行区（`ReplayRange`，含 RuntimeEffect
//!    `Picture` children 指向的 ops range）内 → skip（CTM 静态不可知）。
//! 4. CTM / patternTransform 非有限、非仿射（wire Concat 第三行非 [0,0,1]）、
//!    不可逆、空窗口、密度/瓦片尺寸非法 → skip。
//!
//! subtree 级守卫（整 subtree 回退）：backdrop 层、
//! RuntimeEffect、DrawSubtreePicture/ReplaySubtreePicture、ReplayRange 即禁
//! 裁；MaskClip 递归核查（深度 8）。
//!
//! batch 级安全依据（封闭 op 解释器事实，两消费端 handler 均已核实；新增可写
//! current fill paint 的 style/filter/path-effect 的 op 时必须同步本 pass）：
//! op 流状态 op 全集对 fill 漆仅有 color（SetFillStyle）、shader（渐变，两端
//! batch handler 均显式清空）、AA、blend 四类写入（engine replay.rs 857/911/
//! 954/1000/1006、web interpret.ts 同构）；style 恒 Fill、mask/image/path-
//! effect 恒无——FillOvalBatch 无论继承调用点漆（OMP emitter 不发 SetFillStyle）
//! 还是走显式色，其执行漆都是纯几何安全的。空间核只可能来自带 image_filter
//! 的 SaveLayer（layer_flags 与 tile canvas save 栈同进退）。
//!
//! Save/Restore/RestoreToCount 模拟与 canvas_lower 同式（keep = count-1 下界
//! 0）。count 为 i32 原样透传（bindings 不 clamp）：仅对有效 count（≥1）正常
//! 模拟；非法 count（≤0）按 Skia 弹光语义保守处理——engine 侧负数 as usize
//! 大值在 Skia 转换处可能 panic 属基线原有行为，本 pass 不扩大也不复刻。坏
//! subtree/paint id 对应候选与短 rects/colors 保守跳过，不新增 panic；全保留的
//! batch 不分配新 Vec/Arc（先计数后拷贝，两遍）。
//!
//! 消费端设备面事实（审计核实）：native 预览以 reset_matrix + 可选
//! render_scale 作 base（device clip = base∘视口，窗口数学抵消，present/
//! mod.rs:119-126）；export 路径的偶数对齐可引入 ±1px 偏差，落在 fp margin
//! （2 + 采样脚印）内；web 消费面尺寸 = 合成尺寸。

use crate::ir::draw_frame::DrawOpFrame;
use crate::ir::draw_op::{DrawOp, Rect4};
use crate::ir::draw_types::RuntimeEffectChildRef;

/// 每帧统计（仅诊断打印用；不进任何二进制 schema）。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PatternCullStats {
    /// 主 frame 直接引用的 FillPattern{rasterize_at:Some} 个数。
    pub patterns_seen: u32,
    /// 完成裁减的 pattern 数（subtree 被替换）。
    pub patterns_culled: u32,
    /// 被剔改写的 FillOvalBatch 条数。
    pub batches_culled: u32,
    /// 尝试剔除的 batch 内椭圆总数（含全保留）。
    pub ovals_total: u64,
    /// 剔除的椭圆数。
    pub ovals_removed: u64,
    /// 引用计数非 1（多消费者）跳过的 pattern 数。
    pub skip_multi_ref: u32,
    /// 落在间接执行区（ReplayRange / RuntimeEffect Picture range）内的跳过数。
    pub skip_replay_range: u32,
    /// 更晚存在带 filter 的 SaveBackdropLayer（backdrop 前缀重执行）跳过数。
    pub skip_backdrop_prefix: u32,
    /// 处于活动 source image-filter SaveLayer 内跳过数（含空间核守卫）。
    pub skip_source_filter: u32,
    /// 窗口/矩阵/密度几何失效跳过数。
    pub skip_window: u32,
    /// subtree 守卫（backdrop/runtime-effect/subtree-picture/replay-range）禁裁数。
    pub skip_guard: u32,
    /// 处于 subtree 内空间核 SaveLayer 而整条保号的 batch 数。
    pub skip_filter_layer: u32,
    /// 矩阵/短数据不可判而原样保号的 batch 数。
    pub skip_bad_batch: u32,
}

/// 单入口：在 `builder.finish()` 之后、`build_media_plan` 之前对当帧整理。
/// `viewport` = 合成视口 (width, height)（composition 单位）。返回统计；
/// `OC_PATTERN_CULL=0` 时整体直通（AB 统一关闭 knob，默认启用）。
pub fn cull_pattern_tiles(frame: &mut DrawOpFrame, viewport: (i32, i32)) -> PatternCullStats {
    if std::env::var_os("OC_PATTERN_CULL").is_some_and(|v| v == *"0") {
        return PatternCullStats::default();
    }
    cull_frame_inner(frame, viewport)
}

/// 环境变量之外的纯实现（单测直入，避免 env 全局态干扰并行测试）。
fn cull_frame_inner(frame: &mut DrawOpFrame, viewport: (i32, i32)) -> PatternCullStats {
    let mut stats = PatternCullStats::default();
    let (vw, vh) = (viewport.0 as f32, viewport.1 as f32);
    if !vw.is_finite() || !vh.is_finite() || vw <= 0.0 || vh <= 0.0 {
        return stats;
    }
    if !frame.ops.iter().any(|op| {
        matches!(
            op,
            DrawOp::FillPattern {
                rasterize_at: Some(_),
                ..
            }
        )
    }) {
        return stats;
    }

    // 预扫一：subtree 引用计数。扫描面 = frame.ops 全量 + 全部 subtree 流
    // （递归一层即可：subtree 内 op 的引用与位置无关）+ children 全量
    // （RuntimeEffectChildRef::SubtreePicture；全量多计只会更保守）。
    let mut refs = vec![0u32; frame.subtrees.len()];
    for op in &frame.ops {
        bump_ref(op, &mut refs);
    }
    for ops in &frame.subtrees {
        for op in ops {
            bump_ref(op, &mut refs);
        }
    }
    for child in &frame.children {
        if let RuntimeEffectChildRef::SubtreePicture(id) = child
            && let Some(c) = refs.get_mut(id.0 as usize)
        {
            *c += 1;
        }
    }
    // paints 的 image_filter 链内 feImage（FilterPrimKind::Image）也执行
    // subtree（engine render_filter_images）——全量扫描，坏 id 不计数。
    for spec in &frame.paints {
        if let Some(f) = spec.image_filter.as_ref() {
            bump_filter_image_refs(f, &mut refs);
        }
    }

    // 预扫二：间接执行区（ReplayRange + RuntimeEffect Picture children），
    // 覆盖 frame.ops 与全部 subtree 流。range 越界时 engine 侧会报错，这里不标记。
    let mut covered = vec![false; frame.ops.len()];
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    collect_ops_ranges(&frame.ops, frame, &mut ranges);
    for ops in &frame.subtrees {
        collect_ops_ranges(ops, frame, &mut ranges);
    }
    for (start, len) in ranges {
        let s = start as usize;
        let e = s.saturating_add(len as usize);
        if e <= covered.len() {
            covered[s..e].fill(true);
        }
    }

    // 预扫三：backdrop 前缀阻塞（suffix 扫描）。仅带 image_filter 的
    // SaveBackdropLayer 触发前缀 picture 重执行（见模块头 2）。
    let mut blocked = vec![false; frame.ops.len()];
    let mut seen = false;
    for i in (0..frame.ops.len()).rev() {
        blocked[i] = seen;
        if let DrawOp::SaveBackdropLayer { paint, .. } = &frame.ops[i]
            && backdrop_filter_present(paint, frame)
        {
            seen = true;
        }
    }

    // 任一 main 级 ReplayRange → 整帧禁裁：range 在调用点以真 canvas 状态
    // 重执行，可改变状态面与 backdrop 前缀顺序（早前 SaveBackdropLayer 被
    // 挪后等），窄版不做范围模拟。RuntimeEffect Picture range 保持按被引用
    // 索引禁裁（下方 covered）。
    let frame_skip_range = frame
        .ops
        .iter()
        .any(|op| matches!(op, DrawOp::ReplayRange { .. }));

    // 主走查：模拟 canvas 状态机（矩阵栈 + source-filter 层栈），逐
    // FillPattern 处理。
    let mut cur = MainState {
        ctm: M33::identity(),
        spatial: false,
    };
    let mut stack: Vec<MainState> = Vec::new();
    for (i, op) in frame.ops.iter().enumerate() {
        match op {
            DrawOp::FillPattern {
                subtree,
                tile,
                pattern_transform,
                rasterize_at: Some(rasterize_at),
                ..
            } => {
                stats.patterns_seen += 1;
                if frame_skip_range || covered[i] {
                    stats.skip_replay_range += 1;
                } else if refs.get(subtree.0 as usize) != Some(&1) {
                    stats.skip_multi_ref += 1;
                } else if blocked[i] {
                    stats.skip_backdrop_prefix += 1;
                } else if cur.spatial {
                    stats.skip_source_filter += 1;
                } else if let Some(win) =
                    pattern_window(&cur.ctm, pattern_transform, tile, rasterize_at, vw, vh)
                {
                    if let Some(ops) = frame.subtrees.get(subtree.0 as usize) {
                        if subtree_guard_ok(frame, ops, 0).is_err() {
                            stats.skip_guard += 1;
                        } else {
                            let base = M33::scale(
                                rasterize_at[0] / tile.width,
                                rasterize_at[1] / tile.height,
                            );
                            if let Some(new_ops) = cull_walk(frame, ops, base, win, &mut stats) {
                                frame.subtrees[subtree.0 as usize] = new_ops;
                                stats.patterns_culled += 1;
                            }
                        }
                    } else {
                        stats.skip_multi_ref += 1;
                    }
                } else {
                    stats.skip_window += 1;
                }
            }
            other => advance_state(&mut cur, &mut stack, other, frame),
        }
    }
    stats
}

/// 主走查的 canvas 状态面：矩阵 + source-filter 层标记。
#[derive(Clone, Copy)]
struct MainState {
    ctm: M33,
    /// 处于活动的 source image-filter（空间核）SaveLayer 内。
    spatial: bool,
}

/// 推进一个 op 的状态面（与 engine replay 的 canvas 语义同构）。
fn advance_state(
    cur: &mut MainState,
    stack: &mut Vec<MainState>,
    op: &DrawOp,
    frame: &DrawOpFrame,
) {
    match op {
        DrawOp::Save => stack.push(*cur),
        DrawOp::SaveLayer {
            paint, backdrop, ..
        } => {
            stack.push(*cur);
            // backdrop:true 时 engine 把 filter 从内容漆上摘下挂 backdrop 槽
            // （replay.rs 1033-1042），层内内容不过滤 → 不算空间核。paint
            // None = save_layer_alpha（无 filter）；坏 paint id fail-closed。
            let spatial = !*backdrop
                && match paint {
                    None => false,
                    Some(pid) => match frame.paints.get(pid.0 as usize) {
                        None => true,
                        Some(spec) => spec
                            .image_filter
                            .as_ref()
                            .is_some_and(|f| !image_filter_is_pixel_local(f)),
                    },
                };
            cur.spatial = cur.spatial || spatial;
        }
        // SaveBackdropLayer 的层内容不带 filter（filter 在 backdrop 槽）。
        DrawOp::SaveBackdropLayer { .. } => stack.push(*cur),
        DrawOp::Restore => {
            if let Some(s) = stack.pop() {
                *cur = s;
            }
        }
        DrawOp::RestoreToCount { count } => {
            let target = ((*count).max(1) - 1) as usize;
            if stack.len() > target {
                *cur = stack[target];
                stack.truncate(target);
            }
        }
        DrawOp::Translate { x, y } => cur.ctm = cur.ctm.concat(&M33::translate(*x, *y)),
        DrawOp::Scale { x, y } => cur.ctm = cur.ctm.concat(&M33::scale(*x, *y)),
        DrawOp::Rotate { degrees, cx, cy } => {
            cur.ctm = cur.ctm.concat(&M33::rotate_about(*degrees, *cx, *cy));
        }
        DrawOp::Skew { sx, sy } => cur.ctm = cur.ctm.concat(&M33::skew(*sx, *sy)),
        DrawOp::Concat { matrix } => cur.ctm = cur.ctm.concat(&M33::from_wire(matrix)),
        _ => {}
    }
}

/// op 对 subtree 的直接引用（引用计数扫描面）。
fn bump_ref(op: &DrawOp, refs: &mut [u32]) {
    let id = match op {
        DrawOp::FillPattern { subtree, .. }
        | DrawOp::StrokePattern { subtree, .. }
        | DrawOp::MaskClip { subtree, .. }
        | DrawOp::ReplaySubtreePicture { subtree, .. } => subtree.0,
        _ => return,
    };
    if let Some(c) = refs.get_mut(id as usize) {
        *c += 1;
    }
}

/// 扫 paint 的 image_filter 链内 feImage（`FilterPrimKind::Image`）对 subtree
/// 的引用（engine `render_filter_images` 会执行这些子树）。仅 `Chain` 载有
/// primitives；`Compose` 递归核查。新增引用载体时须同步此扫描。坏 id 不计数
/// （越界子树无 pattern 可指）。
fn bump_filter_image_refs(filter: &crate::canvas::paint::ImageFilterSpec, refs: &mut [u32]) {
    use crate::canvas::paint::{FilterPrimKind, ImageFilterSpec};
    match filter {
        ImageFilterSpec::Chain(chains) => {
            for chain in chains {
                for p in &chain.primitives {
                    if let FilterPrimKind::Image { subtree } = &p.kind
                        && let Some(c) = refs.get_mut(*subtree as usize)
                    {
                        *c += 1;
                    }
                }
            }
        }
        ImageFilterSpec::Compose(a, b) => {
            bump_filter_image_refs(a, refs);
            bump_filter_image_refs(b, refs);
        }
        _ => {}
    }
}

/// op 携带的 ops 间接执行区：ReplayRange 本体 + RuntimeEffect 的 Picture
/// children 所指的 frame.ops range（child 画布尺寸/采样域不可静态核查，
/// 覆盖范围内的候选一律 skip）。
fn collect_ops_ranges(ops: &[DrawOp], frame: &DrawOpFrame, out: &mut Vec<(u32, u32)>) {
    for op in ops {
        match op {
            DrawOp::ReplayRange { range } => out.push((range.start_op, range.op_len)),
            DrawOp::RuntimeEffect { children, .. } => {
                let s = children.start as usize;
                let e = s
                    .saturating_add(children.len as usize)
                    .min(frame.children.len());
                for child in frame.children.get(s..e).into_iter().flatten() {
                    if let RuntimeEffectChildRef::Picture(range) = child {
                        out.push((range.start_op, range.op_len));
                    }
                }
            }
            _ => {}
        }
    }
}

/// SaveBackdropLayer 是否带 backdrop 滤镜（决定前缀 picture 是否真被绘制）。
/// paint 缺失 → 无 backdrop；坏 paint id → fail-closed（视为有）。
fn backdrop_filter_present(
    paint: &Option<crate::ir::draw_types::PaintId>,
    frame: &DrawOpFrame,
) -> bool {
    match paint {
        None => false,
        Some(pid) => frame
            .paints
            .get(pid.0 as usize)
            .is_none_or(|spec| spec.image_filter.is_some()),
    }
}

/// 已证逐像素、无空间扩散的 image filter：ColorFilter（Matrix/BlendColor/
/// gamma 四变体均 output(p)=f(input(p))，Skia 层 bounds 恒等不外扩），以及
/// 两侧均为纯 ColorFilter 的 `Compose(a, b)`——Compose 的层 bounds = 外层
/// mapBounds(内层 mapBounds(bounds))，两侧均恒等时整体恒等、输出仍逐像素
/// f(g(p))。主会话已核 shadow_filter.rs:148-199：CSS 多项颜色滤镜（默认
/// hue-rotate+saturate）即折叠为 Compose(next, inner) 链，放行它才能解除
/// 语料 17 帧的误杀。Blur（含 sigma 0）/DropShadow/Chain 一律视为空间核；
/// Chain 内即使全为 ColorFilter 也不放行（SVG 多 primitive 语义未逐项核实）。
fn image_filter_is_pixel_local(filter: &crate::canvas::paint::ImageFilterSpec) -> bool {
    use crate::canvas::paint::ImageFilterSpec as I;
    match filter {
        I::ColorFilter(_) => true,
        I::Compose(a, b) => image_filter_is_pixel_local(a) && image_filter_is_pixel_local(b),
        _ => false,
    }
}

/// subtree 级守卫（与 engine 诊断版同款）：危险依赖整 subtree 禁裁。
fn subtree_guard_ok(frame: &DrawOpFrame, ops: &[DrawOp], depth: u8) -> Result<(), &'static str> {
    if depth > 8 {
        return Err("too-deep");
    }
    for op in ops {
        match op {
            DrawOp::SaveLayer { backdrop: true, .. } | DrawOp::SaveBackdropLayer { .. } => {
                return Err("backdrop");
            }
            DrawOp::RuntimeEffect { .. } => return Err("runtime-effect"),
            DrawOp::DrawSubtreePicture { .. } | DrawOp::ReplaySubtreePicture { .. } => {
                return Err("subtree-picture");
            }
            DrawOp::ReplayRange { .. } => return Err("replay-range"),
            DrawOp::MaskClip { subtree, .. } => {
                let Some(mask_ops) = frame.subtrees.get(subtree.0 as usize) else {
                    return Err("bad-subtree");
                };
                subtree_guard_ok(frame, mask_ops, depth + 1)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// 纯几何：device 视口 → texel 窗口。forward 与 engine fill_pattern_skia 的
/// shader_lm 同构后整体 invert（不手工拼逆分解），viewport 映 AABB，外扩
/// `2 + 采样脚印`，完整落在 tile 内才通过。
fn pattern_window(
    ctm: &M33,
    pt: &[f32; 6],
    tile: &Rect4,
    rasterize_at: &[f32; 2],
    vw: f32,
    vh: f32,
) -> Option<Rect> {
    if !ctm.is_finite() || !ctm.is_affine() {
        return None;
    }
    let sx = rasterize_at[0] / tile.width;
    let sy = rasterize_at[1] / tile.height;
    if !sx.is_finite() || !sy.is_finite() || sx <= 0.0 || sy <= 0.0 {
        return None;
    }
    let tile_w = tile.width * sx;
    let tile_h = tile.height * sy;
    if !tile_w.is_finite() || !tile_h.is_finite() || tile_w <= 0.0 || tile_h <= 0.0 {
        return None;
    }
    let pt_m = M33::from_affine(pt);
    if !pt_m.is_finite() || !pt_m.is_affine() {
        return None;
    }
    // texel → scale(1/s) → translate(tile) → pt → user → ctm → device。
    let shader_lm = pt_m
        .concat(&M33::translate(tile.x, tile.y))
        .concat(&M33::scale(1.0 / sx, 1.0 / sy));
    let d2t = ctm.concat(&shader_lm).invert()?;
    if !d2t.is_finite() {
        return None;
    }
    let (wl, wt, wr, wb) = d2t.map_rect(0.0, 0.0, vw, vh);
    if !wl.is_finite() || !wt.is_finite() || !wr.is_finite() || !wb.is_finite() {
        return None;
    }
    if wl >= wr || wt >= wb {
        return None;
    }
    // 采样脚印：设备像素映到 texel 的最大位移上界（线性部分行绝对和取大，
    // 对旋转/skew 保守）。与诊断版同式 margin = 2 + fp。
    let fp = (d2t.m[0].abs() + d2t.m[1].abs()).max(d2t.m[3].abs() + d2t.m[4].abs());
    let margin = 2.0 + fp;
    let win = Rect {
        x0: (wl - margin).floor(),
        y0: (wt - margin).floor(),
        x1: (wr + margin).ceil(),
        y1: (wb + margin).ceil(),
    };
    if win.x1 <= win.x0 || win.y1 <= win.y0 {
        return None;
    }
    if win.x0 < 0.0 || win.y0 < 0.0 || win.x1 > tile_w || win.y1 > tile_h {
        return None;
    }
    Some(win)
}

/// subtree 走查：模拟 tile canvas（base = scale(sx,sy)）上的矩阵/层栈，对
/// 可剔 batch 做整椭圆剔除。返回 Some(new_ops) = 有剔除（新 Vec）；None =
/// 无任何改动（含全保留）。
fn cull_walk(
    frame: &DrawOpFrame,
    ops: &[DrawOp],
    base: M33,
    win: Rect,
    stats: &mut PatternCullStats,
) -> Option<Vec<DrawOp>> {
    let mut cur = MainState {
        ctm: base,
        spatial: false,
    };
    let mut stack = Vec::new();
    let mut out = None;
    for (idx, op) in ops.iter().enumerate() {
        if let DrawOp::FillOvalBatch { count, .. } = op {
            if cur.spatial {
                stats.skip_filter_layer += 1;
            } else {
                match cull_batch(&cur.ctm, op, win) {
                    BatchCull::Replaced(filtered) => {
                        stats.ovals_total += *count as u64;
                        if let DrawOp::FillOvalBatch { count: kept, .. } = &filtered {
                            stats.batches_culled += 1;
                            stats.ovals_removed += (*count - *kept) as u64;
                        }
                        out.get_or_insert_with(|| ops[..idx].to_vec())
                            .push(filtered);
                        continue;
                    }
                    BatchCull::Untouched => stats.ovals_total += *count as u64,
                    BatchCull::BadData => stats.skip_bad_batch += 1,
                }
            }
        } else {
            // backdrop 与间接执行已被 subtree_guard_ok 排除；主帧和 tile
            // 共用同一套矩阵/滤镜状态推进，避免 Save/Restore 语义漂移。
            advance_state(&mut cur, &mut stack, op, frame);
        }
        push_clone(&mut out, op);
    }
    out
}

fn push_clone(out: &mut Option<Vec<DrawOp>>, op: &DrawOp) {
    if let Some(v) = out.as_mut() {
        v.push(op.clone());
    }
}

enum BatchCull {
    /// 原样保留（全保留；无新分配）。
    Untouched,
    /// 筛选后的新 batch。
    Replaced(DrawOp),
    /// 矩阵/短数据不可判：fail-closed 原样保留。
    BadData,
}

/// 窗口（tile 根像素坐标）先外扩 halo 2 根像素（AA 外沿 ≤1 设备像素余量，
/// 在根像素域扩张、不假定 local 缩放恒 1），再经当前矩阵逆映射成 local AABB，
/// 对 batch 逐椭圆做严格不相交剔除。保序、位模式不变；坏索引/短数据不新增
/// panic（原样保留，replay 端行为不变）。
fn cull_batch(ctm: &M33, op: &DrawOp, win: Rect) -> BatchCull {
    let DrawOp::FillOvalBatch {
        rects,
        colors,
        count,
    } = op
    else {
        return BatchCull::Untouched;
    };
    let n = *count as usize;
    if rects.len() < n.saturating_mul(4) || colors.len() < n.saturating_mul(4) {
        return BatchCull::BadData;
    }
    if !ctm.is_finite() || !ctm.is_affine() {
        return BatchCull::BadData;
    }
    let Some(inv) = ctm.invert() else {
        return BatchCull::BadData;
    };
    let halo = win.outset(2.0);
    let (pl, pt, pr, pb) = inv.map_rect(halo.x0, halo.y0, halo.x1, halo.y1);
    if !pl.is_finite()
        || !pt.is_finite()
        || !pr.is_finite()
        || !pb.is_finite()
        || pl >= pr
        || pt >= pb
    {
        return BatchCull::BadData;
    }
    // 两遍：先计数（全保留则不分配），再拷贝保留项。
    let keeps = |i: usize| -> bool {
        let o = i * 4;
        let (x, y, w, h) = (rects[o], rects[o + 1], rects[o + 2], rects[o + 3]);
        !x.is_finite()
            || !y.is_finite()
            || !w.is_finite()
            || !h.is_finite()
            || w <= 0.0
            || h <= 0.0
            || !(x + w).is_finite()
            || !(y + h).is_finite()
            // 严格相交测试：与 halo 域边界相触也算不相交（剔除）；椭圆对
            // halo 的 2px 保护由外扩本身承担。
            || (x < pr && x + w > pl && y < pb && y + h > pt)
    };
    let mut kept = 0usize;
    for i in 0..n {
        if keeps(i) {
            kept += 1;
        }
    }
    if kept == n {
        return BatchCull::Untouched;
    }
    let mut out_rects = Vec::with_capacity(kept * 4);
    let mut out_colors = Vec::with_capacity(kept * 4);
    for i in 0..n {
        if keeps(i) {
            let o = i * 4;
            out_rects.extend_from_slice(&rects[o..o + 4]);
            out_colors.extend_from_slice(&colors[o..o + 4]);
        }
    }
    BatchCull::Replaced(DrawOp::FillOvalBatch {
        rects: out_rects.into(),
        colors: out_colors.into(),
        count: kept as u32,
    })
}

// ---------------------------------------------------------------------------
// 最小 3×3 矩阵（列向量约定）——pass 内部专用，不进公共 API。
// ---------------------------------------------------------------------------

/// 行主序存储、列向量作用：
/// `x' = m0·x + m1·y + m2`，`y' = m3·x + m4·y + m5`，`w' = m6·x + m7·y + m8`。
#[derive(Clone, Copy, Debug, PartialEq)]
struct M33 {
    m: [f32; 9],
}

/// 轴对齐矩形（f32，开区间语义与 skia 一致：left<right 才非空）。
#[derive(Clone, Copy, Debug, PartialEq)]
struct Rect {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
}

impl Rect {
    fn outset(self, d: f32) -> Rect {
        Rect {
            x0: self.x0 - d,
            y0: self.y0 - d,
            x1: self.x1 + d,
            y1: self.y1 + d,
        }
    }
}

impl M33 {
    fn identity() -> M33 {
        M33 {
            m: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        }
    }

    /// wire `Concat{matrix}`（行主序 3×3，列向量序与 engine
    /// `Matrix::new_all(m0,m3,m6, m1,m4,m7, m2,m5,m8)` 同构）。
    fn from_wire(w: &[f32; 9]) -> M33 {
        M33 {
            m: [w[0], w[3], w[6], w[1], w[4], w[7], w[2], w[5], w[8]],
        }
    }

    /// SVG affine [sx, ky, kx, sy, tx, ty]（engine `Matrix::from_affine` 同序）。
    fn from_affine(a: &[f32; 6]) -> M33 {
        M33 {
            m: [a[0], a[2], a[4], a[1], a[3], a[5], 0.0, 0.0, 1.0],
        }
    }

    fn translate(x: f32, y: f32) -> M33 {
        M33 {
            m: [1.0, 0.0, x, 0.0, 1.0, y, 0.0, 0.0, 1.0],
        }
    }

    fn scale(x: f32, y: f32) -> M33 {
        M33 {
            m: [x, 0.0, 0.0, 0.0, y, 0.0, 0.0, 0.0, 1.0],
        }
    }

    /// canvas.skew(sx, sy)：x' = x + sx·y，y' = sy·x + y。
    fn skew(sx: f32, sy: f32) -> M33 {
        M33 {
            m: [1.0, sx, 0.0, sy, 1.0, 0.0, 0.0, 0.0, 1.0],
        }
    }

    fn rotate(degrees: f32) -> M33 {
        let rad = degrees.to_radians();
        let (s, c) = rad.sin_cos();
        M33 {
            m: [c, -s, 0.0, s, c, 0.0, 0.0, 0.0, 1.0],
        }
    }

    /// canvas.rotate(degrees, Some((cx, cy))) = T(c) ∘ R ∘ T(−c)。
    fn rotate_about(degrees: f32, cx: f32, cy: f32) -> M33 {
        M33::translate(cx, cy)
            .concat(&M33::rotate(degrees))
            .concat(&M33::translate(-cx, -cy))
    }

    /// concat(a, b) = a∘b（b 先作用），3×3 一般乘积。
    fn concat(&self, b: &M33) -> M33 {
        let a = &self.m;
        let c = &b.m;
        let mut o = [0.0f32; 9];
        for r in 0..3 {
            for k in 0..3 {
                o[r * 3 + k] = a[r * 3] * c[k] + a[r * 3 + 1] * c[3 + k] + a[r * 3 + 2] * c[6 + k];
            }
        }
        M33 { m: o }
    }

    fn is_finite(&self) -> bool {
        self.m.iter().all(|v| v.is_finite())
    }

    /// 无透视（第三行 [0,0,1]）。wire Concat 可携带透视，此处显式拒绝。
    fn is_affine(&self) -> bool {
        self.m[6] == 0.0 && self.m[7] == 0.0 && self.m[8] == 1.0
    }

    /// 仿射逆（调用方须先 `is_affine`；奇异/非有限 → None）。
    fn invert(&self) -> Option<M33> {
        let [a, b, tx, c, d, ty, ..] = self.m;
        let det = a * d - b * c;
        if !det.is_finite() || det == 0.0 {
            return None;
        }
        let inv = M33 {
            m: [
                d / det,
                -b / det,
                (b * ty - tx * d) / det,
                -c / det,
                a / det,
                (tx * c - a * ty) / det,
                0.0,
                0.0,
                1.0,
            ],
        };
        if inv.is_finite() { Some(inv) } else { None }
    }

    /// 仿射映射点（调用方须先 `is_affine`）。
    fn map_point(&self, x: f32, y: f32) -> (f32, f32) {
        (
            self.m[0] * x + self.m[1] * y + self.m[2],
            self.m[3] * x + self.m[4] * y + self.m[5],
        )
    }

    /// 仿射映射矩形的 4 角，返回 AABB（与 skia `map_rect` 一致）。
    fn map_rect(&self, l: f32, t: f32, r: f32, b: f32) -> (f32, f32, f32, f32) {
        let (x0, y0) = self.map_point(l, t);
        let (x1, y1) = self.map_point(r, t);
        let (x2, y2) = self.map_point(l, b);
        let (x3, y3) = self.map_point(r, b);
        (
            x0.min(x1).min(x2).min(x3),
            y0.min(y1).min(y2).min(y3),
            x0.max(x1).max(x2).max(x3),
            y0.max(y1).max(y2).max(y3),
        )
    }
}

#[cfg(test)]
mod tests;
