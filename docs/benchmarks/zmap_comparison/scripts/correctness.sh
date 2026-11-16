#!/bin/bash
# H0 gate: identical behaviour of stock, Expanse-built and swapped zmap.
set -eu
OUT=/results/correctness; mkdir -p $OUT/out
/scripts/netns_setup.sh
GW=$(cat /tmp/gwmac)
echo "== dryrun packet streams (3000 targets)"
# Sequence numbers and source ports come from a per-run random validation
# key, so the comparable stream is the ordered (daddr, dport) sequence.
for v in stock stock2 expanse swap; do
  b=/build/stock/src/zmap; e=""
  [ $v = expanse ] && b=/build/expanse/src/zmap
  [ $v = swap ] && e="LD_LIBRARY_PATH=/opt/expanse/compat"
  env $e ip netns exec scan $b --dryrun -n 3000 -e 7 -T 1 -i veth0 -S 192.0.2.1 -G $GW \
    -p 80,443,8080,8443 -b /scripts/empty_blocklist.conf --disable-syslog -q -v 0 \
    -c 0 198.18.0.0/15 > $OUT/dryrun_$v.txt 2>/dev/null
  paste -d' ' <(grep -o 'daddr: [0-9.]*' $OUT/dryrun_$v.txt) <(grep -o 'dest: [0-9]*' $OUT/dryrun_$v.txt) > $OUT/dryrun_$v.targets
  echo "$v $(wc -l < $OUT/dryrun_$v.txt) lines; full sha $(sha256sum < $OUT/dryrun_$v.txt | cut -c1-16); target-sequence sha $(sha256sum < $OUT/dryrun_$v.targets | cut -c1-16) ($(wc -l < $OUT/dryrun_$v.targets) targets)"
done
echo "== live loopback scans"
RATE=${RATE:-100000}
for cfg in "e1 1 1000000" "e2 2 1000000" "evict 2 65536" "evict1 1 65536"; do
  set -- $cfg
  for v in stock expanse stock+swap; do
    KEEP_OUTPUT=1 python3 /scripts/measure.py "corr-$1-$v" $v window $2 $RATE 4242 $3 $OUT/out
  done
done > $OUT/live.jsonl
python3 - <<'PY'
import json
rows=[json.loads(l) for l in open("/results/correctness/live.jsonl")]
keys=("rc","rows","out_sha256","success_unique","success_total","failure_total","validation_passed","pcap_drop","pcap_ifdrop","packets_sent")
for r in rows: print(r["arm"], {k:r.get(k) for k in keys})
PY
