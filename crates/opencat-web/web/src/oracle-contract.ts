/// `window.__opencatOracle` 合同单一 owner。
///
/// 生产方是 oracle harness 页（`crates/opencat-web-oracle/web/test-oracle.html`，
/// 经 `/wasm/opencat.js` bundle import 本模块构造合同对象）；消费方是
/// `opencat-web-oracle` crate——Rust 侧无法 import TS，eval 字符串里的方法名
/// 字面量必须与本表逐字一致（`harness.rs` 顶部注释锚定此文件）。
///
/// 新增/改名方法：先改这里的常量与类型，再同步 harness 页对象与 Rust eval
/// 字符串，三处一体。

/** Harness 页 `__opencatOracle` 暴露的全部方法名。 */
export const ORACLE_METHODS = [
  // 渲染通道（BrowserHarness 类型化调用）
  'renderFrame',
  'renderFramesToMp4',
  // eval_js 调试通道逐步拆用（栈里可见精确失败点）
  'openDesign',
  'buildFrameIr',
  'dumpFrameOps',
  'drawFrame',
  'captureCanvasPng',
  'probeVideoTimes',
  // 调试通道直接透出的内部面
  'getRendererOrThrow',
  'getReusableSurface',
  'releaseReusableSurface',
  'renderEncodedDrawFrame',
] as const;

export type OracleMethod = (typeof ORACLE_METHODS)[number];

/** `renderFrame`（hex 通道）的响应：一帧原始 RGBA。 */
export interface OracleRenderFrameResult {
  width: number;
  height: number;
  rgbaHex: string;
}

/** `renderFramesToMp4`（视频通道）的响应：位流经 `POST /oracle-video`
/// 带外落盘，WebDriver 只回元数据。 */
export interface OracleRenderMp4Result {
  width: number;
  height: number;
  fps: number;
  firstFrame: number;
  framesEncoded: number;
  /** 页侧 MP4 长度——与落盘文件字节数不一致即 POST 被截断，不可信。 */
  bytes: number;
  /** 逐帧编码耗时（ms），诊断用。 */
  frameTimings?: number[];
}
