#!/bin/bash
set -u
log() { echo "$(date -u +%FT%TZ) $*"; }
log 'lock acquired'
docker volume create oqeval-w >/dev/null
docker run --rm -i -v oqeval-w:/w oqeval:24.04 tar -C /w -xf - < ~/oqeval-bundle.tar && rm -f ~/oqeval-bundle.tar
docker run --rm -v oqeval-w:/w oqeval:24.04 cat /w/scripts/run_on_ref.sh > ~/oqeval-run_on_ref.sh
log 'init datadir'
docker run --rm --network none --cpuset-cpus=0-15 -v oqeval-w:/w oqeval:24.04 bash -c '. /w/scripts/prep_runtime.sh; python3 /w/driver/oqdrv.py init /w/data-bench /w/graphs-bench' || { log 'init failed'; exit 2; }
bash ~/oqeval-run_on_ref.sh
log ALL-DONE
