# hyperframes-launch

`create-opencat` 模板的就地改造示例：目录即 vite 工程，构建一条命令完成
esbuild bundle（编译后的 CSS 嵌进 bundle）+ `opencat render`。

```sh
bun install      # 或 npm install
npm run build    # esbuild bundle + 渲染 → dist/video.mp4（49.8s @ 30fps，1494 帧）
```

预览：build 产出 `hyperframes-launch.bundle.jsx`（样式已嵌入），
`opencat-see hyperframes-launch.bundle.jsx`。

- 入口：`hyperframes-launch.jsx`（保留在目录根部，`vite.config.mjs` 的 `entry` 指向它，
  仓库内既有脚本/文档引用的路径不变）
- 对齐历程与已知误差源见 `handoff.md`；模板本身见 `packages/create-opencat/`
