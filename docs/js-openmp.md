# js-openmp v1：JS 上的 OpenMP 子集规范

> 规范对齐对象：**OpenMP API Version 6.1 Public Comment Draft**（OpenMP TR15，
> 2026-07-09，下称 TR15）。本文定义一个为 JavaScript 设计的 OpenMP 子集：
> **标注面语法严格取自 TR15，零发明**；实现不必覆盖全子集，但每一项的
> 语义必须按 TR15 精确映射。标注在 JS 里是零语义注释——任何引擎
> （V8 / Hermes / quickjs）直跑语料行为不变；语义由 **host**（嵌入 JS
> 引擎的本地运行时，如 opencat 引擎）实现。

## 1. 定位

- js-openmp 的「编译器/运行时」不在 JS 引擎里，而在 **host** 里。JS 引擎
  只负责执行「循环体函数」这类普通 JS 代码。因此子集天然引擎无关：
  V8、Hermes、quickjs 都能作为执行后端，只要 host 能为它实现 §7 的
  worker 适配面。
- 与 asm.js / esbuild `@__PURE__` 同族：注释式标注，浏览器直跑零语义。
- 铁律：**不发明 OpenMP 没有的写法**。JS 特有的工程需求（确定性随机数、
  数组写回）不在标注面解决，走 §6 的实现协议。

## 2. 执行模型（TR15 §1.2 → JS）

TR15：parallel 构造组建 team（primary thread + worker threads），隐式
barrier 收尾，只有 primary 继续执行。

JS 映射：**team = N 个相互隔离的引擎实例**（每实例独占一个宿主线程）。
这一形态是三大引擎的公共最大公约数：

| 引擎 | 「独立实例」 | 官方并发形态 |
|---|---|---|
| V8 | `v8::Isolate` | 任意多 isolate 并行 |
| quickjs | `JSRuntime` | 「多 runtime 可共存、不可交换对象、runtime 内无多线程」（官方 C API 文档） |
| Hermes | `HermesRuntime` | 独立堆实例 |
| 浏览器 | `WorkerGlobalScope` | Web Worker + 结构化克隆 |

**SharedArrayBuffer / Atomics 不进协议**（Hermes 无 SAB、quickjs 真共享
受限）。数据进出全部走消息式 copy-in / copy-out（§6），同构
Web Worker 的结构化克隆语义。

## 3. 标注语法

形态：`// @omp parallel for [clause[ [,] clause] ...]`，独立一行，紧贴
`for` 上一行。子句间空白或可选逗号分隔（TR15 §5.2）。标注哪个 `for`，
就旁路以它为最外层的**整个循环嵌套**（TR15 §6.4.1 canonical loop nest
form：内层可以是任意嵌套的 canonical `for`，循环头之间允许声明）。

### 3.1 constructs

| 构造 | TR15 | 语义 |
|---|---|---|
| `parallel` | §18.1 | 组建 team，块内 SPMD 冗余执行 |
| `parallel for` | §18.1+§19.6.1（combined） | 组建 team 并把迭代划分给 team |
| `for` | §19.6.1 | 在已绑定 region 内划分迭代 |
| `simd` | §18.5 | 迭代可用 SIMD lane 并发执行；每元素结果与串行一致 |
| `parallel for simd` | §25（combined） | 划分 + 向量化 |

v1 实现：`parallel for`（isolate 团队后端）、`parallel for simd` /
`for simd`（std::simd 后端）。`parallel`（非循环 SPMD 块）认识、暂不做。

### 3.2 clauses

| 子句 | TR15 | v1 |
|---|---|---|
| `shared(list)` | §7.3.1 | ✓ |
| `private(list)` | §7.3.2 | ✓ |
| `firstprivate(list)` | §7.3.3 | ✓ |
| `default(shared\|none)` | §7.4 | ✓ |
| `schedule(static\|dynamic[,chunk]\|guided\|auto\|runtime)` | §19.6.3 | ✓ |
| `collapse(n)` | §6.4.5 | ✓（校验 n ≤ 巢深） |
| `num_threads(n)` | §18.1.2 | ✓ |
| `reduction(op:list)` | §8.10 | ✓（op ∈ `+` `*`，f64 面） |
| `lastprivate` `linear` `ordered` `if` `nowait` `allocate` `order` `proc_bind` `copyin` `schedule` 修饰符 | 各章 | 认识但不支持：**整条标注不处理** = 原循环照跑 |
| 未知子句 | — | 同上，绝不猜 |

### 3.3 数据环境语义（TR15 §7 精确映射）

- **shared**：所有线程共享同一对象。JS 消息模型下的实现=每行 copy-in
  副本 + 约束「行间不写共享变量」（写共享数组走 §6.2 写回协议）。
  TR15 允许实现不检查数据竞争（§1.1）：行间写共享标量是 non-conforming
  程序，后果自负。
- **private**：每行未初始化私有副本。JS 映射=行函数首注入
  `let x;`——读未写 = `undefined`，与 TR15 §7.3.2 的未初始化语义一致。
- **firstprivate**：私有副本 + 行起点按外层值拷贝。
- **预定属性**（TR15 §7.1.1，first matching rule）：构造内声明的变量
  private；**循环迭代变量 private**（parallel for 的 affected loop）。
  构造内声明（`let`/`const`）天然行私有，无需标注。
- **default(shared)**（缺省）：未列出的外层自由标识符隐式 shared，
  host 用 AST 语义分析自动并入。
- **default(none)**：每个引用必须显式出现在数据子句中，否则整条不处理。

### 3.4 reduction（TR15 §8.10）

语义：每个 list item 对每行生成私有副本（按 initializer 初始化），行内
照常读写；区域结束由 combiner 把各私有副本合并回原变量。

| op | initializer | combiner |
|---|---|---|
| `+` | `0` | `a + b` |
| `*` | `1` | `a * b` |

JS 映射：行函数首注入 `let acc = <initializer>;`，行函数产出 acc 的行
终值；host 在区域末按**固定行序左折叠**执行 combiner。

> **实现收紧声明**：TR15 允许实现以任意顺序、任意方式组合私有副本
> （浮点不结合 ⇒ 结果依赖合并序）。js-openmp 为满足「同参数两跑逐位
> 一致」（确定性交付物要求），收紧为固定行序左折叠。这是对规范的
> **实现约束**，不是新语法；符合 TR15 §1.5 合规定义（实现必须按
> 本规范语义执行——本规范在此处明确化其顺序）。

### 3.5 schedule（TR15 §19.6.3）

| 形态 | 语义 |
|---|---|
| `static` | 迭代按线程近似等分，禁止窃取（均匀负载最优） |
| `dynamic[,chunk]` | 迭代按 chunk 粒度从共享队列拉取 |
| `guided` | chunk 从大变小自适应 |
| `auto` | 实现自决 |
| `runtime` | 由 `OMP_SCHEDULE` 类环境量决定（v1 = auto） |

调度只影响**负载分布**，不影响结果（产出按行序合并，见 §5）。

## 4. simd 构造语义（TR15 §18.5）

`simd` 允许实现以 SIMD lane 并发执行迭代：每 lane 一个迭代，chunk 内
保留词法前向依赖，**每元素结果与串行执行一致**。

JS 映射（无显式向量语法）：

| 后端 | simd 的实现地位 |
|---|---|
| isolate 团队 | 宽容接受：V8 JIT 自行优化；语义约束（逐元素串行等价）天然满足 |
| std::simd lane kernel | **本质实现面**：直线循环体编译为 `f64x8` lane kernel；直线 if 用 lane select。feature `simd` 门控（`#![feature(portable_simd)]`，仅 nightly；构建 `cargo build --release -p opencat --features opencat-engine/simd`） |
| opcode/SoA、JS 腿 | simd 构造 + feature off 时 simd 分流整体不参与，同 `parallel for` |

### 4.1 lane kernel 实装面（opencat simd_kernel）

- **lane 模型**：lane = 外层迭代，8 个一组打包 `f64x8`；尾批以 NaN 补位
  并用 active 面抑制其副作用；批间 rayon indexed 保序（schedule 子句
  照 §3.5 映射）。内层嵌套 `for` 所有 lane 同步迭代；`if` 用 mask select；
  `break`/`continue` 逐 lane 记 flag（余下迭代对已退出 lane 为 no-op）。
- **白名单**（oxc AST 直读验证）：`let/const`、`if/else`、同形 `for`、
  `break/continue`；算术 `+ - * / %`、比较 `< <= > >= == !=`、
  `&&`/`||`/`!`/`?:`、赋值 `= += -= *= /= %=`；`Math.` 的 `abs min max
  floor ceil round sqrt hypot sin cos exp log atan atan2 pow`；
  `__ocOmpSet/__ocOmpAcc` 数组写。
  **拒绝项**：位运算、`===`/`!==`、页面函数调用（含 PRNG 采集）、数组
  读、reduction、ctx capture、`while`/箭头函数/`new` 等 JS 复杂形态。
  拒绝 ⇒ 回落 isolate（语义等价）。
- **布尔面约定**：比较与 truthy 产出统一归一为 {0.0, 1.0}（mask
  `select(splat(1.0), splat(0.0))`）。`Mask::cast()` 的 true = -1.0，
  会污染 `1 - x`、乘积链等把布尔当数参与的表达式——lane/标量对拍实锤。
- **已知近似**（逐位对拍可能失败的差异点）：
  - `&&`/`||` 双臂都求值（无短路）——臂含数组写时结果可能不同。
  - `Math.min/max` 不分辨 ±0（NaN 传播复刻）。
  - 超越函数逐 lane 调 Rust libm：lane 域与标量域逐位一致（VERIFY
    判据）。与 JS 腿的逐位一致**未经语料验证**——验收语料只含加减
    乘除与比较；含超越函数的循环上线前必须先跑 §9 的双腿对拍。
- **写产出通道**：`__ocOmpSet/__ocOmpAcc` 产出 `(channel, idx, acc, val)`
  四元组，批末收集；idx 为负/非有限/越界时该写静默丢弃（TypedArray
  语义）。数组通道面向 TypedArray；**数组读不在白名单**（跨 lane 读会
  引入批内依赖，lane kernel 不做依赖链）。
- **逐位验证**：`OC_OMP_VERIFY=1` 时每批与标量逐点重算对拍——两边产出
  按 `(channel, idx, acc, val.to_bits())` 排序后逐位比较；任何一点不符
  即 poison 本循环、后续帧落 isolate。f64 加减乘除在 lane 与标量下同为
  IEEE 精确运算 ⇒ 直线算术循环两腿逐位一致（examples/omp-simd 验收）。

## 5. 确定性收紧（实现约束，全后端统一）

TR15 允许实现并发重排（§1.2「并发执行可因数值操作结合变化产生不同
结果」）。js-openmp 的交付物是逐帧像素确定的渲染器，因此在实现层统一
收紧（不进标注面）：

1. **行产出按行序应用**：行完成序无关，产出槽位填 row_index，合并序 =
   行序 ⇒ 字节确定。
2. **PRNG 预采**（§6.1）：循环内随机数在派发前于主实例按原序预采。
3. **reduction 固定序折叠**（§3.4）。
4. **数学逐位**：同一引擎内同源码 ⇒ `Math.*` 逐位一致；跨引擎不保证
   （V8 ieee754.cc 与 quickjs musl 不同源），需要跨引擎逐位时由 host
   注入确定性数学白名单（预留，v1 不做）。

## 6. JS 工程协议（实现协议，不进标注面）

### 6.1 PRNG 预采

循环体内「零参裸调用」（页面 PRNG，如 `random()`）由 host 在派发前用
**页面自己的 PRNG 函数**按消费序预采进 `Float64Array`；worker 行函数内
的调用替换为按行段取预采值。⇒ 行并行确定 + 页面 PRNG 原样跑
（不重实现 PRNG）。行内有状态 PRNG（如 erand48 风格，状态数组是行私有
变量）无需预采——它就是普通 JS 变量，天然行私有。

### 6.2 共享数组写回（Set/Acc）

循环体内对共享数组的下标写在文本级重写为记录调用：

- `c[expr] = v` → `__ocOmpSet(c, expr, v)`
- `c[expr] += v` → `__ocOmpAcc(c, expr, v)`

行内写到行缓冲，行结束产出 `(slot, value)` 对列表，host 按行序应用。
数组本身 **copy-in 零字节**（不进 worker）。数组实参在 bundle 里以令牌
对象传递（令牌带 `__ocOmpArr` 槽号与长度），worker 不持有数组内容。

语义限制（如实声明）：`Acc` = 对**帧初值 0** 的累加——这是 TR15 §8
array-section reduction（initializer=0）的映射；`+=` 之前的帧初值必须
是 0（语料侧 `c.fill(0)` 形态），否则是 non-conforming 使用。同帧同槽
多次 Set/Acc 合法（行缓冲复现任意组合，行末只 flush 每槽终值）。

### 6.3 ctx 垫片（canvas 输出面）

循环体内 canvas 上下文调用由 worker 侧垫片 recorder 记录为数值段
（fillStyle 模板串按 JS 原生语义串化后解析；`|0`、`toFixed` 全部走
JS 原生），行产出为 op 缓冲，host 按行序直推命令流（如
`FillOvalBatch`）。

限制（如实声明）：垫片实例每 worker 各自一份，**ctx 状态（fillStyle
等）不跨行保持**——循环体第 i 行设置的样式不会延续到第 i+1 行；依赖
跨行样式残留的循环是 non-conforming 使用（行并行下本就 data race）。
垫片只实现循环体内高频面（fillStyle/beginPath/arc/fill 及状态五连）；
超面调用在 worker 内抛错 ⇒ 整条毒化落 JS 腿。

### 6.4 快照浮点合同（copy-in 逐位无损）

plain capture 的值走「页面活值 → host 序列化 → worker 解析」往返。
合同：**f64 往返必须逐位无损**（正确舍入）。Rust 侧的硬约束：
`serde_json` 必须启用 `float_roundtrip`——默认 feature 的快速解析算法
对边界值可错 1 ulp（smallpt 实锤：方向向量分量 `...9491` 进 worker 变
`...9493`，只有球缘擦边射线翻转命中 → 稀疏像素分歧，极难归因）。
判定「快照无损」必须对全部 capture 值做逐位（bits）比较，抽点相等
不构成证据。

**TypedArray 不走文本**：plain 树里探到的 TypedArray 由 host
`copy_contents` 一次 memcpy 进 `FrameEnv.shared`（`SharedBuf { kind,
data: Arc<[u8]> }` 快照，逐位无损天然成立）；args_json 该节点只留描述
符 `{"__ocOmpShared": {"slot": k, "len": N}}`（N = 元素数，字节长 =
N × 元素大小）。worker 装载 capture 时按描述符从 `shared[k]` 重建同
类型 TypedArray（每 worker 独立字节克隆 + 独立 backing store——v1 快
照语义，行函数对它的写不出 worker，写回只有 §6.2 的 Set/Acc 通道）。
`__ocOmpShared` 是保留键：页面数据若在 capture 实参树里自带同名键会
与协议冲突（与 `__ocOmpArr` 同一约定）。walker 的其余面按
JSON.stringify 语义复刻（own enumerable、getter 取值、函数值省略、
NaN→null）；toJSON/Date/环等特例整子树回落 JSON 老路（环 → stringify
抛错 → null），回落决策只依赖树形不依赖帧数据，字节确定。

## 7. worker 适配面（引擎无关性的切面）

host 侧协议类型与 trait **不得 import 任何具体引擎类型**；每个引擎
提供一个薄适配层：

```text
trait JsWorkerBackend: spawn_worker(bundle_key, bundle_src) -> Box<dyn JsWorker>
trait JsWorker:        run_row(row, &FrameEnv) -> Result<RowOut>
```

适配要点（quickjs / Hermes 实现者参照）：

1. `eval` bundle 源码一次，得到行函数句柄（bundle 是闭包：capture 绑定
   注入 + 页面函数照抄 + 内层循环嵌套体）。
2. `run_row` 传入行号与 prng 行段；行函数产出 `RowOut`
   `{ slots, ops, red }` 三通道。
3. 数据面 = 结构化克隆（number/string/bool/null/plain array/plain
   object/Float64Array 段），无跨实例共享对象。装载 capture 时把
   args_json 树里的 `__ocOmpShared` 描述符换回 `env.shared[slot]` 的
   TypedArray（§6.4）；不支持 binary 槽的适配层可以直接把描述符当
   plain 对象喂给 worker——但那会读不到数据，等价于标注面未覆盖该
   实参形态，应报错毒化。
4. 引擎内 `Math.*` 逐位一致由「同引擎同源码」保证。

## 8. 降级语义

任何失败（标注超面 / bundle 构造失败 / 行执行异常）⇒ 整条标注毒化 ⇒
原循环照跑，语义等同「pragma 未被编译器处理」（TR15 §1.1：实现只执行
用户显式指定的并行；未被识别的指令是普通注释）。环境开关
`OC_OMP_DISABLE=1` 强制全降级（对拍 JS 腿）。

## 9. 验证协议（每个改动必跑）

1. `OC_OMP_TRACE=1` 确认 took over，无 degraded/poisoned 行。
2. 帧级对拍：native 腿 vs `OC_OMP_DISABLE=1` JS 腿，抽**可见动画帧**
   md5（f0 常被开场遮罩盖住，是假阳性验证帧）。
3. seg 两窗 PNG 直出 `diff -rq`（0:30 与 300:330）。
4. 全片回归 k3diff，p8 不退化。
5. 性能：同机 ABAB 交替 + omp 段 take wall 均值——**跨日 WALL 不可比**。

## 10. 实现索引（opencat）

- `crates/opencat-engine/src/script/omp/transform.rs` — oxc 前端：标注
  定位、ScopeTree capture 分析、守卫/骨架/RowBundle 发射。
- `crates/opencat-engine/src/script/omp/protocol.rs` — 引擎无关协议
  （RowBundle/FrameEnv/RowOut/JsWorker trait）。
- `crates/opencat-engine/src/script/omp/team.rs` — worker 团队调度
  （schedule 映射、按行序收集）。
- `crates/opencat-engine/src/script/omp/v8_team.rs` — V8 isolate 适配层。
- `crates/opencat-engine/src/script/omp/mod.rs` — prep/take 生命周期、
  三栈分流、行产出应用。
- `crates/opencat-engine/src/script/omp/simd_kernel.rs`（feature 门控）—
  std::simd lane kernel 编译器。
- 兜底链：`vm_soa.rs`（SoA 解释批算）→ `vm.rs`（标量 VM）→ JS 腿。
