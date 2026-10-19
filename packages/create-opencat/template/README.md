# OpenCat Video Project

JSX 写合成，tailwind 写样式，npm 组件照常装，一条命令出视频。

```sh
bun install       # 或 npm install
npm run build    # esbuild bundle + tailwind sidecar + opencat render → dist/video.mp4
npm run dev      # watch src/**/*.jsx 与 src/app.css，自动重编 sidecar（配合 opencat-see 预览）
```

## 前置条件

`opencat` 可执行文件需可用（二选一）：

- 设置环境变量 `OPENCAT_BIN` 指向编译产物（如 `F:/OpenCat/target/release/opencat.exe`）
- 或把 `target/release` 加入 `PATH`

引擎构建：在 OpenCat 仓库执行 `cargo build --release --bin opencat`。

## 标准工程流：npm 组件

构建分两步，全部由 `opencat-plugin.mjs` 自动完成：

1. **esbuild bundle**：入口连同 npm 依赖打成 `src/<入口名>.bundle.jsx`（构建
   产物，已 gitignore）。`lucide-react` 等真包在这里被解析折叠。
2. **opencat render 渲染 bundle**：bundle 与入口同目录，`base_dir`（资产/
   字体/sidecar 相对入口目录解析）约定不变。

React 系 import（`react` / `react/jsx-runtime`）被 alias 到 `react-shim.mjs`
垫片：组件树按 React 语义创建；`<svg>` 子树在垫片里序列化成 markup，走引擎
`<svg inline>` 通道（usvg 解析）。图标因此像真正的 HTML 工程那样工作：

```jsx
import { Home } from 'lucide-react';

<Home size={120} color="#ff3b30" class="absolute left-[60px] top-[60px]" />
```

`color` 沿 SVG 语义解析 `currentColor`（继承属性）；width/height/class 挂在
svg 根元素上参与布局与 CSS，svg 内部不进布局。可用 hooks 见 `opencat/runtime`
导出（`useRef` 等也可从 `react` 导入）。

## 工程约定

- **入口**：`src/index.jsx`（`vite.config.mjs` 里可改 `entry` / `output`）。
  引擎以入口文件所在目录为 `base_dir` 解析相对路径，所以字体、图片、音频等
  资产都放在 `src/` 下面（如 `src/assets/fonts/...`）。
- **样式**：`class` 一律写 tailwind 风格，`npm run build` 时由插件编译成
  `src/tailwind.css` sidecar（引擎按此约定路径读取；该文件是构建产物，已 gitignore）。
  手写 CSS 放 `src/app.css`，会与 utilities 同层叠。
- **动效**：元素是静态的，动效全部写在 `useGSAP` 钩子里；`meta` 导出声明
  尺寸/帧率/时长，命令行不可覆盖。
- **已知限制**：运行时动态拼接的类名（`left-[' + x + 'px]`）不会被 tailwind
  静态扫描，请改内联 `style`；引擎 CSS 子集不支持 `background` 简写，用
  `background-color` / `background-image`。

多文件：入口里 `import` 相对路径模块（`./scenes/intro.jsx`）即可，esbuild 会
一并打进 bundle，引擎按模块图逐帧重放。

## 格式参考

OpenCat 仓库 `skills/opencat-creator/references/jsx-format.md`。
