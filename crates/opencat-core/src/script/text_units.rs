//! Text unit segmentation (graphemes / words) for script-driven per-unit overrides.

use crate::scene::script::mutations::TextUnitGranularity;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone)]
pub struct ScriptTextUnitMeta {
    pub index: usize,
    pub text: String,
    pub start: usize,
    pub end: usize,
}

pub fn describe_text_units(
    text: &str,
    granularity: TextUnitGranularity,
) -> Vec<ScriptTextUnitMeta> {
    match granularity {
        TextUnitGranularity::Grapheme => describe_grapheme_units(text),
        TextUnitGranularity::Word => {
            if contains_cjk(text) {
                return describe_grapheme_units(text);
            }
            UnicodeSegmentation::split_word_bounds(text)
                .filter(|s| !s.is_empty())
                .scan(0usize, |offset, w| {
                    let start = *offset;
                    *offset += w.len();
                    Some((start, *offset, w))
                })
                .enumerate()
                .map(|(index, (start, end, w))| ScriptTextUnitMeta {
                    index,
                    text: w.to_string(),
                    start,
                    end,
                })
                .collect()
        }
    }
}

fn describe_grapheme_units(text: &str) -> Vec<ScriptTextUnitMeta> {
    UnicodeSegmentation::graphemes(text, true)
        .scan(0usize, |offset, g| {
            let start = *offset;
            *offset += g.len();
            Some((start, *offset, g))
        })
        .enumerate()
        .map(|(index, (start, end, g))| ScriptTextUnitMeta {
            index,
            text: g.to_string(),
            start,
            end,
        })
        .collect()
}

/// Return grapheme cluster strings for a text (used by the JS `__text_graphemes` bridge).
pub fn grapheme_strings(text: &str) -> Vec<String> {
    UnicodeSegmentation::graphemes(text, true)
        .map(|g| g.to_string())
        .collect()
}

/// Return byte ranges for word-mode segmentation, matching `describe_text_unit_ranges`.
/// CJK text falls back to grapheme ranges.
pub fn word_ranges(text: &str) -> Vec<[usize; 2]> {
    if contains_cjk(text) {
        return UnicodeSegmentation::graphemes(text, true)
            .scan(0usize, |offset, g| {
                let start = *offset;
                *offset += g.len();
                Some([start, *offset])
            })
            .collect();
    }
    UnicodeSegmentation::split_word_bounds(text)
        .filter(|s| !s.is_empty())
        .scan(0usize, |offset, w| {
            let start = *offset;
            *offset += w.len();
            Some([start, *offset])
        })
        .collect()
}

fn contains_cjk(text: &str) -> bool {
    text.chars().any(|ch| {
        matches!(
            ch as u32,
            0x3400..=0x4DBF
                | 0x4E00..=0x9FFF
                | 0xF900..=0xFAFF
                | 0x20000..=0x2A6DF
                | 0x2A700..=0x2B73F
                | 0x2B740..=0x2B81F
                | 0x2B820..=0x2CEAF
                | 0x3040..=0x309F
                | 0x30A0..=0x30FF
                | 0xAC00..=0xD7AF
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphemes_split_emojis_correctly() {
        let units = describe_text_units("a😀b", TextUnitGranularity::Grapheme);
        assert_eq!(units.len(), 3);
        assert_eq!(units[1].text, "😀");
    }

    #[test]
    fn cjk_text_falls_back_to_graphemes_under_word_mode() {
        let units = describe_text_units("你好", TextUnitGranularity::Word);
        assert_eq!(units.len(), 2);
        assert_eq!(units[0].text, "你");
    }

    #[test]
    fn word_mode_segments_latin_text() {
        let units = describe_text_units("hello world", TextUnitGranularity::Word);
        let words: Vec<&str> = units.iter().map(|u| u.text.as_str()).collect();
        assert!(words.contains(&"hello"));
        assert!(words.contains(&"world"));
    }

    #[test]
    fn word_ranges_matches_describe_text_units() {
        let text = "hello, world!";
        let units = describe_text_units(text, TextUnitGranularity::Word);
        let ranges = word_ranges(text);
        assert_eq!(units.len(), ranges.len());
        for (u, [start, end]) in units.iter().zip(ranges.iter()) {
            assert_eq!(u.start, *start);
            assert_eq!(u.end, *end);
        }
    }

    #[test]
    fn word_ranges_cjk_falls_back_to_graphemes() {
        let ranges = word_ranges("你好世界");
        assert_eq!(ranges.len(), 4);
    }
}
