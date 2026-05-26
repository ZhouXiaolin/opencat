// 1:1 port of faac/libfaac/blockswitch.{h,c}.
//
// The original C file mixes the psychoacoustic model (`psymodel2`) and the
// long/short block-switching driver; we keep the same layout in Rust. The
// model is the only one registered with the encoder, so we expose its
// functions directly instead of replicating the `psymodel_t` vtable.

#![allow(dead_code)]

use crate::codec::{ChannelInfo, ElementType};
use crate::codec::{BLOCK_LEN_LONG, BLOCK_LEN_SHORT, CoderInfo, NSFB_SHORT, WindowType};
use crate::analysis::FftTables;
use crate::analysis::mdct;

// Energies are stored as `float` (f32) in the C original via `typedef float
// psyfloat`, even when faac_real is double. Match that for bit-exactness.
pub type Psyfloat = f32;

#[derive(Default)]
pub struct PsyData {
    pub band_s: i32,
    pub lastband: i32,
    pub eng_prev: [[Psyfloat; NSFB_SHORT]; 8],
    pub eng: [[Psyfloat; NSFB_SHORT]; 8],
    pub eng_next: [[Psyfloat; NSFB_SHORT]; 8],
    pub eng_next2: [[Psyfloat; NSFB_SHORT]; 8],
}

pub struct PsyInfo {
    pub size: i32,
    pub size_s: i32,
    pub prev_samples: [f64; BLOCK_LEN_LONG],
    pub block_type: WindowType,
    pub data: PsyData,
}

impl Default for PsyInfo {
    fn default() -> Self {
        Self {
            size: 0,
            size_s: 0,
            prev_samples: [0.0; BLOCK_LEN_LONG],
            block_type: WindowType::default(),
            data: PsyData::default(),
        }
    }
}

#[derive(Default)]
pub struct GlobalPsyInfo {
    pub sample_rate: f64,
    pub hann_window: Vec<f64>,
    pub hann_window_s: Vec<f64>,
    pub shared_work_buff_long: Vec<f64>,
    pub shared_work_buff_short: Vec<f64>,
    pub mdct_xr: Vec<f64>,
    pub mdct_xi: Vec<f64>,
}

impl PsyInfo {
    fn check_short(&mut self, quality: f64) {
        const PREVS: usize = 2;
        const NEXTS: usize = 2;

        let lastband = self.data.lastband as usize;
        let firstband: usize = 2;
        self.block_type = WindowType::OnlyLongWindow;

        let mut lasteng: Option<&[Psyfloat]> = None;
        for win in 0..(PREVS + 8 + NEXTS) {
            let eng: &[Psyfloat] = if win < PREVS {
                &self.data.eng_prev[win + 8 - PREVS]
            } else if win < (PREVS + 8) {
                &self.data.eng[win - PREVS]
            } else {
                &self.data.eng_next[win - PREVS - 8]
            };

            if let Some(last) = lasteng {
                let mut toteng = 0.0f64;
                let mut volchg = 0.0f64;
                for sfb in firstband..lastband {
                    let e = eng[sfb] as f64;
                    let le = last[sfb] as f64;
                    toteng += if e < le { e } else { le };
                    volchg += (e - le).abs();
                }
                if (volchg / toteng) * quality > 3.0 {
                    self.block_type = WindowType::OnlyShortWindow;
                    break;
                }
            }
            lasteng = Some(eng);
        }
    }
}

impl GlobalPsyInfo {
    pub fn new(
        psy: &mut [PsyInfo],
        num_channels: u32,
        sample_rate: u32,
    ) -> Self {
        let hann_window: Vec<f64> = (0..BLOCK_LEN_LONG * 2)
            .map(|i| {
                let arg =
                    2.0 * std::f64::consts::PI * (i as f64 + 0.5) / (BLOCK_LEN_LONG * 2) as f64;
                0.5 * (1.0 - arg.cos())
            })
            .collect();
        let hann_window_s: Vec<f64> = (0..BLOCK_LEN_SHORT * 2)
            .map(|i| {
                let arg =
                    2.0 * std::f64::consts::PI * (i as f64 + 0.5) / (BLOCK_LEN_SHORT * 2) as f64;
                0.5 * (1.0 - arg.cos())
            })
            .collect();

        for channel in 0..num_channels as usize {
            let p = &mut psy[channel];
            p.size = BLOCK_LEN_LONG as i32;
            p.size_s = BLOCK_LEN_SHORT as i32;
            p.prev_samples = [0.0f64; BLOCK_LEN_LONG];
        }

        Self {
            sample_rate: sample_rate as f64,
            hann_window,
            hann_window_s,
            shared_work_buff_long: vec![0.0f64; 2 * BLOCK_LEN_LONG],
            shared_work_buff_short: vec![0.0f64; 2 * BLOCK_LEN_SHORT],
            mdct_xr: vec![0.0f64; BLOCK_LEN_LONG / 2],
            mdct_xi: vec![0.0f64; BLOCK_LEN_LONG / 2],
        }
    }
}

impl PsyInfo {
    pub fn buffer_update(
        &mut self,
        fft_tables: &FftTables,
        gpsy: &mut GlobalPsyInfo,
        new_samples: &[f64],
        bandwidth: u32,
        cb_width_short: &[i32],
        num_cb_short: i32,
    ) {
        let size = self.size as usize;
        let size_s = self.size_s as usize;
        self.data.band_s = (size_s as u32 * bandwidth * 2 / gpsy.sample_rate as u32) as i32;

        {
            let trans_buff = &mut gpsy.shared_work_buff_long;
            trans_buff[..size].copy_from_slice(&self.prev_samples[..size]);
            trans_buff[size..2 * size].copy_from_slice(&new_samples[..size]);
        }

        for win in 0..8 {
            let trans_buff_s_len = 2 * size_s;
            let src_offset = win * BLOCK_LEN_SHORT + (BLOCK_LEN_LONG - BLOCK_LEN_SHORT) / 2;
            let src =
                gpsy.shared_work_buff_long[src_offset..src_offset + trans_buff_s_len].to_vec();
            gpsy.shared_work_buff_short[..trans_buff_s_len].copy_from_slice(&src);

            let win_table = if trans_buff_s_len == BLOCK_LEN_LONG * 2 {
                &gpsy.hann_window
            } else {
                &gpsy.hann_window_s
            };
            for (sample, &w) in gpsy.shared_work_buff_short[..trans_buff_s_len]
                .iter_mut()
                .zip(win_table.iter())
            {
                *sample *= w;
            }

            {
                let buf = &mut gpsy.shared_work_buff_short;
                let xr = &mut gpsy.mdct_xr;
                let xi = &mut gpsy.mdct_xi;
                mdct(fft_tables, buf, trans_buff_s_len, xr, xi);
            }

            self.data.eng_prev[win] = self.data.eng[win];
            self.data.eng[win] = self.data.eng_next[win];
            self.data.eng_next[win] = self.data.eng_next2[win];
            self.data.eng_next2[win] = [0.0; NSFB_SHORT];

            let mut first;
            let mut last = 0i32;
            let mut sfb = 0usize;
            let band_s = self.data.band_s;
            while sfb < num_cb_short as usize {
                first = last;
                last = first + cb_width_short[sfb];
                if first < 1 {
                    first = 1;
                }
                if first >= band_s {
                    break;
                }
                let mut e = 0.0f64;
                for l in (first as usize)..(last as usize) {
                    let v = gpsy.shared_work_buff_short[l];
                    e += v * v;
                }
                self.data.eng_next2[win][sfb] = e as Psyfloat;
                sfb += 1;
            }
            self.data.lastband = sfb as i32;
            while sfb < num_cb_short as usize {
                self.data.eng_next2[win][sfb] = 0.0;
                sfb += 1;
            }
        }

        self.prev_samples[..size].copy_from_slice(&new_samples[..size]);
    }
}

pub fn psy_calculate(
    channel_info: &[ChannelInfo],
    psy: &mut [PsyInfo],
    num_channels: u32,
    mut quality: f64,
) {
    if quality < 0.4 {
        quality = 0.4;
    }
    for channel in 0..num_channels as usize {
        if !channel_info[channel].present {
            continue;
        }
        match channel_info[channel].element_type {
            ElementType::Cpe if channel_info[channel].ch_is_left => {
                let left_chan = channel;
                let right_chan = channel_info[channel].paired_ch as usize;
                let (lo, hi) = psy.split_at_mut(right_chan);
                lo[left_chan].check_short(quality);
                hi[0].check_short(quality);
            }
            ElementType::Lfe => {
                psy[channel].block_type = WindowType::OnlyLongWindow;
            }
            ElementType::Sce => {
                psy[channel].check_short(quality);
            }
            _ => {}
        }
    }
}

pub fn block_switch(coder: &mut [CoderInfo], psy: &[PsyInfo], num_channels: u32) {
    let mut desire = WindowType::OnlyLongWindow;
    for channel in 0..num_channels as usize {
        if psy[channel].block_type == WindowType::OnlyShortWindow {
            desire = WindowType::OnlyShortWindow;
        }
    }
    for channel in 0..num_channels as usize {
        let lasttype = coder[channel].block_type;
        let want_short = desire == WindowType::OnlyShortWindow
            || coder[channel].desired_block_type == WindowType::OnlyShortWindow;
        if want_short {
            if lasttype == WindowType::OnlyLongWindow || lasttype == WindowType::ShortLongWindow {
                coder[channel].block_type = WindowType::LongShortWindow;
            } else {
                coder[channel].block_type = WindowType::OnlyShortWindow;
            }
        } else if lasttype == WindowType::OnlyShortWindow
            || lasttype == WindowType::LongShortWindow
        {
            coder[channel].block_type = WindowType::ShortLongWindow;
        } else {
            coder[channel].block_type = WindowType::OnlyLongWindow;
        }
        coder[channel].desired_block_type = desire;
    }
}
