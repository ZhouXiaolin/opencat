use opencat_core::parse::document::ParsedComposition;

pub fn parse_source(input: &str) -> anyhow::Result<ParsedComposition> {
    let trimmed = input.trim();
    if trimmed.starts_with('{') {
        opencat_core::parse::jsonl::parse_with_base_dir(input, None)
    } else {
        opencat_core::parse::markup::parse_with_base_dir(input, None)
    }
}
