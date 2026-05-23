export type {
  CompositionInfo,
  JsonlFile,
  ResourceMeta,
} from './types';

export {
  blobCount,
  clearBlobs,
  getBlobBytes,
  getRenderer,
  getRendererOrThrow,
  initWasm,
  preloadAssets,
  setWasmBaseUrl,
} from './wasm';
export type { WebRendererInstance } from './wasm';

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
  registerVideoGlobals,
  setWorkerBaseUrl,
} from './video-decoder';
export type {
  VideoPreviewQuality,
  VideoSourceMeta,
} from './video-decoder';

export {
  clearCachedVideoFrames,
  getCachedVideoFrameRgba,
  injectVideoFramesForRender,
} from './video-frame-injector';

export {
  downloadMp4,
  exportMp4,
  exportPngFrame,
  initFFmpeg,
} from './exporter';
