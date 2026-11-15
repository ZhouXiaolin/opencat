import { installRasterCanvasReplay } from './raster-replay';
import { installOmpRunner } from './omp/omp-runner';

// Offscreen-canvas pixel readback for the stub canvas realm: the wasm native
// `__opencatRasterCanvas` hands its compiled OCIR frame to this hook. Install
// once at module load — the hook resolves CanvasKit lazily at call time.
installRasterCanvasReplay();
// @omp 团队执行体：Rust `WebOmpTeam` 经全局 `__ocOmpExecute` 派发（同上，
// 模块加载时装一次；Rust natives 在 dispatcher 安装期注册）。
installOmpRunner();

export type {
  AudioPlan,
  AudioPlanSegment,
  CompositionFile,
  CompositionInfo,
  ResourceMeta,
} from './types';
export { compositionFrameCount } from './types';

export {
  blobCount,
  clearAssetReader,
  clearBlobs,
  getAudioPlan,
  getBlobBytes,
  getRenderer,
  getRendererOrThrow,
  initWasm,
  openDesign,
  preloadAssets,
  setAssetReader,
  setWasmBaseUrl,
} from './wasm';
export { loadDefaultFontsIntoWasm } from './fonts';
export type { DefaultFontUrls } from './fonts';
export type { AssetReader, AssetReaderResult, WebRendererInstance } from './wasm';

export {
  renderEncodedDrawFrame,
  MediaValidationError,
  isEncodedDrawEnvelope,
  decodeDrawEnvelope,
  dumpEncodedFrameOps,
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
  prepareCatalogVideoSources,
  prefetchVideoFramesForRender,
} from './media/video-frame-injector';

export {
  createSurfaceWithFallback,
  getReusableSurface,
  releaseReusableSurface,
} from './media/surface';

export {
  downloadMp4,
  exportMp4,
  exportPngFrame,
  initFFmpeg,
} from './media/exporter';
export type { ExportProgressStage } from './media/exporter';

export { RenderSession } from './render-session';
export type { RenderSessionOptions } from './render-session';

export { ORACLE_METHODS } from './oracle-contract';
export type { OracleMethod, OracleRenderFrameResult, OracleRenderMp4Result } from './oracle-contract';

