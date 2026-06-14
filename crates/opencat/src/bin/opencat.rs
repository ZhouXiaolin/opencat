use std::ffi::OsString;
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use opencat_core::parse::lint::{Severity, lint_markup};
use opencat_engine::render::{
    EncodingConfig, render_from_jsonl_with_base, render_single_frame_png_with_base,
};

#[derive(Parser)]
#[command(name = "opencat", version, about = "Render and lint OpenCat markup (.xml)")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Render a markup file to MP4, or to a single PNG frame.
    Render {
        /// Input `.xml` markup file.
        input: PathBuf,
        /// Output path. Defaults to `out/<stem>.mp4`; a `.png` path renders one frame.
        #[allow(dead_code)]
        output: Option<String>,
        /// Frame index — only honored when `output` ends with `.png`.
        frame: Option<u32>,
    },
    /// Lint markup file(s): bad lucide names, missing/extra attributes, forbidden
    /// Tailwind classes, malformed XML, …
    Lint {
        /// Input `.xml` markup file(s).
        inputs: Vec<PathBuf>,
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
        } => render(input, output, frame),
        Command::Lint { inputs } => lint(inputs),
    }
}

/// Preserve the legacy bare-invocation form (`opencat input.xml …`) by injecting the
/// implicit `render` subcommand when the first argument is not itself a known
/// subcommand or a help/version flag. `opencat lint …` is left untouched.
fn default_render_subcommand(mut args: Vec<OsString>) -> Vec<OsString> {
    let inject = match args.get(1).and_then(|s| s.to_str()) {
        Some("render") | Some("lint") | Some("help") | Some("-h") | Some("--help")
        | Some("-V") | Some("--version") => false,
        _ => true,
    };
    if inject {
        args.insert(1, OsString::from("render"));
    }
    args
}

fn render(input: PathBuf, output: Option<String>, frame: Option<u32>) -> anyhow::Result<()> {
    let source = std::fs::read_to_string(&input)?;
    let base_dir = input.parent();

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
            render_from_jsonl_with_base(&source, base_dir, &output, &config)
        })?;
    }

    println!("Done: {}", output);
    Ok(())
}

fn lint(inputs: Vec<PathBuf>) -> anyhow::Result<()> {
    if inputs.is_empty() {
        return Err(anyhow::anyhow!(
            "no input file provided; usage: opencat lint <input>..."
        ));
    }

    let mut total_errors = 0usize;
    let mut total_warnings = 0usize;
    let mut any_printed = false;

    for input in &inputs {
        let source = match std::fs::read_to_string(input) {
            Ok(source) => source,
            Err(err) => {
                eprintln!("{}: {err}", input.display());
                total_errors += 1;
                continue;
            }
        };

        let diags = lint_markup(&source);
        if diags.is_empty() {
            continue;
        }

        any_printed = true;
        println!("{}:", input.display());
        for diag in &diags {
            let pos = match (diag.line, diag.col) {
                (Some(row), Some(col)) => format!("{row}:{col}"),
                _ => "-".to_string(),
            };
            match diag.severity {
                Severity::Error => total_errors += 1,
                Severity::Warning => total_warnings += 1,
            }
            println!(
                "  {:<7} {:<6} {}",
                diag.severity.as_str(),
                pos,
                diag.message
            );
        }
    }

    if !any_printed {
        println!("No problems found.");
    }
    println!(
        "\nfound {} error(s), {} warning(s)",
        total_errors, total_warnings
    );

    if total_errors > 0 {
        std::process::exit(1);
    }
    Ok(())
}

fn is_png_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
}
