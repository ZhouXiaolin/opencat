use opencat::{Composition, EncodingConfig, ScriptDriver, parse};

const INPUT: &str = include_str!("hangzhou_landmarks.jsonl");
fn main() -> anyhow::Result<()> {
    let parsed = parse(INPUT)?;

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

    let encode_config = EncodingConfig::mp4();
    std::fs::create_dir_all("out")?;
    composition.render("out/parsed.mp4", &encode_config)?;
    println!("Rendered out/parsed.mp4");

    Ok(())
}
