//! The WOFF2 header

use bytes::Buf;
use four_cc::FourCC;
use thiserror::Error;

use crate::{buffer_util::BufExt, magic_numbers::WOFF2_SIGNATURE};

#[derive(Error, Debug)]
pub enum Woff2HeaderError {
    #[error("Truncated header")]
    Truncated,
    #[error("Invalid magic word")]
    InvalidMagicWord,
    #[error("Excess padding")]
    ExcessPadding,
    #[error("Overlapping streams")]
    OverlappingStreams,
}

pub struct Woff2Header {
    pub signature: FourCC,
    pub flavor: FourCC,
    pub length: u32,
    pub num_tables: u16,
    pub reserved: u16,
    pub total_sfnt_size: u32,
    pub total_compressed_size: u32,
    pub major_version: u16,
    pub minor_version: u16,
    pub meta_offset: u32,
    pub meta_length: u32,
    pub meta_orig_length: u32,
    pub private_offset: u32,
    pub private_length: u32,
}

impl Woff2Header {
    pub fn from_buf(buffer: &mut impl Buf) -> Result<Self, Woff2HeaderError> {
        if buffer.remaining() < 48 {
            return Err(Woff2HeaderError::Truncated);
        }

        Ok(Self {
            signature: buffer.get_four_cc(),
            flavor: buffer.get_four_cc(),
            length: buffer.get_u32(),
            num_tables: buffer.get_u16(),
            reserved: buffer.get_u16(),
            total_sfnt_size: buffer.get_u32(),
            total_compressed_size: buffer.get_u32(),
            major_version: buffer.get_u16(),
            minor_version: buffer.get_u16(),
            meta_offset: buffer.get_u32(),
            meta_length: buffer.get_u32(),
            meta_orig_length: buffer.get_u32(),
            private_offset: buffer.get_u32(),
            private_length: buffer.get_u32(),
        })
    }

    pub fn is_valid_header(&self) -> Result<(), Woff2HeaderError> {
        if self.signature != WOFF2_SIGNATURE {
            return Err(Woff2HeaderError::InvalidMagicWord);
        }

        // TODO: Add other checks

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::test_resources::LATO_V22_LATIN_REGULAR;

    use super::Woff2Header;

    #[test]
    fn test_header() {
        let mut buffer = Cursor::new(LATO_V22_LATIN_REGULAR);
        let header = Woff2Header::from_buf(&mut buffer).unwrap();
        assert!(header.is_valid_header().is_ok());
    }
}
