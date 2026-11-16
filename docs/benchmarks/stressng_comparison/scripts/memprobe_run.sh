#!/bin/bash
# inside ubuntu:22.04, /w mounted: build memprobe against the stock Judy.h, run under both libraries.
set -eu
apt-get update -qq >/dev/null && apt-get install -y -qq gcc libc6-dev >/dev/null 2>&1
cd /w
gcc -O2 -I/w/stock/usr/include memprobe.c -L/w/stock/usr/lib/x86_64-linux-gnu -lJudy -o memprobe
SL=/w/stock/usr/lib/x86_64-linux-gnu; EL=/w/expcompat/lib
for shape in "100000 1 500" "100000 20 500" "100000 10 100000" "10000 1000 100000" "100 100000 4294967295"; do
  for arm in S:$SL E:$EL; do
    printf '%s ' "${arm%%:*}"; LD_LIBRARY_PATH=${arm#*:} ./memprobe $shape
  done
done
