import { defineConfig, type Plugin } from 'vite';
import { resolve } from 'path';
import type { Connect } from 'vite';
import fs from 'fs';

const CK_WASM_SRC = resolve(__dirname, 'node_modules/canvaskit-wasm/bin/full/canvaskit.wasm');
const CK_WASM_DEST = resolve(__dirname, 'public/canvaskit/canvaskit.wasm');

function ensureCanvaskitWasm(): void {
  const dir = resolve(__dirname, 'public/canvaskit');
  if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
  if (!fs.existsSync(CK_WASM_DEST)) {
    fs.copyFileSync(CK_WASM_SRC, CK_WASM_DEST);
  }
}

function serveStaticDirs(basePath: string, dirs: { mount: string; path: string; mime?: Record<string, string> }[]): Plugin {
  return {
    name: `serve-${basePath}`,
    configureServer(server) {
      for (const { mount, path: dirPath, mime } of dirs) {
        server.middlewares.use(mount, ((req: any, res: any, next: any) => {
          const url = decodeURIComponent(req.url || '');
          const filePath = resolve(dirPath, url.slice(1));
          if (filePath.startsWith(dirPath) && fs.existsSync(filePath)) {
            const stat = fs.statSync(filePath);
            if (stat.isFile()) {
              const ext = filePath.split('.').pop()?.toLowerCase();
              const defaultMime: Record<string, string> = {
                jsonl: 'application/jsonl',
                json: 'application/json',
                svg: 'image/svg+xml',
              };
              res.writeHead(200, {
                'Content-Type': mime?.[ext || ''] || defaultMime[ext || ''] || 'application/octet-stream',
                'Content-Length': stat.size,
                'Access-Control-Allow-Origin': '*',
              });
              fs.createReadStream(filePath).pipe(res);
              return;
            }
            if (stat.isDirectory()) {
              const files = fs.readdirSync(filePath).filter(f => f.endsWith('.jsonl') || f.endsWith('.svg'));
              res.writeHead(200, { 'Content-Type': 'text/html' });
              const base = req.url.replace(/\/$/, '');
              res.end(files.map(f => `<a href="${base}/${f}">${f}</a>`).join('\n'));
              return;
            }
          }
          next();
        }) as Connect.NextHandleFunction);
      }
    },
  };
}

export default defineConfig({
  root: __dirname,
  server: {
    fs: {
      allow: [__dirname, resolve(__dirname, '..')],
    },
  },
  plugins: [
    serveStaticDirs('static', [
      { mount: '/json', path: resolve(__dirname, '..', 'json') },
      { mount: '/lucide', path: resolve(__dirname, '..', 'lucide') },
    ]),
    {
      name: 'canvaskit-wasm',
      buildStart() {
        ensureCanvaskitWasm();
      },
      configureServer(server) {
        ensureCanvaskitWasm();
      },
    },
  ],
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
  optimizeDeps: {
    exclude: ['opencat-web'],
  },
});
