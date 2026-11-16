#!/bin/bash
# On the reference host, run under `flock /run/expanse-bench/expanse-bench.flock`.
# Quiet-host gate, then the two pre-registered runs. Process names are logged, not gated
# on: another session merely waiting on this lock would otherwise block the gate forever.
set -uo pipefail
log() { echo "$(date -u +%FT%TZ) $*"; }
busy_delta() {   # busy core-equivalents over 5 s, from /proc/stat
  local a b; a=($(head -1 /proc/stat)); sleep 5; b=($(head -1 /proc/stat))
  awk -v ub="$(( (b[1]+b[2]+b[3]+b[6]+b[7]+b[8]) - (a[1]+a[2]+a[3]+a[6]+a[7]+a[8]) ))" \
      -v t="$(( (b[1]+b[2]+b[3]+b[4]+b[5]+b[6]+b[7]+b[8]) - (a[1]+a[2]+a[3]+a[4]+a[5]+a[6]+a[7]+a[8]) ))" \
      'BEGIN{printf "%.2f", 24*ub/t}'
}
quiet() {
  local d l1; d=$(busy_delta); l1=$(cut -d' ' -f1 /proc/loadavg)
  log "gate: containers=$(docker ps -q | wc -l) load1=$l1 busy=$d"
  [ "$(docker ps -q | wc -l)" = 0 ] && awk -v l="$l1" -v d="$d" 'BEGIN{exit !(l < 1.5 && d < 0.5)}'
}
wait_quiet() {   # three consecutive quiet checks; give up after ~60 min
  local ok=0 t=0
  while [ $ok -lt 3 ]; do
    if quiet; then ok=$((ok+1)); else ok=0; fi
    t=$((t+1)); [ $t -gt 150 ] && return 1
    sleep 15
  done
}
log "procs: $(pgrep -fa '[z]map|[m]ysqld|[s]tress-ng|[k]vbench|[s]moke_[0-9]|[l]lama-server|[R]unner.Worker' | tr '\n' ';' | cut -c1-400)"
for RUN in run1:11 run2:22; do
  wait_quiet || { log "host not quiet after 60 min; abort"; exit 3; }
  log "start ${RUN%%:*} load=$(cat /proc/loadavg) governor=$(cat /sys/devices/system/cpu/cpu2/cpufreq/scaling_governor)"
  docker run --rm --network none --cpuset-cpus=0-15 -v oqeval-w:/w oqeval:24.04 bash /w/scripts/bench_ref.sh ${RUN%%:*} ${RUN##*:} > ~/oqeval-ref-${RUN%%:*}.log 2>&1
  log "end ${RUN%%:*} rc=$? load=$(cat /proc/loadavg)"
done
