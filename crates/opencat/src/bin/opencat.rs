use opencat_engine::render::{EncodingConfig, render_from_jsonl};

fn main() -> anyhow::Result<()> {
    let path = if let Some(path) = std::env::args().nth(1) {
        path
    } else {
        return Err(anyhow::anyhow!("No input file provided"));
    };

    let jsonl = std::fs::read_to_string(&path)?;
    let output = std::env::args().nth(2).unwrap_or_else(|| "output.mp4".to_string());

    println!("Rendering {} -> {}", path, output);

    let config = EncodingConfig::mp4();
    render_from_jsonl(&jsonl, &output, &config)?;

    println!("Done: {}", output);
    Ok(())
}
