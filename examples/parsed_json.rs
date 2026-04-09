use opencat::{Composition, EncodingConfig, RenderSession, ScriptDriver, parse, render_audio_chunk};

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
        .audio_sources(parsed.audio_sources.clone())
        .root(move |_ctx| root.clone())
        .build()?;

    let mut session = RenderSession::new();
    if let Some(chunk) = render_audio_chunk(&composition, &mut session, 0.0, 2048)? {
        println!(
            "Rendered initial audio chunk: {} sample frames @ {}Hz",
            chunk.samples.len() / chunk.channels as usize,
            chunk.sample_rate
        );
    }

    let encode_config = EncodingConfig::mp4();
    std::fs::create_dir_all("out")?;
    composition.render("out/parsed.mp4", &encode_config)?;
    println!("Rendered out/parsed.mp4");

    Ok(())
}
