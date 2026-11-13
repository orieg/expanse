#!/usr/bin/env python3
"""Head-against-base reading of two pairs of `compare` runs (an engine
increment's pre-registration, docs/BENCHMARKING.md).

Inputs are the `wallclock-baseline-compare-<run>` artifacts
(`expanse.baseline.v1`, per-arm `samples_ns`), two runs at the increment's
head and two at its merge base. For every Expanse cell of the lookup and
iteration groups (`map_get`, `map_get_cold_dram`, `set_lookup`, `map_iter`):

- **Shift**: head time / base time, a two-sample BCa 95% interval of the ratio
  of means, head run i paired with base run i; the reading uses both pairings.
- **Side** against each competitor in the same run (Expanse time / competitor
  time): FASTER when the upper end is below 1 in both runs, SLOWER when the
  lower end is above 1 in both, EQUIVALENT when both intervals lie in
  [0.95, 1.05], otherwise INCONCLUSIVE.

Rules, as the pre-registration states them:
- **C1, competitor side.** No (cell, competitor) that is FASTER or EQUIVALENT
  at the base is SLOWER at the head.
- **C2, no slowdown against the base.** No cell has its shift lower end above
  1.02 in both pairings.

Usage:
  compare_shift.py --head H1 H2 --base B1 B2 [--out artifact.json]
  compare_shift.py --self-test
"""
import argparse
import json
import random
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from bca_bootstrap import bca_bootstrap_ratio_ci_with_method  # noqa: E402

GROUPS = ("map_get", "map_get_cold_dram", "set_lookup", "map_iter")
C2_CEILING = 1.02


def cells(path):
    """(cell key without the implementation, implementation) -> samples_ns."""
    art = json.loads(Path(path).read_text())
    if art.get("schema") != "expanse.baseline.v1":
        sys.exit(f"STOP: {path} is not an expanse.baseline.v1 artifact")
    out = {}
    for a in art["arms"]:
        if a["id"].split("/")[0] not in GROUPS or a.get("status") != "OK":
            continue
        parts = a["id"].split("/")
        impl = a["function_id"]
        key = "/".join(p for p in parts if p != impl)
        out[(key, impl)] = a["samples_ns"]
    if not out:
        sys.exit(f"STOP: no lookup or iteration arm in {path}")
    return out


def ratio(num, den):
    r, lo, hi, method = bca_bootstrap_ratio_ci_with_method(num, den, num_resamples=2000, seed=42)
    return {"ratio": r, "ci_lower": lo, "ci_upper": hi, "ci_method": method}


def side(cis):
    if all(c["ci_upper"] < 1.0 for c in cis):
        return "FASTER"
    if all(c["ci_lower"] > 1.0 for c in cis):
        return "SLOWER"
    if all(c["ci_lower"] >= 0.95 and c["ci_upper"] <= 1.05 for c in cis):
        return "EQUIVALENT"
    return "INCONCLUSIVE"


def read(head, base):
    """The shift and side rows, and the rule failures."""
    hs, bs = [cells(p) for p in head], [cells(p) for p in base]
    keys = sorted({k for k, impl in hs[0] if impl == "expanse"})
    rows, failures = [], []
    for key in keys:
        if any((key, "expanse") not in c for c in hs + bs):
            failures.append(f"{key}: Expanse arm missing in a run")
            continue
        shift = [ratio(hs[i][(key, "expanse")], bs[i][(key, "expanse")]) for i in range(2)]
        row = {"cell": key, "shift": shift, "sides": {}}
        if all(s["ci_lower"] > C2_CEILING for s in shift):
            failures.append(f"C2: {key}: shift lower end above {C2_CEILING} in both pairings")
        for comp in sorted({impl for k, impl in hs[0] if k == key and impl != "expanse"}):
            if any((key, comp) not in c for c in hs + bs):
                continue
            at_base = side([ratio(b[(key, "expanse")], b[(key, comp)]) for b in bs])
            at_head = side([ratio(h[(key, "expanse")], h[(key, comp)]) for h in hs])
            row["sides"][comp] = {"base": at_base, "head": at_head}
            if at_base in ("FASTER", "EQUIVALENT") and at_head == "SLOWER":
                failures.append(f"C1: {key} against {comp}: {at_base} at the base, SLOWER at the head")
        rows.append(row)
    return rows, failures


def self_test():
    rng = random.Random(7)

    def art(scale_expanse, scale_comp):
        arms = []
        for key, base_ns in (("map_get/random/10000/{}/hit", 40.0), ("map_iter/random/10000/{}", 5.0)):
            for impl, s in (("expanse", scale_expanse), ("hash", scale_comp)):
                arms.append({"id": key.format(impl), "function_id": impl, "status": "OK",
                             "samples_ns": [base_ns * s * (1 + rng.gauss(0, 0.01)) for _ in range(100)]})
        return {"schema": "expanse.baseline.v1", "arms": arms}

    import tempfile
    with tempfile.TemporaryDirectory() as td:
        def write(name, a):
            p = Path(td) / name
            p.write_text(json.dumps(a))
            return p
        same = [write(f"s{i}.json", art(1.0, 1.02)) for i in range(4)]
        rows, failures = read(same[:2], same[2:])
        assert not failures, failures
        assert all(abs(r["shift"][0]["ratio"] - 1) < 0.01 for r in rows), rows
        # A 5% slowdown of Expanse fails C2, and turns FASTER against the
        # competitor (0.98) into SLOWER (1.03): C1 too.
        slow = [write(f"h{i}.json", art(1.05, 1.02)) for i in range(2)]
        rows, failures = read(slow, same[2:])
        assert any(f.startswith("C2:") for f in failures), failures
        assert any(f.startswith("C1:") for f in failures), failures
    print("compare_shift.py --self-test: all checks passed")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--head", nargs=2)
    ap.add_argument("--base", nargs=2)
    ap.add_argument("--out")
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        self_test()
        return
    if not a.head or not a.base:
        ap.error("--head and --base each take two artifacts")
    rows, failures = read(a.head, a.base)
    print("| cell | shift pairing 1 | shift pairing 2 | sides (base -> head) |")
    print("|---|---|---|---|")
    for r in rows:
        sh = [f"{s['ratio']:.3f} [{s['ci_lower']:.3f}, {s['ci_upper']:.3f}]" for s in r["shift"]]
        sides = "; ".join(f"{c}: {v['base']} -> {v['head']}" for c, v in r["sides"].items())
        print(f"| {r['cell']} | {sh[0]} | {sh[1]} | {sides} |")
    print("\nverdict:", "PASS" if not failures else "FAIL")
    for f in failures:
        print("  " + f)
    if a.out:
        Path(a.out).write_text(json.dumps({"head": a.head, "base": a.base, "rows": rows,
                                           "failures": failures}, indent=1) + "\n")


if __name__ == "__main__":
    main()
