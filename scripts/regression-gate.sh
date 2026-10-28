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
#               hyperframes-launch）
#   MAX_FRAC    avg_p8 门限（默认 0.02）
#   SKIP_BUILD=1            复用现有 release 二进制
#   OPENCAT_BIN             覆盖自动解析的 release opencat 路径
#   CHROME_BIN / CHROMEDRIVER_BIN / CHROMEDRIVER_URL   web 渲染需要
#   EXCLUDE_<例名去连字符大写>   web 侧排除窗（如 EXCLUDE_HYPERFRAMESLAUNCH）。
#               默认 hyperframes-launch=213-549,866-905,907-975（Chrome
#               WebCodecs 解不了 HEVC 源素材的平台排除窗）。
#
# 前置：
#   - cargo / bun / python / ffmpeg 在 PATH
#   - web facade：(cd crates/opencat-web/web && bun install && bun run build)
#   - 示例依赖：(cd examples/<n> && bun install)
# ---------------------------------------------------------------------------
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

REF_ROOT="${REF_ROOT:-F:/hyperframes-launches}"
EXAMPLES=${EXAMPLES:-"k3-promo vfx-heygen-combined hyperframes-launch"}
MAX_FRAC="${MAX_FRAC:-0.02}"
# hyperframes-launch：Chrome WebCodecs 解不了 HEVC 源素材（beat4-clip.mp4 /
# pill.mp4 / Frame 1&3.mp4），web 侧排除这些窗口；engine 侧全片。
: "${EXCLUDE_HYPERFRAMESLAUNCH:=213-549,866-905,907-975}"
OUT_BASE="$ROOT/out/gate"

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

# bundle 名随入口文件名（index.jsx → index.bundle.jsx；
# vfx-heygen-combined.jsx → vfx-heygen-combined.bundle.jsx）。
find_bundle() {
    local dist="$1"
    if [ -f "$dist/index.bundle.jsx" ]; then
        echo "$dist/index.bundle.jsx"
        return
    fi
    ls -1 "$dist"/*.bundle.jsx 2>/dev/null | head -1
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

PASS=0
FAIL=0
FAILED_LABELS=()

mkdir -p "$OUT_BASE"

for n in $EXAMPLES; do
    echo "============================================"
    echo "  $n"
    echo "============================================"

    ENGINE_VIDEO="$ROOT/examples/$n/dist/video.mp4"
    WEB_VIDEO="$OUT_BASE/$n-web.mp4"

    # 1) bundle + engine 渲染：标准工程流（vite 插件 closeBundle 内渲染）。
    if [ ! -d "$ROOT/examples/$n/node_modules" ]; then
        echo "[FAIL] $n: examples/$n/node_modules missing — run (cd examples/$n && bun install)" >&2
        FAIL=$((FAIL+1)); FAILED_LABELS+=("$n(engine-deps)")
        continue
    fi
    if ! (cd "$ROOT/examples/$n" && bun run build); then
        echo "[FAIL] $n: bundle/engine render failed" >&2
        FAIL=$((FAIL+1)); FAILED_LABELS+=("$n(engine-build)")
        continue
    fi
    if [ ! -f "$ENGINE_VIDEO" ]; then
        echo "[FAIL] $n: engine video missing at $ENGINE_VIDEO" >&2
        FAIL=$((FAIL+1)); FAILED_LABELS+=("$n(engine-video)")
        continue
    fi

    REF_VIDEO="$(ls -1 "$REF_ROOT/$n/renders/"*swgl*.mp4 2>/dev/null | sort | tail -1 || true)"
    if [ -z "$REF_VIDEO" ]; then
        echo "[FAIL] $n: swgl reference not found under $REF_ROOT/$n/renders/" >&2
        FAIL=$((FAIL+1)); FAILED_LABELS+=("$n(reference)")
        continue
    fi
    echo "  reference: $REF_VIDEO"

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

    BUNDLE="$(find_bundle "$ROOT/examples/$n/dist" || true)"
    if [ -z "$BUNDLE" ]; then
        echo "[FAIL] $n: no *.bundle.jsx under examples/$n/dist" >&2
        FAIL=$((FAIL+1)); FAILED_LABELS+=("$n(bundle)")
        continue
    fi
    echo "  bundle: $BUNDLE"
    RENDER_ARGS=( "$BUNDLE" --out "$WEB_VIDEO" --base-dir "$ROOT/examples/$n" )
    if [ -n "$EXCLUDE" ]; then
        RENDER_ARGS+=(--exclude "$EXCLUDE")
    fi
    if ! "$ROOT/target/release/$WEB_EXE" "${RENDER_ARGS[@]}"; then
        echo "[FAIL] $n/web: web render failed" >&2
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
