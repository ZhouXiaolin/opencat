// 1:1 port of faac/libfaac/stereo.{h,c} — Intensity Stereo + M/S processing.

#![allow(dead_code)]

use crate::codec::{ChannelInfo, ElementType};
use crate::codec::{BLOCK_LEN_SHORT, CoderInfo, JointMode, WindowType};
use crate::bitstream::{HCB_INTENSITY, HCB_INTENSITY2, HCB_NONE, HCB_ZERO};
use crate::util::lrint;

fn stereo(
    cl: &mut CoderInfo,
    cr: &mut CoderInfo,
    sl0: &mut [f64],
    sr0: &mut [f64],
    sfcnt: &mut i32,
    wstart: i32,
    wend: i32,
    mut phthr: f64,
) {
    if phthr == 0.0 {
        return;
    }

    phthr = 1.0 / phthr;

    let sfmin = if cl.block_type == WindowType::OnlyShortWindow {
        1
    } else {
        8
    };

    *sfcnt += sfmin;
    let sfbn = cl.sfbn;

    for sfb in sfmin as usize..sfbn as usize {
        let start = cl.sfb_offset[sfb] as usize;
        let end = cl.sfb_offset[sfb + 1] as usize;

        let mut enrgs = 0.0_f64;
        let mut enrgd = 0.0_f64;
        let mut enrgl = 0.0_f64;
        let mut enrgr = 0.0_f64;

        for win in wstart..wend {
            let off = win as usize * BLOCK_LEN_SHORT;
            for l in start..end {
                let lx = sl0[off + l];
                let rx = sr0[off + l];
                let sum = lx + rx;
                let diff = lx - rx;
                enrgs += sum * sum;
                enrgd += diff * diff;
                enrgl += lx * lx;
                enrgr += rx * rx;
            }
        }

        let mut ethr = enrgl.sqrt() + enrgr.sqrt();
        ethr *= ethr;
        ethr *= phthr;
        let efix = enrgl + enrgr;

        if efix <= 0.0 {
            *sfcnt += 1;
            continue;
        }

        let mut hcb = HCB_NONE;
        let mut vfix = 0.0_f64;
        if enrgs >= ethr {
            hcb = HCB_INTENSITY;
            vfix = (efix / enrgs).sqrt();
        } else if enrgd >= ethr {
            hcb = HCB_INTENSITY2;
            vfix = (efix / enrgd).sqrt();
        }

        if hcb != HCB_NONE {
            if enrgl == 0.0 || enrgr == 0.0 {
                *sfcnt += 1;
                continue;
            }
            let step = 10.0 / 1.50515;
            let sf = lrint((enrgl / efix).log10() * step);
            let pan = lrint((enrgr / efix).log10() * step) - sf;

            if pan > 30 {
                cl.book[*sfcnt as usize] = HCB_ZERO;
                *sfcnt += 1;
                continue;
            }
            if pan < -30 {
                cr.book[*sfcnt as usize] = HCB_ZERO;
                *sfcnt += 1;
                continue;
            }
            cl.sf[*sfcnt as usize] = sf;
            cr.sf[*sfcnt as usize] = -pan;
            cr.book[*sfcnt as usize] = hcb;

            for win in wstart..wend {
                let off = win as usize * BLOCK_LEN_SHORT;
                for l in start..end {
                    let sum = if hcb == HCB_INTENSITY {
                        sl0[off + l] + sr0[off + l]
                    } else {
                        sl0[off + l] - sr0[off + l]
                    };
                    sl0[off + l] = sum * vfix;
                }
            }
        }
        *sfcnt += 1;
    }
}

fn midside(
    coder: &CoderInfo,
    channel: &mut ChannelInfo,
    sl0: &mut [f64],
    sr0: &mut [f64],
    sfcnt: &mut i32,
    wstart: i32,
    wend: i32,
    thrmid: f64,
    thrside: f64,
) {
    let sfmin = if coder.block_type == WindowType::OnlyShortWindow {
        1
    } else {
        8
    };
    let sfbn = coder.sfbn;

    for _sfb in 0..sfmin {
        channel.ms_info.ms_used[*sfcnt as usize] = false;
        *sfcnt += 1;
    }

    const PH_NONE: i32 = 0;
    const PH_IN: i32 = 1;
    const PH_OUT: i32 = 2;

    for sfb in sfmin as usize..sfbn as usize {
        let start = coder.sfb_offset[sfb] as usize;
        let end = coder.sfb_offset[sfb + 1] as usize;

        let mut enrgs = 0.0_f64;
        let mut enrgd = 0.0_f64;
        let mut enrgl = 0.0_f64;
        let mut enrgr = 0.0_f64;
        for win in wstart..wend {
            let off = win as usize * BLOCK_LEN_SHORT;
            for l in start..end {
                let lx = sl0[off + l];
                let rx = sr0[off + l];
                let sum = 0.5 * (lx + rx);
                let diff = 0.5 * (lx - rx);
                enrgs += sum * sum;
                enrgd += diff * diff;
                enrgl += lx * lx;
                enrgr += rx * rx;
            }
        }

        let mut ms = 0i32;
        if enrgl.min(enrgr) * thrmid >= enrgs.max(enrgd) {
            let mut phase = PH_NONE;
            if enrgs * thrmid * 2.0 >= enrgl + enrgr {
                ms = 1;
                phase = PH_IN;
            } else if enrgd * thrmid * 2.0 >= enrgl + enrgr {
                ms = 1;
                phase = PH_OUT;
            }

            if ms != 0 {
                for win in wstart..wend {
                    let off = win as usize * BLOCK_LEN_SHORT;
                    for l in start..end {
                        let (sum, diff) = if phase == PH_IN {
                            (sl0[off + l] + sr0[off + l], 0.0)
                        } else {
                            (0.0, sl0[off + l] - sr0[off + l])
                        };
                        sl0[off + l] = 0.5 * sum;
                        sr0[off + l] = 0.5 * diff;
                    }
                }
            }
        }

        if enrgl.min(enrgr) <= thrside * enrgl.max(enrgr) {
            for win in wstart..wend {
                let off = win as usize * BLOCK_LEN_SHORT;
                for l in start..end {
                    if enrgl < enrgr {
                        sl0[off + l] = 0.0;
                    } else {
                        sr0[off + l] = 0.0;
                    }
                }
            }
        }

        channel.ms_info.ms_used[*sfcnt as usize] = ms != 0;
        *sfcnt += 1;
    }
}

pub fn aac_stereo(
    coder: &mut [CoderInfo],
    channel: &mut [ChannelInfo],
    s: &mut [Vec<f64>],
    maxchan: i32,
    quality: f64,
    mode: JointMode,
) {
    let thr075: f64 = 1.09 - 1.0; // ~0.75 dB
    let thrmax: f64 = 1.25 - 1.0; // ~2 dB
    let sidemin: f64 = 0.1; // -20 dB
    let sidemax: f64 = 0.3; // ~-10.5 dB
    let isthrmax: f64 = std::f64::consts::SQRT_2 - 1.0;

    let mut thrmid = 1.0_f64;
    let mut thrside = 0.0_f64;
    let mut isthr = 1.0_f64;

    match mode {
        JointMode::Ms => {
            thrmid = thr075 / quality;
            if thrmid > thrmax {
                thrmid = thrmax;
            }
            thrside = sidemin / quality;
            if thrside > sidemax {
                thrside = sidemax;
            }
            thrmid += 1.0;
        }
        JointMode::Is => {
            isthr = 0.18 / (quality * quality);
            if isthr > isthrmax {
                isthr = isthrmax;
            }
            isthr += 1.0;
        }
        JointMode::None => {}
    }

    thrmid *= thrmid;
    thrside *= thrside;
    isthr *= isthr;

    // Pass 1: HCB_NONE init for all coders/groups/bands.
    for chn in 0..maxchan as usize {
        if !channel[chn].present {
            continue;
        }
        let cp = &mut coder[chn];
        let mut bookcnt = 0usize;
        for _group in 0..cp.groups.n {
            for _band in 0..cp.sfbn {
                cp.book[bookcnt] = HCB_NONE;
                cp.sf[bookcnt] = 0;
                bookcnt += 1;
            }
        }
    }

    // Pass 2: stereo coding per CPE left-channel.
    for chn in 0..maxchan as usize {
        if !channel[chn].present {
            continue;
        }
        if !(channel[chn].element_type == ElementType::Cpe && channel[chn].ch_is_left) {
            continue;
        }

        let rch = channel[chn].paired_ch as usize;
        channel[chn].common_window = false;
        channel[chn].ms_info.is_present = false;
        channel[rch].ms_info.is_present = false;

        if coder[chn].block_type != coder[rch].block_type {
            continue;
        }
        if coder[chn].groups.n != coder[rch].groups.n {
            continue;
        }

        channel[chn].common_window = true;
        let mut groups_match = true;
        for cnt in 0..coder[chn].groups.n as usize {
            if coder[chn].groups.len[cnt] != coder[rch].groups.len[cnt] {
                channel[chn].common_window = false;
                groups_match = false;
                break;
            }
        }
        if !groups_match {
            continue;
        }

        if mode == JointMode::Ms {
            channel[chn].common_window = true;
            channel[chn].ms_info.is_present = true;
            channel[rch].ms_info.is_present = true;
        }

        // Split coder/channel/spectrum to obtain disjoint &mut references for chn and rch.
        // Per get_channel_info, paired CPE always has chn < rch.
        let (coder_l, coder_r) = coder.split_at_mut(rch);
        let cl = &mut coder_l[chn];
        let cr = &mut coder_r[0];

        let (s_l, s_r) = s.split_at_mut(rch);
        let sl_buf = &mut s_l[chn][..];
        let sr_buf = &mut s_r[0][..];

        let (chan_l, chan_r) = channel.split_at_mut(rch);
        let ch_l = &mut chan_l[chn];
        // ch_r unused below — only ch_l holds ms_used. Re-borrow just in case.
        let _ch_r = &mut chan_r[0];

        let mut sfcnt = 0i32;
        let mut start = 0i32;
        let group_count = cl.groups.n;
        for group in 0..group_count {
            let end = start + cl.groups.len[group as usize];
            match mode {
                JointMode::Ms => {
                    midside(cl, ch_l, sl_buf, sr_buf, &mut sfcnt, start, end, thrmid, thrside);
                }
                JointMode::Is => {
                    stereo(cl, cr, sl_buf, sr_buf, &mut sfcnt, start, end, isthr);
                }
                JointMode::None => {}
            }
            start = end;
        }
    }
}
