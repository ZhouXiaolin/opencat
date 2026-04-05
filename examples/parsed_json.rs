use opencat::{Composition, EncodingConfig, ScriptDriver, parse};

const INPUT: &str = include_str!("pizza-palace-1280x720.jsonl");
fn main() -> anyhow::Result<()> {
    let input = if let Some(path) = std::env::args().nth(1) {
        std::fs::read_to_string(path)?
    } else {
        INPUT.to_string()
    };

    let parsed = parse(&input)?;

    println!("Parsed composition: {}x{}", parsed.width, parsed.height);

    let root = parsed.root;
    let script = parsed.script.unwrap_or_default();
    let driver = ScriptDriver::from_source(&script)?;

    let composition = Composition::new("parsed")
        .size(parsed.width, parsed.height)
        .fps(parsed.fps as u32)
        .frames(parsed.frames as u32)
        .root(move |_ctx| root.clone())
        .script_driver(driver)
        .build()?;

    let encode_config = EncodingConfig::png();
    std::fs::create_dir_all("out")?;
    composition.render("out/parsed.png", &encode_config)?;
    println!("Rendered out/parsed.png");

    Ok(())
}
