#!/usr/bin/env python3
"""
Generates docs/benchmarks/dns_names/README.md from the committed
results/baseline_*.json artifacts of the suite's two dispatches.

Every number, verdict and label is derived from the artifacts (AGENTS.md
section 8.2): the verdicts apply the rules pre-registered in METHODOLOGY.md
sections 4 to 6 as amended by A1, and nothing below is a stamped constant
about a result. With no artifact, or with one dispatch only, the page says so
and every timing verdict that needs two dispatches reads INTERMEDIATE.

    python3 docs/benchmarks/dns_names/scripts/generate_readme.py [--check]

`--check` exits non-zero if README.md differs from what the artifacts produce.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

BASE = Path(__file__).resolve().parent.parent
RESULTS = BASE / "results"
README_PATH = BASE / "README.md"
sys.path.insert(0, str(Path(__file__).resolve().parent))
import run_all  # noqa: E402  (the runner's own pairing: per-round ratio, BCa on the logs)
ISSUE = "[#1800](https://github.com/orieg/expanse/issues/1800)"

RUNS = ("", "_run2")
EVAL_N = (100_000, 1_000_000)
ORDERS = ("sorted", "shuffled")
FILES = {"dns_get": "get", "dns_predecessor": "predecessor",
         "dns_closest_encloser": "closest_encloser",
         "dns_predecessor_concurrent": "concurrent_readers", "dns_memory": "memory"}


#: A pair with no prediction that the runner does not publish: the control arm
#: against `qp_trie`. Formed here from the cells' own `rounds_raw`, by the
#: runner's pairing, so the escape pass can be separated from the engine.
DERIVED_PAIR = ("expanse_direct", "qp_trie")
DERIVED_NAME = "expanse_direct_over_qp_trie"


def load(stem: str, results: Path) -> list[dict | None]:
    """The artifact of each dispatch, `None` where it is not committed."""
    out = []
    for suffix in RUNS:
        path = results / f"baseline_{stem}{suffix}.json"
        art = json.loads(path.read_text(encoding="utf-8")) if path.is_file() else None
        for cell in (art or {}).get("cells", []):
            arms = {r["arm"] for r in cell["rounds_raw"]}
            if set(DERIVED_PAIR) <= arms:
                num, den = (run_all.by_round(cell["rounds_raw"], a, "ns_per_op")
                            for a in DERIVED_PAIR)
                cell["ratios"][DERIVED_NAME] = run_all.paired(num, den, DERIVED_NAME, True)
        out.append(art)
    return out


def n_fmt(n: int) -> str:
    return f"{n:,}"


def iv_fmt(iv: dict | None) -> str:
    if iv is None:
        return "not run"
    if iv.get("ci_lower") is None:
        return f"{iv['point']:.3f} (no interval)"
    tag = "" if iv["ci_method"] == "bca" else f" ({iv['ci_method']})"
    return f"{iv['point']:.3f} [{iv['ci_lower']:.3f}, {iv['ci_upper']:.3f}]{tag}"


def side(iv: dict, threshold: float = 1.0) -> str:
    """Where an interval lies against `threshold`: `below`, `above` or `spans`."""
    if iv.get("ci_lower") is None:
        return "spans"
    if iv["ci_upper"] < threshold:
        return "below"
    if iv["ci_lower"] > threshold:
        return "above"
    return "spans"


def cell_verdict(ivs: list[dict | None], predicted: str, threshold: float = 1.0) -> str:
    """METHODOLOGY section 4's label for one cell from its two dispatches."""
    if any(iv is None for iv in ivs):
        return "INTERMEDIATE"
    sides = [side(iv, threshold) for iv in ivs]
    if "spans" in sides:
        return "BOUNDARY_RESULT"
    if all(s == predicted for s in sides):
        return "PASS"
    if all(s != predicted for s in sides):
        return "REFUTED"
    return "INTERMEDIATE"


def unpredicted_label(ivs: list[dict | None]) -> str:
    """A cell with no prediction: a loss only when wholly above 1.0 in every dispatch run."""
    have = [iv for iv in ivs if iv is not None]
    sides = {side(iv) for iv in have}
    if sides == {"above"}:
        return "UNPREDICTED_LOSS" if len(have) == len(ivs) else "above 1.0 (one dispatch)"
    if sides == {"below"}:
        return "below 1.0" if len(have) == len(ivs) else "below 1.0 (one dispatch)"
    return "spans 1.0 or the dispatches differ"


def overall(verdicts: list[str]) -> str:
    """PASS only if every cell is (section 4); otherwise the labels present, worst first."""
    kinds = set(verdicts)
    if kinds == {"PASS"}:
        return "PASS"
    for label in ("REFUTED", "INTERMEDIATE", "BOUNDARY_RESULT"):
        if kinds == {label}:
            return label
    return "not PASS"


def breakdown(verdicts: list[str]) -> str:
    return ", ".join(f"{v} {verdicts.count(v)}" for v in sorted(set(verdicts)))


def cells_of(art: dict | None) -> dict:
    if art is None:
        return {}
    return {(c["population"], c["order"], c["probe_class"]): c for c in art["cells"]}


def keys_of(arts: list[dict | None]) -> list[tuple]:
    seen: dict[tuple, None] = {}
    for a in arts:
        for k in cells_of(a):
            seen.setdefault(k)
    return list(seen)


def ratio(arts: list[dict | None], key: tuple, name: str) -> list[dict | None]:
    out = []
    for a in arts:
        cell = cells_of(a).get(key)
        out.append(cell["ratios"].get(name) if cell else None)
    return out


def timing_prediction(arts, name: str, predicted: str, pops=EVAL_N) -> tuple[list[str], list[str]]:
    """Per-cell verdicts of one timing prediction, and one line per non-PASS cell."""
    verdicts, notes = [], []
    for key in keys_of(arts):
        if key[0] not in pops:
            continue
        ivs = ratio(arts, key, name)
        v = cell_verdict(ivs, predicted)
        verdicts.append(v)
        if v != "PASS":
            notes.append(f"`{name}` n = {n_fmt(key[0])}, {key[1]}, `{key[2]}`: {v} "
                         f"({'; '.join(iv_fmt(iv) for iv in ivs)})")
    return verdicts, notes


def memory_rows(art: dict | None) -> dict:
    if art is None:
        return {}
    return {(r["population"], r["order"], r["arm"]): r for r in art["memory"]}


def exact_prediction(arts, check) -> list[str]:
    """PASS / REFUTED per cell and dispatch for an exact prediction; `check(rows, derived)`
    yields `(holds, ...)` per cell."""
    verdicts = []
    present = [a for a in arts if a is not None]
    if not present:
        return []
    per_cell: dict = {}
    for a in present:
        derived = {d["population"]: d for d in a.get("derived", {}).get("bounds", [])}
        for cell, holds in check(memory_rows(a), derived):
            per_cell.setdefault(cell, []).append(holds)
    for holds in per_cell.values():
        if len(holds) < len(arts):
            verdicts.append("INTERMEDIATE")
        else:
            verdicts.append("PASS" if all(holds) else ("REFUTED" if not any(holds) else "INTERMEDIATE"))
    return verdicts


def check_e1(rows, derived):
    for n in EVAL_N:
        for order in ORDERS:
            for arm in ("expanse_direct", "expanse_ordered"):
                r, d = rows.get((n, order, arm)), derived.get(n, {}).get(arm)
                if r is None or d is None:
                    continue
                yield (n, order, arm), (r["str_nodes"] == d["shells"]
                                         and r["suffix_leaves"] == d["suffixes"]
                                         and r["mem_used"] >= d["mem_used_lower_bound"])


def check_bytes(a: str, b: str, field: str, orders=ORDERS):
    def check(rows, _derived):
        for n in EVAL_N:
            for order in orders:
                x, y = rows.get((n, order, a)), rows.get((n, order, b))
                if x is None or y is None:
                    continue
                yield (n, order), x[field] < y[field]
    return check


def scaling(arts, arm: str, readers: int) -> list[dict | None]:
    out = []
    for a in arts:
        cell = None if a is None else next(
            (c for c in a["throughput"] if c["arm"] == arm and c["readers"] == readers), None)
        out.append(cell["scaling"] if cell else None)
    return out


def provenance(arts_by_stem: dict) -> list[str]:
    lines = []
    for i, suffix in enumerate(RUNS):
        arts = [a[i] for a in arts_by_stem.values() if a[i] is not None]
        label = f"Dispatch {i + 1}"
        if not arts:
            lines.append(f"- {label}: not committed.")
            continue
        p = arts[0]["provenance"]
        windows = [w["load"] for a in arts for w in a["provenance"].get("windows", [])]
        foreign = [w["foreign_busy_cpus"] for w in windows
                   if isinstance(w.get("foreign_busy_cpus"), (int, float))]
        host = p.get("host_description") or p["host"].get("cpu_model") or "host not described"
        load = (f"{len(windows)} load windows, one per harness process; largest busy-CPU figure "
                f"from outside the run {max(foreign):.2f} core-equivalents"
                if foreign else "no load window was measured")
        commits = sorted({a["provenance"]["commit"] for a in arts})
        lines.append(f"- {label} (`baseline_*{suffix}.json`): {host}; core pin `{p.get('core_pin')}`; "
                     f"governor `{p['host'].get('scaling_governor')}`; commit "
                     f"{', '.join(f'`{c}`' for c in commits)}; run {p.get('run_id') or 'not recorded'}; "
                     f"{load}.")
    return lines


def measured_tag(arts: list[dict | None]) -> str:
    """The provenance tag of a table: host and commit of the dispatches it draws on."""
    have = [a for a in arts if a is not None]
    if not have:
        return "(unmeasured)"
    p = have[0]["provenance"]
    host = p.get("host_description") or p["host"].get("cpu_model") or "host not described"
    commits = sorted({a["provenance"]["commit"] for a in have})
    return f"(measured: {host}, {', '.join(commits)})"


def timing_table(arts, workload: str, arms: tuple, pairs: tuple) -> list[str]:
    lines = [f"Workload `{workload}` {measured_tag(arts)}. Arm columns are median ns per "
             f"operation in dispatch 1; a ratio is the first arm's time over the second's, "
             f"geometric mean of the per-round ratios with its BCa 95% interval, dispatch 1 "
             f"then dispatch 2. Below 1.0 the first arm is faster.", ""]
    head = ["n", "order", "probes"] + [f"`{a}`" for a in arms] + [f"`{p}`" for p in pairs]
    lines += ["| " + " | ".join(head) + " |", "|" + "---|" * len(head)]
    for key in keys_of(arts):
        cell = cells_of(arts[0]).get(key) or cells_of(arts[1]).get(key)
        row = [n_fmt(key[0]), key[1], f"`{key[2]}`"]
        row += [f"{cell['ns_per_op'][a]:.1f}" for a in arms]
        row += [" · ".join(iv_fmt(iv) for iv in ratio(arts, key, p)) for p in pairs]
        lines.append("| " + " | ".join(row) + " |")
    return lines


def render(results: Path = RESULTS) -> str:
    A = {wid: load(stem, results) for wid, stem in FILES.items()}
    get, pred, enc = A["dns_get"], A["dns_predecessor"], A["dns_closest_encloser"]
    conc, mem = A["dns_predecessor_concurrent"], A["dns_memory"]
    dispatches = sum(1 for i in range(len(RUNS)) if any(a[i] is not None for a in A.values()))

    out = ["# DNS-Shaped Keys: Expanse vs. BTreeMap and QP-Trie", "",
           "<!-- Generated by scripts/generate_readme.py from results/baseline_*.json; "
           "do not edit by hand. -->", "",
           "`ExpanseOrderedBytesMap` against `std::collections::BTreeMap` and "
           "[`qp-trie`](https://github.com/sdleffler/qp-trie-rs) `=0.8.2` on domain names stored "
           "as reversed-label keys: exact lookup, the predecessor lookup behind a "
           "denial-of-existence answer, closest encloser, live heap, and predecessor lookups "
           "from concurrent readers. The suite measures the index alone, on synthetic names. "
           f"Design, arms, pre-registration and claims ceiling: [METHODOLOGY.md](METHODOLOGY.md); "
           f"tracking issue {ISSUE}.", ""]

    out += ["## Provenance", ""]
    if dispatches == 0:
        out += [f"No reference-host run is committed yet; every table below is empty and every "
                f"verdict is pending the two dispatches METHODOLOGY.md §4 requires ({ISSUE}).", ""]
    else:
        out += provenance(A) + [""]
        if dispatches < len(RUNS):
            out += [f"One dispatch of two is committed. A timing verdict needs both "
                    f"(METHODOLOGY.md §4), so each reads INTERMEDIATE until the second lands "
                    f"({ISSUE}).", ""]

    # ---- verdicts -------------------------------------------------------
    rows, notes = [], []

    def add(label: str, verdicts: list[str], more: list[str] = ()):
        if not verdicts:
            rows.append(f"| {label} | 0 | not measured | |")
            return "not measured"
        rows.append(f"| {label} | {len(verdicts)} | {overall(verdicts)} | {breakdown(verdicts)} |")
        notes.extend(more)
        return overall(verdicts)

    V = {}
    V["E1"] = add("E1 node census and `mem_used` bound match `scripts/dns_name_bounds.py`",
                  exact_prediction(mem, check_e1))
    for pid, arts, name, predicted, text, pops in (
        ("G1", get, "expanse_ordered_over_btreemap", "below",
         "G1 `dns_get`: `expanse_ordered` faster than `btreemap`", EVAL_N),
        ("G2", get, "expanse_ordered_over_qp_trie", "below",
         "G2 `dns_get`: `expanse_ordered` faster than `qp_trie`", EVAL_N),
        ("G3", get, "expanse_ordered_over_expanse_direct", "above",
         "G3 `dns_get`: `expanse_ordered` slower than `expanse_direct`", EVAL_N),
        ("N1", pred, "expanse_ordered_over_btreemap", "below",
         "N1 `dns_predecessor`: `expanse_ordered` faster than `btreemap` at n = 1,000,000",
         (1_000_000,)),
    ):
        v, n = timing_prediction(arts, name, predicted, pops)
        V[pid] = add(text, v, n)
    rows.append("| N2 `dns_predecessor` against `qp_trie` | 0 | NOT_APPLICABLE | "
                "`qp-trie` 0.8.2 has no predecessor or range call |")
    for pid, name, text in (
        ("L1", "expanse_ordered_over_btreemap",
         "L1 `dns_closest_encloser`: `expanse_ordered` faster than `btreemap`"),
        ("L2", "expanse_ordered_over_qp_trie",
         "L2 `dns_closest_encloser`: `expanse_ordered` faster than `qp_trie`"),
    ):
        v, n = timing_prediction(enc, name, "below")
        V[pid] = add(text, v, n)
    V["M1"] = add("M1 `dns_memory`: `expanse_ordered` fewer requested bytes than `btreemap`, sorted build",
                  exact_prediction(mem, check_bytes("expanse_ordered", "btreemap",
                                                    "requested_bytes", ("sorted",))))
    V["M2"] = add("M2 `dns_memory`: `expanse_ordered` fewer requested bytes than `qp_trie`, both orders",
                  exact_prediction(mem, check_bytes("expanse_ordered", "qp_trie", "requested_bytes")))
    V["M3"] = add("M3 `dns_memory`: `expanse_direct` has the smaller `mem_used()`, both orders",
                  exact_prediction(mem, check_bytes("expanse_direct", "expanse_ordered", "mem_used")))
    for pid, arm, predicted, threshold, text in (
        ("C1", "sync_locked", "below", 2.0,
         "C1 `sync_locked` does not scale: upper bound of S(8) below 2"),
        ("C2", "clone_per_reader", "above", 4.0,
         "C2 `clone_per_reader` scales: lower bound of S(8) above 4 (A1.1)"),
        ("C3", "sync_get", "above", 4.0, "C3 `sync_get` scales: lower bound of S(8) above 4"),
    ):
        ivs = scaling(conc, arm, 8)
        if all(iv is None for iv in ivs):
            V[pid] = add(text, [])
            continue
        v = cell_verdict(ivs, predicted, threshold)
        V[pid] = add(text, [v], [] if v == "PASS" else
                     [f"S(8) of `{arm}`: {v} ({'; '.join(iv_fmt(iv) for iv in ivs)})"])

    out += ["## Pre-registered verdicts", "",
            "Evaluated at n ∈ {100,000, 1,000,000} in both build orders, under METHODOLOGY.md "
            "§4 and §5 as amended by A1. A timing cell is PASS only when its interval lies on "
            "the predicted side of the threshold in both dispatches. No result here has had "
            "independent review.", "",
            "| Prediction | Cells | Verdict | Breakdown |", "|---|---|---|---|"] + rows + [""]

    # ---- decision -------------------------------------------------------
    out += ["## Decision (METHODOLOGY.md §6)", ""]
    if dispatches < len(RUNS):
        out += [f"Not decided: the rule reads verdicts that need two dispatches ({ISSUE}).", ""]
    else:
        def refuted(arts, name):
            v, _ = timing_prediction(arts, name, "below")
            return "REFUTED" in v
        winning = [pid for pid, blocked in (
            ("G1", refuted(get, "expanse_ordered_over_qp_trie")),
            ("N1", False),
            ("L1", refuted(enc, "expanse_ordered_over_qp_trie")),
        ) if V[pid] == "PASS" and not blocked]
        advantage = V["G1"] == "PASS" and V["N1"] == "PASS"
        out += [("- Winning cell (a PASS on G1, N1 or L1 with no REFUTED cell against `qp_trie` "
                 "on the same operation): "
                 + (", ".join(winning) + "." if winning else
                    "none. By the pre-registered rule this ends the DNS line of work.")),
                ("- A measured advantage to build on (G1 and N1 both PASS): "
                 + ("met." if advantage else
                    f"not met (G1 {V['G1']}, N1 {V['N1']}).")),
                "- Ceiling: these synthetic names, these two twin implementations at these "
                "versions, one host, single-threaded reads apart from the reader-scaling "
                "section. Not a claim about real zones, about B-trees or QP-tries in general, "
                "or about a name server's throughput.", ""]

    # ---- losses ---------------------------------------------------------
    out += ["## Cells that are not PASS, and unpredicted cells", ""]
    if not notes and dispatches == 0:
        out += ["None yet: nothing is measured.", ""]
    else:
        out += [f"- {n}" for n in notes] or ["- Every predicted cell is PASS."]
        loss = []
        for arts, wid, names in (
            (get, "dns_get", (DERIVED_NAME,)),
            (pred, "dns_predecessor", ("expanse_ordered_over_btreemap",
                                       "expanse_ordered_owned_over_btreemap")),
            (enc, "dns_closest_encloser", (DERIVED_NAME,)),
        ):
            for key in keys_of(arts):
                for name in names:
                    if (wid == "dns_predecessor" and name == "expanse_ordered_over_btreemap"
                            and key[0] == 1_000_000):
                        continue  # N1's cells, judged above
                    if key[0] not in EVAL_N:
                        continue
                    ivs = ratio(arts, key, name)
                    loss.append(f"`{wid}` `{name}` n = {n_fmt(key[0])}, {key[1]}, `{key[2]}` "
                                f"(no prediction): {unpredicted_label(ivs)} "
                                f"({'; '.join(iv_fmt(iv) for iv in ivs)})")
        out += [f"- {n}" for n in loss]
        out += ["", "Cells at n = 1,000 and 10,000 are `NOT_PREREGISTERED`; they are in the "
                "tables below and carry no verdict.", ""]

    # ---- tables ---------------------------------------------------------
    if dispatches:
        out += ["## Exact lookup, 50% hit", ""]
        out += timing_table(get, "dns_get",
                            ("expanse_ordered", "expanse_direct", "btreemap", "qp_trie"),
                            ("expanse_ordered_over_btreemap", "expanse_ordered_over_qp_trie",
                             "expanse_ordered_over_expanse_direct", DERIVED_NAME)) + [""]
        out += ["## Predecessor of an absent name", ""]
        out += timing_table(pred, "dns_predecessor",
                            ("expanse_ordered", "expanse_ordered_owned", "expanse_direct",
                             "btreemap"),
                            ("expanse_ordered_over_btreemap",
                             "expanse_ordered_owned_over_btreemap",
                             "expanse_ordered_over_expanse_direct")) + [""]
        out += ["## Closest encloser", "",
                "One operation is a whole probe: k + 1 exact lookups, k uniform in 1..=3.", ""]
        out += timing_table(enc, "dns_closest_encloser",
                            ("expanse_ordered", "expanse_direct", "btreemap", "qp_trie"),
                            ("expanse_ordered_over_btreemap", "expanse_ordered_over_qp_trie",
                             "expanse_ordered_over_expanse_direct", DERIVED_NAME)) + [""]

        m = mem[0] or mem[1]
        if m is not None:
            mr = memory_rows(m)
            same = (mem[0] is not None and mem[1] is not None
                    and mem[0]["memory"] == mem[1]["memory"])
            out += ["## Live heap after the build", "",
                    f"Workload `dns_memory` {measured_tag(mem)}. Requested bytes per name of "
                    f"everything the build left live (exact counts, no interval), with "
                    f"`mem_used()` per name beside it for the Expanse arms. "
                    + ("The two dispatches' counts are identical." if same else
                       "One dispatch is committed." if None in mem else
                       "THE TWO DISPATCHES' COUNTS DIFFER; the table shows dispatch 1."), "",
                    "| n | order | `expanse_ordered` | its `mem_used` | `expanse_direct` | "
                    "its `mem_used` | `btreemap` | `qp_trie` |", "|---|---|---|---|---|---|---|---|"]
            for n in sorted({k[0] for k in mr}):
                for order in ORDERS:
                    def b(arm, field="requested_bytes"):
                        return f"{mr[(n, order, arm)][field] / n:.2f}"
                    out.append(f"| {n_fmt(n)} | {order} | {b('expanse_ordered')} | "
                               f"{b('expanse_ordered', 'mem_used')} | {b('expanse_direct')} | "
                               f"{b('expanse_direct', 'mem_used')} | {b('btreemap')} | {b('qp_trie')} |")
            out.append("")
            bounds = m.get("derived", {}).get("bounds", [])
            if bounds:
                out += ["Derived by `scripts/dns_name_bounds.py` for the same names (E1), per name:", "",
                        "| n | `expanse_ordered` lower bound on `mem_used` | `expanse_direct` lower bound | "
                        "`BTreeMap` bounds |", "|---|---|---|---|"]
                for d in bounds:
                    n = d["population"]
                    lo, hi = d["btreemap_bytes_bounds"]
                    out.append(f"| {n_fmt(n)} | {d['expanse_ordered']['mem_used_lower_bound'] / n:.2f} | "
                               f"{d['expanse_direct']['mem_used_lower_bound'] / n:.2f} | "
                               f"{lo / n:.1f} to {hi / n:.1f} |")
                out.append("")

        c = conc[0] or conc[1]
        if c is not None:
            pin = c["provenance"].get("core_pin")
            out += ["## Predecessor from concurrent readers", "",
                    f"Workload `dns_predecessor_concurrent` {measured_tag(conc)}, core pin `{pin}`, "
                    f"n = {n_fmt(c['throughput'][0]['population'])}, sorted build, no writer. "
                    f"S(W) is throughput at W readers over throughput at one reader, paired per "
                    f"round, with its BCa 95% interval, dispatch 1 then dispatch 2. These cells "
                    f"are comparable with each other and with nothing taken under another pin "
                    f"(METHODOLOGY.md §5). `clone_per_reader` holds one copy of the map per "
                    f"reader (A1.1).", "",
                    "| arm | W | Mops/s (dispatch 1) | S(W) |", "|---|---|---|---|"]
            for cell in c["throughput"]:
                ivs = scaling(conc, cell["arm"], cell["readers"])
                out.append(f"| `{cell['arm']}` | {cell['readers']} | {cell['ops_per_s'] / 1e6:.3f} | "
                           f"{' · '.join(iv_fmt(iv) for iv in ivs)} |")
            ratios = [None if a is None else a["clone_per_reader_over_sync_locked"].get("8")
                      for a in conc]
            out += ["", f"Throughput of `clone_per_reader` over `sync_locked` at W = 8, same "
                    f"workload and pin: {' · '.join(iv_fmt(iv) for iv in ratios)}. "
                    f"METHODOLOGY.md §5 reopens an optimistic predecessor only if the lower bound "
                    f"is at least 2 and a caller cannot use snapshots; the suite measures the "
                    f"first condition only.", ""]
            if V["C3"] != "PASS":
                out += [f"C3, the control, is {V['C3']}: the one-shot `get` does not scale under this "
                        f"pin. By METHODOLOGY.md §5, C1 therefore says nothing about the lock. "
                        f"`sync_reader_get` (A1.2) makes the same lookups through a reader each "
                        f"thread registers once; its S(W) rows above show what the per-call "
                        f"registration costs. It carries no prediction.", ""]

    out += ["## Reproduce", "",
            "```bash", "docs/benchmarks/dns_names/run.sh          # full run, on the Linux reference host",
            "docs/benchmarks/dns_names/run.sh --quick  # smoke run into results/quick/ (gitignored)",
            "python3 docs/benchmarks/dns_names/scripts/generate_readme.py", "```", "",
            "On a pull request, `/benchmark dns_names` runs the same runner on the bare-metal "
            "host. The harnesses are `crates/expanse/benches/dns_*.rs`; each carries its "
            "`# Workload shape` table.", ""]
    return "\n".join(out)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true",
                    help="exit non-zero if README.md differs from what the artifacts produce")
    ap.add_argument("--results", type=Path, default=RESULTS,
                    help="read artifacts from this directory and print the page (never written)")
    args = ap.parse_args(argv)
    text = render(args.results)
    if args.results != RESULTS:
        print(text)
        return 0
    if args.check:
        if not README_PATH.is_file() or README_PATH.read_text(encoding="utf-8") != text:
            print("docs/benchmarks/dns_names/README.md is out of date: run "
                  "python3 docs/benchmarks/dns_names/scripts/generate_readme.py", file=sys.stderr)
            return 1
        print("dns_names README.md matches its artifacts")
        return 0
    README_PATH.write_text(text, encoding="utf-8")
    print(f"wrote {README_PATH}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
