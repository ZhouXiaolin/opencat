# OpenCat

用 JSX 描述画面、布局与动画，Rust + Skia 渲染，FFmpeg 编码成 MP4。一条命令出片，
不需要浏览器，不需要截图，也不需要显卡。

## 构建

前提：

- Rust 工具链（edition 2024）。
- FFmpeg 开发库（编码 MP4 用，由 `pkg-config` 自动查找）：

```sh
sudo apt install \
  libavcodec-dev libavformat-dev libavutil-dev \
  libavfilter-dev libswscale-dev libswresample-dev
```

编译 CLI：

```sh
cargo build --release -p opencat
```

产物是 `target/release/opencat`。

## 快速开始

```sh
opencat render hello.jsx hello.mp4        # 整片
opencat render hello.jsx hello.png 45     # 第 45 帧
opencat view hello.jsx                    # 原生窗口预览
```

`render` 的第二个参数可以是文件，也可以是帧号；输出路径以 `.png` 结尾时只渲染一帧。
输入文件里的相对资源路径按该文件所在目录解析，产物在别处时加 `--base-dir`。

## 直接渲染 hyperframes 工程

opencat 也能直接吃 hyperframes 的原始 `.html` 工程。不改文件，不先打包：

```sh
opencat render project/index.html out.mp4
opencat render project/index.html frame.png 30 --base-dir project
```

进度：

- **已支持**：`data-composition-*` 组合协议、`[start, end)` 可见窗、嵌套
  composition、CSS 组合器级联、`<video>` 抽帧、远程脚本与字体的抓取和缓存、
  canvas / SKSL / three。
- **动画**：页面里的 GSAP timeline 被包成 paused + 按帧 seek，结果只取决于帧号。
- **对齐**：k3-promo 已通过验收；hyperframes-launch、HF-heygen-stripe、
  inspector-launch、vfx-heygen-combined 都能整片渲染。与参考视频的差异像素
  比例（p8）：stripe 4.23%、inspector 7.94%；vfx-heygen-combined 的 2D 段
  0.004–0.07，3D 折射段仍偏高。
- **缺口**：`::before` / `::after`、`mask-image`、`@keyframes` 尚未绘制；
  `offsetWidth` 用布局探针近似。

## JSX 示例

### 1. 最小合成

```jsx
import { useGSAP } from 'opencat/runtime';

// 尺寸、帧率、时长只在这里声明
export const meta = { width: 1920, height: 1080, fps: 30, duration: 4 };

export default function Hello() {
  useGSAP((ctx) => {
    ctx.from('title', { opacity: 0, y: 60, duration: 0.8, ease: 'power3.out' });
    ctx.from('subtitle', { opacity: 0, y: 24, duration: 0.8, delay: 0.3, ease: 'power2.out' });
  });

  return (
    <div id="root" class="relative w-[1920px] h-[1080px] bg-slate-950 overflow-hidden">
      <text id="title" class="absolute left-0 top-[430px] w-[1920px] text-center text-[120px] font-bold text-white">
        Hello OpenCat
      </text>
      <text id="subtitle" class="absolute left-0 top-[600px] w-[1920px] text-center text-[40px] text-slate-400">
        JSX 进，MP4 出
      </text>
    </div>
  );
}
```

`class` 里的 tailwind 类名会编译成普通 CSS，交给内置的 CSS 引擎排版。
动画写在 `useGSAP` 里，时间单位是秒。

### 2. 多场景与转场

```jsx
function Scene1() {
  return (
    <div id="scene1" duration="4" class="absolute inset-0 bg-slate-900">
      <text class="absolute left-[160px] top-[460px] text-[96px] text-white">第一幕</text>
    </div>
  );
}

function Scene2() {
  return (
    <div id="scene2" duration="4" class="absolute inset-0 bg-indigo-950">
      <text class="absolute left-[160px] top-[460px] text-[96px] text-white">第二幕</text>
    </div>
  );
}

export default function Main() {
  return (
    <tl id="main-tl" class="absolute inset-0">
      <Scene1 />
      <transition from="scene1" to="scene2" effect="fade" duration="0.6" />
      <Scene2 />
    </tl>
  );
}
```

场景根节点的 `duration` 决定它在时间轴上占多长。场景内的时钟从 0 开始，
场景内的 `useGSAP` 不需要自己算起始时间。内置转场有 fade、slide、wipe、
clock_wipe、iris、light_leak。

### 3. 组件与 props

```jsx
function Card({ bg, title, children }) {
  return (
    <div class={'w-[420px] rounded-2xl p-8 ' + bg}>
      <text class="text-[28px] font-bold text-slate-900">{title}</text>
      {children}
    </div>
  );
}

<Card bg="bg-white" title="卡片标题">
  <text class="mt-3 text-[20px] text-slate-500">props.children 从调用处组合。</text>
</Card>
```

任何 JSX 函数都是组件，`props.children` 组合调用处的标记。

### 更多能力

- **GSAP**：官方 `gsap` 包在引擎内运行，时间轴按帧求值，结果只与 `t` 有关。
- **Canvas 与 SKSL**：`<canvas>` 节点接受 CanvasKit API，自定义效果用
  `CK.RuntimeEffect.Make(SKSL)`。
- **3D**：官方 `three` 包直接运行，`WebGLRenderer` 的输出合成进画面。
- **音频与字幕**：多轨混音自动合成，SRT 字幕跨场景显示。

## Docker 镜像

镜像里运行纯 CPU 渲染（ANGLE + SwiftShader），不需要显卡与 X11。

准备与构建：

```sh
git submodule update --init --recursive
sh docker/fetch-prebuilt.sh                 # 下载 Skia 与 V8 预编译包
DOCKER_BUILDKIT=1 docker build -t opencat:bookworm .
```

运行：

```sh
docker run --rm \
  -v "$PWD":/work -w /work \
  opencat:bookworm \
  render hello.jsx out.mp4
```

构建参数、预编译包的来源与镜像内的环境变量写在 `docker/README.md`。

## CLI

```sh
opencat render <input.jsx> [output] [frame] [--base-dir DIR] [--frames SPEC] [--jobs N]
opencat view   <input.jsx> [--base-dir DIR] [--scale S]
```

| 参数 | 说明 |
|---|---|
| `output` | 默认 `out/<stem>.mp4`；以 `.png` 结尾时渲染单帧 |
| `frame` | 单帧渲染的帧号 |
| `--frames` | 渲染 PNG 序列，`start:end[:step]` 或逗号列表 |
| `--jobs` | 分块多进程渲染后再拼接，`N` 为进程数 |
| `--base-dir` | 相对资源路径的根目录，默认是输入文件所在目录 |
| `--scale` | `view` 的预览比例，(0, 1] |

## 许可

MIT
