declare module '../pkg/opencat_web.js' {
  const init: () => Promise<void>;
  export default init;

  export class WebRenderer {
    constructor();
    build_frame(jsonl: string, frame: number, ck_canvas: unknown, resources_json: string): void;
    inject_video_frame(asset_id: string, frame: number, rgba: Uint8Array, width: number, height: number): void;
    clear_video_cache(asset_id: string): void;
    plan_video_frames(jsonl: string, frame: number, resources_json: string): string;
    inject_image_bytes(asset_id: string, bytes: Uint8Array): void;
    clear_image_blobs(): void;
    decode_audio_file(asset_id: string, data: Uint8Array): Promise<void>;
    get_audio_samples(asset_id: string, start_secs: number, duration_secs: number, target_rate: number): string;
    play_audio_at(asset_id: string, offset_secs: number, duration_secs: number): void;
    stop_audio(): void;
    set_audio_volume(volume: number): void;
    clear_audio_cache(): void;
    audio_context_time(): number;
  }

  export function init_canvaskit(): void;
  export function preload_assets(jsonl: string): Promise<string>;
  export function get_blob_bytes(asset_id: string): Uint8Array | undefined;
  export function clear_blobs(): void;
  export function blob_count(): number;
}
