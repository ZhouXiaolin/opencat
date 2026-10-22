#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn main() {
    eprintln!("opencat-see 目前仅支持 macOS / Windows / Linux");
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
mod app {
    use std::num::{NonZeroU16, NonZeroU32};
    use std::time::{Duration, Instant};

    use anyhow::{Context, Result, anyhow};
    use std::path::Path;
    use std::sync::Arc;

    use opencat::{
        EngineDrawExecutor, EngineLoader, GeneratedImageCache, MediaContext, V8JsContext,
        build_audio_track_from_pipeline, duration_secs_to_frames, generated_image_cache_from_env,
        open,
    };
    use opencat_core::script::js_context::JsContext;
    use opencat_engine::present::{PreviewFrame, PreviewPresenter, WindowHandle};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    #[cfg(target_os = "linux")]
    use raw_window_handle::{HasDisplayHandle, RawDisplayHandle};
    use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, Source};
    use winit::application::ApplicationHandler;
    use winit::dpi::PhysicalSize;
    use winit::event::WindowEvent;
    #[cfg(not(target_os = "linux"))]
    use winit::event_loop::EventLoop;
    use winit::event_loop::{ActiveEventLoop, ControlFlow};
    use winit::window::{Window, WindowAttributes};

    const AUDIO_SAMPLE_RATE: u32 = 48_000;
    const AUDIO_CHANNELS: u16 = 2;

    struct AudioRenderSource {
        samples: Arc<Vec<f32>>,
        sample_rate: NonZeroU32,
        channels: NonZeroU16,
        position: usize,
        loop_sample_frames: usize,
    }

    impl Iterator for AudioRenderSource {
        type Item = f32;

        fn next(&mut self) -> Option<Self::Item> {
            let total = self.loop_sample_frames.saturating_mul(self.channels.get() as usize);
            if total == 0 {
                return Some(0.0);
            }
            if self.position >= total {
                self.position = 0;
            }
            let sample = self.samples.get(self.position).copied().unwrap_or(0.0);
            self.position += 1;
            Some(sample)
        }
    }

    impl Source for AudioRenderSource {
        fn current_span_len(&self) -> Option<usize> {
            None
        }

        fn channels(&self) -> rodio::ChannelCount {
            self.channels
        }

        fn sample_rate(&self) -> rodio::SampleRate {
            self.sample_rate
        }

        fn total_duration(&self) -> Option<Duration> {
            None
        }
    }

    struct AudioPlayback {
        _sink: MixerDeviceSink,
        player: Player,
    }

    impl AudioPlayback {
        fn new(
            samples: Option<Arc<Vec<f32>>>,
            loop_sample_frames: usize,
        ) -> Result<Option<Self>> {
            let Some(samples) = samples else {
                return Ok(None);
            };
            if samples.is_empty() || loop_sample_frames == 0 {
                return Ok(None);
            }

            let sample_rate = NonZeroU32::new(AUDIO_SAMPLE_RATE)
                .ok_or_else(|| anyhow!("audio sample rate must be non-zero"))?;
            let channels = NonZeroU16::new(AUDIO_CHANNELS)
                .ok_or_else(|| anyhow!("audio channel count must be non-zero"))?;
            let source = AudioRenderSource {
                samples,
                sample_rate,
                channels,
                position: 0,
                loop_sample_frames,
            };

            let sink = DeviceSinkBuilder::open_default_sink()
                .context("failed to open default audio output device")?;
            let player = Player::connect_new(&sink.mixer());
            player.append(source);
            Ok(Some(Self {
                _sink: sink,
                player,
            }))
        }

        fn frame_index(&self, total_frames: u32, fps: u32) -> u32 {
            if total_frames <= 1 {
                return 0;
            }

            let fps = fps.max(1);
            let loop_secs = total_frames as f64 / fps as f64;
            if loop_secs <= f64::EPSILON {
                return 0;
            }

            let elapsed_secs = self.player.get_pos().as_secs_f64();
            let loop_pos_secs = elapsed_secs % loop_secs;
            ((loop_pos_secs * fps as f64).floor() as u32) % total_frames
        }

        fn next_redraw_deadline(&self, fps: u32) -> Instant {
            let fps = fps.max(1);
            let frame_position = self.player.get_pos().as_secs_f64() * fps as f64;
            let fractional = frame_position.fract();
            let remaining_secs = if fractional <= f64::EPSILON {
                1.0 / fps as f64
            } else {
                (1.0 - fractional) / fps as f64
            };
            Instant::now() + Duration::from_secs_f64(remaining_secs.max(0.001))
        }
    }

    /// 从 winit 窗口解出平台句柄。这是本程序唯一需要认识平台的地方——
    /// 图形上下文、Skia 后端、每帧 surface 全在 `opencat_engine::present` 里。
    fn window_handle(window: &Window) -> Result<WindowHandle> {
        let raw = window
            .window_handle()
            .map_err(|e| anyhow!("window handle unavailable: {e}"))?
            .as_raw();

        #[cfg(target_os = "windows")]
        {
            match raw {
                RawWindowHandle::Win32(handle) => Ok(WindowHandle {
                    hwnd: handle.hwnd.get() as *mut std::ffi::c_void,
                }),
                _ => Err(anyhow!("expected Win32 window handle on Windows")),
            }
        }

        #[cfg(target_os = "macos")]
        {
            match raw {
                RawWindowHandle::AppKit(handle) => Ok(WindowHandle {
                    ns_view: handle.ns_view.as_ptr(),
                    scale_factor: window.scale_factor(),
                }),
                _ => Err(anyhow!("expected AppKit window handle on macOS")),
            }
        }

        #[cfg(target_os = "linux")]
        {
            let display = match window
                .display_handle()
                .map_err(|e| anyhow!("display handle unavailable: {e}"))?
                .as_raw()
            {
                RawDisplayHandle::Xlib(handle) => handle
                    .display
                    .ok_or_else(|| anyhow!("Xlib display handle is null"))?
                    .as_ptr(),
                _ => {
                    return Err(anyhow!(
                        "opencat-see on Linux currently requires X11 (force the winit X11 backend)"
                    ));
                }
            };
            match raw {
                RawWindowHandle::Xlib(handle) => Ok(WindowHandle {
                    x11_display: display,
                    x11_window: handle.window as u64,
                }),
                _ => Err(anyhow!(
                    "expected Xlib window handle on Linux; Wayland is not supported yet"
                )),
            }
        }
    }

    struct SeeApp {
        pipeline: opencat::EnginePipeline,
        media_ctx: MediaContext,
        executor: EngineDrawExecutor,
        generated_cache: GeneratedImageCache,
        audio_samples: Option<Arc<Vec<f32>>>,
        loop_sample_frames: usize,
        win_w: u32,
        win_h: u32,
        width: i32,
        height: i32,
        /// 合成画布的缩放（见 `run()`）；1.0 表示按合成原分辨率预览。
        render_scale: f32,
        total_frames: u32,
        fps: u32,
        frame_duration: Duration,
        window: Option<Window>,
        presenter: Option<PreviewPresenter>,
        audio_playback: Option<AudioPlayback>,
        next_tick: Instant,
        fallback_frame_index: u32,
    }

    impl SeeApp {
        fn preview_frame(
            &mut self,
            presenter: &mut PreviewPresenter,
            frame_index: u32,
        ) -> Result<()> {
            presenter.render_frame(PreviewFrame {
                pipeline: &mut self.pipeline,
                media_ctx: &mut self.media_ctx,
                executor: &mut self.executor,
                generated_cache: &mut self.generated_cache,
                width: self.width,
                height: self.height,
                frame_index,
                render_scale: self.render_scale,
            })
        }
    }

    impl ApplicationHandler for SeeApp {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.window.is_some() {
                return;
            }

            #[cfg(target_os = "linux")]
            {
                // EGL path needs an X11 display handle (pure Wayland wl_surface
                // support would require wayland-egl). Backend forced via
                // EventLoopBuilder::with_x11 below.
                let x11_ok = event_loop
                    .display_handle()
                    .map_err(|e| anyhow!("display handle unavailable: {e}"))
                    .and_then(|handle| match handle.as_raw() {
                        RawDisplayHandle::Xlib(xlib) => {
                            if xlib.display.is_none() {
                                Err(anyhow!("Xlib display handle is null"))
                            } else {
                                Ok(())
                            }
                        }
                        _ => Err(anyhow!(
                            "opencat-see on Linux currently requires X11 (set WAYLAND_DISPLAY= to force X11)"
                        )),
                    });
                if let Err(error) = x11_ok {
                    eprintln!("failed to start preview: {error:#}");
                    event_loop.exit();
                    return;
                }
            }

            let attributes = WindowAttributes::default()
                .with_title("OpenCat See")
                .with_inner_size(PhysicalSize::new(self.win_w, self.win_h))
                .with_resizable(false);
            let window = match event_loop.create_window(attributes) {
                Ok(window) => window,
                Err(error) => {
                    eprintln!("failed to create window: {error}");
                    event_loop.exit();
                    return;
                }
            };
            // 呈现器把 wgpu 加入本窗口的图形上下文，再把 3D 渲染器注入管线——
            // 这样 3D 输出纹理对本节点的 Skia 有效，合成时零拷贝导入。必须在
            // 首帧渲染前完成（`preset` 只设一次）。后端不支持时保持懒初始化
            //（走 CPU 读回）。
            let mut presenter = match window_handle(&window).and_then(|handle| {
                unsafe { PreviewPresenter::for_window(handle, &self.pipeline, self.width, self.height) }
            }) {
                Ok(presenter) => presenter,
                Err(error) => {
                    eprintln!("failed to create render target: {error:#}");
                    event_loop.exit();
                    return;
                }
            };
            if let Err(error) = self.preview_frame(&mut presenter, 0) {
                eprintln!("failed to render warmup frame: {error:#}");
                event_loop.exit();
                return;
            }
            std::thread::sleep(Duration::from_millis(100));

            self.audio_playback = match AudioPlayback::new(
                self.audio_samples.clone(),
                self.loop_sample_frames,
            ) {
                Ok(playback) => playback,
                Err(error) => {
                    eprintln!("failed to start audio playback: {error:#}");
                    event_loop.exit();
                    return;
                }
            };
            self.next_tick = self
                .audio_playback
                .as_ref()
                .map(|playback| playback.next_redraw_deadline(self.fps))
                .unwrap_or_else(|| Instant::now() + self.frame_duration);
            self.fallback_frame_index = 1 % self.total_frames;
            self.window = Some(window);
            self.presenter = Some(presenter);
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            _window_id: winit::window::WindowId,
            event: WindowEvent,
        ) {
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::RedrawRequested => {
                    // 借出呈现器（`&mut self` 的 `preview_frame` 需要独占 self），
                    // 画完再放回。
                    let Some(mut presenter) = self.presenter.take() else {
                        return;
                    };
                    let frame_index = self
                        .audio_playback
                        .as_ref()
                        .map(|playback| playback.frame_index(self.total_frames, self.fps))
                        .unwrap_or(self.fallback_frame_index);
                    let result = self.preview_frame(&mut presenter, frame_index);
                    self.presenter = Some(presenter);
                    if let Err(error) = result {
                        eprintln!("render error: {error:#}");
                        event_loop.exit();
                        return;
                    }
                    if let Some(playback) = self.audio_playback.as_ref() {
                        self.next_tick = playback.next_redraw_deadline(self.fps);
                    } else {
                        self.fallback_frame_index = (frame_index + 1) % self.total_frames;
                        self.next_tick = Instant::now() + self.frame_duration;
                    }
                }
                _ => {}
            }
        }

        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            if self.window.is_none() {
                return;
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_tick));
            if let Some(window) = self.window.as_ref() {
                if Instant::now() >= self.next_tick {
                    window.request_redraw();
                }
            }
        }
    }

    pub fn run() -> Result<()> {
        // `--base-dir <DIR>`：输入是 dist/ 里的构建产物时，资产/字体相对路径
        // 仍按工程目录解析。缺省 = 输入文件所在目录。
        let mut args = std::env::args().skip(1);
        let mut input_arg: Option<String> = None;
        let mut base_dir_arg: Option<String> = None;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--base-dir" => base_dir_arg = args.next(),
                _ if input_arg.is_none() => input_arg = Some(arg),
                other => return Err(anyhow!("unexpected argument `{other}`")),
            }
        }
        let input_path =
            input_arg.ok_or_else(|| anyhow!("usage: opencat-see <input.jsx> [--base-dir <DIR>]"))?;
        let input_path = Path::new(&input_path);
        let source_text =
            std::fs::read_to_string(input_path).context("failed to read composition source")?;
        let base_dir = match base_dir_arg {
            Some(dir) => Path::new(&dir).to_path_buf(),
            None => input_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf(),
        };
        let cache_base =
            dirs::home_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let cache_dir = cache_base.join(".opencat").join("assets");
        let loader = EngineLoader::new(base_dir, cache_dir).context("failed to create loader")?;
        let ctx = V8JsContext::new().context("failed to create js context")?;
        let mut pipeline =
            open(&source_text, loader, ctx).context("failed to open pipeline")?;
        let info = pipeline.info().clone();
        let total_frames = duration_secs_to_frames(info.duration, info.fps).max(1);
        let fps = info.fps.max(1);
        // 预览缩放，取值 (0, 1]，由 `OC_SEE_SCALE` 指定。默认 1.0（原分辨率）：
        // 实测合成只占每帧 ~3.5ms，瓶颈在视频解码，降分辨率收益很小。
        let scale = std::env::var("OC_SEE_SCALE")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|s| *s > 0.0 && *s <= 1.0)
            .unwrap_or(1.0);
        let width = ((info.width as f32) * scale).round().max(1.0) as i32;
        let height = ((info.height as f32) * scale).round().max(1.0) as i32;

        let mut media_ctx = MediaContext::new();
        media_ctx.set_composition_fps(info.fps);
        // 播放时视频解码是瓶颈（一帧 1080p 软解可达数十毫秒），所以把解码挪到
        // 后台线程，按 web 预览器那套「播放头前后各留一段帧」的窗口持续预取。
        media_ctx.enable_realtime_decode();
        // 预热所有视频素材：第一次解一个 1080p 素材要 ~300ms（探测占大头），落在
        // 播放中间就是一帧卡顿，还会连锁触发跳帧。这里用开播前的时间做掉。两边都要
        // 预热——解码线程和本线程各有自己的解码器和元数据缓存。
        let video_assets: Vec<_> = pipeline
            .pipeline()
            .catalog()
            .videos
            .keys()
            .cloned()
            .collect();
        for asset_id in &video_assets {
            let Some(path) = pipeline
                .loader()
                .handle(asset_id)
                .and_then(|h| h.local_path())
                .map(Path::to_path_buf)
            else {
                continue;
            };
            media_ctx.warmup_video(&path);
            let _ = media_ctx.video_info(&path);
        }
        let executor = EngineDrawExecutor::new();
        let generated_cache = generated_image_cache_from_env();

        let audio_track = build_audio_track_from_pipeline(&pipeline)
            .context("failed to premix audio track")?;
        let loop_sample_frames = composition_sample_frames(total_frames, fps, NonZeroU32::new(AUDIO_SAMPLE_RATE).unwrap());
        let audio_samples = audio_track.map(|track| Arc::new(track.samples));

        #[cfg(target_os = "linux")]
        let event_loop = {
            use winit::event_loop::EventLoopBuilder;
            use winit::platform::x11::EventLoopBuilderExtX11;
            let mut builder = EventLoopBuilder::new();
            // Force the winit X11 backend (see resumed()).
            builder.with_x11();
            builder
                .build()
                .map_err(|e| anyhow!("failed to create event loop: {e}"))?
        };
        #[cfg(not(target_os = "linux"))]
        let event_loop = EventLoop::new().map_err(|e| anyhow!("failed to create event loop: {e}"))?;

        let mut app = SeeApp {
            pipeline,
            media_ctx,
            executor,
            generated_cache,
            audio_samples,
            loop_sample_frames,
            win_w: width as u32,
            win_h: height as u32,
            width,
            height,
            render_scale: scale,
            total_frames,
            fps,
            frame_duration: Duration::from_secs_f64(1.0 / fps as f64),
            window: None,
            presenter: None,
            audio_playback: None,
            next_tick: Instant::now(),
            fallback_frame_index: 1 % total_frames,
        };

        event_loop
            .run_app(&mut app)
            .map_err(|e| anyhow!("event loop error: {e}"))
    }

    fn composition_sample_frames(frames: u32, fps: u32, sample_rate: NonZeroU32) -> usize {
        ((frames as u64 * sample_rate.get() as u64) / fps.max(1) as u64) as usize
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn main() -> anyhow::Result<()> {
    app::run()
}
