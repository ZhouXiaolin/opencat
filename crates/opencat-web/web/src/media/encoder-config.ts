/// MP4 编码配置单源。
///
/// 两个编码 adapter 共用这里的 codec 串与码率公式：
/// - 交付路（`media/exporter.ts`，WebAV Combinator）——文件体积优先；
/// - 对拍路（oracle harness 页，裸 WebCodecs VideoEncoder）——cut 帧稳定性
///   优先，码率给足。
///
/// 约定注释（colorRange 不设 = full）是两路共同遵守的 wire 语义，跟着本
/// 模块走，不再各写一遍。

/// H.264 High profile codec 串：level 4.0 覆盖 ≤1088 行（1080p），更高行取
/// level 5.1。交付路从 baseline 升 High 是对齐对拍路——baseline 在整帧切换
/// 处重建质量更差，dev 交付与 oracle 对拍共用一条 codec 选择。
export function avcCodecString(height: number): string {
  return height <= 1088 ? 'avc1.640028' : 'avc1.640033';
}

/// 对拍路码率：0.30 bits/pixel/frame（1080p30 ≈19 Mbps，clamp 8 Mbps）。
/// 0.10 时代软编码器把整帧切换 crush +8-10 luma（hyperframes-launch
/// f116-119，IDR cut 帧也压），0.30 消除；文件体积约翻倍——对拍门接受，
/// 交付路（WebAV 默认码率）不用它。
export function probeBitrate(width: number, height: number, fps: number): number {
  return Math.max(8_000_000, Math.round(width * height * fps * 0.30));
}
