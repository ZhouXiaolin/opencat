#!/usr/bin/env bash
# 渲染门禁：超时（默认 600s）或提交内存超限（默认 5120 MB）即杀进程树。
# 用法: ./scripts/render-guard.sh [--timeout 秒] [--mem MB] -- <命令...>
#
# 内存口径 = 私有提交（PrivateMemorySize64），不是工作集：ANGLE/D3D11 的
# 暂存资源与未触碰的分配只记提交、不进工作集——工作集 2.3G 时提交已顶满
# 系统提交上限（本机页文件小），表现为 `memory allocation of N bytes failed`。
set -uo pipefail

TIMEOUT=600
MEM_MB=5120
while [ $# -gt 0 ]; do
    case "$1" in
        --timeout) TIMEOUT="$2"; shift 2 ;;
        --mem) MEM_MB="$2"; shift 2 ;;
        --) shift; break ;;
        *) break ;;
    esac
done
[ $# -gt 0 ] || { echo "usage: $0 [--timeout s] [--mem mb] -- cmd..." >&2; exit 2; }

# 私有提交（MB），按进程树合计（`--jobs` 分块渲染的子进程一并计入）；
# PowerShell 不可用时退回 tasklist 工作集（只看根进程）。
TREE_MEM_PS1=$(cygpath -w "$(dirname "$0")/tree-mem.ps1" 2>/dev/null || echo "$(dirname "$0")/tree-mem.ps1")
mem_mb() {
    local m
    m=$(powershell -NoProfile -ExecutionPolicy Bypass -File "$TREE_MEM_PS1" -Root "$1" 2>/dev/null | tr -d '\r ')
    if [ -n "$m" ] && [ "$m" -gt 0 ] 2>/dev/null; then
        echo $((m / 1048576))
        return
    fi
    m=$(tasklist //FI "PID eq $1" //FO CSV //NH 2>/dev/null | awk -F'","' 'NR==1{print $5}' | tr -d ' K,"')
    [ -n "$m" ] && echo $((m / 1024))
}

start=$(date +%s)
"$@" &
pid=$!
win=$pid
peak=0
while kill -0 "$pid" 2>/dev/null; do
    # MSYS 的 $! 不是 Windows PID，且 fork→exec 之间 WINPID 会换（先是 fork
    # 出的 msys 壳，exec 后才是原生进程）：每轮重读，taskkill/查询都用它。
    win=$(cat "/proc/$pid/winpid" 2>/dev/null || echo "$win")
    elapsed=$(( $(date +%s) - start ))
    if [ "$elapsed" -ge "$TIMEOUT" ]; then
        echo "[guard] TIMEOUT ${elapsed}s > ${TIMEOUT}s, kill $pid (peak_mem=${peak}MB)" >&2
        taskkill //F //T //PID "$win" >/dev/null 2>&1
        wait "$pid" 2>/dev/null
        exit 124
    fi
    mb=$(mem_mb "$win")
    if [ -n "${mb:-}" ]; then
        [ "$mb" -gt "$peak" ] && peak=$mb
        if [ "$mb" -gt "$MEM_MB" ]; then
            echo "[guard] MEM ${mb}MB > ${MEM_MB}MB, kill $pid (elapsed=${elapsed}s)" >&2
            taskkill //F //T //PID "$win" >/dev/null 2>&1
            wait "$pid" 2>/dev/null
            exit 137
        fi
    fi
    sleep 1
done
wait "$pid"
rc=$?
echo "[guard] rc=$rc elapsed=$(( $(date +%s) - start ))s peak_mem=${peak}MB" >&2
exit "$rc"
