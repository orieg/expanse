#!/usr/bin/env python3
"""Fuel-metered WebAssembly benchmark driver: the Callgrind analogue for the
wasm targets (#629).

Runs every export of `crates/expanse-wasm-fuel` under wasmtime with fuel
metering and reports the exact fuel each arm consumed. Fuel is charged per
executed instruction and is deterministic for a given module and runtime
version, so an arm's cost is an integer with no variance. Each export is run
twice on fresh instances and the two readings must agree to the unit, or the
script refuses to produce a result (§8.1: no silent degradation).

    python3 scripts/wasm_fuel.py --build wasm32                # build + measure
    python3 scripts/wasm_fuel.py --build wasm64                # nightly, -Z build-std
    python3 scripts/wasm_fuel.py --module target/.../x.wasm --target wasm32
    python3 scripts/wasm_fuel.py --build wasm32 --check-baseline results/baseline_wasm_fuel.json
    python3 scripts/wasm_fuel.py --build wasm32 --save-baseline results/baseline_wasm_fuel.json
    python3 scripts/wasm_fuel.py --self-test

Measured region: every export takes a phase; phase 0 is setup only, phase 1
is setup plus the arm's loop; the arm is the difference. Gate policy mirrors
`perf_report.py`: the check fails on a single-worst regression above
`--max-regression` (5%) or on two or more arms above the 0.5% noise floor —
fuel has no noise, the floor exists so a one-unit codegen ripple on an
unrelated arm is reported but does not fail a PR on its own.

Requires the `wasmtime` Python package (`pip install wasmtime`); the script
exits non-zero with that hint if it is missing. Never falls back to a mock.
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

REPO_ROOT = Path(__file__).resolve().parent.parent
CRATE = "expanse-wasm-fuel"
MODULE_NAME = "expanse_wasm_fuel.wasm"
TARGETS = {
    "wasm32": "wasm32-unknown-unknown",
    "wasm64": "wasm64-unknown-unknown",
}
DISTS = {"sequential": 0, "clustered": 1, "random": 2}
ARMS = [
    "map_insert",
    "map_get",
    "map_iterate",
    "map_range",
    "map_remove",
    "set_insert",
    "set_contains",
    "set_iterate",
    "set_range",
    "set_remove",
]
MEM_EXPORTS = ["map_mem_used", "set_mem_used"]
DEFAULT_POP = 10_000
NOISE_FLOOR_PCT = 0.5
MAX_REGRESSION_PCT = 5.0
FUEL_BUDGET = 1 << 62


def fail(msg: str) -> None:
    print(f"wasm_fuel.py: error: {msg}", file=sys.stderr)
    sys.exit(1)


def load_wasmtime():
    try:
        import wasmtime  # type: ignore
    except ImportError:
        fail("the `wasmtime` Python package is not installed — `pip install wasmtime` (no fallback is provided)")
    return wasmtime


def wasmtime_version() -> str:
    try:
        import importlib.metadata as md

        return md.version("wasmtime")
    except Exception:
        return "unknown"


def rustc_version(nightly: bool) -> str:
    cmd = ["rustc", "+nightly", "--version"] if nightly else ["rustc", "--version"]
    try:
        return subprocess.run(cmd, capture_output=True, text=True, check=True).stdout.strip()
    except Exception as e:  # pragma: no cover - environment
        fail(f"cannot run {' '.join(cmd)}: {e}")
    return ""


def git_commit() -> str:
    try:
        return subprocess.run(
            ["git", "rev-parse", "--short=8", "HEAD"], cwd=REPO_ROOT, capture_output=True, text=True, check=True
        ).stdout.strip()
    except Exception:
        return "unknown"


def build_module(target: str) -> Path:
    """Builds the fuel module for `wasm32` or `wasm64` and returns its path."""
    triple = TARGETS[target]
    if target == "wasm64":
        cmd = [
            "cargo", "+nightly", "build", "--release", "-p", CRATE,
            "--target", triple, "-Z", "build-std=std,panic_abort",
        ]
    else:
        cmd = ["cargo", "build", "--release", "-p", CRATE, "--target", triple]
    print(f"wasm_fuel.py: {' '.join(cmd)}", file=sys.stderr)
    proc = subprocess.run(cmd, cwd=REPO_ROOT, capture_output=True, text=True)
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr)
        fail(f"build failed for {triple} (exit {proc.returncode})")
    path = REPO_ROOT / "target" / triple / "release" / MODULE_NAME
    if not path.exists():
        fail(f"build succeeded but {path} is missing")
    return path


class Runner:
    """Instantiates the module fresh for every call so allocator state from one
    call cannot leak into the next reading."""

    def __init__(self, module_path: Path, target: str):
        self.wt = load_wasmtime()
        cfg = self.wt.Config()
        cfg.consume_fuel = True
        if target == "wasm64":
            cfg.wasm_memory64 = True
        self.engine = self.wt.Engine(cfg)
        try:
            self.module = self.wt.Module.from_file(self.engine, str(module_path))
        except Exception as e:
            fail(f"cannot load {module_path}: {e}")
        exports = {e.name for e in self.module.exports}
        missing = [n for n in ARMS + MEM_EXPORTS if n not in exports]
        if missing:
            fail(f"module lacks exports {missing}; is it built from crates/{CRATE}?")

    def call(self, export: str, *args: int) -> Tuple[int, int]:
        """Returns (result, fuel_consumed) for one call on a fresh instance."""
        store = self.wt.Store(self.engine)
        store.set_fuel(FUEL_BUDGET)
        inst = self.wt.Instance(store, self.module, [])
        fn = inst.exports(store)[export]
        result = fn(store, *args)
        return int(result), FUEL_BUDGET - store.get_fuel()

    def measure(self, export: str, *args: int) -> Tuple[int, int]:
        """Two fresh runs; they must agree exactly."""
        r1, f1 = self.call(export, *args)
        r2, f2 = self.call(export, *args)
        if (r1, f1) != (r2, f2):
            fail(f"{export}{args}: two runs disagree ({f1} vs {f2} fuel, {r1} vs {r2} result); fuel is not deterministic here — refusing to report")
        return r1, f1


def run_suite(module_path: Path, target: str, pop: int, dists: List[str]) -> Dict[str, Any]:
    runner = Runner(module_path, target)
    arms: List[Dict[str, Any]] = []
    for arm in ARMS:
        for dist in dists:
            d = DISTS[dist]
            r0, f0 = runner.measure(arm, pop, d, 0)
            r1, f1 = runner.measure(arm, pop, d, 1)
            if f1 <= f0:
                fail(f"{arm}/{dist}: phase 1 ({f1}) consumed no more fuel than phase 0 ({f0}); the arm's loop was elided")
            if r0 == r1:
                fail(f"{arm}/{dist}: phase 1 returned the same checksum as phase 0; the arm's loop did no observable work")
            arms.append(
                {
                    "name": f"{arm}/{dist}",
                    "fuel": f1 - f0,
                    "fuel_setup": f0,
                    "fuel_total": f1,
                    "per_op": (f1 - f0) / pop,
                    "checksum": f"0x{r1:016x}",
                }
            )
    mem: Dict[str, int] = {}
    for exp in MEM_EXPORTS:
        for dist in dists:
            r, _ = runner.measure(exp, pop, DISTS[dist])
            mem[f"{exp.split('_')[0]}/{dist}"] = r
    return {
        "instrument": "wasmtime-fuel",
        "target": TARGETS[target],
        "wasmtime": wasmtime_version(),
        "rustc": rustc_version(target == "wasm64"),
        "host": f"{platform.system()} {platform.machine()}",
        "commit": git_commit(),
        "pop": pop,
        "arms": arms,
        "mem_used_bytes": mem,
        "mem_used_bytes_per_key": {k: v / pop for k, v in mem.items()},
    }


def index_arms(result: Dict[str, Any]) -> Dict[str, int]:
    return {a["name"]: int(a["fuel"]) for a in result.get("arms", [])}


def compare(current: Dict[str, Any], baseline: Dict[str, Any], max_regression_pct: float) -> Tuple[bool, List[str], List[Tuple[str, int, int, float]]]:
    """Returns (failed, messages, rows[name, base, cur, delta_pct])."""
    cur, base = index_arms(current), index_arms(baseline)
    rows: List[Tuple[str, int, int, float]] = []
    regressed: List[Tuple[str, float]] = []
    for name, c in cur.items():
        if name not in base:
            rows.append((name, 0, c, float("nan")))
            continue
        b = base[name]
        delta = (c / b - 1) * 100 if b else float("nan")
        rows.append((name, b, c, delta))
        if delta > NOISE_FLOOR_PCT:
            regressed.append((name, delta))
    msgs: List[str] = []
    missing = sorted(set(base) - set(cur))
    if missing:
        msgs.append(f"{len(missing)} baseline arm(s) absent from this run: {missing} — coverage loss, not a pass")
    worst = max((d for _, d in regressed), default=0.0)
    failed = bool(missing) or worst > max_regression_pct or len(regressed) >= 2
    if regressed:
        msgs.append(
            f"{len(regressed)} arm(s) regressed > {NOISE_FLOOR_PCT}% (worst: {worst:+.2f}%, single-worst threshold {max_regression_pct}%, two-arm rule):"
        )
        msgs += [f"  - {n}: {d:+.2f}%" for n, d in sorted(regressed, key=lambda x: -x[1])]
    if base.get("__target__") is None and baseline.get("target") != current.get("target"):
        msgs.append(f"baseline target {baseline.get('target')} differs from this run's {current.get('target')}")
        failed = True
    return failed, msgs, rows


def markdown(result: Dict[str, Any], rows: Optional[List[Tuple[str, int, int, float]]] = None) -> str:
    out = [
        f"### wasm fuel — `{result['target']}` (wasmtime {result['wasmtime']}, {result['rustc']}, commit `{result['commit']}`, N={result['pop']:,})",
        "",
    ]
    if rows:
        out += ["| arm | baseline fuel | fuel | vs baseline | fuel / op |", "|---|---:|---:|---:|---:|"]
        per = {a["name"]: a["per_op"] for a in result["arms"]}
        for name, b, c, d in rows:
            tag = "🆕" if b == 0 else ("🔴" if d > NOISE_FLOOR_PCT else ("🟢" if d < -NOISE_FLOOR_PCT else "="))
            dv = "—" if b == 0 else f"{d:+.2f}%"
            out.append(f"| {tag} `{name}` | {b:,} | {c:,} | {dv} | {per[name]:,.1f} |")
    else:
        out += ["| arm | fuel | fuel / op |", "|---|---:|---:|"]
        for a in result["arms"]:
            out.append(f"| `{a['name']}` | {a['fuel']:,} | {a['per_op']:,.1f} |")
    out.append("")
    out += ["| structure / dist | `mem_used` B/key |", "|---|---:|"]
    for k, v in result["mem_used_bytes_per_key"].items():
        out.append(f"| `{k}` | {v:.3f} |")
    return "\n".join(out) + "\n"


def load_baseline(path: Path, target: str) -> Dict[str, Any]:
    if not path.exists():
        fail(f"baseline {path} not found")
    data = json.loads(path.read_text(encoding="utf-8"))
    entries = data if isinstance(data, list) else [data]
    for e in entries:
        if e.get("target") == TARGETS[target]:
            return e
    fail(f"baseline {path} has no entry for {TARGETS[target]}")
    return {}


def save_baseline(path: Path, result: Dict[str, Any]) -> None:
    entries: List[Dict[str, Any]] = []
    if path.exists():
        data = json.loads(path.read_text(encoding="utf-8"))
        entries = data if isinstance(data, list) else [data]
    entries = [e for e in entries if e.get("target") != result["target"]] + [result]
    entries.sort(key=lambda e: e["target"])
    path.write_text(json.dumps(entries, indent=1) + "\n", encoding="utf-8")


def self_test() -> None:
    """Fail-then-pass pins of the gate policy on synthetic data."""
    base = {"target": "wasm32-unknown-unknown", "arms": [{"name": n, "fuel": 1000} for n in ("a/x", "b/x", "c/x")]}

    def cur(vals: Dict[str, int]) -> Dict[str, Any]:
        return {"target": "wasm32-unknown-unknown", "arms": [{"name": n, "fuel": v} for n, v in vals.items()]}

    # exact match passes
    f, _, _ = compare(cur({"a/x": 1000, "b/x": 1000, "c/x": 1000}), base, MAX_REGRESSION_PCT)
    assert not f, "identical run must pass"
    # one arm +0.4% is under the floor: pass
    f, _, _ = compare(cur({"a/x": 1004, "b/x": 1000, "c/x": 1000}), base, MAX_REGRESSION_PCT)
    assert not f, "one arm under the noise floor must pass"
    # one arm +1%: reported, single arm under 5%: pass
    f, msgs, _ = compare(cur({"a/x": 1010, "b/x": 1000, "c/x": 1000}), base, MAX_REGRESSION_PCT)
    assert not f and any("1 arm(s) regressed" in m for m in msgs), "a single sub-threshold regression is reported, not fatal"
    # two arms +1%: fail (two-arm rule)
    f, _, _ = compare(cur({"a/x": 1010, "b/x": 1010, "c/x": 1000}), base, MAX_REGRESSION_PCT)
    assert f, "two arms over the floor must fail"
    # one arm +6%: fail (single-worst)
    f, _, _ = compare(cur({"a/x": 1060, "b/x": 1000, "c/x": 1000}), base, MAX_REGRESSION_PCT)
    assert f, "single-worst over threshold must fail"
    # coverage loss: fail even with improvements
    f, msgs, _ = compare(cur({"a/x": 900, "b/x": 900}), base, MAX_REGRESSION_PCT)
    assert f and any("absent" in m for m in msgs), "a missing arm is coverage loss, not a pass"
    # target mismatch: fail
    f, _, _ = compare({"target": "wasm64-unknown-unknown", "arms": base["arms"]}, base, MAX_REGRESSION_PCT)
    assert f, "comparing across targets must fail"
    # improvements pass and render green
    r = {"target": "wasm32-unknown-unknown", "wasmtime": "x", "rustc": "y", "commit": "z", "pop": 10, "arms": [{"name": "a/x", "fuel": 900, "per_op": 90.0}], "mem_used_bytes_per_key": {"map/x": 1.0}}
    _, _, rows = compare(r, base, MAX_REGRESSION_PCT)
    assert "🟢" in markdown(r, rows)
    print("wasm_fuel.py --self-test: all checks passed")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--build", choices=list(TARGETS), help="build the module for this target, then measure it")
    ap.add_argument("--module", type=Path, help="path to a prebuilt module (requires --target)")
    ap.add_argument("--target", choices=list(TARGETS), help="target of --module")
    ap.add_argument("--pop", type=int, default=DEFAULT_POP)
    ap.add_argument("--quick", action="store_true", help="random distribution only")
    ap.add_argument("--json", type=Path, help="write the full result here")
    ap.add_argument("--markdown", type=Path, help="write a markdown table here")
    ap.add_argument("--save-baseline", type=Path)
    ap.add_argument("--check-baseline", type=Path)
    ap.add_argument("--max-regression", type=float, default=MAX_REGRESSION_PCT)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        self_test()
        return
    if args.build:
        target = args.build
        module = build_module(target)
    elif args.module and args.target:
        target, module = args.target, args.module
    else:
        ap.error("give --build <target>, or --module PATH with --target")
    dists = ["random"] if args.quick else list(DISTS)
    result = run_suite(module, target, args.pop, dists)

    rows = None
    failed = False
    if args.check_baseline:
        baseline = load_baseline(args.check_baseline, target)
        failed, msgs, rows = compare(result, baseline, args.max_regression)
        for m in msgs:
            print(("::error::" if failed else "::notice::") + m if os.environ.get("GITHUB_ACTIONS") else m)
    md = markdown(result, rows)
    print(md)
    if args.json:
        args.json.write_text(json.dumps(result, indent=1) + "\n", encoding="utf-8")
    if args.markdown:
        args.markdown.write_text(md, encoding="utf-8")
    if args.save_baseline:
        save_baseline(args.save_baseline, result)
        print(f"saved baseline for {result['target']} to {args.save_baseline}")
    if failed:
        print("wasm_fuel.py: regression gate FAILED", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
