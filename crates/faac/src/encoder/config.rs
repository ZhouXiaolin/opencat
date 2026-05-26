use crate::codec::{Configuration, InputFormat, StreamFormat};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    Raw,
    Adts,
}

impl OutputFormat {
    fn to_stream_format(self) -> StreamFormat {
        match self {
            Self::Raw => StreamFormat::Raw,
            Self::Adts => StreamFormat::Adts,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncoderConfig {
    pub bit_rate: u64,
    pub bandwidth: Option<u32>,
    pub quality: Option<u64>,
    pub output_format: OutputFormat,
    pub use_lfe: bool,
    pub use_tns: bool,
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self {
            bit_rate: 64_000,
            bandwidth: None,
            quality: None,
            output_format: OutputFormat::Adts,
            use_lfe: true,
            use_tns: false,
        }
    }
}

impl EncoderConfig {
    pub fn bit_rate(mut self, bit_rate: u64) -> Self {
        self.bit_rate = bit_rate;
        self
    }

    pub fn quality(mut self, quality: u64) -> Self {
        self.quality = Some(quality);
        self
    }

    pub fn bandwidth(mut self, bandwidth: u32) -> Self {
        self.bandwidth = Some(bandwidth);
        self
    }

    pub fn output_format(mut self, format: OutputFormat) -> Self {
        self.output_format = format;
        self
    }

    pub fn use_lfe(mut self, use_lfe: bool) -> Self {
        self.use_lfe = use_lfe;
        self
    }

    pub fn use_tns(mut self, use_tns: bool) -> Self {
        self.use_tns = use_tns;
        self
    }

    pub(super) fn apply_to_c_config(&self, cfg: &mut Configuration) {
        cfg.input_format = InputFormat::F32;
        cfg.output_format = self.output_format.to_stream_format();
        cfg.use_lfe = self.use_lfe;
        cfg.use_tns = self.use_tns;
        cfg.band_width = self.bandwidth.unwrap_or(0);

        if let Some(quality) = self.quality {
            cfg.quantqual = quality;
            cfg.bit_rate = 0;
        } else {
            cfg.quantqual = 0;
            cfg.bit_rate = self.bit_rate;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{EncoderConfig, OutputFormat};
    use crate::codec::{Configuration, InputFormat, StreamFormat};

    #[test]
    fn high_level_config_applies_to_c_configuration() {
        let config = EncoderConfig {
            bit_rate: 128_000,
            bandwidth: Some(12_345),
            quality: Some(90),
            output_format: OutputFormat::Raw,
            use_lfe: false,
            use_tns: true,
        };
        let mut c_config = Configuration::default();

        config.apply_to_c_config(&mut c_config);

        assert_eq!(c_config.input_format, InputFormat::F32);
        assert_eq!(c_config.output_format, StreamFormat::Raw);
        assert!(!c_config.use_lfe);
        assert!(c_config.use_tns);
        assert_eq!(c_config.band_width, 12_345);
        assert_eq!(c_config.quantqual, 90);
        assert_eq!(c_config.bit_rate, 0);
    }
}
