#!/usr/bin/env bash
# 夹逼式消融测量：每个变体前后各跑一次基线，用相邻两次基线的均值做参照。
# 这台机器有其他负载，单次 wall time 波动 ±20%，夹逼能把漂移从差值里消掉。
#
# 同时按**阶段**归因（script/layout/display/backend）：某个缓存只该影响它服务
# 的那个阶段。script 阶段不受任何后端缓存影响，所以它的 Δ 就是本次测量的噪声
# 地板——自校准。
#
# 用法: tools/ablate-bracket.sh <input> <variant[,variant...]>
set -uo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO"

INPUT="${1:-examples/hyperframes-launch/hyperframes-launch.jsx}"
IFS=',' read -r -a VARIANTS <<<"${2:?comma-separated variants}"
BIN="$REPO/target/release/opencat"
OUT="$REPO/out/bracket/$(basename "${INPUT%.*}")"
mkdir -p "$OUT"

phases() {
    # 从 profile 日志里取四个阶段的 ms/frame
    local log="$1"
    python3 - "$log" <<'PY'
import re, sys
text = open(sys.argv[1], errors="replace").read()
m = re.search(r"avg ms/frame: script ([\d.]+), frame_state ([\d.]+), resolve ([\d.]+), layout ([\d.]+), display ([\d.]+), backend ([\d.]+)", text)
print(*m.groups()) if m else print("nan nan nan nan nan nan")
PY
}

run() {
    # run <label> <tokens>  -> 打印 "<elapsed> <script> <layout> <display> <backend>"
    local label="$1" tokens="$2"
    local log="$OUT/$label.log"
    local start end
    start=$(date +%s)
    if [ -n "$tokens" ]; then
        OC_ABLATE="$tokens" "$BIN" render "$INPUT" "$OUT/$label.mp4" >"$log" 2>&1
    else
        env -u OC_ABLATE "$BIN" render "$INPUT" "$OUT/$label.mp4" >"$log" 2>&1
    fi
    local code=$?
    end=$(date +%s)
    if [ $code -ne 0 ]; then
        echo "FAIL FAIL FAIL FAIL FAIL"
        return 1
    fi
    read -r script fs resolve layout display backend < <(phases "$log")
    echo "$(( end - start )) $script $layout $display $backend"
}

echo "=== warm-up (冷 page cache 不计入) ==="
run warmup "" || exit 1

printf 'variant\twall_s\tscript\tlayout\tdisplay\tbackend\n' > "$OUT/bracket.tsv"
for v in "${VARIANTS[@]}"; do
    echo "=== [$v]"
    read -r b1w b1s b1l b1d b1b < <(run "base-before-$v" "")
    read -r vw vs vl vd vb < <(run "$v" "$v")
    read -r b2w b2s b2l b2d b2b < <(run "base-after-$v" "")

    # 相邻基线均值 = 该变体的参照
    refw=$(python3 -c "print(($b1w+$b2w)/2)")
    refs=$(python3 -c "print(($b1s+$b2s)/2)")
    refl=$(python3 -c "print(($b1l+$b2l)/2)")
    refd=$(python3 -c "print(($b1d+$b2d)/2)")
    refb=$(python3 -c "print(($b1b+$b2b)/2)")

    python3 - "$v" "$vw" "$vs" "$vl" "$vd" "$vb" "$refw" "$refs" "$refl" "$refd" "$refb" <<'PY' | tee -a "$OUT/bracket.tsv"
import sys
v, w, s, l, d, b, rw, rs, rl, rd, rb = sys.argv[1:]
f = lambda x: float(x)
print("%s\t%+.1f\t%+.2f\t%+.2f\t%+.2f\t%+.2f" % (
    v, f(w)-f(rw), f(s)-f(rs), f(l)-f(rl), f(d)-f(rd), f(b)-f(rb)))
PY
done

echo
echo "=== Δ vs 相邻基线（正 = 关掉缓存后变慢 = 缓存有收益）==="
column -t -s$'\t' "$OUT/bracket.tsv"
