import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';

// 标准 web 工程：vite + solid-js；build 产出 dist/index.html + assets/*.js。
// 渲染 = `opencat render examples/liquid-brand-refraction/dist/index.html`。
export default defineConfig({
  plugins: [solid()],
  base: './',
  build: { outDir: 'dist', emptyOutDir: true, target: 'esnext' },
});
