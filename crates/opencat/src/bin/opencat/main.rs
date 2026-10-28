mod render;
mod view;

use std::ffi::OsString;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "opencat",
    version,
    about = "Render or preview an OpenCat composition (.jsx)"
)]
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
        output: Option<String>,
        /// Frame index — only honored when `output` ends with `.png`.
        frame: Option<u32>,
        /// Directory for relative asset paths (`assets/fonts/...`). Defaults to
        /// the input file's directory. Set it when the input is a build artifact
        /// living outside the project (e.g. `dist/index.bundle.jsx`).
        #[arg(long, value_name = "DIR")]
        base_dir: Option<PathBuf>,
    },
    /// Preview a composition in a native window (looped playback with audio).
    View {
        /// Input `.jsx` composition file — the same artifact `render` consumes
        /// (usually the compiled `dist/<stem>.bundle.jsx`).
        input: PathBuf,
        /// Directory for relative asset paths (`assets/fonts/...`). Defaults to
        /// the input file's directory. Set it when the input is a build artifact
        /// living outside the project (e.g. `dist/index.bundle.jsx`).
        #[arg(long, value_name = "DIR")]
        base_dir: Option<PathBuf>,
        /// Preview scale (0, 1]. Defaults to 1.0 (composition resolution).
        #[arg(long)]
        scale: Option<f32>,
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
        } => render::render(input, output, frame, base_dir),
        Command::View {
            input,
            base_dir,
            scale,
        } => view::view(input, base_dir, scale),
    }
}

/// Preserve the legacy bare-invocation form (`opencat input.jsx …`) by injecting the
/// implicit `render` subcommand when the first argument is not itself a known
/// subcommand or a help/version flag.
fn default_render_subcommand(mut args: Vec<OsString>) -> Vec<OsString> {
    let inject = match args.get(1).and_then(|s| s.to_str()) {
        Some("render") | Some("view") | Some("help") | Some("-h") | Some("--help") | Some("-V")
        | Some("--version") => false,
        _ => true,
    };
    if inject {
        args.insert(1, OsString::from("render"));
    }
    args
}
