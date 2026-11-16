#!/bin/bash
# Sourced inside oqeval:24.04 before any server run. Models a host where stock
# libjudy is installed runtime-only (libJudy.so.1, what variant A needs) and
# libexpanse is installed with a libJudy.so link (what the B build recorded as
# NEEDED). The stock unversioned dev link would otherwise capture variant B.
rm -f /usr/lib/x86_64-linux-gnu/libJudy.so /usr/lib/x86_64-linux-gnu/libJudy.a
cp /w/prefix-expanse/lib/libexpanse.so /usr/local/lib/
ln -sf libexpanse.so /usr/local/lib/libJudy.so
ldconfig
for V in A B C; do echo "ldd $V: $(ldd /w/plugins/$V/ha_oqgraph.so | grep -E 'Judy|expanse')"; done
