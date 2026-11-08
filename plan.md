# vfx 原始 html 直渲对齐计划

分支 `feat/html5ever-hyperframes`。终极目标：**用 OpenCat 的 core/engine 完美执行 hyperframes 原始 html**。

**总目标（2026-11-06 拍板）：至少四个工程 index.html 直渲 + 各自 swgl 参考 k3diff avg_p8 < 0.02：**
`k3-promo`（已达标 0.0175 全片 html 版）、`vfx-heygen-combined`（本计划主体）、`hyperframes-launch`（hf）、
`liquid-brand-refraction`。

- 渲染输入：`F:/hyperframes-launches/vfx-heygen-combined/index.html`（1617 行，未做任何改动）
- 参考：`F:/hyperframes-launches/vfx-heygen-combined/renders/vfx-heygen-combined_2026-10-05_swgl.mp4`
  （1920×1080@30，788 帧 / 26.267s）
- 门：`python tools/k3diff.py <ref> <our> --out <dir>`，**avg_p8 < 0.02**
- 不写测试；只用 k3diff。`examples/vfx-heygen-combined`（jsx 移植版）不参与本计划，它的分数与本目标无关。

**GL 执行端拍板（2026-11-06）：engine 的 WebGL 命令流执行换 `mozangle`（servo 维护的 ANGLE crate，
Windows D3D11 后端），不再直调桌面 GL。** 动机：PMREM（three r147 半浮点 RT 多 pass 预滤波）在
WGL/NVIDIA 桌面 GL 上驱动层段错误（`gl.create_vertex_array()` 内部崩，OC_GL_TRACE 定位实证）；
ANGLE 严格按 WebGL/GLES 语义校验参数（无效参数返回错误不崩驱动），且语义贴近 swgl 参考实现。

## 一、起点（2026-11-06 实测）

**当前一帧都渲不出。** 原样命令与报错：

```
$ ./target/release/opencat.exe render F:/hyperframes-launches/vfx-heygen-combined/index.html out/x.png 0
Error: html: nested composition (data-composition-src) not supported yet (1 host(s))
```

把 `data-composition-src` 属性临时改名（工程目录内 `_probe.html`，探针）后继续推进，暴露下一层阻塞：

```
Error: failed to download asset from https://cdn.jsdelivr.net/npm/gsap@3.14.2/dist/gsap.min.js
  ... tunnel error: unsuccessful        # 代理抖动，重试后通过
Error: script eval: ReferenceError: Request is not defined
    at mc.load (<anonymous>:1148:499169)
    at GLTFLoader.load (<anonymous>:1790:11)
```

所以当前的阻塞是三层串起来的，不是渲染质量问题：

1. **`data-composition-src` 嵌套未实现** —— `#vfx-intro` 引 `compositions/vfx-text-cursor.html`
   （794 行，`data-start=0`、`data-duration=6.48`，覆盖 0–194 帧，占全片 25%）。
2. **CDN 依赖无离线保障** —— html 头部引 8 个远程脚本：gsap 3.14.2、three r147 及其
   DRACOLoader / GLTFLoader / CopyShader / LuminosityHighPassShader / EffectComposer /
   RenderPass / ShaderPass / UnrealBloomPass。网络抖一次整帧渲染就失败，全片渲染不可复现。
3. **运行时缺 Web 平台面** —— three 的 FileLoader 需要 `Request`/`fetch`（读 `models/iphone.glb`）；
   再往上 `new THREE.WebGLRenderer({canvas})` 需要 `canvas.getContext('webgl'|'webgl2')`。

## 二、目标工程的渲染依赖清单（从 index.html 读出）

| 元素 | 依赖能力 | 现状 |
| --- | --- | --- |
| `#vfx-intro` → `compositions/vfx-text-cursor.html` | 嵌套 composition（子时间轴 + 窗口） | ❌ 硬报错 |
| `<video id="bg-video" src="assets/bg.mp4" data-start="5.58" data-duration="5">` | video 解码 + 帧同步 + 窗口 | ❌ 未接 |
| `<canvas id="theater">` + three r147 WebGLRenderer + EffectComposer/UnrealBloomPass | GL 上下文 + 后处理链 | ❌ html 通道无 GL |
| `<canvas id="chromatic-canvas">` / `#portal-canvas` | GLSL shader（全屏场） | ❌ |
| `<canvas id="chromatic-cap" layoutsubtree>` + 内嵌 `<section>` | canvas 承载 DOM 子树（新 API） | ❌ 未支持 |
| `<img>` ×3（png/svg）、`captures/heygen-mobile/screen-strip.png` | 图片/SVG 光栅 | ✅ 大体可用 |
| 内联 `<svg><path>` 图标 | svg 通道 | ✅ 已有 svg-align 能力 |
| CSS 布局/渐变/滤镜（`filter: blur/drop-shadow`） | html 通道 CSS | ⚠️ 部分（filter 未全） |
| 页面脚本（1000+ 行：时间线、shader、打字、DOM 动画） | JS DOM + MutationStore | ✅ 可用（k3 已验证） |

## 三、里程碑（按阻塞顺序，不按工作量）

### M0 渲出第一帧（解除三层阻塞）
- [ ] **M0.1 嵌套 composition**：宿主 div 按 `data-start/duration/width/height` 挂载子文档，
      子文档的脚本在自己的 realm 里跑（或共享 realm + 时间轴命名空间隔离），子时间轴按
      `t_local = t_host - start` 驱动，窗口外不进渲染。判据：`_probe.html` 恢复原属性后能出图。
- [ ] **M0.2 依赖可复现**：CDN 脚本预取进磁盘缓存，并提供「工程内 vendor 目录优先」的解析顺序；
      判据：断网重渲同一帧成功，且缓存命中时耗时 < 1s。
- [ ] **M0.3 `fetch`/`Request`/`Headers`/`XMLHttpRequest` 最小面**：至少覆盖 three FileLoader 的
      二进制加载路径（本地文件 + 缓存 + http(s)），返回 ArrayBuffer/Blob 语义正确。
- [ ] **M0.4 `canvas.getContext('webgl2'|'webgl')`**：桥到引擎既有 GL 通道（jsx 通道已有 GL stream
      DrawOp），支持 three 用到的子集：buffer/texture/program/uniform/framebuffer/readPixels。
      **这是最大的一块**，先做「能出图」，不追求完整 API。
- 判据：第 0/100/300/700 帧能渲出图 × 无错误 × 与参考同区段目视同构。

### M1 逐场景点亮（每场景独立判据）
按时间顺序，每段先「有 vs 无」，再「对不对」：

- [ ] 0–194 帧：intro（`vfx-text-cursor` 子工程）—— 依赖 M0.1；先对 DOM/文本，再对光线场。
- [ ] 167–317 帧：`bg-video` 素材（5.58s 起 5s）—— 依赖 video 通道。
- [ ] 全片：`#theater` iPhone 场景 —— 依赖 M0.3/M0.4；先对几何/构图，再对光照/色相，
      最后对 bloom（UnrealBloomPass 的参数与权重）。
- [ ] 400–500 帧：chromatic-split 全屏 shader。
- [ ] 尾段：portal shader + CTA。

### M2 结构与低频对齐（每段 p8 收敛）
- [ ] 段级指标表（每 50 帧）作为唯一进度表，每次全片渲染后追加。
- [ ] 先消「整块缺失/位置错」，再消「色带/亮度差」（可用低通 p8 作为结构指标）。

### M3 颗粒与位级
- [ ] intro 的 `fract(sin(gx*489+gy*647+progress*1527.168)*492)` sin-hash 颗粒：
      参考由 SwiftShader(swgl) 渲，我们的 GL 后端浮点近似可能不同。
      **先测同一像素的参考值 vs 我们值**，再决定是「对齐实现」还是「标为已知地板」。

## 四、顺带解决的余下问题（撞到就修，不单独立项）

- [ ] `letterSpacing` 进 canvas mock 的 measureText（k3 遗留；vfx 打字场景同样依赖测量）。
- [ ] `@font-face` / 工程内字体（vfx html 用 `font-family: Inter, system-ui, ...`，系统无 Inter）。
- [ ] `filter: blur()/drop-shadow()`（时间线大量进出场用）。
- [ ] `<canvas layoutsubtree>` 的 DOM 子树承载。
- [ ] 视频通道（`<video>` + data-start/duration + muted + 逐帧 seek）。
- [ ] html 通道的 k3 不退化：全局改动后 k3-promo/engine 必须仍 ≈0.0124（当前 0.0175 全片 html 版）。

## 五、纪律

- 只认 k3diff；全片渲染约 3 分钟、比对约 1 分钟，改完先单帧定位。
- 单帧：`./target/release/opencat.exe render <html> out.png <frame>`
- 任何全局改动（布局/文本/GL/脚本宿主）都跑 `SKIP_BUILD=1 ./scripts/regression-gate.sh` 三例。
- 探针一律落在工程内 `_probe*.html` 或 `OC_*` 环境变量，收尾即删；
  `grep -rn "OC_[A-Z]" crates/` 与工程内 `_probe*` 都必须清零。
- 每完成一个里程碑，把该段典型帧的并排图存 `out/vfx-align/`，指标写进本文底部。

## 六、风险（先摆明）

1. **M0.4 体量最大**：three r147 依赖的 WebGL 面不小（含 framebuffer 链、浮点纹理、readPixels），
   可能要和 jsx 通道的 GL 实现合并，才不至于写两套。
2. **swgl 是软件光栅**：即使 shader 逻辑一致，浮点/精度差异也会让 sin-hash 颗粒、bloom 阈值不同。
   全片 2% 的达成与否很可能卡在 M3，而不是布局。
3. **离线可复现**是硬前置：网络抖动会让「同一份代码渲出不同结果」，这比任何视觉差距都危险。

## 七、进度记录

- 2026-11-06 起点：一帧不出。阻塞①嵌套 composition（硬报错）②CDN 依赖无离线保障
  ③`Request is not defined`（three FileLoader）④html 通道无 GL 上下文。工程内探针 `_probe.html` 已建（收尾删）。
- 2026-11-06 上半场（本会话）：M0 大块已过，进入渲染链排雷。
  - 上会话遗留改动确认可用：M0.1 嵌套 composition（splice + 分段 seek）、属性选择器全量、
    `getContext('webgl')` 桥进 core 录制面、fetch/Blob 资产垫进 html 通道。
  - 修 extract 回传：JS serializeNode 的 `src` 按 http(s) 分流成 `path`/`url`（serde 端 DomElement::Image/Video
    只认这两个字段；静态 seed 的 path/url 原样带回）——img 首个报错由此而来。
  - 缺资产降级（浏览器 onerror 语义）：本地文件缺失 loader 跳过 + 1×1 透明 PNG 占位进缓存注册
    handle，prepare 不再 fail-fast。首个实例：`captures/heygen-mobile/screen-strip.png` 工程缺失
    （HANDOFF.md 列了但目录空）；JS 面被页面自己的 `complete && naturalHeight > screenH` 防御吞掉，
    drawImage 不触发。**strip 段内容差异记 M1 段落债**（参考里手机屏是真实捕获页面）。
  - svg img 链路：imagesize 不认 svg 字节 → core 新增 `svg_raster`（usvg 尺寸 + resvg 光栅成预乘
    RGBA，vendor 依赖，text feature 关）；probe 分流 + `static_image_scaled` 解码分流。`_mini-svg`
    探针验证通过。**挂账：img 无 CSS 尺寸时布局塌 0（ImageMeta 固有尺寸未接布局）**。
  - **vfx index.html f0/60/100/300/700 全部出图（exit 0）**，但内容差距大：f60/f100 全黑
    （intro 子工程在宿主内未渲染——下一段主敌）；f300 红块 vs 参考打字机（参考 f300 仍在 intro，
    时间轴与 data-duration 语义待核）；f700 灰 vs 参考 portal。M1 逐场景点亮从 intro 开始。
  - **回归门（ANGLE 切换后）**：k3-promo/engine **PASS 0.01241（与基线逐位一致）**；web 三例
    ChromeDriver 缺失（环境）。**vfx jsx 28.59→86.3%、hf engine 6.69→10.06% 恶化，未提交**：
    归因实验（GL 5 文件回 HEAD → vfx jsx 回 0.28589 基线）证明恶化全来自 ANGLE 执行端；
    tagAsWebGL2 已限定 html 通道（`__ocHtmlChannel`），限定后 jsx f300 逐位不变——
    **根因在 ANGLE×three r152「WebGL1 判定」命令流路径**（three 场景不渲染，liquid shader
    正常；蓝噪声场占位）。下一批：修 three r152 WebGL1 语义流在 ANGLE GLES3 上的绘制
    （候选：GLSL100 shader 直编的行为差 / OES_VAO 扩展面 / 纹理上传语义）。
    注意 stash 陷阱：`git checkout stash@{0} --` 恢复了过时 WIP（已归零重打补丁）。
  - 探针清单（收尾删）：`_probe.html` `_mini-svg.html` `_mini-gl.html` `_mini-gl2.html`
    `_probe2/3/4.html` `_probeN.html`。
