# hyperframes-launch

`create-opencat` 模板的就地改造示例：目录即 vite 工程，构建一条命令完成
tailwind sidecar 编译 + `opencat render`。

```sh
bun install      # 或 npm install
npm run build    # 重编 tailwind.css + 渲染 → dist/video.mp4（49.8s @ 30fps，1494 帧）
npm run dev      # watch *.jsx 变更重编 sidecar，配合 opencat-see 预览：
                 # opencat-see hyperframes-launch.jsx
```

- 入口：`hyperframes-launch.jsx`（保留在目录根部，`vite.config.mjs` 的 `entry` 指向它，
  仓库内既有脚本/文档引用的路径不变）
- `tailwind.css` 是构建产物但**入库**（与其它 examples 的 sidecar 约定一致，
  `opencat render` 直跑该目录时依赖它）
- 对齐历程与已知误差源见 `handoff.md`；模板本身见 `packages/create-opencat/`
