// Shared RPC envelope between the main-thread decoder client
// and the video-decode worker.

export type VideoPreviewQuality = 'scrubbing' | 'realtime' | 'exact';

export interface VideoSourceMeta {
  width: number;
  height: number;
  durationSecs: number | null;
}

// ── Requests (main → worker) ──

export interface PrepareRequest {
  type: 'prepare';
  id: number;
  assetId: string;
  buffer: ArrayBuffer; // transferred
}

export interface GetFrameRequest {
  type: 'getFrame';
  id: number;
  assetId: string;
  timeSecs: number;
  quality: VideoPreviewQuality;
}

export interface PrefetchFrameRequest {
  type: 'prefetchFrame';
  id: number;
  assetId: string;
  timeSecs: number;
  quality: VideoPreviewQuality;
}

export interface ReleaseRequest {
  type: 'release';
  id: number;
  assetId: string;
}

export type WorkerRequest =
  | PrepareRequest
  | GetFrameRequest
  | PrefetchFrameRequest
  | ReleaseRequest;

// ── Responses (worker → main) ──

export interface PrepareResponse {
  type: 'prepare';
  id: number;
  meta: VideoSourceMeta;
}

export interface GetFrameResponse {
  type: 'getFrame';
  id: number;
  frame: VideoFrame | null; // transferred
}

export interface PrefetchFrameResponse {
  type: 'prefetchFrame';
  id: number;
  ok: boolean;
}

export interface ReleaseResponse {
  type: 'release';
  id: number;
}

export interface ErrorResponse {
  type: 'error';
  id: number;
  message: string;
}

export type WorkerResponse =
  | PrepareResponse
  | GetFrameResponse
  | PrefetchFrameResponse
  | ReleaseResponse
  | ErrorResponse;
