use super::{
    FAAC_CFG_VERSION, InputFormat, JointMode, LOW, MAX_CHANNELS, MPEG4,
    NSFB_LONG, NSFB_SHORT, ShortControl, StreamFormat,
};

#[derive(Clone)]
pub struct SrInfo {
    pub sampling_rate: u32,
    pub num_cb_long: i32,
    pub num_cb_short: i32,
    pub cb_width_long: [i32; NSFB_LONG],
    pub cb_width_short: [i32; NSFB_SHORT],
}

#[derive(Clone)]
pub struct Configuration {
    pub version: i32,
    pub mpeg_version: u32,
    pub aac_object_type: u32,
    pub jointmode: JointMode,
    pub use_lfe: bool,
    pub use_tns: bool,
    pub bit_rate: u64,
    pub band_width: u32,
    pub quantqual: u64,
    pub output_format: StreamFormat,
    pub psymodelidx: u32,
    pub input_format: InputFormat,
    pub shortctl: ShortControl,
    pub channel_map: [i32; MAX_CHANNELS],
    pub pnslevel: i32,
}

impl Default for Configuration {
    fn default() -> Self {
        let mut channel_map = [0i32; MAX_CHANNELS];
        for (i, v) in channel_map.iter_mut().enumerate() {
            *v = i as i32;
        }
        Self {
            version: FAAC_CFG_VERSION,
            mpeg_version: MPEG4,
            aac_object_type: LOW,
            jointmode: JointMode::Is,
            use_lfe: true,
            use_tns: false,
            bit_rate: 64000,
            band_width: 0,
            quantqual: 0,
            output_format: StreamFormat::Adts,
            psymodelidx: 0,
            input_format: InputFormat::I32,
            shortctl: ShortControl::Normal,
            channel_map,
            pnslevel: 4,
        }
    }
}
