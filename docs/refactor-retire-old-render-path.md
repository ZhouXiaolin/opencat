# 旧渲染路径退役计划（执行文档）

> 本文档自包含：包含目标、已查证事实、已钉死决策、分阶段改动、验证步骤、风险与回滚。
> 可脱离对话独立执行。分支：`refactor/retire-old-render-path`（基于 `main`）。
> 修订：2026-07-19（对照源码审核后落盘）。

## 目标

删除 `crates/opencat-engine/src/render.rs` 里 Composition-based 的旧**渲染入口**
（`render` / `render_mp4` / `render_frame_rgba` / `render_frame_with_target` 等全家），
让 `EnginePipeline`（新路径）成为唯一渲染入口。保留 opencat-see 的 GPU 直绘能力
（改喂 pipeline 产出的 IR 给现有 consumer），保留 web crate 不受影响。

**明确不删**：引擎 `RenderSession` 类型（inspect 继续用）、core `RenderSession`、
`OutputFormat` / `EncodingConfig` / `Mp4Config`、`media/audio` 底层预混实现。

## 已钉死决策（审核后）

这些是审核阶段钉死的选择，执行时不要再二选一：

1. **引擎 `RenderSession` 保留**，只删其上的旧渲染函数。
   `collect_frame_layout_rects`（`inspect/mod.rs`）深度依赖
   `session.core.{catalog,layout_session,font_db}` 与
   `session.platform.{asset_paths,script}`，是 resolve+layout 探针，不是完整渲染；
   迁到 `EnginePipeline` 不自然。不为了删类型硬迁 inspect。
2. **阶段 2 脚本只挂在 `Node` 上**（`.script_source` / `.script_driver`），
   **禁止**依赖 `ParsedComposition.script`——`DefaultPipeline::open_parsed`
   完全忽略该字段，只取 `parsed.root` 建 Composition。
3. **阶段 3 必须拆除 `RenderTargetHandle`**，不只换 consumer。
   阶段 4 会删 `runtime/target.rs` / `frame_view.rs`，see 必须在阶段 3
   改为直接调用 `MetalSkiaRenderTarget` / `WglSkiaRenderTarget` 的
   begin/end/present，否则阶段 4 删不动。
4. **音频预混用选项 a**：从 pipeline `info.audio_plan` 抽
   `build_audio_track_from_pipeline`，opencat-see 与 `render_from_jsonl` 共用。
   不绑旧 `build_audio_track` 包装。
5. **公共 API 直接 breaking**（无 deprecation 过渡期）。
   清理 `opencat/src/lib.rs` re-export，changelog/提交说明写明迁移路径：
   旧 `render_*` / `render_frame_*` → `render_from_jsonl*` / `EnginePipeline`。
6. **不给 `EnginePipeline` 加 `render_frame_to_canvas`**——consumer 已接受任意 canvas。

## 已查证事实（动手前必读）

这些结论来自对源码的逐行核查，是整个方案的地基。**改代码前若行为与这些事实矛盾，停下来重新核查，不要硬推。**

1. **IR / 执行器层早已合一**。新旧路径都产出 `(DrawOpFrame, FrameMediaPlan)`，
   都走 `EngineDrawExecutor::execute` → `replay::replay_frame`（`crates/opencat-engine/src/executor/`）。
   真正分歧只在"谁拥有状态"和"输出到哪个 canvas"。
   - 旧路径：`opencat_core::pipeline::frame::render_frame(composition, idx, &mut RenderSession, script, blob_store)`
   - 新路径：`EnginePipeline::render_frame(i)`（trait 方法）
   - 两者最终都调用 core 里的同一个 `render_frame_with_state`。

2. **`EngineLoaderFrameConsumer` 已接受任意 `&mut Canvas`**
   （`crates/opencat-engine/src/consumer.rs:187-192`）。
   保留 GPU 直绘**不需要给 pipeline 加新 API**——opencat-see 把 MetalSkiaRenderTarget
   的 canvas 喂进 consumer 即可。这是整个方案最关键的一点。

3. **opencat-see 当前源码编译不过**（独立于本次重构的既有问题）：
   - 调用了不存在的 `opencat::host::backend::skia::renderer::shared_raster_engine_typed`
     （`crates/opencat/src/bin/opencat-see.rs:99`）
   - `RenderSession::new(engine)` 传参错误（`:101`）——引擎的 `RenderSession::new()` 无参
     （`crates/opencat-engine/src/render.rs:33`）
   - 整文件 `#[cfg(any(target_os = "macos", target_os = "windows"))]`，Linux CI 跳过。
   - 阶段 3 必须先修好这两个既有编译错误，再做迁移。

4. **两个同名 `RenderSession`，引擎的类型保留、旧渲染函数删除**：
   - 引擎的：`crates/opencat-engine/src/render.rs:27`（字段 `core: opencat_core::runtime::session::RenderSession`）
     —— **inspect 继续用，阶段 4 不删类型**。
   - core 的：`crates/opencat-core/src/runtime/session.rs:18` —— **web crate 用的是这个**
     (`crates/opencat-web/src/wasm_bridge.rs:21,43,61`)，绝不能动。

5. **opencat-see 和 web 预览本质不同**（用户最初以为一样，核查后纠正）：
   - web 的 `WebFrameConsumer`（`crates/opencat-web/src/consumer.rs`）只是 IR 序列化器，
     返回 `Vec<u8>` OCIR blob，真正的光栅化在 JS/CanvasKit 那侧。
   - opencat-see 是完整的原生 GPU 光栅化器，直接画进 CAMetalLayer drawable / WGL framebuffer 0。
   - 所以"迁移 opencat-see"= 改用 `EnginePipeline` + `EngineLoaderFrameConsumer`（原生 Skia replay），
     不是复用 web 的序列化模式。

6. **opencat-see 现在是 GPU 直绘，不是 RGBA blit**：
   - `MetalSkiaRenderTarget`（opencat-see.rs:303-）持有 `current_surface: Option<skia_safe::Surface>`，
     `begin_frame` 从 CAMetalLayer 取 drawable 包成 Skia backend render target。
   - 主循环（opencat-see.rs:773, 809）`render_frame_with_target` → `present_frame()`。
   - 它**不**回读 RGBA。迁移保留 GPU 直绘即保留这套 surface 管理，只换"画什么"的来源，
     **并拆除 `RenderTargetHandle` 间接层**（见决策 3）。

7. **`runtime/surface.rs` 的 `MetalEncodeBridge` 是零引用死代码**（workspace grep 零调用）。
   `runtime/target.rs`、`runtime/render_registry.rs`、`RenderBackend`、`RenderFrameViewKind`
   的唯一生产消费者是 opencat-see（+ 旧 `render_frame_to_target`）。
   阶段 3 拆掉 see 对 handle 的依赖后，阶段 4 可整文件删除。

8. **`RenderBackend` 枚举是死的**：`default_render_backend()`（`runtime/render_registry.rs:3`）
   无条件返回 `Software`；`render_png`/`render_mp4` 对 `Accelerated` 直接 `return Err`
   （render.rs:502-506, 524-528）。`Accelerated` 分支运行时不可达。

9. **render.rs 测试可迁到 pipeline**（活跃约 19 个 `render_frame_rgba` 调用 + 1 纯对齐方法测）：
   - `DefaultPipeline::open_parsed(parsed, loader, scripts, font_db)`
     接受程序化 `ParsedComposition`（`crates/opencat-core/src/pipeline/default.rs:76`），
     无需序列化成 JSONL。
   - `ParsedComposition` 字段全 public（`document.rs:131-141`）：
     `width/height: i32`、`fps: i32`、`duration: f64`、`root: Node`、
     `script: Option<String>`（**open_parsed 忽略**）、`audio_sources`、`font_manifest`。
   - 测试 font_db 用 `crate::fonts::engine_default_font_db()`
     （与 `RenderSession::new` 内部一致，`render.rs:39-41`）。

10. **跨帧状态两边都复用，不是"旧每帧新建 session"**：
    旧测试同一 `make_test_session()` 上连渲 frame 0/1；pipeline 也跨帧复用 cache/history。
    真正差异是状态载体（`RenderSession.core` vs `DefaultPipeline` 的 cache/history）
    与 script host（`EnginePlatform.script` vs `LiveScriptHost`/QJS）。
    断言失败时按这两点查。

11. **opencat-see 音频可改预混整轨循环**：
    - 现用 `render_audio_chunk` 流式（opencat-see.rs:98-129 后台线程，2048 帧块）。
    - rodio `Source` impl（opencat-see.rs:181-211）是无限迭代器
      （`current_span_len=None`、`total_duration=None`），只靠 `next()` 不停 yield。
    - 改成启动时一次性预混 `Vec<f32>`，`Source::next()` 按位置取模循环——drop-in。

12. **音频语义两条路径不等价，阶段 3 前必须对照**：
    - 新路径：`info.audio_plan.segments`（`collect_audio_plan` 预计算）。
    - 旧路径：`media/audio.rs::resolve_audio_intervals` 逐帧扫 active scene，
      处理 `AudioAttachment::Scene`。
    - 阶段 3 抽 `build_audio_track_from_pipeline` 前，对照两者对 scene-attach 的覆盖；
      若 plan 缺 scene-attach，先补 plan 或 see 暂走 composition+loader 的 interval 混合。
      **不得静默换语义。**

13. **web crate 零接触旧路径**：grep 确认 opencat-web 不用引擎 `RenderSession` /
    `RenderBackend` / `RenderTargetHandle` / `RenderFrameViewKind` / 任何旧 `render_*` 函数。
    退役对它零影响。

14. **CLI 已走新路径**：`crates/opencat/src/bin/opencat.rs` 只用
    `render_from_jsonl_with_base` / `render_single_frame_png_with_base`。
    旧 `render` / `render_with_progress` 对 CLI 已是死代码。

15. **类型尺寸事实**（写 helper / ParsedComposition 时用）：
    - `ParsedComposition.fps: i32`
    - `CompositionInfo.fps: u32`、`width/height: u32`
    - `RenderSessionHeader.fps: u32`、`composition_size: (u32, u32)`
    - `Composition.fps: u32`、`width/height: i32`

## 公共 API 破坏清单（阶段 4 删除，阶段 3 起 see 不再用）

从 `opencat` / `opencat-engine` 对外表面移除（直接 breaking，无 `#[deprecated]`）：

| 符号 | 替代 |
|---|---|
| `render` / `render_with_progress` / `render_with_backend*` | `render_from_jsonl*` |
| `render_frame_rgba` / `render_frame_rgb` | `render_single_frame_from_jsonl*` 或自建 pipeline+consumer |
| `render_frame_with_target` / `render_frame_to_target` | pipeline + `EngineLoaderFrameConsumer` + 自有 surface |
| `build_audio_track` / `render_audio_chunk`（render.rs 包装） | `build_audio_track_from_pipeline`（阶段 3 抽出） |
| `RenderBackend` | 删除（死枚举） |
| `RenderTargetHandle` / `RenderFrameViewKind` | 删除；GPU 调用方直接管 surface |
| `default_render_session` | inspect 若需要可继续 `RenderSession::new()` |

**保留**：`RenderSession`（引擎，inspect）、`EncodingConfig` / `Mp4Config` / `OutputFormat`、
`render_from_jsonl*` / `render_single_frame_*`、`EnginePipeline`、`collect_frame_layout_rects`。

## 实施节奏

**4 阶段，每阶段独立可提交、可二分回滚。每阶段结束必须 `cargo build` + `cargo test` 全绿。**
顺序：阶段 1（减码）→ 阶段 2（迁测试）→ 阶段 3（迁 opencat-see + 拆 handle）→ 阶段 4（删除）。

---

## 阶段 1：抽新路径内部 helper（零风险减码）

### 目的

消除 `render_from_jsonl` 内部 PNG/MP4/单帧**三处重复**的
surface+canvas+consumer+read_pixels 装配（Shotgun Surgery 信号）。
这步与退役正交，是阶段 2 迁移测试的基础设施。

### 改动（仅 `crates/opencat-engine/src/render.rs`）

**新增** module-private helper。签名钉死如下
（区分 composition 逻辑尺寸 vs surface/readback 尺寸——MP4 对齐时两者可不同）：

```rust
/// 渲染 pipeline 的单帧到 RGBA。
/// - `surface_w/h`：建 surface 与 `read_pixels` 的尺寸（MP4 传 aligned）
/// - `composition_w/h`：写入 `RenderSessionHeader.composition_size`（保持逻辑尺寸）
/// 每帧新建 surface（接受一次性开销；后续若热路径需要可再改为复用）。
fn render_pipeline_frame_to_rgba(
    pipeline: &mut EnginePipeline,
    media_ctx: &mut MediaContext,
    executor: &mut EngineDrawExecutor,
    surface_w: u32,
    surface_h: u32,
    composition_w: u32,
    composition_h: u32,
    fps: u32,
    frames: u32,
    frame_index: u32,
) -> Result<Vec<u8>> {
    let (mut frame, media_plan) = pipeline.render_frame(frame_index)?;

    let mut surface = surfaces::raster_n32_premul((surface_w as i32, surface_h as i32))
        .ok_or_else(|| anyhow!("failed to create skia raster surface"))?;

    // SAFETY: skia_safe::Canvas wraps a C++ ref-counted object with interior mutability.
    // All draw methods take &self at the Rust level while mutating internal C++ state.
    // The surface owns the canvas and no other references exist at this point.
    #[allow(invalid_reference_casting)]
    let canvas: &mut skia_safe::Canvas =
        unsafe { &mut *(surface.canvas() as *const skia_safe::Canvas as *mut skia_safe::Canvas) };

    let header = RenderSessionHeader {
        composition_size: (composition_w, composition_h),
        fps,
        frames,
    };
    let mut consumer = crate::consumer::EngineLoaderFrameConsumer {
        executor,
        loader: pipeline.loader(),
        media_ctx,
        canvas,
    };
    consumer.consume_frame(&header, &mut frame, &media_plan)?;

    let image = surface.image_snapshot();
    let image_info = ImageInfo::new(
        (surface_w as i32, surface_h as i32),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );
    let mut rgba = vec![0u8; (surface_w as usize) * (surface_h as usize) * 4];
    let read_ok = image.read_pixels(
        &image_info,
        rgba.as_mut_slice(),
        surface_w as usize * 4,
        (0, 0),
        CachingHint::Allow,
    );
    if !read_ok {
        return Err(anyhow!("failed to read pixels from skia surface"));
    }
    Ok(rgba)
}
```

**改三处调用点**调这个 helper：

1. **PNG 分支**（render.rs:151-196）：
   surface/composition 尺寸都用 `info.width/height`：
   ```rust
   let rgba = render_pipeline_frame_to_rgba(
       &mut pipeline, &mut media_ctx, &mut executor,
       info.width, info.height, info.width, info.height,
       info.fps, frame_count, i,
   )?;
   ```
   保留 `image::RgbaImage::from_raw(...).save(filename)`。
   注意：现码 PNG 循环外复用 surface；helper 每帧新建——功能等价，接受开销。

2. **MP4 闭包**（render.rs:217-258）：
   surface 用 `aligned_info`，header 用原始 `info`：
   ```rust
   let rgba = render_pipeline_frame_to_rgba(
       &mut pipeline, &mut media_ctx, &mut executor,
       aligned_info.0, aligned_info.1, info.width, info.height,
       info.fps, frame_count, frame_index,
   )?;
   Ok(rgba)
   ```
   > 现码 MP4 实际是 surface 用 info、read_pixels 用 aligned——本身别扭。
   > helper 统一为 "aligned 建 surface + 原始 composition header"，语义更干净。
   > **可能改变奇数宽高的编码结果**，验证时必须覆盖奇数尺寸 example。

3. **`render_single_frame_from_jsonl_with_base`**（render.rs:308-350）：
   尺寸都用 `info`，返回 `(rgba, info.width, info.height)`。

三处 `#[allow(invalid_reference_casting)]` 收敛到 helper 内一处。

### 验证

```bash
cargo test -p opencat-engine --release
# CLI 出片对比（偶数尺寸）
cargo run --bin opencat --release --features profile -- examples/<某例>.xml -o /tmp/after.mp4
# 与重构前产物对比（帧数、文件大小量级、抽帧像素抽样）
# 奇数尺寸：找 width 或 height 为奇数的 example（或临时改 size）再出片，确认不崩且画面正常
```

### 不碰

旧路径函数、consumer.rs、opencat-see、web crate。

---

## 阶段 2：迁移 render.rs 测试到 EnginePipeline

### 目的

让旧路径函数失去测试覆盖，为阶段 4 删除扫清障碍。

### 涉及测试（render.rs `#[cfg(test)] mod tests`，行 688-1810）

活跃调用 `render_frame_rgba` 的测试（约 19 个，以实际为准）：
`bold_amount_text_renders_every_ascii_glyph`、
`subtree_cache_does_not_apply_node_opacity_twice`、
`split_text_gsap_api_renders_text_property_layer`、
`subtree_cache_preserves_shadow_outside_node_bounds_during_opacity_animation`、
`display_list_and_subtree_cache_both_preserve_overflow_clipping`、
`canvas_node_draw_image_uses_asset_alias_in_backend`、
`subtree_cache_preserves_rust_driven_scale_animation`、
`subtree_cache_invalidation_tracks_descendant_transform_changes`、
`layered_caption_renders_above_timeline_transition`、
`layered_single_scene_renders_bottom_scene_before_caption_overlay`、
`layered_root_caption_without_active_entry_does_not_fail_rendering`、
`timeline_caption_sibling_renders_above_transition`、
`nested_timeline_transition_renders_real_composite`、
`root_timeline_renders_without_root_transition_special_case`、
`gltransition_runtime_effect_samples_timeline_children`、
`light_leak_runtime_effect_samples_timeline_children`、
`script_can_target_hidden_canvas_descendant`、
`nested_canvas_hidden_children_not_visible_without_explicit_draw_picture`、
`indirect_canvas_recursion_returns_error`。

**保留不动**：`composition_alignment_for_video_encoding_rounds_up_to_even_dimensions`
——测的是 `Composition::aligned_for_video_encoding()` 纯方法，不依赖旧渲染路径。

### 改动

**替换测试基础设施**——`make_test_session` / `render_frame_rgba` 全部改成 pipeline 路径：

```rust
fn make_test_pipeline_from_scene(
    scene: impl Into<opencat_core::parse::node::Node>,
    width: i32, height: i32, fps: u32, duration: f64,
) -> EnginePipeline {
    use opencat_core::parse::{ParsedComposition, document::FontManifest};
    // 脚本必须已经挂在 scene/Node 上（.script_source / .script_driver）。
    // ParsedComposition.script 会被 open_parsed 忽略，不要往这里塞。
    let parsed = ParsedComposition {
        width,
        height,
        fps: fps as i32,
        duration,
        root: scene.into(),
        script: None,
        audio_sources: vec![],
        font_manifest: FontManifest::default(),
    };
    let tmp = tempfile::TempDir::new().unwrap();
    let cache = tmp.path().join("cache");
    std::fs::create_dir_all(&cache).unwrap();
    // 注意：TempDir 不能在 pipeline 返回后 drop 掉 cache 路径；
    // 测试里要么 leak tmp（std::mem::forget）、要么把 TempDir 和 pipeline 一起持有。
    // 简单做法：用固定于 target/ 的唯一子目录，测完 remove_dir_all。
    let loader = crate::resource::loader::EngineLoader::new(
        tmp.path().to_path_buf(), cache,
    ).unwrap();
    std::mem::forget(tmp); // 测试进程结束由 OS 回收；或改用 target/opencat-test-<pid>-<name>
    let ctx = crate::js_context::RqJsContext::new().unwrap();
    opencat_core::pipeline::DefaultPipeline::open_parsed(
        parsed, loader, ctx, crate::fonts::engine_default_font_db(),
    ).unwrap()
}

let mut pipeline = make_test_pipeline_from_scene(scene, w, h, fps, duration);
let mut media_ctx = MediaContext::new();
media_ctx.set_composition_fps(fps);
let mut executor = crate::executor::EngineDrawExecutor::new();
let frames = duration_secs_to_frames(duration, fps);
let rgba = render_pipeline_frame_to_rgba(
    &mut pipeline, &mut media_ctx, &mut executor,
    w as u32, h as u32, w as u32, h as u32, fps, frames, 0,
)?;
```

> 测试 helper 的 temp 目录策略执行时选一种干净方案：
> 优先 `target/opencat-test-{pid}-{test_name}` + 末尾 `remove_dir_all`，
> 避免 `TempDir` 与 pipeline 生命周期纠缠。

**特殊处理**：

- **带 script 的测试**（`subtree_cache_does_not_apply_node_opacity_twice` 等）：
  现码已是 `.script_source(...)` 挂在 node 上——**保持**，把挂好脚本的 node
  放进 `ParsedComposition.root`。不要改成 `ParsedComposition { script: Some(...) }`。

- **`bold_amount_text_renders_every_ascii_glyph`**：
  字形位置预测改用 `crate::fonts::engine_default_font_db()`（公开工厂）。
  该测试无文档 `<fonts>`，db 与 pipeline 一致，安全。

- **`split_text_gsap_api_renders_text_property_layer`**：
  从 jsonl 文件读——直接走 `crate::pipeline::open(jsonl_text, loader, ctx)`，
  比构造 ParsedComposition 更简单，且 `pipeline::open` 会正确挂脚本。

- **`composition_alignment_for_video_encoding_*`**：**保留不动**。

### 验证

```bash
cargo test -p opencat-engine --release
```

**逐个测试比对**迁移前后像素结果——重点是这 5 个（行为最微妙）：
- `subtree_cache_does_not_apply_node_opacity_twice`
- `subtree_cache_preserves_shadow_outside_node_bounds_during_opacity_animation`
- `display_list_and_subtree_cache_both_preserve_overflow_clipping`
- `subtree_cache_preserves_rust_driven_scale_animation`
- `subtree_cache_invalidation_tracks_descendant_transform_changes`

它们测的是缓存 + opacity/shadow/clip 的交互，最易出回归。
若断言失败，排查 **状态载体差异**（`RenderSession.core` cache vs pipeline cache/history）
与 **script host 差异**（两边都跨帧复用，不是"新建 session"问题）。

### 不碰

旧路径函数仍保留（测试不再调它们，但函数还在）、opencat-see。

---

## 阶段 3：迁移 opencat-see 到 EnginePipeline + 拆除 RenderTargetHandle

### 目的

让 opencat-see 脱离旧路径与 `RenderTargetHandle`，同时修好既有编译错误。
**只在 macos/windows 编译**（Linux 上 `#[cfg]` 跳过）。
本阶段完成后，阶段 4 才能安全删除 `target.rs` / `frame_view.rs`。

### 子任务（按序）

#### 3.0 re-export 补齐

`crates/opencat/src/lib.rs`（及必要时 `opencat-engine/src/lib.rs`）补上 see 需要的：

- `opencat_engine::pipeline`（或 `pipeline::open`）
- `EngineLoader` / `EnginePipeline`
- `js_context::RqJsContext`（或 re-export `RqJsContext`）
- `consumer::EngineLoaderFrameConsumer`
- `executor::EngineDrawExecutor`
- `media::MediaContext`
- `opencat_core::platform::frame_consumer::RenderSessionHeader`
- `opencat_core::frame_ctx::duration_secs_to_frames`（若尚未可达）

阶段 4 清理旧 re-export 时保留这些新导出。

#### 3.1 修既有编译错误

- 删除对不存在的 `shared_raster_engine_typed` 的调用（opencat-see.rs:99）。
- `RenderSession::new(engine)`（:101）——整体改用 pipeline 后整段删除。

#### 3.2 渲染主循环：pipeline + consumer + 直接 surface API

**启动时**（替代 `Composition::new()...build()` + `RenderSession`）：

```rust
let source_text = std::fs::read_to_string(&input_path)?;
let cache_base = dirs::home_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
let cache_dir = cache_base.join(".opencat").join("assets");
let base_dir = input_path.parent().unwrap_or(Path::new(".")).to_path_buf();
let loader = opencat::/* re-export */EngineLoader::new(base_dir, cache_dir)?;
let ctx = opencat::/* re-export */RqJsContext::new()?;
let mut pipeline = opencat::/* re-export */pipeline::open(&source_text, loader, ctx)?;
let info = pipeline.info().clone();
let mut media_ctx = MediaContext::new();
media_ctx.set_composition_fps(info.fps);
let mut executor = EngineDrawExecutor::new();
```

**拆除 `RenderTargetHandle`**（关键）：

- 停止构造 `RenderTargetHandle::new(...).with_frame_view_resolver(...).with_present_frame(...)`。
- 主循环直接持有 `Box<MetalSkiaRenderTarget>` / `Box<WglSkiaRenderTarget>`。
- 将 target 上的 `begin_frame` / `end_frame` / `present_frame` 提升为 bin 内可调用
  （现有 bridge fn 是 `unsafe fn` 给 handle 用的；改为在 target impl 上提供
  普通方法，或主循环直接调已有的方法——以源码可见性为准，必要时把方法改 `pub`）。
- **删除**对 `RenderTargetHandle` / `RenderFrameViewKind` 的 import 与字段。

**每帧**（替代 `render_frame_with_target`）：

```rust
// 1) 直接 begin_frame（不再经 handle）
gpu_target.begin_frame(info.width as i32, info.height as i32)?;

// 2) 取 canvas：current_surface 在 MetalSkiaRenderTarget / WglSkiaRenderTarget 上
//    .canvas() 返回 &Canvas，需要与 render.rs 相同的 unsafe &mut 转换
#[allow(invalid_reference_casting)]
let canvas: &mut skia_safe::Canvas = unsafe {
    let surface = gpu_target
        .current_surface
        .as_mut()
        .expect("frame surface begun");
    &mut *(surface.canvas() as *const skia_safe::Canvas as *mut skia_safe::Canvas)
};

// 3) pipeline 产 IR
let (mut frame, media_plan) = pipeline.render_frame(frame_index)?;
let header = RenderSessionHeader {
    composition_size: (info.width, info.height),
    fps: info.fps,
    frames: duration_secs_to_frames(info.duration, info.fps),
};
let mut consumer = EngineLoaderFrameConsumer {
    executor: &mut executor,
    loader: pipeline.loader(),
    media_ctx: &mut media_ctx,
    canvas,
};
consumer.consume_frame(&header, &mut frame, &media_plan)?;

// 4) end + present（直接调 target 方法）
gpu_target.end_frame()?;
gpu_target.present_frame()?;
```

> canvas 获取方式以 `MetalSkiaRenderTarget` / `WglSkiaRenderTarget` 实际字段可见性为准。
> 若 `current_surface` 是 private，加一个 `fn canvas_mut(&mut self) -> Result<&mut Canvas>`
> 封装 unsafe，比把字段改 pub 更干净。

**保留不动**：CAMetalLayer drawable / WGL framebuffer 0 的创建与 surface 生命周期管理。
只换"画什么"的来源，并去掉 handle 间接层。

#### 3.3 音频改预混整轨 + 循环

**前置核查（阻塞本子任务）**：对照
`collect_audio_plan`（`opencat-core/src/parse/preflight.rs`）与
`resolve_audio_intervals`（`opencat-engine/src/media/audio.rs`）对
`AudioAttachment::Scene` 的覆盖。不等价则先补 plan 或改用 interval 路径，
**不得静默换语义**。

**删除**：
- `render_audio_chunk_looping`（opencat-see.rs:262-300）
- 后台流式线程（opencat-see.rs:98-129）
- 对 `render_audio_chunk` 的调用

**抽公共预混函数**（推荐落在 `crates/opencat-engine/src/render.rs` 或 `media/audio.rs`）：

```rust
/// 从 pipeline 的 audio_plan 预混整轨。opencat-see 与 render_from_jsonl 共用。
pub(crate) fn build_audio_track_from_pipeline(
    pipeline: &EnginePipeline, // 或 &CompositionInfo + &EngineLoader
) -> Result<Option<AudioTrack>> {
    // 把 render_from_jsonl 里现有的 audio_plan 预混逻辑（render.rs:110-148）搬过来
    // ...
}
```

`render_from_jsonl_with_base` 内联预混改为调这个函数。

**`AudioRenderSource` 改持有 `Arc<Vec<f32>>`**：

```rust
struct AudioRenderSource {
    samples: Arc<Vec<f32>>,
    sample_rate: NonZeroU32,
    channels: NonZeroU16,
    position: usize,
    loop_sample_frames: usize, // = composition_sample_frames(...)
}

fn next(&mut self) -> Option<f32> {
    if self.position >= self.loop_sample_frames * self.channels.get() as usize {
        self.position = 0;
    }
    let sample = self.samples.get(self.position).copied().unwrap_or(0.0);
    self.position += 1;
    Some(sample)
}
```

`current_span_len` / `total_duration` 仍返回 `None`（无限流）。
`composition_sample_frames`（opencat-see.rs:837）循环长度计算保留。

#### 3.4 验证门槛

```bash
# macOS 上（硬门槛；本机 Linux 跑不了，交叉编译通常也缺 Apple SDK）
cargo build --bin opencat-see --release
# 手跑一个带音频的示例预览，确认：
#   - 窗口正常显示动画
#   - 音频循环播放无爆音/断续
#   - frame_index 与音频同步（现有 frame_index/next_redraw_deadline 逻辑保留）
#   - 确认不再依赖 RenderTargetHandle（grep 清零）
```

Linux 上 `cargo build --workspace` 应仍跳过 opencat-see（cfg 不变）。
**无 macOS 验证不得合并阶段 3 为"完成"**——可先提交结构迁移，但 PR/提交说明
必须标 `runtime-unverified-on-macos`，并由有 macOS 的人 follow-up。

### 阶段 3 完成后的状态

- 旧路径函数**仍存在**于 render.rs（测试已不调、see 已不调、CLI 从未调）→ 死代码。
- see **不再引用** `RenderTargetHandle` / `RenderFrameViewKind` / 旧 `render_*`。
- `build_audio_track_from_pipeline` 已抽出，新路径与 see 共用。
- 删除留给阶段 4。

---

## 阶段 4：删除旧渲染入口 + 死代码（大清理）

### 前置确认（已钉死，执行时再 grep 复核）

- 引擎 `RenderSession` **保留**（inspect 用）。
- see 已无 `RenderTargetHandle`（阶段 3 完成）。
- 测试已无 `render_frame_rgba` / `make_test_session`（阶段 2 完成）。
- `ensure_assets_preloaded` 若仅旧路径调用 → 一并删；grep 确认。

### 改动

#### 4.1 `crates/opencat-engine/src/render.rs` 删除函数

- `render` / `render_with_progress` / `render_with_backend` / `render_with_backend_progress`
- `render_mp4` / `render_png`
- `render_frame_rgba` / `render_frame_rgb`
- `render_frame_to_target` / `render_frame_with_target`
- `build_audio_track` / `render_audio_chunk`（**render.rs 包装**；`media/audio` 底层保留）

类型：
- **删除** `RenderBackend`
- **保留** `RenderSession`（inspect）
- **保留** `OutputFormat` / `EncodingConfig` / `Mp4Config`（新路径用）
- **保留** `render_from_jsonl*` / `render_single_frame_*` / `build_audio_track_from_pipeline`
  / `render_pipeline_frame_to_rgba`（若仍 module-private 可留）

删除阶段 2 已不再需要的 `make_test_session`。

#### 4.2 `crates/opencat-engine/src/consumer.rs` 删除

- `EngineFrameConsumer`（旧路径独享）
- `AssetPathSource for opencat_core::resource::AssetPathStore`（若仅旧 consumer 用）
- **保留** `EngineLoaderFrameConsumer`、`prepare_frame`、
  `AssetPathSource for EngineLoader`、`ConsumerError`。

#### 4.3 删除整个文件

- `crates/opencat-engine/src/runtime/target.rs`
- `crates/opencat-engine/src/runtime/surface.rs`（死代码 `MetalEncodeBridge`）
- `crates/opencat-engine/src/runtime/render_registry.rs`
- `crates/opencat-engine/src/runtime/frame_view.rs` —— 删前 grep 确认零外部引用
- 从 `runtime/mod.rs` 移除对应 `mod` 声明。

若 `runtime/preflight.rs::ensure_assets_preloaded` 已无调用者 → 删除该函数或整文件
（grep 确认；`runtime/audio.rs` 等无关模块保留）。

#### 4.4 清理 re-export

- `crates/opencat-engine/src/lib.rs`：移除对删除符号的 `pub use`；
  **保留** `RenderSession` re-export（inspect / 外部若仍用）。
- `crates/opencat/src/lib.rs`：移除
  `render` / `render_with_progress` / `render_with_backend_progress` /
  `render_frame_rgba` / `render_frame_rgb` / `render_frame_with_target` /
  `render_frame_to_target` / `build_audio_track` / `render_audio_chunk` /
  `RenderBackend` / `RenderTargetHandle` / `RenderFrameViewKind` /
  `default_render_session`。
  **保留** `RenderSession`、`EncodingConfig` / `Mp4Config` / `OutputFormat`、
  新路径函数、阶段 3 补的 pipeline/consumer/executor re-export。

### 验证

```bash
cargo build --workspace --release
cargo test --workspace --release
cargo clippy --workspace --release 2>&1 | grep -E "warning|error" | head
cargo build -p opencat-web  # 确认 web 没受影响

# 手动冒烟
cargo run --bin opencat --release --features profile -- examples/<某例>.xml -o /tmp/after.mp4
```

确认 clippy 无新 `dead_code` warning；grep 确认旧符号零残留（除文档/注释）。

---

## 风险与回滚

| 风险 | 缓解 |
|---|---|
| 阶段 2 测试迁移引入像素回归 | 每个测试迁移后单独跑；重点 5 个 cache/opacity/shadow/clip 测试若失败，查状态载体与 script host 差异（两边都跨帧复用） |
| 阶段 1 helper 改变奇数尺寸 MP4 语义 | 验证覆盖奇数宽高 example；surface=aligned、header=原始 |
| opencat-see macOS 验证缺失（本机 Linux） | 阶段 3 硬门槛需 macOS；无验证则提交标 `runtime-unverified-on-macos` |
| 音频 plan 与 scene-attach 语义差 | 阶段 3.3 前置对照 `collect_audio_plan` vs `resolve_audio_intervals`；不等价先补 plan |
| 删 `RenderTargetHandle` 后 see 编译/运行断 | 阶段 3 先于阶段 4；阶段 3 验收含 grep handle 清零 |
| web crate 意外受影响 | 每阶段 `cargo build -p opencat-web`；web 用 core session，零交集（事实 13） |
| 公共 API 破坏吓到外部调用方 | 提交说明写迁移表；本仓库内 CLI/see 已迁完再删 |
| inspect 被误伤 | 引擎 `RenderSession` **保留**（决策 1）；阶段 4 不删类型 |

**回滚粒度**：每阶段独立提交，任一阶段出问题 `git revert <commit>` 回到上一稳定点。

## 不在本次范围

- web crate 的渲染路径（已正确分叉，不动）
- 给 `EnginePipeline` 加 `render_frame_to_canvas` 之类新 API（不需要）
- `EnginePlatform` 重构（共享基础设施，与渲染路径退役无关）
- 把 inspect 迁到 pipeline / 引入 `InspectSession`（另开任务，若未来要删引擎 `RenderSession`）
- `media/audio` 底层 interval 预混与 `audio_plan` 的长期合一（阶段 3 只做对照与 see 迁移）

---

## 执行环境备忘（skia 构建）

执行时若遇到 skia-bindings 0.93.1 构建失败（`git-sync-deps` / `fetch-gn` 报
`Network is unreachable`），原因和应对：

- skia-bindings 的 build script 在 binary-cache 未命中时会 fallback 到源码构建，
  需要从 `chrome-infra-packages.appspot.com` 下载 `gn`——该域名在国内常被墙。
- 项目开了 `binary-cache` 特性（`crates/opencat-engine/Cargo.toml`），
  命中时直接下预编译二进制，不走源码构建。
- **若你环境之前成功构建过**（gn 已落盘、binary-cache 已下），后续构建复用缓存、不联网。
- **首次或缓存失效时**需联网；可用代理或镜像。环境变量：
  - `SKIA_BINARIES_URL` 可指向镜像（`build_support/binary_cache/env.rs`）
  - `FORCE_SKIA_BUILD=1` 强制源码构建（需完整网络）
- 验证命令统一用 `--release`（与已知能跑的 `cargo run --bin opencat --release --features profile` 一致），
  避免 debug/release profile 切换触发 skia 重构建。

> 阶段 1 的改动不碰 skia 依赖指纹（只改 Rust 源码），理论上不会触发 skia 重构建。
> 但保险起见，每阶段验证前确认 skia 缓存在（`ls` 看产物），再跑 test。
