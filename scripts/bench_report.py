#!/usr/bin/env python3
"""
scripts/bench_report.py — Automated Head-to-Head Benchmark Comparison Report Tool.

Executes standalone fast comparative benchmark sweeps across key distributions
and generates GitHub Flavored Markdown comparison tables ready for PR descriptions,
comments, or documentation.

This harness reports the median of interleaved rounds. A median of three rounds
is not a sampling distribution, so these tables carry no confidence interval and
cannot on their own support a §8.4 wall-clock claim. The interval-bearing
numbers come from the criterion suites, harvested by
`scripts/bench_baseline.py`; pass `--baseline results/baseline_*.json` to append
that section here so the PR comment carries the CI alongside the head-to-head
medians.

Usage:
  python3 scripts/bench_report.py --quick
  python3 scripts/bench_report.py --pop 1000000 --dist all --format markdown
  python3 scripts/bench_report.py --pop 100000 --format json --output report.json
  python3 scripts/bench_report.py --input report.json --format markdown
  python3 scripts/bench_report.py --input report.json --baseline results/baseline_comparative.json
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

sys.path.insert(0, str(Path(__file__).resolve().parent))

import bench_baseline  # noqa: E402


def get_repo_root() -> Path:
    """Returns the repository root directory."""
    return Path(__file__).resolve().parent.parent


def run_benchmark_harness(
    pop: int,
    dist: str,
    rounds: int,
    root: Path,
    target_cpu: Optional[str] = None,
) -> Dict[str, Any]:
    """Executes the Rust benchmark harness and parses its JSON output."""
    cmd = [
        "cargo",
        "run",
        "--release",
        "-p",
        "expanse-trie",
        "--example",
        "bench_lookup_compare",
        "--",
        "--pop",
        str(pop),
        "--dist",
        dist,
        "--rounds",
        str(rounds),
        "--json",
    ]

    env = dict(os.environ)
    if target_cpu and target_cpu not in ("baseline", "none", "generic", "default"):
        rustflags = env.get("RUSTFLAGS", "")
        env["RUSTFLAGS"] = f"{rustflags} -C target-cpu={target_cpu}".strip()

    try:
        proc = subprocess.run(
            cmd,
            cwd=root,
            capture_output=True,
            text=True,
            check=True,
            env=env,
        )
    except subprocess.CalledProcessError as exc:
        print(f"Error running benchmark harness:\n{exc.stderr}", file=sys.stderr)
        raise exc
    except FileNotFoundError:
        print("Error: 'cargo' not found on PATH.", file=sys.stderr)
        sys.exit(1)

    # Locate JSON in stdout (in case Cargo emitted compilation warnings)
    raw_out = proc.stdout
    json_start = raw_out.find("{")
    if json_start == -1:
        print(f"Error: No JSON found in harness output:\n{raw_out}", file=sys.stderr)
        sys.exit(1)

    json_str = raw_out[json_start:]
    try:
        data = json.loads(json_str)
        if target_cpu:
            data["target_cpu"] = target_cpu
        return data
    except json.JSONDecodeError as err:
        print(f"Error parsing harness JSON: {err}\nOutput was:\n{json_str}", file=sys.stderr)
        sys.exit(1)


# Parity band shared by the ratio markers and the printed legend ("⚪ Parity
# (±5%)"). The code and the legend must agree: the legend is the published
# contract, and a wider band would hide real losses behind a parity marker.
PARITY_BAND = 0.05


def classify_ratio(
    expanse_val: float,
    baseline_val: float,
    higher_is_better: bool = True,
    noise_floor: float = PARITY_BAND,
) -> Optional[Tuple[str, float]]:
    """Classifies a measured pair as ('win'|'parity'|'loss', ratio), or None
    when either side is missing/non-positive."""
    if expanse_val <= 0.0 or baseline_val <= 0.0:
        return None
    if higher_is_better:
        ratio = expanse_val / baseline_val
    else:
        ratio = baseline_val / expanse_val
    if ratio >= (1.0 + noise_floor):
        return "win", ratio
    if ratio <= (1.0 - noise_floor):
        return "loss", ratio
    return "parity", ratio


def fmt_speedup(
    expanse_val: float,
    baseline_val: float,
    higher_is_better: bool = True,
    noise_floor: float = PARITY_BAND,
) -> str:
    """Computes and formats a speedup multiplier."""
    classified = classify_ratio(expanse_val, baseline_val, higher_is_better, noise_floor)
    if classified is None:
        return "—"
    kind, ratio = classified
    if kind == "win":
        return f"**{ratio:.2f}x** 🟢"
    if kind == "loss":
        return f"{ratio:.2f}x 🔴"
    return f"{ratio:.2f}x ⚪"


def derive_summary(results: Dict[str, Any]) -> List[str]:
    """Derives win/parity/loss counts for point-lookup latency strictly from
    the parsed results. No prose beyond the tallies: the tables carry the data."""
    baselines = [
        ("btree", "`std::BTreeMap`"),
        ("hashbrown", "`hashbrown::HashMap`"),
        ("libjudy", "`libjudy (stock JudyL)`"),
    ]
    lines: List[str] = []
    for key, label in baselines:
        tally = {"win": 0, "parity": 0, "loss": 0}
        total = 0
        for res in results.values():
            exp_lkp = res.get("expanse", {}).get("lookup_ns", 0.0)
            other = res.get(key) or {}
            classified = classify_ratio(
                exp_lkp, other.get("lookup_ns", 0.0), higher_is_better=False
            )
            if classified is None:
                continue
            tally[classified[0]] += 1
            total += 1
        if total:
            lines.append(
                f"- vs {label}: **{tally['win']} faster · {tally['parity']} parity · "
                f"{tally['loss']} slower** (point lookup, {total} distribution(s))"
            )
    if not lines:
        return []
    return [
        "---",
        f"**Measured Summary** (derived from the tables above; parity band ±{PARITY_BAND * 100:.0f}%):",
        *lines,
    ]


def render_markdown(
    data: Dict[str, Any], baseline: Optional[Dict[str, Any]] = None
) -> str:
    """Formats benchmark results into GitHub Flavored Markdown tables.

    `baseline` is an optional `scripts/bench_baseline.py` artifact; when given,
    its BCa interval table is appended so a wall-clock claim in the same comment
    shows the interval it is gated on (§8.4).
    """
    pop = data.get("pop", 1_000_000)
    system = data.get("system", {})
    os_name = system.get("os", "unknown")
    arch = system.get("arch", "unknown")
    results = data.get("results", {})

    lines: List[str] = [
        "## ⚡ Head-to-Head Benchmark Comparison Report",
        "",
        f"> **Target Population**: $N = {pop:,}$ keys · **System**: `{os_name}/{arch}`",
        "> **Methodology**: Interleaved execution rounds, median reported. Latency in ns/op (lower is better), throughput in Mops/s (higher is better).",
        "",
    ]

    has_libjudy = any(
        res.get("libjudy") is not None for res in results.values()
    )

    for dist, res in results.items():
        exp = res.get("expanse", {})
        hashb = res.get("hashbrown", {})
        btree = res.get("btree", {})
        judy = res.get("libjudy")

        lines.extend([
            f"### Distribution: `{dist}`",
            "",
        ])

        if has_libjudy and judy is not None:
            lines.extend([
                "| Target | Point Lookup (ns) | Lookup (Mops/s) | Cold Insert (Mops/s) | Full Iter (Mops/s) | Range Scan (Mops/s) | Memory (B/key) | Lookup vs BTree | Lookup vs hash | Lookup vs libjudy |",
                "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|",
            ])

            exp_lkp = exp.get("lookup_ns", 0.0)
            hash_lkp = hashb.get("lookup_ns", 0.0)
            btree_lkp = btree.get("lookup_ns", 0.0)
            judy_lkp = judy.get("lookup_ns", 0.0)

            ratio_btree = fmt_speedup(exp_lkp, btree_lkp, higher_is_better=False)
            ratio_hash = fmt_speedup(exp_lkp, hash_lkp, higher_is_better=False)
            ratio_judy = fmt_speedup(exp_lkp, judy_lkp, higher_is_better=False)

            lines.append(
                f"| **`ExpanseMap`** | **{exp.get('lookup_ns', 0.0):.2f}** | **{exp.get('lookup_mops', 0.0):.2f}** | **{exp.get('insert_mops', 0.0):.2f}** | **{exp.get('iter_mops', 0.0):.2f}** | **{exp.get('range_mops', 0.0):.2f}** | **{exp.get('bytes_per_key', 0.0):.2f}** | {ratio_btree} | {ratio_hash} | {ratio_judy} |"
            )
            lines.append(
                f"| `hashbrown::HashMap` | {hashb.get('lookup_ns', 0.0):.2f} | {hashb.get('lookup_mops', 0.0):.2f} | {hashb.get('insert_mops', 0.0):.2f} | {hashb.get('iter_mops', 0.0):.2f} | *N/A (unsupported)* | {hashb.get('bytes_per_key', 0.0):.2f} | — | Baseline | — |"
            )
            lines.append(
                f"| `std::BTreeMap` | {btree.get('lookup_ns', 0.0):.2f} | {btree.get('lookup_mops', 0.0):.2f} | {btree.get('insert_mops', 0.0):.2f} | {btree.get('iter_mops', 0.0):.2f} | {btree.get('range_mops', 0.0):.2f} | {btree.get('bytes_per_key', 0.0):.2f} | Baseline | — | — |"
            )
            lines.append(
                f"| `libjudy (stock JudyL)` | {judy.get('lookup_ns', 0.0):.2f} | {judy.get('lookup_mops', 0.0):.2f} | {judy.get('insert_mops', 0.0):.2f} | {judy.get('iter_mops', 0.0):.2f} | — | {judy.get('bytes_per_key', 0.0):.2f} | — | — | Baseline |"
            )
        else:
            lines.extend([
                "| Target | Point Lookup (ns) | Lookup (Mops/s) | Cold Insert (Mops/s) | Full Iter (Mops/s) | Range Scan (Mops/s) | Memory (B/key) | Lookup vs BTree | Lookup vs hash |",
                "|---|---:|---:|---:|---:|---:|---:|---:|---:|",
            ])

            exp_lkp = exp.get("lookup_ns", 0.0)
            hash_lkp = hashb.get("lookup_ns", 0.0)
            btree_lkp = btree.get("lookup_ns", 0.0)

            ratio_btree = fmt_speedup(exp_lkp, btree_lkp, higher_is_better=False)
            ratio_hash = fmt_speedup(exp_lkp, hash_lkp, higher_is_better=False)

            lines.append(
                f"| **`ExpanseMap`** | **{exp.get('lookup_ns', 0.0):.2f}** | **{exp.get('lookup_mops', 0.0):.2f}** | **{exp.get('insert_mops', 0.0):.2f}** | **{exp.get('iter_mops', 0.0):.2f}** | **{exp.get('range_mops', 0.0):.2f}** | **{exp.get('bytes_per_key', 0.0):.2f}** | {ratio_btree} | {ratio_hash} |"
            )
            lines.append(
                f"| `hashbrown::HashMap` | {hashb.get('lookup_ns', 0.0):.2f} | {hashb.get('lookup_mops', 0.0):.2f} | {hashb.get('insert_mops', 0.0):.2f} | {hashb.get('iter_mops', 0.0):.2f} | *N/A (unsupported)* | {hashb.get('bytes_per_key', 0.0):.2f} | — | Baseline |"
            )
            lines.append(
                f"| `std::BTreeMap` | {btree.get('lookup_ns', 0.0):.2f} | {btree.get('lookup_mops', 0.0):.2f} | {btree.get('insert_mops', 0.0):.2f} | {btree.get('iter_mops', 0.0):.2f} | {btree.get('range_mops', 0.0):.2f} | {btree.get('bytes_per_key', 0.0):.2f} | Baseline | — |"
            )

        lines.append("")

    lines.extend(derive_summary(results))
    lines.extend([
        "",
        "<sub>Medians of interleaved rounds — no sampling distribution, so no confidence "
        "interval and no §8.4 wall-clock claim rests on this table alone. The "
        "interval-bearing arms are in the BCa section (<code>scripts/bench_baseline.py</code>), "
        "when a baseline artifact is supplied.</sub>",
        "",
        f"<sub>🟢 Faster than baseline · ⚪ Parity (±{PARITY_BAND * 100:.0f}%) · 🔴 Slower than baseline. Generated automatically via <code>scripts/bench_report.py</code>.</sub>\n",
    ])

    if baseline is not None:
        lines.extend(["", render_baseline_ci_section(baseline)])

    return "\n".join(lines)


def load_baseline_artifact(path: str | Path) -> Dict[str, Any]:
    """Loads a `results/baseline_*.json` artifact, failing loudly on a bad schema."""
    with open(path, "r", encoding="utf-8") as handle:
        artifact = json.load(handle)
    if artifact.get("schema") != bench_baseline.SCHEMA:
        raise ValueError(
            f"{path}: schema {artifact.get('schema')!r} is not "
            f"{bench_baseline.SCHEMA!r}; not a bench_baseline artifact"
        )
    return artifact


def render_baseline_ci_section(artifact: Dict[str, Any]) -> str:
    """Renders the BCa interval table for a committed baseline artifact.

    Delegates to `bench_baseline.render_markdown` so the report and the gate
    read the same numbers from the same definition (§8.4).
    """
    return bench_baseline.render_markdown(artifact)


def render_table(data: Dict[str, Any]) -> str:
    """Formats benchmark results into plain text terminal tables."""
    pop = data.get("pop", 1_000_000)
    system = data.get("system", {})
    results = data.get("results", {})

    lines: List[str] = [
        f"\n================================================================================",
        f"  HEAD-TO-HEAD BENCHMARK REPORT (N = {pop:,}, {system.get('os')}/{system.get('arch')})",
        f"================================================================================",
    ]

    for dist, res in results.items():
        lines.append(f"\n[ Distribution: {dist} ]")
        lines.append(
            f"{'Target':<22} | {'Lookup (ns)':>11} | {'Lookup (Mops)':>13} | {'Insert (Mops)':>13} | {'Iter (Mops)':>11} | {'Range (Mops)':>12} | {'B/key':>7}"
        )
        lines.append(f"{'-'*22}-+-{'-'*11}-+-{'-'*13}-+-{'-'*13}-+-{'-'*11}-+-{'-'*12}-+-{'-'*7}")

        exp = res.get("expanse", {})
        lines.append(
            f"{'ExpanseMap':<22} | {exp.get('lookup_ns', 0.0):>11.2f} | {exp.get('lookup_mops', 0.0):>13.2f} | {exp.get('insert_mops', 0.0):>13.2f} | {exp.get('iter_mops', 0.0):>11.2f} | {exp.get('range_mops', 0.0):>12.2f} | {exp.get('bytes_per_key', 0.0):>7.2f}"
        )

        hashb = res.get("hashbrown", {})
        lines.append(
            f"{'hashbrown (HashMap)':<22} | {hashb.get('lookup_ns', 0.0):>11.2f} | {hashb.get('lookup_mops', 0.0):>13.2f} | {hashb.get('insert_mops', 0.0):>13.2f} | {hashb.get('iter_mops', 0.0):>11.2f} | {'N/A':>12} | {hashb.get('bytes_per_key', 0.0):>7.2f}"
        )

        btree = res.get("btree", {})
        lines.append(
            f"{'BTreeMap (std)':<22} | {btree.get('lookup_ns', 0.0):>11.2f} | {btree.get('lookup_mops', 0.0):>13.2f} | {btree.get('insert_mops', 0.0):>13.2f} | {btree.get('iter_mops', 0.0):>11.2f} | {btree.get('range_mops', 0.0):>12.2f} | {btree.get('bytes_per_key', 0.0):>7.2f}"
        )

        if judy := res.get("libjudy"):
            lines.append(
                f"{'libjudy (stock)':<22} | {judy.get('lookup_ns', 0.0):>11.2f} | {judy.get('lookup_mops', 0.0):>13.2f} | {judy.get('insert_mops', 0.0):>13.2f} | {judy.get('iter_mops', 0.0):>11.2f} | {'—':>12} | {judy.get('bytes_per_key', 0.0):>7.2f}"
            )

    lines.append("\n================================================================================\n")
    return "\n".join(lines)


def render_extended_pop_markdown(pop_reports: List[Dict[str, Any]]) -> str:
    """Formats multi-population scaling comparison tables."""
    lines: List[str] = [
        "## 📈 Multi-Population Scaling Report ($N = 10\\text{k} \\rightarrow 100\\text{k} \\rightarrow 1\\text{M}$)",
        "",
        "> Evaluates cache-hierarchy scaling transitions from L2 residency ($N=10\\text{k}$) to LLC ($N=100\\text{k}$) and cold DRAM ($N=1\\text{M}$).",
        "",
    ]

    dists = ["sequential", "random", "clustered"]

    for dist in dists:
        lines.extend([
            f"### Distribution: `{dist}` Scaling Matrix",
            "",
            "| Population ($N$) | Container | Point Lookup (ns) | Lookup (Mops/s) | Cold Insert (Mops/s) | Full Iter (Mops/s) | Range Scan (Mops/s) | Memory (B/key) | vs libjudy | vs BTreeMap |",
            "|---|---|---:|---:|---:|---:|---:|---:|---:|---:|",
        ])

        for data in pop_reports:
            pop = data.get("pop", 0)
            res = data.get("results", {}).get(dist, {})
            exp = res.get("expanse", {})
            judy = res.get("libjudy", {})
            btree = res.get("btree", {})

            exp_lkp = exp.get("lookup_ns", 0.0)
            judy_lkp = judy.get("lookup_ns", 0.0) if judy else 0.0
            btree_lkp = btree.get("lookup_ns", 0.0) if btree else 0.0

            ratio_judy = fmt_speedup(exp_lkp, judy_lkp, higher_is_better=False) if judy else "—"
            ratio_btree = fmt_speedup(exp_lkp, btree_lkp, higher_is_better=False) if btree else "—"

            pop_label = f"**{pop:,}**"
            lines.append(
                f"| {pop_label} | **`ExpanseMap`** | **{exp.get('lookup_ns', 0.0):.2f}** | **{exp.get('lookup_mops', 0.0):.2f}** | **{exp.get('insert_mops', 0.0):.2f}** | **{exp.get('iter_mops', 0.0):.2f}** | **{exp.get('range_mops', 0.0):.2f}** | **{exp.get('bytes_per_key', 0.0):.2f}** | {ratio_judy} | {ratio_btree} |"
            )
            if judy:
                lines.append(
                    f"| {pop_label} | `libjudy (JudyL)` | {judy.get('lookup_ns', 0.0):.2f} | {judy.get('lookup_mops', 0.0):.2f} | {judy.get('insert_mops', 0.0):.2f} | {judy.get('iter_mops', 0.0):.2f} | — | {judy.get('bytes_per_key', 0.0):.2f} | Baseline | — |"
                )
            if btree:
                lines.append(
                    f"| {pop_label} | `std::BTreeMap` | {btree.get('lookup_ns', 0.0):.2f} | {btree.get('lookup_mops', 0.0):.2f} | {btree.get('insert_mops', 0.0):.2f} | {btree.get('iter_mops', 0.0):.2f} | {btree.get('range_mops', 0.0):.2f} | {btree.get('bytes_per_key', 0.0):.2f} | — | Baseline |"
                )

        lines.append("")

    return "\n".join(lines)


def render_arch_sweep_markdown(arch_reports: List[Dict[str, Any]]) -> str:
    """Formats target CPU microarchitecture scaling comparison tables."""
    lines: List[str] = [
        "## 🏎️ Target CPU Microarchitecture Scaling Matrix",
        "",
        "> Evaluates instruction set features: `baseline` (generic x86-64-v1) vs `x86-64-v2` (+POPCNT) vs `x86-64-v3` (+AVX2/BMI2) vs `native`.",
        "",
        "| Distribution | Target CPU | Expanse Lookup (ns) | Expanse Lookup (Mops) | Expanse Insert (Mops) | vs Baseline Lookup | vs Stock JudyL |",
        "|---|---|---:|---:|---:|---:|---:|",
    ]

    dists = ["sequential", "random", "clustered"]

    for dist in dists:
        baseline_ns: Optional[float] = None

        for data in arch_reports:
            cpu = data.get("target_cpu", "baseline")
            res = data.get("results", {}).get(dist, {})
            exp = res.get("expanse", {})
            judy = res.get("libjudy", {})

            exp_lkp = exp.get("lookup_ns", 0.0)
            judy_lkp = judy.get("lookup_ns", 0.0) if judy else 0.0

            if baseline_ns is None:
                baseline_ns = exp_lkp
                vs_base = "Baseline"
            else:
                vs_base = fmt_speedup(exp_lkp, baseline_ns, higher_is_better=False)

            vs_judy = fmt_speedup(exp_lkp, judy_lkp, higher_is_better=False) if judy else "—"

            lines.append(
                f"| `{dist}` | `{cpu}` | **{exp_lkp:.2f}** | **{exp.get('lookup_mops', 0.0):.2f}** | **{exp.get('insert_mops', 0.0):.2f}** | {vs_base} | {vs_judy} |"
            )

    lines.append("")
    return "\n".join(lines)


def self_test() -> int:
    """Unit-style checks for the pure rendering helpers. No cargo required."""
    # 1. Legend/band agreement: the parity band is the ±5% the legend prints.
    assert abs(PARITY_BAND - 0.05) < 1e-12, "parity band must match the printed ±5% legend"
    # lower-is-better latency pairs: 104 vs 100 is inside the band, 106/94 are not.
    assert fmt_speedup(100.0, 104.0, higher_is_better=False).endswith("⚪")
    assert fmt_speedup(100.0, 106.0, higher_is_better=False).endswith("🟢")
    assert fmt_speedup(106.0, 100.0, higher_is_better=False).endswith("🔴")
    assert fmt_speedup(0.0, 100.0, higher_is_better=False) == "—"

    # 2. Summary is derived strictly from the parsed results.
    data = {
        "pop": 10_000,
        "system": {"os": "linux", "arch": "x86_64"},
        "results": {
            "sequential": {
                "expanse": {"lookup_ns": 10.0},
                "hashbrown": {"lookup_ns": 12.0},
                "btree": {"lookup_ns": 40.0},
                "libjudy": {"lookup_ns": 11.0},
            },
            "random": {
                "expanse": {"lookup_ns": 30.0},
                "hashbrown": {"lookup_ns": 15.0},
                "btree": {"lookup_ns": 60.0},
                "libjudy": {"lookup_ns": 27.0},
            },
        },
    }
    summary = "\n".join(derive_summary(data["results"]))
    assert "vs `std::BTreeMap`: **2 faster · 0 parity · 0 slower**" in summary, summary
    assert "vs `hashbrown::HashMap`: **1 faster · 0 parity · 1 slower**" in summary, summary
    assert "vs `libjudy (stock JudyL)`: **1 faster · 0 parity · 1 slower**" in summary, summary

    # 3. The rendered report carries no hardcoded findings prose.
    md = render_markdown(data)
    for fabricated in (
        "4× to 10×",
        "3.4× smaller",
        "outperforms stock",
        "across all key distributions",
        "Key Architectural Findings",
    ):
        assert fabricated not in md, f"hardcoded findings prose leaked into report: {fabricated!r}"
    assert "Measured Summary" in md
    assert "±5%" in md

    # 4. No results at all -> no summary block, not a fabricated one.
    assert derive_summary({}) == []

    # 5. The interleaved-median tables disclaim the interval they do not have,
    #    and a supplied baseline artifact surfaces its CI in the same report.
    assert "no confidence interval" in md, md
    artifact = bench_baseline.build_artifact(
        [
            {"id": "map_get/random/1000000/expanse", "samples_ns": [100.0 + i for i in range(40)]},
            {"id": "map_get/random/1000000/btree", "samples_ns": [140.0 + i for i in range(40)]},
        ],
        suite="self-test",
        host_desc="synthetic fixture host",
        commit="0" * 40,
        run_id="self-test",
        confidence=0.95,
        num_resamples=1000,
        seed=42,
        min_n=3,
        fixture=True,
    )
    md_ci = render_markdown(data, artifact)
    assert "Wall-Clock BCa Confidence Intervals" in md_ci
    assert "`map_get/random/1000000/expanse`" in md_ci
    assert "FIXTURE ARTIFACT" in md_ci, "a fixture artifact must be labelled as one"
    # Point estimate enclosed by the interval it is printed next to (§8.4).
    for arm in artifact["arms"]:
        assert arm["ci_lower_ns"] <= arm["point_ns"] <= arm["ci_upper_ns"], arm
    # The head-to-head tables are unchanged by the appended section.
    assert md_ci.startswith(md.split("<sub>Medians of interleaved rounds")[0])
    # A non-artifact JSON is rejected rather than rendered as an empty CI table.
    import tempfile as _tempfile

    with _tempfile.TemporaryDirectory() as _tmp:
        _bad = Path(_tmp) / "not_an_artifact.json"
        _bad.write_text(json.dumps({"schema": "something.else"}), encoding="utf-8")
        try:
            load_baseline_artifact(_bad)
        except ValueError as exc:
            assert "not a bench_baseline artifact" in str(exc), exc
        else:
            raise AssertionError("a foreign schema must be rejected loudly")

    print("bench_report.py --self-test: all checks passed")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Automated Head-to-Head Benchmark Comparison Report Tool for Expanse."
    )
    parser.add_argument(
        "--quick",
        action="store_true",
        help="Fast smoke mode with N = 10,000 keys.",
    )
    parser.add_argument(
        "--extended",
        action="store_true",
        help="Extended mode running multi-population sweeps (10k, 100k, 1M) and arch sweeps.",
    )
    parser.add_argument(
        "--arch-sweep",
        action="store_true",
        help="Run microarchitecture target-cpu sweep (baseline, x86-64-v2, x86-64-v3, native).",
    )
    parser.add_argument(
        "--target-cpu",
        type=str,
        default=None,
        help="Specific target CPU architecture (e.g. x86-64-v3, native; default: generic baseline).",
    )
    parser.add_argument(
        "--pop",
        type=int,
        default=1_000_000,
        help="Target key population (default: 1,000,000; 10,000 in --quick mode).",
    )
    parser.add_argument(
        "--pop-sweep",
        type=str,
        default=None,
        help="Comma-separated populations to sweep (e.g. '10000,100000,1000000').",
    )
    parser.add_argument(
        "--dist",
        choices=["sequential", "random", "clustered", "sparse", "all"],
        default="all",
        help="Key distribution to evaluate (default: all).",
    )
    parser.add_argument(
        "--format",
        choices=["markdown", "json", "table"],
        default="markdown",
        help="Output format (default: markdown).",
    )
    parser.add_argument(
        "--output",
        "-o",
        type=str,
        help="Optional output file path to write results.",
    )
    parser.add_argument(
        "--rounds",
        type=int,
        default=3,
        help="Number of interleaved benchmarking rounds (default: 3).",
    )
    parser.add_argument(
        "--input",
        "-i",
        type=str,
        help="Optional input JSON file with precomputed benchmark results.",
    )
    parser.add_argument(
        "--baseline",
        type=str,
        help=(
            "Optional results/baseline_*.json from scripts/bench_baseline.py. Appends "
            "the BCa 95%% CI table so a wall-clock claim in this report shows the "
            "interval it is gated on (AGENTS.md §8.4)."
        ),
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="Run unit-style checks on the rendering helpers and exit.",
    )

    args = parser.parse_args()
    if args.self_test:
        return self_test()
    root = get_repo_root()

    baseline_artifact: Optional[Dict[str, Any]] = None
    if args.baseline:
        try:
            baseline_artifact = load_baseline_artifact(args.baseline)
        except (OSError, ValueError, json.JSONDecodeError) as exc:
            # Fail loudly (§8.1): a missing or malformed baseline must not
            # silently render a report with no interval where one was asked for.
            print(f"Error loading baseline artifact: {exc}", file=sys.stderr)
            return 1

    if args.input:
        with open(args.input, "r", encoding="utf-8") as f:
            data = json.load(f)
        rendered = (
            render_markdown(data, baseline_artifact)
            if args.format == "markdown"
            else json.dumps(data, indent=2)
        )
    elif args.extended or args.pop_sweep:
        pops = [int(p.strip()) for p in args.pop_sweep.split(",")] if args.pop_sweep else [10_000, 100_000, 1_000_000]
        pop_reports = []
        for p in pops:
            print(f"Running population sweep N = {p:,}...", file=sys.stderr)
            data = run_benchmark_harness(
                pop=p,
                dist=args.dist,
                rounds=args.rounds,
                root=root,
                target_cpu=args.target_cpu,
            )
            pop_reports.append(data)

        if args.arch_sweep:
            is_x86 = platform.machine().lower() in ("x86_64", "amd64", "x86")
            archs = ["baseline", "x86-64-v2", "x86-64-v3", "native"] if is_x86 else ["baseline", "native"]
            arch_reports = []
            for a in archs:
                print(f"Running arch sweep target-cpu = {a} (N = 10,000)...", file=sys.stderr)
                data = run_benchmark_harness(
                    pop=10_000,
                    dist=args.dist,
                    rounds=args.rounds,
                    root=root,
                    target_cpu=a,
                )
                arch_reports.append(data)
            rendered = render_extended_pop_markdown(pop_reports) + "\n\n" + render_arch_sweep_markdown(arch_reports)
        else:
            rendered = render_extended_pop_markdown(pop_reports)
    elif args.arch_sweep:
        is_x86 = platform.machine().lower() in ("x86_64", "amd64", "x86")
        archs = ["baseline", "x86-64-v2", "x86-64-v3", "native"] if is_x86 else ["baseline", "native"]
        arch_reports = []
        pop = 10_000 if args.quick else args.pop
        for a in archs:
            print(f"Running arch sweep target-cpu = {a}...", file=sys.stderr)
            data = run_benchmark_harness(
                pop=pop,
                dist=args.dist,
                rounds=args.rounds,
                root=root,
                target_cpu=a,
            )
            arch_reports.append(data)
        rendered = render_arch_sweep_markdown(arch_reports)
    else:
        pop = 10_000 if args.quick and args.pop == 1_000_000 else args.pop
        data = run_benchmark_harness(
            pop=pop,
            dist=args.dist,
            rounds=args.rounds,
            root=root,
            target_cpu=args.target_cpu,
        )
        if args.format == "json":
            rendered = json.dumps(data, indent=2) + "\n"
        elif args.format == "table":
            rendered = render_table(data)
        else:
            rendered = render_markdown(data, baseline_artifact)

    if args.output:
        out_path = Path(args.output)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(rendered, encoding="utf-8")
        print(f"Report written to {out_path}", file=sys.stderr)
    else:
        print(rendered, end="")

    return 0


if __name__ == "__main__":
    sys.exit(main())
