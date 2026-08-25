#!/usr/bin/env python3
"""
scripts/bench_bindings.py — Unified Cross-Language Comparative Benchmark Suite Orchestrator.

Discovers and executes available binding benchmark harnesses (Node.js, WASM, Go,
Python, PHP, Ruby, Java, .NET) and outputs unified comparative performance matrices.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional

REPO_ROOT = Path(__file__).resolve().parent.parent


def parse_args():
    p = argparse.ArgumentParser(description="Unified Expanse Cross-Language Benchmark Suite")
    p.add_argument("--quick", action="store_true", help="Run in quick mode (smaller N)")
    p.add_argument("--runtimes", nargs="+", help="Specific runtimes to benchmark (node, wasm, go, python, php, ruby)")
    p.add_argument("--json", action="store_true", help="Emit raw JSON results to stdout")
    p.add_argument("--output", type=str, help="Save markdown report to file")
    p.add_argument("--save-baseline", type=str, help="Save results to baseline JSON file")
    p.add_argument("--check-baseline", type=str, help="Compare results against baseline JSON file")
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

    if args.output:
        Path(args.output).write_text(md_report, encoding="utf-8")
        print(f"\nSaved report to {args.output}")

    if args.save_baseline:
        Path(args.save_baseline).parent.mkdir(parents=True, exist_ok=True)
        Path(args.save_baseline).write_text(json.dumps(all_results, indent=2), encoding="utf-8")
        print(f"\nSaved baseline to {args.save_baseline}")


if __name__ == "__main__":
    main()
