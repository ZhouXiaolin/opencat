use opencat::{Composition, EncodingConfig, RenderBackend, ScriptDriver, parse};

fn main() -> anyhow::Result<()> {
    let input = if let Some(path) = std::env::args().nth(1) {
        std::fs::read_to_string(path)?
    } else {
        return Err(anyhow::anyhow!("No input file provided"));
    };

    let parsed = parse(&input)?;

    println!("Parsed composition: {}x{}", parsed.width, parsed.height);

    let root = parsed.root;
    let script = parsed.script.unwrap_or_default();
    let driver = ScriptDriver::from_source(&script)?;
    let root = root.script_driver(driver);

    let composition = Composition::new("parsed")
        .size(parsed.width, parsed.height)
        .fps(parsed.fps as u32)
        .frames(parsed.frames as u32)
        .root(move |_ctx| root.clone())
        .build()?;

    let encode_config = EncodingConfig::mp4();
    std::fs::create_dir_all("out")?;
    composition.render_with_backend("out/parsed.mp4", &encode_config, RenderBackend::SkiaMetal)?;
    println!("Rendered out/parsed.mp4");

    Ok(())
}
