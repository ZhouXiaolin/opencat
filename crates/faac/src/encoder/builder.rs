use crate::codec::MAX_CHANNELS;

use super::{
    EncodeInfo, Encoder, EncoderConfig, EncoderCore, EncoderError, OutputFormat, SampleWords,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncoderBuilder {
    sample_rate: u32,
    channels: u32,
    config: EncoderConfig,
}

impl EncoderBuilder {
    pub fn new(sample_rate: u32, channels: u32) -> Self {
        Self {
            sample_rate,
            channels,
            config: EncoderConfig::default(),
        }
    }

    pub fn config(mut self, config: EncoderConfig) -> Self {
        self.config = config;
        self
    }

    pub fn bit_rate(mut self, bit_rate: u64) -> Self {
        self.config.bit_rate = bit_rate;
        self.config.quality = None;
        self
    }

    pub fn bandwidth(mut self, bandwidth: u32) -> Self {
        self.config.bandwidth = Some(bandwidth);
        self
    }

    pub fn quality(mut self, quality: u64) -> Self {
        self.config.quality = Some(quality);
        self
    }

    pub fn output_format(mut self, output_format: OutputFormat) -> Self {
        self.config.output_format = output_format;
        self
    }

    pub fn use_lfe(mut self, use_lfe: bool) -> Self {
        self.config.use_lfe = use_lfe;
        self
    }

    pub fn use_tns(mut self, use_tns: bool) -> Self {
        self.config.use_tns = use_tns;
        self
    }

    pub fn open(self) -> Result<Encoder, EncoderError> {
        if self.channels == 0 || self.channels as usize > MAX_CHANNELS {
            return Err(EncoderError::InvalidChannelCount(self.channels));
        }

        let (core, open_info) = EncoderCore::open(self.sample_rate, self.channels, &self.config)?;

        let info = EncodeInfo::from_open_info(self.sample_rate, self.channels, &open_info);

        Ok(Encoder {
            core,
            info,
            sample_words: SampleWords::with_capacity(open_info.input_samples as usize),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::EncoderBuilder;
    use crate::encoder::EncoderError;

    #[test]
    fn rejects_zero_channels() {
        let err = EncoderBuilder::new(44_100, 0).open().unwrap_err();

        assert_eq!(err, EncoderError::InvalidChannelCount(0));
    }
}
