#!/usr/bin/env python3
"""README §7.1, §7.2 and §7.6 for the concurrent HOT-ROWEX arm (#692), derived from results/.

Every table row in those three sections, and every figure in the prose around
them that the two committed runs determine, is computed here from
``results/baseline_concurrent.json`` (run 1) and
``results/baseline_concurrent_run2.json`` (run 2) — AGENTS.md §8.2: nothing in a
generated table is typed by hand. §7.3 comes from ``integer_tables.py``.

    python3 docs/benchmarks/hot_comparison/scripts/concurrent_tables.py [--quick]
    python3 docs/benchmarks/hot_comparison/scripts/concurrent_tables.py --self-test

``scripts/check_readme_tables.py`` runs this and requires every row it prints to
appear verbatim in the README, and every prose line it prints to appear with
whitespace collapsed (blockquote markers included), so a re-measurement that
changes a digit fails the gate until the README carries it. ``--write`` there
re-splices the table rows; prose is corrected by hand.

## Prose is rendered for the outcome it describes, and refuses any other

A sentence such as "the set arm is ROWEX's in run 1 and claims no winner in
run 2" is only true for one arrangement of verdicts. Rather than substitute new
numbers into a sentence whose words no longer fit them, each prose renderer
asserts the outcome shape its wording states and exits non-zero naming the
sentence when the artifacts no longer have that shape (AGENTS.md §8.1). A
re-measurement that changes a verdict therefore fails here, and the fix is to
rewrite that sentence in both the README and this file.

## What stays hand-maintained, and why

- The `b868fb2e` comparisons: §7.1's last bullet, the half-sentence in §7.2's
  "Writers with readers present" bullet, §7.2's correction table and the
  paragraph after it. They set the current pair against the superseded
  `b868fb2e` pair, whose artifacts are no longer in the tree (they were
  replaced in place by #952), so nothing here can derive them.
- Sentences that carry no measured figure: the pre-registration and method
  citations, the "unmeasured" disclosures, §7.6's closing paragraph, and the
  parts of §7's provenance blockquote the artifacts do not record (host OS
  release, L3 size, the runner command, the bootstrap resample count).
- §7.4 and §7.5 are outside this generator's scope.

The METHODOLOGY §11.5 registrations are encoded once, in the renderers below.
Ratios are Expanse ÷ ROWEX throughput, so above 1.000 means Expanse is faster.
"""

from __future__ import annotations

import copy
import json
import re
import sys
from pathlib import Path

BASE = Path(__file__).resolve().parent.parent
REPO_ROOT = BASE.parent.parent.parent
RESULTS = BASE / "results" / ("quick" if "--quick" in sys.argv else "")
RUNS = ("baseline_concurrent.json", "baseline_concurrent_run2.json")

ARMS = ("set", "map")
C1_WRITERS = (1, 2, 4, 8, 16)
C2_WRITERS = (0, 1, 2, 4, 8)
WORDS = {1: "one", 2: "two", 4: "four", 8: "eight", 16: "sixteen"}
SUPERSCRIPT = str.maketrans("0123456789", "⁰¹²³⁴⁵⁶⁷⁸⁹")


class ShapeError(SystemExit):
    """The artifacts no longer have the outcome a sentence's wording states."""


def expect(cond: bool, where: str, wording: str) -> None:
    if not cond:
        raise ShapeError(
            f"concurrent_tables.py: {where}: the artifacts no longer match the wording "
            f"\"{wording}\"; rewrite that sentence in README §7 and in this generator "
            f"(AGENTS.md §8.1 — a sentence is never re-filled with numbers it does not describe)")


# --------------------------------------------------------------------------
# cell access and formatting
# --------------------------------------------------------------------------

def cell(art: dict, arm: str, pillar: str, w: int) -> dict:
    found = [c for c in art["throughput"] if (c["arm"], c["pillar"], c["writers"]) == (arm, pillar, w)]
    if len(found) != 1:
        raise SystemExit(f"concurrent_tables.py: expected one {arm} {pillar} W={w} cell, found {len(found)}")
    return found[0]


def ratio(c: dict, role: str) -> float:
    return c[f"{role}_expanse_over_rowex"]


def ri(c: dict, role: str) -> str:
    """A ratio with its BCa 95% interval, as the README prints it."""
    return f"{ratio(c, role):.3f} [{c[f'{role}_ci_lower']:.3f}, {c[f'{role}_ci_upper']:.3f}]"


def verdict(c: dict, role: str) -> str:
    return c[f"{role}_verdict"]


def overlap(a: dict, b: dict, role: str) -> bool:
    return not (a[f"{role}_ci_upper"] < b[f"{role}_ci_lower"] or b[f"{role}_ci_upper"] < a[f"{role}_ci_lower"])


def rng(vals, fmt: str = "{:.3f}") -> str:
    """`lo–hi` over formatted values, or one value when they format alike."""
    lo, hi = fmt.format(min(vals)), fmt.format(max(vals))
    return lo if lo == hi else f"{lo}–{hi}"


def roles(pillar: str, w: int) -> tuple[str, ...]:
    if pillar == "C1":
        return ("writer",)
    return ("reader",) if w == 0 else ("reader", "writer")


# --------------------------------------------------------------------------
# §7 — the provenance figures of the section's opening blockquote
# --------------------------------------------------------------------------

def provenance(r1: dict, r2: dict) -> list[str]:
    p1, p2 = r1["provenance"], r2["provenance"]
    for key in ("commit", "hot_commit", "tbb_commit", "platform", "rustflags", "cxx_flags", "core_pin"):
        expect(p1.get(key) == p2.get(key), f"§7 provenance ({key})",
               "harness commit …, two runs — one build configuration for both")
    cells = [c for art in (r1, r2) for c in art["throughput"] + art["health"]]
    expect(all(c.get("cpus_allowed") == p1["core_pin"] for c in cells), "§7 provenance",
           "every row records `Cpus_allowed_list …`")
    expect(all((c.get("load") or {}).get("foreign_busy_cpus") is not None for c in cells), "§7 provenance",
           "a load snapshot per cell")
    glibc = re.search(r"glibc([\d.]+)", p1["platform"])
    expect(glibc is not None, "§7 provenance", "both on glibc … `malloc`")
    tput_rounds = {c["rounds"] for art in (r1, r2) for c in art["throughput"]}
    health_rounds = {c["rounds"] for art in (r1, r2) for c in art["health"]}
    expect(len(tput_rounds) == 1 and len(health_rounds) == 1, "§7 provenance",
           "N rounds per throughput cell and M per health cell")
    loads = [[s["load1"] for s in art["provenance"]["loads"]] for art in (r1, r2)]
    foreign = max(c["load"]["foreign_busy_cpus"] for c in cells)
    pin = p1["core_pin"]
    workloads = ", ".join(f"`{w}`" for w in workload_ids(r1))
    return [
        f"HOT `{p1['hot_commit']}` with its pinned TBB 2018 `{p1['tbb_commit']}`, built from the nested "
        f"submodule, no system TBB; harness commit `{p1['commit']}`, two runs;",
        f"benchmark shell pinned to CPUs {pin.replace('-', '–')} and every row records "
        f"`Cpus_allowed_list {pin}`; writers + readers ≤ "
        f"{max(c['writers'] + c['readers'] for c in cells)}; both arms "
        f"`{p1['rustflags']}` / `{p1['cxx_flags'].split()[0]}`, both on glibc {glibc.group(1)} `malloc`; "
        f"load average {loads[0][0]:.2f} and {loads[1][0]:.2f} at the two runs' starts and at most "
        f"{max(loads[0]):.2f} and {max(loads[1]):.2f} during them — the sweep's own threads — with a load "
        f"snapshot per cell putting foreign busy CPU at no more than {foreign:.2f} core-equivalents in any "
        f"cell of either run; {tput_rounds.pop()} rounds per throughput cell and {health_rounds.pop()} per "
        f"health cell,",
        f"`results/{RUNS[0]}`, `results/{RUNS[1]}`; workloads {workloads})*.",
    ]


def workload_ids(art: dict) -> list[str]:
    return [next(c["workload_id"] for c in art["throughput"] if c["arm"] == arm) for arm in ARMS]


# --------------------------------------------------------------------------
# §7.1 and §7.2 — run-1 level tables
# --------------------------------------------------------------------------

LEVEL_HEAD = ("| W | set: ROWEX M/s | set: Expanse M/s | ratio [BCa 95%] | verdict | "
              "map: ROWEX M/s | map: Expanse M/s | ratio [BCa 95%] | verdict |")
LEVEL_RULE = "|--:|---:|---:|---|---|---:|---:|---|---|"
VERDICT_BOLD = {"expanse": "Expanse", "rowex": "**ROWEX**", "BOUNDARY_RESULT": "`BOUNDARY_RESULT`"}
VERDICT_PLAIN = {"expanse": "Expanse", "rowex": "ROWEX", "BOUNDARY_RESULT": "`BOUNDARY_RESULT`"}
POSSESSIVE = {"expanse": "Expanse's", "rowex": "ROWEX's", "BOUNDARY_RESULT": "`BOUNDARY_RESULT`"}


def level_cells(c: dict, role: str) -> tuple[str, str]:
    """ROWEX and Expanse medians, the winner's in bold; neither when the interval spans parity."""
    rowex, exp = f"{c[f'rowex_{role}_mops_median']:.2f}", f"{c[f'expanse_{role}_mops_median']:.2f}"
    if verdict(c, role) == "expanse":
        exp = f"**{exp}**"
    elif verdict(c, role) == "rowex":
        rowex = f"**{rowex}**"
    return rowex, exp


def level_table(r1: dict, pillar: str, role: str, writers) -> list[str]:
    out = [LEVEL_HEAD, LEVEL_RULE]
    for w in writers:
        row = [str(w)]
        for arm in ARMS:
            c = cell(r1, arm, pillar, w)
            label = VERDICT_BOLD[verdict(c, role)]
            if pillar == "C1" and w >= 16:
                label += " · *not pre-registered (SMT)*"  # METHODOLOGY §11.5.4
            row += [*level_cells(c, role), ri(c, role), label]
        out.append("| " + " | ".join(row) + " |")
    return out


def first_rowex_w(art: dict, arm: str) -> int | None:
    return next((w for w in C1_WRITERS if verdict(cell(art, arm, "C1", w), "writer") == "rowex"), None)


def section_71(r1: dict, r2: dict) -> list[str]:
    runs = (r1, r2)
    c1 = [c for art in runs for c in art["throughput"] if c["pillar"] == "C1"]
    prefill, fresh = {c["prefill"] for c in c1}, {c["fresh_keys"] for c in c1}
    expect(len(prefill) == 1 and len(fresh) == 1, "§7.1 intro", "2²⁰ fresh keys into a 2²⁰ prefill")
    pf, fk = prefill.pop(), fresh.pop()
    expect(pf & (pf - 1) == 0 and fk & (fk - 1) == 0, "§7.1 intro", "2²⁰ fresh keys into a 2²⁰ prefill")
    power = lambda n: "2" + str(n.bit_length() - 1).translate(SUPERSCRIPT)  # noqa: E731

    # "ROWEX wins from four writers": the least W from which every writer cell,
    # both arms, both runs, is ROWEX's.
    all_rowex_from = next((w for w in C1_WRITERS if all(
        verdict(cell(art, arm, "C1", v), "writer") == "rowex"
        for art in runs for arm in ARMS for v in C1_WRITERS if v >= w)), None)
    expect(all_rowex_from is not None, "§7.1 heading", "ROWEX wins from … writers")
    out = [f"### 7.1 Writer throughput as writer count scales — ROWEX wins from {WORDS[all_rowex_from]} writers\n",
           f"W writers each insert their slice of {power(fk)} fresh keys into a {power(pf)} prefill; fixed "
           f"work, so both arms grow by exactly the same population every round.\n",
           *level_table(r1, "C1", "writer", C1_WRITERS), ""]

    # One writer: METHODOLOGY §11.5.2 registered an Expanse win.
    w1 = {(i, arm): cell(art, arm, "C1", 1) for i, art in enumerate(runs, 1) for arm in ARMS}
    expect(all(verdict(c, "writer") == "expanse" for c in w1.values()), "§7.1 one writer",
           "Expanse wins with one writer — `CONFIRMED`")
    out.append(
        f"- **Expanse wins with one writer** — {ri(w1[1, 'set'], 'writer')} (set) and "
        f"{ri(w1[1, 'map'], 'writer')} (map) in run 1, {ri(w1[2, 'set'], 'writer')} and "
        f"{ri(w1[2, 'map'], 'writer')} in run 2 — **`CONFIRMED`** (§11.5.2, medium-high).")

    # Two writers: §11.5.1 registered ROWEX or BOUNDARY_RESULT.
    w2 = {(i, arm): cell(art, arm, "C1", 2) for i, art in enumerate(runs, 1) for arm in ARMS}
    expect(verdict(w2[1, "map"], "writer") == verdict(w2[2, "map"], "writer") == "expanse", "§7.1 two writers",
           "The map arm is Expanse's in both runs — `REFUTED`")
    expect((verdict(w2[1, "set"], "writer"), verdict(w2[2, "set"], "writer")) == ("rowex", "BOUNDARY_RESULT"),
           "§7.1 two writers", "The set arm is ROWEX's in run 1 … and claims no winner in run 2")
    out.append(
        f"- **At two writers the arms split.** The map arm is Expanse's in both runs, "
        f"{ri(w2[1, 'map'], 'writer')} and {ri(w2[2, 'map'], 'writer')} — **`REFUTED`** in Expanse's favour, "
        f"since §11.5.1 registered ROWEX or `BOUNDARY_RESULT`. The set arm is ROWEX's in run 1, "
        f"{ri(w2[1, 'set'], 'writer')}, and claims no winner in run 2, {ri(w2[2, 'set'], 'writer')}: the runs "
        f"disagree on a winner, so under `docs/BENCHMARKING.md` rule 18 the cell is direction-only. Both "
        f"outcomes lie inside the registered row, which is **`CONFIRMED`** on the set arm.")

    # Crossover W* ∈ [2, 4] (§11.5.1).
    ws = {(i, arm): first_rowex_w(art, arm) for i, art in enumerate(runs, 1) for arm in ARMS}
    expect(all(v is not None and 2 <= v <= 4 for v in ws.values()), "§7.1 crossover",
           "The crossover lies inside the registered W* ∈ [2, 4] — `CONFIRMED`")
    expect(ws[1, "map"] == ws[2, "map"] and ws[1, "set"] != ws[2, "set"], "§7.1 crossover",
           "W* = … on the map arm in both runs; on the set arm W* = … in run 1 and … in run 2")
    no_winner = [w for w in C1_WRITERS if w < ws[2, "set"]
                 and verdict(cell(r2, "set", "C1", w), "writer") == "BOUNDARY_RESULT"]
    expect(ws[1, "set"] < ws[2, "set"] and no_winner == [ws[1, "set"]], "§7.1 crossover",
           "where W = … claims no winner")
    out.append(
        f"- **The crossover lies inside the registered W\\* ∈ [2, 4]** — **`CONFIRMED`** (§11.5.1, medium): "
        f"W\\* = {ws[1, 'map']} on the map arm in both runs; on the set arm W\\* = {ws[1, 'set']} in run 1 and "
        f"{ws[2, 'set']} in run 2, where W = {no_winner[0]} claims no winner.")

    # W ≥ 4 (§11.5.1, high), and each arm's scaling.
    wide = [cell(art, arm, "C1", w) for art in runs for arm in ARMS for w in C1_WRITERS if w >= 4]
    expect(all(verdict(c, "writer") == "rowex" for c in wide), "§7.1 W ≥ 4",
           "At W ≥ 4 ROWEX wins every cell — `CONFIRMED`")
    margins = [1 / ratio(c, "writer") for c in wide]
    seq = {arm: [cell(r1, arm, "C1", w)["expanse_writer_mops_median"] for w in C1_WRITERS] for arm in ARMS}
    to_eight = [C1_WRITERS.index(w) for w in C1_WRITERS if w <= 8]
    expect(all(all(seq[arm][i] < seq[arm][i + 1] for i in to_eight[:-1]) for arm in ARMS), "§7.1 W ≥ 4",
           "Expanse's aggregate writer throughput rises with writer count to eight on both arms")

    def scale(arm: str, side: str, w: int) -> str:
        return rng([cell(art, arm, "C1", w)[f"{side}_writer_mops_median"]
                    / cell(art, arm, "C1", 1)[f"{side}_writer_mops_median"] for art in runs], "{:.2f}")

    top = C1_WRITERS[-1]
    out.append(
        f"- **At W ≥ 4 ROWEX wins every cell, by {rng(margins, '{:.2f}').replace('–', '×–')}×** across "
        f"W = 4, 8 and {top} in both runs — **`CONFIRMED`** (§11.5.1, high). Expanse's aggregate writer "
        f"throughput rises with writer count to eight on both arms — set "
        f"{' → '.join(f'{v:.2f}' for v in seq['set'])} M inserts/s, map "
        f"{' → '.join(f'{v:.2f}' for v in seq['map'])} in run 1 — reaching {scale('set', 'expanse', top)}× (set) "
        f"and {scale('map', 'expanse', top)}× (map) its one-writer rate at {WORDS[top]} across the two runs. "
        f"ROWEX scales {scale('set', 'rowex', 8)}× (set) and {scale('map', 'rowex', 8)}× (map) at W = 8 and "
        f"{scale('set', 'rowex', top)}× / {scale('map', 'rowex', top)}× at W = {top}, where the {WORDS[top]} "
        f"threads occupy both SMT siblings of every P-core.")
    return out


def section_72(r1: dict, r2: dict) -> list[str]:
    runs = (r1, r2)
    readers = {c["readers"] for art in runs for c in art["throughput"] if c["pillar"] == "C2"}
    expect(len(readers) == 1, "§7.2 intro", "Eight readers probe …")
    nr = readers.pop()
    out = ["### 7.2 Readers alongside writers\n",
           f"{WORDS[nr].capitalize()} readers probe a 50/50 stream against the prefill while W writers insert; "
           f"W = 0 is the reader-only reference.\n",
           *level_table(r1, "C2", "reader", C2_WRITERS), ""]

    # Reader-only: map registered an Expanse win (§11.5.2), set a BOUNDARY_RESULT.
    w0 = {(i, arm): cell(art, arm, "C2", 0) for i, art in enumerate(runs, 1) for arm in ARMS}
    expect(all(verdict(c, "reader") == "expanse" for c in w0.values()), "§7.2 reader-only",
           "Reader-only (W = 0): Expanse wins on both arms")
    out.append(
        f"- **Reader-only (W = 0):** Expanse wins on both arms. The map row is **`CONFIRMED`** (§11.5.2, "
        f"medium), {ri(w0[1, 'map'], 'reader')} and {ri(w0[2, 'map'], 'reader')}. The set row was registered as "
        f"`BOUNDARY_RESULT` and landed as an Expanse win in both runs, {ri(w0[1, 'set'], 'reader')} and "
        f"{ri(w0[2, 'set'], 'reader')} — recorded as a registered no-winner that resolved in Expanse's favour, "
        f"not as a confirmed prediction.")

    # Readers under writer load: §11.5.1 registered a ROWEX win.
    loaded = [w for w in C2_WRITERS if w > 0]
    under = {arm: [cell(art, arm, "C2", w) for art in runs for w in loaded] for arm in ARMS}
    expect(all(verdict(c, "reader") == "expanse" for arm in ARMS for c in under[arm]), "§7.2 readers under load",
           "Expanse wins every cell in both runs — `REFUTED`")
    every_overlaps = all(overlap(cell(r1, arm, "C2", w), cell(r2, arm, "C2", w), "reader")
                         for arm in ARMS for w in C2_WRITERS)
    expect(every_overlaps, "§7.2 readers under load", "Every reader cell's two intervals overlap (§7.6)")

    def keep(arm: str, side: str, w: int) -> str:
        return rng([cell(art, arm, "C2", w)[f"{side}_reader_mops_median"]
                    / cell(art, arm, "C2", 0)[f"{side}_reader_mops_median"] for art in runs], "{:.2f}")

    lo_w, hi_w = loaded[0], loaded[-1]
    ws = ", ".join(str(w) for w in loaded[:-1]) + f" and {loaded[-1]}"
    out.append(
        f"- **Readers under writer load: Expanse wins every cell in both runs** — "
        f"{rng([ratio(c, 'reader') for c in under['set']])} on the set arm and "
        f"{rng([ratio(c, 'reader') for c in under['map']])} on the map arm across W = {ws} — so the registered "
        f"ROWEX win (§11.5.1, medium-high) is **`REFUTED`** in Expanse's favour on all "
        f"{WORDS[len(ARMS) * len(loaded)]} cells. With {WORDS[lo_w]} writer, Expanse's {WORDS[nr]} readers keep "
        f"{keep('set', 'expanse', lo_w)}× (set) and {keep('map', 'expanse', lo_w)}× (map) of their reader-only "
        f"rate and ROWEX's keep {keep('set', 'rowex', lo_w)}× and {keep('map', 'rowex', lo_w)}×; with "
        f"{WORDS[hi_w]} writers, {keep('set', 'expanse', hi_w)}× and {keep('map', 'expanse', hi_w)}× against "
        f"{keep('set', 'rowex', hi_w)}× and {keep('map', 'rowex', hi_w)}× (the two runs' range). Every reader "
        f"cell's two intervals overlap (§7.6).")

    # Writers with readers present: not registered; reported. The sentence
    # comparing the set W = 2 cell with the `b868fb2e` pair sits between the two
    # lines below and is hand-maintained (see the module docstring).
    cw = {(i, arm, w): cell(art, arm, "C2", w) for i, art in enumerate(runs, 1) for arm in ARMS for w in loaded}
    expect(all(verdict(cw[i, arm, 1], "writer") == "expanse" for i in (1, 2) for arm in ARMS)
           and verdict(cw[1, "map", 2], "writer") == verdict(cw[2, "map", 2], "writer") == "expanse"
           and verdict(cw[1, "set", 2], "writer") == verdict(cw[2, "set", 2], "writer") == "rowex",
           "§7.2 writers with readers",
           "Expanse wins the W = 1 writer cell on both arms … and the map arm at W = 2 … The set arm's W = 2 "
           "writer cell is ROWEX's in both runs")
    c1w1 = {arm: cell(r1, arm, "C1", 1) for arm in ARMS}
    lv = lambda c, side: f"{c[f'{side}_writer_mops_median']:.2f}"  # noqa: E731
    out.append(
        f"- **Writers with readers present** *(not registered as a separate row; reported)*: with {WORDS[nr]} "
        f"readers probing, the Expanse single writer runs at {lv(cw[1, 'set', 1], 'expanse')} M inserts/s (set) "
        f"and {lv(cw[1, 'map', 1], 'expanse')} (map), against {lv(c1w1['set'], 'expanse')} and "
        f"{lv(c1w1['map'], 'expanse')} without readers; ROWEX's at {lv(cw[1, 'set', 1], 'rowex')} and "
        f"{lv(cw[1, 'map', 1], 'rowex')}, against {lv(c1w1['set'], 'rowex')} and {lv(c1w1['map'], 'rowex')} "
        f"(run 1). Expanse wins the W = 1 writer cell on both arms — set {ri(cw[1, 'set', 1], 'writer')} and "
        f"{ri(cw[2, 'set', 1], 'writer')}, map {ri(cw[1, 'map', 1], 'writer')} and "
        f"{ri(cw[2, 'map', 1], 'writer')} — and the map arm at W = 2, {ri(cw[1, 'map', 2], 'writer')} and "
        f"{ri(cw[2, 'map', 2], 'writer')}. The set arm's W = 2 writer cell is ROWEX's in both runs, "
        f"{ri(cw[1, 'set', 2], 'writer')} and {ri(cw[2, 'set', 2], 'writer')};")
    wide = {arm: [cw[i, arm, w] for i in (1, 2) for w in loaded if w >= 4] for arm in ARMS}
    expect(all(verdict(c, "writer") == "rowex" for arm in ARMS for c in wide[arm]), "§7.2 writers with readers",
           "ROWEX wins every writer cell at W ≥ 4")
    out.append(
        f"ROWEX wins every writer cell at W ≥ 4: {rng([ratio(c, 'writer') for c in wide['set']])} (set) and "
        f"{rng([ratio(c, 'writer') for c in wide['map']])} (map).")
    return out


# --------------------------------------------------------------------------
# §7.6 — every ratio cell of the two runs side by side
# --------------------------------------------------------------------------

def section_76(r1: dict, r2: dict) -> list[str]:
    p1, p2 = r1["provenance"], r2["provenance"]
    expect(p1["commit"] == p2["commit"] and p1["core_pin"] == p2["core_pin"], "§7.6 intro",
           "The arm was run twice on the reference host at one commit, both under the P-core pin")
    out = ["### 7.6 Between-run spread: two runs at one commit (#735)\n",
           f"The arm was run twice on the reference host **at one commit**, `{p1['commit']}`, both under the "
           f"P-core pin with a load snapshot per cell. The binaries are identical, so the table below is "
           f"run-to-run spread on this host and nothing else *(workloads: "
           f"{', '.join(f'`{w}`' for w in workload_ids(r1))})*.\n",
           "| Arm | cell | W | R | run 1 | run 2 | intervals overlap | verdict |",
           "|---|---|--:|--:|---|---|---|---|"]
    moved, disagree = [], []
    for arm in ARMS:
        for pillar, writers in (("C1", C1_WRITERS), ("C2", C2_WRITERS)):
            for w in writers:
                for role in roles(pillar, w):
                    a, b = cell(r1, arm, pillar, w), cell(r2, arm, pillar, w)
                    ok = overlap(a, b, role)
                    if not ok:
                        moved.append((arm, pillar, role, w))
                    va, vb = verdict(a, role), verdict(b, role)
                    if va == vb:
                        label = VERDICT_PLAIN[va]
                    else:
                        label = f"runs disagree ({VERDICT_PLAIN[va]} / {VERDICT_PLAIN[vb]}) — direction-only"
                        disagree.append((arm, pillar, role, w, va, vb))
                    out.append(f"| {arm} | {pillar} {role} | {w} | {a['readers']} | {ri(a, role)} | {ri(b, role)} | "
                               f"{'yes' if ok else '**no**'} | {label} |")
    n = len(out) - 4
    out.append("")
    expect(not moved, "§7.6 summary", f"None of the {n} ratio cells moved past its own interval")
    expect(len(disagree) == 1, "§7.6 summary", "in 1 cell the runs disagree on a winner")
    arm, pillar, role, w, va, vb = disagree[0]
    expect(r1["memory"] == r2["memory"], "§7.6 summary",
           "The concurrent memory cells are byte-identical between the runs")
    out.append(
        f"**None of the {n} ratio cells moved past its own interval**, and **in 1 cell the runs disagree on a "
        f"winner**: {arm} {pillar} W = {w}, {POSSESSIVE[va]} in run 1 and {POSSESSIVE[vb]} in run 2, with "
        f"overlapping intervals. That cell is direction-only; every other cell has the same verdict in both "
        f"runs. The concurrent memory cells are byte-identical between the runs, which is the control: a "
        f"deterministic census taken by the same code on the same host reproduces exactly, so the wall-clock "
        f"spread is not the instrument reading differently.")
    return out


def render(r1: dict, r2: dict) -> str:
    out = [f"<!-- generated by scripts/concurrent_tables.py from results/ at commit {r1['provenance']['commit']} -->\n",
           "## 7. The concurrent arm: HOT-ROWEX against the multi-writer engine\n",
           *provenance(r1, r2), "",
           *section_71(r1, r2), "",
           *section_72(r1, r2), "",
           *section_76(r1, r2), ""]
    return "\n".join(out)


def load_runs(results: Path = RESULTS) -> tuple[dict, dict]:
    paths = [results / name for name in RUNS]
    missing = [str(p) for p in paths if not p.is_file()]
    if missing:
        raise SystemExit(f"concurrent_tables.py: missing artifact(s): {', '.join(missing)}")
    r1, r2 = (json.loads(p.read_text()) for p in paths)
    return r1, r2


# --------------------------------------------------------------------------
# self-test
# --------------------------------------------------------------------------

# Verbatim from README §7 as committed at d4a7617c. They pin this file's
# formatting against a literal copy, independently of the README it is checked
# against (AGENTS.md §8.12.3: pin the motivating defect).
PINNED = (
    "| 1 | 5.33 | **7.36** | 1.382 [1.367, 1.398] | Expanse | 2.95 | **3.88** | 1.347 [1.296, 1.413] | Expanse |",
    "| 16 | **33.50** | 14.84 | 0.456 [0.444, 0.483] | **ROWEX** · *not pre-registered (SMT)* | **21.11** | 10.18 | "
    "0.480 [0.468, 0.489] | **ROWEX** · *not pre-registered (SMT)* |",
    "| 8 | 70.57 | **81.56** | 1.143 [1.104, 1.172] | Expanse | 30.37 | **62.97** | 2.090 [2.022, 2.160] | Expanse |",
    "| set | C1 writer | 2 | 0 | 0.981 [0.972, 0.995] | 0.984 [0.974, 1.011] | yes | "
    "runs disagree (ROWEX / `BOUNDARY_RESULT`) — direction-only |",
    "| map | C2 writer | 8 | 8 | 0.716 [0.690, 0.741] | 0.756 [0.727, 0.780] | yes | ROWEX |",
    "**None of the 28 ratio cells moved past its own interval**, and **in 1 cell the runs disagree on a winner**: "
    "set C1 W = 2, ROWEX's in run 1 and `BOUNDARY_RESULT` in run 2, with overlapping intervals.",
    "reaching 2.01–2.02× (set) and 2.62–2.65× (map) its one-writer rate at sixteen across the two runs.",
    "0.54–0.55× and 0.60× against 0.55–0.56× and 0.42–0.43× (the two runs' range).",
    "load average 1.13 and 0.95 at the two runs' starts and at most 5.76 and 6.29 during them",
)


def _self_test() -> int:
    sys.path.insert(0, str(REPO_ROOT / "scripts"))
    import check_readme_tables as crt

    failures: list[str] = []
    suite = crt.Suite("hot_comparison", "concurrent_tables.py", enforce=True)
    r1, r2 = load_runs(BASE / "results")
    text = render(r1, r2)

    def refused(mutate, want: str) -> None:
        a, b = copy.deepcopy(r1), copy.deepcopy(r2)
        mutate(a, b)
        try:
            render(a, b)
        except ShapeError as e:
            if want not in str(e):
                failures.append(f"refusal did not name {want!r}: {e}")
            return
        failures.append(f"a mutation that falsifies {want!r} rendered instead of refusing")

    # 1. The committed artifacts reproduce the committed README.
    in_sync = not crt.check(suite, text)
    if not in_sync:
        failures.append("the committed artifacts do not reproduce README §7: " + "; ".join(crt.check(suite, text)))

    # 2. Pinned literal rows and figures.
    flat = " ".join(text.split())
    for line in PINNED:
        if " ".join(line.split()) not in flat:
            failures.append(f"pinned line not rendered: {line[:100]}")

    # 3. One changed ratio changes the row and the prose, and the checker sees
    #    both against the real README.
    a, b = copy.deepcopy(r1), copy.deepcopy(r2)
    cell(a, "set", "C1", 1)["writer_expanse_over_rowex"] = 1.3872
    mutated = render(a, b)
    if "| 1 | 5.33 | **7.36** | 1.387 [1.367, 1.398] |" not in mutated:
        failures.append("a changed run-1 ratio did not change the §7.1 row")
    found = crt.check(suite, mutated)
    if not any("1.387 [1.367, 1.398]" in f and "7.1" in f for f in found):
        failures.append(f"check_readme_tables did not report the changed §7.1 row: {found}")

    # 4. An altered README row is a finding.
    import tempfile
    readme = suite.readme.read_text()
    altered = readme.replace("| 16 | **33.50** | 14.84 |", "| 16 | **33.51** | 14.84 |")
    if altered == readme:
        failures.append("the README row the self-test alters is no longer present")
    tmp = Path(tempfile.mkdtemp()) / "README.md"
    tmp.write_text(altered)
    suite.readme = tmp
    # The checker names the first drifted row per section, so the row is only
    # named when the unaltered README was in sync (case 1).
    found = crt.check(suite, text)
    if not any("7.1" in f and (not in_sync or "33.50" in f) for f in found):
        failures.append(f"an altered README row (33.50 -> 33.51) was not reported: {found}")

    # 5. Outcome shapes the prose does not describe are refused, not re-filled.
    # W = 8 rather than 4, so the crossover (still W* = 4) is not what refuses.
    def flip_w8(a, b):
        cell(b, "map", "C1", 8)["writer_verdict"] = "expanse"
    refused(flip_w8, "At W ≥ 4 ROWEX wins every cell")

    def split_reader(a, b):
        c = cell(b, "set", "C2", 8)
        c["reader_ci_lower"], c["reader_ci_upper"] = 1.300, 1.400
    refused(split_reader, "Every reader cell's two intervals overlap")

    def agree_w2(a, b):
        cell(b, "set", "C1", 2)["writer_verdict"] = "rowex"
    refused(agree_w2, "claims no winner in run 2")

    def memory_drift(a, b):
        b["memory"][0]["hot_alloc_bytes_per_key"] += 0.01
    refused(memory_drift, "byte-identical")

    for msg in failures:
        print(f"  FAIL {msg}")
    if failures:
        print(f"concurrent_tables.py --self-test: {len(failures)} failure(s)")
        return 1
    print("concurrent_tables.py --self-test: all checks passed")
    return 0


def main() -> int:
    if "--self-test" in sys.argv:
        return _self_test()
    print(render(*load_runs()))
    return 0


if __name__ == "__main__":
    sys.exit(main())
