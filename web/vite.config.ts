import fs from 'node:fs';
import { resolve } from 'node:path';
import { defineConfig } from 'vite';
import type { Connect, Plugin } from 'vite';

const ROOT_DIR = __dirname;
const REPO_ROOT = resolve(ROOT_DIR, '..');
const NODE_MODULES = resolve(ROOT_DIR, 'node_modules');

const CANVASKIT_WASM_SRC = resolve(NODE_MODULES, 'canvaskit-wasm/bin/full/canvaskit.wasm');
const CANVASKIT_WASM_DEST = resolve(ROOT_DIR, 'public/canvaskit/canvaskit.wasm');

const OPENCAT_WEB_DIST = resolve(REPO_ROOT, 'crates/opencat-web/web/dist');
const WASM_PUBLIC_DIR = resolve(ROOT_DIR, 'public/wasm');

type StaticDir = {
  mount: string;
  path: string;
  mime?: Record<string, string>;
};

const DEFAULT_STATIC_MIME: Record<string, string> = {
  jsonl: 'application/jsonl',
  json: 'application/json',
  svg: 'image/svg+xml',
};

function ensureDir(dir: string): void {
  if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
}

function ensureCanvaskitWasm(): void {
  ensureDir(resolve(ROOT_DIR, 'public/canvaskit'));
  if (!fs.existsSync(CANVASKIT_WASM_DEST)) fs.copyFileSync(CANVASKIT_WASM_SRC, CANVASKIT_WASM_DEST);
}

function ensureOpencatWasm(): void {
  ensureDir(WASM_PUBLIC_DIR);
  const files = [
    'opencat_web.js',
    'opencat_web_bg.wasm',
    'workers/video-decode-worker.js',
  ];

  for (const f of files) {
    const src = resolve(OPENCAT_WEB_DIST, f);
    if (!fs.existsSync(src)) continue;
    const destDir = resolve(WASM_PUBLIC_DIR, f.includes('/') ? f.substring(0, f.lastIndexOf('/')) : '');
    if (destDir) ensureDir(destDir);
    const dest = resolve(WASM_PUBLIC_DIR, f);
    if (!fs.existsSync(dest) || fs.statSync(src).mtimeMs > fs.statSync(dest).mtimeMs) {
      fs.copyFileSync(src, dest);
    }
  }
}

function assetPlugin(name: string, ensureAssets: () => void): Plugin {
  return {
    name,
    buildStart() {
      ensureAssets();
    },
    configureServer() {
      ensureAssets();
    },
  };
}

function serveStaticDirs(dirs: StaticDir[]): Plugin {
  return {
    name: 'static-dirs',
    configureServer(server) {
      for (const dir of dirs) {
        const handler: Connect.NextHandleFunction = (req, res, next) => {
          const url = decodeURIComponent(req.url || '');
          const filePath = resolve(dir.path, url.slice(1));
          if (!filePath.startsWith(dir.path) || !fs.existsSync(filePath)) {
            next();
            return;
          }

          const stat = fs.statSync(filePath);
          if (stat.isFile()) {
            const ext = filePath.split('.').pop()?.toLowerCase() || '';
            res.writeHead(200, {
              'Content-Type': dir.mime?.[ext] || DEFAULT_STATIC_MIME[ext] || 'application/octet-stream',
              'Content-Length': stat.size,
              'Access-Control-Allow-Origin': '*',
            });
            fs.createReadStream(filePath).pipe(res);
            return;
          }

          if (stat.isDirectory()) {
            const files = fs.readdirSync(filePath).filter((file) => file.endsWith('.jsonl') || file.endsWith('.svg'));
            const base = (req.url || '').replace(/\/$/, '');
            res.writeHead(200, { 'Content-Type': 'text/html' });
            res.end(files.map((file) => `<a href="${base}/${file}">${file}</a>`).join('\n'));
            return;
          }

          next();
        };

        server.middlewares.use(dir.mount, handler);
      }
    },
  };
}

export default defineConfig({
  root: ROOT_DIR,
  server: {
    headers: {
      'Cross-Origin-Opener-Policy': 'same-origin',
      'Cross-Origin-Embedder-Policy': 'require-corp',
    },
    fs: {
      allow: [ROOT_DIR, REPO_ROOT],
    },
    proxy: {
      '/assets-proxy': {
        target: 'http://127.0.0.1:8080',
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/assets-proxy/, ''),
      },
    },
  },
  plugins: [
    serveStaticDirs([
      { mount: '/json', path: resolve(REPO_ROOT, 'json') },
      { mount: '/lucide', path: resolve(REPO_ROOT, 'lucide') },
      { mount: '/fixtures', path: resolve(REPO_ROOT, 'testsupport/fixtures') },
    ]),
    assetPlugin('canvaskit-wasm', ensureCanvaskitWasm),
    assetPlugin('opencat-wasm', ensureOpencatWasm),
  ],
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },

});
