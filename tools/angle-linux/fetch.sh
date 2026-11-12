#!/usr/bin/env bash
# 取官方 ANGLE Linux 构建（libEGL.so + libGLESv2.so）与 SwiftShader ICD。
#
# 来源：Electron 发行包内含 ANGLE 官方动态库与 SwiftShader Vulkan ICD，
# 是免编译拿到「同一个 ANGLE 实现」的最省路径（ANGLE 官方仓库需
# depot_tools + gclient sync 才可自建）。
#
# 用途：engine 的 GL 命令流执行端。ANGLE 严格按 WebGL/GLES 语义校验参数，
# 与 web 端（Chrome）的 WebGL 实现同源——GL stream → 桌面 GL 的转换层
# 因此可以整体省掉。
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ELECTRON_VERSION="${ELECTRON_VERSION:-33.4.11}"
URL="https://github.com/electron/electron/releases/download/v${ELECTRON_VERSION}/electron-v${ELECTRON_VERSION}-linux-x64.zip"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "==> 下载 Electron v${ELECTRON_VERSION}（ANGLE + SwiftShader 来源）"
curl -fsSL -o "$TMP/electron.zip" "$URL"
unzip -q "$TMP/electron.zip" -d "$TMP/dist"

echo "==> 安装到 $DIR"
for f in libEGL.so libGLESv2.so libvk_swiftshader.so libvulkan.so.1; do
  cp "$TMP/dist/$f" "$DIR/$f"
done

cat > "$DIR/vk_swiftshader_icd.json" <<EOF
{"file_format_version": "1.0.0", "ICD": {"library_path": "$DIR/libvk_swiftshader.so", "api_version": "1.0.5"}}
EOF

echo "==> 完成："
ls -la "$DIR"
