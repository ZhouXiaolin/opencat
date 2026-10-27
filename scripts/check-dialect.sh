#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# 方言反查（ARCHITECTURE.md §3 壳子原则的验收闸门）
#
# examples/ 里出现方言 API / ctx.* 残留 = 承接面泄漏信号。
# 正确动作是把能力沉进 core/engine/web，让例子回归标准——不是改例子迁就引擎。
#
# 黑名单分两层：
#   STRICT  已退役面：命中即 fail（gate 挂）。
#   PENDING 批次目标面（ARCHITECTURE.md §12 路线图）：命中仅 warn，
#           对应纵切批次闭环时移入 STRICT。
#
# Usage: check-dialect.sh        # 从仓库根任意位置运行均可
# ---------------------------------------------------------------------------
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SCAN_DIR="$ROOT/examples"

if [ ! -d "$SCAN_DIR" ]; then
    echo "[check-dialect] no examples/ directory, nothing to scan"
    exit 0
fi

# grep 的公共排除：打包产物、资产、依赖
EXCLUDES=(--include='*.jsx' --include='*.js' --include='*.mjs'
          --exclude-dir=node_modules --exclude-dir=assets --exclude-dir=dist)

# 已退役面（ARCHITECTURE.md §10 先例）：命中即 fail
STRICT_PATTERNS=(
    'ctx\.splitText\b'            # 批次 C4：官方 SplitText 唯一路
    'ctx\.gsap\b'                 # 批次 1：真 gsap import 唯一路
    'ctx\.morphSVG\b'             # 批次 1：官方 MorphSVGPlugin
    'ctx\.motionPath\b'           # 批次 1：官方 MotionPathPlugin
    'ctx\.scrambleText\b'         # 批次 1：官方 ScrambleTextPlugin
    'ctx\.CanvasKit\b'            # 批次 ②：CanvasKitInit import 唯一路
    'ctx\.getCanvasById\b'        # 批次 ②：CK.MakeCanvasSurface(id) 唯一路
    'useCanvasKit\b'              # 批次 ②：同上
    'useCanvas\b'                 # 批次 ②：useFrame + MakeCanvasSurface 拆解
    'ly\.(pixel|scan|draw|surface|source)\b'  # 批次 ⑤：liangyi 退役——官方 RuntimeEffect(SKSL) / ctx.surface 唯一路
    '<text[\s>/]'                 # 批次 ②：自有 Text 标签退役——HTML p/span 唯一路
)

# 批次目标面：迁移闭环时移入 STRICT
PENDING_PATTERNS=(
    'ctx\.three(js)?\b'           # ③ three 纵切：import * as THREE from 'three'
    'useThree\b'                  # ③ three 纵切：同上
)

scan() {
    local label="$1"; shift
    local hit=0
    for pattern in "$@"; do
        while IFS=: read -r file line text; do
            [ -z "$file" ] && continue
            printf '  [%s] %s:%s: %s\n' "$label" "${file#"$ROOT"/}" "$line" "$(echo "$text" | sed 's/^[[:space:]]*//')"
            hit=1
        done < <(grep -rEn "$pattern" "$SCAN_DIR" "${EXCLUDES[@]}" 2>/dev/null || true)
    done
    return $hit
}

echo "===== [DIALECT] 方言反查（壳子原则闸门）====="

FAIL=0
if scan STRICT "${STRICT_PATTERNS[@]}"; then :; else
    echo "[FAIL] 已退役方言残留（命中即泄漏）"
    FAIL=1
fi
if scan PENDING "${PENDING_PATTERNS[@]}"; then :; else
    echo "[WARN] 批次目标方言（§12 路线图闭环后转 STRICT）"
fi

if [ "$FAIL" -ne 0 ]; then
    exit 1
fi
echo "[PASS] 无已退役方言残留"
exit 0
