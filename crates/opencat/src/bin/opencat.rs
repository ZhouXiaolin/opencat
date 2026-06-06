use std::path::Path;

use opencat_engine::render::{
    EncodingConfig, render_from_jsonl_with_base, render_single_frame_png_with_base,
};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let path = if let Some(path) = args.next() {
        path
    } else {
        return Err(anyhow::anyhow!("No input file provided"));
    };

    let source = std::fs::read_to_string(&path)?;
    let base_dir = Path::new(&path).parent();

    let output = if let Some(out) = args.next() {
        out
    } else {
        let stem = Path::new(&path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let out_dir = "out";
        std::fs::create_dir_all(out_dir)?;
        format!("{out_dir}/{stem}.mp4")
    };

    let frame_index = args
        .next()
        .map(|arg| arg.parse::<u32>())
        .transpose()
        .map_err(|err| anyhow::anyhow!("invalid frame index: {err}"))?;
    if let Some(extra) = args.next() {
        return Err(anyhow::anyhow!("unexpected extra argument: {extra}"));
    }

    if is_png_path(Path::new(&output)) {
        let frame_index = frame_index.unwrap_or(0);
        println!("Rendering frame {}: {} -> {}", frame_index, path, output);
        opencat_core::profile::run_from_env(|| {
            render_single_frame_png_with_base(&source, base_dir, &output, frame_index)
        })?;
    } else {
        if frame_index.is_some() {
            return Err(anyhow::anyhow!(
                "frame index is only supported when output path ends with .png"
            ));
        }
        println!("Rendering {} -> {}", path, output);
        let config = EncodingConfig::mp4();
        opencat_core::profile::run_from_env(|| {
            render_from_jsonl_with_base(&source, base_dir, &output, &config)
        })?;
    }

    println!("Done: {}", output);
    Ok(())
}

fn is_png_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
}
