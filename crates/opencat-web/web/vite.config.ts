import { resolve } from 'node:path';
import { defineConfig } from 'vite';

export default defineConfig({
  build: {
    lib: {
      entry: {
        'opencat-web': resolve(__dirname, 'src/index.ts'),
        'workers/video-decode-worker': resolve(__dirname, 'src/workers/video-decode-worker.ts'),
      },
      formats: ['es'],
    },
    rollupOptions: {
      external: [
        '@webav/av-cliper',
        'canvaskit-wasm',
        /\/pkg\/opencat_web/,
      ],
    },
    target: 'esnext',
    minify: false,
    sourcemap: true,
  },
});
