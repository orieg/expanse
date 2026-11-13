#!/usr/bin/env python3
"""One engine increment against its merge base and stock, on every stress-ng
cell (docs/BENCHMARKING.md, the increment's pre-registration).

A wall-clock instrument for engine increments outside METHODOLOGY.md's
pre-registration. Three arms run the same stress-ng binary and differ only in
`LD_LIBRARY_PATH`:
- G: libexpanse built at the increment's head (`SNG_INC_HEAD`);
- M: libexpanse built at its merge base (`SNG_INC_BASE`);
- S: stock libJudy.

Every cell of METHODOLOGY.md §5 (`judy` at 1 Ki, 256 Ki and 4 Mi keys, W = 1,
2, 4, 8; `sparsematrix` judy and hashjudy), one process per arm per cell per
round. The rounds cycle through the six orders of the three arms, and each
round shuffles the cells. Load windows and provenance are `driver.py`'s.

Usage: diag_increment.py <run_number> <host_label> <out_prefix>
Environment: SNG_D, REPO_ROOT, SNG_INC_HEAD and SNG_INC_BASE (the commits
whose `build_layout_lib.sh` output sits at `$SNG_D/expcompat-<commit>/lib`),
SNG_INC_ISSUE, SNG_INC_ROUNDS (default 12, a multiple of 6).
"""
import itertools
import json
import os
import random
import sys
import time
from pathlib import Path

import driver

bp = driver.bp
HEAD, BASE = os.environ.get("SNG_INC_HEAD"), os.environ.get("SNG_INC_BASE")
if not HEAD or not BASE or HEAD == BASE:
    sys.exit("STOP: set SNG_INC_HEAD and SNG_INC_BASE to two different commits")
ARMS = {"G": f"{driver.D}/expcompat-{HEAD}/lib", "M": f"{driver.D}/expcompat-{BASE}/lib",
        "S": driver.ARMS["S"]}
ORDERS = list(itertools.permutations(ARMS))
ROUNDS = int(os.environ.get("SNG_INC_ROUNDS", "12"))
ISSUE = int(os.environ.get("SNG_INC_ISSUE", "0"))


def library_proof():
    """Each Expanse arm maps its own libexpanse file, and S maps no Expanse object."""
    proof = {arm: driver.mapped_objects(arm, libdir) for arm, libdir in ARMS.items()}
    for arm in ("G", "M"):
        if not any(x.startswith(ARMS[arm]) and "expanse" in x for x in proof[arm]):
            sys.exit(f"STOP: arm {arm} did not map libexpanse from {ARMS[arm]}: {proof}")
    if any("expanse" in x for x in proof["S"]):
        sys.exit(f"STOP: arm S mapped an Expanse object: {proof}")
    return proof


def main():
    run, host, prefix = int(sys.argv[1]), sys.argv[2], sys.argv[3]
    if ROUNDS % len(ORDERS):
        sys.exit(f"STOP: SNG_INC_ROUNDS={ROUNDS} is not a multiple of {len(ORDERS)}")
    if not ISSUE:
        sys.exit("STOP: set SNG_INC_ISSUE to the issue the increment is for")
    driver.ARMS = ARMS
    shas = {a: driver.sha256(f"{d}/libJudy.so.1") for a, d in ARMS.items()}
    if shas["G"] == shas["M"]:
        sys.exit("STOP: arms G and M are the same file")
    driver.D = f"{driver.D}/diag_increment"
    os.makedirs(f"{driver.D}/out", exist_ok=True)
    prov = bp.new_provenance(
        "stressng_comparison", ISSUE,
        "G/S, G/M and M/S: the ratio of two arms' means of stress-ng's own per-phase rate "
        "(or wall clock), with a two-sample BCa 95% interval over the run's rounds",
        repo_root=driver.REPO_ROOT, host_label=host, run=run, rounds=ROUNDS,
        diagnostic="an engine increment against its merge base and stock, outside METHODOLOGY.md's "
                   "pre-registration (docs/BENCHMARKING.md, the increment's pre-registration)",
        arm_set="increment", increment={"head": HEAD, "base": BASE},
        binary=driver.BIN, arms=ARMS, arm_orders=["".join(o) for o in ORDERS],
        cell_isolation="process",
        sha256={"binary": driver.sha256(driver.BIN), **{f"arm_{a}": s for a, s in shas.items()}},
        library_proof=library_proof())
    cs = driver.cells()
    with open(f"{prefix}.jsonl", "a") as out:
        for rnd in range(ROUNDS):
            order = list(cs)
            random.Random(run * 1000 + rnd).shuffle(order)
            arms = ORDERS[(rnd + run) % len(ORDERS)]
            for cell, w, args in order:
                for arm in arms:
                    rec = driver.run_one(prov, run, rnd, cell, w, args, arm)
                    rec["order"] = "".join(arms)
                    out.write(json.dumps(rec) + "\n")
                    out.flush()
                    if rec["rc"] != 0 or rec["failed"]:
                        sys.exit(f"STOP: non-zero exit or stress-ng failure in {cell} W{w} {arm}")
            print(f"{time.strftime('%H:%M:%S')} round {rnd} done", flush=True)
    bp.add_load(prov, "end")
    Path(f"{prefix}.provenance.json").write_text(json.dumps(prov, indent=2) + "\n")


if __name__ == "__main__":
    main()
