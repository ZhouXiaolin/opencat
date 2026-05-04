#!/usr/bin/env bash
# scripts/check_core_purity.sh
# 本地手工执行；CI 设施未来如果接入 GitHub Actions / GitLab CI，再把它接进 .yml。
set -euo pipefail
cd "$(dirname "$0")/.."

echo "[1/4] cargo check -p opencat-core --no-default-features --lib"
cargo check -p opencat-core --no-default-features --lib

echo "[2/4] cargo check -p opencat-core --no-default-features --lib --tests"
cargo check -p opencat-core --no-default-features --lib --tests

echo "[3/4] core 不引 host (cargo check 已验证编译；下面列出所有引用供透明审计)"
grep -rnE "opencat::host|crate::host|super::host" crates/opencat-core/src/ || echo "  (no references found)"
echo "  所有引用必须位于 #[cfg(feature = \"host-default\")] 门控后方"

echo "[4/4] cargo tree without host deps"
forbidden=$(cargo tree -p opencat-core --no-default-features --prefix none --edges normal 2>/dev/null | grep -E "ffmpeg-next|skia-safe|rquickjs|reqwest|tokio|rodio" || true)
if [[ -n "$forbidden" ]]; then
  echo "FAIL: forbidden host deps in core build:"
  echo "$forbidden"
  exit 1
fi

echo "OK: core purity verified."
