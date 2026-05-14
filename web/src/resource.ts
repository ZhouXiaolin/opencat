// ── Web Resource Loader ──
//
// 下载和元数据读取已经全部迁到 Rust 侧（参见 wasm.ts::preloadAssets）。
// 本模块只负责：
//   1. 从 wasm BlobStore 取字节 → CanvasKit 解码成 SkImage → 缓存
//   2. Lucide 图标 SVG 加载（轻量、独立流程）

import type { LoadedImage } from './types';
import { getBlobBytes } from './wasm';

let CanvasKit: any = null;

const imageCache = new Map<string, LoadedImage>();

export function setCanvasKit(ck: any): void {
  CanvasKit = ck;
}

export function getCachedImage(assetId: string): LoadedImage | undefined {
  return imageCache.get(assetId);
}

// ── Image decode (bytes 来自 Rust BlobStore) ──

/** 用 wasm BlobStore 里的字节 + CanvasKit 解出 SkImage，缓存到 imageCache。 */
export function decodeImageFromBlob(assetId: string): LoadedImage | null {
  if (!CanvasKit) throw new Error('CanvasKit not initialized');
  const cached = imageCache.get(assetId);
  if (cached) return cached;

  const bytes = getBlobBytes(assetId);
  if (!bytes) {
    console.warn(`[resource] no blob for assetId=${assetId}`);
    return null;
  }

  const ckImage = CanvasKit.MakeImageFromEncoded(bytes);
  if (!ckImage) {
    console.warn(`[resource] CanvasKit failed to decode: ${assetId}`);
    return null;
  }

  const loaded: LoadedImage = {
    path: assetId,
    ckImage,
    width: ckImage.width(),
    height: ckImage.height(),
  };
  imageCache.set(assetId, loaded);
  return loaded;
}

// ── Lucide icon SVGs (独立简单流程，不走 wasm) ──

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
