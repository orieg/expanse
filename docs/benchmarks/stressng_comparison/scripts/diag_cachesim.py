#!/usr/bin/env python3
"""Simulated-cache control for the 4 Mi phases' per-process state (#1679, #1680).

README.md, "Diagnostic: the 4 Mi phases under a simulated cache". Runs
`phase_probe` under Callgrind's cache simulator, with the reference host's L1D
and L2 geometry, once per (arm, shape, layout), where a layout is a value of
valgrind's `--aspace-minaddr` that moves every client mapping by whole pages.
Reads the inclusive simulated events of the API call the phase consists of
(`JudyLIns` for `judy_4m`, `JudyLDel` for `del_4m`) and the layout lines the
probe prints (`SNG_LAYOUT=1`). Deterministic: no timing, no host load.

Usage (inside a container with valgrind and cc):
  diag_cachesim.py <probe.c dir> <N lib dir> <S lib dir> <out.json> [jobs]
"""
import concurrent.futures as cf
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile

# Reference host (Intel Core i9-12900F, P-core): L1D 48 KiB 12-way, L2 1.25 MiB
# 10-way, 64-byte lines. L1I 32 KiB 8-way.
D1, LL, I1 = "49152,12,64", "1310720,10,64", "32768,8,64"
SHAPES = {"judy_4m": "JudyLIns", "del_4m": "JudyLDel"}
OPS = 4_194_304
# 24 layouts: the client's lowest address moved by k pages within a 2 MiB
# window and by whole 2 MiB steps, so mapping starts differ modulo 2 MiB.
LAYOUTS = [0x10000000 + k * 0x1000 + (k % 3) * 0x200000 for k in range(0, 24 * 21, 21)]


def index_span(geometry):
    """Bytes of address an L1-style set index spans: sets x line size.

    For the L1D geometry this is 49,152 / (12 x 64) = 64 sets x 64 B = 4,096 B,
    one 4 KiB page, so the simulated L1D set of an access depends only on its
    offset within the page, which a page-granular layout change preserves.
    """
    size, assoc, line = (int(x) for x in geometry.split(","))
    return size // (assoc * line) * line


assert index_span(D1) == 4096, "L1D index span is not one page"
assert index_span(LL) == 131072, "L2 index span changed"
EVENTS = ["Ir", "Dr", "Dw", "I1mr", "D1mr", "D1mw", "ILmr", "DLmr", "DLmw"]


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def one(probe, libdir, arm, shape, minaddr):
    with tempfile.TemporaryDirectory() as td:
        out = f"{td}/cg.out"
        env = dict(os.environ, LD_LIBRARY_PATH=libdir, SNG_LAYOUT="1")
        p = subprocess.run(["valgrind", "-q", "--tool=callgrind", "--cache-sim=yes",
                            f"--D1={D1}", f"--LL={LL}", f"--I1={I1}",
                            f"--aspace-minaddr={minaddr:#x}", f"--callgrind-out-file={out}",
                            probe, shape, "1", "-", "-"], env=env, capture_output=True, text=True)
        if p.returncode != 0:
            sys.exit(f"STOP: {arm} {shape} {minaddr:#x} exited {p.returncode}: {p.stderr[-600:]}")
        layout = [ln.split()[1:] for ln in p.stdout.splitlines() if ln.startswith("layout ")]
        if not any(x[0] == "map" for x in layout):
            sys.exit(f"STOP: no layout lines for {arm} {shape} {minaddr:#x}")
        ann = subprocess.run(["callgrind_annotate", "--inclusive=yes", "--threshold=100", out],
                             capture_output=True, text=True, check=True).stdout
    fn = SHAPES[shape]
    rows = [ln for ln in ann.splitlines() if re.search(rf"[:\s]{fn}(\s|\[|$)", ln)]
    if len(rows) != 1:
        sys.exit(f"STOP: {len(rows)} inclusive rows for {fn} on {arm} {shape} {minaddr:#x}")
    nums = [int(x.replace(",", "")) for x in re.findall(r"([\d,]+)\s+\(", rows[0])]
    if len(nums) != len(EVENTS):
        sys.exit(f"STOP: {len(nums)} event columns for {fn}: {rows[0]}")
    return dict(arm=arm, shape=shape, minaddr=minaddr, layout=layout,
                events=dict(zip(EVENTS, nums)), per_op={e: v / OPS for e, v in zip(EVENTS, nums)})


def main():
    src, libn, libs, dest = sys.argv[1:5]
    jobs = int(sys.argv[5]) if len(sys.argv) > 5 else 8
    probe = os.path.join(tempfile.mkdtemp(), "phase_probe")
    subprocess.run(["cc", "-std=c11", "-O2", "-Wall", "-Werror", f"-I{src}", f"{src}/phase_probe.c",
                    "-o", probe, f"{libs}/libJudy.so.1"], check=True)
    arms = {"N": libn, "S": libs}
    work = [(a, s, m) for a in arms for s in SHAPES for m in LAYOUTS]
    with cf.ThreadPoolExecutor(jobs) as ex:
        cells = list(ex.map(lambda w: one(probe, arms[w[0]], *w), work))
    vg = subprocess.run(["valgrind", "--version"], capture_output=True, text=True).stdout.strip()
    cc = subprocess.run(["cc", "--version"], capture_output=True, text=True).stdout.splitlines()[0]
    prov = dict(diagnostic="4 Mi phases under a simulated cache, outside METHODOLOGY.md's pre-registration",
                instrument=f"{vg} callgrind --cache-sim=yes --D1={D1} --LL={LL} --I1={I1}",
                compiler=cc, ops=OPS, layouts=[hex(m) for m in LAYOUTS], shapes=SHAPES,
                sha256={"probe_src": sha(f"{src}/phase_probe.c"), "probe": sha(probe),
                        "arm_N": sha(f"{libn}/libJudy.so.1"), "arm_S": sha(f"{libs}/libJudy.so.1")})
    with open(dest, "w") as f:
        json.dump({"provenance": prov, "cells": cells}, f, indent=1)
        f.write("\n")
    for a in arms:
        for s in SHAPES:
            v = [c["per_op"]["D1mr"] for c in cells if c["arm"] == a and c["shape"] == s]
            maps = {json.dumps([x for x in c["layout"] if x[0] == "map"]) for c in cells
                    if c["arm"] == a and c["shape"] == s}
            print(f"{a} {s}: layouts {len(maps)}, D1mr/op {min(v):.4f}-{max(v):.4f}")


if __name__ == "__main__":
    main()
