/**
 * Web OMP runner 调度合同定点验证（批次 D 专项，非常设门）：
 *
 * 用真实 Worker 池（Bun worker = 真线程 + 真 SAB futex）跑 runner，
 * 覆盖 static / dynamic(1) / dynamic(chunk) / 无子句 / num_threads 上限
 * / overflow fallback / rowerr fallback / 团队大小重建，逐帧输出与独立
 * 串行参考逐位比对，并经 console 面包屑观测实际池 worker 数不超
 * num_threads 请求。
 *
 * 协议合同对位 protocol.rs bundle IIFE：行产出 = [nSlots, nOvals] 头 +
 * slots 四元组 + ovals 七元组；每行精确消费 2 个 PRNG（stride=2）。
 * 行数学全用整数，规避跨 realm 浮点差异话题。
 */
import { afterAll, beforeAll, describe, expect, it } from 'bun:test';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ompExecute } from './omp-runner';

// ── Bun 环境替身：runner 的 worker 池用 Blob URL 起 realm（浏览器合同），
// Bun 1.3 不支持（BuildMessage: Blob URL is missing）。替身 = 构造 Blob
// 时同步捕获引导串 → 落地临时文件 → createObjectURL 返回文件路径
// （Bun Worker 接受绝对路径）。只影响本测试进程。
const RealBlob = Blob;
class TestBlob extends RealBlob {
  src: string;
  constructor(parts: BlobPart[], opts?: BlobPropertyBag) {
    super(parts, opts);
    this.src = parts.map((p) => (typeof p === 'string' ? p : '')).join('');
  }
}
(globalThis as unknown as { Blob: unknown }).Blob = TestBlob;
const tmpDir = mkdtempSync(join(tmpdir(), 'oc-ompw-test-'));
let workerFileSeq = 0;
URL.createObjectURL = ((obj: Blob): string => {
  const file = join(tmpDir, `worker-${workerFileSeq++}.js`);
  writeFileSync(file, (obj as TestBlob).src ?? '');
  return file;
}) as typeof URL.createObjectURL;
URL.revokeObjectURL = (() => {}) as typeof URL.revokeObjectURL;

// ── console 面包屑捕获（runner 的唯一可观测面）──
const logs: string[] = [];
const origInfo = console.info.bind(console);
console.info = (...args: unknown[]) => {
  logs.push(args.map((a) => String(a)).join(' '));
  origInfo(...args);
};
const logsSince = (mark: number): string[] => logs.slice(mark);
const workerCountFrom = (lines: string[], tag: string): number => {
  const line = lines.find((l) => l.includes(tag));
  expect(line).toBeDefined();
  const m = /workers=(\d+)/.exec(line as string);
  expect(m).not.toBeNull();
  return Number(m![1]);
};

// ── 合成 bundle（protocol.rs IIFE 同合同）──
// caps = (w, h, k)：h = 行数，k = 每行 oval 数（k 漂移 → 行段长度漂移，
// 驱动 overflow fallback；k=9 且 y=5 → 行体抛错，驱动 errRow fallback）。
const SRC = `(function () {
  let W = 0, H = 0, K = 1;
  let __ocPrngBuf = null, __ocPrngCur = 0;
  function __ocNext() {
    if (!__ocPrngBuf || __ocPrngCur >= __ocPrngBuf.length) throw new Error('prng overrun');
    return __ocPrngBuf[__ocPrngCur++];
  }
  function __ocSetCaps(w, h, k) { W = w; H = h; K = k; }
  function __ocHeader() { const ys = []; for (let y = 0; y < H; y++) ys.push(y); return ys; }
  function __ocRow(y) {
    const a = __ocNext(), b = __ocNext();
    if (K === 9 && y === 5) throw new Error('boom row');
    const out = new Float64Array(2 + 4 + 7 * K);
    out[0] = 1; out[1] = K;
    out[2] = 0; out[3] = y; out[4] = 0; out[5] = a * 2 + b * 3 + y;
    for (let i = 0; i < 7 * K; i++) out[6 + i] = (y + i) * (a + b + 1);
    return out;
  }
  return { __ocHeader, __ocRow, __ocSetCaps, __ocNext, __ocSetPrng(buf) { __ocPrngBuf = buf; __ocPrngCur = 0; } };
})()`;

const ROWS = 64;
const STRIDE = 2;

function makePrng(): Float64Array {
  const prng = new Float64Array(ROWS * STRIDE);
  for (let i = 0; i < prng.length; i++) prng[i] = ((i * 37) % 101) / 101;
  return prng;
}

/** 独立串行参考：主 realm 直接逐行跑同一 bundle（runner runSerial 同构）。 */
function serialRef(k: number): Float64Array {
  const inst = new Function(`return ${SRC}`)();
  inst.__ocSetCaps(192, ROWS, k);
  const ys: number[] = inst.__ocHeader();
  expect(ys.length).toBe(ROWS);
  const prng = makePrng();
  const segs: Float64Array[] = [];
  let total = 0;
  for (let i = 0; i < ROWS; i++) {
    inst.__ocSetPrng(prng.subarray(i * STRIDE, (i + 1) * STRIDE));
    const seg = inst.__ocRow(ys[i]).slice();
    segs.push(seg);
    total += seg.length;
  }
  const out = new Float64Array(total);
  let off = 0;
  for (const s of segs) {
    out.set(s, off);
    off += s.length;
  }
  return out;
}

function expectEq(actual: Float64Array, expected: Float64Array, tag: string): void {
  if (actual.length !== expected.length) {
    throw new Error(`${tag}: length ${actual.length} != ${expected.length}`);
  }
  for (let i = 0; i < expected.length; i++) {
    if (actual[i] !== expected[i]) {
      throw new Error(`${tag}: mismatch at [${i}]: ${actual[i]} != ${expected[i]}`);
    }
  }
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

type Sched = { staticFlag: boolean; chunk: number; numThreads: number | null };

/** 一次串行量测帧（每 key 首帧）+ 轮询到池真正派发，返回各帧输出。 */
async function driveToPool(key: string, k: number, sched: Sched, maxTries = 80): Promise<{ outs: Float64Array[]; lines: string[] }> {
  const mark = logs.length;
  const prng = makePrng();
  const args = () => [SRC, key, JSON.stringify([192, ROWS, k]), [], [], prng, sched.staticFlag, sched.chunk, sched.numThreads] as const;
  const outs: Float64Array[] = [];
  outs.push(ompExecute(...args())); // 量测帧（串行）
  for (let i = 0; i < maxTries; i++) {
    const before = logs.length;
    const out = ompExecute(...args());
    if (logs.slice(before).some((l) => l.includes('[ompw] first dispatch'))) {
      outs.push(out);
      return { outs, lines: logsSince(mark) };
    }
    outs.push(out);
    await sleep(25);
  }
  throw new Error(`${key}: pool dispatch never happened\n${logsSince(mark).join('\n')}`);
}

describe('omp-runner 调度合同（批次 D 定点）', () => {
  beforeAll(() => {
    (globalThis as unknown as { __ocOmpThreads?: number }).__ocOmpThreads = 4;
  });
  afterAll(() => {
    delete (globalThis as unknown as { __ocOmpThreads?: number }).__ocOmpThreads;
  });

  it('无子句（缺省 work-stealing）：输出与串行逐位一致，worker=缺省 4', async () => {
    const ref = serialRef(3);
    const { outs, lines } = await driveToPool('sched/default', 3, { staticFlag: false, chunk: 1, numThreads: null });
    for (const o of outs) expectEq(o, ref, 'default');
    expect(lines.find((l) => l.includes('sched=dynamic,1'))).toBeDefined();
    expect(workerCountFrom(lines, 'first dispatch')).toBe(4);
  });

  it('schedule(static)：连续行段，输出与串行逐位一致', async () => {
    const ref = serialRef(3);
    const { outs, lines } = await driveToPool('sched/static', 3, { staticFlag: true, chunk: 1, numThreads: null });
    for (const o of outs) expectEq(o, ref, 'static');
    expect(lines.find((l) => l.includes('sched=static'))).toBeDefined();
    expect(workerCountFrom(lines, 'first dispatch')).toBe(4);
  });

  it('schedule(dynamic,4)：chunk 原子取块，输出与串行逐位一致', async () => {
    const ref = serialRef(3);
    const { outs, lines } = await driveToPool('sched/dyn4', 3, { staticFlag: false, chunk: 4, numThreads: null });
    for (const o of outs) expectEq(o, ref, 'dynamic,4');
    expect(lines.find((l) => l.includes('sched=dynamic,4'))).toBeDefined();
    expect(workerCountFrom(lines, 'first dispatch')).toBe(4);
  });

  it('num_threads(2)：池 worker 数受上限约束，输出不变', async () => {
    const ref = serialRef(3);
    const { outs, lines } = await driveToPool('sched/nt2', 3, { staticFlag: false, chunk: 1, numThreads: 2 });
    for (const o of outs) expectEq(o, ref, 'nt2');
    expect(workerCountFrom(lines, 'pool spawned')).toBe(2);
    expect(workerCountFrom(lines, 'first dispatch')).toBe(2);
  });

  it('num_threads(1) + static：单 worker 跑满全段，输出不变', async () => {
    const ref = serialRef(3);
    const { outs, lines } = await driveToPool('sched/nt1-static', 3, { staticFlag: true, chunk: 1, numThreads: 1 });
    for (const o of outs) expectEq(o, ref, 'nt1-static');
    expect(lines.find((l) => l.includes('sched=static'))).toBeDefined();
    expect(workerCountFrom(lines, 'first dispatch')).toBe(1);
  });

  it('num_threads 超额请求（999）：worker 数只给缺省面 4（≤ 请求）', async () => {
    const ref = serialRef(3);
    const { outs, lines } = await driveToPool('sched/nt-big', 3, { staticFlag: false, chunk: 1, numThreads: 999 });
    for (const o of outs) expectEq(o, ref, 'nt-big');
    expect(workerCountFrom(lines, 'first dispatch')).toBe(4);
  });

  it('团队大小变化（覆盖 4→2）：杀池重建，新池 worker=2，输出不变', async () => {
    const ref = serialRef(3);
    const g = globalThis as unknown as { __ocOmpThreads?: number };
    const { outs, lines } = await driveToPool('sched/team', 3, { staticFlag: false, chunk: 1, numThreads: null });
    expect(workerCountFrom(lines, 'first dispatch')).toBe(4);
    g.__ocOmpThreads = 2; // 下一帧起 wantThreads=2 → respawn
    try {
      const mark = logs.length;
      const prng = makePrng();
      const args = () => [SRC, 'sched/team', JSON.stringify([192, ROWS, 3]), [], [], prng, false, 1, null] as const;
      outs.push(ompExecute(...args())); // respawn 帧（串行，输出不变）
      expectEq(outs[outs.length - 1], ref, 'team respawn frame');
      let dispatched = -1;
      for (let i = 0; i < 80; i++) {
        const before = logs.length;
        const out = ompExecute(...args());
        if (logs.slice(before).some((l) => l.includes('[ompw] first dispatch'))) {
          dispatched = outs.length;
          outs.push(out);
          break;
        }
        outs.push(out);
        await sleep(25);
      }
      expect(dispatched).toBeGreaterThanOrEqual(0);
      for (const o of outs) expectEq(o, ref, 'team-change');
      expect(logsSince(mark).some((l) => l.includes('threads 4->2'))).toBe(true);
      expect(workerCountFrom(logsSince(mark), 'pool respawned')).toBe(2);
    } finally {
      g.__ocOmpThreads = 4;
    }
  });

  it('overflow fallback：行段超槽 → 当帧串行 + 扩表重建，输出不变', async () => {
    // 帧 1 用 k=1 量测（rowStride 定为 13）；帧 2 起 k=3（行段 27 > 13）
    // → worker 置 overflow → 当帧串行 + 重建；帧 3+ 走新池。
    const ref1 = serialRef(1);
    const ref3 = serialRef(3);
    const mark = logs.length;
    const prng = makePrng();
    const call = (k: number) =>
      ompExecute(SRC, 'sched/overflow', JSON.stringify([192, ROWS, k]), [], [], prng, false, 1, null);
    expectEq(call(1), ref1, 'overflow measure frame');
    // 等池就绪并完成一次 k=1 派发（建立 announced 基线）。
    for (let i = 0; i < 80; i++) {
      const before = logs.length;
      const out = call(1);
      if (logs.slice(before).some((l) => l.includes('[ompw] first dispatch'))) {
        expectEq(out, ref1, 'overflow pre-fallback dispatch');
        break;
      }
      await sleep(25);
    }
    const out2 = call(3); // 越顶帧 → overflow fallback 串行
    expectEq(out2, ref3, 'overflow fallback frame');
    expect(logsSince(mark).some((l) => l.includes('[ompw] fallback overflow=1'))).toBe(true);
    // 恢复判据：重建池（rowStride=83 ≥ 27）真正派发的帧——新池的
    // first dispatch 行 + 该帧无新 fallback。
    const afterFallback = logs.length;
    let recovered = false;
    for (let i = 0; i < 80; i++) {
      const before = logs.length;
      const out = call(3);
      const newLines = logs.slice(before);
      if (newLines.some((l) => l.includes('[ompw] first dispatch')) && !newLines.some((l) => l.includes('[ompw] fallback'))) {
        expectEq(out, ref3, 'overflow recovered dispatch');
        recovered = true;
        break;
      }
      await sleep(25);
    }
    expect(recovered).toBe(true);
    // 重建池再派发：announced 每池一行 → fallback 后新池必然再打一行
    // first dispatch（fallback 重建路径成功时不打 respawned 行）。
    expect(logs.slice(afterFallback).filter((l) => l.includes('[ompw] first dispatch')).length).toBeGreaterThanOrEqual(1);
  });

  it('rowerr fallback（毒化路径前端）：行体抛错帧抛出，健康帧恢复且输出不变', async () => {
    const ref3 = serialRef(3);
    const prng = makePrng();
    const call = (k: number) =>
      ompExecute(SRC, 'sched/rowerr', JSON.stringify([192, ROWS, k]), [], [], prng, false, 1, null);
    expectEq(call(3), ref3, 'rowerr measure frame');
    for (let i = 0; i < 80; i++) {
      const before = logs.length;
      const out = call(3);
      if (logs.slice(before).some((l) => l.includes('[ompw] first dispatch'))) {
        expectEq(out, ref3, 'rowerr pre-dispatch');
        break;
      }
      await sleep(25);
    }
    // k=9 帧：worker 行体抛错 → errRow → 串行重跑同抛（上层 core 毒化跑
    // 原循环，输出由构造保证）。runner 合同 = 错误上抛，不静默半应用。
    expect(() => call(9)).toThrow('boom row');
    expect(logs.some((l) => l.includes('errRow=5'))).toBe(true);
    // 恢复：单次错误不毒化，健康帧重建池后照常并行且输出一致。
    let recovered = false;
    for (let i = 0; i < 80; i++) {
      const before = logs.length;
      const out = call(3);
      if (logs.slice(before).some((l) => l.includes('[ompw] first dispatch'))) {
        expectEq(out, ref3, 'rowerr recovered dispatch');
        recovered = true;
        break;
      }
      expectEq(out, ref3, 'rowerr recovery serial frame');
      await sleep(25);
    }
    expect(recovered).toBe(true);
  });
});
