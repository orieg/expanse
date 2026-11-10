#!/usr/bin/env python3
"""The #1679 delete fix on the stress-ng `judy` cells (README.md, "Diagnostic: delete fix").

A diagnostic outside METHODOLOGY.md's pre-registration (#1679). Three arms run
the same stress-ng binary and judy stressor and differ only in
`LD_LIBRARY_PATH`:
- B: libexpanse built at #1708's merge base (`99ef70855`);
- D: libexpanse built at #1708's head (`dd12b6e0e`), the removal-only lower
  bound on a `BranchU`'s live slots;
- S: stock libJudy.

Every `sng_judy_*` cell of METHODOLOGY.md §5 (1 Ki, 256 Ki and 4 Mi keys, W =
1, 2, 4, 8), one process per arm per cell per round. The rounds cycle through
the six orders of the three arms. Load windows and the provenance are
`diag_lib_ab.py`'s.

Usage: diag_delete.py <run_number> <host_label> <out_prefix>
Environment: SNG_D, REPO_ROOT, SNG_DELETE_ROUNDS (default 12, a multiple of 6).
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
ARMS = {"B": f"{driver.D}/expcompat-99ef70855/lib",
        "D": f"{driver.D}/expcompat-dd12b6e0e/lib",
        "S": driver.ARMS["S"]}
ORDERS = list(itertools.permutations(ARMS))
ROUNDS = int(os.environ.get("SNG_DELETE_ROUNDS", "12"))


def library_proof():
    """B and D each map their own libexpanse file, and S maps no Expanse object."""
    proof = {arm: driver.mapped_objects(arm, libdir) for arm, libdir in ARMS.items()}
    for arm in ("B", "D"):
        if not any(x.startswith(ARMS[arm]) and "expanse" in x for x in proof[arm]):
            sys.exit(f"STOP: arm {arm} did not map libexpanse from {ARMS[arm]}: {proof}")
    if any("expanse" in x for x in proof["S"]):
        sys.exit(f"STOP: arm S mapped an Expanse object: {proof}")
    return proof


def main():
    run, host, prefix = int(sys.argv[1]), sys.argv[2], sys.argv[3]
    if ROUNDS % len(ORDERS):
        sys.exit(f"STOP: SNG_DELETE_ROUNDS={ROUNDS} is not a multiple of {len(ORDERS)}")
    driver.ARMS = ARMS
    driver.D = f"{driver.D}/diag_delete"
    os.makedirs(f"{driver.D}/out", exist_ok=True)
    prov = bp.new_provenance(
        "stressng_comparison", 1679,
        "D/B, D/S and B/S: the ratio of two arms' means of stress-ng's own per-phase rate "
        "(or wall clock), with a two-sample BCa 95% interval over the run's rounds",
        repo_root=driver.REPO_ROOT, host_label=host, run=run, rounds=ROUNDS,
        diagnostic="the #1679 delete fix, outside METHODOLOGY.md's pre-registration",
        binary=driver.BIN, arms=ARMS, arm_orders=["".join(o) for o in ORDERS],
        cell_isolation="process",
        sha256={"binary": driver.sha256(driver.BIN),
                **{f"arm_{a}": driver.sha256(f"{d}/libJudy.so.1") for a, d in ARMS.items()}},
        library_proof=library_proof())
    cs = [c for c in driver.cells() if c[0].startswith("sng_judy_")]
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
