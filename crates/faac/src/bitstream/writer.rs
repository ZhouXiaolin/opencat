// 1:1 port of faac/libfaac/bitstream.c — low-level bit writer.
//
// The AAC syntax layer (WriteBitstream / WriteCPE / WriteICS / etc.) lives in
// the same C file but depends on `faacEncStruct`, `ChannelInfo`, the huffman
// writers, etc. Those land in `bitstream::syntax` later once the dependent
// modules are ported. The bit-packing primitives below are usable today.

#![allow(dead_code)]

/// Bits per byte.
pub const BYTE_NUMBIT: i32 = 8;
/// Bits in an `unsigned long` on the original C side.
pub const LONG_NUMBIT: i32 = 32;

/// Helpers for converting bit counts to byte counts (matches C `bit2byte`).
#[inline]
pub fn bit2byte(a: i64) -> i64 {
    (a + BYTE_NUMBIT as i64 - 1) / BYTE_NUMBIT as i64
}

// --- Raw bitstream syntax-element constants -----------------------------------

pub const LEN_SE_ID: i32 = 3;
pub const LEN_TAG: i32 = 4;
pub const LEN_GLOB_GAIN: i32 = 8;
pub const LEN_COM_WIN: i32 = 1;
pub const LEN_ICS_RESERV: i32 = 1;
pub const LEN_WIN_SEQ: i32 = 2;
pub const LEN_WIN_SH: i32 = 1;
pub const LEN_MAX_SFBL: i32 = 6;
pub const LEN_MAX_SFBS: i32 = 4;
pub const LEN_PRED_PRES: i32 = 1;
pub const LEN_MASK_PRES: i32 = 2;
pub const LEN_MASK: i32 = 1;
pub const LEN_PULSE_PRES: i32 = 1;

pub const LEN_TNS_PRES: i32 = 1;
pub const LEN_TNS_NFILTL: i32 = 2;
pub const LEN_TNS_NFILTS: i32 = 1;
pub const LEN_TNS_COEFF_RES: i32 = 1;
pub const LEN_TNS_LENGTHL: i32 = 6;
pub const LEN_TNS_LENGTHS: i32 = 4;
pub const LEN_TNS_ORDERL: i32 = 5;
pub const LEN_TNS_ORDERS: i32 = 3;
pub const LEN_TNS_DIRECTION: i32 = 1;
pub const LEN_TNS_COMPRESS: i32 = 1;
pub const LEN_GAIN_PRES: i32 = 1;

pub const LEN_F_CNT: i32 = 4;
pub const LEN_BYTE: i32 = 8;

// --- Syntax-element IDs -------------------------------------------------------

pub const ID_SCE: u64 = 0;
pub const ID_CPE: u64 = 1;
pub const ID_CCE: u64 = 2;
pub const ID_LFE: u64 = 3;
pub const ID_DSE: u64 = 4;
pub const ID_PCE: u64 = 5;
pub const ID_FIL: u64 = 6;
pub const ID_END: u64 = 7;

/// Big-endian, MSB-first bit writer backed by a caller-supplied byte buffer.
///
/// Mirrors the C `BitStream` struct field-for-field except for the `size`
/// member (replaced by `data.len()`) and the unused `numByte`. The buffer is
/// borrowed mutably for the BitStream's lifetime.
pub struct BitStream<'a> {
    pub data: &'a mut [u8],
    pub num_bit: i64,
    pub current_bit: i64,
}

impl<'a> BitStream<'a> {
    /// OpenBitStream: zero-fill the caller buffer and reset counters.
    pub fn open(buffer: &'a mut [u8]) -> Self {
        buffer.fill(0);
        Self {
            data: buffer,
            num_bit: 0,
            current_bit: 0,
        }
    }

    /// `size` in C is the buffer capacity in bytes.
    #[inline]
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// CloseBitStream returns the number of bytes occupied (ceil of num_bit/8).
    pub fn close(self) -> usize {
        bit2byte(self.num_bit) as usize
    }

    /// `BufferNumBit` accessor for the syntax layer.
    #[inline]
    pub fn num_bits(&self) -> i64 {
        self.num_bit
    }

    /// Pack `num_bit` MSB-first bits of `data` into the stream. Caller is
    /// responsible for ensuring `data` fits in `num_bit` bits — extra high
    /// bits are masked off here, matching the C code.
    ///
    /// Constraint: `0 <= num_bit < 64`. The original code declares `unsigned
    /// long data` which is 32 or 64 bits depending on the C platform; the
    /// encoder never writes more than ~19 bits at a time (Huffman codeword
    /// length cap), so `u64` is safe.
    pub fn put_bit(&mut self, mut data: u64, mut num_bit: i32) {
        if num_bit == 0 {
            return;
        }

        let current_bit = self.current_bit as usize;
        let bit_offset = current_bit & 7;
        let mut byte_idx = current_bit >> 3;

        // Advance the bookkeeping immediately (mirrors C: it writes to data
        // through a local pointer, but updates BitStream first).
        self.current_bit += num_bit as i64;
        self.num_bit = self.current_bit;

        // Mask input to numBit bits so spurious high bits cannot leak in.
        data &= (1u64 << num_bit) - 1;

        if bit_offset + num_bit as usize <= 8 {
            // Fast path: write fits in one byte.
            if bit_offset == 0 {
                self.data[byte_idx] = 0;
            }
            let shift = 8 - bit_offset - num_bit as usize;
            self.data[byte_idx] |= (data << shift) as u8;
        } else {
            // Multi-byte write: first partial byte, then whole bytes, then
            // trailing partial byte.
            let first_bits = 8 - bit_offset;
            if bit_offset == 0 {
                self.data[byte_idx] = 0;
            }
            self.data[byte_idx] |= (data >> (num_bit as usize - first_bits)) as u8;
            byte_idx += 1;
            num_bit -= first_bits as i32;

            while num_bit >= 8 {
                self.data[byte_idx] = ((data >> (num_bit - 8)) & 0xFF) as u8;
                byte_idx += 1;
                num_bit -= 8;
            }

            if num_bit > 0 {
                let shift = 8 - num_bit as usize;
                self.data[byte_idx] = ((data & ((1u64 << num_bit) - 1)) << shift) as u8;
            }
        }
    }

    /// `ByteAlign(bitStream, writeFlag, bitsSoFar)`. Returns the number of
    /// padding bits added (or that would be added). When called in
    /// "count-only" mode the C version uses `bitsSoFar`; when writing it
    /// uses the actual cursor. We expose this via two methods so callers do
    /// not have to pass a flag.
    pub fn byte_align_write(&mut self) -> i32 {
        let len = self.num_bit;
        let j = ((8 - (len % 8)) % 8) as i32;
        for _ in 0..j {
            self.put_bit(0, 1);
        }
        j
    }

    /// Count-only variant: returns how many padding bits would be added given
    /// `bits_so_far`, without touching the stream.
    pub fn byte_align_count(bits_so_far: i32) -> i32 {
        (8 - (bits_so_far % 8)) % 8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_bit_msb_first_single_byte() {
        let mut buf = [0u8; 4];
        let mut bs = BitStream::open(&mut buf);
        bs.put_bit(0b1, 1); // bit 7
        bs.put_bit(0b01, 2); // bits 5-6 = 01
        bs.put_bit(0b1, 1); // bit 4
        bs.put_bit(0b1010, 4); // bits 0-3
        assert_eq!(bs.num_bit, 8);
        assert_eq!(bs.data[0], 0b1_01_1_1010);
    }

    #[test]
    fn put_bit_spans_bytes() {
        let mut buf = [0u8; 4];
        let mut bs = BitStream::open(&mut buf);
        bs.put_bit(0xFFFF, 12); // syncword: high 12 bits == 1111_1111_1111
        assert_eq!(bs.data[0], 0xFF);
        assert_eq!(bs.data[1] & 0xF0, 0xF0);
        assert_eq!(bs.num_bit, 12);
    }

    #[test]
    fn put_bit_long_value_split_across_3_bytes() {
        let mut buf = [0u8; 8];
        let mut bs = BitStream::open(&mut buf);
        // Write 4 high bits to align, then a 16-bit value spanning two bytes.
        bs.put_bit(0xA, 4); // 0b1010 in upper nibble of buf[0]
        bs.put_bit(0xBEEF, 16);
        // Expect: 0xAB, 0xEE, 0xF0
        assert_eq!(bs.data[0], 0xAB);
        assert_eq!(bs.data[1], 0xEE);
        assert_eq!(bs.data[2], 0xF0);
        assert_eq!(bs.num_bit, 20);
    }

    #[test]
    fn close_rounds_up_to_byte() {
        let mut buf = [0u8; 4];
        let mut bs = BitStream::open(&mut buf);
        bs.put_bit(0b101, 3);
        assert_eq!(bs.close(), 1);

        let mut buf2 = [0u8; 4];
        let mut bs2 = BitStream::open(&mut buf2);
        bs2.put_bit(0xFF, 8);
        bs2.put_bit(0b1, 1);
        assert_eq!(bs2.close(), 2);
    }

    #[test]
    fn byte_align_pads_to_byte_boundary() {
        let mut buf = [0u8; 4];
        let mut bs = BitStream::open(&mut buf);
        bs.put_bit(0xF, 4);
        let padded = bs.byte_align_write();
        assert_eq!(padded, 4);
        assert_eq!(bs.num_bit, 8);
        assert_eq!(bs.data[0] & 0x0F, 0); // padding is zero bits
    }
}
