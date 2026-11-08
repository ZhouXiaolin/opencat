use std::num::Wrapping;

use bytes::BufMut;
use thiserror::Error;

/// Calculates the sum of (big-endian) `u32`s in a block of data.
///
/// If the data is not a multiple of 4 bytes long, it is treated as if padded with zeroes at the
/// end.
pub fn calculate_checksum(data: &[u8]) -> u32 {
    let chunks = data.chunks_exact(4);
    // maybe there's a more elegant way of doing this, but i think this is alright
    let last = u32::from_be_bytes(match chunks.remainder() {
        &[] => [0; 4],
        &[b0] => [b0, 0, 0, 0],
        &[b0, b1] => [b0, b1, 0, 0],
        &[b0, b1, b2] => [b0, b1, b2, 0],
        _ => unreachable!("ChunksExact::remainder is guaranteed to return a slice of length < n"),
    });
    (chunks
        // we can get rid of this `try_into().unwrap()` once `&[T]::array_chunks` is stabilized
        .map(|slice| Wrapping(u32::from_be_bytes(slice.try_into().unwrap())))
        .sum::<Wrapping<u32>>()
        + Wrapping(last))
    .0
}

#[derive(Debug, Error)]
pub enum ChecksumError {
    #[error("Truncated `head` table")]
    Truncated,
}

/// Sets the `checksum_adjustment` field in the `head` table to the specified value.
pub fn set_checksum_adjustment(head_table: &mut [u8], value: u32) -> Result<(), ChecksumError> {
    // table version: 4 bytes
    // font revision: 4 bytes
    // checksum adjustment: 4 bytes
    if head_table.len() < 12 {
        return Err(ChecksumError::Truncated);
    }
    let mut checksum_field = &mut head_table[8..12];
    checksum_field.put_u32(value);
    Ok(())
}

/// Calculates the value for the `checksum_adjustment` field in the `head` table.
pub fn calculate_font_checksum_adjustment(font: &[u8]) -> u32 {
    const CHECKSUM_MINUEND: u32 = 0xB1B0AFBA;
    let checksum = calculate_checksum(font);
    CHECKSUM_MINUEND.wrapping_sub(checksum)
}
