use crate::analysis::FftTables;
use crate::codec::*;

pub struct Windows {
    pub sin_long: Vec<f64>,
    pub sin_short: Vec<f64>,
    pub kbd_long: Vec<f64>,
    pub kbd_short: Vec<f64>,
}

pub struct FilterBankBuffers {
    pub freq_buff: Vec<Vec<f64>>,
    pub overlap_buff: Vec<Vec<f64>>,
    pub windows: Windows,
    pub work_long: Vec<f64>,
    pub mdct_xr: Vec<f64>,
    pub mdct_xi: Vec<f64>,
}

impl FilterBankBuffers {
    pub fn new(num_channels: usize) -> Self {
        let freq_buff = (0..num_channels)
            .map(|_| vec![0.0f64; 2 * FRAME_LEN])
            .collect();
        let overlap_buff = (0..num_channels).map(|_| vec![0.0f64; FRAME_LEN]).collect();

        let sin_window_long: Vec<f64> = (0..BLOCK_LEN_LONG)
            .map(|i| {
                (std::f64::consts::PI / (2.0 * BLOCK_LEN_LONG as f64) * (i as f64 + 0.5)).sin()
            })
            .collect();
        let sin_window_short: Vec<f64> = (0..BLOCK_LEN_SHORT)
            .map(|i| {
                (std::f64::consts::PI / (2.0 * BLOCK_LEN_SHORT as f64) * (i as f64 + 0.5)).sin()
            })
            .collect();

        let mut kbd_window_long = vec![0.0f64; BLOCK_LEN_LONG];
        let mut kbd_window_short = vec![0.0f64; BLOCK_LEN_SHORT];

        calculate_kbd_window(&mut kbd_window_long, 4.0, BLOCK_LEN_LONG * 2);
        calculate_kbd_window(&mut kbd_window_short, 6.0, BLOCK_LEN_SHORT * 2);

        Self {
            freq_buff,
            overlap_buff,
            windows: Windows {
                sin_long: sin_window_long,
                sin_short: sin_window_short,
                kbd_long: kbd_window_long,
                kbd_short: kbd_window_short,
            },
            work_long: vec![0.0f64; 2 * BLOCK_LEN_LONG],
            mdct_xr: vec![0.0f64; BLOCK_LEN_LONG / 2],
            mdct_xi: vec![0.0f64; BLOCK_LEN_LONG / 2],
        }
    }

    pub fn process_channel(
        &mut self,
        fft_tables: &FftTables,
        coder_info: &CoderInfo,
        p_in_data: &[f64],
        channel: usize,
    ) {
        let p_out_mdct = &mut self.freq_buff[channel];
        let p_overlap = &mut self.overlap_buff[channel];

        self.work_long[..BLOCK_LEN_LONG].copy_from_slice(&p_overlap[..BLOCK_LEN_LONG]);
        self.work_long[BLOCK_LEN_LONG..2 * BLOCK_LEN_LONG]
            .copy_from_slice(&p_in_data[..BLOCK_LEN_LONG]);
        p_overlap[..BLOCK_LEN_LONG].copy_from_slice(&p_in_data[..BLOCK_LEN_LONG]);

        let (sin_window_long, sin_window_short) = (&self.windows.sin_long, &self.windows.sin_short);
        let (kbd_window_long, kbd_window_short) = (&self.windows.kbd_long, &self.windows.kbd_short);

        let first_window: &[f64] = match coder_info.prev_window_shape {
            WindowShape::Sine => match coder_info.block_type {
                WindowType::OnlyLongWindow | WindowType::LongShortWindow => sin_window_long,
                _ => sin_window_short,
            },
            WindowShape::Kbd => match coder_info.block_type {
                WindowType::OnlyLongWindow | WindowType::LongShortWindow => kbd_window_long,
                _ => kbd_window_short,
            },
        };

        let second_window: &[f64] = match coder_info.window_shape {
            WindowShape::Kbd => match coder_info.block_type {
                WindowType::OnlyLongWindow | WindowType::ShortLongWindow => kbd_window_long,
                _ => kbd_window_short,
            },
            WindowShape::Sine => match coder_info.block_type {
                WindowType::OnlyLongWindow | WindowType::ShortLongWindow => sin_window_long,
                _ => sin_window_short,
            },
        };

        let transf_buf = &mut self.work_long;
        match coder_info.block_type {
            WindowType::OnlyLongWindow => {
                for i in 0..BLOCK_LEN_LONG {
                    p_out_mdct[i] = transf_buf[i] * first_window[i];
                    p_out_mdct[i + BLOCK_LEN_LONG] =
                        transf_buf[i + BLOCK_LEN_LONG] * second_window[BLOCK_LEN_LONG - 1 - i];
                }
                mdct(
                    fft_tables,
                    p_out_mdct,
                    2 * BLOCK_LEN_LONG,
                    &mut self.mdct_xr,
                    &mut self.mdct_xi,
                );
            }
            WindowType::LongShortWindow => {
                for i in 0..BLOCK_LEN_LONG {
                    p_out_mdct[i] = transf_buf[i] * first_window[i];
                }
                p_out_mdct[BLOCK_LEN_LONG..BLOCK_LEN_LONG + NFLAT_LS]
                    .copy_from_slice(&transf_buf[BLOCK_LEN_LONG..BLOCK_LEN_LONG + NFLAT_LS]);
                for i in 0..BLOCK_LEN_SHORT {
                    p_out_mdct[i + BLOCK_LEN_LONG + NFLAT_LS] = transf_buf
                        [i + BLOCK_LEN_LONG + NFLAT_LS]
                        * second_window[BLOCK_LEN_SHORT - i - 1];
                }
                for i in (BLOCK_LEN_LONG + NFLAT_LS + BLOCK_LEN_SHORT)
                    ..(BLOCK_LEN_LONG + NFLAT_LS + BLOCK_LEN_SHORT + NFLAT_LS)
                {
                    p_out_mdct[i] = 0.0;
                }
                mdct(
                    fft_tables,
                    p_out_mdct,
                    2 * BLOCK_LEN_LONG,
                    &mut self.mdct_xr,
                    &mut self.mdct_xi,
                );
            }
            WindowType::ShortLongWindow => {
                for i in 0..NFLAT_LS {
                    p_out_mdct[i] = 0.0;
                }
                for i in 0..BLOCK_LEN_SHORT {
                    p_out_mdct[i + NFLAT_LS] = transf_buf[i + NFLAT_LS] * first_window[i];
                }
                p_out_mdct[NFLAT_LS + BLOCK_LEN_SHORT..NFLAT_LS + BLOCK_LEN_SHORT + NFLAT_LS]
                    .copy_from_slice(
                        &transf_buf
                            [NFLAT_LS + BLOCK_LEN_SHORT..NFLAT_LS + BLOCK_LEN_SHORT + NFLAT_LS],
                    );
                for i in 0..BLOCK_LEN_LONG {
                    p_out_mdct[i + BLOCK_LEN_LONG] =
                        transf_buf[i + BLOCK_LEN_LONG] * second_window[BLOCK_LEN_LONG - i - 1];
                }
                mdct(
                    fft_tables,
                    p_out_mdct,
                    2 * BLOCK_LEN_LONG,
                    &mut self.mdct_xr,
                    &mut self.mdct_xi,
                );
            }
            WindowType::OnlyShortWindow => {
                let mut p_o_offset = NFLAT_LS;
                let mut p_out_offset = 0usize;
                let mut first_win = first_window;
                for _k in 0..MAX_SHORT_WINDOWS {
                    for i in 0..BLOCK_LEN_SHORT {
                        p_out_mdct[p_out_offset + i] = transf_buf[p_o_offset + i] * first_win[i];
                        p_out_mdct[p_out_offset + i + BLOCK_LEN_SHORT] = transf_buf
                            [p_o_offset + i + BLOCK_LEN_SHORT]
                            * second_window[BLOCK_LEN_SHORT - i - 1];
                    }
                    mdct(
                        fft_tables,
                        &mut p_out_mdct[p_out_offset..],
                        2 * BLOCK_LEN_SHORT,
                        &mut self.mdct_xr,
                        &mut self.mdct_xi,
                    );
                    p_out_offset += BLOCK_LEN_SHORT;
                    p_o_offset += BLOCK_LEN_SHORT;
                    first_win = second_window;
                }
            }
        }
    }
}

fn izero(x: f64) -> f64 {
    const EPSILON: f64 = 1e-41;
    let mut sum = 1.0f64;
    let mut u = 1.0f64;
    let halfx = x / 2.0;
    let mut n = 1;
    loop {
        let temp = halfx / n as f64;
        n += 1;
        let temp = temp * temp;
        u *= temp;
        sum += u;
        if u < EPSILON * sum {
            break;
        }
    }
    sum
}

fn calculate_kbd_window(win: &mut [f64], alpha: f64, length: usize) {
    let alpha = alpha * std::f64::consts::PI;
    let ibeta = 1.0 / izero(alpha);
    let half = length >> 1;

    let mut sum = 0.0f64;
    for i in 0..half {
        let tmp = 4.0 * i as f64 / length as f64 - 1.0;
        win[i] = izero(alpha * (1.0 - tmp * tmp).sqrt()) * ibeta;
        sum += win[i];
    }

    sum = 1.0 / sum;
    let mut tmp = 0.0f64;
    for i in 0..half {
        tmp += win[i];
        win[i] = (tmp * sum).sqrt();
    }
}

pub fn mdct(fft_tables: &FftTables, data: &mut [f64], n: usize, xr: &mut [f64], xi: &mut [f64]) {
    let freq = TWOPI / n as f64;
    let n2 = n >> 1;
    let n4 = n >> 2;
    let n8 = n >> 3;

    let cfreq = freq.cos();
    let sfreq = freq.sin();

    let mut c = (freq * 0.125).cos();
    let mut s = (freq * 0.125).sin();

    let mut n1 = n2 as i32 - 1;
    let mut n2_idx = 0i32;

    for i in 0..n8 {
        let idx_n1 = (n4 as i32 + n1) as usize;
        let idx_n1_base2 = (n as i32 + n4 as i32 - 1 - n1) as usize;
        let idx_n2 = (n4 as i32 + n2_idx) as usize;
        let idx_n2_base1 = (n4 as i32 - 1 - n2_idx) as usize;

        let tempr = data[idx_n1] + data[idx_n1_base2];
        let tempi = data[idx_n2] - data[idx_n2_base1];

        xr[i] = tempr * c + tempi * s;
        xi[i] = tempi * c - tempr * s;

        let cold = c;
        c = c * cfreq - s * sfreq;
        s = s * cfreq + cold * sfreq;

        n1 -= 2;
        n2_idx += 2;
    }

    for i in n8..n4 {
        let idx_n1 = (n4 as i32 + n1) as usize;
        let idx_n1_base1 = (n4 as i32 - 1 - n1) as usize;
        let idx_n2 = (n4 as i32 + n2_idx) as usize;
        let idx_n2_base2 = (n as i32 + n4 as i32 - 1 - n2_idx) as usize;

        let tempr = data[idx_n1] - data[idx_n1_base1];
        let tempi = data[idx_n2] + data[idx_n2_base2];

        xr[i] = tempr * c + tempi * s;
        xi[i] = tempi * c - tempr * s;

        let cold = c;
        c = c * cfreq - s * sfreq;
        s = s * cfreq + cold * sfreq;

        n1 -= 2;
        n2_idx += 2;
    }

    match n {
        n if n == BLOCK_LEN_SHORT * 2 => fft_tables.fft(xr, xi, 6),
        n if n == BLOCK_LEN_LONG * 2 => fft_tables.fft(xr, xi, 9),
        _ => {}
    }

    c = (freq * 0.125).cos();
    s = (freq * 0.125).sin();
    let cfreq = freq.cos();
    let sfreq = freq.sin();

    let mut n2_idx = 0usize;

    for i in 0..n4 {
        let tempr = 2.0 * (xr[i] * c + xi[i] * s);
        let tempi = 2.0 * (xi[i] * c - xr[i] * s);

        data[n2_idx] = -tempr;
        data[n2 - 1 - n2_idx] = tempi;
        data[n2 + n2_idx] = -tempi;
        data[n - 1 - n2_idx] = tempr;

        let cold = c;
        c = c * cfreq - s * sfreq;
        s = s * cfreq + cold * sfreq;

        n2_idx += 2;
    }
}
