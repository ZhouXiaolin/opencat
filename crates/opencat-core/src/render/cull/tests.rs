//! 单测：守卫/矩阵/多引用/原始记录不变/滤镜/RestoreToCount/跨 repeat/边界。
//! 全部走 `cull_frame_inner`，不修改进程环境变量。
//!
//! 主场景（与 inspector 语料同构）：位图 tile 1600×1220、显示 2.7×、
//! patternTransform 平移 (-540,-810)：texel 窗 = (197,297,914,703)，halo 后
//! = (195,295,916,705)。

use super::*;
use crate::canvas::paint::{
    ColorFilterSpec, FilterChainSpec, FilterPrimKind, FilterPrimSpec, ImageFilterSpec, PaintSpec,
};
use crate::ir::draw_types::{
    BytesRangeId, ChildRange, DrawOpRange, EffectId, PaintId, PathId, SubtreeId,
};

fn vp() -> (i32, i32) {
    (1920, 1080)
}

/// inspector 场景的 patternTransform（缩放 + 平移使视口窗口落在 tile 中部）。
fn pt() -> [f32; 6] {
    [2.7, 0.0, 0.0, 2.7, -540.0, -810.0]
}

fn tile() -> Rect4 {
    Rect4 {
        x: 0.0,
        y: 0.0,
        width: 1600.0,
        height: 1220.0,
    }
}

fn pattern(subtree: u32) -> DrawOp {
    pattern_with(subtree, pt(), tile(), [1600.0, 1220.0])
}

fn pattern_with(subtree: u32, transform: [f32; 6], tile: Rect4, rasterize: [f32; 2]) -> DrawOp {
    DrawOp::FillPattern {
        path: PathId(0),
        subtree: SubtreeId(subtree),
        tile,
        pattern_transform: transform,
        opacity: 1.0,
        rasterize_at: Some(rasterize),
    }
}

fn oval_batch(rects: &[[f32; 4]]) -> DrawOp {
    let mut flat = Vec::with_capacity(rects.len() * 4);
    let mut colors = Vec::with_capacity(rects.len() * 4);
    for r in rects {
        flat.extend_from_slice(r);
        colors.extend_from_slice(&[10, 20, 30, 255]);
    }
    DrawOp::FillOvalBatch {
        rects: flat.into(),
        colors: colors.into(),
        count: rects.len() as u32,
    }
}

fn batch_of(op: &DrawOp) -> (&[f32], &[u8], u32) {
    let DrawOp::FillOvalBatch {
        rects,
        colors,
        count,
    } = op
    else {
        panic!("not a batch");
    };
    (rects.as_ref(), colors.as_ref(), *count)
}

fn paint_with_filter(f: ImageFilterSpec) -> PaintSpec {
    PaintSpec {
        image_filter: Some(f),
        ..Default::default()
    }
}

fn blur_paint() -> PaintSpec {
    paint_with_filter(ImageFilterSpec::Blur {
        sigma_x: 4.0,
        sigma_y: 4.0,
        crop_rect: None,
        decal: true,
    })
}

fn feimage_paint(subtree: u32) -> PaintSpec {
    paint_with_filter(ImageFilterSpec::Chain(vec![FilterChainSpec {
        primitives: vec![FilterPrimSpec {
            crop: None,
            srgb: true,
            kind: FilterPrimKind::Image { subtree },
        }],
        output: 0,
        source_crop: None,
    }]))
}

fn save_layer(paint: Option<PaintId>, backdrop: bool) -> DrawOp {
    DrawOp::SaveLayer {
        bounds: None,
        paint,
        alpha: 1.0,
        backdrop,
    }
}

fn build(ops: Vec<DrawOp>, subtrees: Vec<Vec<DrawOp>>, paints: Vec<PaintSpec>) -> DrawOpFrame {
    DrawOpFrame {
        ops,
        subtrees,
        paints,
        ..Default::default()
    }
}

/// 三点批：窗内 (500,400)、窗外右 (3000,500)、窗外上 (100,50)。
fn three_ovals() -> Vec<DrawOp> {
    vec![oval_batch(&[
        [500.0, 400.0, 10.0, 10.0],
        [3000.0, 500.0, 10.0, 10.0],
        [100.0, 50.0, 10.0, 10.0],
    ])]
}

#[test]
fn culls_out_of_window_ovals() {
    let mut f = build(vec![pattern(0)], vec![three_ovals()], vec![]);
    let st = cull_frame_inner(&mut f, vp());
    assert_eq!(st.patterns_seen, 1);
    assert_eq!(st.patterns_culled, 1);
    assert_eq!(st.batches_culled, 1);
    assert_eq!(st.ovals_total, 3);
    assert_eq!(st.ovals_removed, 2);
    let (rects, colors, count) = batch_of(&f.subtrees[0][0]);
    assert_eq!(count, 1);
    assert_eq!(rects, [500.0, 400.0, 10.0, 10.0]);
    assert_eq!(colors, [10, 20, 30, 255]);
}

#[test]
fn window_edge_touching_culled() {
    // halo 右沿 = 914+2 = 916：915 保留；916 与 halo 边界相触 → 剔。
    let mut f = build(
        vec![pattern(0)],
        vec![vec![oval_batch(&[
            [915.0, 300.0, 10.0, 10.0],
            [916.0, 300.0, 10.0, 10.0],
        ])]],
        vec![],
    );
    let st = cull_frame_inner(&mut f, vp());
    assert_eq!(st.ovals_removed, 1);
    let (rects, _, count) = batch_of(&f.subtrees[0][0]);
    assert_eq!(count, 1);
    assert_eq!(rects, [915.0, 300.0, 10.0, 10.0]);
}

#[test]
fn fully_inside_batch_untouched() {
    let mut f = build(
        vec![pattern(0)],
        vec![vec![oval_batch(&[[500.0, 400.0, 10.0, 10.0]])]],
        vec![],
    );
    let before = f.subtrees.clone();
    let st = cull_frame_inner(&mut f, vp());
    assert_eq!(st.patterns_culled, 0);
    assert_eq!(st.ovals_total, 1);
    assert_eq!(st.ovals_removed, 0);
    assert_eq!(f.subtrees, before);
}

#[test]
fn cross_repeat_and_oversized_windows_rejected() {
    // 平移 0：窗口外扩越过 tile 原点（x0<0）→ skip（跨 repeat 保守失效）。
    let mut f = build(
        vec![pattern_with(
            0,
            [2.7, 0.0, 0.0, 2.7, 0.0, 0.0],
            tile(),
            [1600.0, 1220.0],
        )],
        vec![three_ovals()],
        vec![],
    );
    let st = cull_frame_inner(&mut f, vp());
    assert_eq!(st.skip_window, 1);
    assert_eq!(st.patterns_culled, 0);
    // 窗口大于 tile（0.5× 位图铺满视口 → texel 窗 3840 > tile 1600）。
    let mut f2 = build(
        vec![pattern_with(
            0,
            [0.5, 0.0, 0.0, 0.5, 0.0, 0.0],
            tile(),
            [1600.0, 1220.0],
        )],
        vec![three_ovals()],
        vec![],
    );
    let st2 = cull_frame_inner(&mut f2, vp());
    assert_eq!(st2.skip_window, 1);
    assert_eq!(st2.patterns_culled, 0);
}

#[test]
fn fixed_density_base_scale_maps_ovals() {
    // rasterize 2× 密度：base=scale(2)，texel 窗 (397,597,1825,1403)，
    // 子树 user 坐标经 base 逆映到 (198.5,297.5,913.5,702.5)。
    let mut f = build(
        vec![pattern_with(0, pt(), tile(), [3200.0, 2440.0])],
        vec![three_ovals()],
        vec![],
    );
    let st = cull_frame_inner(&mut f, vp());
    assert_eq!(st.patterns_culled, 1);
    assert_eq!(st.ovals_removed, 2);
    let (_, _, count) = batch_of(&f.subtrees[0][0]);
    assert_eq!(count, 1);
}

#[test]
fn nonzero_tile_origin_density_pt_order_maps_ovals() {
    // 非零原点 + 各向异性密度 s=(5,10)，使 T(tile) 与 S(1/s) 不可交换。
    // pt·T(tile)·S(1/s) 手算得 device=(2*texel_x-40, 2*texel_y-460)。
    // 视口逆映为 (20,230,980,770)，外扩 margin 2.5 后为 (17,227,983,773)。
    // 再加 2px halo 并除以密度，local 窗=(3,22.5,197,77.5)。
    // 第一椭圆保留；第二在右外；第三底边20低于窗口上边22.5。
    // 交换 T/S 或 pt/T 会越过 repeat 边界；丢密度会误删第一椭圆。
    let mut f = build(
        vec![pattern_with(
            0,
            [10.0, 0.0, 0.0, 20.0, -440.0, -1060.0],
            Rect4 {
                x: 40.0,
                y: 30.0,
                width: 200.0,
                height: 100.0,
            },
            [1000.0, 1000.0],
        )],
        vec![vec![oval_batch(&[
            [100.0, 50.0, 10.0, 10.0],
            [300.0, 50.0, 10.0, 10.0],
            [100.0, 10.0, 10.0, 10.0],
        ])]],
        vec![],
    );
    let st = cull_frame_inner(&mut f, vp());
    assert_eq!(st.patterns_culled, 1);
    assert_eq!(st.batches_culled, 1);
    assert_eq!(st.ovals_total, 3);
    assert_eq!(st.ovals_removed, 2);
    let (rects, colors, count) = batch_of(&f.subtrees[0][0]);
    assert_eq!(count, 1);
    assert_eq!(rects, [100.0, 50.0, 10.0, 10.0]);
    assert_eq!(colors, [10, 20, 30, 255]);
}

#[test]
fn rotate_ctm_and_subtree_translate_tracked() {
    // 主 ctm 旋转 90°（绕视口中心）+ patternTransform 平移 -2000：
    // texel 窗 = (417,1577,1503,3503)。子树坐标 = tile 根像素（密度 1）。
    let mut f = build(
        vec![
            DrawOp::Rotate {
                degrees: 90.0,
                cx: 960.0,
                cy: 540.0,
            },
            pattern_with(
                0,
                [1.0, 0.0, 0.0, 1.0, 0.0, -2000.0],
                Rect4 {
                    x: 0.0,
                    y: 0.0,
                    width: 4000.0,
                    height: 4000.0,
                },
                [4000.0, 4000.0],
            ),
        ],
        vec![vec![
            oval_batch(&[[3000.0, 540.0, 10.0, 10.0]]),
            DrawOp::Translate { x: 2000.0, y: 0.0 },
            oval_batch(&[[-1500.0, 2640.0, 10.0, 10.0]]),
        ]],
        vec![],
    );
    let st = cull_frame_inner(&mut f, vp());
    assert_eq!(st.patterns_culled, 1);
    assert_eq!(st.batches_culled, 1);
    assert_eq!(st.ovals_removed, 1);
    let (rects, _, count) = batch_of(&f.subtrees[0][2]);
    assert_eq!(count, 1);
    assert_eq!(rects, [-1500.0, 2640.0, 10.0, 10.0]);
}

#[test]
fn perspective_and_singular_matrices_rejected() {
    // wire 第三行携带透视（w2=0.3）→ 非仿射拒绝。
    let mut f = build(
        vec![
            DrawOp::Concat {
                matrix: [1.0, 0.0, 0.3, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            },
            pattern(0),
        ],
        vec![three_ovals()],
        vec![],
    );
    let st = cull_frame_inner(&mut f, vp());
    assert_eq!(st.skip_window, 1);
    // 奇异 scale(0,0) → 不可逆拒绝。
    let mut f2 = build(
        vec![DrawOp::Scale { x: 0.0, y: 0.0 }, pattern(0)],
        vec![three_ovals()],
        vec![],
    );
    let st2 = cull_frame_inner(&mut f2, vp());
    assert_eq!(st2.skip_window, 1);
    assert_eq!(st2.patterns_culled, 0);
}

#[test]
fn multi_reference_subtree_skipped() {
    let mask = |s: u32| DrawOp::MaskClip {
        subtree: SubtreeId(s),
        kind: 0,
        region: Rect4 {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
    };
    // 同帧第二个引用（MaskClip）。
    let mut f = build(vec![pattern(0), mask(0)], vec![three_ovals()], vec![]);
    let st = cull_frame_inner(&mut f, vp());
    assert_eq!(st.skip_multi_ref, 1);
    assert_eq!(st.patterns_culled, 0);
    // children 表 SubtreePicture 引用。
    let mut f2 = build(vec![pattern(0)], vec![three_ovals()], vec![]);
    f2.children = vec![RuntimeEffectChildRef::SubtreePicture(SubtreeId(0))];
    let st2 = cull_frame_inner(&mut f2, vp());
    assert_eq!(st2.skip_multi_ref, 1);
    // paint feImage 链引用（engine render_filter_images 会执行子树）。
    let mut f3 = build(
        vec![pattern(0)],
        vec![three_ovals()],
        vec![feimage_paint(0)],
    );
    let st3 = cull_frame_inner(&mut f3, vp());
    assert_eq!(st3.skip_multi_ref, 1);
    // 其他 subtree 内部的 MaskClip 引用。
    let mut f4 = build(vec![pattern(0)], vec![three_ovals(), vec![mask(0)]], vec![]);
    let st4 = cull_frame_inner(&mut f4, vp());
    assert_eq!(st4.skip_multi_ref, 1);
}

#[test]
fn backdrop_prefix_blocks_only_later_filtered_layers() {
    let pid = PaintId(0);
    let bp = |p: Option<PaintId>| DrawOp::SaveBackdropLayer {
        bounds: None,
        radius: [0.0; 4],
        paint: p,
    };
    // pattern 之后出现带 filter 的 SaveBackdropLayer → 前缀重执行，禁裁。
    let mut f = build(
        vec![pattern(0), bp(Some(pid))],
        vec![three_ovals()],
        vec![blur_paint()],
    );
    let st = cull_frame_inner(&mut f, vp());
    assert_eq!(st.skip_backdrop_prefix, 1);
    assert_eq!(st.patterns_culled, 0);
    // backdrop 层在 pattern 之前 → 不阻塞，正常裁。
    let mut f2 = build(
        vec![bp(Some(pid)), pattern(0)],
        vec![three_ovals()],
        vec![blur_paint()],
    );
    let st2 = cull_frame_inner(&mut f2, vp());
    assert_eq!(st2.skip_backdrop_prefix, 0);
    assert_eq!(st2.patterns_culled, 1);
    // 无 filter paint 的 SaveBackdropLayer 不阻塞（use_snapshot 需 backdrop）。
    let mut f3 = build(vec![pattern(0), bp(None)], vec![three_ovals()], vec![]);
    let st3 = cull_frame_inner(&mut f3, vp());
    assert_eq!(st3.skip_backdrop_prefix, 0);
    assert_eq!(st3.patterns_culled, 1);
    // 坏 paint id：fail-closed 阻塞。
    let mut f4 = build(
        vec![pattern(0), bp(Some(PaintId(99)))],
        vec![three_ovals()],
        vec![],
    );
    let st4 = cull_frame_inner(&mut f4, vp());
    assert_eq!(st4.skip_backdrop_prefix, 1);
}

#[test]
fn main_source_filter_layer_skips_pattern() {
    let pid = PaintId(0);
    // blur（空间核）层内 → skip。
    let mut f = build(
        vec![save_layer(Some(pid), false), pattern(0), DrawOp::Restore],
        vec![three_ovals()],
        vec![blur_paint()],
    );
    let st = cull_frame_inner(&mut f, vp());
    assert_eq!(st.skip_source_filter, 1);
    assert_eq!(st.patterns_culled, 0);
    // 逐像素 ColorFilter 层 → 放行（bounds 恒等，无空间扩散）。
    let mut f2 = build(
        vec![save_layer(Some(pid), false), pattern(0), DrawOp::Restore],
        vec![three_ovals()],
        vec![paint_with_filter(ImageFilterSpec::ColorFilter(Box::new(
            ColorFilterSpec::Matrix([0.0; 20]),
        )))],
    );
    let st2 = cull_frame_inner(&mut f2, vp());
    assert_eq!(st2.skip_source_filter, 0);
    assert_eq!(st2.patterns_culled, 1);
    // backdrop:true 的层内容不过滤（engine 摘 filter 挂 backdrop 槽）→ 放行。
    let mut f3 = build(
        vec![save_layer(Some(pid), true), pattern(0), DrawOp::Restore],
        vec![three_ovals()],
        vec![blur_paint()],
    );
    let st3 = cull_frame_inner(&mut f3, vp());
    assert_eq!(st3.skip_source_filter, 0);
    assert_eq!(st3.patterns_culled, 1);
    // paint None（save_layer_alpha）→ 放行。
    let mut f4 = build(
        vec![save_layer(None, false), pattern(0), DrawOp::Restore],
        vec![three_ovals()],
        vec![],
    );
    let st4 = cull_frame_inner(&mut f4, vp());
    assert_eq!(st4.skip_source_filter, 0);
    assert_eq!(st4.patterns_culled, 1);
    // 坏 paint id fail-closed → skip。
    let mut f5 = build(
        vec![
            save_layer(Some(PaintId(99)), false),
            pattern(0),
            DrawOp::Restore,
        ],
        vec![three_ovals()],
        vec![],
    );
    let st5 = cull_frame_inner(&mut f5, vp());
    assert_eq!(st5.skip_source_filter, 1);
    // 层内 Save/Restore 出层后（两层均弹）→ 状态恢复，放行。
    let mut f6 = build(
        vec![
            save_layer(Some(pid), false),
            DrawOp::Save,
            DrawOp::Restore,
            DrawOp::Restore,
            pattern(0),
        ],
        vec![three_ovals()],
        vec![blur_paint()],
    );
    let st6 = cull_frame_inner(&mut f6, vp());
    assert_eq!(st6.skip_source_filter, 0);
    assert_eq!(st6.patterns_culled, 1);
}

#[test]
fn main_restore_to_count_pops_spatial_layer() {
    let pid = PaintId(0);
    // 层内 skip；RestoreToCount{1} 弹两层后 → 放行（engine 诊断版同款）。
    let mut f = build(
        vec![
            save_layer(Some(pid), false),
            pattern(0),
            DrawOp::RestoreToCount { count: 1 },
            pattern(1),
        ],
        vec![three_ovals(), three_ovals()],
        vec![blur_paint()],
    );
    let st = cull_frame_inner(&mut f, vp());
    assert_eq!(st.patterns_seen, 2);
    assert_eq!(st.skip_source_filter, 1);
    assert_eq!(st.patterns_culled, 1);
}

#[test]
fn subtree_filter_layer_batches_kept() {
    let pid = PaintId(0);
    let mut f = build(
        vec![pattern(0)],
        vec![vec![
            save_layer(Some(pid), false),
            oval_batch(&[[3000.0, 500.0, 10.0, 10.0]]),
            DrawOp::Restore,
            oval_batch(&[[3000.0, 500.0, 10.0, 10.0]]),
        ]],
        vec![blur_paint()],
    );
    let st = cull_frame_inner(&mut f, vp());
    assert_eq!(st.skip_filter_layer, 1);
    assert_eq!(st.batches_culled, 1);
    assert_eq!(st.ovals_removed, 1);
    // 层内 batch 原样（位模式不变），层外 batch 被剔。
    let (r1, _, c1) = batch_of(&f.subtrees[0][1]);
    assert_eq!(c1, 1);
    assert_eq!(r1, [3000.0, 500.0, 10.0, 10.0]);
    let (r2, _, c2) = batch_of(&f.subtrees[0][3]);
    assert_eq!(c2, 0);
    assert!(r2.is_empty());
}

#[test]
fn restore_to_count_in_subtree_pops_filter_layer() {
    // SaveLayer+Save 双层后 RestoreToCount{1} 弹光（keep=count-1 下界 0），
    // 后续 batch 可裁——与 engine 诊断版 RestoreToCount 测试同构。
    let pid = PaintId(0);
    let mut f = build(
        vec![pattern(0)],
        vec![vec![
            save_layer(Some(pid), false),
            DrawOp::Save,
            oval_batch(&[[3000.0, 500.0, 10.0, 10.0]]),
            DrawOp::RestoreToCount { count: 1 },
            oval_batch(&[[3000.0, 500.0, 10.0, 10.0]]),
        ]],
        vec![blur_paint()],
    );
    let st = cull_frame_inner(&mut f, vp());
    assert_eq!(st.skip_filter_layer, 1);
    assert_eq!(st.batches_culled, 1);
    // 超界 RestoreToCount：不动栈，无 panic，层内 batch 继续保号。
    let mut f2 = build(
        vec![pattern(0)],
        vec![vec![
            save_layer(Some(pid), false),
            oval_batch(&[[3000.0, 500.0, 10.0, 10.0]]),
            DrawOp::RestoreToCount { count: 99 },
            oval_batch(&[[3000.0, 500.0, 10.0, 10.0]]),
        ]],
        vec![blur_paint()],
    );
    let st2 = cull_frame_inner(&mut f2, vp());
    assert_eq!(st2.skip_filter_layer, 2);
    assert_eq!(st2.batches_culled, 0);
    // 非法 count（≤0）：按 Skia 弹光语义保守处理，不 panic。
    let mut f3 = build(
        vec![pattern(0)],
        vec![vec![
            save_layer(Some(pid), false),
            DrawOp::RestoreToCount { count: -3 },
            oval_batch(&[[3000.0, 500.0, 10.0, 10.0]]),
        ]],
        vec![blur_paint()],
    );
    let st3 = cull_frame_inner(&mut f3, vp());
    assert_eq!(st3.batches_culled, 1);
    // 空栈 Restore 不 panic。
    let mut f4 = build(
        vec![pattern(0)],
        vec![vec![
            DrawOp::Restore,
            oval_batch(&[[3000.0, 500.0, 10.0, 10.0]]),
        ]],
        vec![],
    );
    let st4 = cull_frame_inner(&mut f4, vp());
    assert_eq!(st4.batches_culled, 1);
}

#[test]
fn subtree_guard_fails_on_dangerous_ops() {
    let pid = PaintId(0);
    let dst = Rect4 {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    // 五类整 subtree 禁裁 op（ReplayRange 单独测：其 range 标记覆盖候选，
    // 走 skip_replay_range 路径）。
    let danger: Vec<DrawOp> = vec![
        save_layer(None, true),
        DrawOp::SaveBackdropLayer {
            bounds: None,
            radius: [0.0; 4],
            paint: Some(pid),
        },
        DrawOp::RuntimeEffect {
            effect: EffectId(0),
            uniforms: BytesRangeId(0),
            children: ChildRange { start: 0, len: 0 },
            dst,
        },
        DrawOp::DrawSubtreePicture {
            owner_id: "x".into(),
            x: 0.0,
            y: 0.0,
        },
        // subtree id 用 5，避免与候选 subtree 0 的引用计数互相干扰。
        DrawOp::ReplaySubtreePicture {
            subtree: SubtreeId(5),
            x: 0.0,
            y: 0.0,
        },
    ];
    for op in danger {
        let mut f = build(
            vec![pattern(0)],
            vec![vec![op.clone(), oval_batch(&[[3000.0, 500.0, 10.0, 10.0]])]],
            vec![blur_paint()],
        );
        let st = cull_frame_inner(&mut f, vp());
        assert_eq!(st.skip_guard, 1, "guard must fail for {op:?}");
        assert_eq!(st.patterns_culled, 0);
        assert_eq!(f.subtrees[0].len(), 2, "subtree untouched");
    }
    // 子树内 ReplayRange：其 range 标记覆盖主帧候选 → skip_replay_range。
    let mut f5 = build(
        vec![pattern(0)],
        vec![vec![DrawOp::ReplayRange {
            range: DrawOpRange {
                start_op: 0,
                op_len: 1,
            },
        }]],
        vec![],
    );
    let st5 = cull_frame_inner(&mut f5, vp());
    assert_eq!(st5.skip_replay_range, 1);
    assert_eq!(st5.patterns_culled, 0);
    // MaskClip 干净子树 → 放行。
    let mask_ok = DrawOp::MaskClip {
        subtree: SubtreeId(1),
        kind: 0,
        region: Rect4 {
            x: 0.0,
            y: 0.0,
            width: 5.0,
            height: 5.0,
        },
    };
    let mut f6 = build(
        vec![pattern(0)],
        vec![
            vec![mask_ok, oval_batch(&[[3000.0, 500.0, 10.0, 10.0]])],
            vec![],
        ],
        vec![],
    );
    let st6 = cull_frame_inner(&mut f6, vp());
    assert_eq!(st6.skip_guard, 0);
    assert_eq!(st6.patterns_culled, 1);
    // 坏 mask 子树 id → 禁裁。
    let mask_bad = DrawOp::MaskClip {
        subtree: SubtreeId(9),
        kind: 0,
        region: Rect4 {
            x: 0.0,
            y: 0.0,
            width: 5.0,
            height: 5.0,
        },
    };
    let mut f7 = build(vec![pattern(0)], vec![vec![mask_bad]], vec![]);
    let st7 = cull_frame_inner(&mut f7, vp());
    assert_eq!(st7.skip_guard, 1);
}

#[test]
fn any_main_replay_range_skips_whole_frame() {
    let mut f = build(
        vec![
            pattern(0),
            DrawOp::ReplayRange {
                range: DrawOpRange {
                    start_op: 0,
                    op_len: 2,
                },
            },
            pattern(0),
        ],
        vec![three_ovals()],
        vec![],
    );
    let st = cull_frame_inner(&mut f, vp());
    assert_eq!(st.patterns_seen, 2);
    assert_eq!(st.skip_replay_range, 2);
    assert_eq!(st.patterns_culled, 0);
}

#[test]
fn runtime_effect_picture_range_covers_pattern() {
    let mut f = build(
        vec![
            pattern(0),
            DrawOp::RuntimeEffect {
                effect: EffectId(0),
                uniforms: BytesRangeId(0),
                children: ChildRange { start: 0, len: 1 },
                dst: Rect4 {
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                },
            },
        ],
        vec![three_ovals()],
        vec![],
    );
    f.children = vec![RuntimeEffectChildRef::Picture(DrawOpRange {
        start_op: 0,
        op_len: 1,
    })];
    let st = cull_frame_inner(&mut f, vp());
    assert_eq!(st.skip_replay_range, 1);
    assert_eq!(st.patterns_culled, 0);
}

#[test]
fn bad_batch_data_no_panic_and_untouched() {
    let short = DrawOp::FillOvalBatch {
        rects: vec![1.0, 2.0].into(),
        colors: vec![255, 255, 255, 255].into(),
        count: 1,
    };
    let mut f = build(vec![pattern(0)], vec![vec![short.clone()]], vec![]);
    let before = f.subtrees.clone();
    let st = cull_frame_inner(&mut f, vp());
    assert_eq!(st.skip_bad_batch, 1);
    assert_eq!(st.batches_culled, 0);
    assert_eq!(f.subtrees, before);
}

#[test]
fn original_records_untouched_after_cull() {
    let other = vec![oval_batch(&[[0.0, 0.0, 5.0, 5.0]])];
    let mut f = build(
        vec![
            DrawOp::Rect {
                rect: Rect4 {
                    x: 1.0,
                    y: 2.0,
                    width: 3.0,
                    height: 4.0,
                },
                paint: PaintId(0),
            },
            pattern(0),
        ],
        vec![three_ovals(), other],
        vec![PaintSpec::default()],
    );
    let ops_before = f.ops.clone();
    let paints_before = f.paints.clone();
    let other_before = f.subtrees[1].clone();
    let st = cull_frame_inner(&mut f, vp());
    assert_eq!(st.patterns_culled, 1);
    assert_eq!(f.ops, ops_before, "frame.ops 不变");
    assert_eq!(f.paints, paints_before, "paints 不变");
    assert_eq!(f.subtrees[1], other_before, "其他 subtree 不变");
}

#[test]
fn subtree_restore_recovers_the_transform() {
    let mut f = build(
        vec![pattern(0)],
        vec![vec![
            DrawOp::Save,
            DrawOp::Translate { x: 3000.0, y: 0.0 },
            oval_batch(&[[500.0, 400.0, 10.0, 10.0]]),
            DrawOp::Restore,
            three_ovals().pop().unwrap(),
        ]],
        vec![],
    );
    let st = cull_frame_inner(&mut f, vp());
    assert_eq!(st.ovals_removed, 3);
    assert_eq!(batch_of(&f.subtrees[0][2]).2, 0);
    assert_eq!(batch_of(&f.subtrees[0][4]).0, [500.0, 400.0, 10.0, 10.0]);
}

#[test]
fn culling_reduces_encoded_coordinate_and_color_pools() {
    use crate::ir::draw_encoding::encode_draw_sections;
    use crate::ir::draw_frame::DrawFrameScratch;

    let mut f = build(vec![pattern(0)], vec![three_ovals()], vec![]);
    let before = encode_draw_sections(&f, &mut DrawFrameScratch::default());
    let st = cull_frame_inner(&mut f, vp());
    let after = encode_draw_sections(&f, &mut DrawFrameScratch::default());
    let removed = st.ovals_removed as usize;
    assert_eq!(removed, 2);
    assert_eq!(before.f32_pool.len() - after.f32_pool.len(), removed * 4);
    assert_eq!(before.bytes.len() - after.bytes.len(), removed * 4);
    assert_eq!(before.ops.len(), after.ops.len());
    assert_eq!(before.subtrees.len(), after.subtrees.len());
}

#[test]
fn composed_color_filters_allow_culling() {
    let filter = ImageFilterSpec::Compose(
        Box::new(ImageFilterSpec::ColorFilter(Box::new(
            ColorFilterSpec::Matrix([0.0; 20]),
        ))),
        Box::new(ImageFilterSpec::ColorFilter(Box::new(
            ColorFilterSpec::Matrix([1.0; 20]),
        ))),
    );
    assert!(image_filter_is_pixel_local(&filter));
    let mut f = build(
        vec![
            save_layer(Some(PaintId(0)), false),
            pattern(0),
            DrawOp::Restore,
        ],
        vec![three_ovals()],
        vec![paint_with_filter(filter)],
    );
    assert_eq!(cull_frame_inner(&mut f, vp()).patterns_culled, 1);
}

#[test]
fn composed_spatial_filter_still_blocks_culling() {
    let filter = ImageFilterSpec::Compose(
        Box::new(ImageFilterSpec::ColorFilter(Box::new(
            ColorFilterSpec::Matrix([0.0; 20]),
        ))),
        Box::new(blur_paint().image_filter.unwrap()),
    );
    assert!(!image_filter_is_pixel_local(&filter));
    let mut f = build(
        vec![
            save_layer(Some(PaintId(0)), false),
            pattern(0),
            DrawOp::Restore,
        ],
        vec![three_ovals()],
        vec![paint_with_filter(filter)],
    );
    let st = cull_frame_inner(&mut f, vp());
    assert_eq!(st.patterns_culled, 0);
    assert_eq!(st.skip_source_filter, 1);
}

#[test]
fn svg_filter_chains_remain_unknown() {
    assert!(!image_filter_is_pixel_local(
        &feimage_paint(0).image_filter.unwrap()
    ));
}
