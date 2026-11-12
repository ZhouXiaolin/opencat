#!/bin/sh
# 下载构建镜像所需的预编译包到 docker/prebuilt/。
#
# 用法：
#   sh docker/fetch-prebuilt.sh
#   HTTPS_PROXY=http://127.0.0.1:7890 sh docker/fetch-prebuilt.sh
#
# 已存在的文件会跳过，可重复执行。
set -eu

# skia-bindings 的版本与预编译包文件名。升级 skia-safe 或修改其 feature
# 后，文件名会变；新文件名可从 skia-bindings 的构建日志里取。
SKIA_TAG=0.153.3
SKIA_ARCHIVE=skia-binaries-b7f043e0b1e2a850e702-x86_64-unknown-linux-gnu-egl-ganesh-gl-jpegd-jpege-pdf-skottie-svg-textlayout-vulkan-wayland-webpd-webpe-x11.tar.gz

# rusty_v8 的版本与两个必需文件：静态库 + 绑定源码。
V8_TAG=v152.2.0
V8_ARCHIVE=librusty_v8_release_x86_64-unknown-linux-gnu.a.gz
V8_BINDING=src_binding_release_x86_64-unknown-linux-gnu.rs

SKIA_URL="https://github.com/rust-skia/skia-binaries/releases/download/$SKIA_TAG/$SKIA_ARCHIVE"
V8_URL="https://github.com/denoland/rusty_v8/releases/download/$V8_TAG"

dir=$(cd "$(dirname "$0")" && pwd)/prebuilt
mkdir -p "$dir/v8"

get() {
    if [ -s "$2" ]; then
        echo "已存在 $2"
        return
    fi
    echo "下载 $1"
    curl -fL --retry 3 --retry-delay 2 -o "$2.part" "$1"
    mv "$2.part" "$2"
}

get "$SKIA_URL" "$dir/$SKIA_ARCHIVE"
get "$V8_URL/$V8_ARCHIVE" "$dir/v8/$V8_ARCHIVE"
get "$V8_URL/$V8_BINDING" "$dir/v8/$V8_BINDING"

tar -tzf "$dir/$SKIA_ARCHIVE" >/dev/null
gunzip -t "$dir/v8/$V8_ARCHIVE"
echo "预编译包就绪：$dir"
