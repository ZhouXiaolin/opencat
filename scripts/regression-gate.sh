#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# OpenCat 验收门 —— 唯一标准（2026-10-28 拍板）
#
# release 模式渲染标准示例的 engine 与 web 视频，与原始 swgl 参考做
# tools/k3diff.py 硬像素比对（mae / maxd / p8，非 SSIM）。没有其他测试。
#
# 每个示例两路比对：
#   engine（全片）    vs swgl 参考
#   web（排除窗后）   vs 裁剪到同一窗口集的 swgl 参考
#
# 门限（swgl 参考的既定验收线）：avg_p8 <= 0.02（2%）。mae 聚合值只记录
# 不设门——swgl 底噪（结构噪声 + 参考自重编码 ~0.7%）使每帧 mae 门无意义。
#
# Usage:
#   ./scripts/regression-gate.sh
#   EXAMPLES="k3-promo" ./scripts/regression-gate.sh          # 单例
#
# Env:
#   REF_ROOT    原始参考工程根（默认 F:/hyperframes-launches）。
#               参考 = $REF_ROOT/<example>/renders/<example>_*_swgl.mp4
#   EXAMPLES    空格分隔示例列表（默认 k3-promo vfx-heygen-combined
#               inspector-launch）
#   MAX_FRAC    avg_p8 门限（默认 0.02）
#   SKIP_BUILD=1            复用现有 release 二进制
#   OPENCAT_BIN             覆盖自动解析的 release opencat 路径
#   WEB_ONLY=1              跳过 engine 腿（复用现有 engine 产物，只跑 web 腿）
#   ENGINE_MAX_S_PER_FRAME  engine 时间门（默认 0.5 s/帧；Vulkan 地板 ~0.13）
#   WEB_MAX_S_PER_FRAME     web 时间门（默认 1.5 s/帧；soft 地板 ~0.6-0.9）
#   TIME_SLACK_S            两腿时间预算的固定余量（默认 120s）
#   CHROME_BIN / CHROMEDRIVER_BIN / CHROMEDRIVER_URL   web 渲染需要
#   EXCLUDE_<例名去连字符大写>   web 侧排除窗（如 EXCLUDE_HYPERFRAMESLAUNCH）。
#               默认 hyperframes-launch=213-549,866-905,907-975（Chrome
#               WebCodecs 解不了 HEVC 源素材的平台排除窗）。
#
# 前置：
#   - cargo / bun / python / ffmpeg 在 PATH
#   - web facade：(cd crates/opencat-web/web && bun install && bun run build)
#   - 示例 HTML：直接取 $REF_ROOT/<n>/index.html（不再构建 JSX bundle）
# ---------------------------------------------------------------------------
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

REF_ROOT="${REF_ROOT:-/home/solaren/Projects/hyperframes-launches}"
EXAMPLES=${EXAMPLES:-"k3-promo vfx-heygen-combined inspector-launch"}
MAX_FRAC="${MAX_FRAC:-0.02}"
# hyperframes-launch：Chrome WebCodecs 解不了 HEVC 源素材（beat4-clip.mp4 /
# pill.mp4 / Frame 1&3.mp4），web 侧排除这些窗口；engine 侧全片。
: "${EXCLUDE_HYPERFRAMESLAUNCH:=213-549,866-905,907-975}"
OUT_BASE="$ROOT/out/gate"

# 时间门禁（2026-11-19）：每腿按 s/帧 预算，到点 SIGTERM（30s 宽限后
# SIGKILL）判 FAIL——渲染正确但环境劣化（GPU 进程崩溃掉 CPU fallback、
# 误配 LD_PRELOAD 等）先在时间上暴露。地板实测：engine Vulkan 硬件
# inspector 1350 帧 ≈3 min（用户实测）≈0.13 s/帧，门 0.5（≈3.7×）；web
# SwiftShader soft ~0.6-0.9 s/帧，门 1.5。TIME_SLACK_S 盖进程启动/收尾。
ENGINE_MAX_S_PER_FRAME="${ENGINE_MAX_S_PER_FRAME:-0.5}"
WEB_MAX_S_PER_FRAME="${WEB_MAX_S_PER_FRAME:-1.5}"
TIME_SLACK_S="${TIME_SLACK_S:-120}"
# 1 = 跳过 engine 腿（复用 $OUT_BASE/$n-engine.mp4，只重跑 web 腿）。
WEB_ONLY="${WEB_ONLY:-0}"

# Linux engine 腿环境缺省（与 scripts/walksheet.sh 同一组；调用方已设则以
# 调用方为准）。web 腿不继承这组（见 web 渲染处）。
if [ "$(uname)" = "Linux" ] && [ -z "${OC_ANGLE_DIR:-}" ]; then
    export OC_ANGLE_DIR="$ROOT/tools/angle-linux"
    export LD_PRELOAD="$ROOT/tools/angle-linux/libvulkan.so.1"
    export VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/radeon_icd.json
    export OC_GPU_PIPELINE=1
fi

EXE=opencat
[ -f "$ROOT/target/release/opencat.exe" ] && EXE=opencat.exe
WEB_EXE=opencat-web-render
[ -f "$ROOT/target/release/opencat-web-render.exe" ] && WEB_EXE=opencat-web-render.exe

if [ ! -f "$ROOT/crates/opencat-web/web/dist/opencat.js" ]; then
    echo "Error: web facade missing at crates/opencat-web/web/dist/opencat.js" >&2
    echo "Build: (cd crates/opencat-web/web && bun install && bun run build)" >&2
    exit 1
fi

PY="$(command -v python || command -v python3)"
if [ -z "$PY" ]; then
    echo "Error: python not found on PATH" >&2
    exit 1
fi

if [ "${SKIP_BUILD:-0}" != "1" ]; then
    echo "--- Build (release) ---"
    cargo build --release --bin opencat --bin opencat-web-render
    echo ""
fi
export OPENCAT_BIN="${OPENCAT_BIN:-$ROOT/target/release/$EXE}"

# 原始 hyperframes HTML 是唯一入口；JSX/Remotion 产物先编译成 dist HTML。
find_entry() {
    local dir="$1"
    if [ -f "$dir/index.html" ]; then
        echo "$dir/index.html"
    fi
}

# ffmpeg select 表达式：排除窗之外的帧，`a-b,c-d` →
# `not(between(n,a,b)+between(n,c,d))`。
select_expr() {
    local parts=()
    local r a b
    IFS=',' read -ra rs <<< "$1"
    for r in "${rs[@]}"; do
        a="${r%%-*}"
        b="${r##*-}"
        parts+=("between(n,$a,$b)")
    done
    local joined
    joined="$(IFS=+; echo "${parts[*]}")"
    echo "not($joined)"
}

# 时间门禁判定（s/帧）：打印实测耗时并判门，返回非 0 = 超门。
time_gate() {
    local n="$1" side="$2" t0="$3" video="$4" max_spf="$5"
    local wall frames spf ok
    wall=$(( $(date +%s) - t0 ))
    frames=$(ffprobe -v error -show_entries stream=nb_frames -of csv=p=0 "$video" 2>/dev/null | head -1 | tr -d '[:space:]' || true)
    case "$frames" in ''|*[!0-9]*) frames="$FRAMES_EST" ;; esac
    [ "$frames" -ge 1 ] 2>/dev/null || frames=1
    spf=$(awk -v w="$wall" -v n="$frames" 'BEGIN{printf "%.2f", w/n}')
    ok=$(awk -v w="$wall" -v n="$frames" -v m="$max_spf" 'BEGIN{print (w/n <= m) ? 1 : 0}')
    echo "  [$n/$side-time] wall=${wall}s frames=$frames -> ${spf}s/frame (gate <=${max_spf}) -> $([ "$ok" = 1 ] && echo PASS || echo FAIL)"
    [ "$ok" = 1 ]
}

PASS=0
FAIL=0
FAILED_LABELS=()

mkdir -p "$OUT_BASE"

for n in $EXAMPLES; do
    echo "============================================"
    echo "  $n"
    echo "============================================"

    ENGINE_VIDEO="$OUT_BASE/$n-engine.mp4"
    WEB_VIDEO="$OUT_BASE/$n-web.mp4"
    ENTRY="$REF_ROOT/$n/index.html"

    REF_VIDEO="$(ls -1 "$REF_ROOT/$n/renders/"*swgl*.mp4 2>/dev/null | sort | tail -1 || true)"
    if [ -z "$REF_VIDEO" ]; then
        echo "[FAIL] $n: swgl reference not found under $REF_ROOT/$n/renders/" >&2
        FAIL=$((FAIL+1)); FAILED_LABELS+=("$n(reference)")
        continue
    fi
    echo "  reference: $REF_VIDEO"
    ENGINE_OK=1
    WEB_OK=1

    # 帧数预算 = 参考时长 ×30fps（语料固定 30fps）；两腿时间门禁共用。
    FRAMES_EST=$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$REF_VIDEO" 2>/dev/null \
        | awk '{printf "%d", ($1 * 30) + 0.5}' || true)
    [ "$FRAMES_EST" -ge 1 ] 2>/dev/null || FRAMES_EST=1

    if [ "$WEB_ONLY" = "1" ]; then
        echo "  [web-only] engine leg skipped (reuse $ENGINE_VIDEO)"
    else
    # 1) engine 直接渲染原始 HTML/dist 文档。
    if [ ! -f "$ENTRY" ]; then
        echo "[FAIL] $n: HTML entry missing at $ENTRY" >&2
        FAIL=$((FAIL+1)); FAILED_LABELS+=("$n(entry)")
        continue
    fi
    ENGINE_T0=$(date +%s)
    ENGINE_BUDGET_S=$(awk -v f="$FRAMES_EST" -v m="$ENGINE_MAX_S_PER_FRAME" -v s="$TIME_SLACK_S" 'BEGIN{printf "%d", f*m+s}')
    if ! timeout --kill-after=30 "$ENGINE_BUDGET_S" "$ROOT/target/release/$EXE" render "$ENTRY" "$ENGINE_VIDEO" --base-dir "$REF_ROOT/$n"; then
        echo "[FAIL] $n: engine HTML render failed (time budget ${ENGINE_BUDGET_S}s)" >&2
        FAIL=$((FAIL+1)); FAILED_LABELS+=("$n(engine-render)")
        continue
    fi

    # 2) engine vs swgl（全片）。
    ENGINE_DIFF="$OUT_BASE/k3diff-$n-engine"
    if ! "$PY" tools/k3diff.py "$REF_VIDEO" "$ENGINE_VIDEO" --out "$ENGINE_DIFF"; then
        echo "[FAIL] $n/engine: k3diff run error" >&2
        FAIL=$((FAIL+1)); FAILED_LABELS+=("$n(engine-k3diff)")
        continue
    fi
    if MAX_FRAC="$MAX_FRAC" "$PY" - "$ENGINE_DIFF/summary.json" engine <<'PYEOF'
import json, os, sys
d = json.load(open(sys.argv[1]))
side, max_frac = sys.argv[2], float(os.environ["MAX_FRAC"])
agg, rows = d["agg"], d["rows"]
ok = agg["avg_p8"] <= max_frac
print(f"  [{side}] agg mae_mean={agg['mae_mean']:.4f} avg_p8={agg['avg_p8']:.5f} "
      f"| frames={len(rows)} | gate avg_p8<={max_frac} -> {'PASS' if ok else 'FAIL'}")
for r in sorted(rows, key=lambda r: (-r["p8"], -r["mae"]))[:10]:
    print(f"    worst frame {r['frame']}: mae={r['mae']:.3f} p8={r['p8']:.5f}")
sys.exit(0 if ok else 1)
PYEOF
    then
        echo "[PASS] $n/engine"
        ENGINE_OK=1
    else
        echo "[FAIL] $n/engine: k3diff gate (avg_p8<=$MAX_FRAC)"
        ENGINE_OK=0
    fi
    time_gate "$n" engine "$ENGINE_T0" "$ENGINE_VIDEO" "$ENGINE_MAX_S_PER_FRAME" || ENGINE_OK=0
    fi # WEB_ONLY

    # 3) web vs swgl。有排除窗时：web 分段渲染后拼接，参考裁剪到同一窗口集。
    EXCLUDE_VAR="EXCLUDE_$(echo "$n" | tr -d '-' | tr '[:lower:]' '[:upper:]')"
    EXCLUDE="${!EXCLUDE_VAR:-}"
    if [ -n "$EXCLUDE" ]; then
        echo "  web exclude windows: $EXCLUDE"
        REF_CMP="$OUT_BASE/$n-ref-cut.mp4"
        SEL="$(select_expr "$EXCLUDE")"
        # setpts 重置时间轴：select 丢帧后原 PTS 不再单调，顺序解码会输出
        # 错位帧序（k3diff 全错的根因）。crf 0 重编码保像素。
        if ! ffmpeg -v error -y -i "$REF_VIDEO" \
                -vf "select='$SEL',setpts=N/FRAME_RATE/TB" -r 30 -fps_mode cfr \
                -c:v libx264 -crf 0 -pix_fmt yuv420p \
                -color_range tv -colorspace bt709 -color_primaries bt709 -color_trc bt709 \
                "$REF_CMP"; then
            echo "[FAIL] $n/web: reference window cut failed" >&2
            FAIL=$((FAIL+1)); FAILED_LABELS+=("$n(ref-cut)")
            continue
        fi
    else
        REF_CMP="$REF_VIDEO"
    fi

    echo "  entry: $ENTRY"
    RENDER_ARGS=( "$ENTRY" --out "$WEB_VIDEO" --base-dir "$REF_ROOT/$n" )
    if [ -n "$EXCLUDE" ]; then
        RENDER_ARGS+=(--exclude "$EXCLUDE")
    fi
    # web 腿环境卫生（webdriver.rs 实测有效组合，勿回退）：engine 腿的
    # LD_PRELOAD 旧 libvulkan 进 Chrome GPU 进程 SIGTRAP×3 → 掉 CPU 光栅
    # fallback（~2s/帧）甚至 WebGL2 上下文创建失败（vfx 2026-11-19 实证）；
    # ICD 必须指系统 RADV。OC_ORACLE_HW_GL=1 照常透传（性能腿）。
    WEB_T0=$(date +%s)
    WEB_BUDGET_S=$(awk -v f="$FRAMES_EST" -v m="$WEB_MAX_S_PER_FRAME" -v s="$TIME_SLACK_S" 'BEGIN{printf "%d", f*m+s}')
    if ! env -u LD_PRELOAD VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/radeon_icd.json \
            timeout --kill-after=30 "$WEB_BUDGET_S" "$ROOT/target/release/$WEB_EXE" "${RENDER_ARGS[@]}"; then
        echo "[FAIL] $n: web render failed (time budget ${WEB_BUDGET_S}s)" >&2
        FAIL=$((FAIL+1)); FAILED_LABELS+=("$n(web-render)")
        continue
    fi

    WEB_DIFF="$OUT_BASE/k3diff-$n-web"
    if ! "$PY" tools/k3diff.py "$REF_CMP" "$WEB_VIDEO" --out "$WEB_DIFF"; then
        echo "[FAIL] $n/web: k3diff run error" >&2
        FAIL=$((FAIL+1)); FAILED_LABELS+=("$n(web-k3diff)")
        continue
    fi
    if MAX_FRAC="$MAX_FRAC" "$PY" - "$WEB_DIFF/summary.json" web <<'PYEOF'
import json, os, sys
d = json.load(open(sys.argv[1]))
side, max_frac = sys.argv[2], float(os.environ["MAX_FRAC"])
agg, rows = d["agg"], d["rows"]
ok = agg["avg_p8"] <= max_frac
print(f"  [{side}] agg mae_mean={agg['mae_mean']:.4f} avg_p8={agg['avg_p8']:.5f} "
      f"| frames={len(rows)} | gate avg_p8<={max_frac} -> {'PASS' if ok else 'FAIL'}")
for r in sorted(rows, key=lambda r: (-r["p8"], -r["mae"]))[:10]:
    print(f"    worst frame {r['frame']}: mae={r['mae']:.3f} p8={r['p8']:.5f}")
sys.exit(0 if ok else 1)
PYEOF
    then
        echo "[PASS] $n/web"
        WEB_OK=1
    else
        echo "[FAIL] $n/web: k3diff gate (avg_p8<=$MAX_FRAC)"
        WEB_OK=0
    fi
    time_gate "$n" web "$WEB_T0" "$WEB_VIDEO" "$WEB_MAX_S_PER_FRAME" || WEB_OK=0

    if [ "$ENGINE_OK" -eq 1 ] && [ "$WEB_OK" -eq 1 ]; then
        PASS=$((PASS+1))
    else
        FAIL=$((FAIL+1))
        [ "$ENGINE_OK" -eq 0 ] && FAILED_LABELS+=("$n(engine-gate)")
        [ "$WEB_OK" -eq 0 ] && FAILED_LABELS+=("$n(web-gate)")
    fi
done

echo ""
echo "============================================"
echo "  Examples: $PASS passed, $FAIL failed${FAILED_LABELS[*]:+ — ${FAILED_LABELS[*]}}"
echo "  Artifacts: $OUT_BASE"
echo "============================================"

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
exit 0
