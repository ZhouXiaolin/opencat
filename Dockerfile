# OpenCat CLI 纯 CPU 渲染镜像。
#
# 构建（仓库根目录）：
#   git submodule update --init --recursive
#   DOCKER_BUILDKIT=1 docker build -t opencat:bookworm .
#
# 构建与运行说明见 docker/README.md。

ARG RUST_IMAGE=rust:1.98.0-bookworm
ARG DEBIAN_IMAGE=debian:bookworm-slim

# ---------------------------------------------------------------------------
# 1. 构建阶段
# ---------------------------------------------------------------------------
FROM ${RUST_IMAGE} AS builder
ARG DEBIAN_FRONTEND=noninteractive
# 预编译包随构建上下文带入，构建时不联网下载 Skia / V8。
ARG SKIA_BINARIES_URL="file:///app/docker/prebuilt/skia-binaries-{key}.tar.gz"
ARG RUSTY_V8_MIRROR=/app/docker/prebuilt/v8
ARG RUSTY_V8_ARCHIVE
# apt.llvm.org 在部分网络下极慢（<10 KB/s）。传这个参数让 LLVM 仓库走代理。
ARG APT_PROXY=""

# apt 源先换成阿里云再 update。Skia 的 bindgen 需要 C++20 的 libclang，
# bookworm 自带的 clang 14 太旧，用 apt.llvm.org 的 LLVM 20。
RUN set -eux; \
    sed -i 's|deb.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list 2>/dev/null || true; \
    sed -i 's|deb.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list.d/*.sources 2>/dev/null || true; \
    apt-get update; \
    apt-get install -y --no-install-recommends ca-certificates curl; \
    echo "deb [trusted=yes] https://apt.llvm.org/bookworm/ llvm-toolchain-bookworm-20 main" \
        > /etc/apt/sources.list.d/llvm.list; \
    apt-get -o Acquire::https::Proxy="${APT_PROXY}" update; \
    apt-get -o Acquire::https::Proxy="${APT_PROXY}" install -y --no-install-recommends \
        libclang-20-dev libclang-common-20-dev \
        pkg-config cmake ninja-build python3 perl \
        libgl-dev libegl-dev libgles-dev libglvnd-dev \
        libwayland-dev libx11-dev libxcb1-dev \
        libasound2-dev libfontconfig1-dev libfreetype-dev \
        zlib1g-dev libpng-dev libjpeg-dev libwebp-dev \
        libavcodec-dev libavformat-dev libavutil-dev \
        libavdevice-dev libavfilter-dev libswscale-dev libswresample-dev \
        xz-utils; \
    rm -rf /var/lib/apt/lists/*

# crate 源统一走 rsproxy。写在 CARGO_HOME 而不是仓库的 .cargo/config.toml，
# 后者保留 wasm32 的 getrandom 配置，两者互不覆盖。
RUN mkdir -p /usr/local/cargo && cat > /usr/local/cargo/config.toml <<'EOF'
[source.crates-io]
replace-with = "rsproxy-sparse"

[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"

[registries.rsproxy]
index = "sparse+https://rsproxy.cn/index/"

[net]
git-fetch-with-cli = true
retry = 10

[http]
timeout = 600
EOF

ENV LIBCLANG_PATH=/usr/lib/llvm-20/lib \
    CARGO_NET_RETRY=10 \
    CARGO_HTTP_TIMEOUT=600 \
    SKIA_BINARIES_URL=${SKIA_BINARIES_URL}

WORKDIR /app
COPY . .

# vendor/naga、vendor/resvg 是 git submodule；Skia 归档必须随上下文带入。
RUN test -f vendor/naga/Cargo.toml && test -f vendor/resvg/Cargo.toml \
 && ls docker/prebuilt/skia-binaries-*.tar.gz >/dev/null \
 || (echo "缺少 vendor submodule 或 docker/prebuilt/skia-binaries-*.tar.gz" >&2; exit 1)

# 传空值时必须 unset：v8 的 build.rs 看到变量存在就会当镜像地址用。
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    set -eux; \
    if [ -z "${RUSTY_V8_MIRROR:-}" ]; then unset RUSTY_V8_MIRROR; fi; \
    if [ -z "${RUSTY_V8_ARCHIVE:-}" ]; then unset RUSTY_V8_ARCHIVE; fi; \
    cargo build --release --locked -p opencat --bin opencat; \
    install -Dm755 target/release/opencat /usr/local/bin/opencat

# ---------------------------------------------------------------------------
# 2. 运行阶段
#
# FFmpeg 用发行版的 5.1（libavcodec 59），不编译。libGLESv2 还需要
# libX11/libXext/libxcb/libdrm。
# ---------------------------------------------------------------------------
FROM ${DEBIAN_IMAGE}
ARG DEBIAN_FRONTEND=noninteractive
RUN set -eux; \
    sed -i 's|deb.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list 2>/dev/null || true; \
    sed -i 's|deb.debian.org|mirrors.aliyun.com|g' /etc/apt/sources.list.d/*.sources 2>/dev/null || true; \
    apt-get update; \
    apt-get install -y --no-install-recommends \
        libgl1 libegl1 \
        libx11-6 libxext6 libxcb1 libdrm2 \
        libfontconfig1 libfreetype6 fontconfig \
        libasound2 libx264-164 zlib1g libstdc++6 libgomp1 \
        libavcodec59 libavformat59 libavutil57 libavdevice59 \
        libavfilter8 libswscale6 libswresample4 \
        fonts-dejavu-core fonts-liberation \
        ca-certificates tini; \
    fc-cache -f; \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/local/bin/opencat /usr/local/bin/opencat

# ANGLE + SwiftShader 预编译库。仓库里的 ICD json 指向开发机，这里重写。
COPY tools/angle-linux/libEGL.so \
     tools/angle-linux/libGLESv2.so \
     tools/angle-linux/libvk_swiftshader.so \
     tools/angle-linux/libvulkan.so.1 \
     /opt/opencat/angle/
RUN printf '%s\n' \
    '{"file_format_version":"1.0.0","ICD":{"library_path":"/opt/opencat/angle/libvk_swiftshader.so","api_version":"1.0.5"}}' \
    > /opt/opencat/angle/vk_swiftshader_icd.json

# 纯 CPU：Skia 合成走 Vulkan backend，设备固定为 SwiftShader；ANGLE 共享同一份 ICD。
ENV OC_ANGLE_DIR=/opt/opencat/angle \
    OC_ANGLE_SWIFTSHADER=1 \
    OC_SKIA_BACKEND=vulkan \
    OC_GPU_PIPELINE=1 \
    OC_WGPU_BACKEND=vulkan \
    VK_ICD_FILENAMES=/opt/opencat/angle/vk_swiftshader_icd.json \
    LD_LIBRARY_PATH=/opt/opencat/angle \
    LANG=C.UTF-8 \
    HOME=/root

WORKDIR /work
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/opencat"]
CMD ["--help"]

LABEL org.opencontainers.image.title="opencat" \
      org.opencontainers.image.description="OpenCat CLI - CPU-only SwiftShader renderer" \
      org.opencontainers.image.licenses="MIT"
