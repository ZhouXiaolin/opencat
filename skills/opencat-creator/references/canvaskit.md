# CanvasKit 绘制

`<canvas>` 提供 CanvasKit 风格的即时绘制表面，用 `useCanvas` 挂每帧绘制。

**职责边界**：`useCanvas` 只做 CanvasKit 式绘制。tween 写 `useGSAP`，three.js 场景写 `useThree`。一个组件可以挂多个钩子；同一个 canvas 上既有绘制又有 tween 时，拆成两个钩子写。

绘制命令本身是标准 CanvasKit 语义，命名空间就是 `canvas.CK`（= `ctx.CanvasKit`），`new CK.Paint()`、`CK.XYWHRect`、`canvas.drawRect` 这些都照常用。这份文件只写 OpenCat 特有的部分：钩子入口、`paint()` 简写、hidden subtree、`ly` 效果内核、render target、文本测量表面，以及这个 CanvasKit 子集的已知限制。

---

## 入口

```jsx
import { useCanvas, useRef } from 'opencat/runtime';

function Field() {
  const c = useRef();
  useCanvas(c, (canvas, t, ctx) => {
    const CK = canvas.CK;                    // = ctx.CanvasKit
    canvas.clear(CK.parseColorString('#0b0f14'));

    const p = canvas.paint({ color: '#4ade80', style: 'fill' });
    canvas.drawRect(CK.XYWHRect(40, 40, 240, 120), p);
  });

  return <canvas id="field" ref={c} class="absolute inset-0 w-[1280px] h-[720px]" />;
}
```

规则：

- ref 来自 `useRef()`，必须指向一个 `<canvas>`，指错直接抛错
- `draw(canvas, t, ctx)` 每帧调用；`t` 是当前场景内秒
- 绘制逻辑必须对 `t` 纯：不用非种子随机、不依赖跨帧累积状态
- `useCanvasKit()` 在模块级取 CanvasKit 命名空间，用于构建可复用的 Paint / Path
- 不要省略 `canvas.clear()`——上一帧内容会残留

### `canvas.paint(opts)` 简写

比手写 `new CK.Paint()` + 一串 setter 短：

```js
const p = canvas.paint({
  color: '#ff8844',        // '#rrggbb' / [r,g,b,a] 0..1 / CK color array
  alpha: 0.8,
  style: 'fill',           // 'fill' | 'stroke'
  strokeWidth: 4,
  cap: 'round', join: 'round',
  dash: [12, 6], dashPhase: 0,
  antiAlias: true,
});
```

`paint()` 只认这些键。**要挂 shader 不能走这里**——用 `new CK.Paint()` 再 `setShader(...)`（见下面 `ly` 的例子）。

---

## 这个 CanvasKit 子集的限制

- `drawColor*()` 只支持 `CK.BlendMode.SrcOver`
- clip 只支持 `CK.ClipOp.Intersect`
- 自定义 typeface 不支持，`new CK.Font(null, size)` 是稳定写法
- image shader / picture shader 主要作 RuntimeEffect child，不要当普通 fill shader
- gradient shader child 没有 JS facade
- 复杂 color filter / image filter 未通过 JS facade 暴露

需要手写 SKSL 时仍可用 `CK.RuntimeEffect.Make(sksl)`，但优先用下面的 `ly` 内核。

---

## Hidden subtree

`<canvas>` 可以有子元素。它们不作为普通 DOM 显示，而是被记录成 hidden subtree，供 canvas 脚本显式采样或重放。用来做"HTML in Canvas"：

```jsx
<canvas id="surface" ref={c} class="w-[360px] h-[480px] rounded-[32px] overflow-hidden">
  <image id="hero" class="w-[360px] h-[480px] object-cover" url="./hero.png" />
  <div id="badge" class="absolute left-[18px] top-[18px] px-[12px] py-[8px] rounded-full bg-black/60">
    <text id="badge-text" class="text-[12px] font-semibold tracking-[2px] text-white">LIVE</text>
  </div>
</canvas>
```

```js
canvas.drawPicture(canvas.getSubTree(), 0, 0);
```

规则：`getSubTree()` 只对 `<canvas>` 有效；`drawPicture(handle, x?, y?)` 把 hidden subtree 作为 picture 重放；子元素也可被 `ctx.getNode(id)` 和 tween 驱动，变更会影响采样。

---

## `ly`：效果内核 DSL

`ly`（全局，也是 `ctx.ly`）用 JS 箭头函数写逐像素 / 顺序扫描效果。`ly` 是 OpenCat 自己的库，所以这里写清怎么写。

**`ly` 路径里没有 CanvasKit。** 不用 `Paint`、不用 `setShader`、不用 `drawRect`、不用 `getSubTree().makeShader`、不用 `TileMode`。内核画到画布用 `ly.draw`，跑进离屏目标用 `ly.surface(w,h).apply/.sweep`。后端（Skia RuntimeEffect 或 CPU 解释器）由引擎按内核能力自行选择，脚本不关心。

```js
const kernel = ly.pixel(
  (uv, u) => {
    const g = uv.x / u.w;
    return [g, g, g, 1];            // straight RGBA（0..1）
  },
  { uniforms: { w: 'float' } }
);

// 画到画布：rect 缺省 = 整画布
ly.draw(canvas, kernel, {
  rect: [0, 0, 1280, 720],          // [x, y, w, h]，可省
  uniforms: { w: 1280.0 },
  sources: [otherSurfaceOrCanvas],  // child，可省
});
```

### 参数约定

| 参数 | 含义 |
| --- | --- |
| 第一参（pixel 类） | `uv`：rect 本地像素坐标（`i + 0.5`），float2 |
| 第一参（scan 类） | `get(dx, dy)`：读本目标 in-progress 像素（clamp-to-edge），返回 float4 |
| 名为 `rect` | dst 的 `[x, y, w, h]`（画布坐标），float4 |
| 名为 `u` | uniforms 访问器 `u.<name>` |
| 其余参数 | child source（依 `opts.sources` 序）：`child.eval(float2)` → float4 |

### 执行动词

| 调用 | 语义 |
| --- | --- |
| `ly.draw(dst, kernel, opts)` | 把内核画到 canvas；`dst` 是 `useCanvas` 回调的 `c`。`opts: { rect?, uniforms?, sources? }` |
| `ly.surface(w, h)` | 离屏目标，**按尺寸 get-or-create** —— 同一句跨帧拿同一张，无 id |
| `target.apply(kernel, opts)` | pixel 内核逐像素重绘本目标；`opts: { uniforms?, sources? }` |
| `target.sweep(kernel, opts)` | scan 内核就地顺序扫描一遍；方向在 `spec.dir` |
| `ly.source(x)` | 把 canvas 句柄 / image 句柄 / surface 包成像素源，供 `opts.sources` |

`opts.sources` 直接收 canvas 句柄、`ly.surface(...)` 或 `ly.source(...)`，顺序 = child 声明序。surface 作 source 时引擎自动烘焙，脚本不碰帧号。

### spec

`{ uniforms: { name: 'type', ... }, backend: 'auto' }`。uniforms 按名声明，键序 = 打包序；`type` ∈ `float` / `float2` / `float3` / `float4`。`kind` 由函数决定：`ly.pixel` 逐像素纯函数，`ly.scan` 顺序扫描（另带 `dir: 'forward' | 'backward'`）。

`uniforms` 传参支持对象形式（按名）或平铺数组（按声明序）。

用了 `h01` / `imul` / `u32` / `i32` 这类整数精确语义的内核会走 CPU 后端（scan 类总是 CPU 单线程）；其余走 SKSL。`spec.backend` 可强制，但强制 `'sksl'` 时用了 CpuOnly op 会报错。

### 白名单（允许）

- 数值 / 布尔字面量、2-4 元数组字面量（构造向量）
- 算术 `+ - * / %`、比较、`&& || !`、三目、`if/else`、`let/const`、赋值（仅 let）
- 有界数值 `for` 循环：`for (let i = a; i < b; i++) { … }`（`i++` / `++i` / `i += 1`；`<` 或 `<=`）
- 顶层用户函数：`const f = (p) => …`（见下）
- stdlib：`abs sign floor ceil fract min max clamp pow sqrt exp exp2 log log2 sin cos tan asin acos atan atan2 mix smoothstep step length distance dot cross normalize`
- 整数 / 位操作内建：`byte(x)`（JS `x|0` 截断，可走 SKSL）、`float(x)`、`noise2(p)`（二维确定性值噪声，可走 SKSL）
- **仅 CPU 后端**：`u32` / `i32`、`imul`、`h01(a,b,c)`（确定性像素哈希 → [0,1)）。用到这几个就走 CPU 解释器
- `Math.*` 纯函数
- 向量 swizzle 读（`.x/.y/.z/.w/.rgba…`，不越界）

### 用户函数

lambda 顶层可声明函数，复用到多处：

```js
ly.pixel((uv, u) => {
  const hash = (p) => fract(sin(dot(p, [127.1, 311.7])) * 43758.5453);
  const fbm = (p) => {
    let v = 0; let a = 0.5;
    for (let o = 0; o < 5; o++) {
      v = v + a * noise(p);
      p = [0.8 * p.x - 0.6 * p.y, 0.6 * p.x + 0.8 * p.y] * 2.02;
      a = a * 0.5;
    }
    return v;
  };
  return [fbm(uv * 0.01), 0, 0, 1];
}, { uniforms: { } });
```

规则：

- **必须 `const`，必须顶层，必须先声明后使用**。`let` 绑定箭头、`function` 声明、`if` 块内声明都拒绝。
- **单态**：每个形参的类型在编译期唯一确定，由调用点与函数体共同推断（`dot(p, [127.1, 311.7])` 把 `p` 钉成 float2）。推断不出来会报错并列出候选。
- **无捕获**：函数体读不到 `uv` / `rect` / `u.*` / `child` / 外层局部。数据一律经形参进入。
- **不可递归**，也**不可互调**：调用图必须无环（先声明后使用天然保证）。
- 形参是可变槽，可重新赋值（上面 `fbm` 的 `p`）。
- 返回 `float` / `float2` / `float3` / `float4` / `bool`，所有 `return` 类型须一致。
- 函数名不得与 lambda 参数或 stdlib 内建同名。

### 拒绝（编译期报错）

嵌套 / 块内函数声明、闭包捕获、`while` / `for-in` / `for-of` / `do-while`、`try/catch`、裸代码块、对象字面量、字符串/模板串、位运算符（提示用 `imul`/`h01`/`byte`）、`**`（提示 `pow`）、`new`/`this`、`Math.random`、对 child/u/Math/builtin 的非白名单用法、`return` 后不可达语句。

**lambda 必须是源码内联的箭头函数。** 原生函数或被压缩的代码会被拒。

### 颜色域

lambda 返回 **straight** 色（codegen 自动预乘）。手写 SKSL 的 `main` 返回**预乘**色。若从 SKSL 移植，先把 rgb 除回 alpha。

---

## 离屏目标

`ly.surface(w, h)` 建一张离屏目标，**按尺寸 get-or-create** —— 同一句跨帧拿到同一张，无 id、无跨帧挂载点：

| 方法 | 语义 |
| --- | --- |
| `target.apply(kernel, opts)` | `ly.pixel` 内核逐像素**重绘**本 surface；`opts: { uniforms?, sources? }`；sources 可采样其它 surface、image 或 canvas 子树 |
| `target.sweep(kernel, opts)` | `ly.scan` 内核（首参 `get`）**就地**顺序扫描一遍；方向声明在内核 `spec.dir`，单线程 |

目标句柄直接作 `ly.draw` 的 `sources` 采样（session 级，不上 wire）。绘制路径需要跨线时引擎**自动烘焙**，脚本不碰帧号、不写 `bake`。

### 典型形态：mask + 双遍 chamfer 距离场

```js
// 1) seed：从另一离屏目标采样 alpha → 打包进本 surface
field.apply(ly.pixel(
  (uv, src) => {
    const c = src.eval(uv + [232, 430]);
    const m = byte(c.a * 255 + 0.5);
    const d = m > 120 ? 0 : 3000;
    return [m / 255, d % 256 / 255, floor(d / 256) / 255, 1];   // R=mask, G/B=dist lo/hi
  }
), { sources: [offscreen] });

// 2) 双遍 chamfer：get(dx,dy) 读 in-progress 缓冲，min 后写回
field.sweep(ly.scan(
  (get) => {
    const c = get(0, 0);
    const d = byte(c.g * 255 + 0.5) + byte(c.b * 255 + 0.5) * 256;
    const l = get(-1, 0), t = get(0, -1), tl = get(-1, -1), tr = get(1, -1);
    const m = min(min(d, byte(l.g * 255 + 0.5) + byte(l.b * 255 + 0.5) * 256 + 3),
                  min(byte(t.g * 255 + 0.5) + byte(t.b * 255 + 0.5) * 256 + 3,
                      min(byte(tl.g * 255 + 0.5) + byte(tl.b * 255 + 0.5) * 256 + 4,
                          byte(tr.g * 255 + 0.5) + byte(tr.b * 255 + 0.5) * 256 + 4)));
    return [c.r, m % 256 / 255, floor(m / 256) / 255, 1];
  }, { dir: 'forward' }));
field.sweep(ly.scan(/* 镜像：get(1,0)/get(0,1)/get(1,1)/get(-1,1) */, { dir: 'backward' }));

// 3) 逐帧绘制：内核以离屏目标直采 field
ly.draw(canvas, kernel, {
  rect: [x, y, w, h],
  uniforms: { t: ctx.current.time },
  sources: [field],
});
```

scan 类约定：首参 `get`，无 `uv`、无 source。

---

## 离屏画布与文本测量

`ly.surface(w, h)` 同时是一张真字体 2D 画布（栅格化）与测量面。测量不碰像素，所以 `ly.surface(1, 1)` 即可。

```js
const m = ly.surface(1, 1);
m.font = '500 100px Inter';
m.letterSpacing = '0px';
m.textBaseline = 'alphabetic';
const mm = m.measureText('Handgloves');
// mm.width / actualBoundingBoxLeft / Right / Ascent / Descent
// mm.fontBoundingBoxAscent / Descent
```

栅格化同 API：`clearRect` / `fillStyle` / `font` / `letterSpacing` / `fillText` / `save` / `restore` / `translate` / `scale(sx, 1)` / `getImageData`。

返回的是标准 Canvas `TextMetrics` 字段名（`actualBoundingBox*` 是字形墨迹范围，`fontBoundingBox*` 是字体行盒）。

支持的 2D 子集：`save` / `restore` / `translate` / `scale(sx, 1)` / `clearRect` / `fillStyle` / `font` / `letterSpacing` / `textBaseline='alphabetic'` / `measureText` / `fillText` / `getImageData`。`scale` 的 y 分量必须是 1，否则报错；没有 `rotate`。

`font` 只接受 `'<weight> <size>px <family>'` 简写，family 列表只解析**第一个**。

把昂贵测量集中到一个共享 surface（按名 get-or-create，重入免费）。

---

## 反模式

- 不要在 `useCanvas` 里写非种子随机——每帧不同。
- 不要依赖 canvas 实例属性跨帧累积状态。
- 不要把手写 SKSL 当默认路径，先想 `ly.pixel` / `ly.scan`。
- 不要在用 `byte()` / `h01()` 的 CPU lambda 上强制 `backend: 'sksl'`。
- 不要给 `ly` 内核路径引入 CanvasKit（`Paint` / `setShader` / `drawRect` / `getSubTree().makeShader` / `TileMode`）——用 `ly.draw` 与 `opts.sources`。
- 不要省略 `canvas.clear()`。
