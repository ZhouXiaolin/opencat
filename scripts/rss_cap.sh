#!/usr/bin/env bash
# 重渲染护栏：OS 层内存帽 + 每秒 RSS 采样。用法：
#   bash scripts/rss_cap.sh <cap_gb> <cmd> [args...]
# RSS 超帽杀子进程（exit 137）；正常退出写 elapsed 与 peak_rss 到 stderr。
set -u
cap_gb=$1; shift
cap_kb=$((cap_gb * 1024 * 1024))

"$@" &
pid=$!
peak=0
start=$(date +%s)
while kill -0 "$pid" 2>/dev/null; do
    rss=$(ps -o rss= -p "$pid" 2>/dev/null | tr -d ' ')
    rss=${rss:-0}
    for child in $(pgrep -P "$pid" 2>/dev/null); do
        crss=$(ps -o rss= -p "$child" 2>/dev/null | tr -d ' ')
        rss=$((rss + ${crss:-0}))
    done
    (( rss > peak )) && peak=$rss
    echo "rss_kb=$rss peak_kb=$peak" >> /tmp/rss_cap_$$.csv
    if (( rss > cap_kb )); then
        kill -9 "$pid" 2>/dev/null
        pkill -9 -P "$pid" 2>/dev/null
        echo "KILLED: rss ${rss}kB > cap ${cap_kb}kB" >&2
        exit 137
    fi
    sleep 1
done
wait "$pid"; rc=$?
end=$(date +%s)
echo "exit=$rc elapsed=$((end - start))s peak_rss=$((peak / 1024))MiB" >&2
exit $rc
