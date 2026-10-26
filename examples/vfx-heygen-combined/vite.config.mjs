import { defineConfig } from 'vite';
import opencat from './opencat-plugin.mjs';

// 本示例是 create-opencat 模板的就地改造：入口保留在目录根部
// （examples/<dir>/<name>.jsx 的既有约定），资产引用路径无需改动。
export default defineConfig({
  plugins: [
    opencat({
      entry: 'vfx-heygen-combined.jsx',
      output: 'dist/video.mp4',
    }),
  ],
});
