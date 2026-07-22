use anyhow::{Context, Result};

/// Pure SRT parser. Host provides decoded UTF-8 bytes; core parses entries.
/// This is the only remaining function in this module after migrating
/// image/video probing to hosts (issue #40).
pub(crate) fn parse_srt_bytes(
    bytes: &[u8],
    fps: u32,
) -> Result<Vec<crate::parse::primitives::SrtEntry>> {
    let text = std::str::from_utf8(bytes).context("srt: not valid utf-8")?;
    crate::parse::primitives::parse_srt(text, fps)
}
