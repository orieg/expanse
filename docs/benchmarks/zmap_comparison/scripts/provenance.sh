#!/bin/sh
# Library provenance: what each binary links and what the loader actually maps.
set -u
echo "== dpkg stock judy"; dpkg-query -W -f='${Package} ${Version}\n' 'libjudy*'
echo "== gcc"; gcc --version | head -1
echo "== libexpanse"; ls -la /opt/expanse/lib /opt/expanse/compat; readelf -d /opt/expanse/lib/libexpanse.so | grep -E 'SONAME|NEEDED' || true
sha256sum /opt/expanse/lib/libexpanse.so /usr/lib/x86_64-linux-gnu/libJudy.so.1*
for b in /build/stock/src/zmap /build/expanse/src/zmap /build/stock/cachehash_bench /build/expanse/cachehash_bench; do
  echo "== $b"; readelf -d "$b" | grep -E 'NEEDED|RUNPATH|RPATH' | grep -i -E 'judy|expanse|PATH'; ldd "$b" | grep -i -E 'judy|expanse'
done
echo "== swap: stock zmap with LD_LIBRARY_PATH=/opt/expanse/compat"; LD_LIBRARY_PATH=/opt/expanse/compat ldd /build/stock/src/zmap | grep -i -E 'judy|expanse'
for v in stock expanse swap; do
  b=/build/stock/src/zmap; e=""
  [ $v = expanse ] && b=/build/expanse/src/zmap
  [ $v = swap ] && e="LD_LIBRARY_PATH=/opt/expanse/compat"
  echo "== LD_DEBUG=libs $v"; env $e LD_DEBUG=libs $b --version 2>&1 | grep -i -E 'calling init: .*(judy|expanse)|trying file=.*(Judy|expanse)' | head -4
done
echo "== JudyHS symbols resolved from"
for v in stock expanse swap; do
  b=/build/stock/src/zmap; e=""
  [ $v = expanse ] && b=/build/expanse/src/zmap
  [ $v = swap ] && e="LD_LIBRARY_PATH=/opt/expanse/compat"
  echo "-- $v"; env $e LD_DEBUG=bindings LD_BIND_NOW=1 $b --version 2>&1 | grep -E "symbol \`JudyHS" | sed 's/^ *[0-9]*: *//' | head -4
done
echo "== zmap undefined Judy symbols"; nm -D --undefined-only /build/stock/src/zmap | grep -i judy
echo "== governor"; cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor 2>/dev/null; cat /sys/devices/system/cpu/cpu2/cpufreq/scaling_governor 2>/dev/null
echo "== cpuset"; cat /sys/fs/cgroup/cpuset.cpus.effective 2>/dev/null; nproc
