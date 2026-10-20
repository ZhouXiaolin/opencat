import { defineConfig } from 'vite';
import opencat from './opencat-plugin.mjs';

// 本示例是 create-opencat 模板的就地改造：入口保留在目录根部
// （examples/<dir>/<name>.jsx 的既有约定），资产/字体相对路径无需改动。
// 溶解效果是纯 JS 逐像素移植（liangyi 退役），不依赖任何 npm 特效包。
export default defineConfig({
  plugins: [
    opencat({
      entry: 'index.jsx',
      output: 'dist/video.mp4',
    }),
  ],
});
