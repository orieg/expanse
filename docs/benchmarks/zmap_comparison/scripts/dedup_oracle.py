#!/usr/bin/env python3
"""Exact oracle for zmap's window dedup on one run's own packet stream.

Replays the output CSV in arrival order through a reference LRU with the
semantics of zmap lib/cachehash.c (get hit -> move to front, repeat=1; miss ->
put, evicting the least recently used entry when full) keyed on
(saddr, sport), and compares zmap's `repeat` column row by row.
usage: dedup_oracle.py <csv> <window>
"""
import sys
from collections import OrderedDict

path, window = sys.argv[1], int(sys.argv[2])
lru = OrderedDict()
rows = mism = repeats = 0
first = None
with open(path) as f:
    hdr = f.readline().strip().split(",")
    i_ip, i_port, i_rep = hdr.index("saddr"), hdr.index("sport"), hdr.index("repeat")
    for line in f:
        c = line.rstrip("\n").split(",")
        k = (c[i_ip], c[i_port])
        if k in lru:
            lru.move_to_end(k)
            exp = 1
        else:
            if len(lru) == window:
                lru.popitem(last=False)
            lru[k] = True
            exp = 0
        got = int(c[i_rep])
        rows += 1; repeats += got
        if got != exp:
            mism += 1
            if first is None:
                first = (rows, line.strip(), exp)
print(f"{path.split('/')[-1]} window={window} rows={rows} repeats={repeats} mismatches={mism} first={first}")
sys.exit(1 if mism else 0)
