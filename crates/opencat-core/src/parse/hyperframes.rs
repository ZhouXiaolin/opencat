//! HyperFrames 扩展协议层（core 内，但不属于 HTML 标准）。
//!
//! 标准层只处理 W3C / 浏览器语义。本模块是 hyperframes 私有属性的唯一
//! 定义点：组合标识、可见时间窗、合成尺寸、媒体相位、着色矩阵、播放器
//! `hf-seek` 事件。标准层通过这里的窄接口读取协议，不写协议字面量。

/// 协议属性名。标准层不得再出现这些字面量。
pub mod attr {
    pub const COMPOSITION_ID: &str = "data-composition-id";
    pub const COMPOSITION_SRC: &str = "data-composition-src";
    pub const START: &str = "data-start";
    pub const DURATION: &str = "data-duration";
    pub const WIDTH: &str = "data-width";
    pub const HEIGHT: &str = "data-height";
    pub const FPS: &str = "data-fps";
    pub const MEDIA_START: &str = "data-media-start";
    pub const COLOR_MATRIX: &str = "data-color-matrix";
    pub const HIDE_AFTER_END: &str = "data-hide-after-end";
}

pub const SEEK_EVENT: &str = "hf-seek";

pub fn seek_event_js() -> String {
    format!(
        "if (globalThis.__ocMode !== 'extract') {{ try {{ globalThis.dispatchEvent(new CustomEvent('{SEEK_EVENT}', {{ detail: {{ time: (globalThis.__ocClockMs() || 0) / 1000 }} }})); }} catch (error) {{ (globalThis.__ocLogs = globalThis.__ocLogs || []).push('{SEEK_EVENT} handler threw: ' + (error && error.stack || error)); }} }}"
    )
}

pub trait AttrSource {
    fn attr(&self, name: &str) -> Option<&str>;
    fn num(&self, name: &str) -> Option<f64> {
        self.attr(name).and_then(|v| v.trim().parse().ok())
    }
    fn flag(&self, name: &str) -> Option<bool> {
        self.attr(name).map(|v| v.is_empty() || v == "true")
    }
    fn text(&self, name: &str) -> Option<String> {
        self.attr(name)
            .map(str::to_string)
            .filter(|s| !s.is_empty())
    }
}

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

    pub fn window(&self) -> TimeWindow {
        TimeWindow {
            start: self.start,
            duration: self.duration,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TimeWindow {
    pub start: Option<f64>,
    pub duration: Option<f64>,
}

impl TimeWindow {
    pub fn is_open_ended(&self) -> bool {
        self.start.is_none()
    }

    pub fn start_secs(&self) -> f64 {
        self.start.unwrap_or(0.0)
    }

    pub fn end_secs(&self) -> Option<f64> {
        self.duration.map(|d| self.start_secs() + d)
    }

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
