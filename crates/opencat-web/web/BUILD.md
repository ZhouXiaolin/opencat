# Build Process

进入 `crates/opencat-web/web` 后，完整构建流程：

```bash
cd crates/opencat-web/web
bun install
bun run build
```

`bun run build` 实际做三步：

```bash
# 1. 编 Rust/WASM，输出到 crates/opencat-web/web/pkg
bun run build:wasm

# 2. 编 JS 包，输出 dist/opencat.js 和 worker，并把 wasm bridge 拷进 dist
bun run build:lib

# 3. 生成 TypeScript 声明，输出 dist/index.d.ts
bun run build:types
```

## 日常开发（仅改 TS/前端代码）

不需要重新编 wasm，可以跑：

```bash
bun run build:lib
bun run build:types
```

**注意顺序：** `build:lib` 会清空 `dist`，所以改完后必须再跑 `build:types`，否则消费项目会找不到 `dist/index.d.ts`。

## 在根目录 web 预览项目中使用本地包

```bash
cd crates/opencat-web/web
bun link

cd ../../../web
bun link opencat.js
bun run build
```

## 发布前检查

在 `crates/opencat-web/web` 里跑：

```bash
bun run build
npm pack --dry-run
```

确认以下文件都在包里：

- `dist/opencat.js`
- `dist/index.d.ts`
- `dist/opencat_web.js`
- `dist/opencat_web_bg.wasm`
- worker 文件
