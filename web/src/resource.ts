// ── Web Resource Loader ──
// Handles image loading.

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
  baseUrl?: string,
): Promise<void> {
  const promises = requests.images.map(async (url) => {
    const assetId = url;
    if (imageCache.has(assetId)) return;
    try {
      const fullUrl = baseUrl ? new URL(url, baseUrl).href : url;
      await loadImage(assetId, fullUrl);
    } catch (err) {
      console.warn(`[resource] Failed to load image: ${url}`, err);
    }
  });
  await Promise.all(promises);
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
