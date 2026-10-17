#!/usr/bin/env bash
# 把一个 JSX 工程目录里的 className 编译成标准 CSS。
#
# 用法: scripts/build-css.sh <jsx-dir>
# 产物: <jsx-dir>/tailwind.css —— 引擎按此约定路径发现并注入 CSS 引擎
#       （见 crates/opencat-engine/src/jsx.rs）。文件不存在时 class 不产生
#       任何声明；先跑本脚本，再渲染。
#
# 输入 CSS 由脚本临时生成：`@import "tailwindcss"` + `@source` 指向该目录，
# tailwind v4 据此扫描 className 并产出 var()/calc()/color-mix() 形态的
# 标准 CSS；手写 CSS 直接追加在 app.css 里同样有效。
set -euo pipefail

dir="${1:?用法: build-css.sh <jsx-dir>}"
dir="${dir%/}"
cd "$dir"

cat > .tailwind-input.css <<'EOF'
@import "tailwindcss";
@source "./**/*.jsx";
EOF

bunx tailwindcss -i .tailwind-input.css -o tailwind.css --minify
rm -f .tailwind-input.css
echo "wrote $dir/tailwind.css"
