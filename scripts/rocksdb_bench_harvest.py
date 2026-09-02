#!/usr/bin/env python3
"""Turns repeated `integrations/rocksdb` bench runs into a BCa interval artifact.

The C++ memtable bench prints one table per invocation and computes nothing
across runs, so a single invocation yields point estimates with no sampling
distribution — publishable only as a hedge, which is why #382 item 5's scan and
lookup ratios stayed pending while the density figure beside them shipped.

This harvests N rounds into the same `expanse.baseline.v1` artifact shape the
criterion suites use, so a published ratio resolves to an artifact and is gated
on its CI lower bound (AGENTS.md §8.4) rather than on a point estimate.

Two metric classes, handled differently on purpose:

  * Throughput (Mops/s) is a sampling metric: each arm gets a BCa interval, and
    each Expanse-vs-baseline speedup gets a two-sample BCa ratio interval.
  * Bytes-per-entry is deterministic allocator accounting — identical every
    run. It carries no interval, and the artifact says why rather than leaving
    the field silently absent. An interval on an exact count would be wrong,
    not missing (§8.4 scopes CI requirements to continuous metrics).

Usage:
    python3 scripts/rocksdb_bench_harvest.py --round r1.txt --round r2.txt ... \
        --out docs/benchmarks/rocksdb_memtable/results/baseline_rocksdb.json --host-desc "..." --commit ... --run-id ...
    python3 scripts/rocksdb_bench_harvest.py --self-test
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
from bca_bootstrap import bca_bootstrap_ci, bca_bootstrap_ratio_ci  # noqa: E402

SCHEMA = "expanse.baseline.v1"
CONFIDENCE = 0.95
RESAMPLES = 2000
SEED = 42

BENCH_HEADER = re.compile(r"^---\s*Benchmark\s+\d+:\s*(\w+)")
MEMORY_HEADER = re.compile(r"^---\s*Memory Density")
THROUGHPUT = re.compile(r"^\s+([A-Za-z]\w*(?:\s*\([^)]+\))?):\s+([\d.]+)\s+Mops/s")
DENSITY = re.compile(r"^\s+([A-Za-z]\w*):\s+[\d.]+\s*MB\s+\(([\d.]+)\s*B/entry\)")

SUBJECT = "ExpanseMemTable"


def _load_average() -> list[float] | None:
    """The host's 1/5/15-minute load average, or None where unavailable.

    `bench_baseline.py` records this in every criterion artifact and this
    harvester did not, so a rocksdb interval could not be checked against the
    contention that produced it. Local benchmark hygiene turns on exactly that
    number: an interval measured beside a busy neighbour is tight and wrong,
    and without the figure a reader has to take the quiet host on trust.
    """
    try:
        return [round(v, 2) for v in os.getloadavg()]
    except (OSError, AttributeError):
        return None


def parse_round(text: str) -> tuple[dict[str, dict[str, float]], dict[str, float]]:
    """One round's output -> ({benchmark -> {impl -> Mops/s}}, {impl -> B/entry})."""
    throughput: dict[str, dict[str, float]] = {}
    density: dict[str, float] = {}
    section: str | None = None
    in_memory = False
    for line in text.splitlines():
        head = BENCH_HEADER.match(line)
        if head:
            section, in_memory = head.group(1), False
            throughput.setdefault(section, {})
            continue
        if MEMORY_HEADER.match(line):
            section, in_memory = None, True
            continue
        if in_memory:
            d = DENSITY.match(line)
            if d:
                density[d.group(1)] = float(d.group(2))
            continue
        if section:
            t = THROUGHPUT.match(line)
            if t:
                throughput[section][t.group(1).strip()] = float(t.group(2))
    return {k: v for k, v in throughput.items() if v}, density


def build(
    rounds: list[str], host_desc: str, commit: str, run_id: str
) -> dict[str, Any]:
    parsed = [parse_round(t) for t in rounds]
    if len(parsed) < 3:
        raise ValueError(
            f"need at least 3 rounds for a BCa interval, got {len(parsed)} — "
            "a point estimate is not publishable under AGENTS.md 8.4"
        )

    benches = sorted(parsed[0][0])
    if not benches:
        raise ValueError("no benchmark sections parsed; refusing to write an empty artifact")

    arms: list[dict[str, Any]] = []
    ratios: list[dict[str, Any]] = []
    for bench in benches:
        impls = sorted(parsed[0][0][bench])
        series = {
            impl: [p[0][bench][impl] for p in parsed if impl in p[0].get(bench, {})]
            for impl in impls
        }
        # An implementation missing from any round would silently shorten its
        # series and produce an interval over fewer samples than claimed.
        for impl, vals in series.items():
            if len(vals) != len(parsed):
                raise ValueError(
                    f"{bench}/{impl}: {len(vals)} samples across {len(parsed)} rounds — "
                    "an arm absent from a round would understate n"
                )
        for impl in impls:
            point, lo, hi = bca_bootstrap_ci(series[impl], CONFIDENCE, RESAMPLES, SEED)
            arms.append(
                {
                    "id": f"{bench}/{impl}",
                    "benchmark": bench,
                    "implementation": impl,
                    "unit": "Mops_per_second",
                    "n": len(series[impl]),
                    "point": round(point, 4),
                    "ci_lower": round(lo, 4),
                    "ci_upper": round(hi, 4),
                    "samples": series[impl],
                }
            )
        for impl in impls:
            if not impl.startswith(SUBJECT):
                continue
            for baseline in impls:
                if baseline.startswith(SUBJECT):
                    continue
                r, lo, hi = bca_bootstrap_ratio_ci(
                    series[impl], series[baseline], CONFIDENCE, RESAMPLES, SEED
                )
                ratios.append(
                    {
                        "id": f"{bench}/{impl}_vs_{baseline}",
                        "benchmark": bench,
                        "subject": impl,
                        "baseline": baseline,
                        "unit": "ratio_of_mean_Mops_higher_is_better",
                        "n": len(series[impl]),
                        "ratio": round(r, 4),
                        "ci_lower": round(lo, 4),
                        "ci_upper": round(hi, 4),
                        "beats_baseline": lo > 1.0,
                    }
                )

    dens = parsed[0][1]
    for p in parsed[1:]:
        if p[1] != dens:
            raise ValueError(
                f"bytes-per-entry differed across rounds ({dens} vs {p[1]}) — "
                "it is deterministic allocator accounting and must not vary"
            )

    return {
        "schema": SCHEMA,
        "kind": "wall_clock_bca",
        "suite": "rocksdb",
        "fixture": "integrations/rocksdb/benches/bench_memtable.cc",
        "provenance": {
            # Anonymised hardware description, never a hostname (section 7).
            "host_description": host_desc,
            "commit": commit,
            "run_id": run_id,
            "rounds": len(parsed),
            "load_average_at_harvest": _load_average(),
            "generated_by": "scripts/rocksdb_bench_harvest.py",
            "source": "repeated `make -C integrations/rocksdb bench` invocations",
        },
        "statistics": {
            "estimator": "mean of per-round throughput (Mops/s)",
            "method": "BCa bootstrap; two-sample for ratios",
            "confidence": CONFIDENCE,
            "num_resamples": RESAMPLES,
            "seed": SEED,
            "point_and_interval_share_one_definition": True,
        },
        "arms": arms,
        "ratios": ratios,
        "memory_bytes_per_entry": {
            "values": dens,
            "interval": None,
            "why_no_interval": (
                "Deterministic allocator accounting — identical in every round, "
                "and verified so here. AGENTS.md 8.4 scopes interval requirements "
                "to continuous and sampling metrics; an interval on an exact "
                "count would be wrong, not missing."
            ),
        },
    }


SELF_TEST_ROUND = """\
--- Benchmark 1: fillrandom (N = 100000) ---
  ExpanseMemTable: {a} Mops/s (22.37 ms)
  SkipListRep:     {b} Mops/s (31.68 ms)

--- Benchmark 4: prefixscan (Sequential Scan across 100K entries) ---
  ExpanseMemTable (Iterator): {c} Mops/s
  SkipListRep:                {d} Mops/s

--- Memory Density & Footprint Analysis ---
  ExpanseMemTable: 1.26 MB (13.2 B/entry)
  SkipListRep:     1.8 MB (18.7 B/entry)
"""


def self_test() -> int:
    r = SELF_TEST_ROUND.format(a=4.47, b=3.16, c=155.04, d=46.67)
    tp, dens = parse_round(r)
    assert set(tp) == {"fillrandom", "prefixscan"}, tp
    assert tp["fillrandom"]["ExpanseMemTable"] == 4.47
    assert tp["prefixscan"]["ExpanseMemTable (Iterator)"] == 155.04, tp["prefixscan"]
    assert dens == {"ExpanseMemTable": 13.2, "SkipListRep": 18.7}, dens

    rounds = [
        SELF_TEST_ROUND.format(a=4.4 + i * 0.05, b=3.1 + i * 0.02, c=155.0, d=46.6)
        for i in range(5)
    ]
    art = build(rounds, "test host", "abc1234", "run/1")
    assert art["schema"] == SCHEMA and art["kind"] == "wall_clock_bca"
    assert art["provenance"]["rounds"] == 5
    # Contention is part of a wall-clock artifact's provenance: an interval
    # measured beside a busy neighbour is tight and wrong.
    la = art["provenance"]["load_average_at_harvest"]
    assert la is None or (len(la) == 3 and all(isinstance(v, float) for v in la)), la
    for a in art["arms"]:
        assert a["ci_lower"] <= a["point"] <= a["ci_upper"], a
        assert a["n"] == 5, a
    fill = next(r for r in art["ratios"] if r["id"].startswith("fillrandom/"))
    assert fill["subject"] == "ExpanseMemTable" and fill["baseline"] == "SkipListRep"
    assert fill["ci_lower"] <= fill["ratio"] <= fill["ci_upper"], fill
    assert fill["beats_baseline"] is True, fill
    # Density is deterministic and carries no interval.
    assert art["memory_bytes_per_entry"]["interval"] is None
    assert art["memory_bytes_per_entry"]["values"]["SkipListRep"] == 18.7

    # Fewer than three rounds cannot yield an interval and must refuse.
    try:
        build(rounds[:2], "h", "c", "r")
        raise AssertionError("2 rounds must be refused")
    except ValueError as e:
        assert "at least 3 rounds" in str(e), e
    # Density drifting across rounds means the accounting is not deterministic.
    drifted = list(rounds)
    drifted[2] = drifted[2].replace("13.2 B/entry", "13.9 B/entry")
    try:
        build(drifted, "h", "c", "r")
        raise AssertionError("drifting density must be refused")
    except ValueError as e:
        assert "deterministic allocator accounting" in str(e), e
    # An arm missing from one round must not silently shorten its series.
    short = list(rounds)
    short[1] = short[1].replace("  SkipListRep:     3.12 Mops/s (31.68 ms)\n", "")
    if short[1] != rounds[1]:
        try:
            build(short, "h", "c", "r")
            raise AssertionError("a missing arm must be refused")
        except ValueError as e:
            assert "understate n" in str(e), e

    print("rocksdb_bench_harvest.py --self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--round", action="append", default=[], help="one round's output file")
    ap.add_argument("--out", help="artifact path to write")
    ap.add_argument("--host-desc", default="")
    ap.add_argument("--commit", default="")
    ap.add_argument("--run-id", default="")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        return self_test()
    if not args.round or not args.out:
        ap.error("--round (repeatable) and --out are required")

    rounds = [Path(f).read_text(encoding="utf-8") for f in args.round]
    art = build(rounds, args.host_desc, args.commit, args.run_id)
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    Path(args.out).write_text(json.dumps(art, indent=2) + "\n", encoding="utf-8")
    print(
        f"rocksdb_bench_harvest.py: {len(art['arms'])} arm(s), "
        f"{len(art['ratios'])} ratio(s) from {len(rounds)} round(s) -> {args.out}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
