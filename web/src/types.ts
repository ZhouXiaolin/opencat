// ── Composition ──

export interface CompositionInfo {
  width: number;
  height: number;
  fps: number;
  frames: number;
}

// ── Parsed JSONL (from WASM) ──

export interface ParsedElement {
  type: string;
  id?: string;
  parentId?: string | null;
  className?: string | null;
  text?: string;
  d?: string;
  path?: string;
  src?: string;
  icon?: string;
  from?: string;
  to?: string;
  effect?: string;
  duration?: number;
  [key: string]: unknown;
}

export interface ParsedResult {
  composition: CompositionInfo | null;
  elements: ParsedElement[];
  elementCount: number;
}

export interface JsonlFile {
  name: string;
  path: string;
}

// ── Resource Management ──

export interface ResourceRequests {
  images: string[];
  videos: string[];
  audios: string[];
  icons: string[];
}

export interface LoadedImage {
  path: string;
  ckImage: any; // CanvasKit Image
  width: number;
  height: number;
}

export interface VideoFrame {
  data: Uint8Array;
  width: number;
  height: number;
}

// ── Display List (mapped from Rust DisplayTree) ──

export interface DisplayRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface DisplayClip {
  bounds: DisplayRect;
  borderRadius: BorderRadius;
}

export interface BorderRadius {
  topLeft: number;
  topRight: number;
  bottomRight: number;
  bottomLeft: number;
}

export interface Color4f {
  r: number;
  g: number;
  b: number;
  a: number;
}

export interface BackgroundFillJson {
  type: 'solid' | 'linearGradient';
  color?: Color4f;
  direction?: string;
  from?: Color4f;
  via?: Color4f | null;
  to?: Color4f;
}

export interface BoxShadowJson {
  offsetX: number;
  offsetY: number;
  blurSigma: number;
  spread: number;
  color: Color4f;
}

export interface DropShadowJson {
  offsetX: number;
  offsetY: number;
  blurSigma: number;
  color: Color4f;
}

export interface InsetShadowJson {
  offsetX: number;
  offsetY: number;
  blurSigma: number;
  spread: number;
  color: Color4f;
}

export interface RectPaintJson {
  background?: BackgroundFillJson | null;
  borderRadius: BorderRadius;
  borderWidth?: number | null;
  borderTopWidth?: number | null;
  borderRightWidth?: number | null;
  borderBottomWidth?: number | null;
  borderLeftWidth?: number | null;
  borderColor?: Color4f | null;
  borderStyle?: string | null;
  blurSigma?: number | null;
  boxShadow?: BoxShadowJson | null;
  insetShadow?: InsetShadowJson | null;
  dropShadow?: DropShadowJson | null;
}

export interface ComputedTextStyleJson {
  color: Color4f;
  textPx: number;
  fontWeight: number;
  letterSpacing: number;
  textAlign: 'left' | 'center' | 'right';
  lineHeight: number;
  lineHeightPx?: number | null;
  textTransform: 'none' | 'uppercase';
  wrapText: boolean;
  lineThrough: boolean;
}

export interface SvgPathPaintJson {
  fill?: BackgroundFillJson | null;
  strokeWidth?: number | null;
  strokeColor?: Color4f | null;
  dropShadow?: DropShadowJson | null;
  strokeDasharray?: number | null;
  strokeDashoffset?: number | null;
}

export interface DisplayTransformJson {
  translationX: number;
  translationY: number;
  bounds: DisplayRect;
  transforms: TransformJson[];
}

export interface TransformJson {
  type: string;
  value?: number;
  x?: number;
  y?: number;
}

export interface CanvasCommandJson {
  type: string;
  [key: string]: unknown;
}

export interface DisplayItemJson {
  type: 'rect' | 'text' | 'bitmap' | 'drawScript' | 'svgPath' | 'timeline';
  bounds: DisplayRect;
  // Rect/Timeline
  paint?: RectPaintJson;
  transition?: {
    progress: number;
    kind: { type: string; direction?: string };
  } | null;
  // Text
  text?: string;
  style?: ComputedTextStyleJson;
  allowWrap?: boolean;
  truncate?: boolean;
  dropShadow?: DropShadowJson | null;
  visualExpandX?: number;
  visualExpandY?: number;
  // Bitmap
  assetId?: string;
  width?: number;
  height?: number;
  objectFit?: string;
  // DrawScript
  commands?: CanvasCommandJson[];
  // SvgPath
  pathData?: string[];
  viewBox?: [number, number, number, number];
  svgPaint?: SvgPathPaintJson;
}

export interface DisplayNodeJson {
  elementId: number;
  transform: DisplayTransformJson;
  opacity: number;
  backdropBlurSigma?: number | null;
  clip?: DisplayClip | null;
  item: DisplayItemJson;
  children: DisplayNodeJson[];
}

export interface DisplayTreeResult {
  composition: CompositionInfo;
  displayTree: DisplayNodeJson;
}

// ── Extended Element for layout computation ──

export interface LayoutElement {
  id: string;
  type: string;
  parentId: string | null;
  className: string;
  text?: string;
  icon?: string;
  path?: string;
  url?: string;
  d?: string;
  src?: string;
  duration?: number;
  children: LayoutElement[];
}
