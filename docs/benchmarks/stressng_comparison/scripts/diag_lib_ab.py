#!/usr/bin/env python3
"""Old-vs-new libexpanse A/B on the `sng_judy_1k` cell (README.md, "Diagnostic").

A diagnostic outside METHODOLOGY.md's pre-registration. Three arms run the same
stress-ng binary and judy stressor and differ only in `LD_LIBRARY_PATH`:
O is libexpanse built at `82eb56af` (the library of the earlier evaluation),
N is the library of the pre-registered runs, and S is stock libJudy. Each
round runs `sng_judy_1k` at every W, one process per arm, and the rounds cycle
through the six orders of the three arms, so every arm runs first, second and
third equally often. `bench_provenance` takes a load snapshot before every
arm's process and closes the window after it, so each pair of adjacent arms
has a snapshot between them.

Usage: diag_lib_ab.py <run_number> <host_label> <out_prefix>
Environment: SNG_D, REPO_ROOT, SNG_OLD_LIB (default `$SNG_D/expcompat-82eb56af/lib`),
SNG_DIAG_ROUNDS (default 12, a multiple of 6).
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
OLD = os.environ.get("SNG_OLD_LIB", f"{driver.D}/expcompat-82eb56af/lib")
ARMS = {"O": OLD, "N": driver.ARMS["E"], "S": driver.ARMS["S"]}
ORDERS = list(itertools.permutations(ARMS))
ROUNDS = int(os.environ.get("SNG_DIAG_ROUNDS", "12"))
CELL = "sng_judy_1k"


def library_proof():
    """O and N each map their own libexpanse file, and S maps no Expanse object."""
    proof = {arm: driver.mapped_objects(arm, libdir) for arm, libdir in ARMS.items()}
    for arm in ("O", "N"):
        if not any(x.startswith(ARMS[arm]) and "expanse" in x for x in proof[arm]):
            sys.exit(f"STOP: arm {arm} did not map libexpanse from {ARMS[arm]}: {proof}")
    if any("expanse" in x for x in proof["S"]):
        sys.exit(f"STOP: arm S mapped an Expanse object: {proof}")
    return proof


def main():
    run, host, prefix = int(sys.argv[1]), sys.argv[2], sys.argv[3]
    if ROUNDS % len(ORDERS):
        sys.exit(f"STOP: SNG_DIAG_ROUNDS={ROUNDS} is not a multiple of {len(ORDERS)}")
    # The arms and the per-process output directory of this diagnostic, so its
    # YAML never overwrites the pre-registered runs' files.
    driver.ARMS = ARMS
    driver.D = f"{driver.D}/diag_lib_ab"
    os.makedirs(f"{driver.D}/out", exist_ok=True)
    prov = bp.new_provenance(
        "stressng_comparison", 1666,
        "O/N, N/S and O/S: the ratio of two arms' means of stress-ng's own per-phase rate "
        "(or wall clock), with a two-sample BCa 95% interval over the run's rounds",
        repo_root=driver.REPO_ROOT, host_label=host, run=run, rounds=ROUNDS,
        diagnostic="old-vs-new libexpanse A/B, outside METHODOLOGY.md's pre-registration",
        binary=driver.BIN, arms=ARMS, arm_orders=["".join(o) for o in ORDERS],
        cell_isolation="process",
        sha256={"binary": driver.sha256(driver.BIN),
                **{f"arm_{a}": driver.sha256(f"{d}/libJudy.so.1") for a, d in ARMS.items()}},
        library_proof=library_proof())
    cs = [c for c in driver.cells() if c[0] == CELL]
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
