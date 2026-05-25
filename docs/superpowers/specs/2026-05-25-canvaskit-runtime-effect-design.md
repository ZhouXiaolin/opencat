# canvaskit-js 子集中实现 SkSL RuntimeEffect API

- **日期**: 2026-05-25
- **作者**: Solaren + Claude
- **状态**: Draft (待审阅)
- **关联**: `json/profile-showcase.xml` 中 s1-canvas 水波纹示例

## 1. 背景与目标

### 1.1 现状

- **canvaskit-js 子集** (`crates/opencat-core/src/script/runtime/canvas_api.js`) 已提供 CanvasKit 同形 API 的一个子集：`Paint` / `Path` / `Font` / `Canvas.{drawRect,drawImage,drawPath,...}` / `canvas.getSubTree()` / `canvas.drawPicture()` 等。
- **底层 IR** (`crates/opencat-core/src/ir/draw_op.rs`) 已有 `DrawOp::RuntimeEffect { effect, uniforms, children, dst }`，配合 `EffectRef`、`RuntimeEffectChildRef::{Image, Picture, Shader}`、`builder.intern_effect/intern_bytes/push_child` 等基础设施。
- **Engine 执行端** (`crates/opencat-engine/src/executor/replay.rs:481`) 已能用 `skia_safe::RuntimeEffect::make_shader(uniform_data, &inputs, None)` 把 IR 中的 RuntimeEffect 真正绘制到 skia canvas（GLTransition / light_leak transition 已经走这条线）。
- **Web 执行端** (`crates/opencat-web/web/src/draw-ir.ts`) 已能从二进制 IR 解出 `RuntimeEffectSpec`，通过 canvaskit-wasm 的 `CanvasKit.RuntimeEffect.Make(sksl).makeShaderWithChildren(...)` 绘制。

### 1.2 问题

XML 中现在的 script:

```js
const sb = c.getSubTree();
const sbShader = sb.makeShader(CK.TileMode.Clamp, CK.TileMode.Clamp);
const effect = CK.RuntimeEffect.Make(sksl);
const shader = effect.makeShaderWithChildren([p, 6.0, 0.035, 0.006], [sbShader]);
const paint = new CK.Paint();
paint.setShader(shader);
c.drawRect(CK.LTRBRect(0,0,360,480), paint);
```

— **跑不通**。canvas_api.js 里：

- `subTree.makeShader` ❌ 不存在
- `CanvasKit.RuntimeEffect` ❌ 不存在
- `effect.makeShaderWithChildren` ❌ 不存在
- `paint.setShader` ❌ 不存在

也就是 RuntimeEffect IR 的所有"难活"在 Rust/skia 那边都完成了，**仅缺脚本层的 wrapper 对象 + 一个 native binding** 把 SkSL + uniforms + children 喂进 `DrawOp::RuntimeEffect`。

### 1.3 目标

让脚本能用与 CanvasKit-WASM 一致的 API 在 canvas 上跑任意 fragment SkSL effect，**且产出的 IR 与现有 GLTransition 路径完全同型**（同一个 `DrawOp::RuntimeEffect`，同一条 engine/web replay 路径）。

### 1.4 非目标

- **不实现 picture-as-shader-child**（即不让 `canvas.getSubTree().makeShader()` 工作）。Solaren 明确说"不需要 subtree，更普世"。Picture child 的需求归到 follow-up。
- 不支持 gradient child / ColorFilter child / ImageFilter child。
- 不支持 SkSL 同步预编译（编译错误回 null）—— 编译延迟到 render 阶段，失败时 replay 跳过 + warn。
- 不支持 SkSL 中 `mat2/mat3/mat4/array` 之类的复合 uniforms。**uniforms 仅支持扁平 float 序列**（与 CanvasKit-WASM 的 `Float32Array` 兼容）。

## 2. 决策摘要

| 决策点 | 选择 |
|---|---|
| API 形状 | 完整对齐 CanvasKit-WASM |
| Child shader 类型 | Image only（picture 暂不实现） |
| SkSL 编译时机 | 延迟，在 render 阶段 `intern_effect` 时触发；按 hash 去重；编译失败 → replay 跳过 + warn |
| Uniforms 类型 | 扁平 `Float32Array` / `number[]`，按 SkSL 声明顺序消费 |
| IR 形态 | 复用现有 `DrawOp::RuntimeEffect`，**不新增 enum 变体** |
| 中间载体 | 扩展 `CanvasMutations` / `DrawScriptDisplayItem`，在 `execute_draw_op` 翻译进主 builder 时做一次 remap |

## 3. 架构

### 3.1 数据流总览

```
JS script              MutationStore               DisplayItem            主 DrawOpBuilder
─────────              ─────────────               ───────────            ───────────────
RuntimeEffect.Make     ─┐                                                
effect.makeShader      ─┤ 仅 wrapper                                     
paint.setShader        ─┤ 不触 native                                    
c.drawRect(rect,paint) ─┴─> __canvas_runtime_effect_draw(...)            
                                │                                        
                                ▼                                        
                       CanvasMutations {                                 
                         commands: [..., DrawOp::RuntimeEffect{          
                                          effect: EffectId(local),       
                                          uniforms: BytesRangeId(local), 
                                          children: ChildRange{l,n},     
                                          dst}],                         
                         script_effects:        [EffectRef{hash, sksl}], 
                         script_uniform_bytes:  [...f32 bytes...],       
                         script_uniform_ranges: [TableRange{...}],       
                         script_children:       [RuntimeEffectChildRef::Image(...)],
                       }                                                 
                                │                                        
                                ▼ display_build 透传                     
                       DrawScriptDisplayItem { commands, script_*, ... } 
                                │                                        
                                ▼ render::helpers::execute_draw_op       
                       remap (local id -> 主 builder id)                 
                       intern_effect / intern_bytes / push_child         
                                │                                        
                                ▼                                        
                                            DrawOp::RuntimeEffect{       
                                              effect: 主 EffectId,       
                                              uniforms: 主 BytesRangeId, 
                                              children: 主 ChildRange,   
                                              dst}                       
                                                  │                      
                                                  ▼ engine/web replay    
                                              [现有 RuntimeEffect 执行路径，零改动]
```

### 3.2 关键不变量

1. **engine / web 的 IR 读取与 replay 零改动**：主 builder 输出的 `DrawOp::RuntimeEffect` 与 GLTransition 产出的形态完全一致。
2. **EffectId / BytesRangeId / ChildRange 在 commands 阶段是 "source-local"，进入主 builder 时被 remap 为 "builder-local"**。这是这套方案唯一需要小心维护的语义。
3. **script_effects / script_uniform_bytes / script_uniform_ranges / script_children 是 per-CanvasMutations 的并行表**，它们与 commands 中的 RuntimeEffect ops 一一对应。
4. **SkSL hash 由 JS 端计算** (FNV-1a 64-bit on UTF-8 bytes)，作为 `EffectRef.hash`。主 builder 的 `intern_effect` 仍然以 hash 去重 —— 多个 frame、多个 canvas 引用同一 SkSL 文本，最终只编译一次 skia RuntimeEffect。

## 4. 详细设计

### 4.1 JS 侧 (`canvas_api.js`)

新增 4 个对象/方法：

```js
// ── RuntimeEffect ───────────────────────────────────────────
class RuntimeEffect {
    constructor(sksl) {
        this.__opencatRuntimeEffect = true;
        this._sksl = String(sksl);
        this._hash = fnv1a64(this._sksl);   // BigInt 内部，传 binding 时切成两个 u32 或 stringify
    }
    delete() {}

    makeShader(uniforms) {
        return makeRuntimeShader(this, uniforms, []);
    }

    makeShaderWithChildren(uniforms, children) {
        return makeRuntimeShader(this, uniforms, children);
    }
}

function makeRuntimeShader(effect, uniforms, children) {
    return {
        __opencatShader: 'runtime',
        effect,
        uniforms: Array.from(uniforms || [], toFiniteNumber),
        children: (children || []).map(ensureChildShader),
    };
}

function ensureChildShader(c) {
    if (!c) throw new Error('child shader is null');
    if (c.__opencatShader === 'image') return c;
    throw new Error('only image child shaders are supported; gradient/picture not yet implemented');
}

// ── Image shader (extends existing ctx.getImage) ─────────────
ctx.getImage = function(assetId) {
    const handle = { __opencatImage: true, assetId: String(assetId), delete() {} };
    handle.makeShader = (tileX, tileY) => ({
        __opencatShader: 'image',
        assetId: handle.assetId,
        tileX: normalizeTileMode(tileX),
        tileY: normalizeTileMode(tileY),
    });
    return handle;
};

// ── TileMode enum ─────────────────────────────────────────────
CanvasKit.TileMode = { Clamp:'clamp', Repeat:'repeat', Mirror:'mirror', Decal:'decal' };

// ── CanvasKit.RuntimeEffect.Make ─────────────────────────────
CanvasKit.RuntimeEffect = {
    Make(sksl) {
        if (typeof sksl !== 'string' || sksl.length === 0) return null;
        return new RuntimeEffect(sksl);
    },
};

// ── Paint.setShader ───────────────────────────────────────────
Paint.prototype.setShader = function(shader) {
    if (shader == null) { this._shader = null; return; }
    if (shader.__opencatShader !== 'runtime' && shader.__opencatShader !== 'image') {
        throw new Error('setShader expects a shader handle');
    }
    this._shader = shader;
};

// ── drawRect 路由 ─────────────────────────────────────────────
// (在 makeCanvas 内部的 drawRect 方法里)
drawRect(rect, paint) {
    const resolvedPaint = ensurePaint(paint);
    const normalized = normalizeRect(rect);
    if (resolvedPaint._shader && resolvedPaint._shader.__opencatShader === 'runtime') {
        const sh = resolvedPaint._shader;
        const eff = sh.effect;
        __canvas_runtime_effect_draw(
            id,
            eff._sksl,
            // hash 切成 hi/lo 两个 u32 传 (binding 层重组成 u64)
            Number((eff._hash >> 32n) & 0xffffffffn),
            Number(eff._hash & 0xffffffffn),
            sh.uniforms,
            JSON.stringify(sh.children),    // [{__opencatShader:'image', assetId, tileX, tileY}, ...]
            normalized.x, normalized.y, normalized.width, normalized.height,
        );
        return this;
    }
    // ... 现有的 fill_rect / stroke_rect 路径不变
}
```

`fnv1a64` 用纯 JS BigInt 实现，~30 行；放在 `canvas_api.js` 顶部 util。

### 4.2 Rust 副表 (`crates/opencat-core/src/script/mutations.rs`)

```rust
use crate::ir::draw_op::DrawOp;
use crate::ir::draw_types::{EffectRef, RuntimeEffectChildRef, TableRange};

#[derive(Debug, Clone, Default)]
pub struct CanvasMutations {
    pub commands: Vec<DrawOp>,
    // ── Script-local RuntimeEffect tables ────────────────────
    pub script_effects: Vec<EffectRef>,
    pub script_uniform_bytes: Vec<u8>,
    pub script_uniform_ranges: Vec<TableRange>,
    pub script_children: Vec<RuntimeEffectChildRef>,
}
```

### 4.3 Recorder (`crates/opencat-core/src/script/recorder/store.rs`)

```rust
impl MutationRecorder for MutationStore {
    fn record_canvas_runtime_effect(
        &mut self,
        id: &str,
        sksl: String,
        sksl_hash: u64,
        uniforms_f32: &[f32],
        children: &[ScriptChildSpec],   // 反序列化自 children_json
        dst: Rect4,
    ) {
        let entry = self.canvas_entry(id);

        // 1. effect: 副表内按 hash 去重
        let effect_local_id = match entry.script_effects.iter().position(|e| e.hash == sksl_hash) {
            Some(i) => i as u32,
            None => {
                entry.script_effects.push(EffectRef { hash: sksl_hash, sksl });
                (entry.script_effects.len() - 1) as u32
            }
        };

        // 2. uniforms: 按 f32 平铺到 byte pool
        let start = entry.script_uniform_bytes.len() as u32;
        for v in uniforms_f32 {
            entry.script_uniform_bytes.extend_from_slice(&v.to_ne_bytes());
        }
        let len = (entry.script_uniform_bytes.len() as u32) - start;
        entry.script_uniform_ranges.push(TableRange { start, len });
        let uniforms_local_id = (entry.script_uniform_ranges.len() - 1) as u32;

        // 3. children: 转 RuntimeEffectChildRef::Image
        let children_start = entry.script_children.len() as u32;
        for c in children {
            entry.script_children.push(c.to_ir_child_ref());
        }
        let children_len = (entry.script_children.len() as u32) - children_start;

        // 4. push 命令
        entry.commands.push(DrawOp::RuntimeEffect {
            effect: EffectId(effect_local_id),
            uniforms: BytesRangeId(uniforms_local_id),
            children: ChildRange { start: children_start, len: children_len },
            dst,
        });
    }
}
```

### 4.4 Binding 注册 (`crates/opencat-core/src/script/bindings.rs`)

```rust
$binding! { node $rec $id canvas_runtime_effect_draw (
    $id: &str,
    sksl: String,
    hash_hi: u32,
    hash_lo: u32,
    uniforms: Vec<f32>,
    children_json: String,
    dst_x: f32, dst_y: f32, dst_w: f32, dst_h: f32,
) {
    let hash = ((hash_hi as u64) << 32) | (hash_lo as u64);
    let children: Vec<ScriptChildSpec> =
        serde_json::from_str(&children_json)
            .map_err(|e| anyhow::anyhow!("children_json decode: {e}"))?;
    $rec.record_canvas_runtime_effect(
        $id, sksl, hash, &uniforms, &children,
        Rect4 { x: dst_x, y: dst_y, width: dst_w, height: dst_h },
    );
}}
```

`ScriptChildSpec` 是一个简单 serde 结构：

```rust
#[derive(serde::Deserialize)]
#[serde(tag = "__opencatShader")]
enum ScriptChildSpec {
    #[serde(rename = "image")]
    Image {
        #[serde(rename = "assetId")] asset_id: String,
        #[serde(rename = "tileX")]   tile_x: TileModeName,
        #[serde(rename = "tileY")]   tile_y: TileModeName,
    },
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum TileModeName { Clamp, Repeat, Mirror, Decal }
```

(暂时只支持 image。tile mode 当前 engine 端 image child 是 hard-coded Clamp, 我们传过去先存着，engine 改造在 §6.1)

### 4.5 DisplayItem 透传 (`crates/opencat-core/src/display/list.rs`)

```rust
pub struct DrawScriptDisplayItem {
    pub bounds: DisplayRect,
    pub commands: Vec<DrawOp>,
    pub script_effects: Vec<EffectRef>,
    pub script_uniform_bytes: Vec<u8>,
    pub script_uniform_ranges: Vec<TableRange>,
    pub script_children: Vec<RuntimeEffectChildRef>,
    pub drop_shadow: Option<DropShadow>,
    pub hidden_subtree: Vec<HiddenChildDisplayNode>,
}
```

`display/build.rs` 把 `CanvasMutations` 的副表整体 clone 进 `DrawScriptDisplayItem`（无重排），field-by-field 透传。

### 4.6 Remap (`crates/opencat-core/src/render/helpers.rs::execute_draw_op`)

```rust
fn execute_draw_op(
    b: &mut DrawOpBuilder,
    op: &DrawOp,
    state: &mut LocalPaintState,
    src: Option<&DrawScriptSource>,    // 新增参数；None 表示非 script 来源（保留给 GLTransition 等直接走 IR 的 caller）
) -> Result<(), RenderError> {
    match op {
        // ... 既有 case ...

        DrawOp::RuntimeEffect { effect, uniforms, children, dst } => {
            let Some(src) = src else {
                // 不是 script 来源，直接 clone（保留 GLTransition 等场景）
                b.push(DrawOp::RuntimeEffect {
                    effect: *effect, uniforms: *uniforms, children: *children, dst: *dst,
                });
                return Ok(());
            };

            // remap effect
            let effect_ref = &src.effects[effect.0 as usize];
            let new_effect = b.intern_effect(effect_ref.hash, &effect_ref.sksl);

            // remap uniforms
            let range = src.uniform_ranges[uniforms.0 as usize];
            let uniform_slice = &src.uniform_bytes[range.start as usize..(range.start + range.len) as usize];
            let new_uniforms = b.intern_bytes(uniform_slice);

            // remap children
            let cstart = children.start as usize;
            let cend = cstart + children.len as usize;
            let new_child_start = b.children_len() as u32;
            for child in &src.children[cstart..cend] {
                b.push_child(child.clone());
            }
            let new_children = ChildRange { start: new_child_start, len: children.len };

            b.push(DrawOp::RuntimeEffect {
                effect: new_effect,
                uniforms: new_uniforms,
                children: new_children,
                dst: *dst,
            });
        }

        // ... 其他 case 不变 ...
    }
    Ok(())
}
```

调用方：
- `render_draw_script` 在 `helpers.rs:1051` 调 `execute_draw_op(..., Some(&DrawScriptSource::from(item)))`
- GLTransition / other helpers 直接调 `execute_draw_op(..., None)`

`DrawScriptSource` 是一个借用结构：
```rust
struct DrawScriptSource<'a> {
    effects: &'a [EffectRef],
    uniform_bytes: &'a [u8],
    uniform_ranges: &'a [TableRange],
    children: &'a [RuntimeEffectChildRef],
}
```

### 4.7 Picture-as-shader 兜底（保持原有 `getSubTree().drawPicture`）

XML 中如果 `RuntimeEffect.Make` 返回 null（编译失败）走 fallback：

```js
}else{
  c.drawPicture(sb,0,0);
}
```

这条 fallback 路径在本方案中**完全不动**：`getSubTree()` / `drawPicture` 沿用现有 `DrawSubtreePicture` 展开机制。

## 5. XML 示例适配

原 XML s1-canvas 段需要把 `sb.makeShader` 改成 `ctx.getImage('s1-decor-img').makeShader(Clamp, Clamp)`：

```js
const CK = ctx.CanvasKit;
const c = ctx.getCanvasById('s1-canvas');
const imgShader = ctx.getImage('s1-decor-img').makeShader(CK.TileMode.Clamp, CK.TileMode.Clamp);

const sksl = [/* ... 同原文 ... */].join('\n');

const effect = CK.RuntimeEffect.Make(sksl);
if (effect) {
    const p = ctx.currentFrame / ctx.sceneFrames;
    const shader = effect.makeShaderWithChildren([p, 6.0, 0.035, 0.006], [imgShader]);
    const paint = new CK.Paint();
    paint.setShader(shader);
    c.drawRect(CK.LTRBRect(0, 0, 360, 480), paint);
} else {
    // fallback 不再画 picture（image 已是 canvas 子元素，会被正常渲染）
}
```

> 注：`<image id="s1-decor-img">` 是 `<canvas>` 的 hidden child，常规流程里它会被画到 canvas 上；为了让 ripple shader 覆盖原画面，drawRect(0,0,360,480) 的清空效果由 SkSL 自身的输出承担。如果出现"双重绘制"（image 先画一次，再被 shader 画一次），SkSL `image.eval(uv+dir*d)` 已经把原图作为底图采样，视觉上是叠加上波纹。如需完全替换，可以在 script 起手先 `c.clear()` 或在渲染管线层关闭 `<canvas>` 的 hidden-subtree 渲染（这是 follow-up，不属本 spec 范围）。

## 6. 已知缺口与 follow-up

### 6.1 Image child tile mode 未透传到 engine

`crates/opencat-engine/src/executor/replay.rs:514` 处的 image-as-child 创建 shader 时 hard-coded `(TileMode::Clamp, TileMode::Clamp)`。本 spec 的脚本 binding 收下了脚本传的 tileX/tileY，但 engine 侧暂时忽略 —— 视觉上对 ripple 例子无影响（采样不会越界）。完整透传放 follow-up。

### 6.2 Picture-as-shader child

`RuntimeEffectChildRef::Picture(DrawOpRange)` 已存在，engine 的 `picture_shader_for_range` 也已工作。要让脚本能 `subTree.makeShader()`，需要在 remap 阶段把 hidden_subtree 当场录制成主 builder 的一段 ops 并产生 DrawOpRange。这条线**不在本 spec 范围**。

### 6.3 SkSL uniform 类型校验

当前完全不校验 uniforms 数量 / SkSL uniform 数量是否一致。skia RuntimeEffect 会在编译/绑定时给出错误，体现为 replay 阶段 warn。Follow-up 可以在 JS 端解析 SkSL 头部的 `uniform xxx` 声明做提前校验。

### 6.4 Shader composition

`paint.setShader(imageShader)` 直接画 rect（即 shader 不嵌进 RuntimeEffect 而是直接当 fill paint）这条路径暂未实现。本 spec 只支持 RuntimeEffect shader 走 drawRect。Follow-up 可以让 image shader 也能直接 fill。

## 7. 验收

### 7.1 单元/集成测试

- **Rust** `crates/opencat-core/src/script/recorder/store.rs`:
  - `record_canvas_runtime_effect_stores_effects_and_uniforms`：一次 record 后，CanvasMutations 副表 + commands 状态正确。
  - `record_canvas_runtime_effect_dedups_by_hash`：同一 sksl 二次 record，effects 表只增长一次。
- **Rust** `crates/opencat-core/src/render/helpers.rs`:
  - `execute_draw_op_remaps_runtime_effect_ids`：构造一个 `DrawScriptDisplayItem`，commands 含 `DrawOp::RuntimeEffect { EffectId(0), BytesRangeId(0), ChildRange{0,1}, ... }`，副表填好；execute 后主 builder 的 effects/byte_ranges/children 正确 intern，输出的 RuntimeEffect op 字段是主 builder id。
- **Rust** `crates/opencat-engine` 已有 `runtime_effect_picture_child_samples_draw_op_range` 等测试；本 spec 不破坏现有用例。
- **集成** 在 `examples/` 加一个最小 ripple 例子（命名 `ripple_canvas.rs`），渲染 1 帧 PNG，断言中心一圈像素与边角像素差异 > 阈值（确认 ripple 起效）。

### 7.2 端到端

- `cargo run --example compare_transitions` 仍跑通。
- 修改后的 `json/profile-showcase.xml` 渲染 414 帧不报错；s1-canvas 区域在中间 frame 上肉眼可见水波纹。
- Web 端 `web/` 加载同 XML，canvas 上同样出现水波纹（验证 IR 跨端一致）。

### 7.3 性能

- 同一 SkSL 跨 frame / 跨 canvas 不重复编译（`intern_effect` 按 hash 去重已保证）。
- 单 frame 内主 builder 的 children/byte_ranges 没有膨胀超过实际所需。

## 8. 实现顺序建议

1. **Rust IR / 副表**：扩展 `CanvasMutations` / `DrawScriptDisplayItem`，加 `record_canvas_runtime_effect` recorder 方法，加单元测试 (§7.1 前两条)。
2. **Render remap**：改 `execute_draw_op` 签名，加 `DrawScriptSource`，写 remap 逻辑，加单元测试 (§7.1 第三条)。
3. **Binding**：注册 `canvas_runtime_effect_draw`，加 `ScriptChildSpec` 反序列化，跑 dispatch 测试。
4. **JS wrapper**：扩展 canvas_api.js，加 `RuntimeEffect` / `TileMode` / `Paint.setShader` / `drawRect` 路由 / `image.makeShader`。
5. **XML 调整 + 端到端**：把 s1-canvas 改成 image-as-child，跑 414 帧，看 ripple。

每一步独立可验证。
