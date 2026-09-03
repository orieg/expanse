#!/usr/bin/env python3
"""Measure what core placement costs a wall-clock benchmark on a hybrid host (#639).

The reference host is a hybrid part: performance cores at ~5.1 GHz and
efficiency cores at ~3.8 GHz. `scripts/perf_counters.py` already refuses to
collect unpinned counters on such a host and prefixes its workload with
`taskset`. Nothing else does, so every wall-clock arm runs wherever the
scheduler puts it, and AGENTS.md §8.4 gates those arms on a BCa 95% interval
whose width is assumed to reflect measurement noise rather than core-class
assignment.

This script answers the question #639 pre-registers, before anything is
changed: **run the same criterion arms pinned and unpinned, interleaved, and
compare the BCa intervals.** If they overlap on every arm, the exposure is
below what the instrument can see and #639 closes as a documentation note. If
they do not, the pin is warranted and this output is the evidence for it.

    python3 scripts/pin_exposure.py --bench compare -p expanse-trie --rounds 6
    python3 scripts/pin_exposure.py --bench compare -p expanse-trie --json pin.json
    python3 scripts/pin_exposure.py --self-test

Rounds alternate pinned/unpinned and flip which condition goes first on odd
rounds, so a monotonic drift over the session (thermal, background load)
cancels between the two conditions instead of loading onto one.

Preflight is fail-loud (§8.1): a non-hybrid host, a missing `taskset`, or a
kernel that publishes no P-core CPU list all refuse to run rather than
silently measuring something else. There is no unpinned-only fallback — a
comparison needs both halves.
"""
from __future__ import annotations

import argparse
import json
import platform
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Sequence, Tuple

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))
from bca_bootstrap import bca_bootstrap_ci  # noqa: E402

SYS_PMU_ROOT = Path("/sys/devices")
P_CORE_PMU = "cpu_core"
E_CORE_PMU = "cpu_atom"
CRITERION_ROOT = REPO_ROOT / "target" / "criterion"
DEFAULT_ROUNDS = 6
RESAMPLES = 2000


class Preflight(Exception):
    """A precondition that makes the measurement meaningless if ignored."""


def fail(msg: str) -> None:
    print(f"pin_exposure.py: {msg}", file=sys.stderr)
    sys.exit(1)


def pmu_cpus(pmu: str) -> Optional[str]:
    """The kernel's CPU list for `pmu`, e.g. `0-15`. `None` if absent."""
    path = SYS_PMU_ROOT / pmu / "cpus"
    try:
        return path.read_text(encoding="utf-8").strip() or None
    except OSError:
        return None


def preflight() -> Tuple[str, str]:
    """Returns (p_core_cpus, e_core_cpus) or raises."""
    if platform.system() != "Linux":
        raise Preflight(
            f"this host is {platform.system()}, and core pinning here is a Linux "
            "`taskset` question about the bare-metal reference host. Run it there."
        )
    if shutil.which("taskset") is None:
        raise Preflight("`taskset` is not on PATH (install `util-linux`); the pinned half cannot run.")
    p_cpus, e_cpus = pmu_cpus(P_CORE_PMU), pmu_cpus(E_CORE_PMU)
    if p_cpus is None:
        raise Preflight(
            f"the kernel publishes no CPU list at {SYS_PMU_ROOT / P_CORE_PMU / 'cpus'}, so this "
            "host exposes no performance-core PMU. Either it is not hybrid — in which case #639 "
            "does not apply to it and the measurement would compare a pin against itself — or the "
            "PMU is named differently and this script needs teaching."
        )
    if e_cpus is None:
        raise Preflight(
            f"a `{P_CORE_PMU}` PMU exists but no `{E_CORE_PMU}`, so the scheduler has no second "
            "core class to migrate onto and pinning cannot change the result. Nothing to measure."
        )
    return p_cpus, e_cpus


def run_bench(bench: str, package: str, pin: Optional[str], filter_: Optional[str]) -> None:
    argv: List[str] = []
    if pin:
        argv += ["taskset", "-c", pin]
    argv += ["cargo", "bench", "--bench", bench, "-p", package]
    if filter_:
        argv += ["--", filter_]
    proc = subprocess.run(argv, cwd=REPO_ROOT, capture_output=True, text=True)
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr[-4000:])
        fail(f"`{' '.join(argv)}` exited {proc.returncode}")


def collect_samples() -> Dict[str, List[float]]:
    """Per-iteration times from every criterion `new/sample.json` under target/criterion.

    Criterion overwrites `new/` on each run, so this is called immediately
    after each run and the values are accumulated by the caller.
    """
    out: Dict[str, List[float]] = {}
    if not CRITERION_ROOT.exists():
        fail(f"{CRITERION_ROOT} does not exist — did the bench run?")
    for sample_path in sorted(CRITERION_ROOT.glob("**/new/sample.json")):
        arm = str(sample_path.parent.parent.relative_to(CRITERION_ROOT))
        try:
            data = json.loads(sample_path.read_text(encoding="utf-8"))
            times, iters = data["times"], data["iters"]
        except (OSError, KeyError, json.JSONDecodeError) as e:
            fail(f"cannot read {sample_path}: {e}")
        if len(times) != len(iters) or not times:
            fail(f"{sample_path}: {len(times)} times for {len(iters)} iters")
        out[arm] = [t / i for t, i in zip(times, iters) if i]
    if not out:
        fail("no criterion sample.json found — the bench produced no samples")
    return out


def overlap(a: Tuple[float, float], b: Tuple[float, float]) -> bool:
    return a[0] <= b[1] and b[0] <= a[1]


def analyse(pinned: Dict[str, List[float]], unpinned: Dict[str, List[float]]) -> List[Dict[str, Any]]:
    rows: List[Dict[str, Any]] = []
    for arm in sorted(set(pinned) & set(unpinned)):
        p, u = pinned[arm], unpinned[arm]
        p_mean, p_lo, p_hi = bca_bootstrap_ci(p, num_resamples=RESAMPLES)
        u_mean, u_lo, u_hi = bca_bootstrap_ci(u, num_resamples=RESAMPLES)
        p_width = (p_hi - p_lo) / p_mean * 100 if p_mean else float("nan")
        u_width = (u_hi - u_lo) / u_mean * 100 if u_mean else float("nan")
        rows.append(
            {
                "arm": arm,
                "pinned_ns": p_mean,
                "unpinned_ns": u_mean,
                "pinned_ci": [p_lo, p_hi],
                "unpinned_ci": [u_lo, u_hi],
                "pinned_ci_width_pct": p_width,
                "unpinned_ci_width_pct": u_width,
                "ratio_unpinned_over_pinned": u_mean / p_mean if p_mean else float("nan"),
                "intervals_overlap": overlap((p_lo, p_hi), (u_lo, u_hi)),
                "n_pinned": len(p),
                "n_unpinned": len(u),
            }
        )
    return rows


def verdict(rows: Sequence[Dict[str, Any]]) -> Tuple[str, str]:
    """(label, sentence) — the reading #639 pre-registered."""
    if not rows:
        return "NO_DATA", "no arm produced samples under both conditions; nothing was measured."
    separated = [r for r in rows if not r["intervals_overlap"]]
    widened = [r for r in rows if r["unpinned_ci_width_pct"] > 2 * r["pinned_ci_width_pct"]]
    if not separated:
        s = (
            f"every one of the {len(rows)} arms has overlapping pinned and unpinned BCa 95% intervals, "
            "so core placement moved no arm further than this instrument can resolve"
        )
        if widened:
            s += (
                f"; {len(widened)} arm(s) nonetheless show an unpinned interval more than twice as wide, "
                "which is the variance the pin would remove even though the point estimates agree"
            )
        return "OVERLAP", s + "."
    worst = max(separated, key=lambda r: abs(r["ratio_unpinned_over_pinned"] - 1))
    return (
        "SEPARATED",
        f"{len(separated)} of {len(rows)} arms have non-overlapping pinned and unpinned intervals; "
        f"the widest is `{worst['arm']}` at {worst['ratio_unpinned_over_pinned']:.3f}× unpinned over pinned. "
        "Core placement is visible to the instrument, so the wall-clock arms warrant the pin "
        "perf_counters.py already applies.",
    )


def render(rows: Sequence[Dict[str, Any]], meta: Dict[str, Any]) -> str:
    label, sentence = verdict(rows)
    out = [
        f"### Core-placement exposure on {meta['host']} — `{meta['bench']}`, {meta['rounds']} interleaved rounds",
        "",
        f"P-cores `{meta['p_cpus']}` (pinned condition), E-cores `{meta['e_cpus']}`; "
        f"unpinned runs with no affinity mask. Samples are criterion per-iteration times, "
        f"pooled across rounds; intervals are BCa 95% over {RESAMPLES} resamples.",
        "",
        "| arm | pinned ns | unpinned ns | unpinned ÷ pinned | pinned CI width | unpinned CI width | intervals |",
        "|---|---:|---:|---:|---:|---:|---|",
    ]
    for r in rows:
        out.append(
            f"| `{r['arm']}` | {r['pinned_ns']:,.2f} | {r['unpinned_ns']:,.2f} | "
            f"{r['ratio_unpinned_over_pinned']:.3f}× | {r['pinned_ci_width_pct']:.2f}% | "
            f"{r['unpinned_ci_width_pct']:.2f}% | {'overlap' if r['intervals_overlap'] else '**separated**'} |"
        )
    out += ["", f"**Verdict: {label}.** {sentence}"]
    return "\n".join(out) + "\n"


def self_test() -> None:
    """Fail-then-pass pins of the reading, on synthetic samples."""
    import random

    rng = random.Random(0xB0A7)
    # 1. identical distributions -> overlap, verdict OVERLAP
    a = [100 + rng.gauss(0, 1) for _ in range(400)]
    b = [100 + rng.gauss(0, 1) for _ in range(400)]
    rows = analyse({"x": a}, {"x": b})
    assert rows[0]["intervals_overlap"], "same distribution must overlap"
    assert verdict(rows)[0] == "OVERLAP"
    # 2. unpinned shifted by the P/E clock ratio -> separated, verdict SEPARATED
    slow = [x / 0.74 for x in b]
    rows = analyse({"x": a}, {"x": slow})
    assert not rows[0]["intervals_overlap"], "a 26% shift must separate"
    label, sentence = verdict(rows)
    assert label == "SEPARATED" and "1.3" in f"{rows[0]['ratio_unpinned_over_pinned']:.3f}"
    assert "`x`" in sentence
    # 3. same mean, far wider unpinned spread -> overlap, but the width note fires
    wide = [100 + rng.gauss(0, 12) for _ in range(400)]
    rows = analyse({"x": a}, {"x": wide})
    assert rows[0]["intervals_overlap"], "same mean must still overlap"
    label, sentence = verdict(rows)
    assert label == "OVERLAP" and "twice as wide" in sentence, sentence
    # 4. an arm present in only one condition is dropped, not half-reported
    rows = analyse({"x": a, "y": a}, {"x": b})
    assert [r["arm"] for r in rows] == ["x"]
    # 5. no shared arm -> NO_DATA, never a pass
    assert verdict(analyse({"x": a}, {"y": b}))[0] == "NO_DATA"
    # 6. a non-hybrid host refuses rather than comparing a pin against itself
    global pmu_cpus
    real = pmu_cpus
    try:
        pmu_cpus = lambda pmu: "0-15" if pmu == P_CORE_PMU else None  # noqa: E731
        if platform.system() == "Linux" and shutil.which("taskset"):
            try:
                preflight()
                raise AssertionError("a host with no E-core PMU must refuse")
            except Preflight as e:
                assert "no second core class" in str(e) or "nothing to measure" in str(e).lower()
    finally:
        pmu_cpus = real
    print("pin_exposure.py --self-test: all checks passed")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bench", help="criterion bench target, e.g. `compare`")
    ap.add_argument("-p", "--package", default="expanse-trie")
    ap.add_argument("--filter", help="criterion filter passed after `--`")
    ap.add_argument("--rounds", type=int, default=DEFAULT_ROUNDS, help=f"interleaved rounds per condition (default {DEFAULT_ROUNDS})")
    ap.add_argument("--json", type=Path)
    ap.add_argument("--markdown", type=Path)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        self_test()
        return
    if not args.bench:
        ap.error("--bench is required")
    try:
        p_cpus, e_cpus = preflight()
    except Preflight as e:
        fail(str(e))
        return

    pinned: Dict[str, List[float]] = {}
    unpinned: Dict[str, List[float]] = {}
    for rnd in range(args.rounds):
        # Flip the order on odd rounds so a monotonic drift over the session
        # does not load onto whichever condition always runs first.
        order = [("pinned", p_cpus), ("unpinned", None)]
        if rnd % 2:
            order.reverse()
        for name, pin in order:
            print(f"round {rnd + 1}/{args.rounds}: {name}", file=sys.stderr)
            run_bench(args.bench, args.package, pin, args.filter)
            target = pinned if name == "pinned" else unpinned
            for arm, samples in collect_samples().items():
                target.setdefault(arm, []).extend(samples)

    rows = analyse(pinned, unpinned)
    meta = {
        "host": platform.node() and f"{platform.machine()} Linux hybrid host",
        "bench": args.bench,
        "rounds": args.rounds,
        "p_cpus": p_cpus,
        "e_cpus": e_cpus,
    }
    md = render(rows, meta)
    print(md)
    if args.markdown:
        args.markdown.write_text(md, encoding="utf-8")
    if args.json:
        label, sentence = verdict(rows)
        args.json.write_text(
            json.dumps({"meta": meta, "verdict": label, "reading": sentence, "arms": rows}, indent=1) + "\n",
            encoding="utf-8",
        )


if __name__ == "__main__":
    main()
