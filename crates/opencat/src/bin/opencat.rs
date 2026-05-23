use std::path::Path;

use opencat_engine::render::{EncodingConfig, render_from_jsonl_with_base};

fn main() -> anyhow::Result<()> {
    let path = if let Some(path) = std::env::args().nth(1) {
        path
    } else {
        return Err(anyhow::anyhow!("No input file provided"));
    };

    let jsonl = std::fs::read_to_string(&path)?;
    let base_dir = Path::new(&path).parent();

    let output = if let Some(out) = std::env::args().nth(2) {
        out
    } else {
        let stem = Path::new(&path).file_stem().unwrap_or_default().to_string_lossy().into_owned();
        let out_dir = "out";
        std::fs::create_dir_all(out_dir)?;
        format!("{out_dir}/{stem}.mp4")
    };

    println!("Rendering {} -> {}", path, output);

    let config = EncodingConfig::mp4();
    render_from_jsonl_with_base(&jsonl, base_dir, &output, &config)?;

    println!("Done: {}", output);
    Ok(())
}
