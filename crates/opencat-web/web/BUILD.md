# Build Process

From `crates/opencat-web/web`, the full build is:

```bash
cd crates/opencat-web/web
bun install
bun run build
```

`bun run build` runs three steps:

```bash
# 1. Compile Rust/WASM into crates/opencat-web/web/pkg
bun run build:wasm

# 2. Compile the JS bundle into dist/opencat.js + worker, and copy the wasm bridge into dist
bun run build:lib

# 3. Generate TypeScript declarations into dist/index.d.ts
bun run build:types
```

## Day-to-day (TS/front-end only)

No wasm rebuild needed:

```bash
bun run build:lib
bun run build:types
```

**Order matters:** `build:lib` clears `dist`, so `build:types` must run afterwards or consumers cannot find `dist/index.d.ts`.

## Using the local package in the root web preview

```bash
cd crates/opencat-web/web
bun link

cd ../../../web
bun link opencat.js
bun run build
```

## Pre-release checklist

From `crates/opencat-web/web`:

```bash
bun run build
npm pack --dry-run
```

Confirm the package contains:

- `dist/opencat.js`
- `dist/index.d.ts`
- `dist/opencat_web.js`
- `dist/opencat_web_bg.wasm`
- the worker files
