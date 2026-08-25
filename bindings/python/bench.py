#!/usr/bin/env python3
"""
Cross-Runtime Comparative Benchmark Suite for Expanse Python Bindings (PyO3).
Compares ExpanseMap and ExpanseSet against Python native dict and set.
"""

from __future__ import annotations

import argparse
import json
import random
import sys
import time
import tracemalloc
from pathlib import Path

# If script directory was auto-prepended to sys.path[0], remove it so Python
# imports the compiled package from site-packages (avoiding source-tree .so lookup failure).
script_dir = str(Path(__file__).resolve().parent)
if sys.path and sys.path[0] == script_dir:
    sys.path.pop(0)

try:
    from expanse_trie import ExpanseMap, ExpanseSet
except ImportError:
    sys.path.insert(0, script_dir)
    try:
        from expanse_trie import ExpanseMap, ExpanseSet
    except ImportError:
        print("Error: expanse_trie module not found. Build with maturin develop or pip install .", file=sys.stderr)
        sys.exit(1)


def parse_args():
    p = argparse.ArgumentParser(description="Expanse Python Bindings Benchmark Suite")
    p.add_argument("--pop", type=int, default=100_000, help="Population size (default: 100,000)")
    p.add_argument("--quick", action="store_true", help="Quick mode (N = 20,000)")
    p.add_argument("--json", action="store_true", help="Emit JSON output for machine parsing")
    return p.parse_args()


def generate_keys(pop: int, dist: str = "random") -> list[int]:
    rng = random.Random(0x0DDB_1A5E_5EED_0001)
    if dist == "sequential":
        return list(range(pop))
    elif dist == "clustered":
        out = []
        base = 0
        for i in range(pop):
            if i % 256 == 0:
                base = rng.getrandbits(64) & ~0xFF
            out.append(base + (i % 256))
        return out
    else:
        return [rng.getrandbits(64) for _ in range(pop)]


def measure(fn, rounds: int = 3) -> float:
    best = float("inf")
    for _ in range(rounds):
        t0 = time.perf_counter()
        fn()
        t1 = time.perf_counter()
        dt = t1 - t0
        if dt < best:
            best = dt
    return best


def run_suite(pop: int, dist: str = "random") -> dict:
    keys = generate_keys(pop, dist)
    probe_keys = keys.copy()
    random.Random(0x9E37_79B9).shuffle(probe_keys)

    # 1. ExpanseMap
    exp_map = ExpanseMap()

    def bench_exp_insert():
        exp_map.clear()
        for k in keys:
            exp_map.insert(k, k ^ 0x55)

    exp_insert_s = measure(bench_exp_insert)

    def bench_exp_lookup():
        sink = 0
        for k in probe_keys:
            v = exp_map.get(k)
            if v is not None:
                sink ^= v
        return sink

    exp_lookup_s = measure(bench_exp_lookup)

    def bench_exp_iter():
        count = 0
        for _k, _v in exp_map.items():
            count += 1
        return count

    exp_iter_s = measure(bench_exp_iter)
    exp_bytes_per_key = float(exp_map.mem_used()) / pop

    # 2. Python dict
    py_dict = {}

    def bench_py_insert():
        py_dict.clear()
        for k in keys:
            py_dict[k] = k ^ 0x55

    py_insert_s = measure(bench_py_insert)

    def bench_py_lookup():
        sink = 0
        for k in probe_keys:
            v = py_dict.get(k)
            if v is not None:
                sink ^= v
        return sink

    py_lookup_s = measure(bench_py_lookup)

    def bench_py_iter():
        count = 0
        for _k, _v in py_dict.items():
            count += 1
        return count

    py_iter_s = measure(bench_py_iter)

    tracemalloc.start()
    sample_dict = {k: k ^ 0x55 for k in keys[:10_000]}
    _cur, peak = tracemalloc.get_traced_memory()
    tracemalloc.stop()
    py_bytes_per_key = float(peak) / 10_000 if peak > 0 else 64.0

    # 3. ExpanseSet
    exp_set = ExpanseSet()

    def bench_exp_set_insert():
        exp_set.clear()
        for k in keys:
            exp_set.insert(k)

    exp_set_insert_s = measure(bench_exp_set_insert)

    def bench_exp_set_lookup():
        count = 0
        for k in probe_keys:
            if exp_set.contains(k):
                count += 1
        return count

    exp_set_lookup_s = measure(bench_exp_set_lookup)

    # 4. Python set
    py_set = set()

    def bench_py_set_insert():
        py_set.clear()
        for k in keys:
            py_set.add(k)

    py_set_insert_s = measure(bench_py_set_insert)

    def bench_py_set_lookup():
        count = 0
        for k in probe_keys:
            if k in py_set:
                count += 1
        return count

    py_set_lookup_s = measure(bench_py_set_lookup)

    to_mops = lambda s: (pop / s) / 1e6
    to_ns = lambda s: (s * 1e9) / pop

    return {
        "dist": dist,
        "pop": pop,
        "expanse_map": {
            "insert_mops": to_mops(exp_insert_s),
            "lookup_mops": to_mops(exp_lookup_s),
            "lookup_ns": to_ns(exp_lookup_s),
            "iter_mops": to_mops(exp_iter_s),
            "bytes_per_key": exp_bytes_per_key,
        },
        "python_dict": {
            "insert_mops": to_mops(py_insert_s),
            "lookup_mops": to_mops(py_lookup_s),
            "lookup_ns": to_ns(py_lookup_s),
            "iter_mops": to_mops(py_iter_s),
            "bytes_per_key": py_bytes_per_key,
        },
        "expanse_set": {
            "insert_mops": to_mops(exp_set_insert_s),
            "lookup_mops": to_mops(exp_set_lookup_s),
            "lookup_ns": to_ns(exp_set_lookup_s),
        },
        "python_set": {
            "insert_mops": to_mops(py_set_insert_s),
            "lookup_mops": to_mops(py_set_lookup_s),
            "lookup_ns": to_ns(py_set_lookup_s),
        },
    }


def render_table(results: list[dict]):
    print("\n================================================================================")
    print("  Expanse Python Bindings Comparative Performance Report")
    print("================================================================================")

    for r in results:
        print(f"\n[ Distribution: {r['dist']} | Population: {r['pop']:,} ]")
        print(f"{'Target':<20} | {'Lookup (ns)':>11} | {'Lookup (Mops)':>13} | {'Insert (Mops)':>13} | {'Iter (Mops)':>11} | {'B/key':>8}")
        print(f"{'-'*20}-+-{'-'*11}-+-{'-'*13}-+-{'-'*13}-+-{'-'*11}-+-{'-'*8}")

        em = r["expanse_map"]
        print(f"{'ExpanseMap':<20} | {em['lookup_ns']:>11.2f} | {em['lookup_mops']:>13.2f} | {em['insert_mops']:>13.2f} | {em['iter_mops']:>11.2f} | {em['bytes_per_key']:>8.2f}")

        pd = r["python_dict"]
        print(f"{'Python native dict':<20} | {pd['lookup_ns']:>11.2f} | {pd['lookup_mops']:>13.2f} | {pd['insert_mops']:>13.2f} | {pd['iter_mops']:>11.2f} | {pd['bytes_per_key']:>8.2f}")

        es = r["expanse_set"]
        print(f"{'ExpanseSet':<20} | {es['lookup_ns']:>11.2f} | {es['lookup_mops']:>13.2f} | {es['insert_mops']:>13.2f} | {'—':>11} | {'—':>8}")

        ps = r["python_set"]
        print(f"{'Python native set':<20} | {ps['lookup_ns']:>11.2f} | {ps['lookup_mops']:>13.2f} | {ps['insert_mops']:>13.2f} | {'—':>11} | {'—':>8}")

    print("\n================================================================================\n")


def main():
    args = parse_args()
    pop = 20_000 if args.quick else args.pop
    dists = ["random", "sequential", "clustered"]
    results = [run_suite(pop, d) for d in dists]

    if args.json:
        print(json.dumps({"runtime": "python", "results": results}, indent=2))
    else:
        render_table(results)


if __name__ == "__main__":
    main()
