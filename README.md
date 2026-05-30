# OpenCat

> 用 XML 写视频，Rust 渲染，一行命令出 MP4。

https://github.com/user-attachments/assets/dfe6e104-691a-4775-94a3-7cf7105f1e2e

---

## 30 秒上手

```xml
<opencat width="390" height="844" fps="30" frames="60">
  <div id="scene1" class="flex items-center justify-center w-full h-full bg-white">
    <text id="title" class="text-[48px] font-bold">Hello OpenCat</text>
  </div>
  <script>
    ctx.fromTo('title', {opacity: 0, y: 30}, {opacity: 1, y: 0, duration: 20, ease: 'spring.gentle'});
  </script>
</opencat>
```

```bash
cargo run --bin opencat -- hello.xml
```

就这样，MP4 已经生成了。

---

## 为什么选 OpenCat

| | OpenCat | Remotion / HyperFrames |
|---|---------|------------------------|
| **渲染方式** | Rust 原生 GPU (Skia) | Chrome snapshot |
| **渲染速度** | 10x | 基准 |
| **部署环境** | 任意环境/浏览器 | 需要 Chromium |
| **动画能力** | 自研 GSAP 兼容 API（覆盖 80%+） | 可直接用 GSAP / anime.js |
| **浏览器渲染** | WASM + CanvasKit | 原生 |
| **确定性输出** | ✅ 跨机器一致 | ❌ |
| **AI 友好** | XML/JSONL 声明式，JSONL可以流式生成 | JSX/HTML，较复杂 |

**一句话总结**：Remotion 复用了 Web 生态，但 Chrome snapshot 方案有天然缺陷——GPU 调用受限、内存开销大、帧率上不去、部署要带 Chromium。OpenCat 原生调用 GPU 和 FFmpeg，性能上限更高。

---

## 能做什么

### 批量生成视频

AI 模型直接输出 XML/JSONL。无需操纵像素，无需 Puppeteer。

### 复杂动效，声明式写法

```js
// 弹簧物理
ctx.fromTo('title', {opacity: 0, y: 30}, {opacity: 1, y: 0, duration: 20, ease: 'spring.gentle'});

// SVG 路径动画
ctx.to('rocket', {path: 'M100 360 C400 80 880 640 1180 360', duration: 120, ease: 'ease-in-out'});

// 逐字动画
ctx.from(ctx.splitText('title', {type: 'chars'}), {opacity: 0, y: 20, stagger: 2, ease: 'spring.wobbly'});

// GSAP 风格 Timeline
ctx.timeline({defaults: {duration: 18, ease: 'spring.gentle'}})
  .from('title', {opacity: 0, y: 30})
  .from('subtitle', {opacity: 0, y: 18}, '-=8');
```

### 多场景时间线 + 转场

```xml
<tl id="main-tl">
  <div id="scene1" duration="120">...</div>
  <transition from="scene1" to="scene2" effect="fade" duration="18" />
  <div id="scene2" duration="120">...</div>
</tl>
```

内置转场：fade / slide / wipe / clock_wipe / iris / light_leak，支持自定义 GLSL。

### 自定义着色器

实现 HTML in Canvas 提案，`<canvas>` 节点支持子树纹理采样 + SkSL RuntimeEffect：

```js
var effect = CK.RuntimeEffect.Make(sksl);
var shader = effect.makeShaderWithChildren([progress], [subtreeShader]);
canvas.drawRect(rect, paint);
```

### 浏览器内实时渲染

```ts
import { initWasm, preloadAssets, getRendererOrThrow } from 'opencat-web';

await initWasm();
const catalog = await preloadAssets(xmlContent);
const renderer = getRendererOrThrow();
renderer.build_frame(xmlContent, frameNumber, canvas, catalog);
```

WebAssembly + CanvasKit，无需服务器，浏览器内完成渲染和导出。

### 更多能力

- **Tailwind 式布局**：`class="flex items-center justify-center gap-4"`
- **GSAP 级动画**：弹簧 / 贝塞尔 / 关键帧 / morphSVG / 路径动画
- **音频混音**：多轨道，场景级挂载，自动混音输出
- **字幕引擎**：SRT 解析，跨场景持久化显示
- **CanvasKit 绑定**：Paint / Path / RuntimeEffect (SkSL)
- **Lucide 图标库**：2000+ 开箱即用
- **确定性渲染**：`value = f(frame)`，跨机器输出一致，适合 AI 训练数据管线

---

## 技术架构

```
XML ──→ Taffy 布局 ──→ Skia 渲染 ──→ 编码 → MP4 / Canvas
              ↑
         QuickJS 动画脚本
```

### 渲染路径

```
CLI:   XML → Rust + Skia (GPU) + FFmpeg → MP4 文件
Web:   XML → WASM + CanvasKit (WebGL) + WebCodecs → Canvas / MP4 导出
```

### 关键技术

- **三阶段增量渲染**：Resolve → Layout → Display，基于 Merkle Tree 指纹跳过无需变动的子树
- **Scene Snapshot 缓存**：整帧无变化时直接复用上一帧，零计算开销
- **双渲染管道**：Rust 输出紧凑二进制 IR（38 种 DrawOp），JS 侧 CanvasKit 解释器执行
- **HTML in Canvas**：实现 `<canvas>` 子树纹理采样提案，支持 SkSL RuntimeEffect 自定义着色器

### Crate 结构

```
opencat
├── crates/
│   ├── opencat-core/      # 核心：布局(Taffy)、文字(cosmic-text)、字体
│   ├── opencat-engine/    # 引擎：Skia 渲染、FFmpeg 编码、QuickJS 脚本
│   ├── opencat-web/       # WASM 目标：浏览器端渲染 + 导出
│   └── opencat/           # CLI：opencat (渲染) + opencat-see (预览)
├── web/                   # Web 应用：视频编辑器
└── json/                  # 示例 XML 文件
```

---

## 快速开始

### CLI

```bash
# 渲染为 MP4
cargo run --bin opencat -- json/profile-showcase.xml

# 桌面播放器实时预览（macOS / Windows）
cargo run --bin opencat-see -- path/to/input.xml

# Hello World
cargo run --example hello_world
```

### Web (WASM)

```bash
cd crates/opencat-web
npm run build
```

```ts
import { initWasm, preloadAssets, getRendererOrThrow, exportMp4, downloadMp4 } from 'opencat-web';

await initWasm();
const catalog = await preloadAssets(xmlContent);
const renderer = getRendererOrThrow();
renderer.build_frame(xmlContent, frameNumber, canvas, catalog);

await exportMp4({ /* ... */ });
downloadMp4();
```

> 浏览器需要 `Cross-Origin-Isolated` 环境（COOP/COEP headers）。

---

## 构建依赖

### CLI (原生)

- Rust toolchain (edition 2024)
- FFmpeg 开发库
- 本地图形后端：macOS (Metal) / Windows (OpenGL)

### Web (WASM)

- Rust toolchain + `wasm-pack`
- Node.js / Bun
- `npm run build` — 一条命令完成 wasm-pack → Vite build → 类型声明

---

## 适合谁用

- **AI 视频管线**：模型输出 XML，引擎渲染视频，接入成本最低
- **Web 应用**：浏览器内集成视频编辑/渲染，无需服务器
- **程序化动画**：确定性 GPU 加速渲染，跨机器一致
- **批量生产**：模板化视频，换数据 = 换 XML

---

## 参考文档

- [XML 格式参考](opencat-creator/references/opencat.md)
- [转场效果](opencat-creator/references/transitions.md)
- [动画系统](opencat-creator/references/animations.md)
- [Canvas API](opencat-creator/references/canvaskit.md)
- [文字动画](opencat-creator/references/text-animations.md)

---

## 社区

- 问题反馈 / 讨论 → [Linux.do 社区](https://linux.do/)
- Bug / 功能建议 → [提 Issue](https://github.com/ZhouXiaolin/opencat/issues)

## 许可证

MIT License
