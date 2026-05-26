use super::{
    WindowShape, WindowType, DATASIZE, MAX_SCFAC_BANDS, MAX_SHORT_WINDOWS, NSFB_LONG,
};
use crate::coding::TnsInfo;

#[derive(Clone, Copy)]
pub struct SpectralData {
    pub data: i32,
    pub len: i32,
}

impl Default for SpectralData {
    fn default() -> Self {
        Self { data: 0, len: 0 }
    }
}

#[derive(Clone)]
pub struct Groups {
    pub n: i32,
    pub len: [i32; MAX_SHORT_WINDOWS],
}

impl Default for Groups {
    fn default() -> Self {
        Self {
            n: 0,
            len: [0; MAX_SHORT_WINDOWS],
        }
    }
}

#[derive(Clone)]
pub struct CoderInfo {
    pub window_shape: WindowShape,
    pub prev_window_shape: WindowShape,
    pub block_type: WindowType,
    pub desired_block_type: WindowType,
    pub global_gain: i32,
    pub sf: [i32; MAX_SCFAC_BANDS],
    pub book: [i32; MAX_SCFAC_BANDS],
    pub bandcnt: i32,
    pub sfbn: i32,
    pub sfb_offset: [i32; NSFB_LONG + 1],
    pub groups: Groups,
    pub s: [SpectralData; DATASIZE],
    pub datacnt: i32,
    pub tns_info: TnsInfo,
}

impl Default for CoderInfo {
    fn default() -> Self {
        Self {
            window_shape: WindowShape::Sine,
            prev_window_shape: WindowShape::Sine,
            block_type: WindowType::OnlyLongWindow,
            desired_block_type: WindowType::OnlyLongWindow,
            global_gain: 0,
            sf: [0; MAX_SCFAC_BANDS],
            book: [0; MAX_SCFAC_BANDS],
            bandcnt: 0,
            sfbn: 0,
            sfb_offset: [0; NSFB_LONG + 1],
            groups: Groups::default(),
            s: [SpectralData::default(); DATASIZE],
            datacnt: 0,
            tns_info: TnsInfo::default(),
        }
    }
}
