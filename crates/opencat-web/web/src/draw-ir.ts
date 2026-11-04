/// OCIR DrawOp 重放面的公共入口。实现拆三层：`draw-ir/decode`（唯一读
/// 字节处，wire 常量来自构建期生成物）、`draw-ir/ck-adapter`（wire 语义
/// → CanvasKit 调用映射）、`draw-ir/interpret`（`renderEncodedDrawFrame`
/// 主重放循环 + svg `<mask>` coverage 栈）；调试转储在 `draw-ir/dump`。
/// 本文件只做转发——公共 API 面与拆层前保持一致。

export { renderEncodedDrawFrame } from './draw-ir/interpret';
export { dumpEncodedFrameOps } from './draw-ir/dump';
export {
  MediaValidationError,
  isEncodedDrawEnvelope,
  decodeDrawEnvelope,
} from './draw-ir/decode';
export type { EncodedDrawFrame } from './draw-ir/decode';
