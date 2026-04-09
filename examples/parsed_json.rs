use opencat::{Composition, EncodingConfig, ScriptDriver, parse};

fn main() -> anyhow::Result<()> {
    let input = if let Some(path) = std::env::args().nth(1) {
        std::fs::read_to_string(path)?
    } else {
        return Err(anyhow::anyhow!("No input file provided"));
    };

    let parsed = parse(&input)?;

    println!("Parsed composition: {}x{}", parsed.width, parsed.height);

    let root = if let Some(script) = parsed.script.as_deref() {
        if script.trim().is_empty() {
            parsed.root
        } else {
            let driver = ScriptDriver::from_source(script)?;
            parsed.root.script_driver(driver)
        }
    } else {
        parsed.root
    };

    let composition = Composition::new("parsed")
        .size(parsed.width, parsed.height)
        .fps(parsed.fps as u32)
        .frames(parsed.frames as u32)
        .global_audio_sources(parsed.global_audio_sources.clone())
        .root(move |_ctx| root.clone())
        .build()?;

    let encode_config = EncodingConfig::mp4();
    std::fs::create_dir_all("out")?;
    composition.render("out/parsed.mp4", &encode_config)?;
    println!("Rendered out/parsed.mp4");

    Ok(())
}
