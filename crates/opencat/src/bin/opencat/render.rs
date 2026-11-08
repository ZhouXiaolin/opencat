use std::path::{Path, PathBuf};

use opencat_engine::render::{
    EncodingConfig, release_gl_sessions, render_from_source_with_base,
    render_single_frame_png_with_base,
};

pub fn render(
    input: PathBuf,
    output: Option<String>,
    frame: Option<u32>,
    base_dir: Option<PathBuf>,
    frames: Option<String>,
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

    let result = if let Some(spec) = frames {
        let list = parse_frames(&spec)?;
        std::fs::create_dir_all(&output)?;
        println!("Rendering {} frames: {} -> {}/", list.len(), input.display(), output);
        let config = EncodingConfig::png_frames(list);
        opencat_core::profile::run_from_env(|| {
            render_from_source_with_base(&source, base_dir, &output, &config)
        })
    } else if is_png_path(Path::new(&output)) {
        let frame_index = frame.unwrap_or(0);
        println!(
            "Rendering frame {}: {} -> {}",
            frame_index,
            input.display(),
            output
        );
        opencat_core::profile::run_from_env(|| {
            render_single_frame_png_with_base(&source, base_dir, &output, frame_index)
        })
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
        })
    };
    // 进程退出前拆掉常驻 GL 会话（成败都拆）。
    release_gl_sessions();
    result?;

    println!("Done: {}", output);
    Ok(())
}

fn is_png_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
}

/// `start:end[:step]`（end 不含）或逗号列表 → 帧号表。
fn parse_frames(spec: &str) -> anyhow::Result<Vec<u32>> {
    let bad = || anyhow::anyhow!("bad --frames spec `{spec}` (want start:end[:step] or a,b,c)");
    if spec.contains(':') {
        let parts: Vec<u32> = spec
            .split(':')
            .map(|p| p.trim().parse::<u32>())
            .collect::<Result<_, _>>()
            .map_err(|_| bad())?;
        let (start, end, step) = match parts.as_slice() {
            [s, e] => (*s, *e, 1),
            [s, e, st] if *st > 0 => (*s, *e, *st),
            _ => return Err(bad()),
        };
        Ok((start..end).step_by(step as usize).collect())
    } else {
        spec.split(',')
            .map(|p| p.trim().parse::<u32>().map_err(|_| bad()))
            .collect()
    }
}
