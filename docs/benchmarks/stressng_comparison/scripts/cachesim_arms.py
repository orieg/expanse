#!/usr/bin/env python3
"""Per-arm instruction and simulated L1D-miss comparison of two builds.

README.md, "Increment: a BranchB's level on its first cache line". Reads two
iai-callgrind output trees of `crates/expanse/benches/instructions.rs`, both
run with Callgrind's cache simulator under the reference host's P-core
geometry, and writes one row per arm: `Ir`, `D1mr` and `D1mw` at the base and
at the head. Deterministic: no timing, no host load.

Each tree comes from, inside one container per build:
  CARGO_PROFILE_BENCH_DEBUG=1 cargo bench -p expanse-trie --bench instructions -- \
    --callgrind-args='--cache-sim=yes --D1=49152,12,64 --LL=1310720,10,64 --I1=32768,8,64'

Usage:
  cachesim_arms.py <base target dir> <head target dir> <base sha> <head sha> \
      <rustc -V> <valgrind --version> <out.json>
"""
import glob
import json
import re
import sys

GEOMETRY = {"D1": "49152,12,64", "LL": "1310720,10,64", "I1": "32768,8,64"}
# The arm families the pre-registration bounds on D1mr + D1mw.
BOUNDED = re.compile(r"::(map_get|set_contains|map_ins|map_remove|sync_)")
BOUND_PCT = 0.5
GEN_4M_OPS = 4_194_304


def load(root):
    """arm -> {event: total} from every callgrind out file under `root`."""
    out = {}
    pattern = f"{root}/iai/expanse-trie/instructions/*/*/callgrind.*.out"
    for path in glob.glob(pattern):
        parts = path.split("/")
        arm = f"{parts[-3]}::{parts[-2]}"
        events = totals = None
        with open(path, errors="replace") as fh:
            for line in fh:
                if line.startswith("events:"):
                    events = line.split()[1:]
                elif line.startswith(("totals:", "summary:")):
                    totals = [int(x) for x in line.split()[1:]]
        if events is None or totals is None:
            raise SystemExit(f"{path}: no events or totals line")
        out[arm] = dict(zip(events, totals))
    if not out:
        raise SystemExit(f"{root}: no callgrind output found")
    return out


def pct(new, old):
    return 100.0 * (new - old) / old if old else 0.0


def main(argv):
    base_dir, head_dir, base_sha, head_sha, rustc, valgrind, out_path = argv[1:8]
    base, head = load(base_dir), load(head_dir)
    if set(base) != set(head):
        raise SystemExit(f"arm sets differ: {sorted(set(base) ^ set(head))[:5]}")
    arms = []
    for arm in sorted(base):
        b, h = base[arm], head[arm]
        arms.append({
            "arm": arm,
            "ir_base": b["Ir"], "ir_head": h["Ir"],
            "d1mr_base": b["D1mr"], "d1mr_head": h["D1mr"],
            "d1mw_base": b["D1mw"], "d1mw_head": h["D1mw"],
            "bounded": bool(BOUNDED.search(arm)),
        })
    d1 = lambda r, s: r[f"d1mr_{s}"] + r[f"d1mw_{s}"]
    over = [r["arm"] for r in arms
            if r["bounded"] and pct(d1(r, "head"), d1(r, "base")) > BOUND_PCT]
    by = {r["arm"]: r for r in arms}
    ins, rem = by["cost::map_ins_slot.gen_4m"], by["cost::map_remove_revgen.gen_4m"]
    summary = {
        "arms": len(arms),
        "arms_ir_changed": sum(r["ir_base"] != r["ir_head"] for r in arms),
        "ir_worst_pct": max(pct(r["ir_head"], r["ir_base"]) for r in arms),
        "bounded_arms": sum(r["bounded"] for r in arms),
        "bounded_arms_over_d1_bound": over,
        "d1_bound_pct": BOUND_PCT,
        "gen_4m_ins_d1mr_per_op_base": ins["d1mr_base"] / GEN_4M_OPS,
        "gen_4m_ins_d1mr_per_op_head": ins["d1mr_head"] / GEN_4M_OPS,
        "gen_4m_remove_d1mr_per_op_base": rem["d1mr_base"] / GEN_4M_OPS,
        "gen_4m_remove_d1mr_per_op_head": rem["d1mr_head"] / GEN_4M_OPS,
    }
    doc = {
        "provenance": {
            "instrument": "Callgrind cache simulator, no prefetcher, virtual addresses",
            "geometry": GEOMETRY,
            "bench": "crates/expanse/benches/instructions.rs (workload core_instructions)",
            "base": base_sha, "head": head_sha,
            "rustc": rustc, "valgrind": valgrind,
            "container": "Ubuntu 24.04, iai-callgrind-runner 0.16.1, CARGO_PROFILE_BENCH_DEBUG=1",
        },
        "summary": summary,
        "arms": arms,
    }
    with open(out_path, "w") as fh:
        json.dump(doc, fh, indent=1)
        fh.write("\n")
    print(json.dumps(summary, indent=1))


if __name__ == "__main__":
    main(sys.argv)
