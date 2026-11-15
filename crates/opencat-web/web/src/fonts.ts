import type { WebRendererInstance } from './wasm';

const DEFAULT_SANS_URL = '/fonts/NotoSansSC-Regular.otf';
const DEFAULT_EMOJI_URL = '/fonts/NotoColorEmoji.ttf';

// html 默认字体面——与 engine `html_default_font_faces()` 同表（Inter
// 400-900 + JetBrains Mono 三面）。engine 在页面声明这些族且无 @font-face
// 时注入；web 宿主对齐同一语义。多余的 face 只被声明同族的页面选中，
// 不改变其他族的 fallback 行为；单个 face 拉取失败按 web 资源语义降级
// （warn + 跳过），不阻断开稿。
const HTML_DEFAULT_FONT_URLS = [
  '/fonts/Inter-400.ttf',
  '/fonts/Inter-500.ttf',
  '/fonts/Inter-600.ttf',
  '/fonts/Inter-700.ttf',
  '/fonts/Inter-800.ttf',
  '/fonts/Inter-900.ttf',
  '/fonts/JetBrainsMono-Regular.ttf',
  '/fonts/JetBrainsMono-500.ttf',
  '/fonts/JetBrainsMono-Bold.ttf',
];

export type DefaultFontUrls = {
  sansSc?: string;
  colorEmoji?: string;
};

/**
 * Fetch default Noto fonts and load them into the WASM fontdb.
 * Must run after `initWasm()` and before the first `build_frame_ir`.
 */
export async function loadDefaultFontsIntoWasm(
  renderer: WebRendererInstance,
  urls: DefaultFontUrls = {},
): Promise<void> {
  const sansUrl = urls.sansSc ?? DEFAULT_SANS_URL;
  const emojiUrl = urls.colorEmoji ?? DEFAULT_EMOJI_URL;

  const [sans, emoji] = await Promise.all([
    fetch(sansUrl).then((r) => {
      if (!r.ok) throw new Error(`Failed to fetch font ${sansUrl}: ${r.status}`);
      return r.arrayBuffer();
    }),
    fetch(emojiUrl).then((r) => {
      if (!r.ok) throw new Error(`Failed to fetch font ${emojiUrl}: ${r.status}`);
      return r.arrayBuffer();
    }),
  ]);

  renderer.load_default_fonts(new Uint8Array(sans), new Uint8Array(emoji));

  await Promise.all(
    HTML_DEFAULT_FONT_URLS.map(async (url) => {
      try {
        const buf = await fetch(url).then((r) => {
          if (!r.ok) throw new Error(`${r.status}`);
          return r.arrayBuffer();
        });
        renderer.load_font_data(new Uint8Array(buf));
      } catch (e) {
        console.warn(`[opencat] default font face unavailable, skipping: ${url} (${e})`);
      }
    }),
  );
}