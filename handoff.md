# handoff — 全官方 GSAP 路线

> 2026-10-21 第 3 轮收工。目标见 [goal.md](./goal.md)：官方 gsap 与全部非交互插件原样跑在
> core 上，自研 `ctx.*` gsap 面整个退役。

## 当前进度

| 阶段 | 状态 |
|---|---|
| 1. Bundle 重建 | 完成（15/15 插件原样收录，官方源码零改动） |
| 2. 统一 example + Chrome 截帧管线 | 完成 |
| 3. 宿主最小子集 | 完成 |
| 4. 逐插件打通 | **14/15**：eases 7、物理 2、DOM 依赖 5；剩 SplitText、Flip |
| 5. 退役 + 迁移 | 未开始 |

统一 example 全片 360 帧无损比对：**ma_e 0.327 / avg_p8 0.764%**。
k3-promo 对真参考回归：**avg_p8 0.177%**（改前 0.736%，本轮的三处修复生效）。

## 本轮完成

### 五个 DOM 插件在统一 example 里跑通

DrawSVG、MorphSVG、MotionPath、ScrambleText、Text。每个都靠宿主面取到真元素数据：
`getAttribute('d')`、`getBBox()`、`getTotalLength()`、`textContent`、`element.style.*`。

逐个用写值探针确认过插件真的在驱动目标（不是「看起来对」）：

| 插件 | 探针看到的写入 |
|---|---|
| DrawSVG | `strokeDasharray = "303.23px,606.56px"` |
| MorphSVG | `d = "M60 300 C112.77 91.66 347.22 91.66 400 300 …"`（插值中） |
| MotionPath | `transform = "translate3d(502.02px, -41.52px, 0px)"` |
| ScrambleText | `text = "DECODEXXXXXXXXX"` |
| Text | `text` 写入生效 |

### 宿主面补的真缺口：`textContent` 读不到初值

`readText` 只查当前帧的写入，没写过就返回 `''`。ScrambleText 按**目标文本长度**
分配字符预算，读到空串就一个字符都不揭示——画面上只有 8 个字符，Chrome 有 15 个。

修法：读过 store 之后回落 `__text_source_get(id)`，也就是 JSX 里声明的文本。

### 验收管线改无损（本轮最关键的发现）

之前用 `-crf 18` 两侧各编一次再跑 k3diff。实测：**同一张 PNG 编解码一次的偏差
就有 ma_e ≈ 2.97**，比插件差异大一个数量级——k3diff 量到的主要是编码器差异，
不是渲染差异。

改为 PNG 序列逐帧比，或 `-qp 0 -pix_fmt yuv444p` 全无损编码。同一帧的读数因此
从 mae 1.69 降到 0.33。`tools/gsap-oracle.mjs` 收尾提示已改成无损命令。

### 两侧统一字体与行高

Chrome 的 `sans-serif` 落到系统 UI 字体、`line-height: normal` 取字体自身度量
（Inter ≈ 1.21）；opencat 是内置 Noto Sans SC + 行高 1.5。钉住之前，字宽差 5px、
基线差 5px（34px 字号），这部分差异与插件毫无关系。

- `examples/gsap-plugins/index.jsx` 新增 `export const fonts`（Inter，两个 weight）
- oracle 页用 `@font-face` 把**同一份字节**绑成 `oracle-sans`
- 两侧都写死 `line-height: 1.5`
- 注意 `font:` 简写会把 line-height 重置成 `normal`，字号与字体族必须分开写

对齐后：字宽 783/784px、基线误差 ≤1px。

### ScrambleText 用单字符集

插件的字符表由 `Math.random` 洗出（`CharSet` 用随机索引取字），两侧同一份
`chars: 'upperCase'` 也必然得到不同乱码。改用 `chars: 'X'`：揭示进度仍逐帧可见，
而两侧逐字节相同。与 RoughEase 用 `randomize:false` 是同一条纪律。

## 剩余差异的构成

按区域分解（360 帧平均，>16 的像素占比）：

| 区域 | p16 | 原因 |
|---|---|---|
| 标题 / 副标题 | 17.9% / 12.2% | 字形边缘浓度（两侧抗锯齿权重不同） |
| ScrambleText | 5.8% | 同上 |
| 其它文字 | 2–7% | 同上 |
| **非文字区** | **0.30%** | 曲线抗锯齿、圆角矩形的 AA |

即：几何与动画数值已对齐，残差集中在文字栅格化。k3-promo 对真参考是 0.736%，
本轮 0.764%，同量级。

## 下一轮

### 1. SplitText 与 Flip（阶段 4 收尾）

两者都是**构造器形态**，不是 tween 插件：

```js
var split = new SplitText('#title', { type: 'chars,words' });
gsap.to(split.chars, { y: -20, stagger: 0.02 });
```

拆分要建真节点树（`createElement` + `appendChild` + `insertBefore`），而当前
`dom_api.js` 里 `appendChild` 等会明确抛错。前置是把 `createElement/appendChild`
通道做实：recorder store 插节点 + 每帧 walk 拾取（goal.md §6 阶段 3 里列的那条，
本轮只做了 scratch element，没做真建树）。

Flip 还要 `getBoundingClientRect` 在布局变化前后给出不同值——同样依赖真建树。

### 2. 阶段 5 退役

- 删 `ctx.timeline/to/from/fromTo/set`（`animation/facade.js:88-399`）
- 删五个原生插件 `animation/plugins/{split_text,morph_svg,motion_path,scramble_text,text}.js`
- 删 `bezier:`/`points:` ease 标签 + Rust `Easing::Samples`（`parse/easing.rs`）
- 11 个 ctx.* 示例迁真包写法，逐个过 k3diff
- grep 检查无退役面残留

## 命令速查

```bash
# 重建 bundle（改 entry.js 后必跑）
bun scripts/build-gsap-bundle.mjs

# 渲染
cargo build --release --bin opencat
./target/release/opencat.exe examples/gsap-plugins/index.jsx out/gsap-plugins.mp4
# 单帧 PNG
./target/release/opencat.exe render examples/gsap-plugins/index.jsx out/f250.png 250

# PNG 序列（无损比对用）
cargo build --release --bin opencat-frames
./target/release/opencat-frames.exe examples/gsap-plugins/index.jsx out/gsap-plugins-frames

# Chrome 参考帧
bun tools/gsap-oracle.mjs --all --fps 30      # -> out/gsap-oracle/f0000.png…
bun tools/gsap-oracle.mjs --frames 250

# 无损比对（必须无损，否则量到的是编码器差异）
FF=/c/ffmpeg-9.0.2/ffmpeg-9.0.2-full_build-shared/bin/ffmpeg.exe
$FF -y -framerate 30 -i out/gsap-oracle/f%04d.png -c:v libx264 -qp 0 -pix_fmt yuv444p out/gsap-oracle-ll.mp4
$FF -y -framerate 30 -i out/gsap-plugins-frames/frame_%04d.png -c:v libx264 -qp 0 -pix_fmt yuv444p out/gsap-plugins-ll.mp4
python3 tools/k3diff.py out/gsap-oracle-ll.mp4 out/gsap-plugins-ll.mp4 --out out/gsap-plugin-diff-ll

# 同一帧色块中心位置对比
python3 tools/gsap-oracle-measure.py out/f250.png out/gsap-oracle/f0250.png
```

## 坑记录

- **k3-promo 必须用 `--base-dir`**：bundle 在 `dist/`，资产/字体相对路径按入口目录解析。
- **本机没有 node**，只有 bun 1.4.2。esbuild 从 `examples/k3-promo/node_modules` 取。
- **JSX 元素白名单**：div/text/canvas/image/audio/video/transition/tl/path/lottie/svg，
  没有 `<group>`，容器用 `div`。
- **裸写的 JSX 文本带缩进**：`<text>\n  HELLO\n</text>` 会给子节点带上换行与空格。
  用 `{'HELLO'}` 包住。
- **`gsap.fromTo(t, from, to)` 没有 position 参数**：要定位就放进 timeline。
- **realm 里 `console.log` 不出 stdout**：调试用 `throw new Error('PROBE ...')`。
- **`ffmpeg` 新版没有 `-vsync`**：用 `-fps_mode passthrough`。
- **Python 读不到 `/tmp`**：Windows 临时目录是 `C:/Users/zhoux/AppData/Local/Temp/`。
- **oracle 页里模板字符串内的反引号**：注释里写 `` `normal` `` 会截断 JS 模板字符串，
  直接报 `Expected ";"`。
- **示例字体相对路径**按入口文件目录算：`examples/gsap-plugins/` 到 `assets/` 要写
  `../../assets/...`。
