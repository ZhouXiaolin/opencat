#![doc = include_str!("../readme.md")]
pub mod decode;

mod buffer_util;
mod checksum;
mod glyf_decoder;
mod magic_numbers;
mod ttf_header;
mod woff2;

#[cfg(test)]
mod test_resources;

pub use decode::convert_woff2_to_ttf;
