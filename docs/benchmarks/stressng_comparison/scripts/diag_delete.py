#!/usr/bin/env python3
"""The #1679 delete fix on the stress-ng `judy` cells (README.md, "Diagnostic: delete fix").

A diagnostic outside METHODOLOGY.md's pre-registration (#1679). Three arms run
the same stress-ng binary and judy stressor and differ only in
`LD_LIBRARY_PATH`:
- B: libexpanse built at #1708's merge base (`99ef70855`);
- D: libexpanse built at #1708's head (`45f83ac5e`), the removal-only lower
  bound on a `BranchU`'s live slots;
- S: stock libJudy.

With `SNG_DELETE_SET=main` the same driver re-measures the `judy` cells against
stock on main after #1708, #1719 and #1720 (README.md, "The #1679 criterion on
main"), with the arms:
- D: the same #1708-head library, the one the delete-fix diagnostic published;
- N: libexpanse built at `5f4892932` (#1720's merge commit; the library source
  is unchanged from there to the design commit);
- S: stock libJudy.

With `SNG_DELETE_SET=sm_main` it re-measures the `sparsematrix judy` cells
against stock on main (README.md, "The sparsematrix cells on main", #1680),
with the arms:
- E: the library of the published comparison (`6ec29a656`, the reference
  host's `expcompat/lib`);
- N: the same main library as `main`;
- S: stock libJudy.

With `SNG_DELETE_SET=hj_main` it re-measures the `sparsematrix hashjudy` cells
against stock on main (README.md, "The hashjudy cells on main"), with the arms
E and S of `sm_main` and N built at `aacc1add4` (HJ_MAIN). `hj_leak` runs the
same cells on four arms: N and S on `b-hjctl`, Nf and Sf on `b-hjfree`, two
builds of the harness's stress-ng source in one container run, the second
with `JLFA` added to `hashjudy_destroy` (`scripts/build_hjfree.sh`).
stress-ng's own `hashjudy_destroy` calls `JLMU` on each array and never
`JLFA`. `hj_perf` runs `hj_leak`'s arms under
`perf stat` (PERF_EVENTS), then one `perf record` per cell and arm for the
share of cycles per object.

Every `sng_judy_*` cell of METHODOLOGY.md §5 (every `sng_sm_judy_*` cell for
`sm_main`, every `sng_sm_hashjudy_*` cell for the `hj_*` sets) (1 Ki, 256 Ki
and 4 Mi keys, W = 1, 2, 4, 8), one process per arm per cell per round. The
rounds cycle through the orders of the arms. Load windows and the provenance
are `diag_lib_ab.py`'s.

Usage: diag_delete.py <run_number> <host_label> <out_prefix>
Environment: SNG_D, REPO_ROOT, SNG_DELETE_ROUNDS (default 12 for three arms and
24 for four, a multiple of the number of orders), SNG_DELETE_SET (`fix`, the
default, `main`, `sm_main`, `hj_main`, `hj_leak` or `hj_perf`).
"""
import itertools
import json
import os
import random
import re
import subprocess
import sys
import time
from pathlib import Path

import driver

bp = driver.bp
# The main library of the hashjudy sets: libexpanse built from a `git archive`
# of main at this commit (scripts/build_layout_lib.sh).
HJ_MAIN = "aacc1add4"
HJ_N = f"{driver.D}/expcompat-{HJ_MAIN}/lib"
# The leak intervention's two stress-ng builds (scripts/build_hjfree.sh): the
# harness's source unchanged, and the same source with `JLFA` added to
# hashjudy_destroy, built in one container run.
HJ_CTL_BIN = f"{driver.D}/b-hjctl/stress-ng"
HJ_FREE_BIN = f"{driver.D}/b-hjfree/stress-ng"
HJ_FOUR = {"N": HJ_N, "S": driver.ARMS["S"], "Nf": HJ_N, "Sf": driver.ARMS["S"]}
# Whole-process counts on the pin's PMU, user and kernel mode separately, and
# the kernel's software fault counters.
PERF_EVENTS = ["cpu_core/cycles/u", "cpu_core/instructions/u",
               "cpu_core/cycles/k", "cpu_core/instructions/k",
               "page-faults", "minor-faults", "major-faults"]
SETS = {
    "fix": {"B": f"{driver.D}/expcompat-99ef70855/lib",
            "D": f"{driver.D}/expcompat-45f83ac5e/lib",
            "S": driver.ARMS["S"]},
    "main": {"D": f"{driver.D}/expcompat-45f83ac5e/lib",
             "N": f"{driver.D}/expcompat-5f4892932/lib",
             "S": driver.ARMS["S"]},
    "sm_main": {"E": driver.ARMS["E"],
                "N": f"{driver.D}/expcompat-5f4892932/lib",
                "S": driver.ARMS["S"]},
    "hj_main": {"E": driver.ARMS["E"], "N": HJ_N, "S": driver.ARMS["S"]},
    "hj_leak": HJ_FOUR,
    "hj_perf": HJ_FOUR,
}
CELL_PREFIX = {"fix": "sng_judy_", "main": "sng_judy_", "sm_main": "sng_sm_judy_",
               "hj_main": "sng_sm_hashjudy_", "hj_leak": "sng_sm_hashjudy_",
               "hj_perf": "sng_sm_hashjudy_"}
HJ_FOUR_BIN = {"N": HJ_CTL_BIN, "S": HJ_CTL_BIN, "Nf": HJ_FREE_BIN, "Sf": HJ_FREE_BIN}
ARM_BIN = {"hj_leak": HJ_FOUR_BIN, "hj_perf": HJ_FOUR_BIN}
ISSUE = {"fix": 1679, "main": 1679, "sm_main": 1680,
         "hj_main": 1666, "hj_leak": 1666, "hj_perf": 1666}
RATIO = {"fix": "D/B, D/S and B/S", "main": "N/S, N/D and D/S", "sm_main": "N/S, N/E and E/S",
         "hj_main": "N/S, N/E and E/S",
         "hj_leak": "N/S, Nf/Sf, N/Nf and S/Sf, and the leak factor (N/S)/(Nf/Sf)",
         "hj_perf": "N/S, Nf/Sf, N/Nf and S/Sf, and the leak factor (N/S)/(Nf/Sf)"}
DIAGNOSTIC = {"fix": "the #1679 delete fix",
              "main": "the #1679 criterion re-measured on main",
              "sm_main": "the sparsematrix judy cells re-measured on main (#1680)",
              "hj_main": "the sparsematrix hashjudy cells re-measured on main",
              "hj_leak": "the hashjudy cells with and without stress-ng's leak",
              "hj_perf": "the hashjudy cells' whole-process counters, with and without the leak"}
SET = os.environ.get("SNG_DELETE_SET", "fix")
if SET not in SETS:
    sys.exit(f"STOP: unknown SNG_DELETE_SET={SET}")
ARMS = SETS[SET]
ORDERS = list(itertools.permutations(ARMS))
ROUNDS = int(os.environ.get("SNG_DELETE_ROUNDS", "24" if len(ARMS) == 4 else "12"))
STOCK = driver.ARMS["S"]
PERF = SET == "hj_perf"
KERNEL_ADDR = re.compile(r"\[k\] 0x[0-9a-f]{8,16}")


def library_proof():
    """Each Expanse arm maps its own libexpanse file, and each stock arm maps no Expanse object."""
    proof = {arm: driver.mapped_objects(arm, libdir) for arm, libdir in ARMS.items()}
    for arm in (a for a in ARMS if ARMS[a] != STOCK):
        if not any(x.startswith(ARMS[arm]) and "expanse" in x for x in proof[arm]):
            sys.exit(f"STOP: arm {arm} did not map libexpanse from {ARMS[arm]}: {proof}")
    for arm in (a for a in ARMS if ARMS[a] == STOCK):
        if any("expanse" in x for x in proof[arm]):
            sys.exit(f"STOP: arm {arm} mapped an Expanse object: {proof}")
    return proof


def check_counters(rec, where):
    """Every event printed a count at 100% running; user and kernel cycles and faults are non-zero."""
    for e in PERF_EVENTS:
        if rec["counters"].get(e) is None:
            sys.exit(f"STOP: {where}: {e} was not counted: {rec['counters']}")
        pct = rec["counter_pct_running"].get(e)
        if pct is None or pct < 100.0:
            sys.exit(f"STOP: {where}: {e} ran {pct}% of the time (multiplexed)")
    for e in ("cpu_core/cycles/u", "cpu_core/cycles/k", "page-faults"):
        if not rec["counters"][e]:
            sys.exit(f"STOP: {where}: {e} read zero")


def perf_preflight(cells):
    """One untimed process of the first cell on arm N under `perf stat`, checked as every round is."""
    cell, w, args = cells[0]
    rec = driver.run_one({}, 0, "pre", cell, w, args, "N", perf_events=PERF_EVENTS)
    if rec["rc"] != 0:
        sys.exit(f"STOP: perf preflight exited {rec['rc']}: {rec.get('log_tail')}")
    check_counters(rec, "preflight")
    return {"cell": cell, "w": w, "counters": rec["counters"],
            "pct_running": rec["counter_pct_running"]}


def object_shares(cells):
    """Share of cycles (user and kernel) per object, one `perf record` per (cell, W, arm)."""
    outdir = f"{driver.D}/record"
    os.makedirs(outdir, exist_ok=True)
    shares = {}
    for cell, w, args in cells:
        for arm in ARMS:
            data = f"{outdir}/{cell}_w{w}_{arm}.data"
            env = dict(os.environ, LD_LIBRARY_PATH=ARMS[arm])
            subprocess.run(["perf", "record", "-q", "-e", "cpu_core/cycles/", "-c", "200003",
                            "-o", data, "--", driver.binary(arm)] + args
                           + ["--seed", driver.SEED, "-t", "300"],
                           env=env, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            rows = {}
            for sort, key in (("dso", "by_object"), ("dso,sym", "by_symbol")):
                out = subprocess.run(["perf", "report", "-i", data, "--stdio", "--no-children",
                                      "--sort", sort, "--percent-limit", "0.5", "-q"],
                                     check=True, capture_output=True, text=True).stdout
                # Kernel samples resolve to raw kernel addresses here, which
                # reveal the kernel's randomised base; they are withheld.
                rows[key] = [{"pct": float(p[0][:-1]),
                              "row": KERNEL_ADDR.sub("[k] <kernel address withheld>", " ".join(p[1:]))}
                             for p in (line.split() for line in out.splitlines())
                             if len(p) >= 2 and p[0].endswith("%")]
                if not rows[key]:
                    sys.exit(f"STOP: perf report --sort {sort} gave no rows for {cell} W{w} {arm}")
            shares[f"{cell}:W{w}:{arm}"] = rows
    return shares


def main():
    run, host, prefix = int(sys.argv[1]), sys.argv[2], sys.argv[3]
    if ROUNDS % len(ORDERS):
        sys.exit(f"STOP: SNG_DELETE_ROUNDS={ROUNDS} is not a multiple of {len(ORDERS)}")
    driver.ARMS = ARMS
    driver.ARM_BIN = ARM_BIN.get(SET, {})
    driver.D = f"{driver.D}/diag_delete" + ("" if SET == "fix" else f"_{SET}")
    os.makedirs(f"{driver.D}/out", exist_ok=True)
    extra = {}
    if driver.ARM_BIN:
        extra["arm_binaries"] = {a: driver.binary(a) for a in ARMS}
        extra["sha256_arm_binaries"] = {a: driver.sha256(driver.binary(a)) for a in ARMS}
    if PERF:
        extra["perf_version"] = subprocess.run(["perf", "--version"], capture_output=True,
                                               text=True).stdout.strip()
        extra["perf_events"] = PERF_EVENTS
        extra["pmu"] = "cpu_core"
    prov = bp.new_provenance(
        "stressng_comparison", ISSUE[SET],
        RATIO[SET]
        + (": the ratio of two arms' means of one whole-process counter "
           if PERF else ": the ratio of two arms' means of stress-ng's own per-phase rate ")
        + "(or wall clock, or max-rss), with a BCa 95% interval over the run's rounds",
        repo_root=driver.REPO_ROOT, host_label=host, run=run, rounds=ROUNDS,
        diagnostic=DIAGNOSTIC[SET] + ", outside METHODOLOGY.md's pre-registration",
        arm_set=SET,
        binary=driver.BIN, arms=ARMS, arm_orders=["".join(o) for o in ORDERS],
        cell_isolation="process",
        sha256={"binary": driver.sha256(driver.BIN),
                **{f"arm_{a}": driver.sha256(f"{d}/libJudy.so.1") for a, d in ARMS.items()}},
        library_proof=library_proof(), **extra)
    cs = [c for c in driver.cells() if c[0].startswith(CELL_PREFIX[SET])]
    if PERF:
        prov["perf_preflight"] = perf_preflight(cs)
    with open(f"{prefix}.jsonl", "a") as out:
        for rnd in range(ROUNDS):
            order = list(cs)
            random.Random(run * 1000 + rnd).shuffle(order)
            arms = ORDERS[(rnd + run) % len(ORDERS)]
            for cell, w, args in order:
                for arm in arms:
                    rec = driver.run_one(prov, run, rnd, cell, w, args, arm,
                                         perf_events=PERF_EVENTS if PERF else None)
                    rec["order"] = "".join(arms)
                    out.write(json.dumps(rec) + "\n")
                    out.flush()
                    if rec["rc"] != 0 or rec["failed"]:
                        sys.exit(f"STOP: non-zero exit or stress-ng failure in {cell} W{w} {arm}")
                    if PERF:
                        check_counters(rec, f"{cell} W{w} {arm} r{rnd}")
            print(f"{time.strftime('%H:%M:%S')} round {rnd} done", flush=True)
    if PERF:
        start = bp.begin_cell(prov, "object_shares")
        prov["object_shares"] = object_shares(cs)
        prov["object_shares_load"] = bp.end_cell(start)
    bp.add_load(prov, "end")
    Path(f"{prefix}.provenance.json").write_text(json.dumps(prov, indent=2) + "\n")


if __name__ == "__main__":
    main()
