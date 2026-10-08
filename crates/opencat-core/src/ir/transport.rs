// ---------------------------------------------------------------------------
// Compressed OCIR transport variant (issue #46, part 1).
//
// OCIR itself is a dense, offset-agnostic op stream (see `draw_encoding`). When
// an envelope must cross a bandwidth-sensitive hop (server → browser), the raw
// bytes can be wrapped in a small container that carries a raw-deflate payload:
//
//   magic "OCZ1" | version u32 | codec u8 | reserved[3] | uncompressed_len u32
//   | deflate stream
//
// The codec is **raw DEFLATE** (RFC 1951), not zlib/gzip — the browser side
// decodes the payload with `DecompressionStream('deflate-raw')`, which has no
// wrapper to strip. Core is the producer (compression belongs with the encoder,
// not in the wasm frame loop); the compressed form is opt-in per hop and never
// replaces the in-memory OCIR the wasm bridge hands to JS.
//
// The envelope is still a pure function of the frame: compression is
// deterministic for a fixed input and level, so encode-then-compress is stable.
// ---------------------------------------------------------------------------

use super::draw_encoding::{IR_MAGIC};
use miniz_oxide::deflate::compress_to_vec;
use miniz_oxide::inflate::decompress_to_vec;

/// Magic bytes for the compressed OCIR container.
pub const IR_COMPRESSED_MAGIC: [u8; 4] = *b"OCZ1";

/// Container format version.
pub const IR_COMPRESSED_VERSION: u32 = 1;

/// Codec id: raw DEFLATE (RFC 1951). Decoded in the browser by
/// `DecompressionStream('deflate-raw')`.
pub const CODEC_DEFLATE_RAW: u8 = 1;

/// Fixed container header length: magic(4) + version(4) + codec(1) +
/// reserved(3) + uncompressed_len(4).
pub const CONTAINER_HEADER_LEN: usize = 16;

/// Container / codec failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// Input is not a compressed OCIR container (bad magic) or too short.
    NotACompressedEnvelope,
    /// Unsupported container version.
    UnsupportedVersion(u32),
    /// Unsupported codec id.
    UnsupportedCodec(u8),
    /// Deflate stream is malformed, or the inflated length disagrees with the
    /// header's `uncompressed_len`.
    MalformedPayload,
    /// The inflated bytes are not a valid OCIR envelope (bad magic).
    NotAnOcirEnvelope,
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotACompressedEnvelope => write!(f, "not a compressed OCIR container"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported OCIR container version {v}"),
            Self::UnsupportedCodec(c) => write!(f, "unsupported OCIR codec {c}"),
            Self::MalformedPayload => write!(f, "malformed deflate payload"),
            Self::NotAnOcirEnvelope => write!(f, "inflated bytes are not an OCIR envelope"),
        }
    }
}

impl std::error::Error for TransportError {}

/// Wrap a raw OCIR envelope in a compressed container (raw DEFLATE).
///
/// `level` is a miniz level (0 = store, 1 = fastest, …, 10 = best). For OCIR the
/// payload is highly repetitive (fixed-length ops, repeated float coordinates),
/// so even low levels shrink it by an order of magnitude.
pub fn compress_ir_envelope(ocir: &[u8], level: u8) -> Vec<u8> {
    let deflated = compress_to_vec(ocir, level);
    let mut out = Vec::with_capacity(CONTAINER_HEADER_LEN + deflated.len());
    out.extend_from_slice(&IR_COMPRESSED_MAGIC);
    out.extend_from_slice(&IR_COMPRESSED_VERSION.to_le_bytes());
    out.push(CODEC_DEFLATE_RAW);
    out.extend_from_slice(&[0u8; 3]); // reserved
    out.extend_from_slice(&(ocir.len() as u32).to_le_bytes());
    out.extend_from_slice(&deflated);
    out
}

/// True if `bytes` starts with the compressed-container magic.
#[inline]
pub fn is_compressed_envelope(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes[0..4] == IR_COMPRESSED_MAGIC
}

/// Unwrap a compressed container back to the raw OCIR envelope.
pub fn decompress_ir_envelope(bytes: &[u8]) -> Result<Vec<u8>, TransportError> {
    if bytes.len() < CONTAINER_HEADER_LEN || !is_compressed_envelope(bytes) {
        return Err(TransportError::NotACompressedEnvelope);
    }
    let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    if version != IR_COMPRESSED_VERSION {
        return Err(TransportError::UnsupportedVersion(version));
    }
    let codec = bytes[8];
    if codec != CODEC_DEFLATE_RAW {
        return Err(TransportError::UnsupportedCodec(codec));
    }
    let expected_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let ocir = decompress_to_vec(&bytes[CONTAINER_HEADER_LEN..])
        .map_err(|_| TransportError::MalformedPayload)?;
    if ocir.len() != expected_len {
        return Err(TransportError::MalformedPayload);
    }
    if ocir.len() < 4 || ocir[0..4] != IR_MAGIC {
        return Err(TransportError::NotAnOcirEnvelope);
    }
    Ok(ocir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::draw_encoding::encode_ir_envelope;
    use crate::ir::draw_frame::DrawFrameScratch;

    fn sample_envelope() -> Vec<u8> {
        // A frame with enough repeated structure that deflate has something to
        // bite on (many identical Translate/Save ops).
        let mut frame = crate::ir::draw_frame::DrawOpFrame::default();
        for i in 0..200u32 {
            frame.ops.push(crate::ir::draw_op::DrawOp::Save);
            frame.ops.push(crate::ir::draw_op::DrawOp::Translate {
                x: (i % 40) as f32,
                y: (i % 7) as f32,
            });
            frame.ops.push(crate::ir::draw_op::DrawOp::Restore);
        }
        let render = crate::ir::RenderFrame {
            draw: frame,
            media: Default::default(),
        };
        let mut scratch = DrawFrameScratch::default();
        encode_ir_envelope(&render, &mut scratch).unwrap()
    }

    #[test]
    fn compress_then_decompress_round_trips() {
        let ocir = sample_envelope();
        let packed = compress_ir_envelope(&ocir, 6);
        assert!(is_compressed_envelope(&packed));
        let restored = decompress_ir_envelope(&packed).unwrap();
        assert_eq!(restored, ocir);
    }

    #[test]
    fn compressed_container_is_smaller_than_raw() {
        let ocir = sample_envelope();
        let packed = compress_ir_envelope(&ocir, 6);
        assert!(
            packed.len() < ocir.len(),
            "compressed {} not smaller than raw {}",
            packed.len(),
            ocir.len()
        );
    }

    #[test]
    fn compression_is_deterministic() {
        let ocir = sample_envelope();
        assert_eq!(
            compress_ir_envelope(&ocir, 6),
            compress_ir_envelope(&ocir, 6),
            "same input + level must produce byte-identical container"
        );
    }

    #[test]
    fn header_carries_length_and_codec() {
        let ocir = sample_envelope();
        let packed = compress_ir_envelope(&ocir, 6);
        assert_eq!(&packed[0..4], b"OCZ1");
        assert_eq!(
            u32::from_le_bytes(packed[4..8].try_into().unwrap()),
            IR_COMPRESSED_VERSION
        );
        assert_eq!(packed[8], CODEC_DEFLATE_RAW);
        assert_eq!(
            u32::from_le_bytes(packed[12..16].try_into().unwrap()) as usize,
            ocir.len()
        );
    }

    #[test]
    fn rejects_bad_magic_and_truncation() {
        assert_eq!(
            decompress_ir_envelope(b"OCIR\x01\x00\x00\x00"),
            Err(TransportError::NotACompressedEnvelope)
        );
        let mut packed = compress_ir_envelope(&sample_envelope(), 6);
        packed.truncate(CONTAINER_HEADER_LEN - 1);
        assert_eq!(
            decompress_ir_envelope(&packed),
            Err(TransportError::NotACompressedEnvelope)
        );
    }

    #[test]
    fn rejects_unsupported_version_and_codec() {
        let ocir = sample_envelope();
        let mut packed = compress_ir_envelope(&ocir, 6);
        packed[4] = 99; // version byte
        assert_eq!(
            decompress_ir_envelope(&packed),
            Err(TransportError::UnsupportedVersion(99))
        );
        let mut packed = compress_ir_envelope(&ocir, 6);
        packed[8] = 7; // codec byte
        assert_eq!(
            decompress_ir_envelope(&packed),
            Err(TransportError::UnsupportedCodec(7))
        );
    }

    #[test]
    fn rejects_payload_that_is_not_ocir() {
        // Valid deflate of garbage (not OCIR) must be refused.
        let packed = compress_ir_envelope(b"not-an-ocir-envelope!!", 6);
        assert_eq!(
            decompress_ir_envelope(&packed),
            Err(TransportError::NotAnOcirEnvelope)
        );
    }
}
