//! The WOFF2 table directory

use bytes::Buf;
use four_cc::FourCC;
use thiserror::Error;

use crate::{
    buffer_util::{pad_to_multiple_of_four, Base128Error, BufExt, SafeBuf, TruncatedError},
    checksum::{calculate_checksum, set_checksum_adjustment, ChecksumError},
    glyf_decoder::{decode_glyf_table, GlyfDecoderError},
    ttf_header::TableRecord,
};

#[derive(Error, Debug)]
pub enum TableDirectoryError {
    #[error("Table Directory truncated")]
    Truncated,
    #[error("Invalid numeric value")]
    InvalidNumeric,
}

impl From<Base128Error> for TableDirectoryError {
    fn from(e: Base128Error) -> Self {
        match e {
            Base128Error::Truncated => TableDirectoryError::Truncated,
            _ => TableDirectoryError::InvalidNumeric,
        }
    }
}

impl From<TruncatedError> for TableDirectoryError {
    fn from(TruncatedError: TruncatedError) -> Self {
        TableDirectoryError::Truncated
    }
}
/// A WOFF2 table directory.
pub struct Woff2TableDirectory {
    pub tables: Vec<TableDirectoryEntry>,
    pub uncompressed_length: u32,
}

impl Woff2TableDirectory {
    /// Read the table directory from the buffer, returning the directory entries and the total length
    /// of the uncompressed data.
    pub fn from_buf(buffer: &mut impl Buf, num_tables: u16) -> Result<Self, TableDirectoryError> {
        let mut tables = Vec::with_capacity(num_tables as usize);
        let mut src_offset: u32 = 0;

        for _ in 0..num_tables {
            let entry = PartialTableDirectoryEntry::from_buf(buffer)?;
            let src_length = entry.transform_length.unwrap_or(entry.orig_length);
            let complete_entry = TableDirectoryEntry {
                transformed: entry.transformed,
                tag: entry.tag,
                dest_length: entry.orig_length,
                src_length,
                src_offset,
            };
            tables.push(complete_entry);
            src_offset += src_length;
        }
        Ok(Woff2TableDirectory {
            tables,
            uncompressed_length: src_offset,
        })
    }

    /// Copies tables (and transforms as necessary) into an output buffer, returning the final
    /// table records.
    ///
    /// Transformed `glyf` and `loca` tables are handled here. Currently, transformed `hmtx` tables are
    /// not supported.
    pub fn write_to_buf(
        &self,
        out_buffer: &mut Vec<u8>,
        decompressed_tables: &[u8],
    ) -> Result<Vec<TableRecord>, WriteTablesError> {
        // header size should always be a multiple of four
        assert_eq!(out_buffer.len() & 3, 0);
        let num_tables = self.tables.len();
        let mut ttf_tables = Vec::with_capacity(num_tables);
        let mut tables_iter = self.tables.iter();
        while let Some(&table) = tables_iter.next() {
            match table.tag {
                GLYF_TAG => {
                    let next_table = tables_iter
                        .next()
                        .ok_or(WriteTablesError::MissingLocaTable)?;
                    if next_table.tag != LOCA_TAG {
                        return Err(WriteTablesError::MissingLocaTable);
                    }
                    if next_table.transformed != table.transformed {
                        return Err(WriteTablesError::GlyfLocaDifferentTransform);
                    }
                    if table.transformed {
                        let (glyf, loca) =
                            decode_glyf_table(&decompressed_tables[table.get_source_range()])?;
                        ttf_tables.push(TableRecord {
                            tag: table.tag,
                            checksum: calculate_checksum(&glyf),
                            offset: out_buffer.len() as u32,
                            length: glyf.len() as u32,
                        });
                        out_buffer.extend_from_slice(&glyf);
                        pad_to_multiple_of_four(out_buffer);
                        ttf_tables.push(TableRecord {
                            tag: next_table.tag,
                            checksum: calculate_checksum(&loca),
                            offset: out_buffer.len() as u32,
                            length: loca.len() as u32,
                        });
                        out_buffer.extend_from_slice(&loca);
                        pad_to_multiple_of_four(out_buffer);
                    } else {
                        push_simple_table_record(
                            table,
                            decompressed_tables,
                            out_buffer,
                            &mut ttf_tables,
                        );
                        push_simple_table_record(
                            *next_table,
                            decompressed_tables,
                            out_buffer,
                            &mut ttf_tables,
                        );
                    }
                }
                // we handle `loca` table with `glyf` above
                LOCA_TAG => return Err(WriteTablesError::MissingGlyfTable),
                HEAD_TAG => {
                    let offset = out_buffer.len();
                    let src = &decompressed_tables[table.get_source_range()];
                    out_buffer.extend_from_slice(src);
                    let head_table = &mut out_buffer[offset..];
                    set_checksum_adjustment(head_table, 0)?;
                    ttf_tables.push(TableRecord {
                        tag: table.tag,
                        checksum: calculate_checksum(head_table),
                        offset: offset as u32,
                        length: head_table.len() as u32,
                    });
                    pad_to_multiple_of_four(out_buffer);
                }
                HMTX_TAG if table.transformed => {
                    return Err(WriteTablesError::Unsupported("transformed hmtx table"));
                }
                _ => push_simple_table_record(
                    table,
                    decompressed_tables,
                    out_buffer,
                    &mut ttf_tables,
                ),
            }
        }
        assert_eq!(ttf_tables.len(), num_tables);
        Ok(ttf_tables)
    }
}

/// A WOFF2 table directory entry.
#[derive(Debug, Copy, Clone)]
pub struct TableDirectoryEntry {
    pub transformed: bool,
    pub tag: FourCC,
    /// The original length of the table (before any transformations)
    pub dest_length: u32,
    /// The length of the table in the decompressed table data
    pub src_length: u32,
    /// The starting offset of the table in the decompressed table data
    pub src_offset: u32,
}

impl TableDirectoryEntry {
    /// Returns the range occupied by the table in the decompressed table data (e.g. for slicing)
    pub fn get_source_range(&self) -> std::ops::Range<usize> {
        self.src_offset as usize..self.src_offset as usize + self.src_length as usize
    }
}

struct PartialTableDirectoryEntry {
    transformed: bool,
    tag: FourCC,
    orig_length: u32,
    transform_length: Option<u32>,
}

impl PartialTableDirectoryEntry {
    fn from_buf(buffer: &mut impl Buf) -> Result<Self, TableDirectoryError> {
        let flags = buffer.oc_take_u8()?;
        let preprocessing_transformation_version = flags & 0xC0;
        let table_ref = flags & 0x3f;
        let tag = if table_ref == 0x3f {
            buffer.try_get_four_cc()?
        } else {
            KNOWN_TABLE_TAGS[table_ref as usize]
        };

        let orig_length = buffer.try_get_base_128()?;
        let is_null_transform = if tag == FourCC(*b"glyf") || tag == FourCC(*b"loca") {
            preprocessing_transformation_version == 0xC0
        } else {
            preprocessing_transformation_version == 0x00
        };
        let transform_length = if is_null_transform {
            None
        } else {
            Some(buffer.try_get_base_128()?)
        };

        Ok(PartialTableDirectoryEntry {
            transformed: !is_null_transform,
            tag,
            orig_length,
            transform_length,
        })
    }
}

const KNOWN_TABLE_TAGS: [FourCC; 63] = [
    FourCC(*b"cmap"),
    FourCC(*b"head"),
    FourCC(*b"hhea"),
    FourCC(*b"hmtx"),
    FourCC(*b"maxp"),
    FourCC(*b"name"),
    FourCC(*b"OS/2"),
    FourCC(*b"post"),
    FourCC(*b"cvt "),
    FourCC(*b"fpgm"),
    FourCC(*b"glyf"),
    FourCC(*b"loca"),
    FourCC(*b"prep"),
    FourCC(*b"CFF "),
    FourCC(*b"VORG"),
    FourCC(*b"EBDT"),
    FourCC(*b"EBLC"),
    FourCC(*b"gasp"),
    FourCC(*b"hdmx"),
    FourCC(*b"kern"),
    FourCC(*b"LTSH"),
    FourCC(*b"PCLT"),
    FourCC(*b"VDMX"),
    FourCC(*b"vhea"),
    FourCC(*b"vmtx"),
    FourCC(*b"BASE"),
    FourCC(*b"GDEF"),
    FourCC(*b"GPOS"),
    FourCC(*b"GSUB"),
    FourCC(*b"EBSC"),
    FourCC(*b"JSTF"),
    FourCC(*b"MATH"),
    FourCC(*b"CBDT"),
    FourCC(*b"CBLC"),
    FourCC(*b"COLR"),
    FourCC(*b"CPAL"),
    FourCC(*b"SVG "),
    FourCC(*b"sbix"),
    FourCC(*b"acnt"),
    FourCC(*b"avar"),
    FourCC(*b"bdat"),
    FourCC(*b"bloc"),
    FourCC(*b"bsln"),
    FourCC(*b"cvar"),
    FourCC(*b"fdsc"),
    FourCC(*b"feat"),
    FourCC(*b"fmtx"),
    FourCC(*b"fvar"),
    FourCC(*b"gvar"),
    FourCC(*b"hsty"),
    FourCC(*b"just"),
    FourCC(*b"lcar"),
    FourCC(*b"mort"),
    FourCC(*b"morx"),
    FourCC(*b"opbd"),
    FourCC(*b"prop"),
    FourCC(*b"trak"),
    FourCC(*b"Zapf"),
    FourCC(*b"Silf"),
    FourCC(*b"Glat"),
    FourCC(*b"Gloc"),
    FourCC(*b"Feat"),
    FourCC(*b"Sill"),
];

pub const GLYF_TAG: FourCC = FourCC(*b"glyf");
pub const LOCA_TAG: FourCC = FourCC(*b"loca");
pub const HEAD_TAG: FourCC = FourCC(*b"head");
pub const HMTX_TAG: FourCC = FourCC(*b"hmtx");

#[derive(Debug, Error)]
pub enum WriteTablesError {
    #[error("glyf table not followed by loca table")]
    MissingLocaTable,
    #[error("loca table not preceded by glyf table")]
    MissingGlyfTable,
    #[error("glyf table and loca table have different transformations")]
    GlyfLocaDifferentTransform,
    #[error("Truncated `head` table")]
    TruncatedHeadTable,
    #[error("Unsupported feature: {0}")]
    Unsupported(&'static str),
    #[error(transparent)]
    GlyfDecoderError(#[from] GlyfDecoderError),
}

impl From<ChecksumError> for WriteTablesError {
    fn from(e: ChecksumError) -> WriteTablesError {
        match e {
            ChecksumError::Truncated => WriteTablesError::TruncatedHeadTable,
        }
    }
}

fn push_simple_table_record(
    table: TableDirectoryEntry,
    decompressed_tables: &[u8],
    out_buffer: &mut Vec<u8>,
    ttf_tables: &mut Vec<TableRecord>,
) {
    let src = &decompressed_tables[table.get_source_range()];
    ttf_tables.push(TableRecord {
        tag: table.tag,
        checksum: calculate_checksum(src),
        offset: out_buffer.len() as u32,
        length: src.len() as u32,
    });
    out_buffer.extend_from_slice(src);
    pad_to_multiple_of_four(out_buffer);
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use four_cc::FourCC;

    use super::Woff2TableDirectory;
    use crate::{test_resources::LATO_V22_LATIN_REGULAR, woff2::header::Woff2Header};

    #[test]
    fn test_sample_font() {
        let mut buffer = Cursor::new(LATO_V22_LATIN_REGULAR);
        let header = Woff2Header::from_buf(&mut buffer).unwrap();
        let tables = Woff2TableDirectory::from_buf(&mut buffer, header.num_tables).unwrap();

        let expected_tags: Vec<_> = [
            *b"GPOS", *b"GSUB", *b"OS/2", *b"cmap", *b"cvt ", *b"fpgm", *b"gasp", *b"glyf",
            *b"loca", *b"head", *b"hhea", *b"hmtx", *b"maxp", *b"name", *b"post", *b"prep",
        ]
        .iter()
        .map(|s| FourCC(*s))
        .collect();

        assert_eq!(
            expected_tags,
            tables
                .tables
                .iter()
                .map(|table| table.tag)
                .collect::<Vec<_>>()
        )
    }
}
