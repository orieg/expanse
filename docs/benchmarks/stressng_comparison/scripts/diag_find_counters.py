#!/usr/bin/env python3
"""Front-end counters on the `sng_judy_1k` find cell (README.md, "Diagnostic: counters").

A diagnostic outside METHODOLOGY.md's pre-registration (#1684). The three arms
of `diag_lib_ab.py` (O: the 82eb56af library file, N: the file of the
pre-registered runs, S: stock libJudy) run `sng_judy_1k` at W = 1 under
`perf stat`, one process per (round, event group, arm). The cell's work is
fixed (`--judy-ops`), so whole-process counts compare directly across arms.

Event groups hold at most four general-purpose events, each beside the fixed
`cycles` and `instructions` counters, because a larger set can read a real
event as zero on this PMU (`scripts/perf_counters.py`, #1177). Every count is
user-mode and on the `cpu_core` PMU, which is where the pin places the run.

Before the timed rounds a preflight counts every general-purpose event alone
and in its group once, on arm N, and stops if an event that counts alone
reads zero in its group or any row ran below 100% of the time (multiplexing).
After the rounds, one `perf record` per arm gives the share of user cycles per
symbol, to say how much of the cycle difference sits in `JudyLGet`.

Usage: diag_find_counters.py <run_number> <host_label> <out_prefix>
Environment: as `diag_lib_ab.py`; SNG_COUNTER_ROUNDS (default 12, a multiple of 6).
"""
import csv
import io
import json
import os
import random
import subprocess
import sys
import time
from pathlib import Path

import diag_lib_ab
import driver

bp = driver.bp
ARMS = diag_lib_ab.ARMS
ORDERS = diag_lib_ab.ORDERS
ROUNDS = int(os.environ.get("SNG_COUNTER_ROUNDS", "12"))
CELL, W = "sng_judy_1k", 1
FIXED = ["cycles", "instructions"]
GROUPS = {
    "uops": ["idq.dsb_uops", "idq.mite_uops", "idq.ms_uops", "lsd.uops"],
    "fe": ["idq_uops_not_delivered.core", "frontend_retired.dsb_miss",
           "dsb2mite_switches.penalty_cycles", "icache_data.stalls"],
    "misc": ["icache_tag.stalls", "itlb_misses.walk_completed",
             "br_misp_retired.all_branches", "baclears.any"],
    # `slots` is the fixed TOPDOWN.SLOTS counter; it gives the issue width the
    # reading rule divides by, measured rather than assumed.
    "br": ["slots", "br_inst_retired.all_branches", "uops_issued.any", "decode.lcp"],
}
GP_EXEMPT = {"slots"}


def ev(name):
    return f"cpu_core/{name}/u"


def perf_stat(events, args, arm, csv_path):
    """One stress-ng process under `perf stat`; returns (rc, wall, rusage, rows)."""
    env = dict(os.environ, LD_LIBRARY_PATH=ARMS[arm])
    cmd = ["perf", "stat", "-x,", "--no-big-num", "-o", csv_path,
           "-e", ",".join(ev(e) for e in events), "--", driver.BIN] + args
    t0 = time.monotonic()
    with open(csv_path + ".log", "w") as lf:
        p = subprocess.Popen(cmd, env=env, stdout=lf, stderr=subprocess.STDOUT)
        _, status, ru = os.wait4(p.pid, 0)
    rc = os.waitstatus_to_exitcode(status)
    if rc != 0 or not Path(csv_path).exists():
        sys.exit(f"STOP: perf stat exited {rc} for {events} on arm {arm}: "
                 f"{Path(csv_path + '.log').read_text()[-800:]}")
    return rc, time.monotonic() - t0, ru, read_csv(csv_path)


def read_csv(path):
    """`perf stat -x,` rows as {event: (count or None, pct_running)}."""
    rows = {}
    for rec in csv.reader(io.StringIO(Path(path).read_text())):
        if len(rec) < 5 or rec[0].startswith("#"):
            continue
        name = rec[2].removeprefix("cpu_core/").removesuffix("/u")
        count = None if rec[0].startswith("<") else float(rec[0])
        rows[name] = (count, float(rec[4]) if rec[4] else None)
    return rows


def check_rows(rows, events, where):
    for e in events:
        if e not in rows:
            sys.exit(f"STOP: {where}: perf printed no row for {e}: {rows}")
        count, pct = rows[e]
        if count is None:
            sys.exit(f"STOP: {where}: {e} was not counted")
        if pct is None or pct < 100.0:
            sys.exit(f"STOP: {where}: {e} ran {pct}% of the time (multiplexed)")


def preflight(args, outdir):
    """Each general-purpose event alone, then in its group, once on arm N."""
    report = {}
    for gname, gev in GROUPS.items():
        _, _, _, together = perf_stat(FIXED + gev, args, "N", f"{outdir}/pre_{gname}.csv")
        check_rows(together, FIXED + gev, f"preflight group {gname}")
        for e in (e for e in gev if e not in GP_EXEMPT):
            _, _, _, alone = perf_stat([e], args, "N", f"{outdir}/pre_{gname}_{e}.csv")
            check_rows(alone, [e], f"preflight {e} alone")
            a, t = alone[e][0], together[e][0]
            if a > 0 and t == 0:
                sys.exit(f"STOP: {e} counts {a:.0f} alone and 0 in group {gname} (#1177)")
            report[e] = {"group": gname, "alone": a, "in_group": t}
    return report


def symbol_shares(arm, args, outdir):
    """Share of user cycles per (object, symbol) in one `perf record` of the cell."""
    data = f"{outdir}/record_{arm}.data"
    env = dict(os.environ, LD_LIBRARY_PATH=ARMS[arm])
    subprocess.run(["perf", "record", "-q", "-e", ev("cycles"), "-c", "200003", "-o", data,
                    "--", driver.BIN] + args, env=env, check=True,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    out = subprocess.run(["perf", "report", "-i", data, "--stdio", "--no-children",
                          "--sort", "dso,sym", "--percent-limit", "0.5", "-q"],
                         check=True, capture_output=True, text=True).stdout
    Path(f"{outdir}/record_{arm}.txt").write_text(out)
    shares = []
    for line in out.splitlines():
        parts = line.split()
        if len(parts) >= 4 and parts[0].endswith("%"):
            shares.append({"pct": float(parts[0][:-1]), "dso": parts[1],
                           "symbol": " ".join(parts[3:])})
    if not shares:
        sys.exit(f"STOP: perf report for arm {arm} gave no symbol rows")
    return shares


def main():
    run, host, prefix = int(sys.argv[1]), sys.argv[2], sys.argv[3]
    if ROUNDS % len(ORDERS):
        sys.exit(f"STOP: SNG_COUNTER_ROUNDS={ROUNDS} is not a multiple of {len(ORDERS)}")
    driver.ARMS = ARMS
    outdir = f"{driver.D}/diag_find_counters/run{run}"
    os.makedirs(outdir, exist_ok=True)
    args = next(a for c, w, a in driver.cells() if c == CELL and w == W)
    args = args + ["--seed", driver.SEED, "-t", "300", "--metrics"]
    perf_version = subprocess.run(["perf", "--version"], capture_output=True, text=True).stdout.strip()
    prov = bp.new_provenance(
        "stressng_comparison", 1684,
        "O/N, N/S and O/S: the ratio of two arms' means of one user-mode counter per process "
        "(or stress-ng's find rate), with a two-sample BCa 95% interval over the run's rounds",
        repo_root=driver.REPO_ROOT, host_label=host, run=run, rounds=ROUNDS,
        diagnostic="front-end counters on the 1k find cell, outside METHODOLOGY.md's pre-registration",
        binary=driver.BIN, arms=ARMS, arm_orders=["".join(o) for o in ORDERS],
        cell=CELL, w=W, stress_ng_args=args, perf_version=perf_version, pmu="cpu_core",
        event_groups={g: FIXED + e for g, e in GROUPS.items()}, cell_isolation="process",
        sha256={"binary": driver.sha256(driver.BIN),
                **{f"arm_{a}": driver.sha256(f"{d}/libJudy.so.1") for a, d in ARMS.items()}},
        library_proof=diag_lib_ab.library_proof())
    prov["preflight"] = preflight(args, outdir)
    with open(f"{prefix}.jsonl", "a") as out:
        for rnd in range(ROUNDS):
            groups = list(GROUPS)
            random.Random(run * 1000 + rnd).shuffle(groups)
            arms = ORDERS[(rnd + run) % len(ORDERS)]
            for g in groups:
                for arm in arms:
                    if driver.mem_available_gib() < driver.MEM_FLOOR_GIB:
                        sys.exit(f"STOP: MemAvailable below {driver.MEM_FLOOR_GIB} GiB")
                    base = f"{outdir}/r{rnd}_{g}_{arm}"
                    start = bp.begin_cell(prov, f"cell:{CELL}:W{W}:{g}:{arm}:r{rnd}")
                    rc, wall, ru, rows = perf_stat(FIXED + GROUPS[g], args + ["--yaml", base + ".yaml"],
                                                   arm, base + ".csv")
                    load = bp.end_cell(start)
                    log = Path(base + ".csv.log").read_text()
                    if rc != 0 or ("fail" in log.lower() and "failed: 0" not in log):
                        sys.exit(f"STOP: non-zero exit or stress-ng failure in {g} {arm} r{rnd}: {log[-800:]}")
                    check_rows(rows, FIXED + GROUPS[g], f"r{rnd} {g} {arm}")
                    rec = dict(run=run, round=rnd, cell=CELL, w=W, group=g, arm=arm,
                               order="".join(arms), rc=rc, failed=False, wall=wall,
                               own_cpu=ru.ru_utime + ru.ru_stime, load=load,
                               counters={e: rows[e][0] for e in FIXED + GROUPS[g]},
                               metrics=driver.parse_yaml_metrics(base + ".yaml"))
                    out.write(json.dumps(rec) + "\n")
                    out.flush()
            print(f"{time.strftime('%H:%M:%S')} round {rnd} done", flush=True)
    start = bp.begin_cell(prov, "symbol_shares")
    prov["symbol_shares"] = {arm: symbol_shares(arm, args, outdir) for arm in ARMS}
    prov["symbol_shares_load"] = bp.end_cell(start)
    bp.add_load(prov, "end")
    Path(f"{prefix}.provenance.json").write_text(json.dumps(prov, indent=2) + "\n")


if __name__ == "__main__":
    main()
