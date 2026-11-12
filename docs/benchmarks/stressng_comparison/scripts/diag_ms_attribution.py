#!/usr/bin/env python3
"""Which instructions issue the delete phase's microcode uops (README.md,
"Diagnostic: taken branches and microcode on the delete phase").

A diagnostic outside METHODOLOGY.md's pre-registration (#1679). For each
delete shape and each arm of `SNG_PHASE_SET=delete_ms` (D, #1708's library,
and S, stock), one `phase_probe` process runs under `perf record`, with
sampling enabled only around the measured phase, as `perf stat` is in
`diag_phase_counters.py`.

The sampled event is the first of `EVENTS` that `perf record` accepts:
`frontend_retired.ms_flows` with precise attribution, which counts retired
instructions delivered by the microcode sequencer and so names the
instruction itself; otherwise `idq.ms_uops` without precision, whose samples
can land a few instructions after the one that issued them. The event used is
recorded beside the samples.

Per (shape, arm) it keeps the raw `perf annotate` and `perf report` text and,
parsed from them, each function's share of the samples and every instruction
holding at least 1% of them. One process per (shape, arm): a census of where
the samples fall, not an interval.

Usage: diag_ms_attribution.py <run_number> <host_label> <out_prefix>
Environment: SNG_D, REPO_ROOT, SNG_PHASE_SET=delete_ms.
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import diag_phase_counters as dpc
import driver
from ms_annotate import parse

bp = driver.bp
EVENTS = [("frontend_retired.ms_flows", "upp"), ("idq.ms_uops", "u")]
PERIOD = 503
MIN_ANNOTATED = 100


def record(event, mods, shape, repeats, arm, data):
    """`perf record` of one probe process, sampling enabled only around its phase."""
    with tempfile.TemporaryDirectory() as td:
        ctl, ack = f"{td}/ctl", f"{td}/ack"
        os.mkfifo(ctl)
        os.mkfifo(ack)
        env = dict(os.environ, LD_LIBRARY_PATH=dpc.ARMS[arm])
        cmd = ["perf", "record", "-q", "-D", "-1", "--control", f"fifo:{ctl},{ack}",
               "-e", f"cpu_core/{event}/{mods}", "-c", str(PERIOD), "-o", data, "--",
               dpc.PROBE, shape, str(repeats), ctl, ack]
        p = subprocess.run(cmd, env=env, capture_output=True, text=True)
    return p.returncode, p.stdout + p.stderr


def annotate(data, outbase):
    """Function shares and per-instruction sample shares of one recording."""
    rep = subprocess.run(["perf", "report", "-i", data, "--stdio", "--no-children",
                          "--sort", "dso,sym", "--percent-limit", "0.5", "-q"],
                         check=True, capture_output=True, text=True).stdout
    ann = subprocess.run(["perf", "annotate", "-i", data, "--stdio", "--no-source"],
                         check=True, capture_output=True, text=True).stdout
    Path(outbase + ".report.txt").write_text(rep)
    Path(outbase + ".annotate.txt").write_text(ann)
    return parse(rep, ann)


def main():
    run, host, prefix = int(sys.argv[1]), sys.argv[2], sys.argv[3]
    if dpc.SET != "delete_ms":
        sys.exit("STOP: run with SNG_PHASE_SET=delete_ms")
    if not shutil.which("objdump"):
        sys.exit("STOP: perf annotate needs objdump on this host")
    outdir = f"{driver.D}/diag_ms_attribution/run{run}"
    os.makedirs(outdir, exist_ok=True)
    dpc.PROBE = dpc.build_probe()
    driver.ARMS = dpc.ARMS
    prov = bp.new_provenance(
        "stressng_comparison", 1679,
        "share of the run's samples per function and per instruction, one process per (shape, arm); "
        "a census, no interval",
        repo_root=driver.REPO_ROOT, host_label=host, run=run,
        diagnostic="microcode-uop attribution on the delete phase, outside METHODOLOGY.md's pre-registration",
        probe="scripts/phase_probe.c", shapes=dpc.SHAPES, arms=dpc.ARMS, pmu="cpu_core", period=PERIOD,
        perf_version=subprocess.run(["perf", "--version"], capture_output=True, text=True).stdout.strip(),
        sha256={"probe": driver.sha256(dpc.PROBE),
                **{f"arm_{a}": driver.sha256(f"{d}/libJudy.so.1") for a, d in dpc.ARMS.items()}})
    cells = []
    for shape, repeats, ops in dpc.SHAPES:
        for arm in dpc.ARMS:
            base = f"{outdir}/{shape}_{arm}"
            tried = []
            for event, mods in EVENTS:
                start = bp.begin_cell(prov, f"record:{shape}:{arm}:{event}")
                rc, log = record(event, mods, shape, repeats, arm, base + ".data")
                load = bp.end_cell(start)
                tried.append({"event": event, "modifiers": mods, "rc": rc, "log": log[-400:]})
                if rc == 0:
                    break
            else:
                sys.exit(f"STOP: perf record refused every event for {shape} {arm}: {tried}")
            n = len(subprocess.run(["perf", "script", "-i", base + ".data", "-F", "ip"], check=True,
                                   capture_output=True, text=True).stdout.split())
            functions, top, total, rep = annotate(base + ".data", base)
                        # Below MIN_ANNOTATED samples (stock's 4 Mi cell had 4, outside
            # any annotatable symbol) the cell is kept with its count and
            # whatever parsed; at or above it, nothing parsed is a failure.
            if n >= MIN_ANNOTATED and (not functions or total == 0):
                sys.exit(f"STOP: {n} samples but none parsed for {shape} {arm}; see {base}.annotate.txt")
            cells.append({"workload_id": f"phase_{shape}", "arm": arm, "ops": ops, "event": event,
                          "modifiers": mods, "tried": tried, "load": load, "samples": total, "perf_script_samples": n, "rep_string_share": rep,
                          "functions": functions, "instructions": top})
            print(f"{shape} {arm}: {event}/{mods}, {total:.0f} samples, rep-string share {rep:.3f}, "
                  f"top {[(x['insn'], round(x['share'], 3)) for x in top[:3]]}", flush=True)
    bp.add_load(prov, "end")
    Path(f"{prefix}.json").write_text(json.dumps({"provenance": prov, "cells": cells}, indent=1) + "\n")


if __name__ == "__main__":
    main()
