#!/usr/bin/env python3
"""stress-ng x Judy timed driver (METHODOLOGY.md §4-§7).

One stress-ng process per (round, cell, arm). Every process is bracketed by a
`bench_provenance` load window, so its foreign load is the host's busy CPU
over its wall time minus the runner's children's CPU. Raw rows go to
`<out>.jsonl`, one per process, and the run's provenance to
`<out>.provenance.json`. Runs under the host bench lock and the core pin that
`run.sh` sets; it neither takes the lock nor pins on its own.

Usage: driver.py <run_number> <host_label> <out_prefix>
Environment: SNG_D (the host's stress-ng tree), REPO_ROOT (this checkout),
SNG_ROUNDS (default 10).
"""
import hashlib
import json
import os
import random
import re
import subprocess
import sys
import time
from pathlib import Path

REPO_ROOT = Path(os.environ["REPO_ROOT"])
sys.path.insert(0, str(REPO_ROOT / "scripts"))
import bench_provenance as bp  # noqa: E402

D = os.environ.get("SNG_D", "/var/tmp/stressng-eval")
BIN = f"{D}/b-stockbuild/stress-ng"
ARMS = {"S": f"{D}/stock/usr/lib/x86_64-linux-gnu", "E": f"{D}/expcompat/lib"}
ROUNDS = int(os.environ.get("SNG_ROUNDS", "10"))
SEED = "12345"
MEM_FLOOR_GIB = 16


def cells():
    """Every (workload_id, W, stressor arguments) of METHODOLOGY.md §5."""
    out = []
    for size, tag, ops in ((1024, "1k", 5000), (262144, "256k", 40), (4194304, "4m", 3)):
        for w in (1, 2, 4, 8):
            out.append((f"sng_judy_{tag}", w, ["--judy", str(w), "--judy-size", str(size),
                                               "--judy-ops", str(ops * w)]))
    # hashjudy leaks every array (stress-ng never calls JLFA), so its ops and W are bounded.
    for m, ws, dops, bops in (("judy", (1, 8), 1000, 4), ("hashjudy", (1, 4), 100, 1)):
        for w in ws:
            out.append((f"sng_sm_{m}_default", w, ["--sparsematrix", str(w), "--sparsematrix-method", m,
                                                   "--sparsematrix-ops", str(dops * w)]))
            out.append((f"sng_sm_{m}_big", w, ["--sparsematrix", str(w), "--sparsematrix-method", m,
                                               "--sparsematrix-items", "1000000",
                                               "--sparsematrix-size", "100000",
                                               "--sparsematrix-ops", str(bops * w)]))
    return out


def parse_yaml_metrics(path):
    """The aggregate metrics block of stress-ng's --yaml for its first stressor."""
    out, inside = {}, False
    with open(path) as f:
        for line in f:
            if line.startswith("metrics:"):
                inside = True
                continue
            if inside:
                if re.match(r"^\s{6}instances:", line) or (line.strip() and not line.startswith(" ")):
                    break
                m = re.match(r"^\s{4,6}-?\s*([\w-]+):\s*(\S+)", line)
                if m:
                    k, v = m.group(1), m.group(2)
                    try:
                        out[k] = float(v)
                    except ValueError:
                        out[k] = v
    return out


def mem_available_gib():
    with open("/proc/meminfo") as f:
        for line in f:
            if line.startswith("MemAvailable:"):
                return int(line.split()[1]) / 1048576
    raise RuntimeError("/proc/meminfo has no MemAvailable")


def sha256(path):
    h = hashlib.sha256()
    with open(os.path.realpath(path), "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def mapped_objects(arm, libdir):
    """The Judy or Expanse objects one untimed `--judy 1` worker maps under `libdir`.

    Reads the mapped objects of the worker children from `/proc/<pid>/maps`,
    and fails loudly if the arm shows none (AGENTS.md §8.1).
    """
    env = dict(os.environ, LD_LIBRARY_PATH=libdir)
    p = subprocess.Popen([BIN, "--judy", "1", "--judy-size", "4194304", "--judy-ops", "12",
                          "-t", "60"], env=env, stdout=subprocess.DEVNULL,
                         stderr=subprocess.DEVNULL)
    seen = set()
    for _ in range(40):
        time.sleep(0.1)
        kids = Path(f"/proc/{p.pid}/task/{p.pid}/children")
        for kid in (kids.read_text().split() if kids.exists() else []):
            try:
                for line in Path(f"/proc/{kid}/maps").read_text().splitlines():
                    path = line.split()[-1] if len(line.split()) >= 6 else ""
                    if "Judy" in path or "expanse" in path:
                        seen.add(path)
            except OSError:
                pass
        if seen:
            break
    p.wait()
    if not seen:
        sys.exit(f"STOP: arm {arm} mapped no Judy or Expanse object")
    return sorted(seen)


def library_proof():
    """Which Judy library each arm's worker actually mapped (METHODOLOGY.md §4)."""
    proof = {arm: mapped_objects(arm, libdir) for arm, libdir in ARMS.items()}
    if not any("expanse" in x for x in proof["E"]) or any("expanse" in x for x in proof["S"]):
        sys.exit(f"STOP: library proof does not separate the arms: {proof}")
    return proof


def run_one(prov, run, rnd, cell, w, args, arm):
    if mem_available_gib() < MEM_FLOOR_GIB:
        sys.exit(f"STOP: MemAvailable below {MEM_FLOOR_GIB} GiB before {cell} W{w} {arm}")
    yaml = f"{D}/out/r{run}_{rnd}_{cell}_w{w}_{arm}.yaml"
    env = dict(os.environ, LD_LIBRARY_PATH=ARMS[arm])
    cmd = [BIN] + args + ["--seed", SEED, "-t", "300", "--metrics", "--yaml", yaml]
    start = bp.begin_cell(prov, f"cell:{cell}:W{w}:{arm}:r{rnd}")
    t0 = time.monotonic()
    logpath = yaml[:-5] + ".log"
    with open(logpath, "w") as lf:
        p = subprocess.Popen(cmd, env=env, stdout=lf, stderr=subprocess.STDOUT)
        _, status, ru = os.wait4(p.pid, 0)
    wall = time.monotonic() - t0
    load = bp.end_cell(start)
    rc = os.waitstatus_to_exitcode(status)
    log = Path(logpath).read_text()
    rec = dict(run=run, round=rnd, cell=cell, w=w, arm=arm, rc=rc, wall=wall,
               own_cpu=ru.ru_utime + ru.ru_stime, ru_maxrss_kb=ru.ru_maxrss, load=load,
               failed=("fail" in log.lower() and "failed: 0" not in log),
               metrics=parse_yaml_metrics(yaml) if os.path.exists(yaml) else {})
    if rc != 0:
        rec["log_tail"] = log[-800:]
    return rec


def main():
    run, host, prefix = int(sys.argv[1]), sys.argv[2], sys.argv[3]
    os.makedirs(f"{D}/out", exist_ok=True)
    prov = bp.new_provenance(
        "stressng_comparison", 1666,
        "E/S: the ratio of the two arms' means of stress-ng's own per-phase rate "
        "(or max-rss, or wall clock), with a two-sample BCa 95% interval over the run's rounds",
        repo_root=REPO_ROOT, host_label=host, run=run, rounds=ROUNDS,
        binary=BIN, arms=ARMS, cell_isolation="process",
        sha256={"binary": sha256(BIN),
                "stock_libjudy": sha256(f"{ARMS['S']}/libJudy.so.1"),
                "expanse": sha256(f"{ARMS['E']}/libJudy.so.1")},
        library_proof=library_proof())
    cs = cells()
    with open(f"{prefix}.jsonl", "a") as out:
        for rnd in range(ROUNDS):
            order = list(cs)
            random.Random(run * 1000 + rnd).shuffle(order)
            arms = ("S", "E") if rnd % 2 == 0 else ("E", "S")
            for cell, w, args in order:
                for arm in arms:
                    rec = run_one(prov, run, rnd, cell, w, args, arm)
                    out.write(json.dumps(rec) + "\n")
                    out.flush()
                    if rec["rc"] != 0 or rec["failed"]:
                        sys.exit(f"STOP: non-zero exit or stress-ng failure in {cell} W{w} {arm}")
            print(f"{time.strftime('%H:%M:%S')} round {rnd} done", flush=True)
    bp.add_load(prov, "end")
    Path(f"{prefix}.provenance.json").write_text(json.dumps(prov, indent=2) + "\n")


if __name__ == "__main__":
    main()
