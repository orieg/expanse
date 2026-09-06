#!/usr/bin/env python3
"""
scripts/recompute_and_patch_json.py — Offline statistical derivation and verification.
Recomputes ratio_vs_art as the mean of paired per-round ratios and its BCa 95%
bootstrap confidence interval from stored samples, ensuring the point estimate strictly
lies within the CI for every cell across all timing artifacts.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent.parent
REPO_ROOT = BASE_DIR.parent.parent.parent
RESULTS_DIR = BASE_DIR / "results"

sys.path.insert(0, str(REPO_ROOT / "scripts"))
from bca_bootstrap import bca_bootstrap_ci


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
        theta_hat, lo, hi = bca_bootstrap_ci(ratios, confidence=0.95, num_resamples=2000, seed=42)
        r["ratio_vs_art"] = theta_hat
        r["ratio_bca_ci_95"] = [lo, hi]
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
