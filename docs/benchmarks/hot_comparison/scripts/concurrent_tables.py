#!/usr/bin/env python3
"""README §7.1, §7.2 and §7.6–§7.9 for the concurrent HOT-ROWEX arm (#692), derived from results/.

Every table row in those sections, and every figure in the prose around them
that the committed runs determine, is computed here from
``results/baseline_concurrent.json`` (run 1) and
``results/baseline_concurrent_run2.json`` (run 2) — AGENTS.md §8.2: nothing in a
generated table is typed by hand. §7.3 comes from ``integer_tables.py``.

Three more pairs feed the later sections. §7.7 sets every cell against the pair
these names held before the re-measurement, kept at ``results/at_6f8d6ba5/``.
§7.8 is the same sweep under the per-core pin,
``results/baseline_concurrent_percore.json`` and its run 2 (AGENTS.md §8.20.5
step 0: a cell is comparable only under its own pin). §7.9 splits cells by which
arm a round timed first (``scripts/round_order.py``).

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

- The correction statements that name what changed between the two commits
  (the blockquote at the top of §7, and the paragraphs of §7.7–§7.9): the
  artifacts record a commit, not what landed between two of them.
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
PERCORE_RUNS = ("baseline_concurrent_percore.json", "baseline_concurrent_percore_run2.json")
PREVIOUS_RUNS = ("at_6f8d6ba5/baseline_concurrent.json", "at_6f8d6ba5/baseline_concurrent_run2.json")
sys.path.insert(0, str(REPO_ROOT / "scripts"))
import round_order  # noqa: E402  (the position split of an interleaved cell)

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

    # "Expanse wins through eight writers": every registered writer cell
    # (W ≤ 8), both arms, both runs, is Expanse's.
    registered = [w for w in C1_WRITERS if w < 16]
    expect(all(verdict(cell(art, arm, "C1", w), "writer") == "expanse"
               for art in runs for arm in ARMS for w in registered),
           "§7.1 heading", "Expanse wins through … writers")
    out = [f"### 7.1 Writer throughput as writer count scales — Expanse wins through {WORDS[registered[-1]]} writers\n",
           f"W writers each insert their slice of {power(fk)} fresh keys into a {power(pf)} prefill; fixed "
           f"work, so both arms grow by exactly the same population every round.\n",
           *level_table(r1, "C1", "writer", C1_WRITERS), ""]

    # One writer: METHODOLOGY §11.5.2 registered an Expanse win.
    w1 = {(i, arm): cell(art, arm, "C1", 1) for i, art in enumerate(runs, 1) for arm in ARMS}
    out.append(
        f"- **Expanse wins with one writer** — {ri(w1[1, 'set'], 'writer')} (set) and "
        f"{ri(w1[1, 'map'], 'writer')} (map) in run 1, {ri(w1[2, 'set'], 'writer')} and "
        f"{ri(w1[2, 'map'], 'writer')} in run 2 — **`CONFIRMED`** (§11.5.2, medium-high).")

    # Two writers: §11.5.1 registered ROWEX or BOUNDARY_RESULT.
    w2 = {(i, arm): cell(art, arm, "C1", 2) for i, art in enumerate(runs, 1) for arm in ARMS}
    out.append(
        f"- **Expanse wins at two writers on both arms, in both runs** — set {ri(w2[1, 'set'], 'writer')} and "
        f"{ri(w2[2, 'set'], 'writer')}, map {ri(w2[1, 'map'], 'writer')} and {ri(w2[2, 'map'], 'writer')} — "
        f"**`REFUTED`** in Expanse's favour, since §11.5.1 registered ROWEX or `BOUNDARY_RESULT`.")

    # Crossover W* ∈ [2, 4] (§11.5.1): none by W = 16 refutes the row.
    ws = {(i, arm): first_rowex_w(art, arm) for i, art in enumerate(runs, 1) for arm in ARMS}
    expect(all(v is None for v in ws.values()), "§7.1 crossover",
           "ROWEX wins no writer cell on either arm in either run")
    top = C1_WRITERS[-1]
    out.append(
        f"- **There is no crossover by {WORDS[top]} writers.** ROWEX wins no writer cell on either arm in either "
        f"run, so the registered W\\* ∈ [2, 4] is **`REFUTED`** (§11.5.1, medium): that row counts \"none by "
        f"W = {top}\" as a refutation.")

    # W = 4 and 8 (§11.5.1, high), the unregistered W = 16, and each arm's scaling.
    wide = [cell(art, arm, "C1", w) for art in runs for arm in ARMS for w in registered if w >= 4]
    margins = [ratio(c, "writer") for c in wide]
    t16 = {(i, arm): cell(art, arm, "C1", top) for i, art in enumerate(runs, 1) for arm in ARMS}
    expect(verdict(t16[1, "set"], "writer") == verdict(t16[2, "set"], "writer") == "expanse"
           and verdict(t16[1, "map"], "writer") == verdict(t16[2, "map"], "writer") == "BOUNDARY_RESULT",
           f"§7.1 W = {top}", "the set arm is Expanse's in both runs … and the map arm claims no winner in either")
    seq = {arm: [cell(r1, arm, "C1", w)["expanse_writer_mops_median"] for w in C1_WRITERS] for arm in ARMS}
    expect(all(all(a < b for a, b in zip(
        [cell(art, arm, "C1", w)["expanse_writer_mops_median"] for w in C1_WRITERS],
        [cell(art, arm, "C1", w)["expanse_writer_mops_median"] for w in C1_WRITERS][1:]))
        for art in runs for arm in ARMS), "§7.1 W ≥ 4",
        f"Expanse's aggregate writer throughput rises with writer count to {WORDS[top]} on both arms in both runs")

    def scale(arm: str, side: str, w: int) -> str:
        return rng([cell(art, arm, "C1", w)[f"{side}_writer_mops_median"]
                    / cell(art, arm, "C1", 1)[f"{side}_writer_mops_median"] for art in runs], "{:.2f}")

    out.append(
        f"- **At W = 4 and 8 Expanse wins every cell, by {rng(margins, '{:.2f}').replace('–', '×–')}×** in both "
        f"runs, so the registered ROWEX win (§11.5.1, high) is **`REFUTED`** in Expanse's favour on all "
        f"{WORDS[len(ARMS) * 2]} cells. At W = {top}, which was not pre-registered, the set arm is Expanse's in "
        f"both runs, {ri(t16[1, 'set'], 'writer')} and {ri(t16[2, 'set'], 'writer')}, and the map arm claims no "
        f"winner in either, {ri(t16[1, 'map'], 'writer')} and {ri(t16[2, 'map'], 'writer')}. Expanse's aggregate "
        f"writer throughput rises with writer count to {WORDS[top]} on both arms in both runs — set "
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

    # Writers with readers present: not registered; reported.
    cw = {(i, arm, w): cell(art, arm, "C2", w) for i, art in enumerate(runs, 1) for arm in ARMS for w in loaded}
    expect(all(verdict(c, "writer") == "expanse" for c in cw.values()), "§7.2 writers with readers",
           "Expanse wins every writer cell with readers present, on both arms in both runs")
    c1w1 = {arm: cell(r1, arm, "C1", 1) for arm in ARMS}
    lv = lambda c, side: f"{c[f'{side}_writer_mops_median']:.2f}"  # noqa: E731
    by_arm = {arm: [ratio(cw[i, arm, w], "writer") for i in (1, 2) for w in loaded] for arm in ARMS}
    out.append(
        f"- **Writers with readers present** *(not registered as a separate row; reported)*: with {WORDS[nr]} "
        f"readers probing, the Expanse single writer runs at {lv(cw[1, 'set', 1], 'expanse')} M inserts/s (set) "
        f"and {lv(cw[1, 'map', 1], 'expanse')} (map), against {lv(c1w1['set'], 'expanse')} and "
        f"{lv(c1w1['map'], 'expanse')} without readers; ROWEX's at {lv(cw[1, 'set', 1], 'rowex')} and "
        f"{lv(cw[1, 'map', 1], 'rowex')}, against {lv(c1w1['set'], 'rowex')} and {lv(c1w1['map'], 'rowex')} "
        f"(run 1). Expanse wins every writer cell with readers present, on both arms in both runs: "
        f"{rng(by_arm['set'])} (set) and {rng(by_arm['map'])} (map) across W = {ws}.")
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
    expect(not disagree, "§7.6 summary", "the two runs agree on every verdict")
    expect(r1["memory"] == r2["memory"], "§7.6 summary",
           "The concurrent memory cells are byte-identical between the runs")
    out.append(
        f"**None of the {n} ratio cells moved past its own interval, and the two runs agree on every verdict.** "
        f"The concurrent memory cells are byte-identical between the runs, which is the control: a "
        f"deterministic census taken by the same code on the same host reproduces exactly, so the wall-clock "
        f"spread is not the instrument reading differently.")
    return out


# --------------------------------------------------------------------------
# §7.7 — every ratio cell against the pair these names held before
# --------------------------------------------------------------------------

def cells_in_order(art: dict):
    for arm in ARMS:
        for pillar, writers in (("C1", C1_WRITERS), ("C2", C2_WRITERS)):
            for w in writers:
                for role in roles(pillar, w):
                    yield arm, pillar, w, role


def verdict_pair(a: dict, b: dict, role: str) -> str:
    va, vb = verdict(a, role), verdict(b, role)
    return VERDICT_PLAIN[va] if va == vb else f"runs disagree ({VERDICT_PLAIN[va]} / {VERDICT_PLAIN[vb]})"


def section_77(previous: tuple[dict, dict], current: tuple[dict, dict]) -> list[str]:
    """docs/BENCHMARKING.md rule 18: a cell moved only when both new intervals lie
    clear of both earlier ones on the same side. Every earlier figure sits behind
    the word "previously": it is superseded, and check_docs_hygiene.py refuses it bare."""
    was, now = previous[0]["provenance"], current[0]["provenance"]
    expect(was["core_pin"] == now["core_pin"], "§7.7 intro", "both pairs under the same pin")
    out = [f"### 7.7 Against the pair previously published at `{was['commit']}` (AGENTS.md §8.7)\n",
           f"| Arm | cell | W | R | previously published at `{was['commit']}` (run 1; run 2) | `{now['commit']}` run 1 | "
           f"`{now['commit']}` run 2 | Expanse M/s, previously → now | ROWEX M/s, previously → now | "
           f"both new intervals clear of both earlier ones | verdict, previously → now |",
           "|---|---|--:|--:|---|---|---|---|---|---|---|"]
    tally = {"up": [], "down": [], "no": []}
    for arm, pillar, w, role in cells_in_order(current[0]):
        a, b = (cell(art, arm, pillar, w) for art in previous)
        c, d = (cell(art, arm, pillar, w) for art in current)
        old_lo, old_hi = min(x[f"{role}_ci_lower"] for x in (a, b)), max(x[f"{role}_ci_upper"] for x in (a, b))
        new_lo, new_hi = min(x[f"{role}_ci_lower"] for x in (c, d)), max(x[f"{role}_ci_upper"] for x in (c, d))
        moved = "up" if new_lo > old_hi else "down" if new_hi < old_lo else "no"
        tally[moved].append((arm, pillar, role, w))

        def level(arts, side):
            return rng([cell(art, arm, pillar, w)[f"{side}_{role}_mops_median"] for art in arts], "{:.2f}")
        out.append(f"| {arm} | {pillar} {role} | {w} | {c['readers']} | previously {ri(a, role)}; {ri(b, role)} | "
                   f"{ri(c, role)} | {ri(d, role)} | previously {level(previous, 'expanse')} → {level(current, 'expanse')} | "
                   f"previously {level(previous, 'rowex')} → {level(current, 'rowex')} | "
                   f"{'**' + moved + '**' if moved != 'no' else 'no'} | "
                   f"previously {verdict_pair(a, b, role)} → {verdict_pair(c, d, role)} |")
    out.append("")
    ups = tally["up"]
    expect(not tally["down"] and ups and all(role == "writer" for _, _, role, _ in ups)
           and all(w >= 2 for _, _, _, w in ups), "§7.7 summary",
           "every cell that moved is a writer cell at W ≥ 2, and it moved up; no reader cell moved")
    writer_cells = [k for k in cells_in_order(current[0]) if k[3] == "writer" and k[2] >= 2]
    expect(sorted(ups) == sorted((a, p, r, w) for a, p, w, r in writer_cells), "§7.7 summary",
           "every writer cell at W ≥ 2 moved up")
    n_reader = sum(1 for k in cells_in_order(current[0]) if k[3] == "reader")
    n_w1 = sum(1 for k in cells_in_order(current[0]) if k[3] == "writer" and k[2] == 1)
    out.append(
        f"**All {len(ups)} writer cells at W ≥ 2 moved up in both new runs, each new interval clear of both "
        f"earlier ones; none of the {n_reader} reader cells and none of the {n_w1} single-writer cells did.**")
    return out


# --------------------------------------------------------------------------
# §7.8 — the per-core pin
# --------------------------------------------------------------------------

def pin_cpus(pin: str) -> int:
    n = 0
    for part in pin.split(","):
        lo, _, hi = part.partition("-")
        n += int(hi or lo) - int(lo) + 1
    return n


def section_78(p1: dict, p2: dict) -> list[str]:
    pin = p1["provenance"]["core_pin"]
    expect(p2["provenance"]["core_pin"] == pin and p1["provenance"]["commit"] == p2["provenance"]["commit"],
           "§7.8 intro", "two runs at one commit under one pin")
    cpus = pin_cpus(pin)
    out = [f"### 7.8 The per-core pin `{pin}` ({cpus} CPUs, one per physical P-core), both runs\n",
           "| Arm | cell | W | R | threads | ROWEX M/s, run 1; run 2 | Expanse M/s, run 1; run 2 | run 1 | run 2 | "
           "intervals overlap | verdict | placement |",
           "|---|---|--:|--:|--:|---|---|---|---|---|---|---|"]
    over = 0
    for arm, pillar, w, role in cells_in_order(p1):
        c, d = cell(p1, arm, pillar, w), cell(p2, arm, pillar, w)
        threads = w + c["readers"]
        if threads > cpus:
            over += 1
            placement = f"**oversubscribed** — {threads} threads on {cpus} CPUs; not comparable across pins"
        else:
            placement = "one CPU per thread"
        out.append(f"| {arm} | {pillar} {role} | {w} | {c['readers']} | {threads} | "
                   f"{c[f'rowex_{role}_mops_median']:.2f}; {d[f'rowex_{role}_mops_median']:.2f} | "
                   f"{c[f'expanse_{role}_mops_median']:.2f}; {d[f'expanse_{role}_mops_median']:.2f} | "
                   f"{ri(c, role)} | {ri(d, role)} | {'yes' if overlap(c, d, role) else '**no**'} | "
                   f"{verdict_pair(c, d, role)} | {placement} |")
    out.append("")
    return out


# --------------------------------------------------------------------------
# §7.9 — the round-order split
# --------------------------------------------------------------------------

# Printed flagged or not: the map arm's single writer, whose Expanse side is
# beyond its interval in both per-core runs and in one `0-15` run.
ALWAYS_SPLIT = ("map", "C1", 1, "writer")


def section_79(pairs: list[tuple[dict, dict]]) -> list[str]:
    out = ["### 7.9 Round-order split: a side's median over the rounds it was timed first, and second\n",
           "| commit | pin | Arm | cell | W | R | threads ÷ CPUs | side | run | median M/s | timed first | timed second | "
           "gap ÷ median | cell interval ÷ ratio | min–max | beyond the interval |",
           "|---|---|---|---|--:|--:|---|---|--:|---:|---:|---:|---:|---:|---|---|"]
    for a, b in pairs:
        prov = a["provenance"]
        cpus = pin_cpus(prov["core_pin"])
        for arm, pillar, w, role in cells_in_order(a):
            c, d = cell(a, arm, pillar, w), cell(b, arm, pillar, w)
            for side in ("expanse", "rowex"):
                splits = [round_order.split(x, role, side, "rowex") for x in (c, d)]
                flags = [sp.gap > sp.interval for sp in splits]
                if not any(flags) and (arm, pillar, w, role) != ALWAYS_SPLIT:
                    continue
                for run, sp, flag in zip(("1", "2"), splits, flags):
                    out.append(f"| `{prov['commit']}` | `{prov['core_pin']}` | {arm} | {pillar} {role} | {w} | "
                               f"{c['readers']} | {w + c['readers']} ÷ {cpus} | "
                               f"{'Expanse' if side == 'expanse' else 'ROWEX'} | {run} | {sp.median:.2f} | "
                               f"{sp.first:.2f} | {sp.second:.2f} | {sp.gap:.1%} | {sp.interval:.1%} | "
                               f"{sp.low:.2f}–{sp.high:.2f} | {'**yes**' if flag else 'no'} |")
    out.append("")
    return out


def render(r1: dict, r2: dict, percore: tuple[dict, dict] | None = None,
           previous: tuple[dict, dict] | None = None) -> str:
    out = [f"<!-- generated by scripts/concurrent_tables.py from results/ at commit {r1['provenance']['commit']} -->\n",
           "## 7. The concurrent arm: HOT-ROWEX against the multi-writer engine\n",
           *provenance(r1, r2), "",
           *section_71(r1, r2), "",
           *section_72(r1, r2), "",
           *section_76(r1, r2), ""]
    if previous:
        out += [*section_77(previous, (r1, r2)), ""]
    if percore:
        out += [*section_78(*percore), ""]
    out += [*section_79([pair for pair in (previous, (r1, r2), percore) if pair]), ""]
    return "\n".join(out)


def load_runs(results: Path = RESULTS, names: tuple[str, str] = RUNS) -> tuple[dict, dict]:
    paths = [results / name for name in names]
    missing = [str(p) for p in paths if not p.is_file()]
    if missing:
        raise SystemExit(f"concurrent_tables.py: missing artifact(s): {', '.join(missing)}")
    r1, r2 = (json.loads(p.read_text()) for p in paths)
    return r1, r2


# --------------------------------------------------------------------------
# self-test
# --------------------------------------------------------------------------

# Verbatim from README §7 as first published for the `929574b5` pair. They pin
# this file's formatting against a literal copy, independently of the README it
# is checked against (AGENTS.md §8.12.3: pin the motivating defect).
PINNED = (
    "| 1 | 5.28 | **7.40** | 1.392 [1.363, 1.407] | Expanse | 2.95 | **3.82** | 1.326 [1.274, 1.393] | Expanse |",
    "| 16 | 33.61 | **49.07** | 1.425 [1.336, 1.505] | Expanse · *not pre-registered (SMT)* | 18.05 | 18.29 | "
    "0.989 [0.911, 1.061] | `BOUNDARY_RESULT` · *not pre-registered (SMT)* |",
    "| 8 | 69.62 | **85.45** | 1.200 [1.158, 1.236] | Expanse | 30.04 | **63.26** | 2.094 [2.008, 2.173] | Expanse |",
    "| map | C1 writer | 16 | 0 | 0.989 [0.911, 1.061] | 0.996 [0.908, 1.096] | yes | `BOUNDARY_RESULT` |",
    "| map | C2 writer | 8 | 8 | 1.095 [1.037, 1.155] | 1.086 [1.032, 1.146] | yes | Expanse |",
    "**None of the 28 ratio cells moved past its own interval, and the two runs agree on every verdict.**",
    "reaching 5.84–6.64× (set) and 4.79–4.83× (map) its one-writer rate at sixteen across the two runs.",
    "0.57–0.58× and 0.59–0.60× against 0.55× and 0.42–0.43× (the two runs' range).",
    "load average 1.31 and 1.47 at the two runs' starts and at most 4.50 and 5.76 during them",
    # §7.7: a superseded figure is only ever printed behind "previously".
    "| set | C1 writer | 8 | 0 | previously 0.546 [0.531, 0.560]; 0.560 [0.545, 0.579] | 1.424 [1.368, 1.485] | "
    "1.447 [1.389, 1.506] | previously 13.47–13.50 → 35.14–35.23 | previously 24.78–24.97 → 25.10–25.16 | **up** | "
    "previously ROWEX → Expanse |",
    "**All 14 writer cells at W ≥ 2 moved up in both new runs, each new interval clear of both earlier ones; none of "
    "the 10 reader cells and none of the 4 single-writer cells did.**",
    # §7.8: a row with more threads than the pin has CPUs says so.
    "| map | C1 writer | 16 | 0 | 16 | 5.65; 5.84 | 2.29; 2.52 | 0.457 [0.395, 0.554] | 0.474 [0.410, 0.566] | yes | "
    "ROWEX | **oversubscribed** — 16 threads on 8 CPUs; not comparable across pins |",
    # §7.9: the split of the map arm's single writer under the per-core pin.
    "| `929574b5` | `0,2,4,6,8,10,12,14` | map | C1 writer | 1 | 0 | 1 ÷ 8 | Expanse | 1 | 3.82 | 4.12 | 3.73 | 10.3% | "
    "8.9% | 3.57–4.65 | **yes** |",
)


def _self_test() -> int:
    sys.path.insert(0, str(REPO_ROOT / "scripts"))
    import check_readme_tables as crt

    failures: list[str] = []
    suite = crt.Suite("hot_comparison", "concurrent_tables.py", enforce=True)
    r1, r2 = load_runs(BASE / "results")
    percore = load_runs(BASE / "results", PERCORE_RUNS)
    previous = load_runs(BASE / "results", PREVIOUS_RUNS)
    text = render(r1, r2, percore=percore, previous=previous)

    # 0. The position split both generators print is pinned where CI already
    #    runs a self-test (scripts/round_order.py has no lane of its own).
    if round_order._self_test() != 0:
        failures.append("scripts/round_order.py --self-test failed")

    def refused(mutate, want: str) -> None:
        a, b = copy.deepcopy(r1), copy.deepcopy(r2)
        mutate(a, b)
        try:
            render(a, b, percore=percore, previous=previous)
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
    mutated = render(a, b, percore=percore, previous=previous)
    if "| 1 | 5.28 | **7.40** | 1.387 [1.363, 1.407] |" not in mutated:
        failures.append("a changed run-1 ratio did not change the §7.1 row")
    found = crt.check(suite, mutated)
    if not any("1.387 [1.363, 1.407]" in f and "7.1" in f for f in found):
        failures.append(f"check_readme_tables did not report the changed §7.1 row: {found}")

    # 4. An altered README row is a finding.
    import tempfile
    readme = suite.readme.read_text()
    altered = readme.replace("| 16 | 33.61 | **49.07** |", "| 16 | 33.62 | **49.07** |")
    if altered == readme:
        failures.append("the README row the self-test alters is no longer present")
    tmp = Path(tempfile.mkdtemp()) / "README.md"
    tmp.write_text(altered)
    suite.readme = tmp
    # The checker names the first drifted row per section, so the row is only
    # named when the unaltered README was in sync (case 1).
    found = crt.check(suite, text)
    if not any("7.1" in f and (not in_sync or "33.61" in f) for f in found):
        failures.append(f"an altered README row (33.61 -> 33.62) was not reported: {found}")

    # 5. Outcome shapes the prose does not describe are refused, not re-filled.
    def flip_w8(a, b):
        cell(b, "map", "C1", 8)["writer_verdict"] = "rowex"
    refused(flip_w8, "Expanse wins through")

    def split_reader(a, b):
        c = cell(b, "set", "C2", 8)
        c["reader_ci_lower"], c["reader_ci_upper"] = 1.300, 1.400
    refused(split_reader, "Every reader cell's two intervals overlap")

    # W = 16 is outside the heading's claim, so the crossover sentence refuses.
    def crossover_at_16(a, b):
        cell(b, "map", "C1", 16)["writer_verdict"] = "rowex"
    refused(crossover_at_16, "ROWEX wins no writer cell")

    def writer_under_readers(a, b):
        cell(b, "set", "C2", 2)["writer_verdict"] = "rowex"
    refused(writer_under_readers, "Expanse wins every writer cell with readers present")

    def disagree(a, b):
        cell(b, "map", "C2", 0)["reader_verdict"] = "BOUNDARY_RESULT"
    refused(disagree, "Reader-only (W = 0): Expanse wins on both arms")

    def memory_drift(a, b):
        b["memory"][0]["hot_alloc_bytes_per_key"] += 0.01
    refused(memory_drift, "byte-identical")

    # 6. §7.7 claims movement only under rule 18: a reader cell pushed clear of
    #    the earlier pair breaks "no reader cell moved", and is refused.
    def reader_moved(a, b):
        for art in (a, b):
            c = cell(art, "map", "C2", 8)
            c["reader_ci_lower"], c["reader_ci_upper"] = 2.300, 2.400
    refused(reader_moved, "no reader cell moved")

    # 7. §7.9 lists a side only when a run puts it beyond the interval: widen
    #    every interval and only the unconditional cell is left.
    a, b = copy.deepcopy(r1), copy.deepcopy(r2)
    for art in (a, b):
        for c in art["throughput"]:
            for role in ("writer", "reader"):
                if f"{role}_ci_lower" in c:
                    c[f"{role}_ci_lower"], c[f"{role}_ci_upper"] = 0.01, 100.0
    rows = [ln for ln in section_79([(a, b)]) if ln.startswith("| `")]
    if len(rows) != 4 or not all("| map | C1 writer | 1 | 0 |" in ln and ln.endswith("| no |") for ln in rows):
        failures.append(f"§7.9 listed a side no run puts beyond its interval: {rows}")

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
    # A --quick sweep writes the two live names only; the other pairs are
    # committed artifacts and are read where they exist.
    def optional(names):
        return load_runs(names=names) if all((RESULTS / n).is_file() for n in names) else None
    print(render(*load_runs(), percore=optional(PERCORE_RUNS), previous=optional(PREVIOUS_RUNS)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
