#!/bin/zsh
# Guarded `opencat render` for this checkout.
#
# A release render runs for 8+ minutes and allocates whole frames of RGBA, so a
# runaway (the 2026-10-12 hyperframes OOM reached 46 GB before the kernel killed
# it) costs the run *and* the machine. This wrapper launches the render under
# `nohup`, samples its RSS once a second, writes a CSV of the memory curve, and
# hard-kills the render as soon as RSS crosses a cap (default 20 GiB).
#
# Usage: tools/render-guard.sh <input.jsx> <output> [max_rss_gb]
#        OC_RENDER_MAX_RSS_GB=6 tools/render-guard.sh a.jsx out/a.mp4
#
# For a long detached run, wrap the guard itself too:
#        nohup tools/render-guard.sh a.jsx out/a.mp4 > /dev/null 2>&1 &
#
# When launched non-interactively (stdin is not a tty -- an agent, a script, a
# torn-down terminal) the guard re-execs itself under `setsid` so the caller's
# process group cannot take the guard -- and the render it watches -- down with
# it. Set OC_RENDER_GUARD_FOREGROUND=1 to force a blocking, attached run.
#
# Exit status: the render's own status, or 137 when the RSS cap killed it.
# Artifacts: out/render-guard/<name>-<stamp>.log (render output)
#            out/render-guard/<name>-<stamp>.rss.csv (per-second RSS in MiB)
#            out/render-guard/<name>-<stamp>.status (final exit line)

set -u
emulate -L zsh

if (( $# < 2 )); then
  print -u2 "usage: tools/render-guard.sh <input.jsx> <output> [max_rss_gb]"
  exit 2
fi

repo=${0:A:h:h}
cd "$repo" || exit 1

input=$1
output=$2
max_rss_gb=${3:-${OC_RENDER_MAX_RSS_GB:-20}}
cap_kb=$(( max_rss_gb * 1024 * 1024 ))

bin=$repo/target/release/opencat
if [[ ! -x $bin ]]; then
  print -u2 "missing $bin -- run: cargo build --release --bin opencat"
  exit 1
fi

log_dir=${OC_RENDER_LOG_DIR:-$repo/out/render-guard}
mkdir -p "$log_dir" || exit 1
stamp=${OC_RENDER_GUARD_STAMP:-$(date +%Y%m%d-%H%M%S)}
stem=$(basename "$input"); stem=${stem%.jsx}
log=$log_dir/$stem-$stamp.log
rss_csv=$log_dir/$stem-$stamp.rss.csv
status_file=$log_dir/$stem-$stamp.status

# Detach into our own session unless interactive (or forced). Without this a
# guard started from a non-tty caller dies with the caller's process group and
# leaves the render running unguarded -- the exact OOM this gate prevents.
if [[ -z ${OC_RENDER_GUARD_DETACHED:-} && ! -t 0 && ${OC_RENDER_GUARD_FOREGROUND:-0} != 1 ]]; then
  export OC_RENDER_GUARD_DETACHED=1 OC_RENDER_GUARD_STAMP=$stamp
  guard_log=$log_dir/$stem-$stamp.guard.log
  setsid --fork "${0:A}" "$@" >"$guard_log" 2>&1 </dev/null &
  print "detached guard pid=$!"
  print "  render log -> $log"
  print "  rss csv    -> $rss_csv"
  print "  status     -> $status_file"
  print "  guard log  -> $guard_log"
  exit 0
fi

# nohup: the render ignores SIGHUP, so it outlives a closed terminal. The guard
# stays its parent on purpose -- `wait` needs that to report the real status.
nohup "$bin" render "$input" "$output" >"$log" 2>&1 &
pid=$!

print "render pid=$pid cap=${max_rss_gb}GiB"
print "  log     -> $log"
print "  rss csv -> $rss_csv"
print "timestamp,rss_mb,elapsed_s" > "$rss_csv"

start=$(date +%s)
peak_kb=0
killed=0
while kill -0 "$pid" 2>/dev/null; do
  rss_kb=$(ps -o rss= -p "$pid" 2>/dev/null | tr -d ' ')
  if [[ -n $rss_kb ]]; then
    (( rss_kb > peak_kb )) && peak_kb=$rss_kb
    now=$(date +%s)
    print "$(date +%H:%M:%S),$(( rss_kb / 1024 )),$(( now - start ))" >> "$rss_csv"
    if (( rss_kb > cap_kb )); then
      print -u2 "RSS $(( rss_kb / 1024 ))MiB > ${max_rss_gb}GiB cap -- killing pid $pid"
      print "$(date +%H:%M:%S),KILL,$(( now - start ))" >> "$rss_csv"
      kill -TERM "$pid" 2>/dev/null
      sleep 5
      kill -KILL "$pid" 2>/dev/null
      killed=1
      break
    fi
  fi
  sleep 1
done

wait "$pid" 2>/dev/null
exit_code=$?
(( killed )) && exit_code=137
elapsed=$(( $(date +%s) - start ))
print "exit=$exit_code elapsed=${elapsed}s peak_rss=$(( peak_kb / 1024 ))MiB"
print "exit=$exit_code elapsed=${elapsed}s peak_rss=$(( peak_kb / 1024 ))MiB" > "$status_file"

if (( killed )); then
  print -u2 "render killed by the RSS cap -- read $log and $rss_csv before re-running"
  exit 137
fi

exit $exit_code
