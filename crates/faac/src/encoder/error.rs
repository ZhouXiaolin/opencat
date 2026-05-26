use std::error::Error;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EncoderError {
    InvalidChannelCount(u32),
    EmptyInput,
    InvalidInputLength { samples: usize, channels: u32 },
    OpenFailed,
    ConfigureFailed,
    EncodeFailed,
}

impl fmt::Display for EncoderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidChannelCount(channels) => {
                write!(f, "invalid channel count: {channels}")
            }
            Self::EmptyInput => {
                f.write_str("input samples are empty; call flush to finish the stream")
            }
            Self::InvalidInputLength { samples, channels } => write!(
                f,
                "interleaved input length {samples} is not divisible by {channels} channels"
            ),
            Self::OpenFailed => f.write_str("failed to open encoder"),
            Self::ConfigureFailed => f.write_str("failed to configure encoder"),
            Self::EncodeFailed => f.write_str("encoder returned an error"),
        }
    }
}

impl Error for EncoderError {}

#[cfg(test)]
mod tests {
    use super::EncoderError;

    #[test]
    fn display_messages_are_stable() {
        assert_eq!(
            EncoderError::InvalidInputLength {
                samples: 3,
                channels: 2,
            }
            .to_string(),
            "interleaved input length 3 is not divisible by 2 channels"
        );
        assert_eq!(
            EncoderError::EmptyInput.to_string(),
            "input samples are empty; call flush to finish the stream"
        );
    }
}
