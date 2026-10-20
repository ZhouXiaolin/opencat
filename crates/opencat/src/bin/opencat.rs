use std::ffi::OsString;
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use opencat_engine::render::{
    EncodingConfig, render_from_source_with_base, render_single_frame_png_with_base,
};

#[derive(Parser)]
#[command(name = "opencat", version, about = "Render an OpenCat composition (.jsx)")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Render a composition file to MP4, or to a single PNG frame.
    Render {
        /// Input `.jsx` composition file.
        input: PathBuf,
        /// Output path. Defaults to `out/<stem>.mp4`; a `.png` path renders one frame.
        #[allow(dead_code)]
        output: Option<String>,
        /// Frame index — only honored when `output` ends with `.png`.
        frame: Option<u32>,
        /// Directory for relative asset paths (`assets/fonts/...`). Defaults to
        /// the input file's directory. Set it when the input is a build artifact
        /// living outside the project (e.g. `dist/index.bundle.jsx`).
        #[arg(long, value_name = "DIR")]
        base_dir: Option<PathBuf>,
    },
}

fn main() -> anyhow::Result<()> {
    let args = default_render_subcommand(std::env::args_os().collect());
    let cli = Cli::parse_from(args);
    match cli.command {
        Command::Render {
            input,
            output,
            frame,
            base_dir,
        } => render(input, output, frame, base_dir),
    }
}

/// Preserve the legacy bare-invocation form (`opencat input.jsx …`) by injecting the
/// implicit `render` subcommand when the first argument is not itself a known
/// subcommand or a help/version flag.
fn default_render_subcommand(mut args: Vec<OsString>) -> Vec<OsString> {
    let inject = match args.get(1).and_then(|s| s.to_str()) {
        Some("render") | Some("help") | Some("-h") | Some("--help") | Some("-V")
        | Some("--version") => false,
        _ => true,
    };
    if inject {
        args.insert(1, OsString::from("render"));
    }
    args
}

fn render(
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
