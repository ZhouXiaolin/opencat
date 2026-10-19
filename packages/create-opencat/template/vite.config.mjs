import { defineConfig } from 'vite';
import opencat from './opencat-plugin.mjs';

export default defineConfig({
  plugins: [
    opencat({
      // 合成入口（引擎以该文件所在目录为 base_dir 解析相对资源）
      entry: 'src/index.jsx',
      // 渲染输出；.mp4 出视频，改 .png 可出单帧
      output: 'dist/video.mp4',
    }),
  ],
});
