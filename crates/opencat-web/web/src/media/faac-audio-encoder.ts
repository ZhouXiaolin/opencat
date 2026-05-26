export interface WasmFaacEncoder {
  readonly inputSamples: number;
  readonly audioSpecificConfig: Uint8Array;
  encodeF32Interleaved(samples: Float32Array): Uint8Array[];
  flush(): Uint8Array[];
  free(): void;
}

export type FaacEncoderFactory = (config: AudioEncoderConfig) => WasmFaacEncoder;

type InstallOptions = {
  createEncoder: FaacEncoderFactory;
};

type AudioEncoderConstructor = typeof AudioEncoder;

const AAC_CODEC = 'mp4a.40.2';

export function installFaacAudioEncoderFallback(options: InstallOptions): () => void {
  if (!canUseFaacAudioEncoderFallback()) {
    throw new Error('EncodedAudioChunk is not available for faac AudioEncoder fallback');
  }
  const nativeAudioEncoder = globalThis.AudioEncoder;
  const fallback = createFallbackAudioEncoder(nativeAudioEncoder, options.createEncoder);

  globalThis.AudioEncoder = fallback as unknown as AudioEncoderConstructor;

  return () => {
    globalThis.AudioEncoder = nativeAudioEncoder;
  };
}

export function canUseFaacAudioEncoderFallback(): boolean {
  return typeof EncodedAudioChunk !== 'undefined';
}

function createFallbackAudioEncoder(
  nativeAudioEncoder: AudioEncoderConstructor | undefined,
  createEncoder: FaacEncoderFactory,
): AudioEncoderConstructor {
  return class FaacFallbackAudioEncoder extends EventTarget implements AudioEncoder {
    ondequeue: ((this: AudioEncoder, ev: Event) => unknown) | null = null;
    private impl: AudioEncoder | null = null;
    private faac: WasmFaacEncoder | null = null;
    private config: AudioEncoderConfig | null = null;
    private callbacks: AudioEncoderInit;
    private firstOutput = true;
    private nextTimestamp = 0;
    private pendingTimestamps: number[] = [];
    private closed = false;

    static async isConfigSupported(config: AudioEncoderConfig): Promise<AudioEncoderSupport> {
      if (isAacConfig(config)) {
        return { supported: true, config };
      }
      if (!nativeAudioEncoder?.isConfigSupported) {
        return { supported: false, config };
      }
      return nativeAudioEncoder.isConfigSupported(config);
    }

    constructor(init: AudioEncoderInit) {
      super();
      this.callbacks = init;
    }

    get encodeQueueSize(): number {
      return this.impl?.encodeQueueSize ?? 0;
    }

    get state(): CodecState {
      if (this.impl) return this.impl.state;
      if (this.closed) return 'closed';
      return this.faac ? 'configured' : 'unconfigured';
    }

    configure(config: AudioEncoderConfig): void {
      this.config = config;
      if (isAacConfig(config)) {
        this.faac = createEncoder(config);
        this.nextTimestamp = 0;
        this.pendingTimestamps = [];
        this.firstOutput = true;
        this.closed = false;
        return;
      }

      if (!nativeAudioEncoder) {
        throw new Error(`AudioEncoder is not available for codec ${config.codec}`);
      }
      this.impl = new nativeAudioEncoder(this.callbacks);
      this.impl.configure(config);
    }

    encode(data: AudioData): void {
      if (this.impl) {
        this.impl.encode(data);
        return;
      }
      const faac = this.requireFaac();
      const config = this.requireConfig();
      const input = copyF32Interleaved(data);
      this.enqueueTimestamps(data);
      for (const bytes of faac.encodeF32Interleaved(input)) {
        this.emitChunk(bytes, this.dequeueTimestamp(config), config);
      }
    }

    async flush(): Promise<void> {
      if (this.impl) {
        await this.impl.flush();
        return;
      }
      const faac = this.requireFaac();
      const config = this.requireConfig();
      for (const bytes of faac.flush()) {
        this.emitChunk(bytes, this.dequeueTimestamp(config), config);
      }
    }

    reset(): void {
      this.impl?.reset();
      this.faac?.free();
      this.impl = null;
      this.faac = null;
      this.config = null;
      this.firstOutput = true;
      this.nextTimestamp = 0;
      this.pendingTimestamps = [];
      this.closed = false;
    }

    close(): void {
      this.impl?.close();
      this.faac?.free();
      this.impl = null;
      this.faac = null;
      this.pendingTimestamps = [];
      this.closed = true;
    }

    private requireFaac(): WasmFaacEncoder {
      if (!this.faac || this.closed) {
        throw new Error('AudioEncoder is not configured');
      }
      return this.faac;
    }

    private requireConfig(): AudioEncoderConfig {
      if (!this.config) {
        throw new Error('AudioEncoder is not configured');
      }
      return this.config;
    }

    private emitChunk(bytes: Uint8Array, timestamp: number, config: AudioEncoderConfig): void {
      const duration = frameDurationMicros(1024, config.sampleRate);
      const chunk = new EncodedAudioChunk({
        type: 'key',
        timestamp,
        duration,
        data: bytes,
      });
      const meta = this.firstOutput
        ? {
            decoderConfig: {
              codec: AAC_CODEC,
              sampleRate: config.sampleRate,
              numberOfChannels: config.numberOfChannels,
              description: this.faac?.audioSpecificConfig,
            },
          }
        : undefined;
      this.firstOutput = false;
      this.nextTimestamp = timestamp + duration;
      this.callbacks.output(chunk, meta);
    }

    private enqueueTimestamps(data: AudioData): void {
      const config = this.requireConfig();
      const faac = this.requireFaac();
      const framesPerChunk = faac.inputSamples / config.numberOfChannels;
      const duration = frameDurationMicros(framesPerChunk, config.sampleRate);
      const chunks = Math.ceil(data.numberOfFrames / framesPerChunk);
      for (let i = 0; i < chunks; i += 1) {
        this.pendingTimestamps.push(data.timestamp + i * duration);
      }
      this.nextTimestamp = data.timestamp + chunks * duration;
    }

    private dequeueTimestamp(config: AudioEncoderConfig): number {
      const timestamp = this.pendingTimestamps.shift();
      if (timestamp !== undefined) return timestamp;
      const fallback = this.nextTimestamp;
      this.nextTimestamp += frameDurationMicros(1024, config.sampleRate);
      return fallback;
    }
  } as unknown as AudioEncoderConstructor;
}

function isAacConfig(config: AudioEncoderConfig): boolean {
  return config.codec.toLowerCase() === AAC_CODEC;
}

function copyF32Interleaved(data: AudioData): Float32Array {
  const size = data.allocationSize({ planeIndex: 0, format: 'f32' });
  const buffer = new ArrayBuffer(size);
  data.copyTo(buffer, { planeIndex: 0, format: 'f32' });
  const normalized = new Float32Array(buffer);
  const samples = new Float32Array(normalized.length);
  for (let i = 0; i < normalized.length; i += 1) {
    samples[i] = normalizedToFaacPcm(normalized[i]);
  }
  return samples;
}

function frameDurationMicros(frames: number, sampleRate: number): number {
  return Math.round((frames / sampleRate) * 1_000_000);
}

function normalizedToFaacPcm(sample: number): number {
  if (sample <= -1) return -32768;
  if (sample >= 1) return 32767;
  return sample * 32768;
}
