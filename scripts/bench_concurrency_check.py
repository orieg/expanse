#!/usr/bin/env python3
"""
scripts/bench_concurrency_check.py — Concurrency scaling-ratio baseline check.

Parses the plain-text tables printed by `cargo bench --bench concurrency -p
expanse-trie` (crates/expanse/benches/concurrency.rs) into per-(engine,
workload, threads) ops/s, saves a JSON baseline, and compares **scaling
ratios** (max-threads ÷ min-threads total ops/s per engine/workload) against
a previous baseline.

Ratios — not absolute ops/s — are the gated quantity: they are robust to
host-load drift and catch exactly the collapse class that matters (readers
stopped scaling, e.g. a lock-free path silently degrading to the mutex
fallback), while tolerating scheduler noise. The threshold is deliberately
generous (default: 30% relative ratio drop).

Warn-only by default: regressions are reported but the exit code stays 0
unless --fail-on-regression is passed (mirrors scripts/bench_bindings.py).
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

# Table header: `=== SyncExpanseMap (95% Read / 5% Write) ===`
# Engine names may themselves contain parentheses/angle brackets
# (e.g. `RwLock<BTreeMap<u64, (Vec<u8>, u32)>>`), so the workload group is
# anchored as the *last* parenthesized run without nested parens.
_HEADER_RE = re.compile(r"^=== (?P<engine>.+) \((?P<workload>[^()]+)\) ===$")

# Data row: `       1         18645123           981234      1.00x`
# (read/write ops are printed with `{:>16.0}` — plain integers).
_ROW_RE = re.compile(
    r"^\s*(?P<threads>\d+)\s+(?P<read>\d+)\s+(?P<write>\d+)\s+(?P<scale>[\d.]+|inf|NaN)x\s*$"
)


def parse_bench_output(text: str) -> List[Dict[str, Any]]:
    """Parses bench stdout into a list of result entries.

    Each entry: {"engine", "workload", "threads": {"<t>": {"read_ops",
    "write_ops", "total_ops"}}, "scaling_ratio", "base_threads",
    "max_threads"}. `scaling_ratio` is None when fewer than two thread
    counts were measured.
    """
    results: List[Dict[str, Any]] = []
    current: Optional[Dict[str, Any]] = None

    for line in text.splitlines():
        m = _HEADER_RE.match(line.strip())
        if m:
            current = {
                "engine": m.group("engine"),
                "workload": m.group("workload"),
                "threads": {},
            }
            results.append(current)
            continue
        if current is None:
            continue
        r = _ROW_RE.match(line)
        if r:
            read_ops = float(r.group("read"))
            write_ops = float(r.group("write"))
            current["threads"][r.group("threads")] = {
                "read_ops": read_ops,
                "write_ops": write_ops,
                "total_ops": read_ops + write_ops,
            }

    for entry in results:
        entry.update(_scaling_ratio(entry["threads"]))
    return results


def _scaling_ratio(threads: Dict[str, Dict[str, float]]) -> Dict[str, Any]:
    """Computes max-threads ÷ min-threads total ops/s (min is 1 in normal runs)."""
    counts = sorted(int(t) for t in threads)
    if len(counts) < 2:
        return {"scaling_ratio": None, "base_threads": None, "max_threads": None}
    lo, hi = counts[0], counts[-1]
    base = threads[str(lo)]["total_ops"]
    top = threads[str(hi)]["total_ops"]
    ratio = (top / base) if base > 0 else None
    return {"scaling_ratio": ratio, "base_threads": lo, "max_threads": hi}


def compare_against_baseline(
    current_results: List[Dict[str, Any]],
    baseline_path: str,
    max_ratio_drop_pct: float = 30.0,
) -> Tuple[bool, str]:
    """Compares current scaling ratios against a baseline JSON.

    Returns (has_regression, plain-text/markdown comparison report).
    """
    path = Path(baseline_path)
    if not path.exists():
        return False, f"Concurrency baseline not found at {baseline_path}; skipping ratio check."

    try:
        baseline = json.loads(path.read_text(encoding="utf-8"))
    except Exception as e:  # noqa: BLE001 — report, don't crash the bench job
        return False, f"Error reading concurrency baseline JSON: {e}"

    base_idx: Dict[Tuple[str, str], Dict[str, Any]] = {
        (entry.get("engine", ""), entry.get("workload", "")): entry
        for entry in baseline.get("results", [])
    }

    lines = [
        "## Concurrency Scaling-Ratio Baseline Comparison",
        "",
        f"> **Baseline**: `{baseline_path}` · **Max Allowed Ratio Drop**: {max_ratio_drop_pct:.1f}%",
        "> Ratio = total ops/s at max threads ÷ total ops/s at base threads (per engine/workload).",
        "",
        "| Engine | Workload | Threads | Ratio (current) | Ratio (baseline) | Delta | Status |",
        "|:---|:---|:---|---:|---:|---:|:---:|",
    ]

    regressions: List[str] = []
    for entry in current_results:
        key = (entry["engine"], entry["workload"])
        cur_ratio = entry.get("scaling_ratio")
        base_entry = base_idx.get(key)
        span = (
            f"{entry.get('base_threads')}→{entry.get('max_threads')}"
            if entry.get("max_threads") is not None
            else "—"
        )
        if cur_ratio is None:
            lines.append(f"| `{key[0]}` | {key[1]} | {span} | — | — | — | ⚪ (single thread count) |")
            continue
        if not base_entry or base_entry.get("scaling_ratio") in (None, 0):
            lines.append(f"| `{key[0]}` | {key[1]} | {span} | {cur_ratio:.2f}x | — | — | ⚪ (no baseline) |")
            continue
        base_ratio = float(base_entry["scaling_ratio"])
        delta_pct = (cur_ratio - base_ratio) / base_ratio * 100.0
        regressed = delta_pct < -max_ratio_drop_pct
        status = "🔴" if regressed else ("🟢" if delta_pct >= 0 else "⚪")
        if regressed:
            regressions.append(
                f"{key[0]} [{key[1]}] scaling ratio dropped {delta_pct:+.1f}% "
                f"({cur_ratio:.2f}x vs {base_ratio:.2f}x at {span} threads)"
            )
        lines.append(
            f"| `{key[0]}` | {key[1]} | {span} | {cur_ratio:.2f}x | {base_ratio:.2f}x | {delta_pct:+.1f}% | {status} |"
        )

    lines.append("")
    if regressions:
        lines.append(
            f"> ⚠️ **Scaling-ratio regressions detected ({len(regressions)})**:\n"
            + "\n".join(f"> - {reg}" for reg in regressions)
        )
    else:
        lines.append("> 🟢 **All scaling ratios within baseline tolerance.**")

    return len(regressions) > 0, "\n".join(lines)


# --------------------------------------------------------------------------
# Self-test (the repo has no Python test harness in CI; nightly runs this
# before the real check).
# --------------------------------------------------------------------------

_SELF_TEST_SAMPLE = """
=== SyncExpanseMap (100% Read / 0% Write) ===
 threads     read ops/sec    write ops/sec      scale
       1         20000000                0      1.00x
       4         70000000                0      3.50x
      16        240000000                0     12.00x

=== RwLock<BTreeMap<u64, (Vec<u8>, u32)>> (50% Read / 50% Write) ===
 threads     read ops/sec    write ops/sec      scale
       1          3000000          3000000      1.00x
      16          4000000          4000000      1.33x
"""

_SELF_TEST_COLLAPSED = _SELF_TEST_SAMPLE.replace("       4         70000000", "       4         20000000").replace(
    "      16        240000000", "      16         21000000"
)


def self_test() -> int:
    import tempfile

    results = parse_bench_output(_SELF_TEST_SAMPLE)
    assert len(results) == 2, f"expected 2 tables, got {len(results)}"

    m = results[0]
    assert m["engine"] == "SyncExpanseMap" and m["workload"] == "100% Read / 0% Write"
    assert m["threads"]["1"]["total_ops"] == 20_000_000.0
    assert m["threads"]["16"]["read_ops"] == 240_000_000.0
    assert m["base_threads"] == 1 and m["max_threads"] == 16
    assert abs(m["scaling_ratio"] - 12.0) < 1e-9, m["scaling_ratio"]

    b = results[1]
    assert b["engine"] == "RwLock<BTreeMap<u64, (Vec<u8>, u32)>>", b["engine"]
    assert b["workload"] == "50% Read / 50% Write"
    assert b["threads"]["1"]["total_ops"] == 6_000_000.0
    assert abs(b["scaling_ratio"] - (8_000_000.0 / 6_000_000.0)) < 1e-9

    with tempfile.TemporaryDirectory() as td:
        baseline = Path(td) / "baseline.json"
        baseline.write_text(json.dumps({"schema": 1, "results": results}), encoding="utf-8")

        # Round-trip: identical run vs its own baseline → no regression.
        has_reg, report = compare_against_baseline(results, str(baseline))
        assert not has_reg, f"false positive on identical results:\n{report}"

        # Synthetic collapse (map ratio 12.0x → ~1.05x) → regression flagged.
        collapsed = parse_bench_output(_SELF_TEST_COLLAPSED)
        has_reg, report = compare_against_baseline(collapsed, str(baseline))
        assert has_reg, f"collapse not detected:\n{report}"
        assert "SyncExpanseMap" in report

        # The generous threshold tolerates a moderate (< threshold) drift.
        drifted = json.loads(json.dumps(results))
        drifted[0]["scaling_ratio"] = 12.0 * 0.8  # -20% > -30% threshold
        has_reg, _ = compare_against_baseline(drifted, str(baseline))
        assert not has_reg, "20% ratio drift must pass the 30% threshold"

        # Missing baseline file → no regression, informative message.
        has_reg, report = compare_against_baseline(results, str(Path(td) / "absent.json"))
        assert not has_reg and "not found" in report

    print("bench_concurrency_check self-test: OK (parser, ratio math, round-trip, collapse detection)")
    return 0


def parse_args():
    p = argparse.ArgumentParser(description="Expanse concurrency scaling-ratio baseline check")
    p.add_argument("--input", type=str, help="Bench output file to parse (default: stdin)")
    p.add_argument("--save-baseline", type=str, help="Save parsed results to baseline JSON file")
    p.add_argument("--check-baseline", type=str, help="Compare scaling ratios against baseline JSON file")
    p.add_argument(
        "--max-ratio-drop-pct",
        type=float,
        default=30.0,
        help="Max allowed relative scaling-ratio drop pct (default: 30%%)",
    )
    p.add_argument(
        "--fail-on-regression",
        action="store_true",
        help="Exit nonzero on regression (default: warn-only)",
    )
    p.add_argument("--output", type=str, help="Also save the comparison report to a file")
    p.add_argument("--self-test", action="store_true", help="Run the built-in parser/ratio self-test and exit")
    return p.parse_args()


def main() -> int:
    args = parse_args()
    if args.self_test:
        return self_test()

    text = Path(args.input).read_text(encoding="utf-8") if args.input else sys.stdin.read()
    results = parse_bench_output(text)
    if not results:
        print("[WARN] no concurrency bench tables found in input", file=sys.stderr)

    has_reg = False
    report = ""
    if args.check_baseline:
        has_reg, report = compare_against_baseline(
            results, args.check_baseline, max_ratio_drop_pct=args.max_ratio_drop_pct
        )
        print(report)

    if args.save_baseline:
        out = Path(args.save_baseline)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps({"schema": 1, "results": results}, indent=2), encoding="utf-8")
        print(f"Saved concurrency baseline to {args.save_baseline}", file=sys.stderr)

    if args.output and report:
        Path(args.output).write_text(report, encoding="utf-8")

    if has_reg and args.fail_on_regression:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
