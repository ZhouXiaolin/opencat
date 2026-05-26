use crate::frame;

use super::{EncoderConfig, EncoderError};

pub(super) struct EncoderCore {
    inner: frame::FrameEncoder,
    output_buffer: Vec<u8>,
}

impl EncoderCore {
    pub(super) fn open(
        sample_rate: u32,
        channels: u32,
        config: &EncoderConfig,
    ) -> Result<(Self, frame::OpenInfo), EncoderError> {
        let (mut inner, open_info) =
            frame::FrameEncoder::open(sample_rate, channels).ok_or(EncoderError::OpenFailed)?;

        let mut c_config = inner.config.clone();
        config.apply_to_c_config(&mut c_config);

        if !inner.set_configuration(c_config) {
            return Err(EncoderError::ConfigureFailed);
        }

        let output_buffer = vec![0; open_info.max_output_bytes as usize];
        Ok((
            Self {
                inner,
                output_buffer,
            },
            open_info,
        ))
    }

    pub(super) fn encode_words(
        &mut self,
        sample_words: &[i32],
        output: &mut Vec<u8>,
    ) -> Result<usize, EncoderError> {
        let written = self
            .inner
            .encode(sample_words, sample_words.len(), &mut self.output_buffer);
        self.append_output(written, output)
    }

    pub(super) fn flush(&mut self, output: &mut Vec<u8>) -> Result<usize, EncoderError> {
        let written = self.inner.encode(&[], 0, &mut self.output_buffer);
        self.append_output(written, output)
    }

    pub(super) fn finish(mut self, output: &mut Vec<u8>) -> Result<usize, EncoderError> {
        let start_len = output.len();
        while self.inner.flush_frame <= 4 {
            let _ = self.flush(output)?;
        }
        Ok(output.len() - start_len)
    }

    fn append_output(&mut self, written: i32, output: &mut Vec<u8>) -> Result<usize, EncoderError> {
        if written < 0 {
            return Err(EncoderError::EncodeFailed);
        }

        let written = written as usize;
        output.extend_from_slice(&self.output_buffer[..written]);
        Ok(written)
    }
}

#[cfg(test)]
mod tests {
    use super::EncoderCore;
    use crate::encoder::EncoderConfig;

    #[test]
    fn core_open_reports_low_level_stream_shape() {
        let config = EncoderConfig::default();

        let (_core, info) = EncoderCore::open(44_100, 1, &config).expect("open core");

        assert_eq!(info.input_samples, 1_024);
        assert!(info.max_output_bytes >= 8_192);
    }
}
