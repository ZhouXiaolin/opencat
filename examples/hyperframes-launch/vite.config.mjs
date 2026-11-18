import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';
import tailwindcss from '@tailwindcss/vite';

// 标准 web 工程：vite + solid + tailwind。产物是 dist/index.html +
// assets/*.js，opencat 的 render 按标准 HTML 导入。扩展面只有
// src/runtime.jsx：Composition、Timeline(<tl>)、Transition(<transition>)。
export default defineConfig({
  plugins: [solid(), tailwindcss()],
  base: './',
  build: { outDir: 'dist', emptyOutDir: true },
});
