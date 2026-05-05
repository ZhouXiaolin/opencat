/// <reference types="vite/client" />

declare module '../wasm/opencat_web.js' {
  export default function init(): Promise<void>;
  export function parse_jsonl(input: string): string;
  export function get_composition_info(input: string): string;
  export function collect_resources_json(input: string): string;
}
