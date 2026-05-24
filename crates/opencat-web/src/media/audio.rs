//! WebAudio 音频解码与播放（基于 web_sys::AudioContext）。
//!
//! - 解码：`AudioContext.decode_audio_data()` 将 ArrayBuffer 解码为 AudioBuffer。
//! - 预览播放：通过 `AudioBufferSourceNode` 按时间偏移播放。
//! - 导出：从 AudioBuffer 中按时间范围提取 f32 PCM 样本。

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{AudioBuffer, AudioBufferSourceNode, AudioContext, AudioContextOptions, GainNode};

/// 解码后的音频 PCM 数据。
#[derive(Clone)]
pub struct DecodedAudio {
    pub sample_rate: u32,
    pub channels: u16,
    /// 交错排列的 f32 样本 (L,R,L,R,...)
    pub samples: Vec<f32>,
    pub duration_secs: f64,
}

/// WebAudio 音频管理器。
///
/// 持有一个全局 [`AudioContext`] 和已解码的 [`AudioBuffer`] 缓存。
pub struct WebAudio {
    ctx: AudioContext,
    gain: GainNode,
    buffers: HashMap<String, AudioBuffer>,
    pcm_cache: HashMap<String, DecodedAudio>,
    active_sources: Vec<AudioBufferSourceNode>,
}

impl WebAudio {
    pub fn new() -> Result<Self> {
        let opts = AudioContextOptions::new();
        opts.set_sample_rate(48000.0);
        let ctx = AudioContext::new_with_context_options(&opts)
            .map_err(|e| anyhow!("AudioContext: {e:?}"))?;
        let gain = ctx
            .create_gain()
            .map_err(|e| anyhow!("createGain: {e:?}"))?;
        gain.connect_with_audio_node(&ctx.destination())
            .map_err(|e| anyhow!("connect: {e:?}"))?;

        Ok(Self {
            ctx,
            gain,
            buffers: HashMap::new(),
            pcm_cache: HashMap::new(),
            active_sources: Vec::new(),
        })
    }

    /// 设置主音量 (0.0 ~ 1.0)。
    pub fn set_volume(&self, volume: f32) {
        self.gain.gain().set_value(volume);
    }

    /// 返回 AudioContext 引用（JS 侧可能需要它）。
    pub fn context(&self) -> &AudioContext {
        &self.ctx
    }

    /// 解码音频文件字节，存入缓存。
    ///
    /// `key` 用于后续查找（通常用 asset_id）。
    pub async fn decode_file(&mut self, key: &str, data: &[u8]) -> Result<()> {
        let array_buf = js_sys::Uint8Array::from(data).buffer();

        let promise = self
            .ctx
            .decode_audio_data(&array_buf)
            .map_err(|e| anyhow!("decodeAudioData: {e:?}"))?;

        let audio_buf: AudioBuffer = JsFuture::from(promise)
            .await
            .map_err(|e| anyhow!("decode await: {e:?}"))?
            .dyn_into()
            .map_err(|_| anyhow!("expected AudioBuffer"))?;

        // 提取 PCM 样本到 DecodedAudio
        let sample_rate = audio_buf.sample_rate() as u32;
        let channels = audio_buf.number_of_channels() as u16;
        let length = audio_buf.length() as usize;
        let duration = length as f64 / sample_rate as f64;

        let mut samples = Vec::new();
        for ch in 0..(channels as u32) {
            let chan_data = audio_buf
                .get_channel_data(ch)
                .map_err(|_| anyhow!("getChannelData({ch})"))?;
            let chan_vec = chan_data.to_vec();

            if channels == 1 {
                // 单声道 → 复制到左右
                samples.reserve(chan_vec.len() * 2);
                for sample in &chan_vec {
                    samples.push(*sample);
                    samples.push(*sample);
                }
            } else if ch == 0 {
                // 左声道：先分配空间
                samples = vec![0.0f32; chan_vec.len() * 2];
                for (i, sample) in chan_vec.iter().enumerate() {
                    samples[i * 2] = *sample;
                }
            } else {
                // 右声道：交错填入
                for (i, sample) in chan_vec.iter().enumerate() {
                    if i * 2 + 1 < samples.len() {
                        samples[i * 2 + 1] = *sample;
                    }
                }
            }
        }

        let decoded = DecodedAudio {
            sample_rate,
            channels: if channels == 1 { 2 } else { channels },
            samples,
            duration_secs: duration,
        };

        self.pcm_cache.insert(key.to_string(), decoded);
        self.buffers.insert(key.to_string(), audio_buf);

        Ok(())
    }

    /// 从指定时间偏移开始播放音频，持续 `duration_secs`。
    ///
    /// 用于预览时与帧时间线同步。
    pub fn play_at(&mut self, key: &str, offset_secs: f64, duration_secs: f64) -> Result<()> {
        let buffer = self
            .buffers
            .get(key)
            .ok_or_else(|| anyhow!("audio buffer not found: {key}"))?;

        let source: AudioBufferSourceNode = self
            .ctx
            .create_buffer_source()
            .map_err(|e| anyhow!("createBufferSource: {e:?}"))?;

        source.set_buffer(Some(buffer));
        source
            .connect_with_audio_node(&self.gain)
            .map_err(|e| anyhow!("connect: {e:?}"))?;

        source
            .start_with_when_and_grain_offset_and_grain_duration(0.0, offset_secs, duration_secs)
            .map_err(|e| anyhow!("start: {e:?}"))?;

        self.active_sources.push(source);

        Ok(())
    }

    /// 获取已解码的 PCM 数据。
    pub fn get_pcm(&self, key: &str) -> Option<&DecodedAudio> {
        self.pcm_cache.get(key)
    }

    /// 移除已解码的音频（释放内存）。
    pub fn remove(&mut self, key: &str) {
        self.buffers.remove(key);
        self.pcm_cache.remove(key);
    }

    /// 按时间范围提取 f32 样本（交错立体声）。
    ///
    /// 用于导出时按帧切片音频。
    pub fn extract_samples(
        pcm: &DecodedAudio,
        start_secs: f64,
        duration_secs: f64,
        target_rate: u32,
    ) -> Vec<f32> {
        let src_rate = pcm.sample_rate as f64;
        let channels = pcm.channels as usize;
        let total_frames = pcm.samples.len() / channels;
        let total_duration = total_frames as f64 / src_rate;

        if start_secs >= total_duration {
            return vec![];
        }

        let start_idx = (start_secs * src_rate) as usize * channels;
        let end_idx =
            ((start_secs + duration_secs).min(total_duration) * src_rate) as usize * channels;

        let src_slice =
            &pcm.samples[start_idx.min(pcm.samples.len())..end_idx.min(pcm.samples.len())];

        if target_rate as f64 == src_rate {
            return src_slice.to_vec();
        }

        let ratio = target_rate as f64 / src_rate;
        let out_frames = ((src_slice.len() / channels) as f64 * ratio) as usize;
        let src_frames = src_slice.len() / channels;
        let mut out = Vec::with_capacity(out_frames * channels);
        for out_frame in 0..out_frames {
            let src_pos = out_frame as f64 / ratio;
            let src_frame = src_pos as usize;
            let frac = (src_pos - src_frame as f64) as f32;
            let next_frame = (src_frame + 1).min(src_frames.saturating_sub(1));
            for ch in 0..channels {
                let a = src_slice[src_frame * channels + ch];
                let b = src_slice[next_frame * channels + ch];
                out.push(a + (b - a) * frac);
            }
        }
        out
    }

    /// 暂停 AudioContext（释放音频资源）。
    pub fn suspend(&self) -> Result<()> {
        let _ = self.ctx.suspend().map_err(|e| anyhow!("suspend: {e:?}"));
        Ok(())
    }

    /// 恢复 AudioContext（需要用户手势后才能恢复）。
    pub fn resume(&self) -> Result<()> {
        let _ = self.ctx.resume().map_err(|e| anyhow!("resume: {e:?}"));
        Ok(())
    }

    /// 返回 AudioContext.currentTime，用于音视频同步。
    pub fn current_time(&self) -> f64 {
        self.ctx.current_time()
    }

    /// 停止所有正在播放的音频，释放 source 节点。
    #[allow(deprecated)]
    pub fn stop_all(&mut self) -> Result<()> {
        for source in self.active_sources.drain(..) {
            let _ = source.stop();
        }
        Ok(())
    }
}
