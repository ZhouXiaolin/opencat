#!/usr/bin/env bash
# 取 rust-skia 官方预编译包（skia-bindings 的 binary-cache 源）。
#
# 用途：Skia 的 C++ 静态库由 rust-skia 官方在 GitHub Releases 上按
# 「crate 版本 + feature 组合」发布。skia-bindings 会在编译时按自己的
# feature 组合算出 key，然后从 SKIA_BINARIES_URL 指定的地址取包。
#
# 本脚本把官方包下载到本地缓存目录，SKIA_BINARIES_URL 指向本地：
#   file:///home/solaren/skia-binaries/skia-binaries-{key}.tar.gz
#
# 用法：
#   tools/skia-binaries/fetch.sh                # 拉 engine 当前 feature 组合的包
#   tools/skia-binaries/fetch.sh <key>          # 拉指定 key 的包
#
# key 由「版本前缀 + target + feature 组合」拼成，见 skia-bindings 的
# build_support/binary_cache/binaries.rs::key。feature 一变 key 就变，
# 缺包时 skia-bindings 会回退到全量源码编译 Skia（很慢），
# 所以改动 skia-safe 的 feature 后要重跑本脚本。
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CACHE="${SKIA_BINARIES_CACHE:-$HOME/skia-binaries}"
TAG="${SKIA_BINARIES_TAG:-0.153.3}"
REPO="https://github.com/rust-skia/skia-binaries/releases/download"

# engine 当前 Linux feature 组合对应的 key（见 crates/opencat-engine/Cargo.toml）。
DEFAULT_KEY="b7f043e0b1e2a850e702-x86_64-unknown-linux-gnu-egl-ganesh-gl-jpegd-jpege-pdf-skottie-svg-textlayout-vulkan-wayland-webpd-webpe-x11"
KEY="${1:-$DEFAULT_KEY}"

mkdir -p "$CACHE"
OUT="$CACHE/skia-binaries-$KEY.tar.gz"

if [ -f "$OUT" ]; then
  echo "==> 已存在，跳过下载：$OUT"
else
  echo "==> 下载 skia-binaries $TAG / $KEY"
  curl -fL --create-dirs -o "$OUT" "$REPO/$TAG/skia-binaries-$KEY.tar.gz"
  echo "==> 完成：$OUT"
fi

echo
echo "提示：key 随 skia-safe 的 feature 组合变化。改了 feature 后重跑本脚本，"
echo "      否则 skia-bindings 会回退到全量源码编译 Skia。"
