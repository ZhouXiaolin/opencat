// @omp 团队执行体（浏览器侧）：engine `v8_team.rs` 的 TS 同码版——同一
// bundle 源码合同（protocol.rs IIFE：`{__ocHeader, __ocRow, __ocSetCaps,
// __ocSetPrng, __ocNext}`），Rust `WebOmpTeam` 经全局 `__ocOmpExecute`
// 派发。
//
// 执行拓扑：worker 池（SAB 任务板 + work-stealing）并行跑行体，主线程
// 自旋等待后按行序拼装（protocol.rs「按行序应用——完成序无关」→ 并行
// 结果按行槽归位即与串行逐字节一致）。worker 是纯 JS realm（bundle 为
// 自包含 IIFE，不涉 wasm）；oracle 静态服务已设 COOP/COEP（跨域隔离），
// SharedArrayBuffer 直接可用。同步合同不动：Rust 侧仍同步拿到
// Float64Array，主线程 spin 只占一个核（oracle 导出本就 CPU 密集）。
//
// caps 通道：caps（argsJson+shared 字节+kinds）每帧可漂移（spacing/相位
// 等动画量），worker 的 caps 不能冻结在 spawn 帧——主线程每帧把最新
// caps 写进 SAB 文本/字节区并以 board[8]=capsSeq 原子发布（先写数据后
// 推 phase，内存模型 store-release：观察到新 phase 必见全部数据写）；
// worker 醒来发现序号更新即解码重设 __ocSetCaps。worker→main 消息面在
// 本环境被主线程事件循环饿死（realm shim 把 setTimeout 变成即时
// resolve 的假 yield），就绪/派发一律走 SAB 原子量，消息只作事后诊断。
//
// 降级链（任何一环不满足即回主线程串行，输出不变）：
//   无 SAB / worker 初始化失败 → 永久串行；
//   行段超上界 → 当帧串行 + 上界扩表重建池；
//   行体抛错 → 当帧串行 + 池重建（连续多次 → 永久串行）；
//   自旋看门狗 → 永久串行。
//   fallback 路径一律 terminate 池再重建：主线程提前返回后 worker 仍在
//   烧行体，下一帧派发会撞上未息的 worker（stale rowsDone++ 会把
//   rowsDone==n 的自旋判据永久打坏），宁杀勿猜。
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
const TA_KINDS: (new (buf: ArrayBufferLike, off: number, len: number) => ArrayLike<number>)[] = [
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
// 语义，行函数对 TA 的写不出本次执行；写回经 __ocOmpSet 落行私有缓冲，
// 故这里的 TA 实为只读面）。byteOffset 感知：调用方给的 buf 可以是
// 更大 buffer 的子视图（worker 侧是 SAB 字节区的槽位子视图）。非描述
// 符容器递归替换成员。
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
    return new TA_KINDS[kind](buf.buffer as ArrayBufferLike, buf.byteOffset, len);
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

// ── worker 池（SAB 任务板）──
// board（Int32Array，16 字）：[0]phase [1]workIdx [2]rowsDone [3]n
// [4]stride [5]overflow [6]errRow [7]ready [8]capsSeq [9..15]保留。
// worker→main 消息在本环境被主线程事件循环饿死（spin 期送不出来，
// terminate 又丢待发消息）——worker 崩溃诊断唯一可靠通道 = catch 路径
// 把错误全文写进 text 区尾部预留位（见 workerBootstrap），watchdog 同步读。
// SAB 字节布局（各区 8B 对齐）：board(64B) → text(u8×textCap：[u32
// kindsLen][u32 argsLen][kinds u8×][args UTF-8]) → bytes(u8×bytesCap：
// [u32 n][u32 len×n][payloads 顺排]) → prng(f64×prngCap) → ys(f64×nCap)
// → results(f64×n×rowStride，每行一个固定槽)。
type WorkerPool = {
  bundleKey: string;
  workers: Worker[];
  sab: SharedArrayBuffer;
  board: Int32Array;
  prng: Float64Array;
  ys: Float64Array;
  results: Float64Array;
  textU8: Uint8Array;
  bytesU8: Uint8Array;
  textOff: number;
  bytesOff: number;
  prngCap: number;
  nCap: number;
  rowStride: number;
  textCap: number;
  bytesCap: number;
  phase: number;
  ready: number;
  announced?: boolean;
};

// caps 区预算（u8 数）：文本头 8B + kinds + UTF-8；字节区头 4(n+1)B +
// 载荷。余量翻倍 + 64KB 头Room，漂移即 writeCaps false → 扩表重建。
function capsBudget(
  argsJson: string,
  bytes: Uint8Array[],
  kinds: number[],
): { textNeed: number; bytesNeed: number } {
  const enc = capsEnc();
  const utf8 = enc.encode(argsJson);
  const textNeed = 8 + kinds.length + utf8.length;
  const align8 = (x: number) => (x + 7) & ~7;
  // SAB 各区 8B 对齐（后随 prng/ys/results 的 f64 视图），预算向上取整；
  // bytes 区每槽起点也按 8 对齐（worker 侧从槽子视图建 f64 视图）。
  let bytesNeed = align8(4 * (bytes.length + 1));
  for (const b of bytes) bytesNeed += align8(b.length);
  return { textNeed: align8(textNeed * 2 + 65536), bytesNeed: align8(bytesNeed * 2 + 65536) };
}

let _capsEnc: TextEncoder | null = null;
function capsEnc(): TextEncoder {
  if (!_capsEnc) _capsEnc = new TextEncoder();
  return _capsEnc;
}

// 派发前把最新 caps 写进 SAB 区（主线程写 → 原子推 capsSeq → 推 phase；
// worker 观察到新 phase 后读区必见本帧 caps）。区不够 → false（调用方
// 当帧串行 + 扩容重建）。
function writeCaps(
  p: WorkerPool,
  argsJson: string,
  bytes: Uint8Array[],
  kinds: number[],
): boolean {
  const utf8 = capsEnc().encode(argsJson);
  const textLen = 8 + kinds.length + utf8.length;
  const align8 = (x: number) => (x + 7) & ~7;
  let bytesLen = align8(4 * (bytes.length + 1));
  for (const b of bytes) bytesLen += align8(b.length);
  if (textLen > p.textCap || bytesLen > p.bytesCap) return false;
  const tHead = new Uint32Array(p.sab, p.textOff, 2);
  tHead[0] = kinds.length;
  tHead[1] = utf8.length;
  p.textU8.set(kinds, 8);
  p.textU8.set(utf8, 8 + kinds.length);
  const bHead = new Uint32Array(p.sab, p.bytesOff, bytes.length + 1);
  bHead[0] = bytes.length;
  let off = align8((bytes.length + 1) * 4);
  for (let i = 0; i < bytes.length; i++) {
    bHead[i + 1] = bytes[i].length;
    p.bytesU8.set(bytes[i], off);
    off = align8(off + bytes[i].length);
  }
  return true;
}

// worker 引导（自包含源码，Blob URL 起 realm）：init 载入 bundle（caps
// 不在 init 装——走 board[8] 序号驱动的 loadCaps，与派发同一通道）；
// 空闲 park 在 phase 字上，醒来先按序号刷新 caps，再 work-stealing
// 认领行、算完写进自己的行槽并 rowsDone+1。行段超槽置 overflow 位；
// 行体抛错置 errRow。认领前复查 phase（派发换代即弃权）。
function workerBootstrap(): string {
  return `
try { self.postMessage({ type: 'boot' }); } catch (e) { self.postMessage({ type: 'fail', msg: 'boot: ' + e }); }
self.onmessageerror = () => { self.postMessage({ type: 'fail', msg: 'messageerror' }); };
self.onmessage = (e) => {
  const m = e.data;
  if (!m || m.type !== 'init') return;
  try {
    const TA_KINDS = [Float64Array, Float32Array, Uint8ClampedArray, Uint8Array, Int8Array, Uint16Array, Int16Array, Uint32Array, Int32Array];
    const TA_SIZE = [8, 4, 4, 1, 1, 2, 2, 4, 4];
    function rebuild(v, bytes, kinds) {
      if (typeof v !== 'object' || v === null) return v;
      const d = v.__ocOmpShared;
      if (d !== undefined) {
        const kind = kinds[d.slot];
        const buf = bytes[d.slot];
        if (buf.length !== d.len * TA_SIZE[kind]) throw new Error('shared: byte size mismatch');
        return new TA_KINDS[kind](buf.buffer, buf.byteOffset, d.len);
      }
      if (Array.isArray(v)) return v.map((x) => rebuild(x, bytes, kinds));
      const out = {};
      for (const k of Object.keys(v)) out[k] = rebuild(v[k], bytes, kinds);
      return out;
    }
    const inst = new Function('return ' + m.src + '\\n//# sourceURL=oc-ompw-' + m.key)();
    const board = new Int32Array(m.sab, 0, 16);
    const textU8 = new Uint8Array(m.sab, m.textOff, m.textCap);
    const bytesU8 = new Uint8Array(m.sab, m.bytesOff, m.bytesCap);
    const prng = new Float64Array(m.sab, m.prngOff, m.prngCap);
    const ys = new Float64Array(m.sab, m.ysOff, m.nCap);
    const results = new Float64Array(m.sab, m.resOff, m.nCap * m.rowStride);
    const dec = new TextDecoder();
    const a8 = (x) => (x + 7) & ~7;
    let capsSeq = 0;
    function loadCaps() {
      const t = new Uint32Array(m.sab, m.textOff, 2);
      const kindsLen = t[0], argsLen = t[1];
      const kinds = Array.from(textU8.subarray(8, 8 + kindsLen));
      // TextDecoder.decode 拒绝 SAB 视图（"must not be shared"）——slice
      // 拷出独立 ArrayBuffer 副本再解码。
      const argsText = dec.decode(textU8.subarray(8 + kindsLen, 8 + kindsLen + argsLen).slice());
      const nBytes = new Uint32Array(m.sab, m.bytesOff, 1)[0];
      const head = new Uint32Array(m.sab, m.bytesOff, nBytes + 1);
      const bytes = [];
      let off = a8((nBytes + 1) * 4);
      for (let i = 0; i < nBytes; i++) { bytes.push(bytesU8.subarray(off, off + head[i + 1])); off = a8(off + head[i + 1]); }
      inst.__ocSetCaps(...JSON.parse(argsText).map((a) => rebuild(a, bytes, kinds)));
      capsSeq = Atomics.load(board, 8);
    }
    let seen = Atomics.load(board, 0);
    // 就绪标记走 SAB（board[7]）：worker→main 消息要等主线程真实事件循
    // 环轮转（oracle 渲染循环几乎不 yield），共享内存原子计数立即可见。
    Atomics.add(board, 7, 1);
    // 空闲等待 = futex park（无超时 Atomics.wait——唤醒只来自派发侧的
    // Atomics.notify，不涉定时器/任务队列，虚拟时钟管不到）。判定实验
    // （render-session 探针，2026-11-17）：主线程 sync 段 notify，park
    // 的 worker 同一时间片内醒来跑完应答（spins=0）——真并行；忙等轮
    // 询版空闲烧一核/worker 反拖慢主线程串行帧，已退役。wait 前值检查
    // （第三参 seen）保证 store-then-notify 与 park 竞态不丢唤醒。
    for (;;) {
      try {
        const p0 = Atomics.load(board, 0);
        if (p0 === seen) {
          Atomics.wait(board, 0, seen);
          continue;
        }
        seen = p0;
        if (Atomics.load(board, 8) !== capsSeq) loadCaps();
        const n = Atomics.load(board, 3);
        const stride = Atomics.load(board, 4);
        for (;;) {
          if (Atomics.load(board, 0) !== p0) break;
          const r = Atomics.add(board, 1, 1);
          if (r >= n) break;
          try {
            inst.__ocSetPrng(prng.subarray(r * stride, (r + 1) * stride));
            const copy = inst.__ocRow(ys[r]).slice();
            if (copy.length > m.rowStride) Atomics.store(board, 5, 1);
            else results.set(copy, r * m.rowStride);
          } catch (err) {
            Atomics.store(board, 6, r);
            try { self.postMessage({ type: 'rowerr', r, n, msg: String(err).slice(0, 160) }); } catch {}
          }
          Atomics.add(board, 2, 1);
        }
      } catch (err) {
        try {
          // 错误全文 → text 区尾部预留位（textLen ≤ cap-64K，永不相撞；
          // charCode 直写免 TextEncoder——realm shim 可能裁全局；非原子
          // 写，watchdog 同步读只是诊断，容忍撕裂）。
          const s = String(err).slice(0, 200);
          new Uint32Array(m.sab, m.textOff + m.textCap - 256, 1)[0] = s.length;
          const tail = new Uint8Array(m.sab, m.textOff + m.textCap - 252, 200);
          for (let i = 0; i < s.length; i++) tail[i] = s.charCodeAt(i) & 0xFF;
        } catch {}
        try { self.postMessage({ type: 'fail', msg: 'loop: ' + String(err).slice(0, 160) }); } catch {}
        return;
      }
    }
  } catch (err) {
    self.postMessage({ type: 'fail', msg: String(err) });
  }
};
`;
}

let pool: WorkerPool | null = null;
/// 任一次看门狗/池级失败 → 永久串行（对位 Rust 侧毒化语义）。
let poolPoisoned = false;
/// bundleKey → 首帧串行量测的行段上界（f64 数）。
const rowStrideMeasure = new Map<string, number>();
/// bundleKey → 容量高水位（n/prng 长度/caps 区预算的历史最大值）：prng
/// 采样数与 argsJson 体量逐帧振荡，池按单帧值定容则振荡越顶即 respawn
/// ——kill+串行帧+8 个冷 worker 的风暴把并行收益吃光。预算按高水位
/// 定，振荡在已见范围内不再触发 respawn。
const capHighWater = new Map<string, { n: number; prngLen: number; textNeed: number; bytesNeed: number }>();
/// 非就绪串行宽限帧计数：worker 生命周期由主线程真实 yield 推进（realm
/// shim 把 setTimeout 变成即时 resolve 的假 yield），池就绪需要等真实
/// 宏任务轮转；超过宽限帧数仍 0 就绪 → 毒化串行（防每帧 ping-pong）。
let graceFrames = 0;
/// 行体抛错重建次数（连续超 3 次 → 毒化）。
let errRespawns = 0;

/// MessageChannel 宏任务：本环境唯一可靠的真实 yield（oracle 导出循环
/// 的背压已在用），realm 的 setTimeout shim 是即时 resolve 的假 yield。
function onRealTask(fn: () => void): void {
  const ch = new MessageChannel();
  ch.port1.onmessage = () => {
    ch.port1.close();
    fn();
  };
  ch.port2.postMessage(0);
}

function killPool(p: WorkerPool | null): void {
  if (!p) return;
  for (const w of p.workers) w.terminate();
}

function spawnPool(
  bundleSrc: string,
  bundleKey: string,
  argsJson: string,
  bytes: Uint8Array[],
  kinds: number[],
  n: number,
  rowStride: number,
  prngCap: number,
  textCap: number,
  bytesCap: number,
): WorkerPool | null {
  try {
    // worker 侧空闲等待 = 无超时 Atomics.wait（futex park，notify 直接
    // 唤醒不涉任务/定时器；带超时的 wait 依赖定时器，虚拟时钟下不可用）。
    if (typeof SharedArrayBuffer === 'undefined' || typeof Worker === 'undefined' || typeof Atomics.wait !== 'function') return null;
    const threads = (globalThis as unknown as { __ocOmpThreads?: number }).__ocOmpThreads
      ?? Math.max(2, Math.min(8, (navigator.hardwareConcurrency || 4) - 1));
    const textOff = 64;
    const bytesOff = textOff + textCap;
    const prngOff = bytesOff + bytesCap;
    const ysOff = prngOff + prngCap * 8;
    const resOff = ysOff + n * 8;
    const sab = new SharedArrayBuffer(resOff + n * rowStride * 8);
    const p: WorkerPool = {
      bundleKey,
      workers: [],
      sab,
      board: new Int32Array(sab, 0, 16),
      prng: new Float64Array(sab, prngOff, prngCap),
      ys: new Float64Array(sab, ysOff, n),
      results: new Float64Array(sab, resOff, n * rowStride),
      textU8: new Uint8Array(sab, textOff, textCap),
      bytesU8: new Uint8Array(sab, bytesOff, bytesCap),
      textOff,
      bytesOff,
      prngCap,
      nCap: n,
      rowStride,
      textCap,
      bytesCap,
      phase: 0,
      ready: 0,
    };
    if (!writeCaps(p, argsJson, bytes, kinds)) return null;
    Atomics.store(p.board, 8, 1);
    const url = URL.createObjectURL(new Blob([workerBootstrap()], { type: 'text/javascript' }));
    for (let i = 0; i < threads; i++) {
      const w = new Worker(url);
      w.onmessage = (e) => {
        if (e.data?.type === 'ready') {
          p.ready += 1;
        } else if (e.data?.type === 'rowerr') console.info(`[ompw] rowerr r=${e.data.r}/${e.data.n}: ${e.data.msg}`);
        else if (e.data?.type === 'fail') console.info(`[ompw] worker fail: ${e.data.msg}`);
      };
      w.onerror = (e) => console.info(`[ompw] worker error: ${e.message}`);
      try {
        w.postMessage({
          type: 'init', sab, textOff, bytesOff, prngOff, ysOff, resOff,
          nCap: n, rowStride, prngCap, textCap, bytesCap,
          src: bundleSrc, key: bundleKey,
        });
      } catch (err) {
        console.info(`[ompw] init postMessage threw: ${err}`);
      }
      p.workers.push(w);
    }
    URL.revokeObjectURL(url);
    return p;
  } catch (err) {
    console.info(`[ompw] spawn failed: ${err}`);
    return null;
  }
}

function runSerial(
  inst: OcBundleInst,
  ys: number[],
  prng: Float64Array,
  stride: number,
  measure?: { max: number },
): Float64Array {
  const segs: Float64Array[] = [];
  let total = 0;
  for (let i = 0; i < ys.length; i++) {
    if (stride > 0) {
      inst.__ocSetPrng(prng.subarray(i * stride, (i + 1) * stride));
    }
    // 行段立即拷贝：__ocRow 返回的是 bundle 常驻 out 缓冲的 subarray 视
    // 图，下一次行调用即覆盖。
    const seg = inst.__ocRow(ys[i]).slice();
    if (measure && seg.length > measure.max) measure.max = seg.length;
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

  const measured = rowStrideMeasure.get(bundleKey);
  const budget = capsBudget(argsJson, sharedBytes, sharedKinds);
  // 容量高水位更新（池 spawn/respawn 一律按高水位定容——见 capHighWater 注）。
  const hw = capHighWater.get(bundleKey) ?? { n: 0, prngLen: 0, textNeed: 0, bytesNeed: 0 };
  hw.n = Math.max(hw.n, n);
  hw.prngLen = Math.max(hw.prngLen, prng.length);
  hw.textNeed = Math.max(hw.textNeed, budget.textNeed);
  hw.bytesNeed = Math.max(hw.bytesNeed, budget.bytesNeed);
  capHighWater.set(bundleKey, hw);
  if (!pool || pool.bundleKey !== bundleKey || poolPoisoned) {
    // 键换代/毒化：旧池 terminate（弃引用会漏 worker 线程）。
    if (pool && pool.bundleKey !== bundleKey) killPool(pool);
    pool = null;
  }
  if (measured === undefined) {
    // 首帧串行量测（产出即本帧结果）：行段上界进表；池 spawn 排到真实
    // 宏任务（Worker 生命周期由主线程真实 yield 推进，同步上下文里
    // new Worker + init 消息会被推迟到首个轮转点）。
    const measure = { max: 16 };
    const out = runSerial(inst, ys, prng, stride, measure);
    rowStrideMeasure.set(bundleKey, measure.max);
    onRealTask(() => {
      if (pool || poolPoisoned) return;
      pool = spawnPool(bundleSrc, bundleKey, argsJson, sharedBytes, sharedKinds, hw.n * 2 + 128, measure.max, hw.prngLen * 2 + 65536, hw.textNeed, hw.bytesNeed);
      if (!pool) poolPoisoned = true; // 结构性不可用（无 SAB/Worker）→ 不再重试
      else console.info(`[ompw] pool spawned workers=${pool.workers.length} sab=${pool.sab.byteLength}B rowStride=${pool.rowStride}`);
    });
    return out;
  }

  // 池容量不足（行数/prng/行段上界越顶）→ 杀旧池重建（按高水位定容，
  // 已见振荡不再触发）。text/bytes 不在此查：respawn 判据若用带 ×2+64K
  // 公式的 budget，则与 spawn 定容同式——args 每长一字节必然越顶，respawn
  // 风暴（30 帧 6 次）；writeCaps 用裸长度判界并自带 respawn，args 要长
  // 过 64KB 余量才真扩容。
  if (pool && (pool.nCap < n || pool.prngCap < prng.length || pool.rowStride < measured)) {
    const why = pool.nCap < n ? `n ${n}>${pool.nCap}` : pool.prngCap < prng.length ? `prng ${prng.length}>${pool.prngCap}` : `rowStride ${measured}>${pool.rowStride}`;
    console.info(`[ompw] respawn (pool cap exceeded: ${why})`);
    killPool(pool);
    pool = null;
    onRealTask(() => {
      if (pool || poolPoisoned) return;
      pool = spawnPool(bundleSrc, bundleKey, argsJson, sharedBytes, sharedKinds, hw.n * 2 + 128, measured, hw.prngLen * 2 + 65536, hw.textNeed, hw.bytesNeed);
      if (!pool) poolPoisoned = true;
      else console.info(`[ompw] pool respawned workers=${pool.workers.length} sab=${pool.sab.byteLength}B rowStride=${pool.rowStride}`);
    });
  }

  const p = pool;
  // 就绪门禁走 SAB 计数（board[7]，worker init 完成立即原子 +1）：init
  // 任务投递受浏览器调度时序影响（串行段线程冻结，首个背压窗口才落地）。
  // 就绪后 worker 已进主循环或已 park——派发 notify 即达（park/wake 实
  // 判 spins=0）；旧心跳门（证明在轮询）随 park 化退役。
  const readyN = p ? Atomics.load(p.board, 7) : 0;
  if (!p || readyN < p.workers.length) {
    if (p) {
      graceFrames += 1;
      if (graceFrames > 60) {
        console.info(`[ompw] pool never became ready after 60 frames (ready=${readyN}/${p.workers.length}) → poison serial`);
        killPool(p);
        pool = null;
        poolPoisoned = true;
      }
    }
    return runSerial(inst, ys, prng, stride);
  }

  // 派发：先写 caps 与数据（非原子写），后原子推进 phase（内存模型上
  // store-release：观察到新 phase 的 worker 必见全部数据写）。caps 区
  // 不够装 → 当帧串行 + 杀池扩容重建。
  if (!writeCaps(p, argsJson, sharedBytes, sharedKinds)) {
    console.info(`[ompw] respawn (writeCaps overflow: raw caps size > SAB region)`);
    killPool(p);
    pool = null;
    onRealTask(() => {
      if (pool || poolPoisoned) return;
      pool = spawnPool(bundleSrc, bundleKey, argsJson, sharedBytes, sharedKinds, hw.n * 2 + 128, measured, hw.prngLen * 2 + 65536, hw.textNeed, hw.bytesNeed);
      if (!pool) poolPoisoned = true;
    });
    return runSerial(inst, ys, prng, stride);
  }
  p.prng.set(prng);
  for (let i = 0; i < n; i++) p.ys[i] = ys[i];
  const b = p.board;
  Atomics.store(b, 1, 0);
  Atomics.store(b, 2, 0);
  Atomics.store(b, 5, 0);
  Atomics.store(b, 6, -1);
  Atomics.store(b, 3, n);
  Atomics.store(b, 4, stride);
  p.phase += 1;
  Atomics.store(b, 8, Atomics.load(b, 8) + 1);
  Atomics.store(b, 0, p.phase);
  Atomics.notify(b, 0, Number.POSITIVE_INFINITY);
  if (!p.announced) {
    p.announced = true;
    console.info(`[ompw] first dispatch n=${n} stride=${stride} workers=${p.workers.length} ready=${readyN}`);
  }
  // 主线程不能 Atomics.wait（同步合同）——自旋到 rowsDone==n；看门狗兜
  // worker 死亡（唯一出口：毒化回串行，内容不变）。
  let spins = 0;
  while (Atomics.load(b, 2) !== n) {
    if (++spins > 300_000_000) {
      // worker 池失效（初始化失败/被杀）：毒化回永久串行。面包屑是唯一
      // 可靠的 worker 侧可见性（fail postMessage 在 spin 期间送不出来，
      // terminate 又把待发消息一并丢弃）。
      let eTxt = '';
      try {
        const eLen = Math.min(new Uint32Array(p.sab, p.textOff + p.textCap - 256, 1)[0], 200);
        const tail = new Uint8Array(p.sab, p.textOff + p.textCap - 252, eLen);
        let s = '';
        for (let i = 0; i < eLen; i++) s += String.fromCharCode(tail[i]);
        eTxt = s;
      } catch { /* 诊断读失败不碍降级 */ }
      console.info(`[ompw] spin watchdog (rowsDone=${Atomics.load(b, 2)} n=${n}) msg="${eTxt}" → poison serial`);
      killPool(p);
      pool = null;
      poolPoisoned = true;
      return runSerial(inst, ys, prng, stride);
    }
  }
  const ovf = Atomics.load(b, 5);
  const errRow = Atomics.load(b, 6);
  if (ovf !== 0 || errRow !== -1) {
    console.info(`[ompw] fallback overflow=${ovf} errRow=${errRow} n=${n} rowStride=${p.rowStride}`);
    // 行段超上界：放宽量测（内容演化收敛，重建池按新上界）。
    // 行体抛错：不放宽（放宽救不了错误），重建池给一次机会，连续多次毒化。
    if (ovf !== 0) {
      rowStrideMeasure.set(bundleKey, Math.max(measured, p.rowStride + (p.rowStride >> 1) + 64));
    } else if (++errRespawns > 3) {
      console.info('[ompw] row errors persist after 3 pool rebuilds → poison serial');
      killPool(p);
      pool = null;
      poolPoisoned = true;
      return runSerial(inst, ys, prng, stride);
    }
    killPool(p);
    pool = null;
    onRealTask(() => {
      if (pool || poolPoisoned) return;
      pool = spawnPool(bundleSrc, bundleKey, argsJson, sharedBytes, sharedKinds, hw.n * 2 + 128, rowStrideMeasure.get(bundleKey) as number, hw.prngLen * 2 + 65536, hw.textNeed, hw.bytesNeed);
      if (!pool) poolPoisoned = true;
    });
    return runSerial(inst, ys, prng, stride);
  }
  // 按行序拼装（协议「按行序应用」：槽位即行序，逐段拷出）。
  const rs = p.rowStride;
  let total = 0;
  const lens = new Int32Array(n);
  for (let r = 0; r < n; r++) {
    const nSlots = p.results[r * rs];
    const nOvals = p.results[r * rs + 1];
    const len = 2 + nSlots * 4 + nOvals * 7;
    lens[r] = len;
    total += len;
  }
  const out = new Float64Array(total);
  let off = 0;
  for (let r = 0; r < n; r++) {
    const len = lens[r];
    out.set(p.results.subarray(r * rs, r * rs + len), off);
    off += len;
  }
  return out;
}

export function installOmpRunner(): void {
  const g = globalThis as unknown as Record<string, unknown>;
  if (typeof g.__ocOmpExecute === 'function') return;
  g.__ocOmpExecute = ompExecute;
}
