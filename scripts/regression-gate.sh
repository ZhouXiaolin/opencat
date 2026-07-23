#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# OpenCat Cross-Platform Regression Gate (Issue #48)
#
# Usage:  regression-gate.sh [--oracle] [--ci]
#
#   --oracle   Also run browser oracle SSIM tests (requires ChromeDriver,
#              ffmpeg, and miniserve). Oracle tests are #[ignore] and run
#              via `cargo test -- --ignored web_frame_oracle`.
#   --ci       CI mode: skip preconditions check, skip interactive steps.
#
# Run this from the repo root. Exits with 0 on success, non-zero on failure.
#
# Preconditions:
#   - cargo, bun on PATH
#   - bun install --frozen-lockfile in web/ (root)
#   - bun install --frozen-lockfile in crates/opencat-web/web
#   - bun install --frozen-lockfile in crates/opencat-engine/testsupport
#   - (oracle only) chromedriver, ffmpeg, miniserve on PATH
#
# Verifies:
#   - Rust: cargo test + clippy for core, engine, opencat (lib)
#   - Web:  bun run test, bunx tsc --noEmit, bun run build (root web)
#   - Crate: tsc build:types + vite build (crates/opencat-web/web)
#   - (oracle only) Browser SSIM: chromedriver oracle tests (AC 5-7)
# ---------------------------------------------------------------------------
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PASS=0
FAIL=0
RUN_ORACLE=0
CI_MODE=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --oracle) RUN_ORACLE=1; shift ;;
        --ci)     CI_MODE=1;   shift ;;
        *)        echo "[ERROR] Unknown option: $1"; exit 1 ;;
    esac
done

pass()  { PASS=$((PASS+1)); echo "[PASS] $*"; }
fail()  { FAIL=$((FAIL+1)); echo "[FAIL] $*"; }

run_check() {
    local label="$1" desc="$2"
    shift 2
    echo ""
    echo "===== [$label] $desc ====="
    if "$@"; then
        pass "$desc"
    else
        fail "$desc"
    fi
}

echo "============================================"
echo " OpenCat Cross-Platform Regression Gate"
echo "   $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
echo "============================================"
echo ""

# ---- Rust: tests (AC1) ----
run_check "RUST" "cargo test -p opencat-core -p opencat-engine -p opencat --lib" \
    cargo test -p opencat-core -p opencat-engine -p opencat --lib

# ---- Rust: clippy (AC2) ----
run_check "RUST" "cargo clippy -p opencat-core --lib" \
    cargo clippy -p opencat-core --lib

run_check "RUST" "cargo clippy -p opencat-engine --lib" \
    cargo clippy -p opencat-engine --lib

run_check "RUST" "cargo clippy -p opencat --lib" \
    cargo clippy -p opencat --lib

# ---- Web root: tests + tsc + build (AC3) ----
echo ""
echo "===== [WEB] Web root ====="
cd "$ROOT/web"

run_check "WEB" "bun run test" \
    bun run test

run_check "WEB" "bunx tsc --noEmit" \
    bunx tsc --noEmit

run_check "WEB" "bun run build" \
    bun run build

# ---- Crate web: types + build (AC4) ----
echo ""
echo "===== [WEB] crates/opencat-web/web ====="
cd "$ROOT/crates/opencat-web/web"

run_check "WEB" "crate tsc build:types" \
    bun run build:types

run_check "WEB" "crate vite build" \
    bunx vite build

# ---- Browser oracle SSIM (AC 5-7) ----
if [ "$RUN_ORACLE" -eq 1 ]; then
    echo ""
    echo "===== [ORACLE] Browser SSIM tests (AC 5-7) ====="
    cd "$ROOT"

    run_check "ORACLE" "chromedriver oracle SSIM tests (AC 5-7)" \
        cargo test -- --ignored web_frame_oracle 2>&1
fi

# ---- Summary ----
echo ""
echo "============================================"
echo "  Results: $PASS passed, $FAIL failed"
echo "  $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
echo "============================================"

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
exit 0
