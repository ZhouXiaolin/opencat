//! hyperframes 扩展协议层——core 内唯一认识 hyperframes 私有语义的地方。
//!
//! # 分层
//!
//! - **标准层**：W3C / 浏览器语义（HTML 解析、CSS 级联与布局、DOM API、
//!   资源加载、canvas/GL 通道）。标准层只处理「元素 + 原始属性表」，不解释
//!   任何 hyperframes 私有属性。
//! - **扩展层**（本模块）：hyperframes 框架私有协议——组合标识
//!   （`data-composition-id` / `data-composition-src`）、可见时间窗
//!   （`data-start` / `data-duration`）、合成尺寸与帧率（`data-width` /
//!   `data-height` / `data-fps`）、媒体相位（`data-media-start`）、媒体
//!   着色矩阵（`data-color-matrix`）、终帧可见性（`data-hide-after-end`）。
//!   这些语义只经本模块进入 core。
//!
//! # 迁移方向
//!
//! JSX 通道与 hyperframes 适配层最终应像 gsap / three sidecar 一样挂在 core
//! 之外，而不是长在 core 的标准语义里。本模块是该迁移的接缝：标准层只调
//! `protocol::` 的窄接口，协议词汇与语义只此一处定义。

/// 协议属性名。标准层不得再出现这些字面量。
pub mod attr {
    /// 组合标识：标记一棵 composition 子树（根选择与查询面收窄用）。
    pub const COMPOSITION_ID: &str = "data-composition-id";
    /// 组合宿主：指向子工程文件，引擎把子工程内联进宿主元素。
    pub const COMPOSITION_SRC: &str = "data-composition-src";
    /// 可见窗起点（秒，合成全局时间）。
    pub const START: &str = "data-start";
    /// 可见窗时长（秒）。
    pub const DURATION: &str = "data-duration";
    /// 合成尺寸宽（像素）。
    pub const WIDTH: &str = "data-width";
    /// 合成尺寸高（像素）。
    pub const HEIGHT: &str = "data-height";
    /// 合成帧率。
    pub const FPS: &str = "data-fps";
    /// 媒体相位：媒体源内的起播偏移（秒）。
    pub const MEDIA_START: &str = "data-media-start";
    /// 媒体着色矩阵（4x5 行主序）。
    pub const COLOR_MATRIX: &str = "data-color-matrix";
    /// 终帧可见性：媒体播完后是否继续显示其末帧。
    pub const HIDE_AFTER_END: &str = "data-hide-after-end";
}

/// 属性读取源：标准层只提供原始属性，协议层负责语义。
pub trait AttrSource {
    fn attr(&self, name: &str) -> Option<&str>;
    fn num(&self, name: &str) -> Option<f64> {
        self.attr(name).and_then(|v| v.trim().parse().ok())
    }
    /// 布尔属性：空值（`data-x`）与 `"true"` 都算真。属性缺席 = `None`
    /// （协议缺席），与「显式为假」区分。
    fn flag(&self, name: &str) -> Option<bool> {
        self.attr(name).map(|v| v.is_empty() || v == "true")
    }
    /// 非空字符串属性。
    fn text(&self, name: &str) -> Option<String> {
        self.attr(name)
            .map(str::to_string)
            .filter(|s| !s.is_empty())
    }
}

/// 元素级协议属性（hyperframes 扩展）。全部可选 = 协议缺席时退化为标准
/// HTML 语义（元素恒可见、无时间窗）。
#[derive(Debug, Default, Clone)]
pub struct ElementProtocol {
    pub composition_id: Option<String>,
    pub composition_src: Option<String>,
    pub start: Option<f64>,
    pub duration: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub fps: Option<f64>,
    pub media_start: Option<f64>,
    pub color_matrix: Option<String>,
    pub hide_after_end: Option<bool>,
}

impl ElementProtocol {
    /// 从原始属性表读协议属性。
    pub fn read(src: &impl AttrSource) -> Self {
        Self {
            composition_id: src.text(attr::COMPOSITION_ID),
            composition_src: src.text(attr::COMPOSITION_SRC),
            start: src.num(attr::START),
            duration: src.num(attr::DURATION),
            width: src.num(attr::WIDTH),
            height: src.num(attr::HEIGHT),
            fps: src.num(attr::FPS),
            media_start: src.num(attr::MEDIA_START),
            color_matrix: src.text(attr::COLOR_MATRIX),
            hide_after_end: src.flag(attr::HIDE_AFTER_END),
        }
    }

    /// 可见时间窗（`[start, start+duration)` 半开窗）。无 `data-start` 时
    /// 恒可见。
    pub fn window(&self) -> TimeWindow {
        TimeWindow {
            start: self.start,
            duration: self.duration,
        }
    }
}

/// 可见时间窗语义。
///
/// 约定：`[start, start+duration)` 半开窗，窗外整棵子树退出布局与绘制。
/// 无 `start` = 恒可见（协议缺席，退化为标准 HTML）。
#[derive(Debug, Default, Clone, Copy)]
pub struct TimeWindow {
    pub start: Option<f64>,
    pub duration: Option<f64>,
}

impl TimeWindow {
    /// 窗是否恒开（协议缺席）。
    pub fn is_open_ended(&self) -> bool {
        self.start.is_none()
    }

    /// 窗的起点（秒）。缺席 = 0。
    pub fn start_secs(&self) -> f64 {
        self.start.unwrap_or(0.0)
    }

    /// 窗的终点（秒）。`duration` 缺席 = 无终点。
    pub fn end_secs(&self) -> Option<f64> {
        self.duration.map(|d| self.start_secs() + d)
    }

    /// `t`（合成全局秒）是否落在窗外。
    pub fn hides(&self, t: f64) -> bool {
        match self.start {
            None => false,
            Some(s) => match self.duration {
                Some(d) => t < s || t >= s + d,
                None => t < s,
            },
        }
    }
}
