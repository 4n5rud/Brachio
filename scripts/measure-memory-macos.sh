#!/bin/sh
# Samples the memory of a running Brachio app on macOS: the app process, the WebKit helper processes
# it started (web content, GPU, networking), and optionally runtime processes and their children.
# docs/memory-benchmark.md describes the procedure.
#
# Usage: scripts/measure-memory-macos.sh APP_PID [SAMPLES] [INTERVAL_SECONDS] [RUNTIME_PID...]
#
# Prints one CSV row per sample, then the median and highest total of each column. Footprint is
# phys_footprint from footprint(1), the figure Activity Monitor shows as Memory; RSS is the resident
# size from ps(1), the closer match to Windows' working set. Both are in MiB.
set -eu

if [ $# -lt 1 ]; then
  echo "usage: $0 APP_PID [SAMPLES] [INTERVAL_SECONDS] [RUNTIME_PID...]" >&2
  exit 2
fi
app=$1
samples=${2:-60}
interval=${3:-1}
shift $(($# < 3 ? $# : 3))
runtimes="$*"

# The WebKit helpers are XPC services of the app, so launchd is their parent; launchctl lists them
# under the app's PID.
helpers() {
  launchctl print "pid/$app" 2>/dev/null |
    awk '/^\tservices = \{/ { inside = 1; next } inside && /^\t\}/ { exit }
         inside && $1 ~ /^[0-9]+$/ && $1 != 0 && $NF ~ /^com\.apple\.WebKit\./ { print $1 }'
}

descendants() {
  for child in $(pgrep -P "$1" 2>/dev/null || true); do
    echo "$child"
    descendants "$child"
  done
}

footprint_mib() {
  [ -n "$1" ] || { echo 0; return; }
  # shellcheck disable=SC2046
  footprint --noCategories -f bytes $(for pid in $1; do printf ' -p %s' "$pid"; done) 2>/dev/null |
    awk '/phys_footprint:/ { sum += $2 } END { printf "%.1f", sum / 1048576 }'
}

rss_mib() {
  [ -n "$1" ] || { echo 0; return; }
  ps -o rss= -p "$(echo "$1" | tr ' ' ',')" 2>/dev/null |
    awk '{ sum += $1 } END { printf "%.1f", sum / 1024 }'
}

rows=$(mktemp)
trap 'rm -f "$rows"' EXIT
echo "elapsed_s,app_footprint,webkit_footprint,runtime_footprint,total_footprint,total_rss,webkit_processes,runtime_processes"
start=$(date +%s)
i=0
while [ "$i" -lt "$samples" ]; do
  kill -0 "$app" 2>/dev/null || { echo "app $app exited; discard this run" >&2; exit 1; }
  web=$(helpers | xargs)
  run=""
  for pid in $runtimes; do run="$run $pid $(descendants "$pid" | xargs)"; done
  run=$(echo "$run" | xargs)
  a=$(footprint_mib "$app")
  w=$(footprint_mib "$web")
  r=$(footprint_mib "$run")
  rss=$(rss_mib "$(echo "$app $web $run" | xargs)")
  total=$(echo "$a $w $r" | awk '{ printf "%.1f", $1 + $2 + $3 }')
  row="$(($(date +%s) - start)),$a,$w,$r,$total,$rss,$(echo "$web" | wc -w | xargs),$(echo "$run" | wc -w | xargs)"
  echo "$row"
  echo "$row" >>"$rows"
  i=$((i + 1))
  if [ "$i" -lt "$samples" ]; then sleep "$interval"; fi
done

echo
awk -F, '{
  n++
  for (c = 2; c <= 6; c++) { v[c, n] = $c + 0; if ($c + 0 > peak[c]) peak[c] = $c + 0 }
} END {
  split("app_footprint webkit_footprint runtime_footprint total_footprint total_rss", names, " ")
  for (c = 2; c <= 6; c++) {
    for (i = 1; i <= n; i++) col[i] = v[c, i]
    for (i = 2; i <= n; i++) { x = col[i]; j = i - 1; while (j > 0 && col[j] > x) { col[j + 1] = col[j]; j-- } col[j + 1] = x }
    median = n % 2 ? col[(n + 1) / 2] : (col[n / 2] + col[n / 2 + 1]) / 2
    printf "%s: median %.1f MiB, peak %.1f MiB over %d samples\n", names[c - 1], median, peak[c], n
  }
}' "$rows"
