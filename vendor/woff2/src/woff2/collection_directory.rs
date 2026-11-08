//! The WOFF2 collection directory

use bytes::{Buf, BufMut};
use four_cc::FourCC;
use thiserror::Error;

use crate::buffer_util::{BufExt, SafeBuf, TruncatedError};
use crate::ttf_header::{TableDirectory, TableRecord};

#[derive(Debug, Error)]
pub enum CollectionHeaderError {
    #[error("Invalid collection header version")]
    InvalidCollectionVersion,
    #[error("Truncated collection header")]
    Truncated,
    #[error("No tables in font")]
    NoTables,
    #[error("Invalid table index")]
    InvalidTableIndex,
}

impl From<TruncatedError> for CollectionHeaderError {
    fn from(TruncatedError: TruncatedError) -> CollectionHeaderError {
        CollectionHeaderError::Truncated
    }
}

#[derive(Clone, Copy)]
#[repr(u32)]
pub enum CollectionHeaderVersion {
    V1 = 0x0001_0000,
    V2 = 0x0002_0000,
}

impl TryFrom<u32> for CollectionHeaderVersion {
    type Error = CollectionHeaderError;
    fn try_from(value: u32) -> Result<Self, CollectionHeaderError> {
        match value {
            value if value == Self::V1 as u32 => Ok(Self::V1),
            value if value == Self::V2 as u32 => Ok(Self::V2),
            _ => Err(CollectionHeaderError::InvalidCollectionVersion),
        }
    }
}

/// A WOFF2 collection directory.
pub struct CollectionHeader {
    pub version: CollectionHeaderVersion,
    pub fonts: Vec<CollectionFontEntry>,
}

impl CollectionHeader {
    /// Reads the collection directory from the buffer
    pub fn from_buf(
        buf: &mut impl Buf,
        total_num_tables: u16,
    ) -> Result<Self, CollectionHeaderError> {
        let version = buf.oc_take_u32()?.try_into()?;
        let num_fonts = buf.try_get_255_u16()?;
        let fonts = (0..num_fonts)
            .map(|_| {
                let num_tables = buf.try_get_255_u16()?;
                if num_tables == 0 {
                    return Err(CollectionHeaderError::NoTables);
                }
                let flavor = buf.try_get_four_cc()?;
                let table_indices = (0..num_tables)
                    .map(|_| {
                        let table_idx = buf.try_get_255_u16()?;
                        if table_idx >= total_num_tables {
                            Err(CollectionHeaderError::InvalidTableIndex)
                        } else {
                            Ok(table_idx)
                        }
                    })
                    .collect::<Result<_, _>>()?;
                Ok(CollectionFontEntry {
                    flavor,
                    table_indices,
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(CollectionHeader { version, fonts })
    }

    /// Calculates the total size of the OpenType Font Collection header, including the table
    /// directories for each font.
    pub fn calculate_header_size(&self) -> usize {
        // 12 bytes for header:
        // 'ttcf' tag (4 bytes), version (4 bytes), num_fonts (4 bytes)
        //
        // then, for each font:
        // table directory offset (4 bytes)
        // table directories: header (12 bytes) + records (16 bytes * num_tables)
        12 + self
            .fonts
            .iter()
            .map(|font| 4 + font.calculate_directory_size())
            .sum::<usize>()
    }

    /// Writes the OpenType Font Collection header to the buffer.
    ///
    /// # Panics
    /// Panics if the buffer does not have enough space for the header.
    pub fn write_to_buf(&self, buffer: &mut impl BufMut, tables: &[TableRecord]) {
        assert!(buffer.remaining_mut() >= self.calculate_header_size());
        buffer.put_slice(&crate::magic_numbers::TTF_COLLECTION_FLAVOR.0);
        // Always output v1, since the dsig fields in v2 need to be invalidated
        // anyway. This is allowed by the spec:
        //
        //     If the value of the version field for the TTC Header in the
        //     CollectionHeader is set to "2.0", a decoder MUST either set the
        //     TTC Header fields {ulDsigTag, ulDsigLength, ulDsigOffset} in the
        //     output collection to null or convert the TTC header format to
        //     version 1 (0x00010000).
        // (https://www.w3.org/TR/WOFF2/#collection_dir_format)
        buffer.put_u32(CollectionHeaderVersion::V1 as u32);
        buffer.put_u32(self.fonts.len() as u32);
        let font_directory_len = self.fonts.len() * std::mem::size_of::<u32>();
        let mut table_directory_offset = 12 + font_directory_len;
        for font in &self.fonts {
            buffer.put_u32(table_directory_offset as u32);
            table_directory_offset += font.calculate_directory_size();
        }
        // now write the table directories for each font
        for font in &self.fonts {
            let font_tables = font
                .table_indices
                .iter()
                .map(|&idx| tables[idx as usize])
                .collect();
            let table_directory = TableDirectory::new(font.flavor, font_tables);
            table_directory.write_to_buf(buffer);
        }
    }
}

pub struct CollectionFontEntry {
    /// The "sfnt version" of the font
    pub flavor: FourCC,
    /// Indices of tables in the WOFF2 table directory
    pub table_indices: Vec<u16>,
}

impl CollectionFontEntry {
    /// Calculates the size of the table directory for the font.
    pub fn calculate_directory_size(&self) -> usize {
        // 12 for table directory header, then the table records
        12 + self.table_indices.len() * std::mem::size_of::<TableRecord>()
    }
}
