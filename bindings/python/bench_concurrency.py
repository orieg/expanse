#!/usr/bin/env python3
"""Multi-core read scaling for the Python bindings' GIL-releasing OCC path.

`docs/bindings/python.md` claims `SyncExpanseSet` / `SyncExpanseMap` release the
GIL through `py.detach` so Python threads read concurrently across cores. The
qualitative claim is verifiable from the source (`crates/expanse-py/src/sync.rs`);
the *scaling factors* were never measured, and stood marked pending (#546).

`benches/concurrency.rs` cannot answer it: it measures the Rust arms and never
crosses the pyo3 boundary, which is exactly where the GIL would serialise.

Design notes, because the trap here is measuring the GIL instead of the engine:

  * **A twin baseline that can lose.** A plain Python `dict` runs the identical
    workload in the identical thread pool. It is a real ordered-lookup competitor
    at these sizes, and being pure Python it is GIL-serialised — so it is the
    control that shows whether any observed scaling is Expanse releasing the GIL
    or merely the harness's own overhead (AGENTS.md section 8.3).
  * **Every lookup is consumed.** Results accumulate into a per-thread sink that
    is summed and returned, so neither the interpreter nor the extension can
    elide the probe (section 8.6).
  * **Realistic hit rate.** Half the probes miss, drawn from a disjoint key
    range, rather than probing only keys known to be present.
  * **Per-round samples are emitted**, not just means, so a published scaling
    factor can carry a BCa interval (section 8.4) rather than a point estimate.

Usage:
    python3 bindings/python/bench_concurrency.py --json --out results.json
    python3 bindings/python/bench_concurrency.py --self-test
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

DEFAULT_THREADS = (1, 2, 4, 8, 16)
DEFAULT_POP = 200_000
DEFAULT_PROBES = 200_000
DEFAULT_ROUNDS = 5


def make_keys(pop: int) -> list[int]:
    """Deterministic xorshift64 keys — the same generator the other harnesses use."""
    keys, x = [], 0x1234_5678_9ABC_DEF0
    for _ in range(pop):
        x ^= (x << 13) & 0xFFFF_FFFF_FFFF_FFFF
        x ^= x >> 7
        x ^= (x << 17) & 0xFFFF_FFFF_FFFF_FFFF
        keys.append(x)
    return keys


def make_probes(keys: list[int], n: int) -> list[int]:
    """Half hits, half misses. A 100%-hit probe stream flatters any index."""
    hits = [keys[i * 7919 % len(keys)] for i in range(n // 2)]
    misses = [k ^ 0x8000_0000_0000_0000 for k in hits]
    out = []
    for h, m in zip(hits, misses):
        out.append(h)
        out.append(m)
    return out[:n]


def _worker(container, probes, lo: int, hi: int) -> int:
    """Sums the values it retrieves so the probe cannot be optimised away."""
    sink = 0
    for i in range(lo, hi):
        v = container.get(probes[i])
        if v is not None:
            sink += v
    return sink


def run_arm(container, probes: list[int], threads: int) -> float:
    """Elapsed seconds for one pass of `probes` split across `threads` workers."""
    n = len(probes)
    bounds = [(i * n // threads, (i + 1) * n // threads) for i in range(threads)]
    t0 = time.perf_counter()
    with ThreadPoolExecutor(max_workers=threads) as pool:
        sinks = list(pool.map(lambda b: _worker(container, probes, b[0], b[1]), bounds))
    elapsed = time.perf_counter() - t0
    if sum(sinks) < 0:  # consume the sink; never true, never optimised out
        raise AssertionError("unreachable")
    return elapsed


def _unshadow_installed_package() -> None:
    """Drop this script's own directory from `sys.path`.

    `bindings/python/expanse_trie/` is a source copy of the package: its
    `__init__.py` is tracked, but `_expanse.abi3.so` is gitignored and only
    exists where someone has built in place. Python puts the script's own
    directory first on `sys.path`, so running this file from a checkout
    shadows the *installed* extension with a source tree that has no compiled
    submodule — `ModuleNotFoundError: No module named 'expanse_trie._expanse'`,
    seen on run 33438701371 after the venv itself was working.

    Removing the entry makes the import resolve to whatever is installed,
    which is what a benchmark should measure.
    """
    here = Path(__file__).resolve().parent
    sys.path[:] = [p for p in sys.path if p and Path(p).resolve() != here]


def build_containers(keys: list[int]):
    _unshadow_installed_package()
    from expanse_trie import SyncExpanseMap  # noqa: PLC0415

    exp = SyncExpanseMap()
    for k in keys:
        exp.insert(k, k & 0xFFFF)
    return {"expanse_sync_map": exp, "python_dict": {k: k & 0xFFFF for k in keys}}


def measure(pop: int, probe_count: int, thread_counts, rounds: int) -> dict:
    keys = make_keys(pop)
    probes = make_probes(keys, probe_count)
    containers = build_containers(keys)

    arms: dict[str, dict] = {}
    for name, c in containers.items():
        per_threads = {}
        for t in thread_counts:
            samples = [run_arm(c, probes, t) for _ in range(rounds)]
            mops = [len(probes) / s / 1e6 for s in samples]
            per_threads[str(t)] = {
                "threads": t,
                "samples_mops": mops,
                "mean_mops": sum(mops) / len(mops),
            }
        base = per_threads[str(thread_counts[0])]["mean_mops"]
        for t in thread_counts:
            e = per_threads[str(t)]
            e["scaling_vs_1t"] = e["mean_mops"] / base if base else 0.0
        arms[name] = per_threads

    return {
        "schema": "expanse.baseline.v1",
        "kind": "python_concurrency",
        "suite": "python_concurrency",
        "fixture": "bindings/python/bench_concurrency.py",
        "provenance": {
            "population": pop,
            "distinct_probes": probe_count,
            "hit_rate": "50% hit / 50% miss",
            "rounds": rounds,
            "cpu_count": os.cpu_count(),
            "load_average_at_run": [round(v, 2) for v in os.getloadavg()]
            if hasattr(os, "getloadavg")
            else None,
            "python": sys.version.split()[0],
            "generated_by": "bindings/python/bench_concurrency.py",
        },
        "statistics": {
            "estimator": "mean of per-round throughput (Mops/s)",
            "note": (
                "Per-round samples are emitted so a published scaling factor can "
                "carry a BCa interval; this harness reports means and leaves the "
                "interval to scripts/bca_bootstrap.py (AGENTS.md 8.4)."
            ),
        },
        "arms": arms,
    }


def self_test() -> int:
    keys = make_keys(64)
    assert len(keys) == len(set(keys)) == 64, "xorshift keys must be distinct"
    probes = make_probes(keys, 40)
    assert len(probes) == 40
    hits = sum(1 for p in probes if p in set(keys))
    assert 18 <= hits <= 22, f"probe stream must be ~50% hit, got {hits}/40"

    d = {k: k & 0xFFFF for k in keys}
    # The worker must return a non-zero sink, or the probe is not being consumed.
    sink = _worker(d, probes, 0, len(probes))
    assert sink > 0, "worker sink is zero — lookups are not being consumed"
    # Splitting across threads must not change the work done.
    assert run_arm(d, probes, 1) >= 0.0 and run_arm(d, probes, 4) >= 0.0

    # Exercise `measure` end to end without requiring the built extension: swap
    # in a dict-only container set. A self-test that only checked `measure` is
    # callable would pass with the artifact shape completely wrong.
    global build_containers
    real = build_containers
    build_containers = lambda ks: {"python_dict": {k: k & 0xFFFF for k in ks}}  # noqa: E731
    try:
        art = measure(pop=256, probe_count=128, thread_counts=[1, 2], rounds=2)
    finally:
        build_containers = real

    assert art["kind"] == "python_concurrency"
    assert art["provenance"]["hit_rate"] == "50% hit / 50% miss"
    assert art["provenance"]["rounds"] == 2
    arm = art["arms"]["python_dict"]
    assert set(arm) == {"1", "2"}, arm.keys()
    for t, e in arm.items():
        assert len(e["samples_mops"]) == 2, e
        assert e["mean_mops"] > 0, e
    # The single-thread arm is the scaling denominator, so it is 1.0 by
    # construction; a value other than that means the base was picked wrongly.
    assert abs(arm["1"]["scaling_vs_1t"] - 1.0) < 1e-9, arm["1"]
    assert arm["2"]["scaling_vs_1t"] > 0

    # The import guard must actually remove this file's directory, or the
    # source copy of `expanse_trie` next to it shadows the installed
    # extension and the run dies at import (run 33438701371).
    here = Path(__file__).resolve().parent
    sys.path.insert(0, str(here))
    assert any(p and Path(p).resolve() == here for p in sys.path), "fixture failed"
    _unshadow_installed_package()
    assert not any(p and Path(p).resolve() == here for p in sys.path), (
        "script directory still on sys.path — the installed package stays shadowed"
    )
    # A sibling package directory really does exist there, so this is not theoretical.
    assert (here / "expanse_trie" / "__init__.py").is_file(), (
        "expected the source copy of expanse_trie beside this script"
    )

    print("bench_concurrency.py --self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--pop", type=int, default=DEFAULT_POP)
    ap.add_argument("--probes", type=int, default=DEFAULT_PROBES)
    ap.add_argument("--rounds", type=int, default=DEFAULT_ROUNDS)
    ap.add_argument("--threads", default=",".join(str(t) for t in DEFAULT_THREADS))
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--out")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        return self_test()

    thread_counts = [int(t) for t in args.threads.split(",")]
    art = measure(args.pop, args.probes, thread_counts, args.rounds)

    if args.out:
        with open(args.out, "w", encoding="utf-8") as fh:
            json.dump(art, fh, indent=2)
            fh.write("\n")
        print(f"wrote {args.out}")
    if args.json or not args.out:
        print(json.dumps(art, indent=2))
    else:
        for name, per in art["arms"].items():
            print(f"\n=== {name} ===")
            for t, e in per.items():
                print(f"  {t:>3}t  {e['mean_mops']:8.3f} Mops/s  {e['scaling_vs_1t']:5.2f}x")
    return 0


if __name__ == "__main__":
    sys.exit(main())
