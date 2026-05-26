use super::EncoderError;

pub(super) trait PcmSample: Copy {
    fn to_faac_word(self) -> i32;
}

impl PcmSample for f32 {
    fn to_faac_word(self) -> i32 {
        self.to_bits() as i32
    }
}

impl PcmSample for i16 {
    fn to_faac_word(self) -> i32 {
        (self as f32).to_bits() as i32
    }
}

pub(super) struct SampleWords {
    words: Vec<i32>,
}

impl SampleWords {
    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self {
            words: Vec::with_capacity(capacity),
        }
    }

    pub(super) fn clear(&mut self) {
        self.words.clear();
    }

    pub(super) fn fill_from_interleaved<T: PcmSample>(&mut self, input: &[T]) {
        self.words.clear();
        self.words
            .extend(input.iter().copied().map(PcmSample::to_faac_word));
    }

    pub(super) fn fill_validated<T: PcmSample>(
        &mut self,
        input: &[T],
        channels: u32,
    ) -> Result<(), EncoderError> {
        validate_interleaved_len(input.len(), channels)?;
        self.fill_from_interleaved(input);
        Ok(())
    }

    pub(super) fn as_slice(&self) -> &[i32] {
        &self.words
    }
}

pub(super) fn validate_interleaved_len(samples: usize, channels: u32) -> Result<(), EncoderError> {
    if samples == 0 {
        return Err(EncoderError::EmptyInput);
    }
    if samples % channels as usize == 0 {
        return Ok(());
    }

    Err(EncoderError::InvalidInputLength { samples, channels })
}

#[cfg(test)]
mod tests {
    use super::{PcmSample, SampleWords};
    use crate::encoder::EncoderError;

    #[test]
    fn i16_samples_encode_as_faac_float_words() {
        for sample in [i16::MIN, -1, 0, 1, i16::MAX] {
            assert_eq!(sample.to_faac_word(), (sample as f32).to_bits() as i32);
        }
    }

    #[test]
    fn f32_samples_encode_as_faac_float_words() {
        for sample in [-32_768.0f32, -1.0, 0.0, 1.0, 32_767.0] {
            assert_eq!(sample.to_faac_word(), sample.to_bits() as i32);
        }
    }

    #[test]
    fn validates_interleaved_sample_lengths() {
        assert_eq!(super::validate_interleaved_len(4, 2), Ok(()));
        assert_eq!(
            super::validate_interleaved_len(0, 2),
            Err(EncoderError::EmptyInput)
        );
        assert_eq!(
            super::validate_interleaved_len(3, 2),
            Err(EncoderError::InvalidInputLength {
                samples: 3,
                channels: 2,
            })
        );
    }

    #[test]
    fn sample_words_reuses_buffer_for_faac_words() {
        let mut words = SampleWords::with_capacity(4);

        words.fill_from_interleaved(&[1i16, -1, 0, i16::MAX]);
        assert_eq!(words.as_slice().len(), 4);
        assert_eq!(words.as_slice()[0], (1f32).to_bits() as i32);

        words.clear();
        assert!(words.as_slice().is_empty());

        words.fill_from_interleaved(&[1.0f32, -1.0]);
        assert_eq!(
            words.as_slice(),
            &[1.0f32.to_bits() as i32, (-1.0f32).to_bits() as i32]
        );
    }

    #[test]
    fn sample_words_validates_input_shape_before_replacing_buffer() {
        let mut words = SampleWords::with_capacity(4);

        words.fill_from_interleaved(&[1i16, -1]);
        let err = words.fill_validated(&[2i16], 2).unwrap_err();

        assert_eq!(
            err,
            EncoderError::InvalidInputLength {
                samples: 1,
                channels: 2,
            }
        );
        assert_eq!(
            words.as_slice(),
            &[1.0f32.to_bits() as i32, (-1.0f32).to_bits() as i32]
        );

        words.fill_validated(&[2i16, -2], 2).expect("valid input");
        assert_eq!(
            words.as_slice(),
            &[2.0f32.to_bits() as i32, (-2.0f32).to_bits() as i32]
        );
    }
}
