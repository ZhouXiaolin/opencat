import type { WebRendererInstance } from './wasm';

const DEFAULT_SANS_URL = '/fonts/NotoSansSC-Regular.otf';
const DEFAULT_EMOJI_URL = '/fonts/NotoColorEmoji.ttf';

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
}