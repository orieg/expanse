#!/usr/bin/env python3
"""A foreign-load reading for a process shorter than the load window's minimum.

`driver.run_one` brackets every stress-ng process with a `bench_provenance`
load window, and `analyze.valid` discards a process whose window has no
foreign reading. A process shorter than `bench_provenance.MIN_WINDOW_S` has
none (its window is a jiffy step divided by almost nothing), so an arm whose
processes sit at that length loses rows until its cell has no interval.

The opt-in `--short-window=round` on `analyze.py` and
`diag_increment_analyze.py` gives such a process the reading of its **round
window**: the window that encloses every arm's process of the same cell, W and
round, which the drivers run back to back. Nothing new is recorded for it. The
driver already appends a snapshot to `provenance.loads` before every process
(`bench_provenance.begin_cell`), so the round window runs from the snapshot
before the round's first arm to the next snapshot after its last arm, and is
recomputed here from the artifact's own provenance with `bench_provenance`'s
functions. It is as long as the arms together, so it clears the minimum when
one arm alone does not.

What the reading is, and is not:
- It is the host's busy CPU over the round window, less the runner's
  children's CPU over the same window, in core-equivalents: the per-process
  figure's definition over a longer window.
- It averages over the sibling arms' processes too. Foreign CPU that fell
  entirely inside the short process reads lower here by the row's `dilution`
  (round-window wall time over process wall time), which every fallback row
  carries and the analyzer prints at its largest.
- The runner's own CPU between two processes is inside the window and is not
  a child's, so it counts as foreign: the reading errs towards discarding.
- Snapshots read back from an artifact carry `monotonic_s` and `child_cpu_s`
  rounded to 1 ms (`bench_provenance.Snapshot`).

A process keeps its own window's reading wherever it has one; the round window
is used only where the process window reads nothing. The fallback number is
never written into `load.foreign_busy_cpus`: it stays under `load_fallback`
with its `rule`, so a reader that does not know the rule still sees no reading
(AGENTS.md section 8.1). `label_problems` is the check of that, run on every
artifact the opt-in writes.

Usage:
  short_window.py --self-test
  short_window.py --census <artifact.json> [...]   what the rule would read on
      a committed analyzer artifact (its provenance and `rounds_raw`); prints
      only, writes nothing.
"""
import json
import math
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent.parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))
import bench_provenance as bp

OPT_IN = "--short-window=round"
RULE_PROCESS = "process_window"
RULE_ROUND = "round_window"
RULE_NONE = "unmeasured"
# `driver.run_one`'s snapshot label: cell:<workload>:W<workers>:<arm>:r<round>.
LABEL = re.compile(r"^cell:(?P<cell>.+):W(?P<w>\d+):(?P<arm>[^:]+):r(?P<round>\d+)$")


def round_windows(prov):
    """`{(cell, w, round): window}` for every round window `prov["loads"]` closes.

    A window opens at the snapshot before the first arm of a (cell, W, round)
    and closes at the first snapshot after its last arm: the next round
    window's opening one, or the run's `end`. A group with no later snapshot
    has no window. Figures come from `bench_provenance`, so a window shorter
    than its minimum reads `None` here as it does there.
    """
    ncpu = prov["host"]["cpus_online"]
    if not ncpu:
        raise ValueError("provenance.host.cpus_online is missing: busy CPU cannot be scaled")
    loads = prov["loads"]
    groups, last = [], None
    for i, snap in enumerate(loads):
        m = LABEL.match(snap.get("label") or "")
        key = (m["cell"], int(m["w"]), int(m["round"])) if m else None
        if key is not None and key == last:
            groups[-1][2].append(m["arm"])
        elif key is not None:
            groups.append((key, i, [m["arm"]]))
        last = key
    keys = [key for key, _, _ in groups]
    for key in keys:
        if keys.count(key) > 1:
            raise ValueError(f"{key} opens two round windows: its arms did not run back to back")
    out = {}
    for key, first, arms in groups:
        end = first + len(arms)
        if end >= len(loads):
            continue
        a, b = loads[first], loads[end]
        host = bp.busy_cpus(a, b.get("stat_busy_jiffies"), b.get("stat_total_jiffies"), ncpu)
        own = bp.own_busy_cpus(a, b.get("child_cpu_s"), b["monotonic_s"])
        out[key] = {"rule": RULE_ROUND, "since": a["label"], "until": b["label"], "arms": arms,
                    "wall_s": round(b["monotonic_s"] - a["monotonic_s"], 3),
                    "busy_cpus_since_prev": host, "own_busy_cpus": own,
                    "foreign_busy_cpus": bp.foreign_busy_cpus(host, own)}
    return out


def fallback_for(rec, windows):
    """The `load_fallback` block of a row whose own window has no foreign reading."""
    win = windows.get((rec["cell"], rec["w"], rec["round"]))
    if win is None or rec["arm"] not in win["arms"]:
        return {"rule": RULE_NONE, "foreign_busy_cpus": None,
                "reason": "no round window in provenance.loads encloses this process"}
    if win["foreign_busy_cpus"] is None:
        return dict(win, rule=RULE_NONE,
                    reason=f"the round window is shorter than {bp.MIN_WINDOW_S} s or lacks a counter")
    return dict(win, process_wall_s=round(rec["wall"], 3),
                dilution=round(win["wall_s"] / rec["wall"], 2) if rec["wall"] > 0 else None)


def annotate(recs, prov):
    """Gives every row without a process-window reading its `load_fallback` block.

    The row's `load` is left as the driver wrote it. A row that already has a
    `load_fallback` is refused: only this function writes one.
    """
    refuse_prelabelled(recs)
    windows = round_windows(prov)
    for rec in recs:
        if rec["load"]["foreign_busy_cpus"] is None:
            rec["load_fallback"] = fallback_for(rec, windows)
    return recs


def refuse_prelabelled(recs):
    """Raw rows never carry `load_fallback`; one that does is not read as a reading."""
    n = sum("load_fallback" in r for r in recs)
    if n:
        sys.exit(f"STOP: {n} raw row(s) already carry `load_fallback`; only {OPT_IN} writes it")


def reading(rec):
    """`(foreign core-equivalents or None, the rule that produced it)` for a row."""
    own = rec["load"]["foreign_busy_cpus"]
    if own is not None:
        return own, RULE_PROCESS
    fb = rec.get("load_fallback")
    if fb and fb["rule"] == RULE_ROUND and fb["foreign_busy_cpus"] is not None:
        return fb["foreign_busy_cpus"], RULE_ROUND
    return None, RULE_NONE


def row_fields(rec):
    """What a `rounds_raw` row adds under the opt-in: the rule, and the fallback block."""
    out = {"load_rule": reading(rec)[1]}
    if "load_fallback" in rec:
        out["load_fallback"] = rec["load_fallback"]
    return out


def summary(recs, is_valid, foreign_max):
    """The `provenance.short_window` block: how many rows each rule read."""
    fb = [r for r in recs if reading(r)[1] == RULE_ROUND]
    by = {}
    for r in fb:
        k = f"{r['cell']}:W{r['w']}:{r['arm']}"
        by[k] = by.get(k, 0) + 1
    return {
        "opt_in": OPT_IN, "rule": RULE_ROUND,
        "definition": "a process whose own load window has no foreign reading takes the reading of "
                      "the window enclosing every arm's process of its cell, W and round, from "
                      "provenance.loads; its row says so under load_rule and load_fallback",
        "min_window_s": bp.MIN_WINDOW_S, "foreign_max": foreign_max,
        "rows": len(recs),
        "rows_process_window": sum(reading(r)[1] == RULE_PROCESS for r in recs),
        "rows_round_window": len(fb),
        "rows_round_window_valid": sum(bool(is_valid(r)) for r in fb),
        "rows_unmeasured": sum(reading(r)[1] == RULE_NONE for r in recs),
        "rows_round_window_by_cell_arm": dict(sorted(by.items())),
        "max_round_window_wall_s": max((r["load_fallback"]["wall_s"] for r in fb), default=None),
        "max_dilution": max((r["load_fallback"]["dilution"] for r in fb
                             if r["load_fallback"]["dilution"] is not None), default=None),
        "mean_round_window_foreign": (round(math.fsum(reading(r)[0] for r in fb) / len(fb), 3)
                                      if fb else None),
    }


def summary_line(n, s):
    """The line the analyzer prints per run under the opt-in."""
    cells = ", ".join(f"{k} {v}" for k, v in s["rows_round_window_by_cell_arm"].items()) or "none"
    return (f"run {n}: short-window fallback ({s['rule']}): {s['rows_round_window']} of {s['rows']} "
            f"processes read on their round window, {s['rows_round_window_valid']} of them valid; "
            f"{s['rows_unmeasured']} still without a reading; largest round window "
            f"{s['max_round_window_wall_s']} s, largest dilution {s['max_dilution']}; by cell-arm: {cells}")


def label_problems(cells):
    """Findings for `rounds_raw` rows whose reading does not say which rule produced it.

    Under the opt-in every row names its rule, a row counted valid without a
    process-window reading carries a numeric round-window block, and
    `foreign_busy_cpus` holds a number only where the process window read one.
    """
    out = []
    for c in cells:
        where = f"{c['workload_id']} W{c['w']} {c['metric']}"
        for r in c["rounds_raw"]:
            who = f"{where} round {r['round']} arm {r['arm']}"
            rule, fb, own = r.get("load_rule"), r.get("load_fallback"), r.get("foreign_busy_cpus")
            if rule not in (RULE_PROCESS, RULE_ROUND, RULE_NONE):
                out.append(f"{who}: no load_rule")
            elif (rule == RULE_PROCESS) != (own is not None):
                out.append(f"{who}: load_rule {rule} but foreign_busy_cpus is {own!r}")
            elif rule == RULE_ROUND and not (
                    isinstance(fb, dict) and fb.get("rule") == RULE_ROUND
                    and isinstance(fb.get("foreign_busy_cpus"), (int, float))
                    and isinstance(fb.get("wall_s"), (int, float))):
                out.append(f"{who}: load_rule {rule} without a numeric load_fallback block")
            elif rule == RULE_NONE and r.get("valid"):
                out.append(f"{who}: valid with no reading")
    return out


def require_labelled(cells):
    """Stops an analyzer that is about to write an unlabelled fallback reading."""
    problems = label_problems(cells)
    if problems:
        sys.exit(f"STOP: {len(problems)} row(s) carry a reading without its rule; first: {problems[0]}")


def census(path):
    """Prints what the rule would read on one committed analyzer artifact."""
    art = json.loads(Path(path).read_text())
    windows = round_windows(art["provenance"])
    rows = {}
    for c in art["cells"]:
        for r in c["rounds_raw"]:
            rows[(c["workload_id"], c["w"], r["round"], r["arm"])] = r
    short, read, both = 0, [], []
    for (cell, w, rnd, arm), r in sorted(rows.items()):
        rec = {"cell": cell, "w": w, "round": rnd, "arm": arm, "wall": r["wall_s"]}
        fb = fallback_for(rec, windows)
        if r["foreign_busy_cpus"] is None:
            short += 1
            if fb["rule"] == RULE_ROUND:
                read.append(fb)
        elif fb["rule"] == RULE_ROUND:
            both.append((r["foreign_busy_cpus"], fb["foreign_busy_cpus"]))
    print(f"{Path(path).name}: {len(rows)} processes, {len(windows)} round windows, "
          f"{sum(w['foreign_busy_cpus'] is None for w in windows.values())} of them without a reading")
    print(f"  processes without a process-window reading: {short}; read on their round window: {len(read)}")
    if read:
        f = [x["foreign_busy_cpus"] for x in read]
        print(f"  their round-window foreign: min {min(f)}, max {max(f)}, mean "
              f"{math.fsum(f) / len(f):.3f}; round window {min(x['wall_s'] for x in read)}-"
              f"{max(x['wall_s'] for x in read)} s; dilution up to {max(x['dilution'] for x in read)}")
    if both:
        d = [b - a for a, b in both]
        print(f"  processes with both readings: {len(both)}; round minus process: min {min(d):.2f}, "
              f"max {max(d):.2f}, mean {math.fsum(d) / len(d):.3f}; verdicts at 0.5 differ on "
              f"{sum((a <= 0.5) != (b <= 0.5) for a, b in both)}")


# --------------------------------------------------------------------------- self-test


def _snap(label, busy, total, child, mono):
    return {"label": label, "stat_busy_jiffies": busy, "stat_total_jiffies": total,
            "child_cpu_s": child, "monotonic_s": mono}


def _fixture():
    """A 24-CPU host, one cell, three arms back to back: G 0.08 s, M 0.39 s, S 0.10 s.

    The host is busy with exactly the children's CPU, so the round window's
    foreign reading is 0.0. Jiffies: 24 per tick of wall time, 100 ticks/s.
    """
    t = [100.0, 100.08, 100.47, 100.57]          # snapshot times: before G, M, S, and after S
    child = [5.0, 5.08, 5.47, 5.57]              # one busy CPU's worth of child time
    labels = ["cell:sng_sm_hashjudy_default:W1:G:r0", "cell:sng_sm_hashjudy_default:W1:M:r0",
              "cell:sng_sm_hashjudy_default:W1:S:r0", "cell:sng_judy_1k:W1:G:r0"]
    loads = [_snap("start", 0, 0, 5.0, 100.0)]
    for lab, ti, ci in zip(labels, t, child):
        ticks = round((ti - 100.0) * 100)
        loads.append(_snap(lab, ticks, ticks * 24, ci, ti))
    prov = {"host": {"cpus_online": 24}, "loads": loads}

    def rec(arm, wall, foreign):
        return {"run": 1, "round": 0, "cell": "sng_sm_hashjudy_default", "w": 1, "arm": arm,
                "rc": 0, "failed": False, "wall": wall, "load": {"foreign_busy_cpus": foreign},
                "metrics": {}}
    return prov, [rec("G", 0.08, None), rec("M", 0.39, 0.01), rec("S", 0.10, 0.02)]


def _rows(recs, is_valid, labelled=True):
    """`rounds_raw` rows as the analyzers build them, with or without the opt-in's fields."""
    rows = []
    for r in recs:
        row = {"round": r["round"], "arm": r["arm"], "valid": is_valid(r),
               "foreign_busy_cpus": r["load"]["foreign_busy_cpus"], "wall_s": r["wall"]}
        if labelled:
            row.update(row_fields(r))
        rows.append(row)
    return [{"workload_id": "sng_sm_hashjudy_default", "w": 1, "metric": "wall", "rounds_raw": rows}]


def _self_test():
    import copy
    failures, checks = [], 0

    def check(name, got, want):
        nonlocal checks
        checks += 1
        if got != want:
            failures.append(f"{name}: got {got!r}, want {want!r}")

    def is_valid(r):
        f = reading(r)[0]
        return f is not None and f <= 0.5

    prov, recs = _fixture()
    win = round_windows(prov)
    key = ("sng_sm_hashjudy_default", 1, 0)
    check("one closed round window; the trailing group has no closing snapshot", sorted(win), [key])
    check("its arms, in run order", win[key]["arms"], ["G", "M", "S"])
    check("it spans the three processes", win[key]["wall_s"], 0.57)
    check("host busy over it", win[key]["busy_cpus_since_prev"], 1.0)
    check("children's CPU over it", win[key]["own_busy_cpus"], 1.0)
    check("foreign over it", win[key]["foreign_busy_cpus"], 0.0)

    # Without the opt-in a short process has no reading, and stays discarded.
    check("no fallback, no reading", reading(recs[0]), (None, RULE_NONE))
    annotate(recs, prov)
    check("the short process reads on its round window", reading(recs[0]), (0.0, RULE_ROUND))
    check("its own load block is untouched", recs[0]["load"], {"foreign_busy_cpus": None})
    check("the block records the dilution", recs[0].get("load_fallback", {}).get("dilution"), 7.12)
    check("a measured process keeps its own reading", reading(recs[1]), (0.01, RULE_PROCESS))
    check("and gains no fallback block", "load_fallback" in recs[1], False)
    check("labelled rows pass", label_problems(_rows(recs, is_valid)), [])
    s = summary(recs, is_valid, 0.5)
    check("the summary counts the fallback rows", (s["rows_round_window"], s["rows_round_window_valid"],
                                                  s["rows_unmeasured"], s["rows_process_window"]),
          (1, 1, 0, 2))
    check("and names the cell-arm", s["rows_round_window_by_cell_arm"],
          {"sng_sm_hashjudy_default:W1:G": 1})
    check("the printed line carries the count", "1 of 3 processes read on their round window"
          in summary_line(1, s), True)

    # The motivating defect, pinned: a fallback number with no label. Each of
    # the three silent shapes must be a finding.
    silent = _rows(recs, is_valid, labelled=False)
    check("a valid row with no rule at all", len(label_problems(silent)), 3)
    in_place = _rows(recs, is_valid)
    in_place[0]["rounds_raw"][0]["foreign_busy_cpus"] = 0.0
    check("the fallback number written into foreign_busy_cpus",
          label_problems(in_place)[:1],
          ["sng_sm_hashjudy_default W1 wall round 0 arm G: load_rule round_window but "
           "foreign_busy_cpus is 0.0"])
    stripped = _rows(recs, is_valid)
    stripped[0]["rounds_raw"][0].pop("load_fallback", None)
    check("a round-window row without its block", len(label_problems(stripped)), 1)
    posing = _rows(recs, is_valid)
    posing[0]["rounds_raw"][0]["load_rule"] = RULE_PROCESS
    check("a fallback row labelled as a process window", len(label_problems(posing)), 1)
    counted = _rows(recs, is_valid)
    counted[0]["rounds_raw"][0].update(load_rule=RULE_NONE, valid=True)
    check("a row counted valid with no reading", len(label_problems(counted)), 1)

    # Foreign CPU inside the round window is read, not averaged away to zero:
    # 0.4 CPU-seconds of other work over 0.57 s is 0.7 core-equivalents.
    busy = copy.deepcopy(prov)
    busy["loads"][4]["stat_busy_jiffies"] += 40
    noisy = [dict(r, load=dict(r["load"])) for r in _fixture()[1]]
    annotate(noisy, busy)
    check("a contended round window reads its load", reading(noisy[0]), (0.7, RULE_ROUND))
    check("and the row is discarded", is_valid(noisy[0]), False)

    # A round window that is itself too short stays unmeasured, with its reason.
    quick = copy.deepcopy(prov)
    for i, (ti, ci) in enumerate(((100.0, 5.0), (100.03, 5.03), (100.06, 5.06), (100.09, 5.09)), 1):
        ticks = round((ti - 100.0) * 100)
        quick["loads"][i].update(stat_busy_jiffies=ticks, stat_total_jiffies=ticks * 24,
                                 child_cpu_s=ci, monotonic_s=ti)
    tiny = [dict(r, load={"foreign_busy_cpus": None}) for r in _fixture()[1]]
    annotate(tiny, quick)
    check("a round window under the minimum reads nothing", reading(tiny[0]), (None, RULE_NONE))
    check("and says why", "shorter than" in tiny[0].get("load_fallback", {}).get("reason", ""), True)
    check("so nothing is valid", [is_valid(r) for r in tiny], [False, False, False])
    check("and the rows still pass the label check", label_problems(_rows(tiny, is_valid)), [])

    # A row whose round has no snapshots in the provenance.
    orphan = [dict(_fixture()[1][0], round=7)]
    annotate(orphan, prov)
    check("a process outside every round window", reading(orphan[0]), (None, RULE_NONE))
    # The last group of a truncated provenance never closed.
    last = [dict(_fixture()[1][0], cell="sng_judy_1k")]
    annotate(last, prov)
    check("a round window with no closing snapshot", reading(last[0]), (None, RULE_NONE))

    # Refusals.
    for name, fn in (("a raw row that already carries load_fallback",
                      lambda: annotate(copy.deepcopy(recs), prov)),
                     ("require_labelled on a silent artifact", lambda: require_labelled(silent))):
        checks += 1
        try:
            fn()
            failures.append(f"{name}: did not stop")
        except SystemExit as e:
            if "STOP" not in str(e):
                failures.append(f"{name}: stopped without STOP: {e}")
    split = copy.deepcopy(prov)
    split["loads"].append(_snap("cell:sng_sm_hashjudy_default:W1:S:r0", 60, 1440, 5.6, 100.6))
    split["loads"].append(_snap("end", 61, 1464, 5.6, 100.61))
    for name, bad in (("arms that did not run back to back", split),
                      ("no CPU count", {"host": {"cpus_online": None}, "loads": prov["loads"]})):
        checks += 1
        try:
            round_windows(bad)
            failures.append(f"{name}: no error")
        except ValueError:
            pass

    for f in failures:
        print("FAIL", f)
    if failures:
        print(f"short_window.py --self-test: {len(failures)} of {checks} checks failed")
        return 1
    print(f"short_window.py --self-test: all {checks} checks passed")
    return 0


if __name__ == "__main__":
    if sys.argv[1:] == ["--self-test"]:
        sys.exit(_self_test())
    if len(sys.argv) > 2 and sys.argv[1] == "--census":
        for p in sys.argv[2:]:
            census(p)
        sys.exit(0)
    sys.exit(__doc__)
