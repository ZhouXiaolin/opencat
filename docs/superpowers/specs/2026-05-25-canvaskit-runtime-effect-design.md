# canvaskit-js 子集中实现 SkSL RuntimeEffect API

- **日期**: 2026-05-25
- **作者**: Solaren + Claude
- **状态**: Draft v2 (待审阅)
- **关联**: `json/profile-showcase.xml` 中 s1-canvas 水波纹示例
- **修订**: v1 在 code review 中暴露了 parallel-table 数据流断点 + Make() 返回值矛盾，v2 改走 inline-op 形态

## 1. 背景与目标

### 1.1 现状

- **canvaskit-js 子集** (`crates/opencat-core/src/script/runtime/canvas_api.js`) 已提供 CanvasKit 同形 API 的子集：`Paint` / `Path` / `Font` / `Canvas.{drawRect,drawImage,drawPath,...}` / `canvas.getSubTree()` / `canvas.drawPicture()` 等。
- **底层 IR** (`crates/opencat-core/src/ir/draw_op.rs`) 已有 `DrawOp::RuntimeEffect { effect, uniforms, children, dst }`，配合 `EffectRef`、`RuntimeEffectChildRef::{Image, Picture, Shader}`、`builder.intern_effect/intern_bytes/push_child` 等基础设施。
- **Engine 执行端** (`crates/opencat-engine/src/executor/replay.rs:481`) 已能用 `skia_safe::RuntimeEffect::make_shader` 把 IR 中的 RuntimeEffect 真正绘制到 skia canvas（GLTransition / light_leak transition 已经走这条线）。
- **Web 执行端** (`crates/opencat-web/web/src/draw-ir.ts`) 已能从二进制 IR 解出 `RuntimeEffectSpec`，通过 canvaskit-wasm 的 `CanvasKit.RuntimeEffect.Make` 绘制。

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

— **跑不通**。canvas_api.js 里 `subTree.makeShader` / `CanvasKit.RuntimeEffect` / `effect.makeShaderWithChildren` / `paint.setShader` 全部不存在。RuntimeEffect IR 的所有"难活"在 Rust/skia 端都完成了，**仅缺脚本层的 wrapper 对象 + 一个 IR 中间变体 + 翻译逻辑**。

### 1.3 目标

让脚本能用与 CanvasKit-WASM 一致的 API 在 canvas 上跑任意 fragment SkSL effect，**且产出的最终 IR 与现有 GLTransition 路径完全同型**（同一个 `DrawOp::RuntimeEffect`，同一条 engine/web replay 路径）。

### 1.4 非目标

- **不实现 picture-as-shader-child**（即不让 `canvas.getSubTree().makeShader()` 工作）。Solaren 明确说"不需要 subtree，更普世"。Picture child 归到 follow-up。
- 不支持 gradient child / ColorFilter child / ImageFilter child。
- **`CK.RuntimeEffect.Make(sksl)` 只做最基本参数类型校验**（非空字符串），不做同步 SkSL 编译。
- SkSL 编译在 render 阶段 `intern_effect` 时由 engine/web 各自的 skia 后端隐式触发；编译失败时该 frame 的 ripple 不出现 + 控制台 warn，不影响其他渲染。
- 不支持 SkSL 中 `mat2/mat3/mat4/array` 之类的复合 uniforms。**uniforms 仅支持扁平 float 序列**（与 CanvasKit-WASM 的 `Float32Array` 兼容）。

## 2. 决策摘要

| 决策点 | 选择 |
|---|---|
| API 形状 | 完整对齐 CanvasKit-WASM |
| Child shader 类型 | Image only（picture 暂不实现） |
| SkSL 编译时机 | 延迟到 render 阶段；编译失败 → 该 op skip + warn |
| `Make()` 返回值 | 仅在 sksl 参数非字符串/空串时返回 null；编译错误不在此 surfacing |
| Uniforms 类型 | 扁平 `Float32Array` / `number[]`，按 SkSL 声明顺序消费 |
| IR 形态 | **新增 inline 中间变体 `DrawOp::ScriptRuntimeEffect`**；render helpers 翻译成现有 `DrawOp::RuntimeEffect` |
| Hash 计算 | 仅在 Rust render helpers 中算（`fxhash::hash64(sksl.as_bytes())` 或 `std::hash::DefaultHasher`）；JS 端、binding、recorder 都不传 hash |

## 3. 架构

### 3.1 数据流总览

```
JS script                MutationStore              主 DrawOpBuilder
─────────                ─────────────              ───────────────
RuntimeEffect.Make    ─┐ 仅 wrapper                
effect.makeShader     ─┤ 不触 native               
paint.setShader       ─┤                            
c.drawRect(rect,paint)─┴─> __canvas_runtime_effect_draw(...)
                                │
                                ▼
                  CanvasMutations.commands.push(
                      DrawOp::ScriptRuntimeEffect {
                          sksl: String,
                          uniforms_bytes: Vec<u8>,
                          children: Vec<RuntimeEffectChildRef::Image(...)>,
                          dst: Rect4,
                      })
                                │
                                ▼ apply_to_canvas (mutations.rs:281)
                       commands.extend (data inline — nothing lost)
                                │
                                ▼ display/build.rs
                       DrawScriptDisplayItem { commands, ... }
                                │
                                ▼ render::helpers::execute_draw_op
                       intern_effect (hash 现算) / intern_bytes / push_child
                                │
                                ▼
                       DrawOp::RuntimeEffect {
                           effect: 主 EffectId,
                           uniforms: 主 BytesRangeId,
                           children: 主 ChildRange,
                           dst,
                       }
                                │
                                ▼ engine/web replay
                       [现有 RuntimeEffect 执行路径，零改动]
```

### 3.2 关键不变量

1. **engine / web 的 IR 读取与 replay 零改动**：主 builder 输出的 `DrawOp::RuntimeEffect` 与 GLTransition 产出形态完全一致。`DrawOp::ScriptRuntimeEffect` 在 replay 时是 no-op（与 `DrawSubtreePicture` 同模式）。
2. **所有 RuntimeEffect 数据 inline 进 op 自身**，跟随 `commands.extend` 自然 propagate，不依赖任何并行表。
3. **多层 StyleMutations stack 合并** (`apply_canvas_mutation_stack`, `resolve.rs:931`) 不需要任何调整 —— `extend` 直接 clone 整个 op 包含 sksl/uniforms/children。
4. **SkSL hash 仅在 render helpers 翻译时算一次**；effect 表跨 frame / 跨 canvas / 跨 transition 按 hash 去重。

## 4. 详细设计

### 4.1 JS 侧 (`crates/opencat-core/src/script/runtime/canvas_api.js`)

新增对象/方法 (uniforms 仅支持扁平 float 序列；mat/array 不支持)：

```js
// ── RuntimeEffect ───────────────────────────────────────────
class RuntimeEffect {
    constructor(sksl) {
        this.__opencatRuntimeEffect = true;
        this._sksl = String(sksl);
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
    // uniforms: Float32Array | number[] of flat scalar floats.
    // mat2/mat3/mat4/array 不支持；如需要请等 follow-up.
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

// ── Image shader (extend existing ctx.getImage) ──────────────
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

function normalizeTileMode(v) {
    if (v === CanvasKit.TileMode.Clamp  || v === 'clamp')  return 'clamp';
    if (v === CanvasKit.TileMode.Repeat || v === 'repeat') return 'repeat';
    if (v === CanvasKit.TileMode.Mirror || v === 'mirror') return 'mirror';
    if (v === CanvasKit.TileMode.Decal  || v === 'decal')  return 'decal';
    throw new Error(`unsupported TileMode: ${v}`);
}

// ── TileMode enum ────────────────────────────────────────────
CanvasKit.TileMode = { Clamp:'clamp', Repeat:'repeat', Mirror:'mirror', Decal:'decal' };

// ── CanvasKit.RuntimeEffect.Make ─────────────────────────────
// Make() 只做参数类型校验，不做 SkSL 编译；编译错在 render 阶段 warn.
CanvasKit.RuntimeEffect = {
    Make(sksl) {
        if (typeof sksl !== 'string' || sksl.length === 0) return null;
        return new RuntimeEffect(sksl);
    },
};

// ── Paint.setShader (Paint class 已存在；ensurePaint 在 :192) ──
Paint.prototype.setShader = function(shader) {
    if (shader == null) { this._shader = null; return; }
    if (shader.__opencatShader !== 'runtime' && shader.__opencatShader !== 'image') {
        throw new Error('setShader expects a shader handle');
    }
    this._shader = shader;
};

// ── drawRect 路由 (在 makeCanvas() 内的 drawRect 方法里) ──────
drawRect(rect, paint) {
    const resolvedPaint = ensurePaint(paint);   // 复用 canvas_api.js:192 现有 helper
    const normalized = normalizeRect(rect);
    if (resolvedPaint._shader && resolvedPaint._shader.__opencatShader === 'runtime') {
        const sh = resolvedPaint._shader;
        __canvas_runtime_effect_draw(
            id,
            sh.effect._sksl,
            sh.uniforms,                       // Vec<f32>
            JSON.stringify(sh.children),       // [{__opencatShader:'image', assetId, tileX, tileY}, ...]
            normalized.x, normalized.y, normalized.width, normalized.height,
        );
        return this;
    }
    // 现有 fill_rect / stroke_rect 路径不变
    // ...
}
```

### 4.2 IR 变体 (`crates/opencat-core/src/ir/draw_op.rs`)

新增 enum 变体，**字段全部 inline**：

```rust
pub enum DrawOp {
    // ... 既有 ...

    /// Script-side 内联 runtime effect。`render::helpers::execute_draw_op`
    /// 翻译为标准 `DrawOp::RuntimeEffect`（intern 进主 builder）；engine/web
    /// replay 端看到它直接 no-op (同 DrawSubtreePicture 模式)。
    ScriptRuntimeEffect {
        sksl: String,
        uniforms_bytes: Vec<u8>,
        children: Vec<RuntimeEffectChildRef>,   // 当前只允许 Image 变体
        dst: Rect4,
    },
}
```

`Hash` impl 跟既有 `DrawOp::DrawSubtreePicture` 一样的写法 (拼接 sksl bytes、uniforms_bytes、children list bytes、dst bits)。

### 4.3 IR 编码 (`crates/opencat-core/src/ir/draw_encoding.rs`)

加 opcode 常量 + write/read case：

```rust
pub const SCRIPT_RUNTIME_EFFECT: u16 = 39;   // DRAW_SUBTREE_PICTURE 是 38
```

编码侧把 sksl/uniforms_bytes/children/dst 写进 payload，写出去（engine 端不读，web 端也不读但要 advance payload pointer）。Web `draw-ir.ts` 加一个 `OP_SCRIPT_RUNTIME_EFFECT = 39` 的 case，跟现有 `OP_DRAW_SUBTREE_PICTURE` 一样消费 payload 后 break (no-op)。

### 4.4 Binding 注册 (`crates/opencat-core/src/script/bindings.rs`)

```rust
$binding! { node $rec $id canvas_runtime_effect_draw (
    $id: &str,
    sksl: String,
    uniforms: Vec<f32>,
    children_json: String,
    dst_x: f32, dst_y: f32, dst_w: f32, dst_h: f32,
) {
    let children: Vec<ScriptChildSpec> =
        serde_json::from_str(&children_json)
            .map_err(|e| anyhow::anyhow!("children_json decode: {e}"))?;
    $rec.record_canvas_runtime_effect(
        $id, sksl, &uniforms, &children,
        Rect4 { x: dst_x, y: dst_y, width: dst_w, height: dst_h },
    );
}}
```

`ScriptChildSpec` serde 结构 (放在 `crates/opencat-core/src/script/bindings.rs` 或 `recorder/mod.rs`):

```rust
#[derive(serde::Deserialize)]
#[serde(tag = "__opencatShader")]
pub enum ScriptChildSpec {
    #[serde(rename = "image")]
    Image {
        #[serde(rename = "assetId")] asset_id: String,
        #[serde(rename = "tileX")]   tile_x: TileModeName,
        #[serde(rename = "tileY")]   tile_y: TileModeName,
    },
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TileModeName { Clamp, Repeat, Mirror, Decal }

impl ScriptChildSpec {
    pub fn to_ir_child_ref(&self) -> RuntimeEffectChildRef {
        match self {
            ScriptChildSpec::Image { asset_id, .. } => {
                // tile_x/tile_y 暂时丢弃：engine replay 端 image-shader 创建时
                // hard-coded Clamp/Clamp (replay.rs:516)，follow-up §6.1 透传
                RuntimeEffectChildRef::Image(ImageRef::Static {
                    asset_id: asset_id.clone(),
                })
            }
        }
    }
}
```

### 4.5 Recorder (`crates/opencat-core/src/script/recorder/mod.rs` + `store.rs`)

**Trait** (`mod.rs:106` 后新增方法)：

```rust
pub trait MutationRecorder {
    // ... 既有方法 ...

    fn record_draw_op(&mut self, id: &str, cmd: DrawOp);
    fn record_draw_picture(&mut self, target_id: &str, owner_id: &str, x: f32, y: f32);

    /// 新增
    fn record_canvas_runtime_effect(
        &mut self,
        id: &str,
        sksl: String,
        uniforms_f32: &[f32],
        children: &[ScriptChildSpec],
        dst: Rect4,
    );

    // ... 既有 ...
}
```

**Impl** (`recorder/store.rs`):

```rust
impl MutationRecorder for MutationStore {
    fn record_canvas_runtime_effect(
        &mut self,
        id: &str,
        sksl: String,
        uniforms_f32: &[f32],
        children: &[ScriptChildSpec],
        dst: Rect4,
    ) {
        let mut uniforms_bytes = Vec::with_capacity(uniforms_f32.len() * 4);
        for v in uniforms_f32 {
            uniforms_bytes.extend_from_slice(&v.to_ne_bytes());
        }
        let ir_children: Vec<RuntimeEffectChildRef> =
            children.iter().map(|c| c.to_ir_child_ref()).collect();
        self.canvas_entry(id).commands.push(DrawOp::ScriptRuntimeEffect {
            sksl,
            uniforms_bytes,
            children: ir_children,
            dst,
        });
    }
}
```

### 4.6 Render helpers (`crates/opencat-core/src/render/helpers.rs::execute_draw_op`)

加一个 match arm，签名不变：

```rust
fn execute_draw_op(
    b: &mut crate::render::builder::DrawOpBuilder,
    op: &DrawOp,
    state: &mut LocalPaintState,
) -> Result<(), RenderError> {
    match op {
        // ... 既有 arms ...

        DrawOp::ScriptRuntimeEffect { sksl, uniforms_bytes, children, dst } => {
            // Hash 现算 (跨 frame intern_effect 仍然按 hash 去重)
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            sksl.as_bytes().hash(&mut hasher);
            let hash = hasher.finish();

            let effect = b.intern_effect(hash, sksl);
            let uniforms = b.intern_bytes(uniforms_bytes);

            // children: 复制到主 builder 的 children Vec
            let child_start = b.children_len() as u32;
            for c in children {
                b.push_child(c.clone());
            }
            let new_children = ChildRange {
                start: child_start,
                len: children.len() as u32,
            };

            b.push(DrawOp::RuntimeEffect {
                effect, uniforms, children: new_children, dst: *dst,
            });
        }

        // ... 其他 arms 不变 ...
    }
    Ok(())
}
```

注：`DrawOpBuilder` 需要暴露 `children_len(&self) -> usize`；若不存在则 1 行 getter 即可。`intern_bytes` 已存在 (`builder.rs:124+`)。

### 4.7 Engine / Web replay no-op

**`crates/opencat-engine/src/executor/replay.rs:562`** —— `DrawOp::DrawSubtreePicture { .. } => Ok(()),` 旁边加：

```rust
DrawOp::ScriptRuntimeEffect { .. } => Ok(()),
```

**`crates/opencat-web/web/src/draw-ir.ts:419`** —— `OP_DRAW_SUBTREE_PICTURE` case 旁边加：

```ts
const OP_SCRIPT_RUNTIME_EFFECT = 39;

// 在主 switch:
case OP_SCRIPT_RUNTIME_EFFECT: {
  // payload: sksl(stringId u32) + uniforms_bytes_len u32 + uniforms_bytes
  //        + children_count u32 + children_bytes + dst (4 f32)
  // 完整 payload 必须 advance；no-op 后 break
  // 具体 advance 逻辑根据 §4.3 encoding 实现决定
  break;
}
```

实际渲染都已经在 §4.6 中通过 `DrawOp::RuntimeEffect` 完成；这里 engine/web 看到的 `ScriptRuntimeEffect` 早已被 helpers 翻译过，残留在 IR 中的只有 `DrawOp::RuntimeEffect`。**这两个 no-op 分支是防御性兜底**，正常路径不会触发（同 `DrawSubtreePicture` 在 engine `replay.rs:562` 是 no-op 但 helpers 已展开的逻辑）。

### 4.8 Fallback 路径

XML 中 `if (effect) { ... } else { c.drawPicture(sb,0,0); }` 这条 fallback：

- `Make()` 仅在 sksl 为空串/非字符串时返回 null，编译错不在此 surfacing
- 实际意义不大，但保留无害；可以视为"sksl 字段被脚本误配为空"的兜底
- 编译错误时，渲染阶段 `intern_effect` 仍会创建 effect entry；engine 的 `RuntimeEffect::make_for_shader(sksl, None)` 会返回 None；`media.runtime_effects[idx]` 实际是 `effect_idx >= media.runtime_effects.len()` 分支 (`replay.rs:488`)，整个 op 被静默 skip。Engine 端可以补一行 `eprintln!("RuntimeEffect compile failed: ...")` 给开发者反馈

## 5. XML 示例适配

原 XML s1-canvas 段把 `sb.makeShader` 改成 `ctx.getImage('s1-decor-img').makeShader(Clamp, Clamp)`：

```js
const CK = ctx.CanvasKit;
const c = ctx.getCanvasById('s1-canvas');
const imgShader = ctx.getImage('s1-decor-img')
    .makeShader(CK.TileMode.Clamp, CK.TileMode.Clamp);

const sksl = [/* ... 同原文 ... */].join('\n');

const effect = CK.RuntimeEffect.Make(sksl);
if (effect) {
    const p = ctx.currentFrame / ctx.sceneFrames;
    const shader = effect.makeShaderWithChildren(
        [p, 6.0, 0.035, 0.006],
        [imgShader]
    );
    const paint = new CK.Paint();
    paint.setShader(shader);
    c.drawRect(CK.LTRBRect(0, 0, 360, 480), paint);
}
// 不需要 else 分支：`<image id="s1-decor-img">` 作为 canvas 的 hidden child
// 默认会被正常渲染；编译失败时 ripple 不出现但底图仍在。
```

## 6. 已知缺口与 follow-up

### 6.1 Image child tile mode 未透传到 engine

`crates/opencat-engine/src/executor/replay.rs:516` 处 image-as-child shader 创建 hard-coded `(TileMode::Clamp, TileMode::Clamp)`。本 spec 的 binding 在 `ScriptChildSpec::Image` 收下脚本传的 tileX/tileY，但 `to_ir_child_ref` 暂时丢弃（`RuntimeEffectChildRef::Image(ImageRef)` 字段不含 tile mode）。视觉上对 ripple 例子无影响。完整透传需要扩展 `RuntimeEffectChildRef::Image` 字段，放 follow-up。

### 6.2 Picture-as-shader child

`RuntimeEffectChildRef::Picture(DrawOpRange)` 已存在，engine 的 `picture_shader_for_range` 也已工作。要让脚本能 `subTree.makeShader()`，需要在 helpers 翻译 ScriptRuntimeEffect 时把 hidden_subtree 当场录制成主 builder 的一段 ops 并产生 DrawOpRange。这条线**不在本 spec 范围**。

### 6.3 SkSL uniform 类型校验

当前完全不校验 uniforms 数量 / SkSL uniform 声明是否匹配。skia RuntimeEffect 会在编译/绑定时给出错误，体现为 replay 阶段静默 skip。Follow-up 可以在 JS 端解析 SkSL 头部的 `uniform xxx` 声明做提前校验。

### 6.4 Shader 直接 fill (无 RuntimeEffect 包装)

`paint.setShader(imageShader)` 直接画 rect（image shader 直接 fill）暂未实现。本 spec 只支持 RuntimeEffect shader 走 drawRect。Follow-up 可以让 image/gradient shader 也能直接 fill。

### 6.5 编译失败的 surfacing

`replay.rs:488` 用 `if effect_idx < media.runtime_effects.len()` 守卫，但编译失败的 effect 可能填进了 vec 仍 idx 在范围内 — 取决于 `prepare_runtime_effects` 的实现（待后续 verify 后补一条 log 行）。

## 7. 验收

### 7.1 单元/集成测试

- **Rust** `crates/opencat-core/src/script/recorder/store.rs`:
  - `record_canvas_runtime_effect_pushes_inline_op`：一次 record 后，CanvasMutations.commands 末尾是 `DrawOp::ScriptRuntimeEffect`，sksl/uniforms_bytes/children/dst 字段值正确。
- **Rust** `crates/opencat-core/src/ir/draw_op.rs`:
  - `script_runtime_effect_hash_is_stable`：相同字段两次构造 hash 相同。
  - `script_runtime_effect_distinct_from_runtime_effect`：与 `DrawOp::RuntimeEffect` 不相等。
- **Rust** `crates/opencat-core/src/render/helpers.rs`:
  - `execute_draw_op_translates_script_runtime_effect`：构造一个 ScriptRuntimeEffect op，过 execute_draw_op，断言主 builder 末尾 op 是 `DrawOp::RuntimeEffect`、`builder.effects` 长度 +1、`builder.byte_ranges` 长度 +1、`builder.children` 长度 += children.len。
  - `execute_draw_op_dedups_effects_across_calls`：两次 record 同一 sksl，主 builder effects 只增长 1。
- **Rust** `crates/opencat-engine` 已有 `runtime_effect_picture_child_samples_draw_op_range` 等测试；本 spec 不破坏现有用例。
- **集成** 在 `examples/` 加一个最小 ripple 例子（`ripple_canvas.rs`），渲染 1 帧 PNG，断言中心一圈像素与边角像素差异 > 阈值。

### 7.2 端到端

- `cargo run --example compare_transitions` 仍跑通。
- 修改后的 `json/profile-showcase.xml` 渲染 414 帧不报错；s1-canvas 区域在中间 frame 上肉眼可见水波纹。
- Web 端 `web/` 加载同 XML，canvas 上同样出现水波纹（验证 IR 跨端一致）。

### 7.3 性能

- 同一 SkSL 跨 frame / 跨 canvas 不重复编译（`intern_effect` 按 hash 去重已保证）。
- Hash 仅在 helpers 中算一次，热路径 cost 可忽略。

## 8. 实现顺序

每一步独立可验证（先 build & test 再继续）：

1. **IR 变体 + encoding** (§4.2, §4.3)
   - `DrawOp::ScriptRuntimeEffect` enum 变体 + Hash impl
   - `draw_encoding.rs` opcode 39 + write/read
   - 单元测试 §7.1 第 2-3 条
2. **Engine/Web no-op 兜底** (§4.7)
   - `replay.rs` arm + `draw-ir.ts` case
   - `cargo build -p opencat-engine` 跑通 + web 端 vite 跑通
3. **MutationRecorder trait + impl** (§4.5)
   - trait 方法 + MutationStore impl
   - 单元测试 §7.1 第 1 条
4. **Binding + ScriptChildSpec** (§4.4)
   - `bindings.rs` 注册 `canvas_runtime_effect_draw`
   - `ScriptChildSpec::to_ir_child_ref` 完整实现
5. **Render helpers 翻译** (§4.6)
   - `execute_draw_op` 加 arm
   - `DrawOpBuilder::children_len()` 如需新增
   - 单元测试 §7.1 第 4-5 条
6. **JS canvas_api 扩展** (§4.1)
   - `RuntimeEffect` / `TileMode` / `Paint.setShader` / `drawRect` 路由 / `image.makeShader`
7. **XML 适配 + 端到端** (§5)
   - 修改 `profile-showcase.xml` s1-canvas script
   - 跑 414 帧渲染 + web 端验证
   - 新增 `examples/ripple_canvas.rs`

## 9. v1 → v2 修订记录

**v2 (本版本)**: 应 code review 修订
- **Critical 1** — v1 在 `CanvasMutations` 加 4 个并行表 + `DrawScriptDisplayItem` 加 4 个并行字段 + `execute_draw_op` remap 增 src 参数。Review 揭示 `apply_to_canvas` (mutations.rs:281) / `ElementDrawSlot` (resolve/tree.rs:13) / `apply_canvas_mutation_stack` (resolve.rs:931) 都是 `Vec<DrawOp>` 流水线，并行表会在 `extend` 时丢失。v2 改为引入 `DrawOp::ScriptRuntimeEffect` inline 变体，所有数据塞进 op 字段，`commands.extend` 自然 propagate，链路零调整。
- **Critical 2** — v1 §1.4 与 §4.7 关于 `Make()` 返回 null 语义矛盾。v2 §1.4 / §2 / §4.1 统一：`Make()` 仅检查参数类型，编译延迟到 render 阶段，失败时 skip + warn。
- **Significant 3** — v2 §4.5 显式列出 `MutationRecorder` trait 必须加 `record_canvas_runtime_effect` 方法。
- **Significant 4** — v2 §4.4 补全 `ScriptChildSpec::to_ir_child_ref` 完整实现。
- **Significant 5** — v2 不再修改 `DrawScriptDisplayItem`，原 v1 列出的 7 处构造点 (`display/build.rs:89,238`, `analyze/compositor.rs:289,384,1067,1099`, `analyze/fingerprint/mod.rs:817`) 全部不需要动。
- **Moderate 6** — v2 §4.1 标注 `ensurePaint` 复用 `canvas_api.js:192` 既有 helper。
- **Moderate 7 + Minor** — v2 删除 JS 端 FNV-1a/BigInt/TextEncoder 全部复杂度，hash 计算下沉到 §4.6 render helpers 一处（DefaultHasher / fxhash 二选一）。
