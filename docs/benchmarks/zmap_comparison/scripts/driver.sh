#!/bin/bash
# Timed runs. Holds the host bench lock for the whole sequence; results go to a
# Docker volume on /var (the /home filesystem is below the 20 GB floor).
set -u
IMG=zmap-judy-eval:d8fc99c9-82eb56af
V=zmapeval-results
S=$HOME/zmap-judy-eval/ctx/scripts
LOG=/dev/stdout
quiet_gate() {
  for i in $(seq 1 30); do
    l1=$(cut -d' ' -f1 /proc/loadavg)
    busy=$(pgrep -fa "[l]lama-server|[u]nsloth|[s]moke_[0-9]|[k]vbench|[r]sync" | wc -l)
    if awk "BEGIN{exit !($l1 < 1.5)}" && [ "$busy" -eq 0 ]; then echo "gate ok load1=$l1 $(date -u +%T)"; return 0; fi
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
    docker run --rm --network none --privileged --cpuset-cpus=0,2,4,6,8,10,12,14 \
      -v $V:/results -v $S:/scripts:ro $IMG bash /scripts/e2e.sh $2 10 200000 > /tmp/zmapeval_$1_$2.log 2>&1
  else
    docker run --rm --network none --cpuset-cpus=0,2,4,6,8,10,12,14 \
      -v $V:/results -v $S:/scripts:ro $IMG bash /scripts/micro.sh $2 15 2 > /tmp/zmapeval_$1_$2.log 2>&1
  fi
  echo "== $1 run $2 rc=$? end $(date -u +%T) load $(cat /proc/loadavg) lines $(wc -l < /tmp/zmapeval_$1_$2.log)"
done
echo DONE
