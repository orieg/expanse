#!/usr/bin/env python3
"""
scripts/bench_bindings.py — Unified Cross-Language Comparative Benchmark Suite Orchestrator.

Discovers and executes available binding benchmark harnesses (Node.js, WASM, Go,
Python, PHP, Ruby, Java, .NET) and outputs unified comparative performance matrices,
with baseline regression detection for nightly gating.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

REPO_ROOT = Path(__file__).resolve().parent.parent


def parse_args():
    p = argparse.ArgumentParser(description="Unified Expanse Cross-Language Benchmark Suite")
    p.add_argument("--quick", action="store_true", help="Run in quick mode (smaller N)")
    p.add_argument("--runtimes", nargs="+", help="Specific runtimes to benchmark (node, wasm, go, python, php, ruby)")
    p.add_argument("--json", action="store_true", help="Emit raw JSON results to stdout")
    p.add_argument("--output", type=str, help="Save markdown report to file")
    p.add_argument("--save-baseline", type=str, help="Save results to baseline JSON file")
    p.add_argument("--check-baseline", type=str, help="Compare results against baseline JSON file")
    p.add_argument("--max-regression-pct", type=float, default=25.0, help="Max allowed throughput regression pct (default: 25%)")
    p.add_argument("--max-memory-regression-pct", type=float, default=10.0, help="Max allowed memory regression pct (default: 10%)")
    return p.parse_args()


def run_node_benchmark(quick: bool) -> Optional[Dict[str, Any]]:
    node_dir = REPO_ROOT / "crates" / "expanse-node"
    bench_js = node_dir / "bench.js"
    if not shutil.which("node") or not bench_js.exists():
        return None

    cmd = ["node", str(bench_js), "--json"]
    if quick:
        cmd.append("--quick")

    try:
        proc = subprocess.run(cmd, cwd=node_dir, capture_output=True, text=True, check=True)
        return json.loads(proc.stdout)
    except Exception as e:
        print(f"[WARN] Node.js benchmark failed: {e}", file=sys.stderr)
        return None


def run_wasm_benchmark(quick: bool) -> Optional[Dict[str, Any]]:
    wasm_dir = REPO_ROOT / "crates" / "expanse-wasm"
    bench_js = wasm_dir / "tests" / "bench.js"
    if not shutil.which("node") or not bench_js.exists():
        return None

    cmd = ["node", str(bench_js), "--json"]
    if quick:
        cmd.append("--quick")

    try:
        proc = subprocess.run(cmd, cwd=wasm_dir, capture_output=True, text=True, check=True)
        return json.loads(proc.stdout)
    except Exception as e:
        print(f"[WARN] WASM benchmark failed: {e}", file=sys.stderr)
        return None


def run_python_benchmark(quick: bool) -> Optional[Dict[str, Any]]:
    py_dir = REPO_ROOT / "bindings" / "python"
    bench_py = py_dir / "bench.py"
    if not shutil.which("python3") or not bench_py.exists():
        return None

    cmd = ["python3", str(bench_py), "--json"]
    if quick:
        cmd.append("--quick")

    try:
        proc = subprocess.run(cmd, cwd=py_dir, capture_output=True, text=True, check=True)
        return json.loads(proc.stdout)
    except Exception as e:
        print(f"[WARN] Python benchmark failed: {e}", file=sys.stderr)
        return None


def run_php_benchmark(quick: bool) -> Optional[Dict[str, Any]]:
    php_dir = REPO_ROOT / "bindings" / "php"
    bench_php = php_dir / "bench.php"
    if not shutil.which("php") or not bench_php.exists():
        return None

    cmd = ["php", str(bench_php), "--json"]
    if quick:
        cmd.append("--quick")

    try:
        proc = subprocess.run(cmd, cwd=php_dir, capture_output=True, text=True, check=True)
        return json.loads(proc.stdout)
    except Exception as e:
        print(f"[WARN] PHP benchmark failed: {e}", file=sys.stderr)
        return None


def run_ruby_benchmark(quick: bool) -> Optional[Dict[str, Any]]:
    rb_dir = REPO_ROOT / "bindings" / "ruby"
    bench_rb = rb_dir / "bench.rb"
    if not shutil.which("ruby") or not bench_rb.exists():
        return None

    cmd = ["ruby", str(bench_rb), "--json"]
    if quick:
        cmd.append("--quick")

    try:
        proc = subprocess.run(cmd, cwd=rb_dir, capture_output=True, text=True, check=True)
        return json.loads(proc.stdout)
    except Exception as e:
        print(f"[WARN] Ruby benchmark failed: {e}", file=sys.stderr)
        return None


def format_markdown_report(all_results: List[Dict[str, Any]]) -> str:
    lines = [
        "# Cross-Language Bindings Comparative Performance Report",
        "",
        "Comparing Expanse bindings against standard runtime collections (`Map`, `dict`, `array`, `Hash`) across key operations and memory density.",
        "",
    ]

    for item in all_results:
        runtime = item.get("runtime", "unknown").upper()
        results = item.get("results", [])
        lines.append(f"## Runtime: `{runtime}`")
        lines.append("")

        for r in results:
            dist = r.get("dist", "random")
            pop = r.get("pop", 0)
            lines.append(f"### Distribution: `{dist}` (N = {pop:,})")
            lines.append("")
            lines.append("| Target Collection | Lookup Latency (ns) | Lookup (Mops/s) | Insert (Mops/s) | Bytes / Key |")
            lines.append("|:---|---:|---:|---:|---:|")

            if "expanse_map" in r:
                em = r["expanse_map"]
                lines.append(f"| **`ExpanseMap`** | **{em.get('lookup_ns', 0.0):.2f} ns** | **{em.get('lookup_mops', 0.0):.2f}** | **{em.get('insert_mops', 0.0):.2f}** | **{em.get('bytes_per_key', 0.0):.2f} B** |")

            for k, v in r.items():
                if k not in ("dist", "pop", "expanse_map", "expanse_set") and isinstance(v, dict):
                    name = k.replace("_", " ").title()
                    b_key = f"{v.get('bytes_per_key', 64.0):.2f} B" if "bytes_per_key" in v else "—"
                    lines.append(f"| `{name}` (baseline) | {v.get('lookup_ns', 0.0):.2f} ns | {v.get('lookup_mops', 0.0):.2f} | {v.get('insert_mops', 0.0):.2f} | {b_key} |")

            lines.append("")

    return "\n".join(lines)


def compare_against_baseline(
    current_results: List[Dict[str, Any]],
    baseline_path: str,
    max_regression_pct: float = 25.0,
    max_memory_regression_pct: float = 10.0,
) -> Tuple[bool, str]:
    """Compares current benchmark results against baseline JSON.

    Returns (has_regression, markdown_comparison_report).
    """
    path = Path(baseline_path)
    if not path.exists():
        return True, f"Error: Baseline file not found at {baseline_path}"

    try:
        baseline_data = json.loads(path.read_text(encoding="utf-8"))
    except Exception as e:
        return True, f"Error reading baseline JSON: {e}"

    base_idx: Dict[Tuple[str, str, int], Dict[str, Any]] = {}
    for item in baseline_data:
        runtime = item.get("runtime", "").lower()
        for r in item.get("results", []):
            dist = r.get("dist", "")
            pop = r.get("pop", 0)
            base_idx[(runtime, dist, pop)] = r

    lines = [
        "## 🌐 Cross-Language Bindings Baseline Comparison",
        "",
        f"> **Baseline**: `{baseline_path}` · **Max Allowed Regression**: {max_regression_pct:.1f}% (throughput/latency), {max_memory_regression_pct:.1f}% (memory)",
        "",
    ]

    regressions: List[str] = []

    for item in current_results:
        runtime = item.get("runtime", "").lower()
        lines.append(f"### Runtime: `{runtime.upper()}`")
        lines.append("")
        lines.append("| Distribution | Metric | Current | Baseline | Delta | Status |")
        lines.append("|:---|:---|---:|---:|---:|:---:|")

        for r in item.get("results", []):
            dist = r.get("dist", "")
            pop = r.get("pop", 0)
            base_entry = base_idx.get((runtime, dist, pop))
            if not base_entry:
                continue

            em_curr = r.get("expanse_map", {})
            em_base = base_entry.get("expanse_map", {})

            # Lookup Throughput (higher is better)
            cur_lm = em_curr.get("lookup_mops", 0.0)
            base_lm = em_base.get("lookup_mops", 0.0)
            if base_lm > 0:
                d_lm = (cur_lm - base_lm) / base_lm * 100.0
                status_lm = "🟢" if d_lm >= 0 else ("🔴" if d_lm < -max_regression_pct else "⚪")
                if d_lm < -max_regression_pct:
                    regressions.append(f"{runtime} {dist} lookup throughput regressed {d_lm:+.1f}% ({cur_lm:.2f} vs {base_lm:.2f} Mops)")
                lines.append(f"| `{dist}` (N={pop:,}) | Lookup (Mops/s) | {cur_lm:.2f} | {base_lm:.2f} | {d_lm:+.1f}% | {status_lm} |")

            # Insert Throughput (higher is better)
            cur_im = em_curr.get("insert_mops", 0.0)
            base_im = em_base.get("insert_mops", 0.0)
            if base_im > 0:
                d_im = (cur_im - base_im) / base_im * 100.0
                status_im = "🟢" if d_im >= 0 else ("🔴" if d_im < -max_regression_pct else "⚪")
                if d_im < -max_regression_pct:
                    regressions.append(f"{runtime} {dist} insert throughput regressed {d_im:+.1f}% ({cur_im:.2f} vs {base_im:.2f} Mops)")
                lines.append(f"| `{dist}` (N={pop:,}) | Insert (Mops/s) | {cur_im:.2f} | {base_im:.2f} | {d_im:+.1f}% | {status_im} |")

            # Memory Density (lower is better)
            cur_bpk = em_curr.get("bytes_per_key", 0.0)
            base_bpk = em_base.get("bytes_per_key", 0.0)
            if base_bpk > 0:
                d_bpk = (cur_bpk - base_bpk) / base_bpk * 100.0
                status_bpk = "🟢" if d_bpk <= 0 else ("🔴" if d_bpk > max_memory_regression_pct else "⚪")
                if d_bpk > max_memory_regression_pct:
                    regressions.append(f"{runtime} {dist} memory density regressed {d_bpk:+.1f}% ({cur_bpk:.2f} vs {base_bpk:.2f} B/key)")
                lines.append(f"| `{dist}` (N={pop:,}) | Memory (B/key) | {cur_bpk:.2f} | {base_bpk:.2f} | {d_bpk:+.1f}% | {status_bpk} |")

        lines.append("")

    has_regressions = len(regressions) > 0
    if has_regressions:
        lines.append(f"> ⚠️ **Regressions Detected ({len(regressions)})**:\n" + "\n".join(f"> - {reg}" for reg in regressions))
    else:
        lines.append("> 🟢 **All binding metrics within baseline tolerance thresholds.**")

    return has_regressions, "\n".join(lines)


def main():
    args = parse_args()
    runners = {
        "node": run_node_benchmark,
        "wasm": run_wasm_benchmark,
        "python": run_python_benchmark,
        "php": run_php_benchmark,
        "ruby": run_ruby_benchmark,
    }

    selected = args.runtimes or list(runners.keys())
    all_results = []

    for name in selected:
        runner = runners.get(name)
        if runner:
            res = runner(args.quick)
            if res:
                all_results.append(res)

    if args.json:
        print(json.dumps(all_results, indent=2))
        return

    md_report = format_markdown_report(all_results)
    print(md_report)

    comp_report = ""
    has_reg = False
    if args.check_baseline:
        has_reg, comp_report = compare_against_baseline(
            all_results,
            args.check_baseline,
            max_regression_pct=args.max_regression_pct,
            max_memory_regression_pct=args.max_memory_regression_pct,
        )
        print("\n" + comp_report)

    if args.output:
        full_report = md_report
        if comp_report:
            full_report += "\n\n---\n\n" + comp_report
        Path(args.output).write_text(full_report, encoding="utf-8")
        print(f"\nSaved report to {args.output}")

    if args.save_baseline:
        Path(args.save_baseline).parent.mkdir(parents=True, exist_ok=True)
        Path(args.save_baseline).write_text(json.dumps(all_results, indent=2), encoding="utf-8")
        print(f"\nSaved baseline to {args.save_baseline}")

    if has_reg:
        sys.exit(1)


if __name__ == "__main__":
    main()
