# hyperframes-launch

`create-opencat` 模板的就地改造示例：目录即 vite 工程，构建一条命令完成
esbuild bundle（编译后的 CSS 嵌进 bundle）+ `opencat render`。

```sh
bun install      # 或 npm install
npm run build    # → dist/index.bundle.jsx + dist/video.mp4（49.8s @ 30fps，1494 帧）
```

构建产物全部在 `dist/`（已 gitignore），源码目录不放编译结果。

- 入口：`index.jsx`（`export function main()` 约定，`vite.config.mjs` 的 `entry`
  指向它）。子场景在 `scenes/`，共享工具在 `shared.jsx`，esbuild 按模块图打包。
- 预览：`opencat view dist/index.bundle.jsx --base-dir .`
  （bundle 不在入口目录，用 `--base-dir` 把资产/字体相对路径指回工程根目录）
- 单帧：`opencat render dist/index.bundle.jsx out.png 625 --base-dir .`

对齐历程与已知误差源见仓库 `docs/`；模板本身见 `packages/create-opencat/`。
