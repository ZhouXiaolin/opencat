export type {
  CompositionFile,
  CompositionInfo,
  ResourceMeta,
} from './types';
export { compositionFrameCount } from './types';

export {
  blobCount,
  clearAssetReader,
  clearBlobs,
  getBlobBytes,
  getRenderer,
  getRendererOrThrow,
  initWasm,
  preloadAssets,
  setAssetReader,
  setWasmBaseUrl,
} from './wasm';
export { loadDefaultFontsIntoWasm } from './fonts';
export type { DefaultFontUrls } from './fonts';
export type { AssetReader, AssetReaderResult, WebRendererInstance } from './wasm';

export {
  renderEncodedDrawFrame,
} from './draw-ir';
export type {
  EncodedDrawFrame,
} from './draw-ir';

export {
  clearVideoCache,
  getDecodedFrameRgba,
  getDecodedVideoFrame,
  getVideoDimensions,
  getVideoDurationSecs,
  prepareVideoSource,
  prefetchDecodedVideoFrame,
  registerVideoGlobals,
  setWorkerBaseUrl,
} from './media/video-decoder';
export type {
  VideoPreviewQuality,
  VideoSourceMeta,
} from './media/video-decoder';

export {
  clearCachedVideoFrames,
  getCachedVideoFrameRgba,
  injectVideoFramesForRender,
  prefetchVideoFramesForRender,
} from './media/video-frame-injector';

export {
  createSurfaceWithFallback,
  downloadMp4,
  exportMp4,
  exportPngFrame,
  initFFmpeg,
} from './media/exporter';
export type { ExportProgressStage } from './media/exporter';
