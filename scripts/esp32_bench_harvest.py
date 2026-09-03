#!/usr/bin/env python3
"""
scripts/esp32_bench_harvest.py

Parses an ESP32 on-device UART harvest log, groups measurements by
(benchmark, arm, n, pop), and computes BCa bootstrap 95% confidence intervals
(>= 1000 resamples) per AGENTS.md §8.4.

Usage:
  python3 scripts/esp32_bench_harvest.py < uart_log.txt
  python3 scripts/esp32_bench_harvest.py --input /path/to/log.txt --out report.md --emit-json results.json
  python3 scripts/esp32_bench_harvest.py --self-test

Nothing in the generated report is stamped: the chip, revision, clock, IDF
version and engine version all come from the provenance line the firmware
prints at boot, and every table cell is derived from that run's samples
(§8.2). A log with no provenance line is refused rather than reported under a
guessed target (§8.1).
"""

import sys
import json
import argparse
import statistics
from collections import defaultdict

import numpy as np

# Fixed so a re-run of the harvester over the same log reproduces the same
# interval. The randomness here is resampling, not measurement.
BOOTSTRAP_SEED = 0x59_79_00
DEFAULT_RESAMPLES = 2000

# Reference offered-load rates for the derived duty-cycle table. These are
# not measured -- they are the rates the measured per-op cost is projected
# against, and are labelled as such wherever they are printed.
DUTY_CYCLE_RATES_HZ = (1, 10, 100, 1000)


# An arm whose slowest repetition exceeds its median by this factor has had
# something land inside a timed window -- a FreeRTOS tick, a flash-cache miss
# storm, a radio housekeeping pass. One such repetition in ten moves the mean
# by more than any code change this suite measures: an ESP32 ingest arm whose
# ten samples were 1355, 1351, 1356, *16121*, 1356, 1351, 1352, 1351, 1355,
# 1355 reported a mean of 2830 against a median of 1355, and the arm's C
# source had not changed at all between the two builds being compared.
#
# So every arm carries min and median next to the mean, and the report leads
# with the median. This follows the on-target convention the STM32 suite
# already uses (docs/benchmarks/stm32h747/results.json records min / median /
# max per fixture). The BCa interval on the mean is kept because §8.4 asks
# for it, but it is not the headline: a bootstrap over a contaminated sample
# faithfully describes a contaminated sample.
CONTAMINATION_RATIO = 2.0


class HarvestError(RuntimeError):
    """Raised when the log cannot support the report being asked for."""


def robust_stats(values):
    """min / median / mean / spread for one arm's repetitions."""
    v = sorted(float(x) for x in values)
    lo, hi = v[0], v[-1]
    med = statistics.median(v)
    return {
        "min": lo,
        "median": med,
        "mean": statistics.fmean(v),
        "max": hi,
        "spread_ratio": (hi / lo) if lo > 0 else 0.0,
        "contaminated": bool(med > 0 and hi > med * CONTAMINATION_RATIO),
    }


def bootstrap_ci_bca(data, num_resamples=DEFAULT_RESAMPLES, alpha=0.05, seed=BOOTSTRAP_SEED):
    """Bias-corrected and accelerated (BCa) bootstrap CI for the mean.

    Efron & Tibshirani, *An Introduction to the Bootstrap* (1993), §14.3:
    the percentile endpoints are shifted by a bias correction ``z0`` (how far
    the bootstrap distribution sits off the observed statistic) and an
    acceleration ``a`` (the jackknife skew of the statistic). A plain
    percentile bootstrap sets both to zero, which is only right for a
    symmetric, unbiased statistic -- cycles-per-op on a microcontroller is
    neither, since it is bounded below and has a long upper tail.

    Returns ``(point_estimate, ci_low, ci_high, method)`` where ``method`` is
    "bca", or "percentile"/"minmax" when the sample cannot support BCa. The
    method is reported rather than silently substituted (§8.1).
    """
    data = np.asarray(data, dtype=float)
    n = len(data)
    theta_hat = float(np.mean(data)) if n else 0.0

    if n < 3:
        # Too few samples for any interval worth the name.
        return theta_hat, float(np.min(data)) if n else 0.0, float(np.max(data)) if n else 0.0, "minmax"

    rng = np.random.default_rng(seed)
    boot_means = rng.choice(data, size=(num_resamples, n), replace=True).mean(axis=1)

    lo_p = 100.0 * (alpha / 2.0)
    hi_p = 100.0 * (1.0 - alpha / 2.0)

    # Every sample identical: the statistic has no spread, so BCa's
    # denominators vanish. The degenerate interval is the exact answer.
    if np.all(data == data[0]):
        return theta_hat, theta_hat, theta_hat, "percentile"

    norm = statistics.NormalDist()

    # Bias correction: where theta_hat falls in the bootstrap distribution.
    prop_less = float(np.mean(boot_means < theta_hat))
    if prop_less <= 0.0 or prop_less >= 1.0:
        # theta_hat outside the bootstrap support; z0 is not finite.
        return theta_hat, float(np.percentile(boot_means, lo_p)), float(np.percentile(boot_means, hi_p)), "percentile"
    z0 = norm.inv_cdf(prop_less)

    # Acceleration: jackknife skewness of the statistic.
    total = data.sum()
    jack = (total - data) / (n - 1)
    jack_dev = jack.mean() - jack
    denom = 6.0 * float((jack_dev ** 2).sum()) ** 1.5
    if denom == 0.0:
        return theta_hat, float(np.percentile(boot_means, lo_p)), float(np.percentile(boot_means, hi_p)), "percentile"
    a = float((jack_dev ** 3).sum()) / denom

    def endpoint(z_alpha):
        adj = z0 + (z0 + z_alpha) / (1.0 - a * (z0 + z_alpha))
        return 100.0 * norm.cdf(adj)

    p_lo = endpoint(norm.inv_cdf(alpha / 2.0))
    p_hi = endpoint(norm.inv_cdf(1.0 - alpha / 2.0))
    if not (0.0 <= p_lo < p_hi <= 100.0):
        return theta_hat, float(np.percentile(boot_means, lo_p)), float(np.percentile(boot_means, hi_p)), "percentile"

    return theta_hat, float(np.percentile(boot_means, p_lo)), float(np.percentile(boot_means, p_hi)), "bca"


def parse_and_process(lines):
    """Splits a harvest log into (provenance, samples, stack report).

    Grouping is by (benchmark, n, pop): `n` alone does not identify a
    workload, because an arm that walks a fixed number of keys costs more
    over a larger population (§8.3).
    """
    records = defaultdict(lambda: defaultdict(list))
    metrics = defaultdict(list)
    provenance = None
    stack = None

    for line in lines:
        line = line.strip()
        if not line.startswith("{") or not line.endswith("}"):
            continue
        try:
            obj = json.loads(line)
        except ValueError:
            # A line torn by a reset mid-print is not a measurement.
            continue

        if "target" in obj and "benchmark" not in obj:
            provenance = obj
            continue
        if "stack_min_free_bytes" in obj:
            stack = obj
            continue
        if "metric" in obj:
            # A non-timing observation (a ratio, a byte count). Kept apart
            # from the cycles-per-op samples so it never reaches a derivation
            # that assumes a cycle count.
            metrics[(obj["metric"], obj.get("arm"), obj.get("pop"))].append(obj)
            continue

        bench = obj.get("benchmark")
        arm = obj.get("arm")
        n = obj.get("n")
        if not (bench and arm and n is not None):
            continue
        records[(bench, n, obj.get("pop"))][arm].append({
            "cycles": obj.get("cycles_per_op"),
            "heap": obj.get("heap_used_bytes"),
            "frag": obj.get("frag_ratio"),
        })

    return records, provenance, stack, metrics


def duty_cycle_table(cycles_per_op, cpu_hz):
    """Fraction of wall time the CPU is busy at each reference offered load.

    Derived, not measured: ``cycles_per_op / cpu_hz`` is the CPU-seconds one
    operation costs, so at R operations per second the duty cycle is that
    product. Above 1.0 the part cannot keep up with the offered rate.
    """
    if not cpu_hz:
        return {}
    secs_per_op = cycles_per_op / float(cpu_hz)
    return {str(rate): secs_per_op * rate for rate in DUTY_CYCLE_RATES_HZ}


def summarise_metrics(metrics):
    """BCa summary of the non-timing observations, keyed as they arrived."""
    out = {}
    for (name, arm, pop), rows in sorted(metrics.items(), key=lambda kv: (kv[0][0], kv[0][1] or "", kv[0][2] or 0)):
        entry = {"metric": name, "arm": arm, "pop": pop, "samples": len(rows), "fields": {}}
        numeric = [k for k in rows[0] if isinstance(rows[0][k], (int, float)) and k not in ("pop",)]
        for field in sorted(numeric):
            vals = [r[field] for r in rows if isinstance(r.get(field), (int, float))]
            if not vals:
                continue
            mean, lo, hi, method = bootstrap_ci_bca(vals)
            entry["fields"][field] = {
                "mean": mean, "ci_95_low": lo, "ci_95_high": hi, "ci_method": method,
            }
        out[f"{name}_{arm}_pop{pop}"] = entry
    return out


def generate_structured_results(records, provenance=None, stack=None, metrics=None):
    """Structured dict with BCa CIs, provenance and derived duty cycle."""
    cpu_hz = (provenance or {}).get("cpu_hz")
    results = {
        "provenance": provenance or {},
        "stack": stack or {},
        "duty_cycle_reference_rates_hz": list(DUTY_CYCLE_RATES_HZ),
        "benchmarks": {},
        "metrics": summarise_metrics(metrics or {}),
    }

    for (bench, n, pop), arm_data in sorted(records.items(), key=lambda kv: (kv[0][0], kv[0][1], kv[0][2] or 0)):
        bench_key = f"{bench}_n{n}" + (f"_pop{pop}" if pop is not None else "")
        entry = {"benchmark": bench, "n": n, "pop": pop, "arms": {}}
        for arm, samples in sorted(arm_data.items()):
            cycles_list = [s["cycles"] for s in samples if s["cycles"] is not None]
            heaps = [s["heap"] for s in samples if s["heap"] is not None]
            frags = [s["frag"] for s in samples if s["frag"] is not None]

            if cycles_list:
                mean_c, ci_l, ci_h, method = bootstrap_ci_bca(cycles_list)
            else:
                mean_c, ci_l, ci_h, method = 0.0, 0.0, 0.0, "none"

            rs = robust_stats(cycles_list) if cycles_list else {}
            entry["arms"][arm] = {
                "cycles_per_op": {
                    # `median` is the figure to quote and compare; `mean` and
                    # its interval are kept for continuity and for §8.4.
                    "median": rs.get("median", 0.0),
                    "min": rs.get("min", 0.0),
                    "max": rs.get("max", 0.0),
                    "spread_ratio": rs.get("spread_ratio", 0.0),
                    "contaminated": rs.get("contaminated", False),
                    "mean": mean_c,
                    "ci_95_low": ci_l,
                    "ci_95_high": ci_h,
                    "ci_method": method,
                    "sample_count": len(cycles_list),
                },
                "duty_cycle_projected": duty_cycle_table(rs.get("median", mean_c), cpu_hz),
                "heap_used_bytes": float(np.mean(heaps)) if heaps else 0.0,
                "frag_ratio": float(np.mean(frags)) if frags else 0.0,
            }
        results["benchmarks"][bench_key] = entry
    return results


def generate_markdown_report(records, provenance=None, stack=None, metrics=None):
    if not provenance:
        raise HarvestError(
            "no provenance line in the log: the firmware prints one "
            '{"target": ...} object at boot and every number is tagged from '
            "it. Refusing to write a report with an unidentified target "
            "(AGENTS.md §8.7)."
        )

    cpu_hz = provenance.get("cpu_hz")
    target = provenance.get("target", "unknown")
    md = [
        f"# Expanse on-device harvest — {target}",
        "",
        "| Fact | Value |",
        "|---|---|",
        f"| Chip | `{target}` |",
        f"| Revision | v{provenance.get('revision', '?')} |",
        f"| Cores | {provenance.get('cores', '?')} |",
        f"| CPU clock | {(cpu_hz or 0) / 1e6:.0f} MHz |",
        f"| ESP-IDF | `{provenance.get('idf', '?')}` |",
        f"| Engine | `{provenance.get('expanse', '?')}` |",
        f"| Free internal heap at boot | {provenance.get('free_internal', '?')} B |",
        f"| Largest free block at boot | {provenance.get('largest_internal', '?')} B |",
    ]
    if stack:
        md.append(
            f"| Main-task stack | {stack.get('stack_min_free_bytes', '?')} B free of "
            f"{stack.get('stack_total_bytes', '?')} B at high-water |"
        )
    md += [
        "",
        "The figure to compare is the **median** of the repetitions. A single "
        "repetition in which a FreeRTOS tick or a flash-cache miss storm lands "
        "inside the timed window moves the mean by more than any code change "
        "this suite measures, so the mean and its BCa 95% bootstrap interval "
        f"({DEFAULT_RESAMPLES} resamples, AGENTS.md §8.4) are reported beside it "
        "rather than as the headline — a bootstrap over a contaminated sample "
        "faithfully describes a contaminated sample. An arm whose slowest "
        f"repetition exceeds its median by more than {CONTAMINATION_RATIO:g}x is "
        "marked ⚠ and its mean should not be compared against anything. This "
        "matches the on-target convention the STM32 suite already uses.",
        "",
        "| Benchmark | N ops | Population | Arm | Cycles/op (median) | Mean [BCa 95% CI] | Samples | Heap used (B) | Frag ratio |",
        "|---|---|---|---|---|---|---|---|---|",
    ]

    for (bench, n, pop), arm_data in sorted(records.items(), key=lambda kv: (kv[0][0], kv[0][1], kv[0][2] or 0)):
        for arm, samples in sorted(arm_data.items()):
            cycles_list = [s["cycles"] for s in samples if s["cycles"] is not None]
            heaps = [s["heap"] for s in samples if s["heap"] is not None]
            frags = [s["frag"] for s in samples if s["frag"] is not None]

            if cycles_list:
                mean_c, ci_l, ci_h, method = bootstrap_ci_bca(cycles_list)
                rs = robust_stats(cycles_list)
                flag = " ⚠" if rs["contaminated"] else ""
                med_str = f"**{rs['median']:.1f}**{flag}"
                mean_str = f"{mean_c:.1f} [{ci_l:.1f}, {ci_h:.1f}] ({method})"
            else:
                med_str, mean_str = "N/A", "N/A"

            heap_str = f"{int(np.mean(heaps))}" if heaps else "N/A"
            frag_str = f"{np.mean(frags):.4f}" if frags else "N/A"
            pop_str = str(pop) if pop is not None else "—"
            md.append(
                f"| `{bench}` | {n} | {pop_str} | `{arm}` | {med_str} | {mean_str} | "
                f"{len(cycles_list)} | {heap_str} | {frag_str} |"
            )

    if metrics:
        md += ["", "## Fragmentation after insert/delete churn", "",
               "Free-pool fragmentation is `1 - largest_free_block / total_free`: "
               "the share of what is free that no single allocation can reach. "
               "`frag_delta` is the change across the churn itself, which is the "
               "arm's subject; it is a ratio, not a timing, and is deliberately "
               "kept out of the duty-cycle derivation below.", "",
               "| Metric | Arm | Population | Field | Mean (BCa 95% CI) | Method | Samples |",
               "|---|---|---|---|---|---|---|"]
        for key, entry in sorted(summarise_metrics(metrics).items()):
            for field, stat in sorted(entry["fields"].items()):
                md.append(
                    f"| `{entry['metric']}` | `{entry['arm']}` | {entry['pop']} | `{field}` | "
                    f"{stat['mean']:.4f} [{stat['ci_95_low']:.4f}, {stat['ci_95_high']:.4f}] | "
                    f"{stat['ci_method']} | {entry['samples']} |"
                )

    if cpu_hz:
        md += [
            "",
            "## Projected duty cycle",
            "",
            "Derived from the measured cycles/op above and the reported clock "
            f"({(cpu_hz or 0) / 1e6:.0f} MHz), not measured: the fraction of wall time the "
            "CPU spends on this operation at a given offered rate. A cell over "
            "1.0 means the part cannot sustain that rate.",
            "",
            "| Benchmark | Population | Arm | " + " | ".join(f"{r}/s" for r in DUTY_CYCLE_RATES_HZ) + " |",
            "|---|---|---|" + "---|" * len(DUTY_CYCLE_RATES_HZ),
        ]
        for (bench, n, pop), arm_data in sorted(records.items(), key=lambda kv: (kv[0][0], kv[0][1], kv[0][2] or 0)):
            for arm, samples in sorted(arm_data.items()):
                cycles_list = [s["cycles"] for s in samples if s["cycles"] is not None]
                if not cycles_list:
                    continue
                duty = duty_cycle_table(robust_stats(cycles_list)["median"], cpu_hz)
                cells = " | ".join(f"{duty[str(r)]:.2e}" for r in DUTY_CYCLE_RATES_HZ)
                pop_str = str(pop) if pop is not None else "—"
                md.append(f"| `{bench}` | {pop_str} | `{arm}` | {cells} |")

    return "\n".join(md) + "\n"


def run_self_tests():
    # 1. BCa against a reference: a strongly right-skewed sample must give an
    #    asymmetric interval, where the plain percentile bootstrap would not.
    skewed = [1.0, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.9, 12.0]
    mean, lo, hi, method = bootstrap_ci_bca(skewed)
    assert method == "bca", method
    assert lo < mean < hi, (lo, mean, hi)
    assert (hi - mean) > (mean - lo), "BCa must widen the tail the data is skewed toward"

    # 2. A degenerate sample must be reported as such, not as a fake interval.
    mean, lo, hi, method = bootstrap_ci_bca([7.0] * 8)
    assert (mean, lo, hi) == (7.0, 7.0, 7.0) and method == "percentile"

    # 3. Determinism: the same log must give the same interval twice.
    assert bootstrap_ci_bca(skewed) == bootstrap_ci_bca(skewed)

    # 4. n and pop together key the group: the aggregation arm reports the
    #    same n over two populations and must not be pooled (#579).
    lines = [
        '{"target": "esp32", "revision": 301, "cores": 2, "cpu_hz": 160000000, "idf": "v6.0", "expanse": "0.5.0", "free_internal": 1, "largest_internal": 1}',
        '{"benchmark": "agg", "arm": "expanse_memtable", "n": 500, "pop": 500, "cycles_per_op": 955.0, "heap_used_bytes": 3480, "frag_ratio": 0.62}',
        '{"benchmark": "agg", "arm": "expanse_memtable", "n": 500, "pop": 500, "cycles_per_op": 957.0, "heap_used_bytes": 3480, "frag_ratio": 0.62}',
        '{"benchmark": "agg", "arm": "expanse_memtable", "n": 500, "pop": 2000, "cycles_per_op": 1555.0, "heap_used_bytes": 12416, "frag_ratio": 0.61}',
        '{"benchmark": "agg", "arm": "expanse_memtable", "n": 500, "pop": 2000, "cycles_per_op": 1563.0, "heap_used_bytes": 12416, "frag_ratio": 0.61}',
        '{"stack_min_free_bytes": 4052, "stack_total_bytes": 8192}',
        '{"metric": "churn_fragmentation", "arm": "expanse_memtable", "pop": 500, "cycles": 8, "frag_before": 0.44, "frag_after": 0.45, "frag_delta": 0.01, "heap_retained_bytes": 476}',
        '{"metric": "churn_fragmentation", "arm": "expanse_memtable", "pop": 500, "cycles": 8, "frag_before": 0.44, "frag_after": 0.46, "frag_delta": 0.02, "heap_retained_bytes": 480}',
    ]
    recs, prov, stack, mets = parse_and_process(lines)
    assert len(recs) == 2, f"populations were pooled: {list(recs)}"
    assert prov["target"] == "esp32"
    assert stack["stack_min_free_bytes"] == 4052
    # A ratio-valued metric must not be filed as a timing sample: it would
    # otherwise reach the duty-cycle derivation and come out as a time.
    assert len(mets) == 1 and len(next(iter(mets.values()))) == 2
    assert all("churn" not in b for b in recs)

    # 5. A torn line (reset mid-print) is dropped, not parsed into a number.
    torn, _, _, _ = parse_and_process(['{"benchmark": "agg", "arm": "exp'])
    assert not torn

    # 5b. The contamination detector, against the sample that motivated it:
    #     one ESP32 repetition in ten spiked 12x and doubled the arm's mean
    #     while its C source had not changed.
    spiky = [1355.60, 1350.85, 1355.60, 16120.60, 1355.68, 1350.85, 1352.10,
             1350.85, 1354.56, 1355.04]
    rs = robust_stats(spiky)
    assert rs["contaminated"], "a 12x outlier in ten samples must be flagged"
    assert abs(rs["median"] - 1354.8) < 1.0, rs["median"]
    assert rs["mean"] > 2800, "the mean is what the median protects against"
    clean = robust_stats([1350.85, 1354.29, 1350.85, 1354.41, 1350.85])
    assert not clean["contaminated"], "a clean sample must not be flagged"

    # 6. Duty cycle is a pure derivation of cycles/op and the clock.
    duty = duty_cycle_table(160.0, 160_000_000)
    assert abs(duty["1"] - 1e-6) < 1e-12 and abs(duty["1000"] - 1e-3) < 1e-9

    # 7. The report is refused without provenance, rather than guessing a target.
    try:
        generate_markdown_report(recs, None, None)
    except HarvestError:
        pass
    else:
        raise AssertionError("a log with no provenance line must be refused")

    # 8. Nothing in the report is stamped: the chip name comes from the log.
    report = generate_markdown_report(recs, prov, stack, mets)
    assert "esp32" in report and "esp32c3" not in report
    assert "frag_delta" in report
    assert "160 MHz" in report

    structured = generate_structured_results(recs, prov, stack, mets)
    assert structured["provenance"]["target"] == "esp32"
    assert "agg_n500_pop500" in structured["benchmarks"]
    assert "agg_n500_pop2000" in structured["benchmarks"]

    print("scripts/esp32_bench_harvest.py --self-test: all checks passed")


def main():
    parser = argparse.ArgumentParser(description="Harvest ESP32 on-device benchmark metrics")
    parser.add_argument("--input", help="Path to UART input log file (reads stdin if omitted)")
    parser.add_argument("--out", help="Path to output markdown file (prints stdout if omitted)")
    parser.add_argument("--emit-json", help="Path to output JSON results file for charting and diffing")
    parser.add_argument("--self-test", action="store_true", help="Run internal unit self-tests")
    args = parser.parse_args()

    if args.self_test:
        run_self_tests()
        return

    if args.input:
        with open(args.input, "r", encoding="utf-8", errors="replace") as f:
            lines = f.readlines()
    else:
        lines = sys.stdin.readlines()

    records, provenance, stack, metrics = parse_and_process(lines)
    if not records:
        raise HarvestError("no benchmark samples in the log")
    report = generate_markdown_report(records, provenance, stack, metrics)

    if args.out:
        with open(args.out, "w", encoding="utf-8") as f:
            f.write(report)
        print(f"Report written to {args.out}")
    else:
        print(report)

    if args.emit_json:
        structured = generate_structured_results(records, provenance, stack, metrics)
        with open(args.emit_json, "w", encoding="utf-8") as f:
            json.dump(structured, f, indent=2)
            f.write("\n")
        print(f"JSON results written to {args.emit_json}")


if __name__ == "__main__":
    try:
        main()
    except HarvestError as exc:
        print(f"error: {exc}", file=sys.stderr)
        sys.exit(1)
