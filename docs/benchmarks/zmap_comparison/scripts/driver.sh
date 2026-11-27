#!/bin/bash
# Timed runs. Holds the host bench lock for the whole sequence; results go to a
# Docker volume on /var (the /home filesystem is below the 20 GB floor).
#
# Environment (defaults are those of the committed 0.18-era record):
#   ZMAP_IMG     image tag
#   ZMAP_VOLUME  results volume
#   ZMAP_SCRIPTS directory holding these scripts on the host
#   ZMAP_CPUSET  container cpuset, one logical CPU per physical core
#   ZMAP_MICRO_CPU  the CPU of that set the harness is pinned to
#   ZMAP_ARMS    "stock expanse" (default) or "stock main expanse"
#   ZMAP_QUIET_LOAD  one-minute load the quiet gate waits for (default 1.5)
set -u
IMG=${ZMAP_IMG:-zmap-judy-eval:d8fc99c9-82eb56af}
V=${ZMAP_VOLUME:-zmapeval-results}
S=${ZMAP_SCRIPTS:-$HOME/zmap-judy-eval/ctx/scripts}
CPUSET=${ZMAP_CPUSET:-0,2,4,6,8,10,12,14}
MCPU=${ZMAP_MICRO_CPU:-2}
ARMS=${ZMAP_ARMS:-stock expanse}
QL=${ZMAP_QUIET_LOAD:-1.5}
LOG=/dev/stdout
quiet_gate() {
  for i in $(seq 1 30); do
    l1=$(cut -d' ' -f1 /proc/loadavg)
    busy=$(pgrep -fa "[l]lama-server|[u]nsloth|[s]moke_[0-9]|[k]vbench|[r]sync" | wc -l)
    if awk "BEGIN{exit !($l1 < $QL)}" && [ "$busy" -eq 0 ]; then echo "gate ok load1=$l1 $(date -u +%T)"; return 0; fi
    echo "gate wait load1=$l1 busy=$busy $(date -u +%T)"; sleep 10
  done
  echo "ABORT: host not quiet"; return 1
}
docker volume create $V >/dev/null
for step in "e2e 1" "micro 1" "e2e 2" "micro 2"; do
  set -- $step
  quiet_gate || exit 1
  echo "== $1 run $2 start $(date -u +%T) load $(cat /proc/loadavg)"
  if [ $1 = e2e ]; then
    docker run --rm --network none --privileged --cpuset-cpus=$CPUSET -e ZMAP_ARMS="$ARMS" \
      -v $V:/results -v $S:/scripts:ro $IMG bash /scripts/e2e.sh $2 10 200000 > /tmp/zmapeval_$1_$2.log 2>&1
  else
    docker run --rm --network none --cpuset-cpus=$CPUSET -e ZMAP_ARMS="$ARMS" \
      -v $V:/results -v $S:/scripts:ro $IMG bash /scripts/micro.sh $2 15 $MCPU > /tmp/zmapeval_$1_$2.log 2>&1
  fi
  echo "== $1 run $2 rc=$? end $(date -u +%T) load $(cat /proc/loadavg) lines $(wc -l < /tmp/zmapeval_$1_$2.log)"
done
echo DONE
