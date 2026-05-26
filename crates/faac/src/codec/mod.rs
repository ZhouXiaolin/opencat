pub mod channels;
mod coder;
mod config;

pub use channels::{ChannelInfo, ElementType};
pub use config::{Configuration, SrInfo};
pub use coder::{CoderInfo, SpectralData};

use std::f64::consts::PI;

pub const FRAME_LEN: usize = 1024;
pub const BLOCK_LEN_LONG: usize = 1024;
pub const BLOCK_LEN_SHORT: usize = 128;

pub const NSFB_LONG: usize = 51;
pub const NSFB_SHORT: usize = 15;
pub const MAX_SHORT_WINDOWS: usize = 8;
pub const MAX_SCFAC_BANDS: usize = (NSFB_SHORT + 1) * MAX_SHORT_WINDOWS;
pub const MAX_CHANNELS: usize = 64;
pub const NFLAT_LS: usize = 448;

pub const FAAC_CFG_VERSION: i32 = 105;

pub const MPEG2: u32 = 1;
pub const MPEG4: u32 = 0;

pub const MAIN: u32 = 1;
pub const LOW: u32 = 2;
pub const SSR: u32 = 3;
pub const LTP: u32 = 4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum InputFormat {
    I16 = 1,
    I24 = 2,
    I32 = 3,
    F32 = 4,
}

pub const FAAC_INPUT_NULL: u32 = 0;
pub const FAAC_INPUT_16BIT: u32 = InputFormat::I16 as u32;
pub const FAAC_INPUT_24BIT: u32 = InputFormat::I24 as u32;
pub const FAAC_INPUT_32BIT: u32 = InputFormat::I32 as u32;
pub const FAAC_INPUT_FLOAT: u32 = InputFormat::F32 as u32;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(i32)]
pub enum ShortControl {
    Normal = 0,
    NoShort = 1,
    NoLong = 2,
}

pub const SHORTCTL_NORMAL: i32 = ShortControl::Normal as i32;
pub const SHORTCTL_NOSHORT: i32 = ShortControl::NoShort as i32;
pub const SHORTCTL_NOLONG: i32 = ShortControl::NoLong as i32;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum StreamFormat {
    Raw = 0,
    Adts = 1,
}

pub const RAW_STREAM: u32 = StreamFormat::Raw as u32;
pub const ADTS_STREAM: u32 = StreamFormat::Adts as u32;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum JointMode {
    None = 0,
    Ms = 1,
    Is = 2,
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(i32)]
pub enum WindowShape {
    Sine = 0,
    Kbd = 1,
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[repr(i32)]
pub enum WindowType {
    #[default]
    OnlyLongWindow = 0,
    LongShortWindow = 1,
    OnlyShortWindow = 2,
    ShortLongWindow = 3,
}

pub use crate::coding::DEF_TNS_RES_OFFSET;

pub const DATASIZE: usize = 3 * FRAME_LEN / 2;

pub const TWOPI: f64 = 2.0 * PI;
