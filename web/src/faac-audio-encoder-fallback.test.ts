import { afterEach, describe, expect, test, vi } from 'vitest';
import {
  installFaacAudioEncoderFallback,
  type FaacEncoderFactory,
  type WasmFaacEncoder,
} from '../../crates/opencat-web/web/src/media/faac-audio-encoder';

class StubEncodedAudioChunk {
  readonly type: EncodedAudioChunkType;
  readonly timestamp: number;
  readonly duration: number | null;
  readonly byteLength: number;
  private data: Uint8Array;

  constructor(init: EncodedAudioChunkInit) {
    this.type = init.type;
    this.timestamp = init.timestamp;
    this.duration = init.duration ?? null;
    this.data = new Uint8Array(init.data as ArrayBuffer);
    this.byteLength = this.data.byteLength;
  }

  copyTo(destination: AllowSharedBufferSource): void {
    new Uint8Array(destination as ArrayBuffer).set(this.data);
  }
}

class StubAudioData {
  readonly format: AudioSampleFormat = 'f32';
  readonly numberOfChannels: number;
  readonly numberOfFrames: number;
  readonly sampleRate: number;
  readonly timestamp: number;
  readonly duration: number;
  closed = false;
  private samples: Float32Array;

  constructor(init: AudioDataInit) {
    this.numberOfChannels = init.numberOfChannels;
    this.numberOfFrames = init.numberOfFrames;
    this.sampleRate = init.sampleRate;
    this.timestamp = init.timestamp;
    this.duration = Math.round((init.numberOfFrames / init.sampleRate) * 1_000_000);
    this.samples = new Float32Array(init.data as ArrayBuffer);
  }

  allocationSize(): number {
    return this.samples.byteLength;
  }

  copyTo(destination: AllowSharedBufferSource): void {
    new Float32Array(destination as ArrayBuffer).set(this.samples);
  }

  clone(): AudioData {
    return this as unknown as AudioData;
  }

  close(): void {
    this.closed = true;
  }
}

function createFactory(): {
  factory: FaacEncoderFactory;
  encoders: WasmFaacEncoder[];
} {
  const encoders: WasmFaacEncoder[] = [];
  const factory: FaacEncoderFactory = vi.fn((config) => {
    let flushCount = 0;
    const encoder: WasmFaacEncoder = {
      inputSamples: config.numberOfChannels * 1024,
      audioSpecificConfig: new Uint8Array([0x11, 0x90]),
      encodeF32Interleaved: vi.fn(() => [new Uint8Array([0xaa, 0xbb])]),
      flush: vi.fn(() => (flushCount++ === 0 ? [new Uint8Array([0xcc])] : [])),
      free: vi.fn(),
    };
    encoders.push(encoder);
    return encoder;
  });
  return { factory, encoders };
}

describe('faac AudioEncoder fallback', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  test('installs a scoped AudioEncoder shim that reports AAC support', async () => {
    const nativeAudioEncoder = vi.fn() as unknown as typeof AudioEncoder;
    Object.assign(nativeAudioEncoder, {
      isConfigSupported: vi.fn(async () => ({ supported: false })),
    });
    vi.stubGlobal('AudioEncoder', nativeAudioEncoder);
    vi.stubGlobal('EncodedAudioChunk', StubEncodedAudioChunk);

    const { factory } = createFactory();
    const restore = installFaacAudioEncoderFallback({ createEncoder: factory });

    await expect(AudioEncoder.isConfigSupported({
      codec: 'mp4a.40.2',
      sampleRate: 48_000,
      numberOfChannels: 2,
    })).resolves.toMatchObject({ supported: true });

    restore();
    expect(globalThis.AudioEncoder).toBe(nativeAudioEncoder);
  });

  test('encodes f32 AudioData through wasm faac and emits WebCodecs chunks', async () => {
    vi.stubGlobal('EncodedAudioChunk', StubEncodedAudioChunk);
    const { factory, encoders } = createFactory();
    const restore = installFaacAudioEncoderFallback({ createEncoder: factory });
    const chunks: EncodedAudioChunk[] = [];
    const metas: (EncodedAudioChunkMetadata | undefined)[] = [];
    const encoder = new AudioEncoder({
      output: (chunk, meta) => {
        chunks.push(chunk);
        metas.push(meta);
      },
      error: (err) => {
        throw err;
      },
    });

    encoder.configure({
      codec: 'mp4a.40.2',
      sampleRate: 48_000,
      numberOfChannels: 2,
      bitrate: 128_000,
    });
    const pcm = new Float32Array(2048);
    pcm[0] = 0.25;
    const audioData = new StubAudioData({
      data: pcm.buffer,
      format: 'f32',
      numberOfChannels: 2,
      numberOfFrames: 1024,
      sampleRate: 48_000,
      timestamp: 10_000,
    });

    encoder.encode(audioData as unknown as AudioData);
    await encoder.flush();

    expect(factory).toHaveBeenCalledOnce();
    const expected = new Float32Array(pcm);
    expected[0] = 8192;
    expect(encoders[0].encodeF32Interleaved).toHaveBeenCalledWith(expected);
    expect(audioData.closed).toBe(false);
    expect(chunks).toHaveLength(2);
    expect(chunks[0].timestamp).toBe(10_000);
    expect(chunks[0].duration).toBe(21_333);
    expect(metas[0]?.decoderConfig).toMatchObject({
      codec: 'mp4a.40.2',
      sampleRate: 48_000,
      numberOfChannels: 2,
    });
    expect(new Uint8Array(metas[0]!.decoderConfig!.description as ArrayBuffer)).toEqual(
      new Uint8Array([0x11, 0x90]),
    );
    expect(metas[1]).toBeUndefined();

    encoder.close();
    expect(encoders[0].free).toHaveBeenCalledOnce();
    restore();
  });

  test('scales normalized WebCodecs f32 PCM to faac full-scale samples', async () => {
    vi.stubGlobal('EncodedAudioChunk', StubEncodedAudioChunk);
    const { factory, encoders } = createFactory();
    const restore = installFaacAudioEncoderFallback({ createEncoder: factory });
    const encoder = new AudioEncoder({
      output: () => {},
      error: (err) => { throw err; },
    });

    encoder.configure({
      codec: 'mp4a.40.2',
      sampleRate: 48_000,
      numberOfChannels: 2,
    });

    encoder.encode(new StubAudioData({
      data: new Float32Array([-1, -0.5, 0, 0.5, 1, 2]).buffer,
      format: 'f32',
      numberOfChannels: 2,
      numberOfFrames: 3,
      sampleRate: 48_000,
      timestamp: 0,
    }) as unknown as AudioData);

    expect(encoders[0].encodeF32Interleaved).toHaveBeenCalledWith(
      new Float32Array([-32768, -16384, 0, 16384, 32767, 32767]),
    );
    restore();
  });

  test('preserves queued timestamps when faac emits delayed frames during flush', async () => {
    vi.stubGlobal('EncodedAudioChunk', StubEncodedAudioChunk);
    const factory: FaacEncoderFactory = () => ({
      inputSamples: 2048,
      audioSpecificConfig: new Uint8Array([0x11, 0x90]),
      encodeF32Interleaved: vi.fn(() => []),
      flush: vi.fn(() => [new Uint8Array([0x01]), new Uint8Array([0x02])]),
      free: vi.fn(),
    });
    const restore = installFaacAudioEncoderFallback({ createEncoder: factory });
    const chunks: EncodedAudioChunk[] = [];
    const encoder = new AudioEncoder({
      output: (chunk) => chunks.push(chunk),
      error: (err) => { throw err; },
    });

    encoder.configure({
      codec: 'mp4a.40.2',
      sampleRate: 48_000,
      numberOfChannels: 2,
    });
    encoder.encode(new StubAudioData({
      data: new Float32Array(2048).buffer,
      format: 'f32',
      numberOfChannels: 2,
      numberOfFrames: 1024,
      sampleRate: 48_000,
      timestamp: 10_000,
    }) as unknown as AudioData);
    encoder.encode(new StubAudioData({
      data: new Float32Array(2048).buffer,
      format: 'f32',
      numberOfChannels: 2,
      numberOfFrames: 1024,
      sampleRate: 48_000,
      timestamp: 31_333,
    }) as unknown as AudioData);

    await encoder.flush();

    expect(chunks.map((chunk) => chunk.timestamp)).toEqual([10_000, 31_333]);
    restore();
  });
});
