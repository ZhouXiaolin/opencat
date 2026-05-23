use anyhow::{Context, Result, bail};
use clap::Parser;
use opencat_engine::render::render_single_frame_from_jsonl;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{PathBuf};

#[derive(Parser)]
struct Cli {
    #[arg(long)]
    generate: bool,
    #[arg(long)]
    check: bool,
    #[arg(long, default_value = "testsupport/golden")]
    root: PathBuf,
    #[arg(long, default_value = "testsupport/golden/manifest.json")]
    manifest: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    samples: Vec<Sample>,
}

#[derive(Serialize, Deserialize)]
struct Sample {
    name: String,
    jsonl: PathBuf,
    frames: Vec<u32>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let is_check = cli.check || (!cli.generate && !cli.check);

    let manifest: Manifest = if cli.manifest.exists() {
        serde_json::from_str(&fs::read_to_string(&cli.manifest)?)?
    } else {
        Manifest {
            samples: vec![
                Sample {
                    name: "path_demo".into(),
                    jsonl: "json/path_demo.jsonl".into(),
                    frames: vec![0, 15, 45, 89],
                },
                Sample {
                    name: "morph_svg_demo".into(),
                    jsonl: "json/morph_svg_demo.jsonl".into(),
                    frames: vec![0, 30, 90, 179],
                },
                Sample {
                    name: "animation_showcase".into(),
                    jsonl: "json/animation_showcase.jsonl".into(),
                    frames: vec![0, 25, 75, 149],
                },
            ],
        }
    };

    let mut failures = Vec::<String>::new();

    for s in &manifest.samples {
        let dir = cli.root.join(&s.name);
        fs::create_dir_all(&dir)?;

        let jsonl_content = fs::read_to_string(&s.jsonl)
            .with_context(|| format!("read {}", s.jsonl.display()))?;

        for &f in &s.frames {
            let (rgba, width, height) = render_single_frame_from_jsonl(&jsonl_content, f)
                .with_context(|| format!("render {} frame {}", s.name, f))?;

            let png = {
                let img = image::RgbaImage::from_raw(width, height, rgba)
                    .context("build png from rgba")?;
                let mut buf = std::io::Cursor::new(Vec::new());
                img.write_to(&mut buf, image::ImageFormat::Png)?;
                buf.into_inner()
            };

            let hash = hex(&Sha256::digest(&png));
            let hash_path = dir.join(format!("frame_{:04}.png.sha256", f));
            let png_path = dir.join(format!("frame_{:04}.png", f));

            if cli.generate {
                fs::write(&hash_path, &hash)?;
                fs::write(&png_path, &png)?;
                println!("WRITE {} frame {}", s.name, f);
            } else if is_check {
                let expected = fs::read_to_string(&hash_path)
                    .with_context(|| format!("baseline missing: {}", hash_path.display()))?;
                if expected.trim() != hash {
                    fs::write(&png_path, &png)?;
                    failures.push(format!(
                        "{} frame {}: expected {} got {}",
                        s.name,
                        f,
                        expected.trim(),
                        hash
                    ));
                }
            }
        }
    }

    if cli.generate {
        let manifest_json = serde_json::to_string_pretty(&manifest)?;
        fs::write(&cli.manifest, manifest_json)?;
        println!("WRITE manifest to {}", cli.manifest.display());
    }

    if !failures.is_empty() {
        for line in &failures {
            eprintln!("FAIL {line}");
        }
        bail!("{} golden mismatches", failures.len());
    }
    println!("OK {} samples", manifest.samples.len());
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}
