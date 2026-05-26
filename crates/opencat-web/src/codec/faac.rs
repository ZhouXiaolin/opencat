use wasm_bindgen::prelude::*;

use faac_rs::{Encoder, EncoderConfig, OutputFormat};

#[wasm_bindgen]
pub struct WebFaacEncoder {
    inner: Option<Encoder>,
    input_samples: u32,
    sample_rate: u32,
    channels: u32,
    flush_calls: u32,
    output: Vec<u8>,
}

#[wasm_bindgen]
impl WebFaacEncoder {
    #[wasm_bindgen(constructor)]
    pub fn new(sample_rate: u32, channels: u32, bit_rate: u32) -> Result<WebFaacEncoder, JsValue> {
        let config = EncoderConfig::default()
            .bit_rate(bit_rate as u64)
            .output_format(OutputFormat::Raw);
        let encoder = Encoder::builder(sample_rate, channels)
            .config(config)
            .open()
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let info = encoder.info();
        Ok(Self {
            inner: Some(encoder),
            input_samples: info.input_samples,
            sample_rate,
            channels,
            flush_calls: 0,
            output: Vec::with_capacity(info.max_output_bytes as usize),
        })
    }

    #[wasm_bindgen(getter)]
    pub fn input_samples(&self) -> u32 {
        self.input_samples
    }

    #[wasm_bindgen(getter)]
    pub fn audio_specific_config(&self) -> Vec<u8> {
        audio_specific_config(self.sample_rate, self.channels)
    }

    pub fn encode_f32_interleaved(&mut self, samples: &[f32]) -> Result<js_sys::Array, JsValue> {
        let encoder = self
            .inner
            .as_mut()
            .ok_or_else(|| JsValue::from_str("faac encoder is closed"))?;
        self.output.clear();
        encoder
            .encode_f32_interleaved(samples, &mut self.output)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(bytes_to_chunks(&self.output))
    }

    pub fn flush(&mut self) -> Result<js_sys::Array, JsValue> {
        let Some(encoder) = self.inner.as_mut() else {
            return Ok(js_sys::Array::new());
        };
        let chunks = js_sys::Array::new();
        while self.flush_calls <= 4 {
            self.flush_calls += 1;
            self.output.clear();
            let written = encoder
                .flush(&mut self.output)
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            if written > 0 {
                chunks.push(&js_sys::Uint8Array::from(self.output.as_slice()));
            }
        }
        self.inner = None;
        Ok(chunks)
    }
}

fn bytes_to_chunks(bytes: &[u8]) -> js_sys::Array {
    let chunks = js_sys::Array::new();
    if bytes.is_empty() {
        return chunks;
    }
    chunks.push(&js_sys::Uint8Array::from(bytes));
    chunks
}

fn audio_specific_config(sample_rate: u32, channels: u32) -> Vec<u8> {
    let object_type = 2u8; // AAC-LC
    let sample_rate_index = aac_sample_rate_index(sample_rate);
    let channel_config = channels.min(7) as u8;
    let first = (object_type << 3) | (sample_rate_index >> 1);
    let second = ((sample_rate_index & 1) << 7) | (channel_config << 3);
    vec![first, second]
}

fn aac_sample_rate_index(sample_rate: u32) -> u8 {
    match sample_rate {
        96_000 => 0,
        88_200 => 1,
        64_000 => 2,
        48_000 => 3,
        44_100 => 4,
        32_000 => 5,
        24_000 => 6,
        22_050 => 7,
        16_000 => 8,
        12_000 => 9,
        11_025 => 10,
        8_000 => 11,
        _ if sample_rate >= 92_017 => 0,
        _ if sample_rate >= 75_132 => 1,
        _ if sample_rate >= 55_426 => 2,
        _ if sample_rate >= 46_009 => 3,
        _ if sample_rate >= 37_566 => 4,
        _ if sample_rate >= 27_713 => 5,
        _ if sample_rate >= 23_004 => 6,
        _ if sample_rate >= 18_783 => 7,
        _ if sample_rate >= 13_856 => 8,
        _ if sample_rate >= 11_502 => 9,
        _ if sample_rate >= 9_391 => 10,
        _ => 11,
    }
}

#[cfg(test)]
mod tests {
    use super::audio_specific_config;

    #[test]
    fn builds_aac_lc_audio_specific_config() {
        assert_eq!(audio_specific_config(48_000, 2), vec![0x11, 0x90]);
        assert_eq!(audio_specific_config(44_100, 1), vec![0x12, 0x08]);
    }
}
