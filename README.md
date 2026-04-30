# OpenCat

> **Tailwind 写布局，GSAP 调动画，CanvasKit 画图形——底层是 Rust + Skia 原生渲染，不跑浏览器。**

OpenCat 是一个 **纯 Rust 原生** 的程序化视频合成引擎——**类似 [Remotion](https://remotion.dev) 用 React 写视频，但 OpenCat 不跑浏览器、不依赖 Node.js**。它把 Skia 渲染、Taffy 布局、QuickJS 脚本和 FFmpeg 编码焊成一个独立的 Rust 二进制。一行命令或几行 JSONL，就能生成一段带转场、动画、字幕和音频的 MP4 视频。

https://github.com/user-attachments/assets/dfe6e104-691a-4775-94a3-7cf7105f1e2e


---

## 设计动机

程序化视频生成并非新概念，但现有方案都存在结构性妥协：

- **SaaS 平台**（Runway、Pika 等）：黑盒渲染，不可控，批量生成成本高昂，无法嵌入自有管线。
- **浏览器方案**（Remotion 等）：复用 Web 生态是聪明的选择，但渲染层被浏览器框死——Puppeteer 截图意味着每次渲染都要拉起一个 Chromium 进程，GPU 调用受限，内存开销大，多核空转。
- **脚本拼凑**（Python + FFmpeg）：灵活但简陋，每一帧的编排、动效、字幕同步、音频混音都需要手搓，缺乏统一的抽象层。

**OpenCat 的思路**：提供一个声明式的 JSONL 格式作为视频的描述语言，底层用 Rust 原生渲染引擎直接输出 MP4。没有浏览器，没有运行时依赖，没有黑盒。AI 模型生成 JSONL，OpenCat 渲染成视频——接入成本最低，部署上限最高。

---

## 和 Remotion 对比

如果你听说过 [Remotion](https://remotion.dev)——那个用 React 写视频的工具——OpenCat 目标相似，但技术路线截然不同：

| 对比项 | Remotion | OpenCat ⬅️ |
|--------|----------|-----------|
| **渲染方式** | React → Puppeteer → headless Chrome 逐帧渲染后捕获 | **JSONL → Skia 原生渲染，无中间层** |
| **运行时** | Node.js + Chrome（约 500MB） | **无需安装任何运行时** |
| **描述格式** | React JSX / TypeScript | **JSONL（纯数据描述，AI / 程序化生成友好）** |
| **帧开销** | 每帧需完成完整浏览器布局→绘制→合成→像素读取 | **直接调 Skia 绘制到内存，无浏览器流程** |
| **确定性** | 受字体回退、GPU 驱动、抗锯齿算法等因素影响 | **动画函数式求值；软件后端输出跨机器字节一致** |
| **硬件利用** | 并行渲染需启动多个 Chrome 实例，内存随并发线性增长 | **原生多线程 + 直接 GPU 调用，资源用满为止** |
| **许可证** | 公司 4 人以上需付费 | **MIT** |
| **启动耗时** | 数秒（Node 初始化 + Chrome 启动） | **毫秒级** |

**本质差异**：Remotion 复用 Web 生态，在浏览器中渲染每一帧后捕获输出。OpenCat 提供了一套声明式 JSONL 格式，由 Rust 原生引擎直接渲染——同样的程序化视频生成目标，一个选择了 Web 兼容性，一个选择了原生性能与部署灵活性。

---

## 核心特色

| 能力 | 说明 |
|------|------|
| **熟悉的 API 表层** | Tailwind 式 className 写样式，GSAP 风格 API 做动效，CanvasKit 子集画图形——Web 开发者零学习成本切入 |
| **Rust 原生内核** | Skia 硬件加速渲染（macOS Metal / Windows OpenGL），Taffy Flexbox/Grid 布局引擎，QuickJS 轻量脚本运行时。没有浏览器，没有 Node.js，没有虚拟机 |
| **确定性动画** | 动画系统是函数式的：`value = f(frame)`。软件后端输出跨机器字节一致，适合 AI 训练数据管线 |
| **JSONL 交换格式** | 每行一个 JSON 对象，AI 易生成、易 diff、易版本控制 |
| **多场景时间线** | 多场景编排 + 转场 + 持久化叠加层（如字幕不随转场消失） |
| **内置转场** | fade / slide / wipe / clock_wipe / iris / light_leak，支持自定义 GLSL |
| **字幕引擎** | SRT 解析，场景内时间线对齐，跨场景持久化显示 |
| **音频混音** | 多轨道音频，场景级挂载，自动混音输出 |
| **GSAP 级动画能力** | 弹簧 / 贝塞尔 / 关键帧 / splitText 逐字逐词 / SVG 路径动画 / 路径变形 / 打字机效果 |
| **Canvas 绘图** | CanvasKit 子集 API，支持程序化矢量绘图与贴图渲染 |
| **Lucide 图标库** | 2000+ 开箱即用图标，写 kebab-case 名称即可引用 |

---

## 一句话看懂技术堆栈

```
JSONL ──→ Taffy 布局 ──→ Skia 渲染 ──→ FFmpeg 编码 → MP4
              ↑
         QuickJS 动画脚本
```

每一层都是独立的 Rust crate，没有隐形的运行时依赖。

---

## 快速开始

```bash
# 看个演示效果
cargo run --bin parse_json -- json/opencat-project-showcase-landscape.jsonl

# 或者用桌面播放器看实时预览（macOS / Windows）
cargo run --bin opencat-player -- path/to/input.jsonl

# 跑个 Hello World 示例
cargo run --example hello_world
```

---

## 一个例子抵过千言万语

```jsonl
{"type": "composition", "width": 390, "height": 844, "fps": 30, "frames": 150}
{"id": "scene1", "parentId": null, "type": "div", "className": "flex flex-col w-[390px] h-[844px] bg-slate-50 justify-center items-center gap-8", "duration": 150}
{"id": "title", "parentId": "scene1", "type": "text", "className": "text-[28px] font-bold text-slate-900", "text": "你好，OpenCat"}
{"id": "subtitle", "parentId": "scene1", "type": "text", "className": "text-[14px] text-slate-500", "text": "Rust 原生的动画合成"}
{"type": "script", "parentId": "scene1", "src": "ctx.fromTo('title',{opacity:0,y:30},{opacity:1,y:0,duration:20,ease:'spring.gentle'});ctx.fromTo('subtitle',{opacity:0},{opacity:1,duration:30,delay:10});"}
```

> **注意**：`className` 只做静态布局，动效全走脚本。别往 className 里塞 transform、transition、animate——那是脚本的地盘。

---

## 项目布局

```text
src/
├── lib.rs               # 公共 API 出口
├── backend/skia/        # Skia 渲染后端（Metal / GL / Software）
├── bin/
│   ├── opencat-player   # 桌面预览播放器
│   └── parse_json       # CLI 渲染器（JSONL → MP4）
├── codec/               # FFmpeg 编码 / 解码
├── element/             # 元素树类型和解析
├── jsonl/               # JSONL 解析 + 场景树构建
├── layout/              # Taffy Flexbox/Grid 布局
├── render.rs            # 高层渲染接口
├── resource/            # 资源管理（图片、媒体）
├── runtime/             # 运行时：JS 引擎、音频、缓存、合成器
├── scene/               # 场景图：节点、合成、缓动、转场
└── style.rs             # 样式系统（颜色、阴影、Tailwind 解析）
```

---

## 适合谁用？

- **AI 视频管线开发者**：让模型生成 JSONL 而不是直接操纵像素
- **程序化动画 / 动态设计作者**：需要确定性 GPU 加速渲染，不想要浏览器层
- **后端 / 基础设施团队**：需要可嵌入自有管线的轻量渲染方案，不想维护 Node.js + Chrome 环境
- **短视频批量生产者**：模板化视频生成，换数据 = 换 JSONL

## 正在进行中

- **可视化编辑器**：目前通过 JSONL 描述画面，GUI 编辑工具在规划中
- **实时预览**：命令行渲染 + 播放器的工作流已可用；设计时即时预览体验仍在迭代
- **更丰富的特效库**：核心转场和动效能力已就绪，更多预设效果持续开发中

---

## 当前限制

- 播放器目前仅支持 macOS 和 Windows
- 构建依赖本地图形库和 FFmpeg
- CanvasKit 是子集实现，非完整版
- 详细的 JSONL 格式参考请见 [`opencat.md`](opencat.md)

---

## 社区支持

- 问题反馈 / 讨论 → [Linux.do 社区](https://linux.do/)
- 发现 bug 或有想法 → [提 Issue](https://github.com/ZhouXiaolin/opencat/issues)

## 许可证

MIT License
