use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};

use crate::core::style::{NodeStyle, impl_node_style_api};

#[derive(Clone, Debug)]
pub struct SrtEntry {
    pub index: u32,
    pub start_frame: u32,
    pub end_frame: u32,
    pub text: String,
}

#[derive(Clone)]
pub struct CaptionNode {
    path: PathBuf,
    entries: Vec<SrtEntry>,
    pub(crate) style: NodeStyle,
}

impl CaptionNode {
    pub fn path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = path.into();
        self
    }

    pub(crate) fn entries(mut self, entries: Vec<SrtEntry>) -> Self {
        self.entries = entries;
        self
    }

    pub fn path_ref(&self) -> &Path {
        &self.path
    }

    pub fn entries_ref(&self) -> &[SrtEntry] {
        &self.entries
    }

    pub fn style_ref(&self) -> &NodeStyle {
        &self.style
    }

    pub fn active_text(&self, frame: u32) -> Option<&str> {
        self.entries
            .iter()
            .find(|entry| frame >= entry.start_frame && frame < entry.end_frame)
            .map(|entry| entry.text.as_str())
    }
}

pub fn caption() -> CaptionNode {
    CaptionNode {
        path: PathBuf::new(),
        entries: Vec::new(),
        style: NodeStyle::default(),
    }
}

fn timestamp_to_frame(ts: &str, fps: u32) -> Result<u32> {
    let parts: Vec<&str> = ts.split(':').collect();
    if parts.len() != 3 {
        return Err(anyhow!("invalid srt timestamp: {}", ts));
    }
    let hours: u32 = parts[0].parse().map_err(|_| anyhow!("invalid hours"))?;
    let minutes: u32 = parts[1].parse().map_err(|_| anyhow!("invalid minutes"))?;
    let sec_ms: Vec<&str> = parts[2].split(',').collect();
    if sec_ms.len() != 2 {
        return Err(anyhow!("invalid srt timestamp seconds: {}", ts));
    }
    let seconds: u32 = sec_ms[0].parse().map_err(|_| anyhow!("invalid seconds"))?;
    let millis: u32 = sec_ms[1].parse().map_err(|_| anyhow!("invalid millis"))?;
    let total_ms = u64::from(hours) * 3_600_000
        + u64::from(minutes) * 60_000
        + u64::from(seconds) * 1000
        + u64::from(millis);
    let scaled = total_ms * u64::from(fps);
    Ok(scaled.div_ceil(1000) as u32)
}

pub fn parse_srt(input: &str, fps: u32) -> Result<Vec<SrtEntry>> {
    let input = input
        .strip_prefix('\u{feff}')
        .unwrap_or(input)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let mut entries = Vec::new();
    for block in input.split("\n\n").filter(|b| !b.trim().is_empty()) {
        let mut lines = block.lines();
        let index = lines
            .next()
            .ok_or_else(|| anyhow!("missing srt index"))?
            .trim()
            .parse::<u32>()?;
        let timing = lines
            .next()
            .ok_or_else(|| anyhow!("missing srt timing line"))?;
        let (start, end) = timing
            .split_once("-->")
            .ok_or_else(|| anyhow!("invalid srt timing separator"))?;
        let text = lines.collect::<Vec<_>>().join("\n");
        entries.push(SrtEntry {
            index,
            start_frame: timestamp_to_frame(start.trim(), fps)?,
            end_frame: timestamp_to_frame(end.trim(), fps)?,
            text,
        });
    }
    Ok(entries)
}

impl_node_style_api!(CaptionNode);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caption_parses_srt_and_selects_entry_for_frame() {
        let entries = parse_srt(
            "1\n00:00:00,000 --> 00:00:01,000\nHello\n\n2\n00:00:01,000 --> 00:00:02,000\nWorld\n",
            30,
        )
        .expect("srt should parse");

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].start_frame, 0);
        assert_eq!(entries[0].end_frame, 30);

        let node = caption().id("subs").path("sub.srt").entries(entries);

        assert_eq!(node.active_text(0), Some("Hello"));
        assert_eq!(node.active_text(29), Some("Hello"));
        assert_eq!(node.active_text(30), Some("World"));
        assert_eq!(node.active_text(61), None);
    }

    #[test]
    fn caption_parses_crlf_srt_and_selects_entries_by_timestamp() {
        let entries = parse_srt(
            "1\r\n00:00:06,000 --> 00:00:07,170\r\n妈的\r\n\r\n2\r\n00:00:09,090 --> 00:00:10,000\r\n（前情提要）\r\n\r\n3\r\n00:00:10,010 --> 00:00:11,970\r\n你说你会一直照顾我 你保证过的\r\n",
            30,
        )
        .expect("crlf srt should parse");

        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].start_frame, 180);
        assert_eq!(entries[0].end_frame, 216);
        assert_eq!(entries[1].start_frame, 273);
        assert_eq!(entries[1].end_frame, 300);
        assert_eq!(entries[2].start_frame, 301);
        assert_eq!(entries[2].end_frame, 360);

        let node = caption().id("subs").path("sub.srt").entries(entries);

        assert_eq!(node.active_text(179), None);
        assert_eq!(node.active_text(180), Some("妈的"));
        assert_eq!(node.active_text(215), Some("妈的"));
        assert_eq!(node.active_text(216), None);
        assert_eq!(node.active_text(272), None);
        assert_eq!(node.active_text(273), Some("（前情提要）"));
        assert_eq!(node.active_text(299), Some("（前情提要）"));
        assert_eq!(node.active_text(300), None);
        assert_eq!(node.active_text(301), Some("你说你会一直照顾我 你保证过的"));
    }
}
