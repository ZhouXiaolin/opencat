// @omp 团队执行体（浏览器侧）：engine `v8_team.rs` 的 TS 同码版——同一
// bundle 源码合同（protocol.rs IIFE：`{__ocHeader, __ocRow, __ocSetCaps,
// __ocNext, __ocSetPrng}`），Rust `WebOmpTeam` 经全局 `__ocOmpExecute`
// 派发。本版主线程单行派发（调度参数只影响负载分布不影响字节）；worker
// + SAB 版迁入时保持同一合同不变。
//
// 行产出 = 单段 Float64Array（头 `[n_slots, n_ovals]` + slots 四元组 +
// ovals 七元组），此处按行序拼接为单数组，Rust 按段头切分。

type OcBundleInst = {
  __ocHeader: () => number[];
  __ocRow: (rowVal: number) => Float64Array;
  __ocSetCaps: (...caps: unknown[]) => void;
  __ocNext: () => number;
  __ocSetPrng: (buf: Float64Array) => void;
  capsKey?: string;
  engaged?: boolean;
};

// kind 码与 Rust `omp_team.rs::kind_code` 对齐（表序即协议）。
const TA_KINDS: (new (buf: ArrayBuffer, off: number, len: number) => ArrayLike<number>)[] = [
  Float64Array,
  Float32Array,
  Uint8ClampedArray,
  Uint8Array,
  Int8Array,
  Uint16Array,
  Int16Array,
  Uint32Array,
  Int32Array,
];
const TA_SIZE = [8, 4, 4, 1, 1, 2, 2, 4, 4];

const bundles = new Map<string, OcBundleInst>();

// 解析树里的 TypedArray 描述符 → 从 shared 重建真 TypedArray（engine
// `rebuild_shared` 同语义：per-execute 字节克隆 + 独立 buffer——v1 快照
// 语义，行函数对 TA 的写不出本次执行）。非描述符容器递归替换成员。
function rebuildShared(
  v: unknown,
  bytes: Uint8Array[],
  kinds: number[],
): unknown {
  if (typeof v !== 'object' || v === null) return v;
  const d = (v as Record<string, unknown>).__ocOmpShared;
  if (d !== undefined) {
    const { slot, len } = d as { slot: number; len: number };
    const kind = kinds[slot];
    const buf = bytes[slot];
    if (buf.length !== len * TA_SIZE[kind]) {
      throw new Error('shared: byte size mismatch');
    }
    return new TA_KINDS[kind](buf.buffer as ArrayBuffer, 0, len);
  }
  if (Array.isArray(v)) return v.map((m) => rebuildShared(m, bytes, kinds));
  const src = v as Record<string, unknown>;
  const out: Record<string, unknown> = {};
  for (const k of Object.keys(src)) {
    out[k] = rebuildShared(src[k], bytes, kinds);
  }
  return out;
}

// capture 装载：同一帧的所有行共享同一 args_json 文本——文本相等即已
// 装载，跳过 setCaps 与 JSON 重解析（engine 用 Arc 指针相等，同值即同
// 语义，文本比较是 web 腿的等价形态）。
function ensureCaps(
  inst: OcBundleInst,
  argsJson: string,
  bytes: Uint8Array[],
  kinds: number[],
): void {
  if (inst.capsKey === argsJson) return;
  const parsed = JSON.parse(argsJson) as unknown[];
  const args = parsed.map((a) => rebuildShared(a, bytes, kinds));
  inst.__ocSetCaps(...args);
  inst.capsKey = argsJson;
}

export function ompExecute(
  bundleSrc: string,
  bundleKey: string,
  argsJson: string,
  sharedBytes: Uint8Array[],
  sharedKinds: number[],
  prng: Float64Array,
  _staticFlag: boolean,
  _chunk: number,
  _numThreads: number | null,
): Float64Array {
  let inst = bundles.get(bundleKey);
  if (!inst) {
    // bundle src 是自包含 IIFE，`return` 包一层取实例值（全局作用域，
    // 免 direct eval）；按 key 缓存——每 realm 首帧构造一次。
    inst = new Function(
      `return ${bundleSrc}\n//# sourceURL=oc-omp-${bundleKey}`,
    )() as OcBundleInst;
    bundles.set(bundleKey, inst);
  }
  ensureCaps(inst, argsJson, sharedBytes, sharedKinds);
  // 参与证明（engine `mark_route` 的 web 对位）：每 bundle 首次执行打一行。
  if (!inst.engaged) {
    inst.engaged = true;
    console.info(`[omp] web team engaged: ${bundleKey}`);
  }
  const ys = inst.__ocHeader();
  const n = ys.length;
  if (n === 0) return new Float64Array(0);
  // 行预采 stride：prng 总数必须按行数均分（TR15 每行迭代数一致；不整
  // 除即语义错位 = 抛 → Rust 侧毒化降级）。
  const stride = prng.length / n;
  if (!Number.isInteger(stride)) {
    throw new Error(`prng ${prng.length} not divisible by rows ${n}`);
  }
  // 行段立即拷贝：__ocRow 返回的是 bundle 常驻 out 缓冲的 subarray 视
  // 图，下一次行调用即覆盖。
  const segs: Float64Array[] = [];
  let total = 0;
  for (let i = 0; i < n; i++) {
    if (stride > 0) {
      inst.__ocSetPrng(prng.subarray(i * stride, (i + 1) * stride));
    }
    const seg = inst.__ocRow(ys[i]);
    const copy = seg.slice();
    segs.push(copy);
    total += copy.length;
  }
  const out = new Float64Array(total);
  let off = 0;
  for (const s of segs) {
    out.set(s, off);
    off += s.length;
  }
  return out;
}

export function installOmpRunner(): void {
  const g = globalThis as unknown as Record<string, unknown>;
  if (typeof g.__ocOmpExecute === 'function') return;
  g.__ocOmpExecute = ompExecute;
}
