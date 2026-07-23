declare module '../pkg/opencat_web.js' {
  const init: () => Promise<void>;
  export default init;

  export class WebRenderer {
    constructor();
    open_design(compositionSource: string): Promise<string>;
    build_frame_ir(frame: number): { ir: Uint8Array; mediaPlan: string };
    get_frame_plan(frame: number): string;
    load_default_fonts(sans_sc: Uint8Array, color_emoji: Uint8Array): void;
    load_font_data(bytes: Uint8Array): void;
    decode_audio_file(asset_id: string, data: Uint8Array): Promise<void>;
    get_audio_samples(asset_id: string, start_secs: number, duration_secs: number, target_rate: number): string;
    play_audio_at(asset_id: string, offset_secs: number, duration_secs: number): void;
    stop_audio(): void;
    set_audio_volume(volume: number): void;
    clear_audio_cache(): void;
    audio_context_time(): number;
    audio_plan(): string;
  }

  export function preload_assets(compositionSource: string): Promise<string>;
  export function get_blob_bytes(asset_id: string): Uint8Array | undefined;
  export function clear_blobs(): void;
  export function blob_count(): number;
}
