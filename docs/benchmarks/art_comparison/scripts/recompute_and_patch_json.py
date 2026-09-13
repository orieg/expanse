#!/usr/bin/env python3
"""
scripts/recompute_and_patch_json.py — Offline statistical derivation and verification.
Recomputes ratio_vs_art as the mean of paired per-round ratios and its BCa 95%
bootstrap confidence interval from stored samples, ensuring the point estimate strictly
lies within the CI for every cell across all timing artifacts.

This script REWRITES the committed artifacts in place, and its output is not
interpreter-independent: CPython 3.12 gave `sum()` Neumaier compensated
summation, which moves `theta_hat = sum(data) / n` by up to a ULP, and a ULP
there moves the bias correction's `less_count` and so the selected percentile
index. The committed intervals were produced on an older interpreter; across
this repository's five BCa-bearing suites, re-running on 3.14 reproduces 976 of
1,230 cells and moves 254. So run it only as part of a re-measurement whose
numbers are re-published under a fresh provenance tag (AGENTS.md §8.7) — never
as a tidy-up pass over artifacts that are already published.
`docs/BENCHMARKING.md` carries the measurement.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent.parent
REPO_ROOT = BASE_DIR.parent.parent.parent
RESULTS_DIR = BASE_DIR / "results"

sys.path.insert(0, str(REPO_ROOT / "scripts"))
from bca_bootstrap import bca_bootstrap_ci_with_method


def process_timing_file(filename: str) -> None:
    p = RESULTS_DIR / filename
    with open(p, "r", encoding="utf-8") as f:
        data = json.load(f)

    meta = data.get("metadata")
    if not meta:
        raise ValueError(f"Artifact {filename} is missing required 'metadata' block!")
    for k in ["host", "kernel", "load_start", "load_end", "harness_sha"]:
        if k not in meta:
            raise ValueError(f"Artifact {filename} metadata missing required key '{k}'!")

    if "data_sha" in meta:
        del meta["data_sha"]

    for r in data["results"]:
        # Per-round rows, not per-arm arrays: the artifact publishes
        # `rounds_raw` so a median or a ratio can be recomputed from it
        # (AGENTS.md section 8.12, #732). A round whose ratio is absent
        # carries null and is skipped, as the harness skipped it.
        rows = r.get("rounds_raw")
        if not rows:
            continue
        ratios = [row["ratio_vs_art"] for row in rows if row.get("ratio_vs_art") is not None]
        if not ratios:
            continue
        theta_hat, lo, hi, ci_method = bca_bootstrap_ci_with_method(
            ratios, confidence=0.95, num_resamples=2000, seed=42
        )
        r["ratio_vs_art"] = theta_hat
        r["ratio_bca_ci_95"] = [lo, hi]
        # Which construction produced the interval, from
        # `bca_bootstrap.CI_METHOD_*` (#880). The key is named
        # `ratio_bca_ci_95`, so a cell where BCa's corrections did not survive
        # the sample has to say so rather than let the key name assert it
        # (AGENTS.md §8.1).
        r["ratio_bca_ci_95_method"] = ci_method
        assert lo <= theta_hat <= hi, f"Point estimate {theta_hat} outside CI [{lo}, {hi}] in {filename} {r}"

    with open(p, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2)
    print(f"Verified {filename}: point estimates strictly inside BCa 95% CIs.")


def process_memory_file(filename: str) -> None:
    p = RESULTS_DIR / filename
    with open(p, "r", encoding="utf-8") as f:
        data = json.load(f)

    meta = data.get("metadata")
    if not meta:
        raise ValueError(f"Artifact {filename} is missing required 'metadata' block!")
    for k in ["host", "kernel", "load_start", "load_end", "harness_sha"]:
        if k not in meta:
            raise ValueError(f"Artifact {filename} metadata missing required key '{k}'!")

    if "data_sha" in meta:
        del meta["data_sha"]

    with open(p, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2)
    print(f"Verified metadata in {filename}.")


def main() -> None:
    for f in ["baseline_lookup_hit.json", "baseline_lookup_miss.json", "baseline_insert.json", "baseline_scan.json"]:
        process_timing_file(f)
    process_memory_file("baseline_memory.json")
    print("All JSON files verified successfully.")


if __name__ == "__main__":
    main()
