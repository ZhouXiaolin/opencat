# GLOSSARY

域词汇表。架构讨论（module / interface / depth / seam / adapter / leverage / locality）沿用 codebase-design 词汇；本表收 OpenCat 域词。与 ARCHITECTURE.md 宪法冲突时以宪法为准。

## 协议与数据面

- **DrawOp IR**：core 产出的绘制指令中间表示（`ir/draw_op.rs` 枚举）。core 可以生成字节但永不解释、永不绘制。
- **OCIR**：DrawOp IR 的序列化线协议（magic `OCIR` + 版本 + 分节目录 + 稠密 op 流）。事实源在 `ir/draw_encoding.rs`；TS 常量由 `schema_gen.rs` 生成、`ocir-schema-gen` bin 在 web 构建前自动重写。
- **gpu_stream**：WebGL/WebGPU 命令流位编码协议（OCIR sec13，v9）。录制面 = core 的 `webgl_api.js`/`webgpu_api.js`；消费面 = engine GL/wgpu executor + 浏览器本身（three 直调真 WebGL2，不在仓内）。
- **重放端**：消费 OCIR 把字节变成像素的宿主。仓内两个 adapter：engine（tiny-skia，`render/executor/replay.rs`）与 web（真 canvaskit.wasm，`draw-ir.ts`）。两端是宪法 §4 设计意图的对拍 oracle，不合并。
- **排版例外**：文字 shaping/断行在 core（cosmic-text）出 glyph runs；重放端只光栅化已定位 runs，不重新排版。两端逐位一致的关键。
- **payload 布局**：单个 opcode 后随字节的字段序契约。唯一事实源在 core；不允许 encode/decode/dump 三处手写互抄。

## 角色与机制

- **壳子**：examples 里的标准代码（标准 React 记法 + npm 官方包 import）。opencat 在壳子的合法存在面只有 `import 'opencat/runtime'` + vite 插件配置。
- **sidecar / 模块表**：项目侧预打包的 CJS 模块段（`/*@@oc-module:<spec>@@*/` 分段）随 bundle 供给 core；core 模块表按目标把包名解析到承接面（web 目标解析到官方包）。
- **出口拦截**：拦截点在出口不在面内——gsap=tween 工厂包装（paused 协议）、three=`WebGLRenderer` 出口、canvaskit=API 调用即记录。
- **渲染门**：唯一验收。release 渲染 k3-promo / vfx-heygen-combined / hyperframes-launch 的 engine+web 视频 vs swgl 参考 `tools/k3diff.py` 硬像素比对（avg_p8≤2%）。入口必须 `dist/index.bundle.jsx`。脚本 `scripts/regression-gate.sh`。
- **fail-loud**：未实现的签名调用必须报错指路，不静默降级。新 op 进协议前两端重放器都必须有实现。
- **seek 模式 / frame = f(t)**：每帧是时间 t 的纯函数，隐式传钟，无墙钟无 rAF。
- **退役判据**：自研面退役须同时满足零 JS 消费方 + 官方/唯一路接通 + 渲染逐位回归通过。
