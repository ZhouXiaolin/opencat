use std::path::{Path, PathBuf};

use opencat_engine::render::{
    EncodingConfig, render_from_source_with_base, render_single_frame_png_with_base,
};

pub fn render(
    input: PathBuf,
    output: Option<String>,
    frame: Option<u32>,
    base_dir: Option<PathBuf>,
) -> anyhow::Result<()> {
    let source = std::fs::read_to_string(&input)?;
    let base_dir = base_dir.as_deref().or_else(|| input.parent());

    let output = output.unwrap_or_else(|| {
        let stem = input
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let _ = std::fs::create_dir_all("out");
        format!("out/{stem}.mp4")
    });

    if is_png_path(Path::new(&output)) {
        let frame_index = frame.unwrap_or(0);
        println!(
            "Rendering frame {}: {} -> {}",
            frame_index,
            input.display(),
            output
        );
        opencat_core::profile::run_from_env(|| {
            render_single_frame_png_with_base(&source, base_dir, &output, frame_index)
        })?;
    } else {
        if let Some(frame) = frame {
            return Err(anyhow::anyhow!(
                "frame index is only supported when output path ends with .png (got `{frame}`)"
            ));
        }
        println!("Rendering {} -> {}", input.display(), output);
        let config = EncodingConfig::mp4();
        opencat_core::profile::run_from_env(|| {
            render_from_source_with_base(&source, base_dir, &output, &config)
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
