#!/usr/bin/env python3
"""Assert the shared chart-theme CSS has not drifted between suites.

Each comparative suite under `docs/benchmarks/<suite>/scripts/` carries its own
`theme.py`. Five of them exist and no two are identical: 47 of ~92-117 lines are
common, and the rest is that suite's own competitor palette, which is correct --
`.b-roaring` belongs to the search suite and nowhere else.

What is NOT correct is the shared chrome silently diverging. Background, border,
grid, axis, divider, every text class and their dark-mode overrides are the same
design system in all five, and a change landing in one copy is invisible in the
other four.

This gate takes the drift-prevention half of that problem without the rewrite.
Consolidating the five into one module was considered and rejected: the SVGs
regenerate byte-identically today, which is the only acceptance test worth
having, and a shared module that emits the same rules in a different ORDER
cannot meet it -- the CSS is embedded in the chart text. A gate keeps the
guarantee and leaves the charts untouched.

A selector is SHARED if it appears in more than one theme. Every shared selector
must have a byte-identical body everywhere it appears. Selectors unique to one
suite are that suite's business and are not checked.

Known-divergent selectors are listed in DIVERGENT below with the reason. They
are reported as warnings, never silently ignored (AGENTS.md 8.1).

    python3 scripts/check_chart_themes.py
    python3 scripts/check_chart_themes.py --self-test
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

THEME_GLOB = "docs/benchmarks/*/scripts/theme.py"

# Selectors that legitimately differ today. Each is a per-suite accent that the
# suite chose, not shared chrome. Listed so the divergence is stated rather than
# discovered; `.b-expanse` is flagged separately below because it is the
# project's own brand colour and arguably should NOT differ.
DIVERGENT: dict[str, str] = {
    # Per-suite accents. A suite picking its own competitor colour is correct.
    ".b-btree": "per-suite competitor accent",
    ".badge-win": "per-suite badge weight",
    ".badge-win-text": "per-suite badge weight",
    ".badge-loss": "per-suite badge weight",
    ".badge-loss-text": "per-suite badge weight",

    # UNREVIEWED. Recorded so the gate is green and the divergence is stated
    # rather than hidden (AGENTS.md 8.1). These are NOT endorsed: each needs a
    # decision, and resolving one re-renders that suite's committed charts,
    # which is why this gate does not do it silently.
    #
    # `.b-expanse` is the project's own brand colour and has two variants across
    # five suites -- the charts do not agree on what Expanse looks like.
    #
    # The five text classes all diverge in exactly ONE suite, llm_inference,
    # which runs ~0.5px larger on every one of them while the other four agree
    # exactly. Either a deliberate choice for that suite's denser charts, or a
    # bump that landed in one copy of five.
    ".b-expanse": "UNREVIEWED, BRAND: 2 variants across 5 suites",
    ".t-title": "UNREVIEWED: llm_inference 12px, the other four 11.5px",
    ".t-sub": "UNREVIEWED: llm_inference 10.5px, the other four 10px",
    ".t-bar-label": "UNREVIEWED: llm_inference 12px, the other four 11px",
    ".t-legend": "UNREVIEWED: llm_inference 11px, the other four 10.5px",
    ".t-axis-label": "UNREVIEWED: llm_inference 10px, the other four 9.5px",
}

RULE_RE = re.compile(r"^\s*((?:\.|:root|\[data-theme)[^\{]*?)\s*\{\{(.*?)\}\}\s*$", re.M)


def parse_rules(text: str) -> dict[str, str]:
    """selector -> normalised body, for one theme module."""
    out: dict[str, str] = {}
    for m in RULE_RE.finditer(text):
        sel = " ".join(m.group(1).split())
        body = " ".join(m.group(2).split())
        out.setdefault(sel, body)
    return out


def base_selector(sel: str) -> str:
    """Strip the dark-mode prefix so `.b-expanse` and its dark form share a key."""
    m = re.search(r"(\.[A-Za-z0-9_-]+)\s*$", sel.split(",")[0])
    return m.group(1) if m else sel


def collect(root: Path) -> dict[str, Path]:
    out = subprocess.run(["git", "ls-files", "--", THEME_GLOB], cwd=root,
                         capture_output=True, text=True, check=True).stdout.split()
    return {p.split("/")[2]: root / p for p in out if p.endswith("theme.py")}


def check(root: Path) -> tuple[list[str], list[str]]:
    themes = collect(root)
    if len(themes) < 2:
        return ([f"expected several suite themes matching {THEME_GLOB}, found {len(themes)}"], [])

    rules: dict[str, dict[str, str]] = {n: parse_rules(p.read_text(encoding="utf-8"))
                                        for n, p in themes.items()}
    by_sel: dict[str, dict[str, str]] = defaultdict(dict)
    for suite, rs in rules.items():
        for sel, body in rs.items():
            by_sel[sel][suite] = body

    errors: list[str] = []
    warnings: list[str] = []
    shared = 0
    for sel, per in sorted(by_sel.items()):
        if len(per) < 2:
            continue
        shared += 1
        if len(set(per.values())) == 1:
            continue
        base = base_selector(sel)
        variants = len(set(per.values()))
        if base in DIVERGENT:
            warnings.append(f"{sel}: {variants} variants across {len(per)} suites — {DIVERGENT[base]}")
            continue
        detail = "; ".join(f"{s}: {b}" for s, b in sorted(per.items()))
        errors.append(
            f"shared selector `{sel}` has {variants} variants across {len(per)} suite themes — "
            f"the shared chart chrome must stay identical, or add it to DIVERGENT in this "
            f"script with the reason. {detail}"
        )
    return errors, warnings + [f"__shared_count__{shared}"]


def self_test() -> int:
    A = 'x = f"""\n  .bg {{ fill: #fff; }}\n  .grid {{ stroke: #eee; }}\n  .b-roaring {{ fill: #f00; }}\n"""\n'
    B = 'x = f"""\n  .bg {{ fill: #fff; }}\n  .grid {{ stroke: #eee; }}\n  .b-art {{ fill: #00f; }}\n"""\n'
    C = 'x = f"""\n  .bg {{ fill: #000; }}\n  .grid {{ stroke: #eee; }}\n"""\n'

    a, b, c = parse_rules(A), parse_rules(B), parse_rules(C)
    assert a[".bg"] == b[".bg"], "identical shared rule must parse equal"
    assert a[".b-roaring"] == "fill: #f00;", a
    assert ".b-art" not in a, "suite-unique selectors must not leak between themes"
    assert c[".bg"] != a[".bg"], "a drifted rule must parse unequal"

    assert base_selector(':root[data-theme="dark"] .bg, [data-theme="dark"] .bg') == ".bg"
    assert base_selector(".b-expanse") == ".b-expanse"

    import tempfile
    with tempfile.TemporaryDirectory() as td:
        root = Path(td)
        for suite, body in (("alpha", A), ("beta", B)):
            d = root / "docs" / "benchmarks" / suite / "scripts"
            d.mkdir(parents=True)
            (d / "theme.py").write_text(body)
        subprocess.run(["git", "init", "-q"], cwd=root, check=True)
        subprocess.run(["git", "add", "-A"], cwd=root, check=True)
        errs, _ = check(root)
        assert not errs, f"identical shared chrome must pass, got {errs!r}"

        # Drift the shared `.bg` in one theme only: that is the whole point.
        (root / "docs" / "benchmarks" / "beta" / "scripts" / "theme.py").write_text(
            B.replace("#fff", "#eee"))
        subprocess.run(["git", "add", "-A"], cwd=root, check=True)
        errs, _ = check(root)
        assert any(".bg" in e for e in errs), f"drifted shared rule must fail, got {errs!r}"

        # A selector listed in DIVERGENT warns instead of failing.
        for suite, fill in (("alpha", "#111"), ("beta", "#222")):
            p = root / "docs" / "benchmarks" / suite / "scripts" / "theme.py"
            p.write_text(f'x = f"""\n  .bg {{{{ fill: #fff; }}}}\n  .b-expanse {{{{ fill: {fill}; }}}}\n"""\n')
        subprocess.run(["git", "add", "-A"], cwd=root, check=True)
        errs, warns = check(root)
        assert not errs, f"DIVERGENT selector must not be fatal, got {errs!r}"
        assert any(".b-expanse" in w for w in warns), f"...but must warn, got {warns!r}"

    print("check_chart_themes.py --self-test: all checks passed")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()

    root = Path(subprocess.run(["git", "rev-parse", "--show-toplevel"],
                               capture_output=True, text=True, check=True).stdout.strip())
    errors, warnings = check(root)
    shared = 0
    for w in warnings:
        if w.startswith("__shared_count__"):
            shared = int(w.removeprefix("__shared_count__"))
            continue
        print(f"::warning::chart theme: {w}")
    for e in errors:
        print(f"::error::{e}")
    listed = [w for w in warnings if not w.startswith("__")]
    unreviewed = [w for w in listed if "UNREVIEWED" in w]
    print(f"check_chart_themes.py: {shared} shared selector(s) across suite themes, "
          f"{len(errors)} drifted, {len(listed)} known-divergent "
          f"({len(unreviewed)} of them UNREVIEWED and awaiting a decision).")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
