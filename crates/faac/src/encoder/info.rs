#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncodeInfo {
    pub sample_rate: u32,
    pub channels: u32,
    pub samples_per_channel: u32,
    pub input_samples: u32,
    pub max_output_bytes: u32,
}

impl EncodeInfo {
    pub(super) fn from_open_info(
        sample_rate: u32,
        channels: u32,
        open_info: &crate::frame::OpenInfo,
    ) -> Self {
        Self {
            sample_rate,
            channels,
            samples_per_channel: open_info.input_samples / channels,
            input_samples: open_info.input_samples,
            max_output_bytes: open_info.max_output_bytes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EncodeInfo;

    #[test]
    fn encode_info_is_copyable_stream_shape() {
        let info = EncodeInfo {
            sample_rate: 44_100,
            channels: 2,
            samples_per_channel: 1_024,
            input_samples: 2_048,
            max_output_bytes: 8_192,
        };
        let copy = info;

        assert_eq!(copy.sample_rate, 44_100);
        assert_eq!(copy.channels, 2);
        assert_eq!(copy.samples_per_channel, 1_024);
        assert_eq!(copy.input_samples, 2_048);
        assert_eq!(copy.max_output_bytes, 8_192);
    }

    #[test]
    fn builds_stream_shape_from_low_level_open_info() {
        let open_info = crate::frame::OpenInfo {
            input_samples: 2_048,
            max_output_bytes: 8_192,
        };

        let info = EncodeInfo::from_open_info(44_100, 2, &open_info);

        assert_eq!(info.sample_rate, 44_100);
        assert_eq!(info.channels, 2);
        assert_eq!(info.samples_per_channel, 1_024);
        assert_eq!(info.input_samples, 2_048);
        assert_eq!(info.max_output_bytes, 8_192);
    }
}
