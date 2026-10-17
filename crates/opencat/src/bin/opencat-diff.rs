//! Hard-pixel frame differ (k3diff semantics) with no Python dependency.
//!
//! Streams both videos as raw `rgb24` from ffmpeg and compares every pixel:
//! `mae`, `maxd`, and the fraction of pixels whose max-channel delta exceeds
//! each threshold in `[2, 4, 8, 16, 32, 64, 128]`. SSIM is deliberately not
//! used: it collapses a frame to one scalar and hides localized misalignment.
//!
//! Usage:
//!   opencat-diff REF TEST --out out/align-hf [--dump 100,164] [--sheet 60]

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use clap::Parser;

const THRESHOLDS: [u32; 7] = [2, 4, 8, 16, 32, 64, 128];
const GRID_COLS: usize = 16;
const GRID_ROWS: usize = 9;

#[derive(Parser)]
#[command(name = "opencat-diff", about = "Hard-pixel frame diff (k3diff metrics)")]
struct Cli {
    /// Reference video.
    ref_path: PathBuf,
    /// Test video.
    test_path: PathBuf,
    /// Directory for pixel.csv, summary.json and dumps.
    #[arg(long)]
    out: PathBuf,
    /// Comma-separated frames to dump as `<frame>-ref.png`, `<frame>-test.png`,
    /// `<frame>-diff.png`.
    #[arg(long, default_value = "")]
    dump: String,
    /// Contact sheet: append a heatmap every N frames.
    #[arg(long, default_value_t = 0)]
    sheet: usize,
    /// Bounding-box threshold (k3diff default 16).
    #[arg(long, default_value_t = 16)]
    bbox_thresh: u32,
    /// Override decode size as `WxH`.
    #[arg(long, default_value = "")]
    size: String,
}

struct Frame {
    buf: Vec<u8>,
}

struct Decoder {
    child: std::process::Child,
    stdout: std::process::ChildStdout,
    frame_len: usize,
    width: u32,
    height: u32,
}

impl Decoder {
    fn spawn(path: &Path, width: u32, height: u32) -> Result<Self> {
        let mut child = Command::new("ffmpeg")
            .args(["-v", "error", "-i"])
            .arg(path)
            .args([
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgb24",
                "-s",
                &format!("{width}x{height}"),
                "-",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("spawn ffmpeg to decode {}", path.display()))?;
        let stdout = child.stdout.take().context("ffmpeg stdout should be piped")?;
        Ok(Self {
            child,
            stdout,
            frame_len: width as usize * height as usize * 3,
            width,
            height,
        })
    }

    fn next_frame(&mut self) -> Result<Option<Frame>> {
        let mut buf = vec![0u8; self.frame_len];
        let mut filled = 0;
        while filled < self.frame_len {
            match self.stdout.read(&mut buf[filled..]) {
                Ok(0) => return Ok(None),
                Ok(n) => filled += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e).context("read raw frame"),
            }
        }
        Ok(Some(Frame { buf }))
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        let _ = self.child.wait();
    }
}

/// Probe width/height/frame count with ffprobe (same source k3diff.py uses).
fn probe(path: &Path) -> Result<(u32, u32, u64)> {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-count_frames",
            "-show_entries",
            "stream=nb_read_frames,width,height",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .with_context(|| format!("spawn ffprobe for {}", path.display()))?;
    if !out.status.success() {
        bail!("ffprobe failed for {}", path.display());
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().next().unwrap_or("");
    let mut parts = line.split(',');
    let w: u32 = parts.next().unwrap_or("0").trim().parse().unwrap_or(0);
    let h: u32 = parts.next().unwrap_or("0").trim().parse().unwrap_or(0);
    let n: u64 = parts.next().unwrap_or("0").trim().parse().unwrap_or(0);
    if w == 0 || h == 0 {
        bail!("ffprobe returned no video stream for {}", path.display());
    }
    Ok((w, h, n))
}

struct Stats {
    frame: u64,
    mae: f64,
    maxd: u32,
    frac: [f64; 7],
    bbox: Option<(u32, u32, u32, u32)>,
    grid_cell: Option<(usize, usize)>,
}

fn diff_frame(a: &[u8], b: &[u8], w: u32, h: u32, bbox_thresh: u32) -> Stats {
    let px = w as usize * h as usize;
    let mut sum: u64 = 0;
    let mut maxd: u32 = 0;
    let mut counts = [0u64; 7];
    let mut bbox: Option<(u32, u32, u32, u32)> = None;
    let mut grid = vec![0f64; GRID_COLS * GRID_ROWS];

    for i in 0..px {
        let base = i * 3;
        let mut dm: u32 = 0;
        for c in 0..3 {
            let d = a[base + c].abs_diff(b[base + c]) as u32;
            sum += u64::from(d);
            dm = dm.max(d);
        }
        maxd = maxd.max(dm);
        for (k, t) in THRESHOLDS.iter().enumerate() {
            if dm > *t {
                counts[k] += 1;
            }
        }
        if dm > bbox_thresh {
            let x = (i % w as usize) as u32;
            let y = (i / w as usize) as u32;
            bbox = Some(match bbox {
                None => (x, y, x, y),
                Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
            });
        }
        let gx = (i % w as usize) * GRID_COLS / w as usize;
        let gy = (i / w as usize) * GRID_ROWS / h as usize;
        grid[gy * GRID_COLS + gx] += f64::from(dm);
    }

    let cell_px = (w as f64 / GRID_COLS as f64) * (h as f64 / GRID_ROWS as f64);
    let mut best = 0usize;
    for (k, v) in grid.iter().enumerate() {
        if *v > grid[best] {
            best = k;
        }
    }
    let grid_cell = if bbox.is_some() {
        Some((best % GRID_COLS, best / GRID_COLS))
    } else {
        None
    };
    let _ = cell_px;

    Stats {
        frame: 0,
        mae: sum as f64 / (px as f64 * 3.0),
        maxd,
        frac: counts.map(|c| c as f64 / px as f64),
        bbox,
        grid_cell,
    }
}

/// Amplified heatmap: 0 -> black, high delta -> red.
fn heat(dm: &[u8], w: u32, h: u32) -> Vec<u8> {
    let mut rgb = vec![0u8; w as usize * h as usize * 3];
    for (i, &d) in dm.iter().enumerate() {
        rgb[i * 3] = (u32::from(d) * 8).min(255) as u8;
        rgb[i * 3 + 1] = (u32::from(d) * 2).min(255) as u8;
    }
    rgb
}

fn save_png(path: &Path, w: u32, h: u32, rgb: &[u8]) -> Result<()> {
    let img = image::RgbImage::from_raw(w, h, rgb.to_vec())
        .with_context(|| format!("build image buffer for {}", path.display()))?;
    img.save(path)
        .with_context(|| format!("save {}", path.display()))
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    fs::create_dir_all(&cli.out)?;

    let (mut w, mut h, nref) = probe(&cli.ref_path)?;
    let (w2, h2, ntest) = probe(&cli.test_path)?;
    if !cli.size.is_empty() {
        let (sw, sh) = cli
            .size
            .split_once(['x', 'X'])
            .context("--size expects WxH")?;
        w = sw.parse()?;
        h = sh.parse()?;
    } else if (w, h) != (w2, h2) {
        bail!("size mismatch ref {w}x{h} test {w2}x{h2}");
    }
    println!("ref frames={nref} test frames={ntest} @ {w}x{h}");

    let dump: Vec<u64> = cli
        .dump
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.trim().parse::<u64>())
        .collect::<Result<_, _>>()
        .context("--dump expects comma-separated frame numbers")?;

    let mut ref_dec = Decoder::spawn(&cli.ref_path, w, h)?;
    let mut test_dec = Decoder::spawn(&cli.test_path, w, h)?;

    let mut rows: Vec<Stats> = Vec::new();
    let mut sheet_tiles: Vec<Vec<u8>> = Vec::new();
    let mut frame: u64 = 0;
    loop {
        let a = match ref_dec.next_frame()? {
            Some(f) => f,
            None => break,
        };
        let b = match test_dec.next_frame()? {
            Some(f) => f,
            None => break,
        };
        let mut st = diff_frame(&a.buf, &b.buf, w, h, cli.bbox_thresh);
        st.frame = frame;

        if dump.contains(&frame) {
            save_png(&cli.out.join(format!("{frame:04}-ref.png")), w, h, &a.buf)?;
            save_png(&cli.out.join(format!("{frame:04}-test.png")), w, h, &b.buf)?;
            let mut dm = vec![0u8; w as usize * h as usize];
            for (i, slot) in dm.iter_mut().enumerate() {
                let base = i * 3;
                *slot = a.buf[base]
                    .abs_diff(b.buf[base])
                    .max(a.buf[base + 1].abs_diff(b.buf[base + 1]))
                    .max(a.buf[base + 2].abs_diff(b.buf[base + 2]));
            }
            save_png(&cli.out.join(format!("{frame:04}-diff.png")), w, h, &heat(&dm, w, h))?;
        }
        if cli.sheet > 0 && frame % cli.sheet as u64 == 0 {
            let mut dm = vec![0u8; w as usize * h as usize];
            for (i, slot) in dm.iter_mut().enumerate() {
                let base = i * 3;
                *slot = a.buf[base]
                    .abs_diff(b.buf[base])
                    .max(a.buf[base + 1].abs_diff(b.buf[base + 1]))
                    .max(a.buf[base + 2].abs_diff(b.buf[base + 2]));
            }
            let full = image::RgbImage::from_raw(w, h, heat(&dm, w, h))
                .context("build sheet tile")?;
            let tile = image::imageops::resize(
                &full,
                w / 4,
                h / 4,
                image::imageops::FilterType::Triangle,
            );
            sheet_tiles.push(tile.into_raw());
        }
        rows.push(st);
        frame += 1;
    }
    if rows.is_empty() {
        bail!("no frames decoded from both inputs");
    }

    // pixel.csv
    {
        let mut f = fs::File::create(cli.out.join("pixel.csv"))?;
        writeln!(f, "frame,mae,maxd,p2,p4,p8,p16,p32,p64,p128,bbox")?;
        for r in &rows {
            let bbox = match r.bbox {
                Some((x0, y0, x1, y1)) => format!("{x0},{y0},{x1},{y1}"),
                None => String::new(),
            };
            writeln!(
                f,
                "{},{:.6},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{}",
                r.frame,
                r.mae,
                r.maxd,
                r.frac[0],
                r.frac[1],
                r.frac[2],
                r.frac[3],
                r.frac[4],
                r.frac[5],
                r.frac[6],
                bbox
            )?;
        }
    }

    // summary.json
    let n = rows.len() as f64;
    let mae_mean = rows.iter().map(|r| r.mae).sum::<f64>() / n;
    let maxd_max = rows.iter().map(|r| r.maxd).max().unwrap_or(0);
    let avg: Vec<f64> = (0..7)
        .map(|k| rows.iter().map(|r| r.frac[k]).sum::<f64>() / n)
        .collect();
    let mut summary = serde_json::json!({
        "agg": {
            "frames": rows.len(),
            "mae_mean": mae_mean,
            "maxd_max": maxd_max,
        },
        "rows": rows.iter().map(|r| serde_json::json!({
            "frame": r.frame,
            "mae": r.mae,
            "maxd": r.maxd,
            "p2": r.frac[0], "p4": r.frac[1], "p8": r.frac[2],
            "p16": r.frac[3], "p32": r.frac[4], "p64": r.frac[5], "p128": r.frac[6],
            "bbox": r.bbox.map(|(x0, y0, x1, y1)| serde_json::json!([x0, y0, x1, y1])),
            "grid_max_cell": r.grid_cell.map(|(x, y)| serde_json::json!([x, y])),
        })).collect::<Vec<_>>(),
    });
    for (k, t) in THRESHOLDS.iter().enumerate() {
        summary["agg"][format!("avg_p{t}")] = serde_json::json!(avg[k]);
    }
    fs::write(
        cli.out.join("summary.json"),
        serde_json::to_string_pretty(&summary)?,
    )?;

    let mut worst: Vec<&Stats> = rows.iter().collect();
    worst.sort_by(|a, b| b.frac[2].partial_cmp(&a.frac[2]).unwrap());
    println!("\n=== worst 25 frames by p8 (pixels changed > 8) ===");
    println!(
        "{:>6} {:>7} {:>5} {:>7} {:>7} {:>7} {:>7} bbox",
        "frame", "mae", "maxd", "p0", "p8", "p16", "p32"
    );
    for r in worst.iter().take(25) {
        println!(
            "{:>6} {:>7.3} {:>5} {:>7.4} {:>7.4} {:>7.4} {:>7.4} {:?}",
            r.frame,
            r.mae,
            r.maxd,
            r.frac[0] + 0.0,
            r.frac[2],
            r.frac[3],
            r.frac[4],
            r.bbox
        );
    }

    println!("\n=== aggregate ===");
    println!("{}", serde_json::to_string_pretty(&summary["agg"])?);

    if !sheet_tiles.is_empty() {
        let cols = 4usize;
        let rows_n = sheet_tiles.len().div_ceil(cols);
        let tw = w / 4;
        let th = h / 4;
        let mut canvas = vec![0u8; cols * tw as usize * rows_n * th as usize * 3];
        let stride = cols * tw as usize * 3;
        for (k, tile) in sheet_tiles.iter().enumerate() {
            let ox = (k % cols) * tw as usize;
            let oy = (k / cols) * th as usize;
            for y in 0..th as usize {
                let dst = oy * stride + y * stride + ox * 3;
                let src = y * tw as usize * 3;
                canvas[dst..dst + tw as usize * 3]
                    .copy_from_slice(&tile[src..src + tw as usize * 3]);
            }
        }
        save_png(
            &cli.out.join("sheet.png"),
            cols as u32 * tw,
            rows_n as u32 * th,
            &canvas,
        )?;
        println!("sheet -> {}", cli.out.join("sheet.png").display());
    }

    println!("artifacts -> {}/pixel.csv, summary.json", cli.out.display());
    Ok(())
}
