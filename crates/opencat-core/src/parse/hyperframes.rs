//! HyperFrames 私有扩展协议层（core 内，但不属于 HTML 标准、也不属于共有扩展）。
//!
//! 共有协议（组合标识、可见时间窗、尺寸、媒体相位、音量、__timelines）
//! 定义在 [`super::shared`]。本模块只放 HF 独有的属性与事件词表；
//! 播放器 seek 事件的派发实现住 JS 扩展层
//! `script/runtime/hyperframes_api.js`（`__ocHyperframes.dispatchSeek`，
//! realm.rs 每帧调入口）。为了兼容既有引用，这里把 shared 的公共项
//! 原样重导出。

/// HF 私有词汇表。与共有表同构：同时派生具名常量与
/// `protocol-vocab.generated.js` 的 `__ocVocab.hfAttr` / `__ocVocab.events`
/// （ocir-schema-gen 生成）——新增只改表。
macro_rules! hf_attrs {
    ($($name:ident => $key:literal),* $(,)?) => {
        /// (名, data-* 键) 对照——JS 词表的生成事实源。
        pub const HF_ATTRS: &[(&str, &str)] = &[$((stringify!($name), $key)),*];
        $(
            pub const $name: &str = $key;
        )*
    };
}

hf_attrs! {
    // HF 独有：轨道分层索引。opencat 的 `<timeline>` 用直接子元素顺序表达
    // 分镜，不需要轨道概念。
    TRACK_INDEX => "data-track-index",
    // opencat 特色 `<transition>` 节点的协议属性（往返契约的两侧输入：
    // 静态 parse 与 JS serializeNode 都从这里取键名）。
    FROM => "data-from",
    TO => "data-to",
    EFFECT => "data-effect",
    DIRECTION => "data-direction",
    TIMING => "data-timing",
    DAMPING => "data-damping",
    STIFFNESS => "data-stiffness",
    MASS => "data-mass",
    SEED => "data-seed",
    HUE_SHIFT => "data-hueShift",
    MASK_SCALE => "data-maskScale",
}

/// HF 私有事件名（JS 派发实现消费生成面，Rust 侧不写事件名字面量）。
pub const HF_EVENTS: &[(&str, &str)] = &[("SEEK", "hf-seek")];

pub mod attr {
    pub use super::super::shared::attr::*;
    pub use super::{DAMPING, DIRECTION, EFFECT, FROM, HUE_SHIFT, MASK_SCALE, MASS, SEED, STIFFNESS, TIMING, TO, TRACK_INDEX};
}

pub use super::shared::{AttrSource, ElementProtocol};
