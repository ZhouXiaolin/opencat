import { defineConfig } from 'vite';
import opencat from './opencat-plugin.mjs';

// corpus 对齐案：CASE=solid|gradient|noise|blend|noise-anim 选入口，
// vite build → bundle + 自动 opencat render 出 dist/<case>.mp4。
const caseName = process.env.CASE || 'solid';
export default defineConfig({
  plugins: [
    opencat({
      entry: `cases/${caseName}.jsx`,
      output: `dist/${caseName}.mp4`,
    }),
  ],
});
