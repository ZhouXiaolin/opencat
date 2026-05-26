// 1:1 port of faac/libfaac/tns.{h,c}.
//
// `TnsEncode` for `OnlyShortWindow` blocks contains code after an early
// `return` in the C original; that's dead code in the reference encoder.
// The Rust port preserves the same observable behaviour (TNS disabled for
// short blocks) and omits the unreachable branch.

#![allow(dead_code)]

use crate::codec::{BLOCK_LEN_LONG, BLOCK_LEN_SHORT, MAX_SHORT_WINDOWS, WindowType};

pub const TNS_MAX_ORDER: usize = 12;
pub const DEF_TNS_GAIN_THRESH: f64 = 1.4;
pub const DEF_TNS_COEFF_THRESH: f64 = 0.1;
pub const DEF_TNS_COEFF_RES: i32 = 4;
pub const DEF_TNS_RES_OFFSET: i32 = 3;
pub const LEN_TNS_NFILTL: usize = 2;
pub const LEN_TNS_NFILTS: usize = 1;

#[derive(Clone)]
pub struct TnsFilterData {
    pub order: i32,
    pub direction: i32,
    pub coef_compress: i32,
    pub length: i32,
    pub a_coeffs: [f64; TNS_MAX_ORDER + 1],
    pub k_coeffs: [f64; TNS_MAX_ORDER + 1],
    pub index: [i32; TNS_MAX_ORDER + 1],
}

impl Default for TnsFilterData {
    fn default() -> Self {
        Self {
            order: 0,
            direction: 0,
            coef_compress: 0,
            length: 0,
            a_coeffs: [0.0; TNS_MAX_ORDER + 1],
            k_coeffs: [0.0; TNS_MAX_ORDER + 1],
            index: [0; TNS_MAX_ORDER + 1],
        }
    }
}

#[derive(Clone)]
pub struct TnsWindowData {
    pub num_filters: i32,
    pub coef_resolution: i32,
    pub tns_filter: [TnsFilterData; 1 << LEN_TNS_NFILTL],
}

impl Default for TnsWindowData {
    fn default() -> Self {
        Self {
            num_filters: 0,
            coef_resolution: 0,
            tns_filter: std::array::from_fn(|_| TnsFilterData::default()),
        }
    }
}

#[derive(Clone)]
pub struct TnsInfo {
    pub tns_data_present: bool,
    pub tns_min_band_number_long: i32,
    pub tns_min_band_number_short: i32,
    pub tns_max_bands_long: i32,
    pub tns_max_bands_short: i32,
    pub tns_max_order_long: i32,
    pub tns_max_order_short: i32,
    pub window_data: [TnsWindowData; MAX_SHORT_WINDOWS],
}

impl Default for TnsInfo {
    fn default() -> Self {
        Self {
            tns_data_present: false,
            tns_min_band_number_long: 0,
            tns_min_band_number_short: 0,
            tns_max_bands_long: 0,
            tns_max_bands_short: 0,
            tns_max_order_long: 0,
            tns_max_order_short: 0,
            window_data: std::array::from_fn(|_| TnsWindowData::default()),
        }
    }
}

const TNS_MIN_BAND_NUMBER_LONG: [u16; 12] = [11, 12, 15, 16, 17, 20, 25, 26, 24, 28, 30, 31];
const TNS_MIN_BAND_NUMBER_SHORT: [u16; 12] = [2, 2, 2, 3, 3, 4, 6, 6, 8, 10, 10, 12];

const TNS_MAX_BANDS_LONG_LOW: [u16; 12] = [31, 31, 34, 40, 42, 51, 46, 46, 42, 42, 42, 39];
const TNS_MAX_BANDS_SHORT_LOW: [u16; 12] = [9, 9, 10, 14, 14, 14, 14, 14, 14, 14, 14, 14];

const TNS_MAX_ORDER_LONG_LOW: u16 = 12;
const TNS_MAX_ORDER_SHORT_LOW: u16 = 7;

impl TnsInfo {
    pub fn init(&mut self, sample_rate_idx: usize) {
        self.tns_max_bands_long = TNS_MAX_BANDS_LONG_LOW[sample_rate_idx] as i32;
        self.tns_max_bands_short = TNS_MAX_BANDS_SHORT_LOW[sample_rate_idx] as i32;
        self.tns_max_order_long = TNS_MAX_ORDER_LONG_LOW as i32;
        self.tns_max_order_short = TNS_MAX_ORDER_SHORT_LOW as i32;
        self.tns_min_band_number_long = TNS_MIN_BAND_NUMBER_LONG[sample_rate_idx] as i32;
        self.tns_min_band_number_short = TNS_MIN_BAND_NUMBER_SHORT[sample_rate_idx] as i32;
    }

    pub fn encode(
        &mut self,
        number_of_bands: i32,
        max_sfb: i32,
        block_type: WindowType,
        sfb_offset: &[i32],
        spec: &mut [f64],
        temp: &mut [f64],
    ) {
        if block_type == WindowType::OnlyShortWindow {
            self.tns_data_present = false;
            return;
        }

        let number_of_windows = 1;
        let window_size = BLOCK_LEN_SHORT;
        let mut start_band = self.tns_min_band_number_long;
        let mut stop_band = number_of_bands;
        let length_in_bands = stop_band - start_band;
        let order = self.tns_max_order_long;
        start_band = start_band.min(self.tns_max_bands_long);
        stop_band = stop_band.min(self.tns_max_bands_long);

        start_band = start_band.min(max_sfb).max(0);
        stop_band = stop_band.min(max_sfb).max(0);

        self.tns_data_present = false;

        for w in 0..number_of_windows {
            let window_data = &mut self.window_data[w];
            window_data.num_filters = 0;
            window_data.coef_resolution = DEF_TNS_COEFF_RES;

            let start_index = w * window_size + sfb_offset[start_band as usize] as usize;
            let length =
                (sfb_offset[stop_band as usize] - sfb_offset[start_band as usize]) as usize;

            let tns_filter = &mut window_data.tns_filter[0];
            let mut k = [0.0f64; TNS_MAX_ORDER + 1];

            let gain = levinson_durbin(order, length as i32, &spec[start_index..], &mut k);

            if gain > DEF_TNS_GAIN_THRESH {
                self.tns_data_present = true;
                window_data.num_filters += 1;
                tns_filter.direction = 0;
                tns_filter.coef_compress = 0;
                tns_filter.length = length_in_bands;
                quantize_reflection_coeffs(order, DEF_TNS_COEFF_RES, &mut k, &mut tns_filter.index);
                let truncated_order = truncate_coeffs(order, DEF_TNS_COEFF_THRESH, &mut k);
                tns_filter.order = truncated_order;
                tns_filter.k_coeffs.copy_from_slice(&k);
                step_up(truncated_order, &k, &mut tns_filter.a_coeffs);
                tns_inv_filter(
                    length,
                    &mut spec[start_index..start_index + length],
                    tns_filter,
                    temp,
                );
            }
        }
    }

    pub fn encode_filter_only(
        &self,
        number_of_bands: i32,
        max_sfb: i32,
        block_type: WindowType,
        sfb_offset: &[i32],
        spec: &mut [f64],
        temp: &mut [f64],
    ) {
        let (number_of_windows, window_size, mut start_band, mut stop_band);
        if block_type == WindowType::OnlyShortWindow {
            number_of_windows = MAX_SHORT_WINDOWS;
            window_size = BLOCK_LEN_SHORT;
            start_band = self.tns_min_band_number_short;
            stop_band = number_of_bands;
            start_band = start_band.min(self.tns_max_bands_short);
            stop_band = stop_band.min(self.tns_max_bands_short);
        } else {
            number_of_windows = 1;
            window_size = BLOCK_LEN_LONG;
            start_band = self.tns_min_band_number_long;
            stop_band = number_of_bands;
            start_band = start_band.min(self.tns_max_bands_long);
            stop_band = stop_band.min(self.tns_max_bands_long);
        }

        start_band = start_band.min(max_sfb).max(0);
        stop_band = stop_band.min(max_sfb).max(0);

        for w in 0..number_of_windows {
            let window_data = &self.window_data[w];
            let tns_filter = &window_data.tns_filter[0];

            let start_index = w * window_size + sfb_offset[start_band as usize] as usize;
            let length =
                (sfb_offset[stop_band as usize] - sfb_offset[start_band as usize]) as usize;

            if self.tns_data_present && window_data.num_filters != 0 {
                tns_inv_filter(
                    length,
                    &mut spec[start_index..start_index + length],
                    tns_filter,
                    temp,
                );
            }
        }
    }
}

// --- Inner kernels ------------------------------------------------------------

fn tns_inv_filter(length: usize, spec: &mut [f64], filter: &TnsFilterData, temp: &mut [f64]) {
    let order = filter.order as usize;
    let a = &filter.a_coeffs;

    if filter.direction != 0 {
        // Startup, initial state is zero
        temp[length - 1] = spec[length - 1];
        let mut k = 0usize;
        let mut i = length as i32 - 2;
        while i > (length as i32 - 1 - order as i32) {
            let iu = i as usize;
            temp[iu] = spec[iu];
            k += 1;
            for j in 1..=k {
                spec[iu] += temp[iu + j] * a[j];
            }
            i -= 1;
        }

        // Filter the rest
        let mut i = length as i32 - 1 - order as i32;
        while i >= 0 {
            let iu = i as usize;
            temp[iu] = spec[iu];
            for j in 1..=order {
                spec[iu] += temp[iu + j] * a[j];
            }
            i -= 1;
        }
    } else {
        temp[0] = spec[0];
        for i in 1..order {
            temp[i] = spec[i];
            for j in 1..=i {
                spec[i] += temp[i - j] * a[j];
            }
        }
        for i in order..length {
            temp[i] = spec[i];
            for j in 1..=order {
                spec[i] += temp[i - j] * a[j];
            }
        }
    }
}

fn truncate_coeffs(f_order: i32, threshold: f64, k_array: &mut [f64]) -> i32 {
    for i in (0..=f_order).rev() {
        let iu = i as usize;
        if k_array[iu].abs() <= threshold {
            k_array[iu] = 0.0;
        }
        if k_array[iu] != 0.0 {
            return i;
        }
    }
    0
}

fn quantize_reflection_coeffs(
    f_order: i32,
    coeff_res: i32,
    k_array: &mut [f64],
    index_array: &mut [i32],
) {
    let iqfac = ((1i32 << (coeff_res - 1)) as f64 - 0.5) / (std::f64::consts::FRAC_PI_2);
    let iqfac_m = ((1i32 << (coeff_res - 1)) as f64 + 0.5) / (std::f64::consts::FRAC_PI_2);

    for i in 1..=f_order as usize {
        let k = k_array[i];
        let asin_k = k.asin();
        if k >= 0.0 {
            index_array[i] = (0.5 + asin_k * iqfac) as i32;
        } else {
            index_array[i] = (-0.5 + asin_k * iqfac_m) as i32;
        }
        let denom = if index_array[i] >= 0 { iqfac } else { iqfac_m };
        k_array[i] = (index_array[i] as f64 / denom).sin();
    }
}

fn autocorrelation(max_order: i32, mut data_size: i32, data: &[f64], r_array: &mut [f64]) {
    for order in 0..=max_order as usize {
        r_array[order] = 0.0;
        for index in 0..data_size as usize {
            r_array[order] += data[index] * data[index + order];
        }
        data_size -= 1;
    }
}

fn levinson_durbin(f_order: i32, data_size: i32, data: &[f64], k_array: &mut [f64]) -> f64 {
    let mut a_buf = [[0.0f64; TNS_MAX_ORDER + 1]; 2];
    let mut cur = 0usize; // aPtr
    let mut lst = 1usize; // aLastPtr
    let mut r_array = [0.0f64; TNS_MAX_ORDER + 1];

    autocorrelation(f_order, data_size, data, &mut r_array);
    let signal = r_array[0];

    if signal == 0.0 {
        k_array[0] = 1.0;
        for order in 1..=f_order as usize {
            k_array[order] = 0.0;
        }
        return 0.0;
    }

    k_array[0] = 1.0;
    a_buf[cur][0] = 1.0;
    a_buf[lst][0] = 1.0;
    let mut error = r_array[0];

    for order in 1..=f_order as usize {
        let mut k_temp = a_buf[lst][0] * r_array[order];
        for i in 1..order {
            k_temp += a_buf[lst][i] * r_array[order - i];
        }
        if error <= 0.0 || k_temp.abs() >= error {
            error = 0.0;
            break;
        }
        k_temp = -k_temp / error;
        k_array[order] = k_temp;
        a_buf[cur][order] = k_temp;
        for i in 1..order {
            a_buf[cur][i] = a_buf[lst][i] + k_temp * a_buf[lst][order - i];
        }
        error *= 1.0 - k_temp * k_temp;
        if error <= 0.0 {
            break;
        }
        std::mem::swap(&mut cur, &mut lst);
    }

    if error <= 0.0 {
        return DEF_TNS_GAIN_THRESH + 1.0;
    }
    signal / error
}

fn step_up(f_order: i32, k_array: &[f64], a_array: &mut [f64]) {
    let mut a_temp = [0.0f64; TNS_MAX_ORDER + 2];
    a_array[0] = 1.0;
    a_temp[0] = 1.0;

    for order in 1..=f_order as usize {
        a_array[order] = 0.0;
        for i in 1..=order {
            a_temp[i] = a_array[i] + k_array[order] * a_array[order - i];
        }
        for i in 1..=order {
            a_array[i] = a_temp[i];
        }
    }
}
