#!/usr/bin/env python3
"""
scripts/leaf_layout_timing.py — the W3 timing of the #1257 id encodings
(docs/ARCHITECTURE.md §3.6): does an encoded key set's memory saving cost
lookups or inserts?

Arms are key sets written by `leaf_layout_census.rs --dump-keys`, so the timed
keys are the censused keys byte for byte:

  orders_text                 #1257's `t%03d:orders:%010d`
  orders_b32a7                the same keys transcoded to aligned base-32
  composite_uniform_enc7x10   the downstream composite, 7-bit ids
  composite_uniform_b32x4a7   the same, aligned base-32 ids
  orders_text_aa              orders_text again: the A/A control, which
                              measures how far two identical arms separate

Every (arm, load order, round) is one process of `benches/leaf_layout_timing.rs`
inside its own load window (`bench_windowed.run_bench_window`). A round runs
every arm under both load orders, in an arm order rotated by the round, so
drift and allocator state do not favour one arm. Each process times the
insert load into a `SyncExpanseStrMap`, then 16 reader threads' Zipfian hits
and misses.

Per pair and load order, each metric's ratio (encoded over text; A/A arm over
its twin) is taken per round and summarised as a mean with its BCa 95%
interval (`scripts/bca_bootstrap.py`). For throughput a ratio above 1 is the
encoded arm faster; for insert time and latency, above 1 is the encoded arm
slower.

Run it under the host bench lock, pinned (the script pins itself):

  python3 scripts/bench_lock.py --suite leaf_layout_timing -- \\
      python3 scripts/leaf_layout_timing.py --keys-dir <dir> --out <file> [--rounds 8]
  python3 scripts/leaf_layout_timing.py --self-test
"""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))

# After the path insert above, which is what makes these importable.
import bench_pin
import bench_windowed
from bca_bootstrap import bca_bootstrap_ci_with_method
from bench_provenance import add_load, attach, estimators, git_sha, host_facts

CONFIDENCE = 0.95
RESAMPLES = 2000
SEED = 42
MIN_ROUNDS = 3

ARMS = ("orders_text", "orders_b32a7", "composite_uniform_enc7x10",
        "composite_uniform_b32x4a7", "orders_text_aa")
# (subject, reference): the ratio is subject over reference, per round.
PAIRS = (("orders_b32a7", "orders_text"),
         ("composite_uniform_b32x4a7", "composite_uniform_enc7x10"),
         ("orders_text_aa", "orders_text"))
ORDERS = ("sorted", "shuffled")
METRICS = ("insert_ns_per_key", "hit_mops", "hit_p50_ns", "hit_p99_ns",
           "miss_mops", "miss_p50_ns", "miss_p99_ns")

ESTIMATOR_RATIO = (
    "paired per round: subject over reference for each metric (encoded over text; the "
    "A/A arm over its twin). The point is the mean of the per-round quotients with its "
    "BCa 95% interval, not the quotient of the two means. Throughput (`*_mops`): above 1 "
    "= subject faster. Insert ns/key and latency percentiles: above 1 = subject slower")
ESTIMATOR_COLUMNS = (
    "per arm, load order and metric: the mean over rounds of each process's value, with "
    "its BCa 95% interval. A process's `*_mops` is total probes over the wall time from "
    "the reader threads' release to the last join; `*_p50_ns`/`*_p99_ns` are percentiles "
    "of every 64th probe timed alone; `insert_ns_per_key` is the single-writer load")
ESTIMATOR_RAW = "rounds_raw holds every process's row verbatim, with its load window"


def arm_files(keys_dir: Path, arm: str) -> tuple[Path, Path]:
    base = "orders_text" if arm == "orders_text_aa" else arm
    return keys_dir / f"{base}.keys", keys_dir / f"{base}.miss"


def rotated(seq, k: int) -> list:
    k %= len(seq)
    return list(seq[k:]) + list(seq[:k])


def interval(samples: list[float]) -> dict:
    """Mean of `samples`, its BCa 95% interval and the construction that produced it."""
    if len(samples) < MIN_ROUNDS:
        return {"point": sum(samples) / len(samples), "ci_lower": None, "ci_upper": None,
                "ci_method": None, "why_no_interval": "fewer than 3 rounds"}
    point, lo, hi, method = bca_bootstrap_ci_with_method(samples, CONFIDENCE, RESAMPLES, SEED)
    return {"point": point, "ci_lower": lo, "ci_upper": hi, "ci_method": method, "n": len(samples)}


def summarise(rows: list[dict]) -> dict:
    """Per-arm intervals and paired per-round ratio intervals."""
    def by_round(arm, order, metric):
        out = {}
        for r in rows:
            if r["arm"] == arm and r["order"] == order:
                if r["round"] in out:
                    raise RuntimeError(f"{arm}/{order}: round {r['round']} measured twice")
                out[r["round"]] = float(r[metric])
        return out

    intervals, ratios = {}, {}
    for order in ORDERS:
        for arm in ARMS:
            for m in METRICS:
                vals = list(by_round(arm, order, m).values())
                if vals:
                    intervals[f"{arm}/{order}/{m}"] = interval(vals)
        for subj, ref in PAIRS:
            for m in METRICS:
                num, den = by_round(subj, order, m), by_round(ref, order, m)
                if not num or not den:
                    continue
                if set(num) != set(den):
                    raise RuntimeError(f"{subj} vs {ref} {order}: not measured in the same rounds")
                if any(v <= 0 for v in den.values()):
                    raise RuntimeError(f"{ref} {order} {m}: a non-positive denominator")
                ratios[f"{subj}_over_{ref}/{order}/{m}"] = interval(
                    [num[k] / den[k] for k in sorted(num)])
    return {"intervals": intervals, "ratios": ratios}


def run(keys_dir: Path, out: Path, rounds: int, threads: int, ops: int) -> int:
    pin = bench_pin.apply("leaf_layout_timing")
    if not os.environ.get("EXPANSE_BENCH_LOCK_HELD"):
        sys.exit("run under scripts/bench_lock.py (docs/BENCHMARKING.md rule 8)")
    for arm in ARMS:
        for f in arm_files(keys_dir, arm):
            if not f.is_file():
                sys.exit(f"missing key file {f}: run leaf_layout_census --dump-keys first")
    prov = {
        "suite": "leaf_layout_timing",
        "commit": git_sha(REPO_ROOT),
        "host": host_facts(),
        "estimators": estimators(ESTIMATOR_RATIO, ESTIMATOR_COLUMNS, ESTIMATOR_RAW),
        "core_pin": pin,
        "rounds": rounds,
        "threads": threads,
        "ops_per_thread": ops,
        "loads": [],
    }
    add_load(prov, "start")
    rows = []
    for rd in range(rounds):
        for arm in rotated(ARMS, rd):
            for order in rotated(ORDERS, rd):
                keys, miss = arm_files(keys_dir, arm)
                label = f"{arm}/{order}/round={rd}"
                res, window = bench_windowed.run_bench_window(
                    prov, "expanse-trie", "leaf_layout_timing",
                    ["--keys", str(keys), "--miss", str(miss), "--order", order,
                     "--round", str(rd), "--threads", str(threads), "--ops", str(ops)],
                    label=label)
                if res.returncode != 0:
                    print(res.stderr, file=sys.stderr)
                    sys.exit(f"{label} failed")
                row = json.loads(res.stdout.strip().splitlines()[-1])
                # The key file by name only: its directory is host-local, and
                # a committed artifact carries no home paths.
                row.update({"arm": arm, "load": window, "keys": keys.name})
                rows.append(row)
                print(f"{label}: insert {row['insert_ns_per_key']:.1f} ns/key, "
                      f"hit {row['hit_mops']:.2f} Mops, miss {row['miss_mops']:.2f} Mops, "
                      f"foreign {window.get('foreign_busy_cpus')}", file=sys.stderr)
    add_load(prov, "end")
    artifact = attach({"rounds_raw": rows, **summarise(rows)}, prov)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(artifact, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {out}", file=sys.stderr)
    return 0


def self_test() -> int:
    # Paired ratios are per-round quotients, taken only over shared rounds.
    rows = []
    for rd in range(4):
        for arm in ARMS:
            for order in ORDERS:
                base = 10.0 + rd
                row = {"arm": arm, "order": order, "round": rd}
                for m in METRICS:
                    row[m] = base * (2.0 if arm == "orders_b32a7" else 1.0)
                rows.append(row)
    s = summarise(rows)
    r = s["ratios"]["orders_b32a7_over_orders_text/sorted/hit_mops"]
    assert abs(r["point"] - 2.0) < 1e-12, r
    aa = s["ratios"]["orders_text_aa_over_orders_text/shuffled/insert_ns_per_key"]
    assert abs(aa["point"] - 1.0) < 1e-12, aa
    assert s["intervals"]["orders_text/sorted/hit_mops"]["point"] == 11.5
    # A round measured for one arm only is refused, not silently dropped.
    bad = [x for x in rows if not (x["arm"] == "orders_text" and x["round"] == 3)]
    try:
        summarise(bad)
    except RuntimeError:
        pass
    else:
        raise AssertionError("unpaired rounds were accepted")
    assert rotated(ARMS, 1)[0] == ARMS[1] and rotated(ORDERS, 3) == ["shuffled", "sorted"]
    print("leaf_layout_timing: self-test passed")
    return 0


def main(argv: list[str]) -> int:
    if "--self-test" in argv:
        return self_test()

    def arg(name, default=None):
        return argv[argv.index(name) + 1] if name in argv else default

    keys_dir = arg("--keys-dir")
    out = arg("--out")
    if not keys_dir or not out:
        print(__doc__)
        return 2
    return run(Path(keys_dir).resolve(), Path(out).resolve(), int(arg("--rounds", "8")),
               int(arg("--threads", "16")), int(arg("--ops", "2000000")))


if __name__ == "__main__":
    sys.exit(main(sys.argv))
