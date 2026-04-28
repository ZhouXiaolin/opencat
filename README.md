# OpenCat

OpenCat 是一个用于构建和渲染基于时间轴的视觉作品的 Rust 工具包。
它结合了声明式场景模型、基于 JSONL 的交换格式、布局引擎以及用于生成动画输出的渲染流水线。

https://github.com/user-attachments/assets/dfe6e104-691a-4775-94a3-7cf7105f1e2e

该项目结构如下：

- `opencat` 库：核心 API，负责解析、场景构建、布局、渲染、音频和运行时支持。
- `opencat-player`：用于预览 JSONL 作品的桌面播放器。
- `parse_json`：一个小型的命令行工具（CLI），用于解析 JSONL 作品并将其渲染为 `out/parsed.mp4`。
- `examples/`：独立演示和实验。

## 主要概念

OpenCat 的设计围绕以下核心支柱展开：

- **声明式场景图 (Declarative Scene Graph)**: 使用节点树结构描述视觉元素。支持多种基本类型（如 `div`, `text`, `image`, `video`, `canvas` 等），并通过父子关系构建复杂的视觉组合。
- **基于 Taffy 的布局引擎**: 集成了 [Taffy](https://github.com/DioxusLabs/taffy) 布局库，支持类似 Tailwind CSS 的样式定义。这使得开发者可以使用熟悉的 Web 开发模式（如 Flexbox 和 Grid）来处理视频帧的自动布局。
- **确定性动画系统 (Deterministic Animation System)**: 动画是函数式的，每一帧的状态都由 `value = f(frame)` 唯一确定。通过 JavaScript 脚本驱动，支持弹簧动画 (Spring)、贝塞尔曲线 (Bezier) 和关键帧动画，确保渲染结果在不同环境下的完全一致性。
- **高性能渲染流水线**: 采用 [Skia](https://skia.org/) 作为底层渲染引擎。支持硬件加速、高效的资源缓存管理以及多线程渲染，能够快速生成高质量的 RGBA 帧或 MP4 视频。
- **JSONL 交换格式**: 提供了一种轻量级的 JSON Lines 格式用于描述整个作品。这种格式易于机器生成和解析，非常适合作为不同工具链之间的中间表示。
- **集成音频处理**: 内置音频引擎，支持多轨道音频合成、音视频同步渲染，并能自动处理场景过渡时的音频衔接。

## 项目布局

```text
src/lib.rs                 核心库
src/bin/opencat-player.rs  桌面播放器
src/bin/parse_json.rs      命令行渲染器
examples/                  示例程序
```

## 快速开始

生成展示视频：

```bash
cargo run --bin parse_json -- json/opencat-project-showcase-landscape.jsonl
```

运行桌面播放器：

```bash
cargo run --bin opencat-player -- path/to/input.jsonl
```

从命令行渲染 JSONL 文件：

```bash
cargo run --bin parse_json -- path/to/input.jsonl
```

运行示例：

```bash
cargo run --example hello_world
```

## 支持的元素类型

OpenCat 支持多种 JSONL 元素类型，对应不同的渲染组件：

| 类型 | HTML 等效项 | 特殊字段 |
|------|-----------------|----------------|
| `div` | `<div>` | — |
| `text` | `<span>` / `<p>` | `text`: 文本内容 |
| `image` | `<img>` | `query`: 图像搜索查询（1-4 个名词） |
| `icon` | Lucide 图标 | `icon`: 连字符格式（kebab-case）的图标名称 |
| `canvas` | `<canvas>` | 需要匹配的脚本 |
| `audio` | `<audio>` | `path` 或 `url` |
| `video` | `<video>` | — |
| `script` | 脚本节点 | `src`: 内联脚本代码; `path`: 外部 JS 文件路径 |
| `tl` | 时间轴节点 | 直接子级为定时场景；相邻对需要过渡 |
| `caption` | 字幕驱动文本节点 | `path`: 本地 SRT 文件 |

script支持canvaskit的子集以及对节点的属性修改

示例
```jsonl
{"type": "composition", "width": 390, "height": 844, "fps": 30, "frames": 150}
{"id": "scene1", "parentId": null, "type": "div", "className": "flex flex-col w-[390px] h-[844px] bg-slate-50 justify-center items-center gap-8", "duration": 150}
{"id": "title", "parentId": "scene1", "type": "text", "className": "text-[28px] font-bold text-slate-900", "text": "ctx.sequence"}
{"id": "subtitle", "parentId": "scene1", "type": "text", "className": "text-[14px] text-slate-500", "text": "heterogeneous choreography"}
{"id": "grid", "parentId": "scene1", "type": "div", "className": "flex flex-col gap-5"}
{"id": "row1", "parentId": "grid", "type": "div", "className": "flex gap-5"}
{"id": "block-1", "parentId": "row1", "type": "div", "className": "w-[90px] h-[90px] rounded-2xl bg-rose-500 shadow-lg"}
{"id": "block-2", "parentId": "row1", "type": "div", "className": "w-[90px] h-[90px] rounded-2xl bg-amber-500 shadow-lg"}
{"id": "block-3", "parentId": "row1", "type": "div", "className": "w-[90px] h-[90px] rounded-2xl bg-emerald-500 shadow-lg"}
{"id": "row2", "parentId": "grid", "type": "div", "className": "flex gap-5"}
{"id": "block-4", "parentId": "row2", "type": "div", "className": "w-[90px] h-[90px] rounded-2xl bg-sky-500 shadow-lg"}
{"id": "block-5", "parentId": "row2", "type": "div", "className": "w-[90px] h-[90px] rounded-2xl bg-indigo-500 shadow-lg"}
{"id": "block-6", "parentId": "row2", "type": "div", "className": "w-[90px] h-[90px] rounded-2xl bg-fuchsia-500 shadow-lg"}
{"type": "script", "parentId": "scene1", "src": "var tw = ctx.typewriter('ctx.sequence', { duration: 22, easing: 'linear', caret: '▍' }); var seq = ctx.sequence([{ from: { opacity: 0, translateY: -15 }, to: { opacity: 1, translateY: 0 }, duration: 12, easing: 'spring-gentle' }, { from: { opacity: 0 }, to: { opacity: 1 }, duration: 10, easing: 'linear' }, { from: { opacity: 0, translateX: -120 }, to: { opacity: 1, translateX: 0 }, duration: 18, easing: 'linear' }, { from: { opacity: 0, scale: 0.6 }, to: { opacity: 1, scale: 1 }, duration: 22, easing: 'spring-gentle', gap: -8 }, { from: { opacity: 0, translateY: -80 }, to: { opacity: 1, translateY: 0 }, duration: 20, easing: 'ease-out' }, { from: { opacity: 0, rotate: -180 }, to: { opacity: 1, rotate: 0 }, duration: 30, easing: 'spring-stiff', at: 32 }, { from: { opacity: 0, translateY: 80 }, to: { opacity: 1, translateY: 0 }, duration: 24, easing: 'spring-default' }, { from: { opacity: 0, scale: 0 }, to: { opacity: 1, scale: 1 }, duration: 28, easing: 'spring-wobbly' }]); ctx.getNode('title').text(tw.text).opacity(seq[0].opacity).translateY(seq[0].translateY); ctx.getNode('subtitle').opacity(seq[1].opacity); ctx.getNode('block-1').opacity(seq[2].opacity).translateX(seq[2].translateX); ctx.getNode('block-2').opacity(seq[3].opacity).scale(seq[3].scale); ctx.getNode('block-3').opacity(seq[4].opacity).translateY(seq[4].translateY); ctx.getNode('block-4').opacity(seq[5].opacity).rotate(seq[5].rotate); ctx.getNode('block-5').opacity(seq[6].opacity).translateY(seq[6].translateY); ctx.getNode('block-6').opacity(seq[7].opacity).scale(seq[7].scale);"}
```
## 注意事项

- 播放器目前支持 macOS 和 Windows。
- 部分构建目标需要本地图形库和 FFmpeg 依赖。
- JSONL 格式参考和相关设计说明请参阅 `opencat.md`。

## 社区支持
- Linux.do 社区：https://linux.do/

## 许可证
MIT License
