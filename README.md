# OpenCat

> JSONL 描述画面，Rust 渲染视频。CLI 一行出 MP4，浏览器几行出 Canvas。

OpenCat 是一个 **Rust 原生** 的程序化视频合成引擎。它用 JSONL 格式声明视频内容，底层由 Skia 渲染、Taffy 布局、QuickJS 脚本驱动。支持两条路径：

- **CLI**：`cargo run --bin opencat` — Rust + FFmpeg 直接输出 MP4
- **Web**：`import { initWasm } from 'opencat-web'` — 浏览器内 CanvasKit + WebCodecs 渲染导出

https://github.com/user-attachments/assets/dfe6e104-691a-4775-94a3-7cf7105f1e2e

---

## 设计动机

程序化视频生成并非新概念，但现有方案都存在结构性妥协：

- **SaaS 平台**（Runway、Pika 等）：黑盒渲染，不可控，批量生成成本高昂，无法嵌入自有管线。
- **浏览器方案**（Remotion 等）：复用 Web 生态是聪明的选择，但渲染层被浏览器框死——Puppeteer 截图意味着每次渲染都要拉起一个 Chromium 进程，GPU 调用受限，内存开销大，多核空转。
- **脚本拼凑**（Python + FFmpeg）：灵活但简陋，每一帧的编排、动效、字幕同步、音频混音都需要手搓，缺乏统一的抽象层。

**OpenCat 的思路**：提供一个声明式的 JSONL 格式作为视频的描述语言，底层用 Rust 原生渲染引擎直接输出 MP4。没有浏览器，没有运行时依赖，没有黑盒。AI 模型生成 JSONL，OpenCat 渲染成视频——接入成本最低，部署上限最高。

---

## 核心特色

| 能力 | 说明 |
|------|------|
| **熟悉的 API 表层** | Tailwind 式 className 写样式，GSAP 风格 API 做动效，CanvasKit 子集画图形 |
| **Rust 原生内核** | Skia 硬件加速渲染（macOS Metal / Windows OpenGL），Taffy Flexbox/Grid 布局引擎 |
| **浏览器内渲染** | 通过 WASM + CanvasKit 在浏览器中实时渲染，WebCodecs 视频解码，WebAV MP4 导出 |
| **确定性动画** | 动画系统是函数式的：`value = f(frame)`。跨机器输出一致，适合 AI 训练数据管线 |
| **JSONL 交换格式** | 每行一个 JSON 对象，AI 易生成、易 diff、易版本控制 |
| **多场景时间线** | 多场景编排 + 转场 + 持久化叠加层 |
| **内置转场** | fade / slide / wipe / clock_wipe / iris / light_leak，支持自定义 GLSL |
| **字幕引擎** | SRT 解析，场景内时间线对齐，跨场景持久化显示 |
| **音频混音** | 多轨道音频，场景级挂载，自动混音输出 |
| **GSAP 级动画** | 弹簧 / 贝塞尔 / 关键帧 / splitText 逐字逐词 / SVG 路径动画 |
| **Lucide 图标库** | 2000+ 开箱即用图标 |

---

## 技术架构

```
JSONL ──→ Taffy 布局 ──→ Skia 渲染 ──→ 编码 → MP4 / Canvas
               ↑
          QuickJS 动画脚本
```

### Crate 结构

```
opencat
├── crates/
│   ├── opencat-core/      # 核心：布局(Taffy)、文字(cosmic-text)、字体、元数据
│   ├── opencat-engine/    # 引擎：Skia 渲染、FFmpeg 编码、QuickJS 脚本、资源请求
│   ├── opencat-web/       # WASM 目标：编译为 wasm32 供浏览器端使用
│   │   ├── src/           # Rust → WASM 桥接层
│   │   └── web/           # TypeScript 前端包 (opencat-web npm 包)
│   └── opencat/           # CLI：opencat (渲染) + opencat-see (桌面播放器)
├── web/                   # Web 应用：基于 opencat-web 的视频编辑器
├── json/                  # JSONL 组合文件
└── examples/              # Rust 示例
```

### 渲染路径

```
CLI:   JSONL → Rust + Skia (GPU) + FFmpeg → MP4 文件
Web:   JSONL → WASM + CanvasKit (WebGL) + WebCodecs → Canvas / MP4 导出
```

---

## 快速开始

### CLI 渲染

```bash
# 渲染为 MP4
cargo run --bin opencat -- json/opencat-project-showcase-landscape.jsonl

# 桌面播放器实时预览（macOS / Windows）
cargo run --bin opencat-see -- path/to/input.jsonl

# 跑个 Hello World
cargo run --example hello_world
```

### Web 渲染

```bash
# 1. 构建 opencat-web
cd crates/opencat-web/web
npm run build

# 2. 在你的 Web 项目中使用
cd your-project
npm install @webav/av-cliper web-demuxer canvaskit-wasm
npm link ../crates/opencat-web/web
```

```ts
import {
  initWasm,
  initCanvasKitWasm,
  setWasmBaseUrl,
  setWorkerBaseUrl,
  preloadAssets,
  getRendererOrThrow,
  exportMp4,
  downloadMp4,
} from 'opencat-web';
import CanvasKitInit from 'canvaskit-wasm/full';

// 初始化
setWasmBaseUrl('/wasm/');
setWorkerBaseUrl('/wasm/');
await initWasm();

const CK = await CanvasKitInit({ locateFile: (f) => '/canvaskit/' + f });
(globalThis as any).__canvasKit = CK;
initCanvasKitWasm();

// 加载合成 + 渲染
const catalog = await preloadAssets(jsonlContent);
const renderer = getRendererOrThrow();
renderer.build_frame(jsonlContent, frameNumber, canvas, catalog);

// 导出 MP4
await exportMp4({ /* ... */ });
downloadMp4();
```

> **注意**：浏览器需要 `Cross-Origin-Isolated` 环境（COOP/COEP headers）才能使用 SharedArrayBuffer。

---

## 一个例子

```jsonl
{"type": "composition", "width": 390, "height": 844, "fps": 30, "frames": 150}
{"id": "scene1", "parentId": null, "type": "div", "className": "flex flex-col w-[390px] h-[844px] bg-slate-50 justify-center items-center gap-8", "duration": 150}
{"id": "title", "parentId": "scene1", "type": "text", "className": "text-[28px] font-bold text-slate-900", "text": "你好，OpenCat"}
{"id": "subtitle", "parentId": "scene1", "type": "text", "className": "text-[14px] text-slate-500", "text": "Rust 原生的动画合成"}
{"type": "script", "parentId": "scene1", "src": "ctx.fromTo('title',{opacity:0,y:30},{opacity:1,y:0,duration:20,ease:'spring.gentle'});ctx.fromTo('subtitle',{opacity:0},{opacity:1,duration:30,delay:10});"}
```

---

## opencat-web API

| 函数 | 说明 |
|------|------|
| `initWasm(baseUrl?)` | 初始化 Rust WASM 模块 |
| `setWasmBaseUrl(url)` | 设置 WASM 文件基础路径 |
| `setWorkerBaseUrl(url)` | 设置视频解码 Worker 基础路径 |
| `initCanvasKitWasm()` | 注册 CanvasKit 到 WASM 侧 |
| `preloadAssets(jsonl)` | 下载资源，返回资源目录 JSON |
| `getRendererOrThrow()` | 获取 WebRenderer 实例 |
| `renderer.build_frame(...)` | 渲染单帧到 CanvasKit 画布 |
| `exportMp4(options)` | 导出 MP4 |
| `exportPngFrame(options)` | 导出当前帧为 PNG |
| `downloadMp4()` | 触发浏览器下载 |

### 类型

```ts
interface CompositionInfo { width: number; height: number; fps: number; frames: number }
interface JsonlFile { name: string; path: string }
interface ResourceMeta { kind: 'image' | 'video' | 'audio' | 'icon'; width?; height?; durationSecs? }
```

---

## 构建依赖

### CLI (原生)

- Rust toolchain (edition 2024)
- FFmpeg 开发库
- 本地图形后端：macOS 需要 Xcode / Metal，Windows 需要 OpenGL

### Web (WASM)

- Rust toolchain + `wasm-pack`
- Node.js / Bun
- `npm run build` — 一条命令完成 wasm-pack → Vite build → 类型声明生成

---

## 适合谁用？

- **AI 视频管线开发者**：让模型生成 JSONL 而不是直接操纵像素
- **Web 应用开发者**：在浏览器中集成视频编辑/渲染能力
- **程序化动画 / 动态设计作者**：需要确定性 GPU 加速渲染
- **后端 / 基础设施团队**：需要可嵌入自有管线的轻量渲染方案
- **短视频批量生产者**：模板化视频生成，换数据 = 换 JSONL

---

## 当前限制

- 桌面播放器仅支持 macOS 和 Windows
- CLI 构建依赖本地图形库和 FFmpeg
- CanvasKit 是子集实现，非完整版
- 详细的 JSONL 格式参考请见 [`opencat.md`](opencat.md)

---

## 社区

- 问题反馈 / 讨论 → [Linux.do 社区](https://linux.do/)
- Bug / 功能建议 → [提 Issue](https://github.com/ZhouXiaolin/opencat/issues)

## 许可证

MIT License
