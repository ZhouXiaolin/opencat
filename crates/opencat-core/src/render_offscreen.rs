//! 离屏 canvas 光栅化的 engine 公开入口。
//!
//! `render` 模块整体是 `pub(crate)`（显示树内部结构），但离屏 canvas 的
//! 脚本期像素化是 engine 绑定层的直接依赖（getImageData/toDataURL/
//! createPattern/drawImage(canvas源) 的同步消费点），在这里 re-export
//! 折叠入口，不暴露其余内部类型。

pub use crate::render::offscreen::compile_offscreen_commands;
