#!/usr/bin/env python3
"""Counters on one phase of the stress-ng shapes Expanse loses (README.md, "Diagnostic: phase counters").

A diagnostic outside METHODOLOGY.md's pre-registration (#1680). `phase_probe`
replays a stressor's key shape through the Judy API and enables `perf stat`
only around the measured phase: the sparsematrix gets (`sm_default`,
`sm_big`) or the 4 Mi `gen_index` inserts (`judy_4m`). Three arms differ only
in `LD_LIBRARY_PATH`: N (the library of the pre-registered runs), N64 (its
source rebuilt with 64-byte function alignment, #1684) and S (stock libJudy).

One process per (round, shape, event group, arm). Groups hold at most four
general-purpose events beside the fixed `cycles` and `instructions`, and a
preflight stops on an event that counts alone and reads zero in its group, or
on any row below 100% running (#1177). Every count is user-mode on the
`cpu_core` PMU.

Usage: diag_phase_counters.py <run_number> <host_label> <out_prefix>
Environment: SNG_D, REPO_ROOT, SNG_PHASE_ROUNDS (default 12, a multiple of 6).
"""
import itertools
import json
import os
import random
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import diag_find_counters as dfc
import driver

bp = driver.bp
# `SNG_PHASE_SET=delete` selects the #1679 delete-phase design (README.md,
# "Diagnostic: delete-phase counters"): D, #1708's library, against stock.
SET = os.environ.get("SNG_PHASE_SET", "gets")
# `delete_fe` is the same design with the 1k find diagnostic's front-end
# groups (README.md, "Diagnostic: front-end resource on the delete phase");
# `delete_ms` has the taken-branch and microcode groups (README.md,
# "Diagnostic: taken branches and microcode on the delete phase").
# `delete_acct` is the accounting ablation (README.md, "Diagnostic: unlocked
# accounting on the delete phase"): A and D, one commit built with and without
# `ablation-unlocked-plain-acct` (retired since the promotion; built from its
# own commit, `cc924b150`), against stock.
# `promote2` is the timed promotion of the unlocked plain-tree accounting
# (README.md, "Promotion, judged on timing"): P (the promotion's head), M (its
# merge base) and S (stock), on the two delete shapes it targets and the four
# shapes whose Callgrind counts it raises.
if SET == "promote2":
    ARMS = {"P": f"{driver.D}/expcompat-1679p2-P/lib", "M": f"{driver.D}/expcompat-1679p2-M/lib",
            "S": driver.ARMS["S"]}
    # (shape, repeats, measured operations per process): repeats sized so a
    # process's phase lasts a few tenths of a second, above the load window's
    # minimum (`bench_provenance.MIN_WINDOW_S`).
    SHAPES = [("del_1k", 10_000, 10_000 * 1024), ("del_4m", 1, 4_194_304),
              ("osc_1", 30, 30 * 50_000), ("j1s_clustered", 400, 400 * 50_000),
              ("j1s_sparse", 150, 150 * 50_000), ("ins_1k", 6_000, 6_000 * 1024)]
    PREFLIGHT = ("del_4m", 1, "M")
elif SET in ("delete", "delete_fe", "delete_ms", "delete_acct"):
    ARMS = {"D": f"{driver.D}/expcompat-45f83ac5e/lib", "S": driver.ARMS["S"]}
    if SET == "delete_acct":
        ARMS = {"A": f"{driver.D}/expcompat-1679acct-A/lib", "D": f"{driver.D}/expcompat-1679acct-D/lib",
                "S": driver.ARMS["S"]}
    # (shape, repeats of the measured phase, measured operations per process)
    # 10,000 repetitions: at 1,000 a process lasted 0.085 s, under the load
    # window's minimum (`bench_provenance.MIN_WINDOW_S`, 0.1 s), and every
    # process read no foreign load and was discarded.
    SHAPES = [("del_1k", 10_000, 10_000 * 1024), ("del_4m", 1, 4_194_304)]
    PREFLIGHT = ("del_4m", 1, "D")
elif SET == "gets":
    ARMS = {"N": driver.ARMS["E"], "N64": f"{driver.D}/expcompat-6ec29a656-fa64/lib", "S": driver.ARMS["S"]}
    SHAPES = [("sm_default", 200, 200 * 20_000), ("sm_big", 1, 2_000_000), ("judy_4m", 1, 4_194_304)]
    PREFLIGHT = ("sm_big", 1, "N")
elif SET == "gets_main":
    # The #1680 shapes on main (README.md, "Diagnostic: where the #1680
    # shapes mispredict and miss"): N is main at `5f4892932`, the library of
    # "The #1679 criterion on main", against stock.
    ARMS = {"N": f"{driver.D}/expcompat-5f4892932/lib", "S": driver.ARMS["S"]}
    SHAPES = [("sm_default", 200, 200 * 20_000), ("sm_big", 1, 2_000_000), ("judy_4m", 1, 4_194_304)]
    PREFLIGHT = ("sm_big", 1, "N")
else:
    sys.exit(f"STOP: unknown SNG_PHASE_SET {SET}")
ORDERS = list(itertools.permutations(ARMS))
ROUNDS = int(os.environ.get("SNG_PHASE_ROUNDS", "12"))
FIXED = ["cycles", "instructions"]
GROUPS = {
    "mem": ["mem_load_retired.l1_miss", "mem_load_retired.l2_miss",
            "mem_load_retired.l3_miss", "br_misp_retired.all_branches"],
    "stall": ["cycle_activity.stalls_total", "cycle_activity.stalls_l1d_miss",
              "cycle_activity.stalls_l2_miss"],
    # Alone: beside the other three it read 0 where it counts alone (#1177).
    "l3": ["cycle_activity.stalls_l3_miss"],
    # `slots` is the fixed TOPDOWN.SLOTS counter.
    "fe": ["slots", "idq_uops_not_delivered.core", "dtlb_load_misses.walk_completed",
           "br_inst_retired.all_branches"],
}
if SET == "delete_fe":
    GROUPS = dfc.GROUPS
elif SET == "delete_ms":
    GROUPS = {
        "ms": ["idq.ms_uops", "idq.ms_switches", "idq.ms_cycles_any", "br_inst_retired.near_taken"],
        "dsb": ["idq.dsb_cycles_any", "idq.dsb_cycles_ok", "idq.dsb_uops", "uops_issued.any"],
        "fe": ["slots", "idq_uops_not_delivered.core", "br_inst_retired.all_branches", "idq.mite_uops"],
    }
elif SET == "promote2":
    GROUPS = {"p2": ["mem_inst_retired.lock_loads", "cycle_activity.stalls_total",
                     "idq_uops_not_delivered.core"]}
elif SET == "delete_acct":
    GROUPS = {"acct": ["slots", "idq_uops_not_delivered.core", "cycle_activity.stalls_total",
                       "br_inst_retired.near_taken"]}
GP_EXEMPT = {"slots"}
PROBE = None


def build_probe():
    """Compiles phase_probe.c against this checkout's clean-room Judy.h."""
    out = f"{driver.D}/diag_phase_counters/phase_probe"
    os.makedirs(os.path.dirname(out), exist_ok=True)
    src = Path(__file__).resolve().parent / "phase_probe.c"
    subprocess.run(["cc", "-std=c11", "-O2", "-Wall", "-Werror", f"-I{driver.REPO_ROOT}/include",
                    str(src), "-o", out, f"{ARMS['S']}/libJudy.so.1"], check=True)
    return out


def perf_phase(events, shape, repeats, arm, base):
    """One probe process under `perf stat`, counters enabled only around its phase."""
    with tempfile.TemporaryDirectory() as td:
        ctl, ack = f"{td}/ctl", f"{td}/ack"
        os.mkfifo(ctl)
        os.mkfifo(ack)
        env = dict(os.environ, LD_LIBRARY_PATH=ARMS[arm])
        cmd = ["perf", "stat", "-x,", "--no-big-num", "-D", "-1", "--control", f"fifo:{ctl},{ack}",
               "-o", base + ".csv", "-e", ",".join(dfc.ev(e) for e in events), "--",
               PROBE, shape, str(repeats), ctl, ack]
        t0 = time.monotonic()
        with open(base + ".log", "w") as lf:
            p = subprocess.Popen(cmd, env=env, stdout=lf, stderr=subprocess.STDOUT)
            _, status, ru = os.wait4(p.pid, 0)
        wall = time.monotonic() - t0
    rc = os.waitstatus_to_exitcode(status)
    if rc != 0 or not Path(base + ".csv").exists():
        sys.exit(f"STOP: perf/probe exited {rc} for {shape} {events} on arm {arm}: "
                 f"{Path(base + '.log').read_text()[-800:]}")
    rows = dfc.read_csv(base + ".csv")
    if SET in ("delete_acct", "promote2"):
        # The probe's own clock over the counted delete passes, kept beside
        # the counters as the phase's wall clock (`phase_probe.c`).
        ns = [ln.split()[1] for ln in Path(base + ".log").read_text().splitlines()
              if ln.startswith("phase_ns ")]
        if len(ns) != 1 or int(ns[0]) <= 0:
            sys.exit(f"STOP: no phase_ns line from the probe for {shape} on arm {arm}")
        rows["phase_ns"] = (float(ns[0]), 100.0)
    return wall, ru, rows


def preflight(outdir):
    report = {}
    for gname, gev in GROUPS.items():
        # `sm_big` misses in every cache level, so each event counts in the
        # thousands at least and a co-scheduling zero cannot hide in noise.
        _, _, together = perf_phase(FIXED + gev, PREFLIGHT[0], PREFLIGHT[1], PREFLIGHT[2],
                                    f"{outdir}/pre_{gname}")
        dfc.check_rows(together, FIXED + gev, f"preflight group {gname}")
        for e in (e for e in gev if e not in GP_EXEMPT):
            _, _, alone = perf_phase([e], PREFLIGHT[0], PREFLIGHT[1], PREFLIGHT[2],
                                     f"{outdir}/pre_{gname}_{e}")
            dfc.check_rows(alone, [e], f"preflight {e} alone")
            a, t = alone[e][0], together[e][0]
            if a > 0 and t == 0:
                sys.exit(f"STOP: {e} counts {a:.0f} alone and 0 in group {gname} (#1177)")
            report[e] = {"group": gname, "alone": a, "in_group": t}
    return report


def main():
    global PROBE
    run, host, prefix = int(sys.argv[1]), sys.argv[2], sys.argv[3]
    if ROUNDS % len(ORDERS):
        sys.exit(f"STOP: SNG_PHASE_ROUNDS={ROUNDS} is not a multiple of {len(ORDERS)}")
    outdir = f"{driver.D}/diag_phase_counters/{SET}_run{run}"
    os.makedirs(outdir, exist_ok=True)
    PROBE = build_probe()
    driver.ARMS = ARMS
    proof = {arm: driver.mapped_objects(arm, d) for arm, d in ARMS.items()}
    for arm in (a for a in ARMS if a != "S"):
        if not any(x.startswith(ARMS[arm]) and "expanse" in x for x in proof[arm]):
            sys.exit(f"STOP: arm {arm} did not map libexpanse from {ARMS[arm]}: {proof}")
    if any("expanse" in x for x in proof["S"]):
        sys.exit(f"STOP: arm S mapped an Expanse object: {proof}")
    if SET == "promote2" and driver.sha256(f"{ARMS['P']}/libJudy.so.1") == driver.sha256(f"{ARMS['M']}/libJudy.so.1"):
        sys.exit("STOP: arms P and M are the same file")
    if SET == "delete_acct" and driver.sha256(f"{ARMS['A']}/libJudy.so.1") == driver.sha256(f"{ARMS['D']}/libJudy.so.1"):
        sys.exit("STOP: arms A and D are the same file, so the feature did not build in")
    prov = bp.new_provenance(
        "stressng_comparison", 1679 if SET.startswith("delete") else 1680,
        "N/S, N64/S and N64/N: the ratio of two arms' means of one user-mode counter over the "
        "measured phase, with a two-sample BCa 95% interval over the run's rounds",
        repo_root=driver.REPO_ROOT, host_label=host, run=run, rounds=ROUNDS,
        diagnostic="phase counters on the shapes Expanse loses, outside METHODOLOGY.md's pre-registration",
        probe="scripts/phase_probe.c", phase_set=SET, shapes=SHAPES, arms=ARMS,
        arm_orders=["".join(o) for o in ORDERS], pmu="cpu_core",
        perf_version=subprocess.run(["perf", "--version"], capture_output=True, text=True).stdout.strip(),
        event_groups={g: FIXED + e for g, e in GROUPS.items()}, cell_isolation="process",
        sha256={"probe": driver.sha256(PROBE),
                **{f"arm_{a}": driver.sha256(f"{d}/libJudy.so.1") for a, d in ARMS.items()}},
        library_proof=proof)
    prov["preflight"] = preflight(outdir)
    with open(f"{prefix}.jsonl", "a") as out:
        for rnd in range(ROUNDS):
            cells = [(s, g) for s in SHAPES for g in GROUPS]
            random.Random(run * 1000 + rnd).shuffle(cells)
            arms = ORDERS[(rnd + run) % len(ORDERS)]
            for (shape, repeats, ops), g in cells:
                for arm in arms:
                    if driver.mem_available_gib() < driver.MEM_FLOOR_GIB:
                        sys.exit(f"STOP: MemAvailable below {driver.MEM_FLOOR_GIB} GiB")
                    base = f"{outdir}/r{rnd}_{shape}_{g}_{arm}"
                    start = bp.begin_cell(prov, f"cell:{shape}:{g}:{arm}:r{rnd}")
                    wall, ru, rows = perf_phase(FIXED + GROUPS[g], shape, repeats, arm, base)
                    load = bp.end_cell(start)
                    dfc.check_rows(rows, FIXED + GROUPS[g], f"r{rnd} {shape} {g} {arm}")
                    rec = dict(run=run, round=rnd, cell=shape, group=g, arm=arm, order="".join(arms),
                               rc=0, failed=False, wall=wall, ops=ops,
                               own_cpu=ru.ru_utime + ru.ru_stime, load=load,
                               counters={e: rows[e][0] for e in FIXED + GROUPS[g]
                                         + (["phase_ns"] if SET in ("delete_acct", "promote2") else [])})
                    out.write(json.dumps(rec) + "\n")
                    out.flush()
            print(f"{time.strftime('%H:%M:%S')} round {rnd} done", flush=True)
    bp.add_load(prov, "end")
    Path(f"{prefix}.provenance.json").write_text(json.dumps(prov, indent=2) + "\n")


if __name__ == "__main__":
    main()
