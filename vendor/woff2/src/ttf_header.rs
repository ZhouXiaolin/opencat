//! Types representing OpenType table directories

use bytes::BufMut;
use four_cc::FourCC;

/// Calculates the size of the OpenType table directory
pub fn calculate_header_size(num_tables: usize) -> usize {
    // sfnt_version:   4 bytes
    // num_tables:     2 bytes
    // search_range:   2 bytes
    // entry_selector: 2 bytes
    // range_shift:    2 bytes
    // table_records:  size_of::<TableRecord>() * num_tables
    12 + std::mem::size_of::<TableRecord>() * num_tables
}

/// An OpenType table directory
pub struct TableDirectory {
    sfnt_version: FourCC,
    num_tables: u16,
    search_range: u16,
    entry_selector: u16,
    range_shift: u16,
    table_records: Vec<TableRecord>,
}

impl TableDirectory {
    /// Build a new table directory, sorting the table records.
    pub fn new(sfnt_version: FourCC, mut table_records: Vec<TableRecord>) -> Self {
        table_records.sort_unstable_by_key(|table| table.tag.0);
        let num_tables: u16 = table_records
            .len()
            .try_into()
            .expect("more than u16::MAX tables!");
        // floor(log2(num_tables))
        let entry_selector = 15 - num_tables.leading_zeros() as u16;
        // (2**entry_selector) * 16
        let search_range = 1 << (entry_selector + 4);
        // num_tables * 16 - search_range
        let range_shift = (num_tables << 4) - search_range;
        TableDirectory {
            sfnt_version,
            num_tables,
            search_range,
            entry_selector,
            range_shift,
            table_records,
        }
    }

    pub fn write_to_buf(&self, buffer: &mut impl BufMut) {
        assert!(buffer.remaining_mut() >= calculate_header_size(self.table_records.len()));
        buffer.put_slice(&self.sfnt_version.0);
        buffer.put_u16(self.num_tables);
        buffer.put_u16(self.search_range);
        buffer.put_u16(self.entry_selector);
        buffer.put_u16(self.range_shift);
        for table in &self.table_records {
            table.write_to_buf(buffer);
        }
    }

    /// Finds the specified table record.
    pub fn find_table(&self, table_tag: FourCC) -> Option<TableRecord> {
        self.table_records
            .binary_search_by_key(&table_tag.0, |table| table.tag.0)
            .ok()
            .map(|idx| self.table_records[idx])
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TableRecord {
    pub tag: FourCC,
    pub checksum: u32,
    pub offset: u32,
    pub length: u32,
}

impl TableRecord {
    pub fn write_to_buf(&self, buffer: &mut impl BufMut) {
        buffer.put_slice(&self.tag.0);
        buffer.put_u32(self.checksum);
        buffer.put_u32(self.offset);
        buffer.put_u32(self.length);
    }

    pub fn get_range(&self) -> std::ops::Range<usize> {
        self.offset as usize..self.offset as usize + self.length as usize
    }
}
