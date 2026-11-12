#!/usr/bin/env python3
"""Layout-only builds on the `sng_judy_1k` cell (README.md, "Diagnostic: layout-only builds").

A diagnostic outside METHODOLOGY.md's pre-registration (#1684). Four arms run
the same stress-ng binary and judy stressor and differ only in
`LD_LIBRARY_PATH`:
- N: the library of the pre-registered runs (`6ec29a656`);
- N64: the same source rebuilt with `-C llvm-args=-align-all-functions=6`, so
  every function starts on a 64-byte boundary;
- O64: the `82eb56af` source rebuilt with the same flag;
- S: stock libJudy.

Each round runs `sng_judy_1k` at every W, one process per arm. The rounds
rotate the arms through the four cyclic orders, so every arm runs in every
position equally often. Load windows and the provenance are `diag_lib_ab.py`'s.

Usage: diag_layout.py <run_number> <host_label> <out_prefix>
Environment: SNG_D, REPO_ROOT, SNG_LAYOUT_ROUNDS (default 12, a multiple of 4).
"""
import json
import os
import random
import sys
import time
from pathlib import Path

import diag_lib_ab
import driver

bp = driver.bp
ARMS = {"N": driver.ARMS["E"],
        "N64": f"{driver.D}/expcompat-6ec29a656-fa64/lib",
        "O64": f"{driver.D}/expcompat-82eb56af-fa64/lib",
        "S": driver.ARMS["S"]}
NAMES = list(ARMS)
ORDERS = [tuple(NAMES[(i + k) % len(NAMES)] for k in range(len(NAMES))) for i in range(len(NAMES))]
ROUNDS = int(os.environ.get("SNG_LAYOUT_ROUNDS", "12"))
CELL = "sng_judy_1k"


def library_proof():
    """Each Expanse arm maps its own libexpanse file, and S maps no Expanse object."""
    proof = {arm: driver.mapped_objects(arm, libdir) for arm, libdir in ARMS.items()}
    for arm in ("N", "N64", "O64"):
        if not any(x.startswith(ARMS[arm]) and "expanse" in x for x in proof[arm]):
            sys.exit(f"STOP: arm {arm} did not map libexpanse from {ARMS[arm]}: {proof}")
    if any("expanse" in x for x in proof["S"]):
        sys.exit(f"STOP: arm S mapped an Expanse object: {proof}")
    return proof


def main():
    run, host, prefix = int(sys.argv[1]), sys.argv[2], sys.argv[3]
    if ROUNDS % len(ORDERS):
        sys.exit(f"STOP: SNG_LAYOUT_ROUNDS={ROUNDS} is not a multiple of {len(ORDERS)}")
    driver.ARMS = ARMS
    diag_lib_ab.ARMS = ARMS
    driver.D = f"{driver.D}/diag_layout"
    os.makedirs(f"{driver.D}/out", exist_ok=True)
    prov = bp.new_provenance(
        "stressng_comparison", 1684,
        "N64/N, O64/N64 and N64/S: the ratio of two arms' means of stress-ng's own per-phase "
        "rate (or wall clock), with a two-sample BCa 95% interval over the run's rounds",
        repo_root=driver.REPO_ROOT, host_label=host, run=run, rounds=ROUNDS,
        diagnostic="layout-only library builds, outside METHODOLOGY.md's pre-registration",
        binary=driver.BIN, arms=ARMS, arm_orders=["-".join(o) for o in ORDERS],
        build_flag="-C llvm-args=-align-all-functions=6 (arms N64, O64 only)",
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
                    rec["order"] = "-".join(arms)
                    out.write(json.dumps(rec) + "\n")
                    out.flush()
                    if rec["rc"] != 0 or rec["failed"]:
                        sys.exit(f"STOP: non-zero exit or stress-ng failure in {cell} W{w} {arm}")
            print(f"{time.strftime('%H:%M:%S')} round {rnd} done", flush=True)
    bp.add_load(prov, "end")
    Path(f"{prefix}.provenance.json").write_text(json.dumps(prov, indent=2) + "\n")


if __name__ == "__main__":
    main()
