// ── Web Resource Loader ──
// Handles image loading with AssetId-aware URL resolution.

import type { LoadedImage, ResourceRequests } from './types';

let CanvasKit: any = null;

// ── Resource Cache ──

const imageCache = new Map<string, LoadedImage>();

export function setCanvasKit(ck: any): void {
  CanvasKit = ck;
}

export function getCachedImage(assetId: string): LoadedImage | undefined {
  return imageCache.get(assetId);
}

// ── AssetId Resolution ──

/** Extract a real HTTP fetch URL from an AssetId string. */
function resolveFetchUrl(assetId: string): string | null {
  if (assetId.startsWith('url:')) return assetId.slice(4);
  if (assetId.startsWith('audio:url:')) return assetId.slice(10);
  if (assetId.startsWith('openverse:')) return null;
  return assetId;
}

/** Parse an openverse:... AssetId back to search parameters. */
function parseOpenverseAssetId(assetId: string): {
  q: string;
  count: number;
  aspect_ratio?: string;
} {
  const m = assetId.match(
    /^openverse:q=(.+?);count=(\d+)(?:;aspect_ratio=(.+))?$/,
  );
  if (!m) throw new Error(`Invalid openverse assetId: ${assetId}`);
  return {
    q: decodeURIComponent(m[1]),
    count: parseInt(m[2]),
    aspect_ratio: m[3] || undefined,
  };
}

/** Resolve an openverse:... AssetId by calling the Openverse API. */
async function queryOpenverse(assetId: string): Promise<string> {
  const params = parseOpenverseAssetId(assetId);
  const url = new URL('https://api.openverse.org/v1/images/');
  url.searchParams.set('q', params.q);
  url.searchParams.set('page_size', String(params.count));
  if (params.aspect_ratio) {
    url.searchParams.set('aspect_ratio', params.aspect_ratio);
  }
  const resp = await fetch(url.toString());
  if (!resp.ok) throw new Error(`Openverse HTTP ${resp.status}`);
  const data = await resp.json();
  const img = data.results?.find((r: any) => r.url || r.thumbnail);
  if (!img) throw new Error(`Openverse: no result for "${params.q}"`);
  return img.url || img.thumbnail;
}

// ── Image Loading ──

export async function loadImage(assetId: string, urlOrPath: string): Promise<LoadedImage> {
  const cached = imageCache.get(assetId);
  if (cached) return cached;

  const resp = await fetch(urlOrPath);
  const blob = await resp.blob();
  const imageBitmap = await createImageBitmap(blob);

  if (!CanvasKit) throw new Error('CanvasKit not initialized');

  const ckImage = CanvasKit.MakeImageFromEncoded(
    new Uint8Array(await blob.arrayBuffer()),
  );

  if (!ckImage) throw new Error(`Failed to decode image: ${urlOrPath}`);

  const loaded: LoadedImage = {
    path: urlOrPath,
    ckImage,
    width: ckImage.width(),
    height: ckImage.height(),
  };
  imageCache.set(assetId, loaded);
  return loaded;
}

export async function loadImages(
  requests: ResourceRequests,
  onProgress?: (loaded: number, total: number) => void,
  baseUrl?: string,
): Promise<void> {
  const total = requests.images.length;
  let loaded = 0;
  onProgress?.(loaded, total);

  for (const assetId of requests.images) {
    if (imageCache.has(assetId)) {
      loaded++;
      onProgress?.(loaded, total);
      continue;
    }
    try {
      let fetchUrl = resolveFetchUrl(assetId);
      if (fetchUrl === null && assetId.startsWith('openverse:')) {
        fetchUrl = await queryOpenverse(assetId);
      }
      if (!fetchUrl) {
        console.warn(`[resource] Cannot resolve: ${assetId}`);
        loaded++;
        onProgress?.(loaded, total);
        continue;
      }
      const fullUrl = baseUrl ? new URL(fetchUrl, baseUrl).href : fetchUrl;
      await loadImage(assetId, fullUrl);
    } catch (err) {
      console.warn(`[resource] Failed to load: ${assetId}`, err);
    } finally {
      loaded++;
      onProgress?.(loaded, total);
    }
  }
}

// ── Icon Loading (Lucide SVGs) ──

const iconSvgCache = new Map<string, string>();

export async function loadLucideIcon(name: string): Promise<string | null> {
  const cached = iconSvgCache.get(name);
  if (cached !== undefined) return cached;

  try {
    const resp = await fetch(`/lucide/${name}.svg`);
    if (!resp.ok) {
      iconSvgCache.set(name, '');
      return null;
    }
    const svg = await resp.text();
    iconSvgCache.set(name, svg);
    return svg;
  } catch {
    iconSvgCache.set(name, '');
    return null;
  }
}

export function getCachedIconSvg(name: string): string | undefined {
  return iconSvgCache.get(name);
}

// ── Cleanup ──

export function clearResourceCache(): void {
  imageCache.clear();
  iconSvgCache.clear();
}
