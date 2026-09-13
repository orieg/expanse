#!/usr/bin/env python3
"""Reads the #802 concurrent arm's verdicts against METHODOLOGY section 5.9.

Two artifacts in, one verdict per hypothesis out. The rules are section 5.9's
as it was locked, and nothing here adds to them:

- **The run must be the one section 5.9 fixed**, or no verdict is read from it:
  commit `0ed8f5e5`, pin `0,2,4,6,8,10,12,14` with source
  `EXPANSE_BENCH_PIN_APPLIED`, rounds 0-4 in every `(idle, paced, free) x R`
  cell for `R` in 1, 2, 4, 7, a 2.0 s window, 250,000 inserts/s offered.
  Section 5.7 is what a run that silently left its pre-registered pin looks
  like, and this refuses it by name.
- **H1** is gated when no paced `R = 7` cell is above the offered rate and no
  writer in one ran out of keys; it is then `PASS` / `REFUTED` /
  `BOUNDARY_RESULT` on paced `S(7)` against 2.0 (section 5.3), otherwise
  `NOT_GATED`. Gateability comes from `paced_rate_report` in the driver, so the
  driver's flag and this reading are one implementation.
- **H2** is the same three-way reading on idle `S(7)` against 3.5.
- **Both runs**: a verdict is claimed only where both runs return it;
  otherwise it is `BOUNDARY_RESULT`.
- **H3** is read only where `fit_usl_with_bootstrap` returns a usable `alpha`
  interval for a run's paced curve, against `predicted_alpha_from_scaling` at
  that run's paced `S(7)` point; otherwise `NOT_EVALUABLE`.

**H3 requires scipy, and is refused without it.** `scripts/fit_usl.py`
refines its OLS fit with non-linear least squares only when scipy imports, and
names which estimator ran. Section 5.10 read H3 with `min_ssr(ols, nlls)`,
under which both paced curves pin `alpha` at 1 with a zero-width interval.
Under the OLS fallback the same two curves return usable intervals,
[0.9578, 1.0] and [0.9669, 1.0], that contain the predicted 1.0. An evaluator
that took whichever estimator imported would read H3 as `PASS` on a host
without scipy and `NOT_EVALUABLE` on one with it, so this one records the
refusal and exits non-zero instead of choosing.

It reads committed artifacts and times nothing, so it is not a harness.

    concurrent_verdicts.py                 # section 5.10's two runs
    concurrent_verdicts.py RUN1 RUN2 [--json]
    concurrent_verdicts.py --self-test
"""
from __future__ import annotations

import argparse
import json
import statistics
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO_ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPO_ROOT / "scripts"))
sys.path.insert(0, str(HERE))

import fit_usl  # noqa: E402
import rocksdb_locate_bound as rlb  # noqa: E402
from concurrent_read_scaling import paced_rate_report  # noqa: E402

RESULTS = HERE.parent / "results"
AMENDED_RUNS = (RESULTS / "baseline_concurrent_reads_amended_h1.json",
                RESULTS / "baseline_concurrent_reads_amended_h1_run2.json")

# Fixed by METHODOLOGY section 5.9 ("The fresh rounds, fixed here").
COMMIT = "0ed8f5e5"
PIN = "0,2,4,6,8,10,12,14"
PIN_SOURCE = "EXPANSE_BENCH_PIN_APPLIED"
MODES = ("free", "idle", "paced")
READERS = (1, 2, 4, 7)
ROUNDS = 5
WINDOW_S = 2.0
OFFERED = 250000.0
GATE_R = 7
# Fixed by section 5.3.
H1_THRESHOLD = 2.0
H2_THRESHOLD = 3.5
# The estimator section 5.10's H3 reading used; see the module docstring.
H3_ESTIMATOR = "min_ssr(ols, nlls)"
H3_RESAMPLES = 2000
H3_SEED = 42


class NotASection59Run(RuntimeError):
    """The artifact is not a run section 5.9 fixed, so no verdict is read from it."""


class EstimatorUnavailable(RuntimeError):
    """H3 cannot be read: the fit did not use the estimator section 5.10 read it with."""


def run_problems(art: dict) -> list[str]:
    """Every way `art` departs from the run section 5.9 fixed; empty when it does not."""
    p = art.get("provenance", {})
    out = []
    commit = str(p.get("commit", ""))
    if not commit.startswith(COMMIT):
        out.append(f"provenance.commit is {commit!r}; section 5.9 fixes {COMMIT}")
    if p.get("core_pin") != PIN:
        out.append(f"provenance.core_pin is {p.get('core_pin')!r}; section 5.9 fixes {PIN}")
    source = (p.get("host") or {}).get("scaling_governor_pin_source")
    if source != PIN_SOURCE:
        out.append(f"the pin's source is {source!r}, not {PIN_SOURCE}")
    settings = art.get("settings", {})
    if settings.get("window_seconds") != WINDOW_S:
        out.append(f"settings.window_seconds is {settings.get('window_seconds')!r}; "
                   f"section 5.9 fixes {WINDOW_S}")
    if settings.get("paced_rate_ops_per_s") != OFFERED:
        out.append(f"settings.paced_rate_ops_per_s is {settings.get('paced_rate_ops_per_s')!r}; "
                   f"section 5.9 fixes {OFFERED}")
    seen: dict[tuple[str, int], list[int]] = {}
    for c in art.get("cells", []):
        seen.setdefault((c["writer_mode"], c["readers"]), []).append(c["round"])
    want = {(m, r) for m in MODES for r in READERS}
    if set(seen) != want:
        out.append(f"cells cover {sorted(seen)}; section 5.9 fixes {sorted(want)}")
    for (mode, readers), rounds in sorted(seen.items()):
        if sorted(rounds) != list(range(ROUNDS)):
            out.append(f"{mode} R={readers} carries rounds {sorted(rounds)}; "
                       f"section 5.9 fixes 0..{ROUNDS - 1}")
    return out


def three_way(ci, threshold: float) -> str:
    """Section 5.3: PASS on the upper bound below, REFUTED on the lower bound above."""
    lo, hi = ci
    if hi < threshold:
        return "PASS"
    if lo > threshold:
        return "REFUTED"
    return "BOUNDARY_RESULT"


def combine(a, b):
    """Section 5.9: a verdict is claimed only where both runs return it."""
    return a if a == b else "BOUNDARY_RESULT"


def h1_gateable(cells: list[dict], offered: float = OFFERED) -> tuple[bool, list[dict]]:
    """Whether H1's gate cell may be gated: section 5.9's rule, on the `R = 7` cell only."""
    flags = [f for f in paced_rate_report(cells, offered)["flags"] if f["readers"] == GATE_R]
    return (not flags, flags)


def h3_estimator() -> str:
    """The estimator `fit_usl` runs on this host, read from a fit rather than assumed."""
    return fit_usl.fit_usl([1.0, 2.0, 4.0, 7.0], [1.0, 1.8, 3.0, 4.0])["estimator"]


def require_h3_estimator(estimator) -> None:
    """Raises unless `estimator` is the one section 5.10 read H3 with."""
    if estimator != H3_ESTIMATOR:
        raise EstimatorUnavailable(
            f"fit_usl ran {estimator!r}, and section 5.10 read H3 with {H3_ESTIMATOR!r}. "
            f"On this arm's paced curves the two disagree about whether the alpha interval "
            f"is usable, so H3 is not read here. Install scipy and re-run.")


def h3_reading(fit: dict, predicted: float) -> str:
    require_h3_estimator(fit.get("estimator"))
    a = fit["alpha_ci"]
    if not a["usable"]:
        return "NOT_EVALUABLE"
    return "PASS" if a["ci_lower"] <= predicted <= a["ci_upper"] else "MISS"


def paced_curve(cells: list[dict]) -> list[list[float]]:
    return [[c["read_mops"] for c in sorted(
        (c for c in cells if c["writer_mode"] == "paced" and c["readers"] == r),
        key=lambda c: c["round"])] for r in READERS]


def evaluate_run(art: dict) -> dict:
    problems = run_problems(art)
    if problems:
        raise NotASection59Run("; ".join(problems))
    cells, scaling = art["cells"], art["scaling"]
    gate = paced_rate_report(cells, OFFERED)["per_readers"][str(GATE_R)]
    gateable, gate_flags = h1_gateable(cells)
    duty = statistics.mean(c["writer_duty_cycle"] for c in cells
                           if c["writer_mode"] == "paced" and c["readers"] == GATE_R)
    paced_s7, idle_s7 = scaling["paced"][f"S({GATE_R})"], scaling["idle"][f"S({GATE_R})"]
    out = {
        "paced_S7": {"point": paced_s7["point"], "ci": list(paced_s7["ci"])},
        "idle_S7": {"point": idle_s7["point"], "ci": list(idle_s7["ci"])},
        "h1_gate_cell_rate_range": [gate["achieved_min_ops_per_s"], gate["achieved_max_ops_per_s"]],
        "h1_gate_cell_flags": gate_flags,
        "h1_gate_cell_mean_duty": duty,
        "h1_boundary_locked_fraction": rlb.gate_boundary_locked_fraction(H1_THRESHOLD, duty),
        "below_gate_cell_max_rate_deviation": max(
            abs(c["write_ops"] / c["elapsed_s"] - OFFERED) for c in cells
            if c["writer_mode"] == "paced" and c["readers"] < GATE_R),
        "H1": three_way(paced_s7["ci"], H1_THRESHOLD) if gateable else "NOT_GATED",
        "H2": three_way(idle_s7["ci"], H2_THRESHOLD),
        "H3": None,
    }
    predicted = rlb.predicted_alpha_from_scaling(paced_s7["point"], GATE_R)
    out["h3_predicted_alpha"] = predicted
    try:
        # Before the fit, not only in h3_reading: a fit that raises would
        # otherwise read NOT_EVALUABLE on a host that cannot run the estimator.
        estimator = h3_estimator()
        require_h3_estimator(estimator)
        try:
            fit = fit_usl.fit_usl_with_bootstrap(list(READERS), paced_curve(cells),
                                                 num_resamples=H3_RESAMPLES, seed=H3_SEED)
        except ValueError as exc:
            out["h3_fit"] = {"estimator": estimator, "raised": str(exc)}
            out["H3"] = "NOT_EVALUABLE"
            return out
        a = fit["alpha_ci"]
        out["h3_fit"] = {"estimator": fit.get("estimator"), "alpha": fit["alpha"],
                         "usable": a["usable"], "problems": a["problems"],
                         "method": a.get("method"), "ci": [a.get("ci_lower"), a.get("ci_upper")]}
        out["H3"] = h3_reading(fit, predicted)
    except EstimatorUnavailable as exc:
        out["h3_error"] = str(exc)
    return out


def verdicts(runs: list[dict]) -> dict:
    r1, r2 = runs
    h3 = None if r1["H3"] is None or r2["H3"] is None else (
        r1["H3"] if r1["H3"] == r2["H3"] else "RUNS_DIFFER")
    return {"H1": combine(r1["H1"], r2["H1"]), "H2": combine(r1["H2"], r2["H2"]), "H3": h3}


def render(runs: list[dict], combined: dict) -> str:
    def s7(c):
        return f"{c['point']:.3f} [{c['ci'][0]:.3f}, {c['ci'][1]:.3f}]"

    def h3cell(r):
        if r.get("h3_error"):
            return "not read (estimator)"
        problems = (r.get("h3_fit") or {}).get("problems")
        return f"{r['H3']}" + (f" ({', '.join(problems)})" if problems else "")

    lines = ["| hypothesis | run 1 | run 2 | verdict |", "|---|---|---|---|",
             f"| H1 paced S(7) CI upper < {H1_THRESHOLD} | {s7(runs[0]['paced_S7'])} {runs[0]['H1']} "
             f"| {s7(runs[1]['paced_S7'])} {runs[1]['H1']} | {combined['H1']} |",
             f"| H2 idle S(7) CI upper < {H2_THRESHOLD} | {s7(runs[0]['idle_S7'])} {runs[0]['H2']} "
             f"| {s7(runs[1]['idle_S7'])} {runs[1]['H2']} | {combined['H2']} |",
             f"| H3 usable alpha interval contains the prediction | {h3cell(runs[0])} "
             f"| {h3cell(runs[1])} | {combined['H3'] or 'not read'} |", ""]
    for i, r in enumerate(runs, 1):
        lo, hi = r["h1_gate_cell_rate_range"]
        lines.append(f"run {i}: paced R={GATE_R} writer {lo:,.0f}-{hi:,.0f} inserts/s "
                     f"(offered {OFFERED:,.0f}; flagged {len(r['h1_gate_cell_flags'])}), "
                     f"R<{GATE_R} within {r['below_gate_cell_max_rate_deviation']:,.0f}; "
                     f"mean duty {r['h1_gate_cell_mean_duty']:.2%}, H1 boundary at locked "
                     f"fraction {r['h1_boundary_locked_fraction']:.4f}")
    return "\n".join(lines)


def self_test() -> int:
    fails: list[str] = []

    def check(name, got, want):
        if got != want:
            fails.append(f"{name}: got {got!r}, want {want!r}")

    # --- section 5.3's three-way reading, and section 5.9's both-runs rule ---
    check("upper bound below", three_way([1.9, 1.99], 2.0), "PASS")
    check("lower bound above", three_way([2.01, 2.2], 2.0), "REFUTED")
    check("straddling", three_way([1.9, 2.1], 2.0), "BOUNDARY_RESULT")
    check("an upper bound AT the threshold is not below it", three_way([1.9, 2.0], 2.0),
          "BOUNDARY_RESULT")
    check("both runs PASS", combine("PASS", "PASS"), "PASS")
    check("one run not gated", combine("PASS", "NOT_GATED"), "BOUNDARY_RESULT")
    check("neither run gated", combine("NOT_GATED", "NOT_GATED"), "NOT_GATED")

    # --- H1's gateability is the R = 7 cell's, and only its ---
    base = [{"writer_mode": "paced", "readers": r, "round": 0,
             "write_ops": 280000 if r == 7 else 499000, "elapsed_s": 2.0,
             "writer_exhausted": 0, "cell": f"p{r}"} for r in READERS]
    check("a short R=7 writer is gated (section 5.9)", h1_gateable(base)[0], True)
    check("an above-offered R=4 cell does not ungate H1",
          h1_gateable([dict(c, write_ops=500010) if c["readers"] == 4 else c for c in base])[0], True)
    check("an above-offered R=7 cell is not gated",
          h1_gateable([dict(c, write_ops=500010) if c["readers"] == 7 else c for c in base])[0], False)
    check("an exhausted R=7 writer is not gated",
          h1_gateable([dict(c, writer_exhausted=1) if c["readers"] == 7 else c for c in base])[0], False)

    # --- H3's reading, and its refusal of any other estimator ---
    usable = {"estimator": H3_ESTIMATOR, "alpha": 0.99,
              "alpha_ci": {"usable": True, "ci_lower": 0.95, "ci_upper": 1.0}}
    check("usable interval containing the prediction", h3_reading(usable, 1.0), "PASS")
    check("usable interval excluding it", h3_reading(usable, 0.5), "MISS")
    check("unusable interval", h3_reading({"estimator": H3_ESTIMATOR,
                                           "alpha_ci": {"usable": False}}, 1.0), "NOT_EVALUABLE")
    try:
        h3_reading(dict(usable, estimator="ols (nlls requested; scipy not importable)"), 1.0)
    except EstimatorUnavailable:
        pass
    else:
        fails.append("an OLS fit must not be read for H3")

    # --- a run section 5.9 did not fix yields no verdict ---
    runs = [json.loads(p.read_text()) for p in AMENDED_RUNS]
    for i, art in enumerate(runs, 1):
        if run_problems(art):
            fails.append(f"section 5.10 run {i} must be a section 5.9 run: {run_problems(art)}")
    auto_pin = json.loads((RESULTS / "baseline_concurrent_reads.json").read_text())
    if not any("core_pin" in x for x in run_problems(auto_pin)):
        fails.append(f"section 5.7's auto-pin run must be refused by its pin: "
                     f"{run_problems(auto_pin)}")
    try:
        evaluate_run(auto_pin)
    except NotASection59Run:
        pass
    else:
        fails.append("evaluate_run read a verdict from section 5.7's run instead of refusing it")
    short = json.loads(json.dumps(runs[0]))
    short["cells"] = [c for c in short["cells"] if not (c["writer_mode"] == "idle" and c["round"] == 4)]
    if not any("rounds" in x for x in run_problems(short)):
        fails.append("a run missing a round must be refused")

    # --- the estimator check at the call site ---
    # Force the host's fit to report the OLS fallback. evaluate_run must record
    # that H3 was not read, and still read H1 and H2.
    real_fit = fit_usl.fit_usl

    def ols_only(*args, **kwargs):
        out = real_fit(*args, **kwargs)
        out["estimator"] = "ols (nlls requested; scipy not importable)"
        return out

    fit_usl.fit_usl = ols_only
    try:
        forced = evaluate_run(runs[0])
    finally:
        fit_usl.fit_usl = real_fit
    if forced["H3"] is not None or "h3_error" not in forced:
        fails.append(f"with the OLS fallback, H3 must be refused, got {forced['H3']!r}")
    check("H1 is still read when H3 is refused", forced["H1"], "PASS")
    # A fit that raises is NOT_EVALUABLE under section 5.9 -- but only once the
    # estimator is the right one. Under the OLS fallback it is still a refusal,
    # or a host without scipy would turn every raising fit into a verdict.
    real_boot = fit_usl.fit_usl_with_bootstrap

    def raises(*args, **kwargs):
        raise ValueError("Too many inadmissible bootstrap fits during resampling")

    def nlls_label(*args, **kwargs):
        out = real_fit(*args, **kwargs)
        out["estimator"] = H3_ESTIMATOR
        return out

    for label, fit_fn, want_error in (("OLS", ols_only, True), ("NLLS", nlls_label, False)):
        fit_usl.fit_usl, fit_usl.fit_usl_with_bootstrap = fit_fn, raises
        try:
            r = evaluate_run(runs[0])
        finally:
            fit_usl.fit_usl, fit_usl.fit_usl_with_bootstrap = real_fit, real_boot
        if want_error and (r["H3"] is not None or "h3_error" not in r):
            fails.append(f"a raising fit under the {label} estimator must be refused, "
                         f"got H3={r['H3']!r}")
        if not want_error:
            check(f"a raising fit under the {label} estimator", r["H3"], "NOT_EVALUABLE")

    # --- section 5.10, reproduced from the committed artifacts ---
    got = [evaluate_run(a) for a in runs]
    combined = verdicts(got)
    check("section 5.10 H1", (got[0]["H1"], got[1]["H1"], combined["H1"]), ("PASS", "PASS", "PASS"))
    check("section 5.10 H2", (got[0]["H2"], got[1]["H2"], combined["H2"]), ("PASS", "PASS", "PASS"))
    check("section 5.10 paced S(7)",
          [[round(g["paced_S7"]["point"], 3)] + [round(x, 3) for x in g["paced_S7"]["ci"]] for g in got],
          [[0.624, 0.618, 0.630], [0.622, 0.618, 0.628]])
    check("section 5.10 idle S(7)",
          [[round(g["idle_S7"]["point"], 3)] + [round(x, 3) for x in g["idle_S7"]["ci"]] for g in got],
          [[0.621, 0.611, 0.631], [0.609, 0.599, 0.614]])
    check("section 5.10 R=7 writer rate ranges",
          [[round(x) for x in g["h1_gate_cell_rate_range"]] for g in got],
          [[139698, 145363], [139403, 143861]])
    check("section 5.10: every R<=4 cell within 221 inserts/s",
          round(max(g["below_gate_cell_max_rate_deviation"] for g in got)), 221)
    check("section 5.10 H1 boundaries",
          [round(g["h1_boundary_locked_fraction"], 4) for g in got], [0.484, 0.4841])
    check("section 5.10 predicted alpha", [g["h3_predicted_alpha"] for g in got], [1.0, 1.0])
    if h3_estimator() == H3_ESTIMATOR:
        check("section 5.10 H3", (got[0]["H3"], got[1]["H3"], combined["H3"]),
              ("NOT_EVALUABLE", "NOT_EVALUABLE", "NOT_EVALUABLE"))
        for i, g in enumerate(got, 1):
            if "zero_width" not in (g.get("h3_fit") or {}).get("problems", []):
                fails.append(f"section 5.10 run {i}: the alpha interval should be zero_width, "
                             f"got {g.get('h3_fit')}")
    else:
        if any("h3_error" not in g for g in got):
            fails.append("without scipy, H3 must be refused on section 5.10's runs")
        print(f"::notice::concurrent_verdicts.py --self-test: section 5.10's H3 reading was not "
              f"reproduced, because this host's fit_usl runs {h3_estimator()!r} rather than "
              f"{H3_ESTIMATOR!r} (scipy is not importable). The refusal was checked instead.")

    if fails:
        print("concurrent_verdicts.py --self-test: FAILED")
        for f in fails:
            print(f"  - {f}")
        return 1
    print("concurrent_verdicts.py --self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("runs", nargs="*", type=Path,
                    help="two artifacts, run 1 then run 2 (default: section 5.10's)")
    ap.add_argument("--json", action="store_true", help="print the readings as JSON")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    paths = args.runs or list(AMENDED_RUNS)
    if len(paths) != 2:
        print(f"::error::expected two artifacts, got {len(paths)}", file=sys.stderr)
        return 2
    try:
        runs = [evaluate_run(json.loads(p.read_text())) for p in paths]
    except NotASection59Run as exc:
        print(f"::error::not a run METHODOLOGY section 5.9 fixed, so no verdict is read: {exc}",
              file=sys.stderr)
        return 1
    combined = verdicts(runs)
    if args.json:
        print(json.dumps({"runs": runs, "verdicts": combined}, indent=2))
    else:
        print(render(runs, combined))
    refused = [r["h3_error"] for r in runs if r.get("h3_error")]
    if refused:
        print(f"::error::H3 was not read: {refused[0]}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
