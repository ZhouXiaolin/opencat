# 渲染核对（分支改动后）

**main 上的 engine 输出是唯一视觉基线。**

| 你改了什么 | 和谁对齐 | 命令入口 |
| --- | --- | --- |
| core / engine | 分支 engine **整段 MP4** vs main engine **整段 MP4** | `scripts/compare-ssim.sh` |
| web | 分支 web **采样帧** vs main 视频对应帧（raw RGBA） | `scripts/compare-mp4.sh` |

默认片：`examples/profile-showcase.jsonl`。

路径约定（本机）：

- main checkout：`/home/solaren/Projects/opencat`
- 功能分支 worktree：`/home/solaren/Projects/opencat-issue-2`

---

## 0. 一次性环境

```bash
export SKIA_BINARIES_URL=file:///tmp/skia-binaries.tar.gz
export CHROME_BIN=/usr/bin/google-chrome
# 可选：CHROMEDRIVER_BIN / CHROMEDRIVER_URL
```

依赖：

- `cargo`、`ffmpeg`、`ffprobe`
- 改 web 时：`chromedriver` + Chrome
- 改 web 时：facade 与 CanvasKit

```bash
# 仅 web 核对需要
cd /home/solaren/Projects/opencat-issue-2
(cd crates/opencat-web/web && bun install && bun run build)
(cd web && bun install)
```

profile-showcase 若拉远程资源，两边都要能访问同一套资源（常见：`127.0.0.1:8080`）。

---

## A. 改了 engine / core → 整段对齐 main

**目标：** 分支 `opencat` 渲出的 `profile-showcase.mp4` 与 main 完全一致。  
**通过：** 逐帧 SSIM `min = avg = max = 1.000000`。

### 启动 / 执行

```bash
# ① main：先渲 reference（必须由当前 main commit 生成）
cd /home/solaren/Projects/opencat
export SKIA_BINARIES_URL=file:///tmp/skia-binaries.tar.gz
cargo run --release --features profile -- examples/profile-showcase.jsonl
# 产出：out/profile-showcase.mp4

# ② 仍从 main checkout 跑脚本（脚本默认对比 worktree opencat-issue-2）
./scripts/compare-ssim.sh examples/profile-showcase.jsonl
```

脚本会：在分支 worktree release 构建 → 渲同一 example → FFmpeg 逐帧 SSIM。

### 看结果

- 终端：`SSIM min/max/avg`
- 报告：`out/compare-profile-showcase/ssim_stats.txt`、`ssim_report.txt`

### 注意

- 从 **main checkout** 调用 `compare-ssim.sh`，不要从 worktree 误跑导致 `MAIN_DIR` 指错。
- reference 过期就在 main 重渲。
- 只比视频像素，不比音频。

---

## B. 改了 web → 采样对齐 main 视频

**目标：** 分支 web（inspect ChromeDriver + `web/test-oracle.html`）在时间轴上的采样帧，与 main/engine 基线帧一致。  
**通过：** 默认每 **0.5s** 一帧，`SSIM ≥ 0.99`（含视频解码差的帧可用 `0.97`）。

前提：同一 example 的 **engine 已对 main 整段对齐（A 通过）**，或确认本分支未改 engine。  
否则 web 差异可能来自分支 engine 漂移。

### 为什么是采样而不是整段 web MP4

- Web 像素合同是 raw RGBA（CanvasKit `readPixels`），不是 WebAV 重编码 MP4。
- 唯一浏览器驱动：`opencat_engine::inspect::browser`（与单帧 oracle 同一套）。
- 0.5s 采样覆盖时序/资源/转场，成本可接受。

### 启动 / 执行

```bash
cd /home/solaren/Projects/opencat-issue-2
export SKIA_BINARIES_URL=file:///tmp/skia-binaries.tar.gz
export CHROME_BIN=/usr/bin/google-chrome

# facade + CanvasKit（若尚未构建）
(cd crates/opencat-web/web && bun install && bun run build)
(cd web && bun install)

# 默认：profile-showcase，每 0.5s 采样
./scripts/compare-mp4.sh examples/profile-showcase.jsonl

# 快速冒烟（只采 N 帧）
INTERVAL_SECS=0.5 MAX_SAMPLES=10 ./scripts/compare-mp4.sh examples/profile-showcase.jsonl
```

等价直接调 CLI：

```bash
cargo build --bin opencat-web-compare --release
./target/release/opencat-web-compare examples/profile-showcase.jsonl \
  --out-dir out/compare-mp4-profile-showcase \
  --interval-secs 0.5
```

### 看结果

- 终端：每帧 `[OK]/[FAIL]` 与汇总
- `out/compare-mp4-profile-showcase/summary.txt`
- `out/compare-mp4-profile-showcase/ssim_samples.csv`
- 失败帧：`out/compare-mp4-profile-showcase/frame-NNNN/{engine,web,diff}.png`

### 环境变量

| 变量 | 含义 | 默认 |
| --- | --- | --- |
| `INTERVAL_SECS` | 采样间隔（秒） | `0.5` |
| `MAX_SAMPLES` | 最多采样帧数 | 不限制 |
| `MIN_SSIM` | 严格阈值 | `0.99` |
| `VIDEO_MIN_SSIM` | 视频帧容差 | `0.97` |
| `SAVE_ALL=1` | 每帧都存 PNG | 关 |
| `SKIP_BUILD=1` | 不重建 CLI | 关 |
| `CHROME_BIN` / `CHROMEDRIVER_BIN` / `CHROMEDRIVER_URL` | 浏览器 | 自动探测 |

### 单帧定点（可选）

```bash
cargo test -p opencat-engine --lib chromedriver_profile_showcase_frame_matches_engine \
  -- --ignored --nocapture
```

---

## 两边都改了

```text
A. compare-ssim.sh  → 分支 engine 对 main = 1.000000
        ↓
B. compare-mp4.sh   → 分支 web 0.5s 采样对基线帧全绿
```

先 A 再 B，不要同时解释两条链路。

---

## 工具与代码位置

| 路径 | 作用 |
| --- | --- |
| `scripts/compare-ssim.sh` | main vs 分支 **engine 整段 MP4** |
| `scripts/compare-mp4.sh` | **web 0.5s 采样** vs engine 帧 |
| `crates/opencat/src/bin/opencat-web-compare.rs` | 采样 CLI |
| `crates/opencat-engine/src/inspect/browser.rs` | ChromeDriver + 静态服务（唯一浏览器 harness） |
| `crates/opencat-engine/src/inspect/web_frame_oracle_tests.rs` | 单帧 ignore 测试 |
| `web/test-oracle.html` | web 像素页 |

---

## 清单

**Engine 改动**

- [ ] main 重渲 `examples/profile-showcase.jsonl`
- [ ] `./scripts/compare-ssim.sh examples/profile-showcase.jsonl` → **1.000000**

**Web 改动**

- [ ] engine 对 main 仍为 1（或未动 engine）
- [ ] facade `bun run build` + `web` 依赖就绪
- [ ] `./scripts/compare-mp4.sh examples/profile-showcase.jsonl` 采样全绿
