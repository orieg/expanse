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
import math
import os
import platform
import random
import re
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
elif SET == "big_main":
    # The 4 Mi insert and delete phases on main (README.md, "Diagnostic: the
    # 4 Mi phases on main"): N is main's library (SNG_MAIN, a commit whose
    # library `build_layout_lib.sh` built), E the published comparison's,
    # against stock.
    ARMS = {"N": f"{driver.D}/expcompat-{os.environ['SNG_MAIN']}/lib", "E": driver.ARMS["E"],
            "S": driver.ARMS["S"]}
    SHAPES = [("judy_4m", 1, 4_194_304), ("del_4m", 1, 4_194_304)]
    PREFLIGHT = ("judy_4m", 1, "N")
elif SET == "big_layout":
    # The 4 Mi phases' per-process layout (README.md, "Diagnostic: the 4 Mi
    # phases' per-process layout"): N and S as in big_main, and NR and SR the
    # same libraries with address-space randomisation off for the probe
    # (`setarch -R`). Every process prints its layout (`phase_probe.c`,
    # SNG_LAYOUT=1).
    ARMS = {"N": f"{driver.D}/expcompat-{os.environ['SNG_MAIN']}/lib",
            "NR": f"{driver.D}/expcompat-{os.environ['SNG_MAIN']}/lib",
            "S": driver.ARMS["S"], "SR": driver.ARMS["S"]}
    SHAPES = [("judy_4m", 1, 4_194_304), ("del_4m", 1, 4_194_304)]
    PREFLIGHT = ("judy_4m", 1, "N")
elif SET == "big_pfn":
    # The 4 Mi phases' physical frames (README.md, "Diagnostic: the 4 Mi
    # phases' physical frames"): N and S as in big_main. Each process's whole
    # per-process command runs under `sudo -n` so its pagemap shows frame
    # numbers, and the probe prints only aggregates of them (SNG_LAYOUT=2).
    ARMS = {"N": f"{driver.D}/expcompat-{os.environ['SNG_MAIN']}/lib", "S": driver.ARMS["S"]}
    SHAPES = [("judy_4m", 1, 4_194_304), ("del_4m", 1, 4_194_304)]
    PREFLIGHT = ("judy_4m", 1, "N")
elif SET == "big_pf":
    # The 4 Mi phases repeated within one process (README.md, "Diagnostic:
    # the 4 Mi phases repeated in one process"): N and S as in big_main. The
    # probe counts each repetition itself (SNG_SELFCOUNT=1) and compares each
    # repetition's [heap] frames with the first's (SNG_LAYOUT=3, aggregates
    # only), its one command under `sudo -n` as in big_pfn.
    ARMS = {"N": f"{driver.D}/expcompat-{os.environ['SNG_MAIN']}/lib", "S": driver.ARMS["S"]}
    REPS = 4
    SHAPES = [("judy_4m", REPS, REPS * 4_194_304), ("del_4m", REPS, REPS * 4_194_304)]
    PREFLIGHT = ("sm_big", 1, "N")
elif SET == "gets_inc":
    # The #1680 shapes on an engine increment (README.md, "Diagnostic: where
    # the #1680 shapes mispredict and miss"): G is the increment's head and M
    # its merge base, the libraries `diag_increment.py` built
    # (SNG_INC_HEAD, SNG_INC_BASE), against stock.
    ARMS = {"G": f"{driver.D}/expcompat-{os.environ['SNG_INC_HEAD']}/lib",
            "M": f"{driver.D}/expcompat-{os.environ['SNG_INC_BASE']}/lib",
            "S": driver.ARMS["S"]}
    SHAPES = [("sm_default", 200, 200 * 20_000), ("sm_big", 1, 2_000_000), ("judy_4m", 1, 4_194_304)]
    PREFLIGHT = ("sm_big", 1, "G")
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
if SET in ("big_layout", "big_pfn"):
    GROUPS = {"mem": GROUPS["mem"]}
elif SET == "big_pf":
    # The probe's own counter group (phase_probe.c, SNG_SELFCOUNT), in its
    # order after cycles and instructions; the preflight runs it under perf.
    GROUPS = {"pf": ["mem_load_retired.l1_miss", "mem_load_retired.fb_hit",
                     "l1d.replacement", "l1d.hwpf_miss"]}
# Arms that run the probe with address-space randomisation off, and the stock arms.
NORAND = {"NR", "SR"}
STOCK = {"S", "SR"}
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


def self_phase(shape, repeats, arm, base):
    """big_pf: one probe process counting its own repetitions (phase_probe.c,
    SNG_SELFCOUNT=1), under `sudo -n` so its pagemap shows frames."""
    events = FIXED + GROUPS["pf"]
    cmd = ["sudo", "-n", "env", f"LD_LIBRARY_PATH={ARMS[arm]}", "SNG_LAYOUT=3", "SNG_SELFCOUNT=1",
           PROBE, shape, str(repeats), "-", "-"]
    t0 = time.monotonic()
    with open(base + ".log", "w") as lf:
        p = subprocess.Popen(cmd, stdout=lf, stderr=subprocess.STDOUT)
        _, status, ru = os.wait4(p.pid, 0)
    wall = time.monotonic() - t0
    rc = os.waitstatus_to_exitcode(status)
    text = Path(base + ".log").read_text()
    if rc != 0:
        sys.exit(f"STOP: probe exited {rc} for {shape} on arm {arm}: {text[-800:]}")
    reps, frames = [], []
    for ln in text.splitlines():
        f = ln.split()
        if f[:1] == ["rep"]:
            # rep <r> cpu <a> <b> enabled <ns> running <ns> counts <6>
            reps.append({"rep": int(f[1]), "cpu": [int(f[3]), int(f[4])], "enabled": int(f[6]),
                         "running": int(f[8]), "counters": dict(zip(events, map(int, f[10:])))})
        elif f[:2] == ["layout", "pfn_rep"]:
            frames.append({"rep": int(f[2]), "pages": int(f[3]), "zero": int(f[4]), "same": int(f[5])})
    if [r["rep"] for r in reps] != list(range(repeats)) or [x["rep"] for x in frames] != list(range(repeats)):
        sys.exit(f"STOP: {shape} on arm {arm} reported reps {[r['rep'] for r in reps]} and frames "
                 f"{[x['rep'] for x in frames]}, not 0..{repeats - 1}")
    for r in reps:
        if r["running"] != r["enabled"] or r["enabled"] == 0:
            sys.exit(f"STOP: {shape} rep {r['rep']} on arm {arm} was multiplexed or never ran: {r}")
        if any(len(r["counters"]) != len(events) or v <= 0 for v in r["counters"].values()):
            sys.exit(f"STOP: {shape} rep {r['rep']} on arm {arm} has a zero counter: {r}")
    for x in frames:
        if x["pages"] == 0 or x["zero"] != 0:
            sys.exit(f"STOP: frames not readable for {shape} rep {x['rep']} on arm {arm}: {x}")
    rows = {e: math.fsum(r["counters"][e] for r in reps) for e in events}
    return wall, ru, rows, reps, frames


def perf_phase(events, shape, repeats, arm, base):
    """One probe process under `perf stat`, counters enabled only around its phase."""
    with tempfile.TemporaryDirectory() as td:
        ctl, ack = f"{td}/ctl", f"{td}/ack"
        os.mkfifo(ctl)
        os.mkfifo(ack)
        env = dict(os.environ, LD_LIBRARY_PATH=ARMS[arm])
        if SET == "big_layout":
            env["SNG_LAYOUT"] = "1"
        norand = ["setarch", platform.machine(), "-R"] if arm in NORAND else []
        # big_pfn: perf writes its rows to stderr, into a file this user
        # opened, so the privileged command leaves no file owned by root.
        out = [] if SET == "big_pfn" else ["-o", base + ".csv"]
        cmd = ["perf", "stat", "-x,", "--no-big-num", "-D", "-1", "--control", f"fifo:{ctl},{ack}",
               *out, "-e", ",".join(dfc.ev(e) for e in events), "--",
               *norand, PROBE, shape, str(repeats), ctl, ack]
        if SET == "big_pfn":
            cmd = ["sudo", "-n", "env", f"LD_LIBRARY_PATH={ARMS[arm]}", "SNG_LAYOUT=2", *cmd]
        t0 = time.monotonic()
        with open(base + ".log", "w") as lf, open(base + ".csv", "w") as cf:
            p = subprocess.Popen(cmd, env=env, stdout=lf,
                                 stderr=cf if SET == "big_pfn" else subprocess.STDOUT)
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
    if SET == "big_pf":
        # The probe's own count covers what perf stat's bracket covers: one
        # judy_4m repetition's instructions agree within 0.5%.
        _, _, viaperf = perf_phase(FIXED + GROUPS["pf"], "judy_4m", 1, "N", f"{outdir}/pre_x_perf")
        _, _, own, _, _ = self_phase("judy_4m", 1, "N", f"{outdir}/pre_x_self")
        a, b = viaperf["instructions"][0], own["instructions"]
        if abs(a / b - 1) > 0.005:
            sys.exit(f"STOP: self-counted instructions {b:.0f} vs perf stat {a:.0f} on judy_4m")
        report["selfcount_crosscheck"] = {"perf_instructions": a, "self_instructions": b}
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
    for arm in (a for a in ARMS if a not in STOCK):
        if not any(x.startswith(ARMS[arm]) and "expanse" in x for x in proof[arm]):
            sys.exit(f"STOP: arm {arm} did not map libexpanse from {ARMS[arm]}: {proof}")
    for arm in (a for a in ARMS if a in STOCK):
        if any("expanse" in x for x in proof[arm]):
            sys.exit(f"STOP: arm {arm} mapped an Expanse object: {proof}")
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
    if SET in ("big_pfn", "big_pf"):
        if subprocess.run(["sudo", "-n", "true"]).returncode != 0:
            sys.exit("STOP: no non-interactive privilege (sudo -n); the frames cannot be read")
        prov["privilege"] = ("sudo -n on each per-process perf stat + phase_probe command" if SET == "big_pfn"
                             else "sudo -n on each per-process phase_probe command")
        prov["numa_nodes"] = len([d for d in os.listdir("/sys/devices/system/node") if re.fullmatch(r"node\d+", d)])
    if SET in ("big_layout", "big_pfn", "big_pf"):
        prov["randomize_va_space"] = Path("/proc/sys/kernel/randomize_va_space").read_text().strip()
        prov["thp_enabled"] = Path("/sys/kernel/mm/transparent_hugepage/enabled").read_text().strip()
        prov["norand_arms"] = sorted(NORAND & set(ARMS))
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
                    if SET == "big_pf":
                        wall, ru, sums, reps, frames = self_phase(shape, repeats, arm, base)
                        rows = {e: (v, 100.0) for e, v in sums.items()}
                    else:
                        wall, ru, rows = perf_phase(FIXED + GROUPS[g], shape, repeats, arm, base)
                    load = bp.end_cell(start)
                    if SET != "big_pf":
                        dfc.check_rows(rows, FIXED + GROUPS[g], f"r{rnd} {shape} {g} {arm}")
                    rec = dict(run=run, round=rnd, cell=shape, group=g, arm=arm, order="".join(arms),
                               rc=0, failed=False, wall=wall, ops=ops,
                               own_cpu=ru.ru_utime + ru.ru_stime, load=load,
                               counters={e: rows[e][0] for e in FIXED + GROUPS[g]
                                         + (["phase_ns"] if SET in ("delete_acct", "promote2") else [])})
                    if SET == "big_pf":
                        rec["reps"], rec["frames"] = reps, frames
                    if SET in ("big_layout", "big_pfn"):
                        lay = [ln.split()[1:] for ln in Path(base + ".log").read_text().splitlines()
                               if ln.startswith("layout ")]
                        if not any(x[0] == "anonhuge_kb" for x in lay) or not any(x[0] == "map" for x in lay):
                            sys.exit(f"STOP: no layout lines from the probe for {shape} on arm {arm}")
                        if SET == "big_pfn":
                            kv = {x[0]: x[1] for x in lay if len(x) > 1}
                            if "pfn_pages" not in kv or int(kv["pfn_pages"]) == 0 or kv.get("pfn_zero") != "0":
                                sys.exit(f"STOP: frames not readable for {shape} on arm {arm}: {kv}")
                            # The probe's `layout map` lines carry virtual addresses only.
                        rec["layout"] = lay
                    out.write(json.dumps(rec) + "\n")
                    out.flush()
            print(f"{time.strftime('%H:%M:%S')} round {rnd} done", flush=True)
    bp.add_load(prov, "end")
    Path(f"{prefix}.provenance.json").write_text(json.dumps(prov, indent=2) + "\n")


if __name__ == "__main__":
    main()
