// 1:1 port of faac/libfaac/quantize.{h,c}.
//
// Notes:
// - SIMD (quantize_sse2) is skipped; the scalar path is the only one.
// - `lrint` uses C round-to-even semantics; Rust's `round_ties_even` matches.
// - `sfstep`/`max_quant_limit` mirror the `QuantizeInit` statics with
//   `OnceLock` lazy initialization (compiler can't const-eval log10/pow).

#![allow(dead_code)]

use std::sync::OnceLock;

use crate::bitstream::{
    HCB_INTENSITY, HCB_INTENSITY2, HCB_NONE, HCB_PNS, HCB_ZERO, MAX_HUFF_ESC_VAL, SF_MIN,
    SF_OFFSET, SF_PNS_OFFSET, clamp_sf_diff,
};
use crate::codec::{
    BLOCK_LEN_LONG, BLOCK_LEN_SHORT, CoderInfo, MAX_SCFAC_BANDS, MAX_SHORT_WINDOWS, NSFB_SHORT,
    SrInfo, WindowType,
};
use crate::util::lrint;

pub const MAGIC_NUMBER: f64 = 0.4054;

pub const DEFQUAL: i32 = 100;
pub const MAXQUAL: i32 = 5000;
pub const MAXQUALADTS: i32 = MAXQUAL;
pub const MINQUAL: i32 = 10;

const NOISEFLOOR: f64 = 0.4;
const SF_CHAIN_UNSET: i32 = i32::MIN;
const MAXSHORTBAND: usize = 36;
const MINSFB: usize = 2;

// --- One-time-initialized constants ------------------------------------------

fn sfstep() -> f64 {
    static SFSTEP: OnceLock<f64> = OnceLock::new();
    // 1 / log10(2^0.25) - 1.50515 dB step from AAC spec
    *SFSTEP.get_or_init(|| 1.0 / 2.0_f64.sqrt().sqrt().log10())
}

fn max_quant_limit() -> f64 {
    static MAX_LIMIT: OnceLock<f64> = OnceLock::new();
    // (8191 + 1 - 0.4054) ^ (4/3)
    *MAX_LIMIT.get_or_init(|| {
        let base = MAX_HUFF_ESC_VAL as f64 + 1.0 - MAGIC_NUMBER;
        base.powf(4.0 / 3.0)
    })
}

pub fn quantize_init() {
    // Force lazy statics now so any later call sees them initialized.
    let _ = sfstep();
    let _ = max_quant_limit();
}

// --- AACQuantCfg --------------------------------------------------------------

pub struct AACQuantCfg {
    pub quality: f64,
    pub max_cbl: i32,
    pub max_cbs: i32,
    pub max_l: i32,
    pub pnslevel: i32,
}

impl Default for AACQuantCfg {
    fn default() -> Self {
        Self {
            quality: 0.0,
            max_cbl: 0,
            max_cbs: 0,
            max_l: 0,
            pnslevel: 0,
        }
    }
}

// --- Scalar quantizer ---------------------------------------------------------

fn quantize_scalar(xr: &[f64], xi: &mut [i32], n: usize, sfacfix: f64) {
    let magic = MAGIC_NUMBER;
    for cnt in 0..n {
        let val = xr[cnt];
        let mut tmp = val.abs();
        tmp *= sfacfix;
        tmp = (tmp * tmp.sqrt()).sqrt(); // (tmp)^0.75
        let q = (tmp + magic) as i32;
        xi[cnt] = if val < 0.0 { -q } else { q };
    }
}

/// Compute gain from integer sfac, clamping against Huffman overflow.
/// Updates `sfac` in place if clamping was applied; returns the usable gain.
fn gain_with_overflow_clamp(sfac: &mut i32, band_peak: f64) -> f64 {
    let mut gain = 10f64.powf(*sfac as f64 / sfstep());
    if band_peak > 0.0 && gain * band_peak > max_quant_limit() {
        gain = max_quant_limit() / band_peak;
        *sfac = (gain.log10() * sfstep()).floor() as i32;
        gain = 10f64.powf(*sfac as f64 / sfstep());
    }
    gain
}

// --- Per-band masking ---------------------------------------------------------

fn bmask(
    coder: &CoderInfo,
    xr0: &[f64],
    bandqual: &mut [f64],
    bandenrg: &mut [f64],
    bandmaxe: &mut [f64],
    gnum: usize,
    quality: f64,
) {
    let gsize = coder.groups.len[gnum] as usize;
    let sfbn = coder.sfbn as usize;
    let total_len = coder.sfb_offset[sfbn] as usize;
    let powm = 0.4_f64;

    let mut totenrg = 0.0_f64;
    for win in 0..gsize {
        let xr = &xr0[win * BLOCK_LEN_SHORT..];
        for cnt in 0..total_len {
            totenrg += xr[cnt] * xr[cnt];
        }
    }
    let enrgcnt = (gsize * total_len) as f64;

    if totenrg < (NOISEFLOOR * NOISEFLOOR) * enrgcnt {
        for sfb in 0..sfbn {
            bandqual[sfb] = 0.0;
            bandenrg[sfb] = 0.0;
        }
        return;
    }

    const NOISETONE: f64 = 0.2;
    const TONEMASK: f64 = 0.45;
    const SHORT_PENALTY: f64 = 0.45;

    for sfb in 0..sfbn {
        let start = coder.sfb_offset[sfb] as usize;
        let end = coder.sfb_offset[sfb + 1] as usize;

        let mut avge = 0.0_f64;
        let mut maxe = 0.0_f64;
        for win in 0..gsize {
            let xr = &xr0[win * BLOCK_LEN_SHORT + start..];
            let n = end - start;
            for cnt in 0..n {
                let v = xr[cnt];
                let e = v * v;
                avge += e;
                if maxe < e {
                    maxe = e;
                }
            }
        }
        bandenrg[sfb] = avge;
        bandmaxe[sfb] = maxe.sqrt();
        let maxe = maxe * gsize as f64;

        let last;
        let mut target;
        if coder.block_type == WindowType::OnlyShortWindow {
            last = BLOCK_LEN_SHORT as f64;
            let avgenrg = (totenrg / last) * (end - start) as f64;
            target = NOISETONE * (avge / avgenrg).powf(powm);
            target += (1.0 - NOISETONE) * TONEMASK * (maxe / avgenrg).powf(powm);
            target *= SHORT_PENALTY;
        } else {
            last = BLOCK_LEN_LONG as f64;
            let avgenrg = (totenrg / last) * (end - start) as f64;
            target = NOISETONE * (avge / avgenrg).powf(powm);
            target += (1.0 - NOISETONE) * TONEMASK * (maxe / avgenrg).powf(powm);
        }

        target *= 10.0 / (1.0 + (start as f64 + end as f64) / last);
        bandqual[sfb] = target * quality;
    }
}

// --- Per-band quantization ----------------------------------------------------

fn qlevel(
    coder: &mut CoderInfo,
    xr0: &[f64],
    bandqual: &[f64],
    bandenrg: &[f64],
    bandmaxe: &[f64],
    gnum: usize,
    pnslevel: i32,
    p_last_abs: &mut i32,
) {
    let gsize = coder.groups.len[gnum] as usize;
    let pnsthr = 0.1 * pnslevel as f64;
    let sfbn = coder.sfbn as usize;

    let mut xitab = [0i32; 8 * MAXSHORTBAND];

    let mut sb = 0usize;
    while sb < sfbn && (coder.bandcnt as usize) < MAX_SCFAC_BANDS {
        let bandcnt = coder.bandcnt as usize;

        if coder.book[bandcnt] != HCB_NONE {
            coder.bandcnt += 1;
            sb += 1;
            continue;
        }

        let start = coder.sfb_offset[sb] as usize;
        let end = coder.sfb_offset[sb + 1] as usize;

        let etot = bandenrg[sb] / gsize as f64;
        let rmsx = (etot / (end - start) as f64).sqrt();

        if rmsx < NOISEFLOOR || bandqual[sb] == 0.0 {
            coder.book[bandcnt] = HCB_ZERO;
            coder.bandcnt += 1;
            sb += 1;
            continue;
        }

        if bandqual[sb] < pnsthr {
            coder.book[bandcnt] = HCB_PNS;
            coder.sf[bandcnt] += lrint(etot.log10() * (0.5 * sfstep()));
            coder.bandcnt += 1;
            sb += 1;
            continue;
        }

        let mut sfac = lrint((bandqual[sb] / rmsx).log10() * sfstep());
        let mut sf_rel = SF_OFFSET - sfac;
        let sf_bias = coder.sf[bandcnt];
        let mut sf_abs = sf_bias + sf_rel;

        let sfacfix;
        if sf_rel < SF_MIN {
            sfacfix = 0.0;
        } else {
            let mut g = gain_with_overflow_clamp(&mut sfac, bandmaxe[sb]);
            sf_rel = SF_OFFSET - sfac;
            sf_abs = sf_bias + sf_rel;

            if *p_last_abs != SF_CHAIN_UNSET {
                let diff = sf_abs - *p_last_abs;
                let clamped_diff = clamp_sf_diff(diff);
                if clamped_diff != diff {
                    sf_abs = *p_last_abs + clamped_diff;
                    sf_rel = sf_abs - sf_bias;
                    sfac = SF_OFFSET - sf_rel;
                    if clamped_diff > 0 {
                        g = gain_with_overflow_clamp(&mut sfac, bandmaxe[sb]);
                        sf_rel = SF_OFFSET - sfac;
                        sf_abs = sf_bias + sf_rel;
                    } else {
                        g = 10f64.powf(sfac as f64 / sfstep());
                    }
                }
            }
            sfacfix = g;
        }

        let end_off = end - start;
        if sfacfix <= 0.0 {
            for i in 0..(gsize * end_off) {
                xitab[i] = 0;
            }
        } else {
            let mut xi_pos = 0usize;
            for win in 0..gsize {
                let xr =
                    &xr0[win * BLOCK_LEN_SHORT + start..win * BLOCK_LEN_SHORT + start + end_off];
                quantize_scalar(xr, &mut xitab[xi_pos..xi_pos + end_off], end_off, sfacfix);
                xi_pos += end_off;
            }
        }

        coder.huffbook(&xitab[..gsize * end_off], gsize * end_off);

        if coder.book[bandcnt] != HCB_ZERO {
            *p_last_abs = sf_abs;
        }
        coder.sf[bandcnt] += sf_rel;
        coder.bandcnt += 1;
        sb += 1;
    }
}

// --- Public API ---------------------------------------------------------------

impl CoderInfo {
    pub fn quantize(&mut self, xr: &[f64], cfg: &AACQuantCfg) -> i32 {
        let mut bandlvl = [0.0f64; MAX_SCFAC_BANDS];
        let mut bandenrg = [0.0f64; MAX_SCFAC_BANDS];
        let mut bandmaxe = [0.0f64; MAX_SCFAC_BANDS];

        self.global_gain = 0;
        self.bandcnt = 0;
        self.datacnt = 0;

        let mut lastsf = SF_CHAIN_UNSET;

        let mut gxr_offset = 0usize;
        for cnt in 0..self.groups.n as usize {
            let group_len = self.groups.len[cnt] as usize;
            let group_end = gxr_offset + group_len * BLOCK_LEN_SHORT;
            let gxr = &xr[gxr_offset..];
            bmask(
                self,
                gxr,
                &mut bandlvl,
                &mut bandenrg,
                &mut bandmaxe,
                cnt,
                cfg.quality / DEFQUAL as f64,
            );
            qlevel(
                self,
                gxr,
                &bandlvl,
                &bandenrg,
                &bandmaxe,
                cnt,
                cfg.pnslevel,
                &mut lastsf,
            );
            gxr_offset = group_end;
        }

        self.global_gain = 0;
        for cnt in 0..self.bandcnt as usize {
            let book = self.book[cnt];
            if book == 0 {
                continue;
            }
            if book != HCB_INTENSITY && book != HCB_INTENSITY2 {
                self.global_gain = self.sf[cnt];
                break;
            }
        }

        let mut lastis = 0i32;
        let mut lastpns = self.global_gain - SF_PNS_OFFSET;
        for cnt in 0..self.bandcnt as usize {
            let book = self.book[cnt];
            if book == HCB_INTENSITY || book == HCB_INTENSITY2 {
                let mut diff = self.sf[cnt] - lastis;
                diff = clamp_sf_diff(diff);
                lastis += diff;
                self.sf[cnt] = lastis;
            } else if book == HCB_PNS {
                let mut diff = self.sf[cnt] - lastpns;
                diff = clamp_sf_diff(diff);
                lastpns += diff;
                self.sf[cnt] = lastpns;
            }
        }

        1
    }

    pub fn group_short_blocks(&mut self, xr: &mut [f64], cfg: &AACQuantCfg) {
        if self.block_type != WindowType::OnlyShortWindow {
            self.groups.n = 1;
            self.groups.len[0] = 1;
            return;
        }

        let maxl = (cfg.max_l / 8) as usize;
        let maxsfb = cfg.max_cbs as usize;
        let fastmin = ((maxsfb as i32 - MINSFB as i32) * 3) >> 2;
        let thr = 3.0_f64;

        let mut e = [0.0f64; NSFB_SHORT];
        let mut min = [0.0f64; NSFB_SHORT];
        let mut max = [0.0f64; NSFB_SHORT];

        calce(xr, &self.sfb_offset, &mut e, maxsfb, maxl);
        resete(&mut min, &mut max, &e, maxsfb);

        let mut win0 = 0i32;
        self.groups.n = 0;
        for win in 1..MAX_SHORT_WINDOWS as i32 {
            let start = win as usize * BLOCK_LEN_SHORT;
            calce(&mut xr[start..], &self.sfb_offset, &mut e, maxsfb, maxl);

            let mut fast = 0i32;
            for sfb in MINSFB..maxsfb {
                if min[sfb] > e[sfb] {
                    min[sfb] = e[sfb];
                }
                if max[sfb] < e[sfb] {
                    max[sfb] = e[sfb];
                }
                if max[sfb] > thr * min[sfb] {
                    fast += 1;
                }
            }
            if fast > fastmin {
                let n = self.groups.n as usize;
                self.groups.len[n] = win - win0;
                self.groups.n += 1;
                win0 = win;
                resete(&mut min, &mut max, &e, maxsfb);
            }
        }
        let n = self.groups.n as usize;
        self.groups.len[n] = MAX_SHORT_WINDOWS as i32 - win0;
        self.groups.n += 1;
    }
}

impl AACQuantCfg {
    pub fn calc_bw(&mut self, bw: &mut u32, rate: i32, sr: &SrInfo) {
        let mut max = ((*bw as i64) * ((BLOCK_LEN_SHORT << 1) as i64) / rate as i64) as i32;
        let mut l = 0i32;
        let mut cnt = 0usize;
        for (i, &w) in sr.cb_width_short[..sr.num_cb_short as usize]
            .iter()
            .enumerate()
        {
            if l >= max {
                break;
            }
            l += w;
            cnt = i + 1;
        }
        self.max_cbs = cnt as i32;
        if self.pnslevel != 0 {
            *bw = (l as u64 * rate as u64 / (BLOCK_LEN_SHORT << 1) as u64) as u32;
        }

        max = ((*bw as i64) * ((BLOCK_LEN_LONG << 1) as i64) / rate as i64) as i32;
        l = 0;
        cnt = 0;
        for (i, &w) in sr.cb_width_long[..sr.num_cb_long as usize]
            .iter()
            .enumerate()
        {
            if l >= max {
                break;
            }
            l += w;
            cnt = i + 1;
        }
        self.max_cbl = cnt as i32;
        self.max_l = l;

        *bw = (l as u64 * rate as u64 / (BLOCK_LEN_LONG << 1) as u64) as u32;
    }
}

fn calce(xr: &mut [f64], bands: &[i32], e: &mut [f64; NSFB_SHORT], maxsfb: usize, maxl: usize) {
    // Mute lines above cutoff freq.
    for l in maxl..bands[maxsfb] as usize {
        xr[l] = 0.0;
    }
    for sfb in MINSFB..maxsfb {
        e[sfb] = 0.0;
        let start = bands[sfb] as usize;
        let end = bands[sfb + 1] as usize;
        for l in start..end {
            e[sfb] += xr[l] * xr[l];
        }
    }
}

fn resete(
    min: &mut [f64; NSFB_SHORT],
    max: &mut [f64; NSFB_SHORT],
    e: &[f64; NSFB_SHORT],
    maxsfb: usize,
) {
    for sfb in MINSFB..maxsfb {
        min[sfb] = e[sfb];
        max[sfb] = e[sfb];
    }
}

pub fn bloc_stat() {
    // PRINTSTAT debug path; no-op like the default C build.
}
