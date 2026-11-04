/// OCIR 调试转储：op 流的人类可读 dump。高频 op 有语义化 detail；
/// 其余 opcode（含新增）由 `OPCODE_FIELDS` 布局表通用化——新增 opcode
/// 无需改 dump 就有正确的逐字段输出（互抄链断根）。

import { OP, OPCODE_FIELDS } from '../generated/ocir-schema.generated';
import {
  type EncodedDrawFrame,
  type OpEntry,
  Payload,
  decodeFrame,
  parseOps,
} from './decode';

export function dumpEncodedFrameOps(encoded: EncodedDrawFrame, limit = 400): string {
  const frame = decodeFrame(encoded);
  const entries = parseOps(frame.ops);
  const names: Record<number, string> = {};
  for (const [name, value] of Object.entries(OP)) names[value as number] = name;

  const f32 = (p: { f32: () => number }, n: number): number[] =>
    Array.from({ length: n }, () => p.f32());
  const fx = (v: number[]): string => v.map((x) => (Math.round(x * 1000) / 1000).toString()).join(',');

  const lines: string[] = [];
  const push = (s: string) => lines.push(s);
  push(`ops=${entries.length} subtrees=${frame.subtrees.length} paths=${frame.paths.length} strings=${frame.strings.length}`);
  const n = Math.min(entries.length, limit);
  for (let i = 0; i < n; i++) {
    const e = entries[i];
    const p = new Payload(frame.ops, e.payloadOffset, e.payloadLen);
    let detail = '';
    switch (e.opcode) {
      case OP.SAVE: break;
      case OP.SAVE_LAYER: {
        const flags = p.u8();
        const v = f32(p, 4);
        detail = `flags=${flags.toString(2).padStart(3, '0')} rect=[${fx(v)}] paint=${p.u32()} alpha=${p.f32().toFixed(2)}`;
        break;
      }
      case OP.SAVE_BACKDROP_LAYER: {
        const flags = p.u8();
        const v = f32(p, 4);
        const paint = p.u32();
        const alpha = p.f32();
        const radii = f32(p, 4);
        detail = `flags=${flags.toString(2).padStart(3, '0')} rect=[${fx(v)}] paint=${paint} alpha=${alpha.toFixed(2)} r=[${fx(radii)}]`;
        break;
      }
      case OP.RESTORE: break;
      case OP.RESTORE_TO_COUNT: detail = `count=${p.u32()}`; break;
      case OP.TRANSLATE: detail = `t=[${fx(f32(p, 2))}]`; break;
      case OP.SCALE: detail = `s=[${fx(f32(p, 2))}]`; break;
      case OP.ROTATE: detail = `r=[${fx(f32(p, 3))}]`; break;
      case OP.SKEW: detail = `k=[${fx(f32(p, 2))}]`; break;
      case OP.CONCAT: detail = `m=[${fx(f32(p, 9))}]`; break;
      case OP.SET_FILL_STYLE: detail = `fill=#${p.u32().toString(16)}`; break;
      case OP.SET_STROKE_STYLE: detail = `stroke=#${p.u32().toString(16)}`; break;
      case OP.SET_LINE_WIDTH: p.f32(); break;
      case OP.SET_GLOBAL_ALPHA: detail = `a=${p.f32().toFixed(3)}`; break;
      case OP.PAINT: detail = `paint=${p.u32()}`; break;
      case OP.RECT: {
        const v = f32(p, 4);
        detail = `rect=[${fx(v)}] paint=${p.u32()}`;
        break;
      }
      case OP.R_RECT: {
        const v = f32(p, 8);
        detail = `rr=[${fx(v.slice(0, 4))}] r=[${fx(v.slice(4))}] paint=${p.u32()}`;
        break;
      }
      case OP.D_RRECT: {
        const outer = f32(p, 8);
        const inner = f32(p, 8);
        detail = `outer=[${fx(outer)}] inner=[${fx(inner)}] paint=${p.u32()}`;
        break;
      }
      case OP.OVAL: {
        const v = f32(p, 4);
        detail = `oval=[${fx(v)}] paint=${p.u32()}`;
        break;
      }
      case OP.CIRCLE: detail = `c=[${fx(f32(p, 3))}] paint=${p.u32()}`; break;
      case OP.DRAW_PATH: detail = `path=${p.u32()} paint=${p.u32()}`; break;
      case OP.IMAGE_RECT: {
        // tag u8 + id-slot 12B (u32 string + u64 time) + src-present u8 +
        // src 4xf32 + dst 4xf32 + paint u32 (0xFFFFFFFF = no paint).
        const tag = p.u8();
        const asset = frame.strings[p.u32()] ?? '?';
        const time = p.u64();
        const hasSrc = p.u8() === 1;
        const src = f32(p, 4);
        const dst = f32(p, 4);
        const paintId = p.u32();
        const paint = paintId === 0xffff_ffff ? '' : ` paint=${paintId}`;
        detail = `tag=${tag} ${asset}@${Number(time) / 1e6}s src=${hasSrc ? `[${fx(src)}]` : 'none'} dst=[${fx(dst)}]${paint}`;
        break;
      }
      case OP.REPLAY_RANGE: detail = `range=[${p.u32()}+${p.u32()}]`; break;
      case OP.DRAW_SUBTREE_PICTURE: detail = `subtree=${p.u32()} at=[${fx(f32(p, 2))}]`; break;
      default: {
        // Wire-authoritative generic dump straight from the OPCODE_FIELDS
        // layout table — new opcodes dump correctly without touching this
        // file (the third hand-written field-order copy ends here).
        const fields = OPCODE_FIELDS[e.opcode] ?? [];
        const parts: string[] = [];
        for (const field of fields) {
          const bytes = field.bytes;
          const width = bytes === 1 ? 'u8' : bytes === 4 ? 'u32' : 'f32x' + bytes / 4;
          parts.push(`${field.name}(${width})`);
          for (let b = 0; b < bytes; b += 4) p.u32();
        }
        detail = parts.join(' ') || `${e.payloadLen}B`;
      }
    }
    push(`${i}: ${names[e.opcode] ?? e.opcode} ${detail}`.trimEnd());
  }
  if (entries.length > n) push(`… ${entries.length - n} more`);
  // Subtree op streams: the picture caches per-subtree draws.
  for (let si = 0; si < frame.subtrees.length && si < 8; si++) {
    const subEntries = parseOps(frame.subtrees[si]);
    push(`— subtree ${si}: ${subEntries.length} ops`);
  }
  return lines.join('\n');
}

