// AAC bitstream syntax layer — the WriteCPE / WriteSCE / WriteICS / WriteADTS
// family from bitstream.c. Kept separate from `bitstream.rs` (which only has
// the byte-level bit writer) so the dependency direction is one-way:
//   bitstream → syntax → frame
//
// Each writer takes a `write` flag mirroring the C `writeFlag` parameter:
// when false, only bit accounting is performed (`bitStream` is touched at
// most by `WriteAACFillBits`, which is harmless because it returns the same
// count in both modes).

#![allow(dead_code)]

use super::writer::{
    BitStream, ID_CPE, ID_END, ID_FIL, ID_LFE, ID_SCE, LEN_BYTE, LEN_COM_WIN, LEN_F_CNT,
    LEN_GAIN_PRES, LEN_GLOB_GAIN, LEN_ICS_RESERV, LEN_MASK, LEN_MASK_PRES, LEN_MAX_SFBL,
    LEN_MAX_SFBS, LEN_PRED_PRES, LEN_PULSE_PRES, LEN_SE_ID, LEN_TAG, LEN_TNS_COEFF_RES,
    LEN_TNS_COMPRESS, LEN_TNS_DIRECTION, LEN_TNS_LENGTHL, LEN_TNS_LENGTHS, LEN_TNS_NFILTL,
    LEN_TNS_NFILTS, LEN_TNS_ORDERL, LEN_TNS_ORDERS, LEN_TNS_PRES, LEN_WIN_SEQ, LEN_WIN_SH,
    bit2byte,
};
use crate::codec::{ChannelInfo, ElementType};
use crate::codec::{CoderInfo, DEF_TNS_RES_OFFSET, MAX_SHORT_WINDOWS, WindowType};
use crate::codec::StreamFormat;

pub const ADTS_FRAMESIZE: usize = 1 << 13;

/// Per-frame state needed by the bitstream syntax layer (the bits of the
/// C `faacEncStruct` that `WriteBitstream` reads).
pub struct FrameCtx<'a> {
    pub output_format: StreamFormat,
    pub mpeg_version: u32,
    pub aac_object_type: u32,
    pub sample_rate_idx: u32,
    pub num_channels: u32,
    pub frame_num: u32,
    pub used_bytes: u32, // computed by CountBitstream, consumed by WriteADTSHeader
    pub name: &'a str,
}

// --- WriteFAACStr -------------------------------------------------------------

fn write_faac_str(stream: &mut BitStream, version: &str, write: bool) -> i32 {
    let str_buf = format!("libfaac {}", version);
    let bytes = str_buf.as_bytes();
    let len = bytes.len() + 1; // include trailing NUL like C `strlen + 1`
    let padbits = ((8 - ((stream.num_bit + 7) % 8)) % 8) as i32;
    let count = len + 3;
    let bitcnt = LEN_SE_ID + 4 + if count < 15 { 0 } else { 8 } + (count as i32) * 8;
    if !write {
        return bitcnt;
    }
    stream.put_bit(ID_FIL, LEN_SE_ID);
    if count < 15 {
        stream.put_bit(count as u64, 4);
    } else {
        stream.put_bit(15, 4);
        stream.put_bit((count - 14) as u64, 8);
    }
    stream.put_bit(0, padbits);
    stream.put_bit(0, 8);
    stream.put_bit(0, 8);
    for &b in bytes {
        stream.put_bit(b as u64, 8);
    }
    stream.put_bit(0, 8); // trailing NUL
    stream.put_bit(0, 8 - padbits);
    bitcnt
}

// --- WriteADTSHeader ----------------------------------------------------------

fn write_adts_header(ctx: &FrameCtx, stream: &mut BitStream, write: bool) -> i32 {
    let bits = 56i32;
    if write {
        stream.put_bit(0xFFFF, 12); // syncword
        stream.put_bit(ctx.mpeg_version as u64, 1);
        stream.put_bit(0, 2); // layer
        stream.put_bit(1, 1); // protection absent
        stream.put_bit((ctx.aac_object_type - 1) as u64, 2);
        stream.put_bit(ctx.sample_rate_idx as u64, 4);
        stream.put_bit(0, 1); // private bit
        stream.put_bit(ctx.num_channels as u64, 3); // channel config
        stream.put_bit(0, 1); // original/copy
        stream.put_bit(0, 1); // home
        stream.put_bit(0, 1); // copyr. id. bit
        stream.put_bit(0, 1); // copyr. id. start
        stream.put_bit(ctx.used_bytes as u64, 13);
        stream.put_bit(0x7FF, 11); // buffer fullness (0x7FF = VBR)
        stream.put_bit(0, 2); // raw_data_blocks_in_frame
    }
    bits
}

// --- CoderInfo bitstream methods ----------------------------------------------

impl CoderInfo {
    fn grouping_bits(&self) -> i32 {
        let mut grouping_bits = 0i32;
        let mut tmp = [0i32; 8];
        let mut index = 0usize;
        for i in 0..self.groups.n {
            for _j in 0..self.groups.len[i as usize] {
                tmp[index] = i;
                index += 1;
            }
        }
        for i in 1..8 {
            grouping_bits <<= 1;
            if tmp[i] == tmp[i - 1] {
                grouping_bits += 1;
            }
        }
        grouping_bits
    }

    fn write_ics_info(&self, stream: &mut BitStream, write: bool) -> i32 {
        let mut bits = 0i32;
        if write {
            stream.put_bit(0, LEN_ICS_RESERV);
            stream.put_bit(self.block_type as i32 as u64, LEN_WIN_SEQ);
            stream.put_bit(self.window_shape as u64, LEN_WIN_SH);
        }
        bits += LEN_ICS_RESERV + LEN_WIN_SEQ + LEN_WIN_SH;

        if self.block_type == WindowType::OnlyShortWindow {
            if write {
                stream.put_bit(self.sfbn as u64, LEN_MAX_SFBS);
                let g = self.grouping_bits();
                stream.put_bit(g as u64, (MAX_SHORT_WINDOWS - 1) as i32);
            }
            bits += LEN_MAX_SFBS + (MAX_SHORT_WINDOWS - 1) as i32;
        } else {
            if write {
                stream.put_bit(self.sfbn as u64, LEN_MAX_SFBL);
            }
            bits += LEN_MAX_SFBL;
            bits += 1;
            if write {
                stream.put_bit(0, LEN_PRED_PRES);
            }
        }
        bits
    }

    fn write_tns_data(&self, stream: &mut BitStream, write: bool) -> i32 {
        let mut bits = 0i32;
        let tns = &self.tns_info;

        if write {
            stream.put_bit(tns.tns_data_present as u64, LEN_TNS_PRES);
        }
        bits += LEN_TNS_PRES;

        if !tns.tns_data_present {
            return bits;
        }

        let (num_windows, len_tns_nfilt, len_tns_length, len_tns_order) =
            if self.block_type == WindowType::OnlyShortWindow {
                (MAX_SHORT_WINDOWS as i32, LEN_TNS_NFILTS, LEN_TNS_LENGTHS, LEN_TNS_ORDERS)
            } else {
                (1, LEN_TNS_NFILTL, LEN_TNS_LENGTHL, LEN_TNS_ORDERL)
            };

        bits += num_windows * len_tns_nfilt;
        for w in 0..num_windows as usize {
            let window = &tns.window_data[w];
            let num_filters = window.num_filters;
            if write {
                stream.put_bit(num_filters as u64, len_tns_nfilt);
            }
            if num_filters != 0 {
                bits += LEN_TNS_COEFF_RES;
                let res_in_bits = window.coef_resolution;
                if write {
                    stream.put_bit((res_in_bits - DEF_TNS_RES_OFFSET) as u64, LEN_TNS_COEFF_RES);
                }
                bits += num_filters * (len_tns_length + len_tns_order);
                for filt in 0..num_filters as usize {
                    let f = &window.tns_filter[filt];
                    let order = f.order;
                    if write {
                        stream.put_bit(f.length as u64, len_tns_length);
                        stream.put_bit(order as u64, len_tns_order);
                    }
                    if order != 0 {
                        bits += LEN_TNS_DIRECTION + LEN_TNS_COMPRESS;
                        if write {
                            stream.put_bit(f.direction as u64, LEN_TNS_DIRECTION);
                            stream.put_bit(f.coef_compress as u64, LEN_TNS_COMPRESS);
                        }
                        let bits_to_xmit = res_in_bits - f.coef_compress;
                        bits += order * bits_to_xmit;
                        if write {
                            for i in 1..=order as usize {
                                let mask = if bits_to_xmit >= 32 {
                                    u64::MAX
                                } else {
                                    (1u64 << bits_to_xmit) - 1
                                };
                                let v = (f.index[i] as i64 as u64) & mask;
                                stream.put_bit(v, bits_to_xmit);
                            }
                        }
                    }
                }
            }
        }
        bits
    }

    fn write_spectral_data(&self, stream: &mut BitStream, write: bool) -> i32 {
        let mut bits = 0i32;
        if write {
            for i in 0..self.datacnt as usize {
                let data = self.s[i].data;
                let len = self.s[i].len;
                if len > 0 {
                    stream.put_bit(data as i64 as u64, len);
                    bits += len;
                }
            }
        } else {
            for i in 0..self.datacnt as usize {
                bits += self.s[i].len;
            }
        }
        bits
    }

    fn write_ics(&self, stream: &mut BitStream, common_window: bool, write: bool) -> i32 {
        let mut bits = 0i32;
        if write {
            stream.put_bit(self.global_gain as u64, LEN_GLOB_GAIN);
        }
        bits += LEN_GLOB_GAIN;

        if !common_window {
            bits += self.write_ics_info(stream, write);
        }

        bits += self.write_books(stream, write);
        bits += self.write_sf(stream, write);

        bits += write_pulse_data(stream, write);
        bits += self.write_tns_data(stream, write);
        bits += write_gain_control_data(stream, write);
        bits += self.write_spectral_data(stream, write);

        bits
    }

    fn write_sce(&self, channel: &ChannelInfo, stream: &mut BitStream, write: bool) -> i32 {
        let mut bits = 0i32;
        if write {
            stream.put_bit(ID_SCE, LEN_SE_ID);
            stream.put_bit(channel.tag as u64, LEN_TAG);
        }
        bits += LEN_SE_ID + LEN_TAG;
        bits += self.write_ics(stream, false, write);
        bits
    }

    fn write_lfe(&self, channel: &ChannelInfo, stream: &mut BitStream, write: bool) -> i32 {
        let mut bits = 0i32;
        if write {
            stream.put_bit(ID_LFE, LEN_SE_ID);
            stream.put_bit(channel.tag as u64, LEN_TAG);
        }
        bits += LEN_SE_ID + LEN_TAG;
        bits += self.write_ics(stream, false, write);
        bits
    }
}

fn write_pulse_data(stream: &mut BitStream, write: bool) -> i32 {
    if write {
        stream.put_bit(0, LEN_PULSE_PRES);
    }
    LEN_PULSE_PRES
}

fn write_gain_control_data(stream: &mut BitStream, write: bool) -> i32 {
    if write {
        stream.put_bit(0, LEN_GAIN_PRES);
    }
    LEN_GAIN_PRES
}

// --- WriteCPE ----------------------------------------------------------------

fn write_cpe(
    coder_l: &CoderInfo,
    coder_r: &CoderInfo,
    channel: &ChannelInfo,
    stream: &mut BitStream,
    write: bool,
) -> i32 {
    let mut bits = 0i32;
    if write {
        stream.put_bit(ID_CPE, LEN_SE_ID);
        stream.put_bit(channel.tag as u64, LEN_TAG);
        stream.put_bit(channel.common_window as u64, LEN_COM_WIN);
    }
    bits += LEN_SE_ID + LEN_TAG + LEN_COM_WIN;

    if channel.common_window {
        bits += coder_l.write_ics_info(stream, write);
        let num_windows = coder_l.groups.n;
        let max_sfb = coder_l.sfbn;
        if write {
            stream.put_bit(channel.ms_info.is_present as u64, LEN_MASK_PRES);
            if channel.ms_info.is_present {
                for g in 0..num_windows as usize {
                    for b in 0..max_sfb as usize {
                        stream.put_bit(
                            channel.ms_info.ms_used[g * max_sfb as usize + b] as u64,
                            LEN_MASK,
                        );
                    }
                }
            }
        }
        bits += LEN_MASK_PRES;
        if channel.ms_info.is_present {
            bits += num_windows * max_sfb * LEN_MASK;
        }
    }

    bits += coder_l.write_ics(stream, channel.common_window, write);
    bits += coder_r.write_ics(stream, channel.common_window, write);
    bits
}

// --- WriteAACFillBits --------------------------------------------------------

fn write_aac_fill_bits(stream: &mut BitStream, num_bits: i32, write: bool) -> i32 {
    let mut left = num_bits;
    let min_bits = LEN_SE_ID + LEN_F_CNT;
    while left >= min_bits {
        if write {
            stream.put_bit(ID_FIL, LEN_SE_ID);
        }
        left -= min_bits;
        let mut num_bytes = left / LEN_BYTE;
        let max_count = (1i32 << LEN_F_CNT) - 1;
        if num_bytes < max_count {
            if write {
                stream.put_bit(num_bytes as u64, LEN_F_CNT);
                for _ in 0..num_bytes {
                    stream.put_bit(0, LEN_BYTE);
                }
            }
        } else {
            if write {
                stream.put_bit(max_count as u64, LEN_F_CNT);
            }
            let max_escape = (1i32 << LEN_BYTE) - 1;
            let max_total = max_count + max_escape;
            if num_bytes > max_total {
                num_bytes = max_total;
            }
            let esc_count = num_bytes - max_count;
            if write {
                stream.put_bit(esc_count as u64, LEN_BYTE);
                for _ in 0..(num_bytes - 1) {
                    stream.put_bit(0, LEN_BYTE);
                }
            }
        }
        left -= LEN_BYTE * num_bytes;
    }
    left
}

fn byte_align(stream: &mut BitStream, write: bool, bits_so_far: i32) -> i32 {
    let len = if write { stream.num_bit as i32 } else { bits_so_far };
    let mut j = (8 - (len % 8)) % 8;
    if len % 8 == 0 {
        j = 0;
    }
    if write {
        for _ in 0..j {
            stream.put_bit(0, 1);
        }
    }
    j
}

// --- Orchestrators ------------------------------------------------------------

/// Walk all SCE/CPE/LFE/Fill elements once with `write=false` to derive
/// `used_bytes`. Returns the total bit count or -1 on overflow.
fn count_bitstream(
    ctx: &mut FrameCtx,
    coder: &[CoderInfo],
    channel: &[ChannelInfo],
    stream: &mut BitStream,
    num_channel: i32,
) -> i32 {
    let mut bits = 0i32;
    if ctx.output_format == StreamFormat::Adts {
        bits += write_adts_header(ctx, stream, false);
    }

    if ctx.frame_num == 4 {
        bits += write_faac_str(stream, ctx.name, false);
    }

    for ch in 0..num_channel as usize {
        if !channel[ch].present {
            continue;
        }
        if channel[ch].element_type != ElementType::Cpe {
            if channel[ch].element_type == ElementType::Lfe {
                bits += coder[ch].write_lfe(&channel[ch], stream, false);
            } else {
                bits += coder[ch].write_sce(&channel[ch], stream, false);
            }
        } else if channel[ch].ch_is_left {
            let rch = channel[ch].paired_ch as usize;
            bits += write_cpe(
                &coder[ch],
                &coder[rch],
                &channel[ch],
                stream,
                false,
            );
        }
    }

    let num_fill_bits = if bits < (8 - LEN_SE_ID) {
        8 - LEN_SE_ID - bits
    } else {
        0
    };
    let num_fill_bits = num_fill_bits + 6;
    let left = write_aac_fill_bits(stream, num_fill_bits, false);
    bits += num_fill_bits - left;
    bits += LEN_SE_ID;
    bits += byte_align(stream, false, bits);

    ctx.used_bytes = bit2byte(bits as i64) as u32;

    if (ctx.used_bytes as usize) > stream.size() {
        eprintln!("frame buffer overrun");
        return -1;
    }
    if ctx.used_bytes >= ADTS_FRAMESIZE as u32 {
        eprintln!("frame size limit exceeded");
        return -1;
    }
    bits
}

/// Two-pass write: count first (sets `used_bytes`), then emit. Returns total
/// bits written or -1 on error.
pub fn write_bitstream(
    ctx: &mut FrameCtx,
    coder: &[CoderInfo],
    channel: &[ChannelInfo],
    stream: &mut BitStream,
    num_channel: i32,
) -> i32 {
    if count_bitstream(ctx, coder, channel, stream, num_channel) < 0 {
        return -1;
    }
    // Reset stream cursor for the actual write pass: count_bitstream tried to
    // write fill bits (count-only path doesn't touch buffer) but the C
    // version's bitstream is freshly opened here. Our caller opens it once,
    // so reset cursor manually.
    stream.num_bit = 0;
    stream.current_bit = 0;
    for b in stream.data.iter_mut() {
        *b = 0;
    }

    let mut bits = 0i32;
    if ctx.output_format == StreamFormat::Adts {
        bits += write_adts_header(ctx, stream, true);
    }
    if ctx.frame_num == 4 {
        bits += write_faac_str(stream, ctx.name, true);
    }

    for ch in 0..num_channel as usize {
        if !channel[ch].present {
            continue;
        }
        if channel[ch].element_type != ElementType::Cpe {
            if channel[ch].element_type == ElementType::Lfe {
                bits += coder[ch].write_lfe(&channel[ch], stream, true);
            } else {
                bits += coder[ch].write_sce(&channel[ch], stream, true);
            }
        } else if channel[ch].ch_is_left {
            let rch = channel[ch].paired_ch as usize;
            bits += write_cpe(
                &coder[ch],
                &coder[rch],
                &channel[ch],
                stream,
                true,
            );
        }
    }

    let num_fill_bits = if bits < (8 - LEN_SE_ID) {
        8 - LEN_SE_ID - bits
    } else {
        0
    };
    let num_fill_bits = num_fill_bits + 6;
    let left = write_aac_fill_bits(stream, num_fill_bits, true);
    bits += num_fill_bits - left;

    bits += LEN_SE_ID;
    stream.put_bit(ID_END, LEN_SE_ID);

    bits += byte_align(stream, true, bits);
    bits
}
