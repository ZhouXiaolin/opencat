import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';
import tailwindcss from '@tailwindcss/vite';

// 标准 web 工程：vite + solid + tailwind。build 产出标准 dist（index.html +
// assets/*.js + css link），渲染 = `opencat render dist/index.html --base-dir dist`。
// opencat 特有的只有两处：src/runtime.jsx 的 Composition 组件（组合根协议
// 属性）与 timeline/transition 标签组件（本例无转场，未用到）。
export default defineConfig({
  plugins: [solid(), tailwindcss()],
  base: './',
  build: { outDir: 'dist', emptyOutDir: true },
});
