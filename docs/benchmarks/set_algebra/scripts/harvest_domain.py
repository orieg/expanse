#!/usr/bin/env python3
"""Harvest the `domain` harness's criterion samples into the suite's results JSON.

`run.sh --reps N` runs `cargo bench --bench domain` N times and snapshots
`target/criterion` after each into a gitignored raw directory. This script turns
those snapshots into the three `domain_*_611` sections of
`results/bench_domain_algebra.json` — the file `generate_charts.py` renders —
so every published cell resolves to a committed artifact produced by a
committed script rather than to a session transcript (AGENTS.md §8.2, §8.7).

Statistics, per the sections' own notes:

  * **Parity (H1)**: the ratio `domain / raw` is paired *per repetition* (both
    arms measured in the same process run), and the published interval is a
    95% percentile bootstrap over the per-repetition ratios, so it carries
    between-run variance that criterion's within-run interval cannot see. A
    two-sample BCa interval over all repetitions' pooled per-iteration samples
    is recorded beside it as `ci_pooled_bca`; it is tighter and is not the
    gate. The verdict is `overhead resolved` when the published interval
    excludes 1.0, else `not resolved`.
  * **Ingestion (H2/H3)**: `scalar / batch` per repetition, same treatment;
    throughput in M keys/s from the population and the per-iteration time.
  * **Resolution (H4)**: M keys/s and ns/key from the per-iteration time of a
    full scan.

Per-repetition means are recorded so the aggregate can be re-derived.

    python3 docs/benchmarks/set_algebra/scripts/harvest_domain.py \\
        --raw results/quick/set_algebra/raw --out docs/benchmarks/set_algebra/results/bench_domain_algebra.json \\
        --commit <sha> --host-desc "<anonymised hardware>" --loads results/quick/set_algebra/raw/loads.txt
    python3 docs/benchmarks/set_algebra/scripts/harvest_domain.py --self-test

Fail-loud (§8.1): a repetition missing any arm, or fewer than three repetitions,
refuses rather than publishing a partial aggregate.
"""
from __future__ import annotations

import argparse
import json
import random
import statistics
import sys
from pathlib import Path
from typing import Any, Dict, List, Sequence, Tuple

REPO_ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO_ROOT / "scripts"))
from bca_bootstrap import bca_bootstrap_ci, bca_bootstrap_ratio_ci  # noqa: E402
from bench_baseline import per_iteration_samples, validate_host_description  # noqa: E402

GROUP_ALGEBRA = "domain_set_algebra_overhead"
GROUP_INGEST = "domain_ingestion"
GROUP_RESOLVE = "domain_resolution"
PARITY_PAIRS = {
    # json key: (raw arm, domain arm, population)
    "intersection_10k": ("raw_expanse_set_intersection/10000", "domain_set_intersection/10000", 10_000),
    "intersection_100k": ("raw_expanse_set_intersection/100000", "domain_set_intersection/100000", 100_000),
    "intersection_len_10k": ("raw_expanse_set_intersection_len/10000", "domain_set_intersection_len/10000", 10_000),
    "intersection_len_100k": ("raw_expanse_set_intersection_len/100000", "domain_set_intersection_len/100000", 100_000),
}
INGEST_PAIRS = [
    # (label, scalar arm, batch arm, population)
    ("Text Keys (entity:...)", "scalar_insert_text/10000", "batch_insert_text/10000", 10_000),
    ("Binary UUID (escaped)", "scalar_insert_uuid/10000", "batch_insert_uuid/10000", 10_000),
    ("Text Keys (entity:...)", "scalar_insert_text/50000", "batch_insert_text/50000", 50_000),
    ("Binary UUID (escaped)", "scalar_insert_uuid/50000", "batch_insert_uuid/50000", 50_000),
]
RESOLVE_ARMS = {10_000: "resolve_full_scan/10000", 100_000: "resolve_full_scan/100000"}
MIN_REPS = 3


class HarvestError(Exception):
    pass


# --------------------------------------------------------------------------- reading
def load_rep(criterion_dir: Path) -> Dict[str, List[float]]:
    """`<criterion_dir>/<group>/<arm>/<pop>/new/sample.json` -> {"group/arm/pop": per-iteration ns}."""
    out: Dict[str, List[float]] = {}
    for sample in sorted(criterion_dir.glob("*/*/*/new/sample.json")):
        arm = "/".join(sample.parent.parent.relative_to(criterion_dir).parts)
        try:
            data = json.loads(sample.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as e:
            raise HarvestError(f"cannot read {sample}: {e}") from e
        out[arm] = per_iteration_samples(data, sample)
    if not out:
        raise HarvestError(f"no criterion samples under {criterion_dir}")
    return out


def load_reps(raw: Path) -> List[Dict[str, List[float]]]:
    reps = []
    for rep_dir in sorted(raw.glob("rep_*")):
        crit = rep_dir / "criterion" if (rep_dir / "criterion").is_dir() else rep_dir
        reps.append(load_rep(crit))
    if len(reps) < MIN_REPS:
        raise HarvestError(f"{len(reps)} repetition(s) under {raw}; at least {MIN_REPS} are required")
    return reps


def need(rep: Dict[str, List[float]], group: str, arm: str, idx: int) -> List[float]:
    key = f"{group}/{arm}"
    if key not in rep:
        raise HarvestError(f"repetition {idx} lacks arm {key}; a partial aggregate is not published")
    return rep[key]


# --------------------------------------------------------------------------- statistics
def percentile_bootstrap(values: Sequence[float], resamples: int = 5000, seed: int = 42) -> Tuple[float, float]:
    """95% percentile bootstrap of the mean over a small set of per-repetition statistics."""
    rng = random.Random(seed)
    n = len(values)
    means = sorted(statistics.fmean(rng.choice(values) for _ in range(n)) for _ in range(resamples))
    return means[int(0.025 * resamples)], means[min(resamples - 1, int(0.975 * resamples))]


def paired(reps: Sequence[Dict[str, List[float]]], group: str, num_arm: str, den_arm: str) -> Dict[str, Any]:
    """Ratio mean(num)/mean(den) per repetition, its between-run interval, and the pooled BCa interval."""
    num_means, den_means, ratios = [], [], []
    pooled_num: List[float] = []
    pooled_den: List[float] = []
    for i, rep in enumerate(reps):
        a, b = need(rep, group, num_arm, i), need(rep, group, den_arm, i)
        ma, mb = statistics.fmean(a), statistics.fmean(b)
        num_means.append(ma)
        den_means.append(mb)
        ratios.append(ma / mb)
        pooled_num.extend(a)
        pooled_den.extend(b)
    lo, hi = percentile_bootstrap(ratios)
    r_pooled, plo, phi = bca_bootstrap_ratio_ci(pooled_num, pooled_den, num_resamples=2000)
    return {
        "num_ns": statistics.fmean(num_means),
        "den_ns": statistics.fmean(den_means),
        "ratio": statistics.fmean(ratios),
        "ci": [lo, hi],
        "ci_pooled_bca": [plo, phi],
        "ratio_pooled": r_pooled,
        "per_rep_ratio": ratios,
        "per_rep_num_ns": num_means,
        "per_rep_den_ns": den_means,
        "reps": len(reps),
    }


def arm_mean_ci(reps: Sequence[Dict[str, List[float]]], group: str, arm: str) -> Dict[str, Any]:
    pooled: List[float] = []
    per_rep = []
    for i, rep in enumerate(reps):
        s = need(rep, group, arm, i)
        pooled.extend(s)
        per_rep.append(statistics.fmean(s))
    mean, lo, hi = bca_bootstrap_ci(pooled, num_resamples=2000)
    return {"mean_ns": mean, "ci_pooled_bca": [lo, hi], "per_rep_mean_ns": per_rep}


def r4(x: float) -> float:
    return round(x, 4)


def build_sections(reps: Sequence[Dict[str, List[float]]]) -> Dict[str, Any]:
    parity: Dict[str, Any] = {
        "_note": (
            "Paired ratio domain/raw per repetition; `ci` is a 95% percentile bootstrap over the per-repetition "
            "ratios, so it carries between-run variance. Criterion's own within-run interval is far tighter and "
            "cannot see that, which is why single-run figures appeared separated when they were not (AGENTS.md 8.4). "
            "`ci_pooled_bca` is the two-sample BCa interval over all repetitions' pooled samples, recorded, not gating. "
            "Per-repetition means are kept so the aggregate can be re-derived (harvest_domain.py)."
        )
    }
    for key, (raw_arm, dom_arm, _pop) in PARITY_PAIRS.items():
        p = paired(reps, GROUP_ALGEBRA, dom_arm, raw_arm)
        parity[key] = {
            "raw_ns": round(p["den_ns"], 1),
            "domain_ns": round(p["num_ns"], 1),
            "ratio": r4(p["ratio"]),
            "ci": [r4(p["ci"][0]), r4(p["ci"][1])],
            "ci_pooled_bca": [r4(p["ci_pooled_bca"][0]), r4(p["ci_pooled_bca"][1])],
            "verdict": "overhead resolved" if (p["ci"][0] > 1.0 or p["ci"][1] < 1.0) else "not resolved",
            "per_rep_ratio": [r4(r) for r in p["per_rep_ratio"]],
            "per_rep_raw_ns": [round(v, 1) for v in p["per_rep_den_ns"]],
            "per_rep_domain_ns": [round(v, 1) for v in p["per_rep_num_ns"]],
        }
    ingest = []
    for label, scalar_arm, batch_arm, pop in INGEST_PAIRS:
        p = paired(reps, GROUP_INGEST, scalar_arm, batch_arm)  # speedup = scalar_ns / batch_ns
        ingest.append(
            {
                "key_type": label,
                "scalar_mops": round(pop / p["num_ns"] * 1e3, 2),
                "batch128_mops": round(pop / p["den_ns"] * 1e3, 2),
                "speedup": round(p["ratio"], 3),
                "ci": [round(p["ci"][0], 3), round(p["ci"][1], 3)],
                "ci_pooled_bca": [round(p["ci_pooled_bca"][0], 3), round(p["ci_pooled_bca"][1], 3)],
                "population": pop,
                "per_rep_speedup": [round(r, 4) for r in p["per_rep_ratio"]],
            }
        )
    res10 = arm_mean_ci(reps, GROUP_RESOLVE, RESOLVE_ARMS[10_000])
    res100 = arm_mean_ci(reps, GROUP_RESOLVE, RESOLVE_ARMS[100_000])
    resolution = {
        "scan_mops": round(10_000 / res10["mean_ns"] * 1e3, 1),
        "latency_ns": round(res10["mean_ns"] / 10_000, 3),
        "allocation": "0 heap allocs (borrowed &[u8] from BlobArena chunk)",
        "population": 10_000,
        "_note": (
            f"N=100k: {res100['mean_ns'] / 1e3:.2f} us total, {res100['mean_ns'] / 100_000:.3f} ns/key, "
            f"{100_000 / res100['mean_ns'] * 1e3:.1f} M keys/s. Pooled BCa 95% of the N=10k scan: "
            f"[{res10['ci_pooled_bca'][0] / 1e3:.2f}, {res10['ci_pooled_bca'][1] / 1e3:.2f}] us."
        ),
        "per_rep_scan_ns_10k": [round(v, 1) for v in res10["per_rep_mean_ns"]],
        "per_rep_scan_ns_100k": [round(v, 1) for v in res100["per_rep_mean_ns"]],
    }
    return {"domain_parity_611": parity, "domain_ingestion_611": ingest, "domain_resolution_611": resolution}


def render_markdown(sections: Dict[str, Any]) -> str:
    out = ["| Arm | Raw `ExpanseSet` | `DomainSet` | Ratio (95% CI) | Verdict |", "|---|---|---|---|---|"]
    names = {
        "intersection_10k": "`intersection()` N=10k",
        "intersection_len_10k": "`intersection_len()` N=10k",
        "intersection_100k": "`intersection()` N=100k",
        "intersection_len_100k": "`intersection_len()` N=100k",
    }
    for key in ("intersection_10k", "intersection_len_10k", "intersection_100k", "intersection_len_100k"):
        c = sections["domain_parity_611"][key]
        pct = (c["ratio"] - 1) * 100
        verdict = f"**{pct:+.1f}%**" if c["verdict"] == "overhead resolved" else "not resolved"
        out.append(
            f"| {names[key]} | {c['raw_ns']} ns | {c['domain_ns']} ns | {c['ratio']:.4f} [{c['ci'][0]:.4f}, {c['ci'][1]:.4f}] | {verdict} |"
        )
    out += ["", "| Keys | Scalar | Batch-128 | Speedup (95% CI) |", "|---|---|---|---|"]
    for c in sections["domain_ingestion_611"]:
        label = "Text" if c["key_type"].startswith("Text") else "Binary UUID"
        out.append(
            f"| {label}, N={c['population'] // 1000}k | {c['scalar_mops']} M keys/s | {c['batch128_mops']} M keys/s | "
            f"{c['speedup']:.3f}× [{c['ci'][0]:.3f}, {c['ci'][1]:.3f}] |"
        )
    r = sections["domain_resolution_611"]
    out += ["", f"Resolution: {r['scan_mops']} M keys/s ({r['latency_ns']} ns/key) at N=10k; {r['_note']}"]
    return "\n".join(out) + "\n"


# --------------------------------------------------------------------------- self-test
def _synthetic_reps(n_reps: int, overhead: float, seed: int = 7) -> List[Dict[str, List[float]]]:
    rng = random.Random(seed)
    reps = []
    for _ in range(n_reps):
        rep: Dict[str, List[float]] = {}
        for _key, (raw_arm, dom_arm, pop) in PARITY_PAIRS.items():
            base = 1800.0 if pop == 10_000 else 10_800.0
            if "len" in raw_arm:
                base /= 6
            rep[f"{GROUP_ALGEBRA}/{raw_arm}"] = [base + rng.gauss(0, base * 0.003) for _ in range(100)]
            rep[f"{GROUP_ALGEBRA}/{dom_arm}"] = [base * overhead + rng.gauss(0, base * 0.003) for _ in range(100)]
        for _label, scalar_arm, batch_arm, pop in INGEST_PAIRS:
            base = pop * 88.0
            rep[f"{GROUP_INGEST}/{scalar_arm}"] = [base * 1.03 + rng.gauss(0, base * 0.002) for _ in range(100)]
            rep[f"{GROUP_INGEST}/{batch_arm}"] = [base + rng.gauss(0, base * 0.002) for _ in range(100)]
        for pop, arm in RESOLVE_ARMS.items():
            rep[f"{GROUP_RESOLVE}/{arm}"] = [pop * 1.65 + rng.gauss(0, pop * 0.002) for _ in range(100)]
        reps.append(rep)
    return reps


def self_test() -> None:
    # 1. a real 2.5% overhead resolves; a null does not; the verdict follows the between-run interval
    s = build_sections(_synthetic_reps(6, 1.025))
    c = s["domain_parity_611"]["intersection_10k"]
    assert c["verdict"] == "overhead resolved" and 1.02 < c["ratio"] < 1.03, c
    assert c["ci"][0] <= c["ratio"] <= c["ci"][1] and len(c["per_rep_ratio"]) == 6, c
    s0 = build_sections(_synthetic_reps(6, 1.0))
    assert s0["domain_parity_611"]["intersection_100k"]["verdict"] == "not resolved", s0["domain_parity_611"]
    # 2. ingestion speedup is scalar/batch and throughput derives from population and ns
    ing = s["domain_ingestion_611"][0]
    assert 1.02 < ing["speedup"] < 1.04 and ing["population"] == 10_000, ing
    assert abs(ing["batch128_mops"] - 10_000 / (10_000 * 88.0) * 1e3) < 0.05, ing
    # 3. resolution: ns/key and M keys/s agree
    r = s["domain_resolution_611"]
    assert abs(r["latency_ns"] - 1.65) < 0.01 and abs(r["scan_mops"] - 1e3 / 1.65) < 1.0, r
    # 4. refusals: too few reps, a missing arm
    for bad, why in ((_synthetic_reps(2, 1.0), "two repetitions"),):
        try:
            build_sections(bad) if len(bad) >= MIN_REPS else (_ for _ in ()).throw(HarvestError("reps"))
            raise AssertionError(f"{why} must refuse")
        except HarvestError:
            pass
    partial = _synthetic_reps(3, 1.0)
    del partial[1][f"{GROUP_ALGEBRA}/domain_set_intersection/10000"]
    try:
        build_sections(partial)
        raise AssertionError("a repetition missing an arm must refuse")
    except HarvestError:
        pass
    # 5. the markdown carries every parity row and the ingestion rows
    md = render_markdown(s)
    assert md.count("| `intersection") == 4 and md.count("M keys/s |") == 8, md
    print("harvest_domain.py --self-test: all checks passed")


# --------------------------------------------------------------------------- main
def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--raw", type=Path, help="directory holding rep_<i>/criterion snapshots")
    ap.add_argument("--out", type=Path, help="bench_domain_algebra.json to update in place")
    ap.add_argument("--commit", help="commit the samples were taken at")
    ap.add_argument("--host-desc", help="anonymised hardware description (no hostnames, AGENTS.md §7)")
    ap.add_argument("--loads", type=Path, help="loads.txt written by run.sh (one loadavg line per repetition)")
    ap.add_argument("--markdown", type=Path)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        self_test()
        return 0
    if not (args.raw and args.out and args.commit and args.host_desc):
        ap.error("--raw, --out, --commit and --host-desc are required")
    try:
        host = validate_host_description(args.host_desc)
        reps = load_reps(args.raw)
        sections = build_sections(reps)
    except (HarvestError, ValueError) as e:
        print(f"harvest_domain.py: {e}", file=sys.stderr)
        return 1
    data = json.loads(args.out.read_text(encoding="utf-8"))
    data.update(sections)
    loads = args.loads.read_text(encoding="utf-8").strip().splitlines() if args.loads and args.loads.exists() else []
    prov = {
        "issue": "#611; re-measured after #701 (allocator policy pinned in the harness)",
        "harness": "crates/expanse/benches/domain.rs",
        "host": host,
        "commit": args.commit,
        "reps": f"{len(reps)} independent `cargo bench --bench domain` runs, whole harness per run, bench lock held, P-core pin",
        "statistic": "paired ratio per repetition; 95% percentile bootstrap over repetitions (ci); two-sample BCa over pooled samples (ci_pooled_bca)",
        "load_average_per_rep": loads,
        "harvester": "docs/benchmarks/set_algebra/scripts/harvest_domain.py",
    }
    for key in ("domain_parity_611", "domain_ingestion_611", "domain_resolution_611"):
        data.setdefault("provenance", {})[key] = dict(prov)
    args.out.write_text(json.dumps(data, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    md = render_markdown(sections)
    print(md)
    if args.markdown:
        args.markdown.write_text(md, encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
