use std::fmt;

mod builder;
mod config;
mod core;
mod error;
mod info;
mod samples;

pub use builder::EncoderBuilder;
pub use config::{EncoderConfig, OutputFormat};
use core::EncoderCore;
pub use error::EncoderError;
pub use info::EncodeInfo;
use samples::{PcmSample, SampleWords};

pub struct Encoder {
    core: EncoderCore,
    info: EncodeInfo,
    sample_words: SampleWords,
}

impl Encoder {
    pub fn builder(sample_rate: u32, channels: u32) -> EncoderBuilder {
        EncoderBuilder::new(sample_rate, channels)
    }

    pub fn info(&self) -> EncodeInfo {
        self.info
    }

    pub fn encode_f32_interleaved(
        &mut self,
        input: &[f32],
        output: &mut Vec<u8>,
    ) -> Result<usize, EncoderError> {
        self.encode_interleaved_samples(input, output)
    }

    pub fn encode_i16_interleaved(
        &mut self,
        input: &[i16],
        output: &mut Vec<u8>,
    ) -> Result<usize, EncoderError> {
        self.encode_interleaved_samples(input, output)
    }

    fn encode_interleaved_samples<T: PcmSample>(
        &mut self,
        input: &[T],
        output: &mut Vec<u8>,
    ) -> Result<usize, EncoderError> {
        self.sample_words
            .fill_validated(input, self.info.channels)?;
        self.encode_words(output)
    }

    pub fn flush(&mut self, output: &mut Vec<u8>) -> Result<usize, EncoderError> {
        self.sample_words.clear();
        self.core.flush(output)
    }

    pub fn finish(mut self, output: &mut Vec<u8>) -> Result<usize, EncoderError> {
        self.sample_words.clear();
        self.core.finish(output)
    }

    fn encode_words(&mut self, output: &mut Vec<u8>) -> Result<usize, EncoderError> {
        self.core.encode_words(self.sample_words.as_slice(), output)
    }
}

impl fmt::Debug for Encoder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Encoder")
            .field("info", &self.info)
            .finish_non_exhaustive()
    }
}
