#!/usr/bin/env bash
# 把一个 JSX 工程目录里的 className 编译成标准 CSS。
#
# 用法:
#   scripts/build-css.sh <jsx-dir>...   # 编译指定目录（可多个）
#   scripts/build-css.sh --all          # 编译 examples/ 下所有含 .jsx 的目录
#
# 产物: <jsx-dir>/tailwind.css —— 引擎按此约定路径发现并注入 CSS 引擎
#       （见 crates/opencat-engine/src/jsx.rs）。文件不存在时 class 不产生
#       任何声明；先跑本脚本，再渲染。
#
# 输入 CSS 由脚本临时生成：`@import "tailwindcss"` + `@source` 指向该目录，
# tailwind v4 据此扫描 className 并产出 var()/calc()/color-mix() 形态的
# 标准 CSS。目录里有 app.css 时一并 @import，手写规则与 utilities 同层叠。
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

dirs=()
if [ "${1:-}" = "--all" ]; then
    while IFS= read -r dir; do
        dirs+=("${dir%/}")
    done < <(find examples -name '*.jsx' -print0 | xargs -0 -n1 dirname | sort -u)
else
    [ "$#" -gt 0 ] || { echo "用法: build-css.sh <jsx-dir>... | --all" >&2; exit 1; }
    dirs=("$@")
fi

for dir in "${dirs[@]}"; do
    (
        cd "$dir"
        input=".tailwind-input.css"
        {
            echo '@import "tailwindcss";'
            echo '@source "./**/*.jsx";'
            [ -f app.css ] && echo '@import "./app.css";'
        } > "$input"
        bunx tailwindcss -i "$input" -o tailwind.css --minify
        rm -f "$input"
        echo "wrote $dir/tailwind.css"
    )
done
