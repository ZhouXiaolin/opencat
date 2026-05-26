// 1:1 port of faac/libfaac/frame.{h,c} — encoder open / encode / close.

#![allow(dead_code)]

use crate::analysis::FftTables;
use crate::analysis::FilterBankBuffers;
use crate::analysis::{GlobalPsyInfo, PsyInfo, block_switch, psy_calculate};
use crate::bitstream::BitStream;
use crate::bitstream::{ADTS_FRAMESIZE, FrameCtx, write_bitstream};
use crate::codec::{ChannelInfo, ElementType};
use crate::codec::{
    CoderInfo, Configuration, FRAME_LEN, InputFormat, JointMode, LOW, MAX_CHANNELS, MPEG2, MPEG4,
    ShortControl, SrInfo, StreamFormat, WindowShape, WindowType,
};
use crate::coding::aac_stereo;
use crate::coding::{AACQuantCfg, MAXQUAL, MAXQUALADTS, MINQUAL, quantize_init};
use crate::tables::sr_info_table;
use crate::util::{get_sr_index, max_bitrate};

const RC_DEADBAND_THRESHOLD: f64 = 0.05;
const RC_DAMPING_FACTOR: f64 = 0.6;
const DEFQUAL: i32 = 100;

pub const LIBFAAC_NAME: &str = "1.50.0";

fn calc_bandwidth(bit_rate: u64, sample_rate: u64) -> u32 {
    let nyquist = (sample_rate / 2) as u32;
    if bit_rate == 0 {
        return nyquist;
    }
    let bw = if bit_rate <= 16000 {
        4000 + (bit_rate / 8) as u32
    } else if bit_rate <= 32000 {
        6000 + (((bit_rate - 16000) * 5 / 16) as u32)
    } else if bit_rate <= 64000 {
        11000 + (((bit_rate - 32000) * 15 / 64) as u32)
    } else if bit_rate <= 128000 {
        18500 + (((bit_rate - 64000) * 3 / 128) as u32)
    } else {
        let v = 20000 + ((bit_rate - 128000) / 16) as u32;
        if v > 20000 { 20000 } else { v }
    };
    if bw > nyquist { nyquist } else { bw }
}

pub struct FrameEncoder {
    pub num_channels: u32,
    pub sample_rate: u32,
    pub sample_rate_idx: u32,
    pub used_bytes: u32,
    pub frame_num: u32,
    pub flush_frame: u32,
    pub sr_info: SrInfo,
    pub sample_buff: Vec<Vec<f64>>,
    pub next3_sample_buff: Vec<Vec<f64>>,
    pub fb: FilterBankBuffers,
    pub coder_info: Vec<CoderInfo>,
    pub channel_info: Vec<ChannelInfo>,
    pub psy_info: Vec<PsyInfo>,
    pub gpsy_info: GlobalPsyInfo,
    pub config: Configuration,
    pub aacquant_cfg: AACQuantCfg,
    pub fft_tables: FftTables,
    pub name: String,
}

pub struct OpenInfo {
    pub input_samples: u32,
    pub max_output_bytes: u32,
}

impl FrameEncoder {
    pub fn open(sample_rate: u32, num_channels: u32) -> Option<(Self, OpenInfo)> {
        if num_channels > MAX_CHANNELS as u32 {
            return None;
        }
        let info = OpenInfo {
            input_samples: FRAME_LEN as u32 * num_channels,
            max_output_bytes: ADTS_FRAMESIZE as u32,
        };

        let sample_rate_idx = get_sr_index(sample_rate) as u32;
        let sr_info = sr_info_table()[sample_rate_idx as usize].clone();

        let mut config = Configuration::default();
        config.mpeg_version = MPEG4;
        config.aac_object_type = LOW;
        config.jointmode = JointMode::Is;
        config.pnslevel = 4;
        config.use_lfe = true;
        config.use_tns = false;
        config.bit_rate = 64000;
        config.band_width = calc_bandwidth(config.bit_rate, sample_rate as u64);
        config.quantqual = 0;
        config.psymodelidx = 0;
        config.shortctl = ShortControl::Normal;
        config.output_format = StreamFormat::Adts;
        config.input_format = InputFormat::I32;

        let mut coder_info = vec![CoderInfo::default(); num_channels as usize];
        for c in coder_info.iter_mut() {
            c.prev_window_shape = WindowShape::Sine;
            c.window_shape = WindowShape::Sine;
            c.block_type = WindowType::OnlyLongWindow;
            c.groups.n = 1;
            c.groups.len[0] = 1;
        }

        let channel_info = vec![ChannelInfo::default(); num_channels as usize];

        let fft_tables = FftTables::new();

        let fb = FilterBankBuffers::new(num_channels as usize);

        let mut psy_info: Vec<PsyInfo> = (0..num_channels).map(|_| PsyInfo::default()).collect();
        let gpsy_info = GlobalPsyInfo::new(&mut psy_info, num_channels, sample_rate);

        let mut enc = Self {
            num_channels,
            sample_rate,
            sample_rate_idx,
            used_bytes: 0,
            frame_num: 0,
            flush_frame: 0,
            sr_info,
            sample_buff: (0..num_channels).map(|_| Vec::new()).collect(),
            next3_sample_buff: (0..num_channels).map(|_| vec![0.0f64; FRAME_LEN]).collect(),
            fb,
            coder_info,
            channel_info,
            psy_info,
            gpsy_info,
            config,
            aacquant_cfg: AACQuantCfg::default(),
            fft_tables,
            name: LIBFAAC_NAME.to_string(),
        };

        // TNS profile-dependent params.
        for ci in &mut enc.coder_info {
            ci.tns_info.init(sample_rate_idx as usize);
        }
        quantize_init();

        Some((enc, info))
    }

    pub fn set_configuration(&mut self, mut new_config: Configuration) -> bool {
        let maxqual = match self.config.output_format {
            StreamFormat::Adts => MAXQUALADTS,
            StreamFormat::Raw => MAXQUAL,
        };

        // Validate input format.
        match new_config.input_format {
            InputFormat::I16 | InputFormat::I32 | InputFormat::F32 => {}
            _ => return false,
        }
        if new_config.aac_object_type != LOW {
            return false;
        }

        self.config.jointmode = new_config.jointmode;
        self.config.use_lfe = new_config.use_lfe;
        self.config.use_tns = new_config.use_tns;
        self.config.aac_object_type = new_config.aac_object_type;
        self.config.mpeg_version = new_config.mpeg_version;
        self.config.output_format = new_config.output_format;
        self.config.input_format = new_config.input_format;
        self.config.shortctl = new_config.shortctl;

        for ci in &mut self.coder_info {
            ci.tns_info.init(self.sample_rate_idx as usize);
        }

        if self.sample_rate == 0 || self.num_channels == 0 {
            return false;
        }
        let max_b = max_bitrate(self.sample_rate) / self.num_channels as u64;
        if new_config.bit_rate > max_b {
            new_config.bit_rate = max_b;
        }

        if new_config.bit_rate != 0 && new_config.band_width == 0 {
            new_config.band_width = calc_bandwidth(new_config.bit_rate, self.sample_rate as u64);
            if new_config.quantqual == 0 {
                let mut q = new_config.bit_rate as f64 * self.num_channels as f64 / 1280.0;
                if q > DEFQUAL as f64 {
                    q = (q - DEFQUAL as f64) * 3.0 + DEFQUAL as f64;
                }
                new_config.quantqual = q as u64;
            }
        }
        if new_config.quantqual == 0 {
            new_config.quantqual = DEFQUAL as u64;
        }

        self.config.bit_rate = new_config.bit_rate;

        if new_config.band_width == 0 {
            new_config.band_width = calc_bandwidth(new_config.bit_rate, self.sample_rate as u64);
        }
        self.config.band_width = new_config.band_width;
        if self.config.band_width < 100 {
            self.config.band_width = 100;
        }
        if self.config.band_width > self.sample_rate / 2 {
            self.config.band_width = self.sample_rate / 2;
        }

        if new_config.quantqual as i32 > maxqual {
            new_config.quantqual = maxqual as u64;
        }
        if (new_config.quantqual as i32) < MINQUAL {
            new_config.quantqual = MINQUAL as u64;
        }
        self.config.quantqual = new_config.quantqual;

        if new_config.mpeg_version == MPEG2 {
            new_config.pnslevel = 0;
        }
        if new_config.pnslevel < 0 {
            new_config.pnslevel = 0;
        }
        if new_config.pnslevel > 10 {
            new_config.pnslevel = 10;
        }
        self.aacquant_cfg.pnslevel = new_config.pnslevel;
        self.aacquant_cfg.quality = new_config.quantqual as f64;

        let mut bw = self.config.band_width;
        self.aacquant_cfg
            .calc_bw(&mut bw, self.sample_rate as i32, &self.sr_info);
        self.config.band_width = bw;

        // channel_map copy.
        for i in 0..MAX_CHANNELS {
            self.config.channel_map[i] = new_config.channel_map[i];
        }

        true
    }

    /// Encode one input chunk. `input` is interleaved samples in the format
    /// chosen by `config.input_format`. The number of samples in `input` is
    /// `num_channels * samples_per_channel`. Returns number of bytes written
    /// to `output`, 0 if more input is needed, or negative on error.
    pub fn encode(&mut self, input: &[i32], samples_input: usize, output: &mut [u8]) -> i32 {
        self.frame_num += 1;
        if samples_input == 0 {
            self.flush_frame += 1;
        }
        if self.flush_frame > 4 {
            return 0;
        }

        let num_channels = self.num_channels as usize;
        ChannelInfo::assign_elements(
            &mut self.channel_info,
            num_channels as i32,
            self.config.use_lfe,
        );

        self.ingest_samples(input, samples_input);

        if self.frame_num <= 3 {
            return 0;
        }

        self.psychoacoustic_analysis();
        self.apply_filterbank();
        self.setup_sfb_offsets();
        self.apply_tns();
        self.apply_stereo();
        self.quantize_channels();

        let frame_bytes = self.write_output(output);
        if frame_bytes < 0 {
            return -1;
        }

        self.rate_control(frame_bytes);
        frame_bytes
    }

    fn ingest_samples(&mut self, input: &[i32], samples_input: usize) {
        let num_channels = self.num_channels as usize;
        let band_width = self.config.band_width;

        for channel in 0..num_channels {
            if self.sample_buff[channel].is_empty() {
                self.sample_buff[channel] = vec![0.0f64; FRAME_LEN];
            }
            std::mem::swap(
                &mut self.sample_buff[channel],
                &mut self.next3_sample_buff[channel],
            );

            let dst = &mut self.next3_sample_buff[channel];
            if samples_input == 0 {
                dst.fill(0.0);
            } else {
                let spc = samples_input / num_channels;
                let ch_map = self.config.channel_map[channel] as usize;
                match self.config.input_format {
                    InputFormat::I16 => {
                        for i in 0..spc {
                            let s = input[i * num_channels + ch_map] as i16;
                            dst[i] = s as f64;
                        }
                    }
                    InputFormat::I32 => {
                        for i in 0..spc {
                            let s = input[i * num_channels + ch_map];
                            dst[i] = (1.0 / 256.0) * (s as f64);
                        }
                    }
                    InputFormat::F32 => {
                        for i in 0..spc {
                            let s = f32::from_bits(input[i * num_channels + ch_map] as u32);
                            dst[i] = s as f64;
                        }
                    }
                    _ => {}
                }
                dst[spc..FRAME_LEN].fill(0.0);
            }

            if self.channel_info[channel].element_type != ElementType::Lfe {
                self.psy_info[channel].buffer_update(
                    &self.fft_tables,
                    &mut self.gpsy_info,
                    &self.next3_sample_buff[channel],
                    band_width,
                    &self.sr_info.cb_width_short,
                    self.sr_info.num_cb_short,
                );
            }
        }
    }

    fn psychoacoustic_analysis(&mut self) {
        let num_channels = self.num_channels;
        let shortctl = self.config.shortctl;

        psy_calculate(
            &self.channel_info,
            &mut self.psy_info,
            num_channels,
            self.aacquant_cfg.quality / DEFQUAL as f64,
        );

        block_switch(&mut self.coder_info, &self.psy_info, num_channels);

        if shortctl == ShortControl::NoShort {
            for c in &mut self.coder_info {
                c.block_type = WindowType::OnlyLongWindow;
            }
        } else if self.frame_num <= 4 || shortctl == ShortControl::NoLong {
            for c in &mut self.coder_info {
                c.block_type = WindowType::OnlyShortWindow;
            }
        }
    }

    fn apply_filterbank(&mut self) {
        for channel in 0..self.num_channels as usize {
            self.fb.process_channel(
                &self.fft_tables,
                &self.coder_info[channel],
                &self.sample_buff[channel],
                channel,
            );
        }
    }

    fn setup_sfb_offsets(&mut self) {
        for channel in 0..self.num_channels as usize {
            self.channel_info[channel].ms_info.is_present = false;

            if self.coder_info[channel].block_type == WindowType::OnlyShortWindow {
                self.coder_info[channel].sfbn = self.aacquant_cfg.max_cbs;
                let sfbn = self.coder_info[channel].sfbn as usize;
                let mut offset = 0i32;
                for sb in 0..sfbn {
                    self.coder_info[channel].sfb_offset[sb] = offset;
                    offset += self.sr_info.cb_width_short[sb];
                }
                self.coder_info[channel].sfb_offset[sfbn] = offset;
                self.coder_info[channel]
                    .group_short_blocks(&mut self.fb.freq_buff[channel], &self.aacquant_cfg);
            } else {
                self.coder_info[channel].sfbn = self.aacquant_cfg.max_cbl;
                self.coder_info[channel].groups.n = 1;
                self.coder_info[channel].groups.len[0] = 1;
                let sfbn = self.coder_info[channel].sfbn as usize;
                let mut offset = 0i32;
                for sb in 0..sfbn {
                    self.coder_info[channel].sfb_offset[sb] = offset;
                    offset += self.sr_info.cb_width_long[sb];
                }
                self.coder_info[channel].sfb_offset[sfbn] = offset;
            }
        }
    }

    fn apply_tns(&mut self) {
        let use_tns = self.config.use_tns;
        for channel in 0..self.num_channels as usize {
            if self.channel_info[channel].element_type != ElementType::Lfe && use_tns {
                let ci = &mut self.coder_info[channel];
                let sfbn = ci.sfbn;
                let block_type = ci.block_type;
                let sfb_offset = ci.sfb_offset;
                ci.tns_info.encode(
                    sfbn,
                    sfbn,
                    block_type,
                    &sfb_offset,
                    &mut self.fb.freq_buff[channel],
                    &mut self.fb.work_long,
                );
            } else {
                self.coder_info[channel].tns_info.tns_data_present = false;
            }
        }

        for channel in 0..self.num_channels as usize {
            if self.channel_info[channel].element_type == ElementType::Lfe {
                self.coder_info[channel].sfbn = 3;
            }
        }
    }

    fn apply_stereo(&mut self) {
        aac_stereo(
            &mut self.coder_info,
            &mut self.channel_info,
            &mut self.fb.freq_buff,
            self.num_channels as i32,
            self.aacquant_cfg.quality / DEFQUAL as f64,
            self.config.jointmode,
        );
    }

    fn quantize_channels(&mut self) {
        for channel in 0..self.num_channels as usize {
            self.coder_info[channel].quantize(&self.fb.freq_buff[channel], &self.aacquant_cfg);
        }

        for channel in 0..self.num_channels as usize {
            if self.channel_info[channel].present
                && self.channel_info[channel].element_type == ElementType::Cpe
                && self.channel_info[channel].ch_is_left
            {
                let rch = self.channel_info[channel].paired_ch as usize;
                let m = self.coder_info[channel].sfbn.max(self.coder_info[rch].sfbn);
                self.coder_info[channel].sfbn = m;
                self.coder_info[rch].sfbn = m;
            }
        }
    }

    fn write_output(&mut self, output: &mut [u8]) -> i32 {
        let mut bit_stream = BitStream::open(output);
        let mut ctx = FrameCtx {
            output_format: self.config.output_format,
            mpeg_version: self.config.mpeg_version,
            aac_object_type: self.config.aac_object_type,
            sample_rate_idx: self.sample_rate_idx,
            num_channels: self.num_channels,
            frame_num: self.frame_num,
            used_bytes: 0,
            name: &self.name,
        };
        let bits = write_bitstream(
            &mut ctx,
            &self.coder_info,
            &self.channel_info,
            &mut bit_stream,
            self.num_channels as i32,
        );
        if bits < 0 {
            return -1;
        }
        self.used_bytes = ctx.used_bytes;
        bit_stream.close() as i32
    }

    fn rate_control(&mut self, frame_bytes: i32) {
        if self.config.bit_rate == 0 {
            return;
        }
        let maxqual = match self.config.output_format {
            StreamFormat::Adts => MAXQUALADTS,
            StreamFormat::Raw => MAXQUAL,
        };
        let desbits = (self.num_channels as i64 * (self.config.bit_rate as i64 * FRAME_LEN as i64))
            / self.sample_rate as i64;
        let mut fix = desbits as f64 / (frame_bytes as f64 * 8.0);
        if fix < 1.0 - RC_DEADBAND_THRESHOLD {
            fix += RC_DEADBAND_THRESHOLD;
        } else if fix > 1.0 + RC_DEADBAND_THRESHOLD {
            fix -= RC_DEADBAND_THRESHOLD;
        } else {
            fix = 1.0;
        }
        fix = (fix - 1.0) * RC_DAMPING_FACTOR + 1.0;
        self.aacquant_cfg.quality *= fix;
        self.aacquant_cfg.quality = self
            .aacquant_cfg
            .quality
            .clamp(MINQUAL as f64, maxqual as f64);
    }
}
