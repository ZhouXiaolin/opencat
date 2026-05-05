export interface CompositionInfo {
  width: number;
  height: number;
  fps: number;
  frames: number;
}

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
