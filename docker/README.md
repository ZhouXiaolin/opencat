# OpenCat 纯 CPU 渲染镜像

镜像里是 `opencat` CLI。它用 ANGLE + SwiftShader 做纯 CPU 渲染，可以渲染
vfx HTML 工程。运行时不需要显卡、X11 或 Chromium。

## 准备

1. 初始化 submodule（`vendor/naga`、`vendor/resvg` 在构建时要用到）：

```sh
git submodule update --init --recursive
```

2. 下载预编译的 Skia 与 V8：

```sh
sh docker/fetch-prebuilt.sh
```

脚本把文件写到 `docker/prebuilt/`。需要外网；网络慢时加代理：

```sh
HTTPS_PROXY=http://127.0.0.1:7890 sh docker/fetch-prebuilt.sh
```

预编译包有两个来源：

- Skia：`skia-binaries` 的 release 归档，构建时以 `file://` 读取，不联网下载。
- V8：`rusty_v8` 的静态库与绑定源码，构建时由 `RUSTY_V8_MIRROR` 指向本地目录。

升级 `skia-bindings` 或改它的 feature 后，Skia 归档的文件名会变。此时改
`fetch-prebuilt.sh` 里的 `SKIA_TAG` 与 `SKIA_ARCHIVE`。

3. ANGLE 与 SwiftShader 的预编译库已在 `tools/angle-linux/`，无需额外下载。

## 构建

在仓库根目录：

```sh
DOCKER_BUILDKIT=1 docker build -t opencat:bookworm .
```

构建过程：

1. builder 阶段用 `rust:1.98.0-bookworm`，经 apt 安装 clang 20、FFmpeg 头文件
   和图形库头文件，然后用 `cargo build --release` 编译 `opencat`。
2. 运行阶段用 `debian:bookworm-slim`，装上运行库、字体，再复制二进制与
   ANGLE 库。
3. 两个阶段都先把 apt 源 `sed` 成阿里云镜像，再 `apt-get update`。

构建参数：

| 参数 | 默认值 | 说明 |
|---|---|---|
| `RUST_IMAGE` | `rust:1.98.0-bookworm` | builder 基础镜像 |
| `DEBIAN_IMAGE` | `debian:bookworm-slim` | 运行基础镜像 |
| `SKIA_BINARIES_URL` | `file:///app/docker/prebuilt/skia-binaries-{key}.tar.gz` | Skia 归档位置 |
| `RUSTY_V8_MIRROR` | `/app/docker/prebuilt/v8` | V8 归档目录 |
| `RUSTY_V8_ARCHIVE` | 空 | 直接指定 V8 静态库归档 |
| `APT_PROXY` | 空 | 给 apt.llvm.org 用的代理，例如 `http://127.0.0.1:7890` |

例子：走代理构建。

```sh
DOCKER_BUILDKIT=1 docker build -t opencat:bookworm \
  --build-arg APT_PROXY=http://127.0.0.1:7890 .
```

## 运行

挂载工程目录到 `/work`。单帧（先验证管线）：

```sh
docker run --rm \
  -v /path/to/project:/work -w /work \
  opencat:bookworm \
  render scene/index.html renders/frame.png 30 --base-dir scene
```

整片：

```sh
docker run --rm \
  -v /path/to/project:/work -w /work \
  opencat:bookworm \
  render scene/index.html renders/video.mp4 --base-dir scene
```

给足内存与 CPU（SwiftShader 会用满多核）：

```sh
docker run --rm --memory=6g --cpus="$(nproc)" ...
```

输出文件属于 root。需要保持宿主用户时加 `--user "$(id -u):$(id -g)"`。

## 说明

- 基础镜像用 bookworm。bullseye 已在 2026-08 结束 LTS，其安全仓库索引与文件
  不同步，`apt-get` 会报 404。
- FFmpeg 用发行版 5.1，不在镜像里编译。构建与运行必须用同一个版本。
- 运行时固定走 SwiftShader：`OC_SKIA_BACKEND=vulkan`、`OC_ANGLE_SWIFTSHADER=1`，
  `VK_ICD_FILENAMES` 指向 `/opt/opencat/angle/vk_swiftshader_icd.json`。
  不要用 `--env` 覆盖这些变量。
- HTML 里的远程脚本与字体由 engine 抓取，缓存在工程的 `.oc-cache/net/`。
  第一次渲染需要外网，之后可离线复现。挂载目录必须可写。
- 缺字体时，把字体文件放进工程目录再挂载。系统回退字体是 DejaVu 与 Liberation。
