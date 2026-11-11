use std::path::{Path, PathBuf};

use opencat_engine::render::{
    EncodingConfig, probe_composition_with_base, release_gl_sessions,
    render_from_source_with_base, render_single_frame_png_with_base,
};

pub fn render(
    input: PathBuf,
    output: Option<String>,
    frame: Option<u32>,
    base_dir: Option<PathBuf>,
    frames: Option<String>,
    jobs: Option<usize>,
    segment: Option<String>,
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
    } else if let Some(spec) = segment {
        let list = parse_frames(&spec)?;
        let (start, end) = match (list.first(), list.last()) {
            (Some(&start), Some(&last)) => (start, last + 1),
            _ => return Err(anyhow::anyhow!("empty --segment `{spec}`")),
        };
        let config = EncodingConfig::mp4_segment(start..end);
        render_from_source_with_base(&source, base_dir, &output, &config)
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
        match jobs.filter(|&jobs| jobs > 1) {
            Some(jobs) => {
                println!("Rendering {} -> {} ({jobs} jobs)", input.display(), output);
                render_chunked(&input, &source, base_dir, &output, jobs)
            }
            None => {
                println!("Rendering {} -> {}", input.display(), output);
                let config = EncodingConfig::mp4();
                opencat_core::profile::run_from_env(|| {
                    render_from_source_with_base(&source, base_dir, &output, &config)
                })
            }
        }
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

/// 每段至少这么多帧：段首要多付一次提取和一帧预热。
const MIN_SEGMENT_FRAMES: u32 = 30;

/// 分块并行渲染：帧是 `f(t)`，各段互不依赖。段数取并发数的两倍——重场景
/// 常集中在片段里，等长切分会让慢段拖住整体；工作队列把先完成的槽位让给
/// 后面的段。每段由子进程（同一可执行文件的 `--segment`）渲成纯视频，
/// 父进程按序拼接并统一编码音轨。
fn render_chunked(
    input: &Path,
    source: &str,
    base_dir: Option<&Path>,
    output: &str,
    jobs: usize,
) -> anyhow::Result<()> {
    use std::process::{Child, Command, Stdio};

    let (frame_count, fps, audio_track) = probe_composition_with_base(source, base_dir)?;
    let segments = (jobs as u32 * 2).min(frame_count / MIN_SEGMENT_FRAMES).max(1);
    let seg_dir = PathBuf::from(format!("{output}.segments"));
    std::fs::create_dir_all(&seg_dir)?;
    let exe = std::env::current_exe()?;
    let seg_path = |k: u32| seg_dir.join(format!("seg_{k:03}.mp4"));

    let mut pending: std::collections::VecDeque<u32> = (0..segments).collect();
    let mut running: Vec<(u32, Child)> = Vec::new();
    let kill_all = |running: &mut Vec<(u32, Child)>| {
        for (_, child) in running.iter_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    };
    while !pending.is_empty() || !running.is_empty() {
        while running.len() < jobs
            && let Some(k) = pending.pop_front()
        {
            let start = frame_count * k / segments;
            let end = frame_count * (k + 1) / segments;
            let log = std::fs::File::create(seg_dir.join(format!("seg_{k:03}.log")))?;
            let mut command = Command::new(&exe);
            command
                .arg("render")
                .arg(input)
                .arg(seg_path(k))
                .arg("--segment")
                .arg(format!("{start}:{end}"))
                .stdout(Stdio::null())
                .stderr(log);
            if let Some(dir) = base_dir {
                command.arg("--base-dir").arg(dir);
            }
            running.push((k, command.spawn()?));
        }
        let mut index = 0;
        while index < running.len() {
            match running[index].1.try_wait()? {
                Some(status) => {
                    let (k, _) = running.remove(index);
                    if !status.success() {
                        kill_all(&mut running);
                        anyhow::bail!(
                            "segment {k} failed ({status}); log: {}",
                            seg_dir.join(format!("seg_{k:03}.log")).display()
                        );
                    }
                    println!("segment {k}/{segments} done");
                }
                None => index += 1,
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    let paths: Vec<PathBuf> = (0..segments).map(seg_path).collect();
    opencat_engine::media::concat_video_segments(&paths, output, fps, audio_track.as_ref())?;
    std::fs::remove_dir_all(&seg_dir)?;
    Ok(())
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
