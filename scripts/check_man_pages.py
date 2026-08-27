#!/usr/bin/env python3
"""
scripts/check_man_pages.py — Validation and consistency check for Section 3 man pages.

Verifies:
  1. All expected Section 3 manual pages exist in man/man3/.
  2. Formatting hygiene (lines <= 80 bytes, matching .nf/.fi blocks, valid .TH/.SH macros).
  3. mandoc lint passes cleanly if mandoc is present.
  4. Every C ABI symbol declared in crates/expanse-capi/include/expanse.h and
     crates/expanse-capi/include/Judy.h is documented in the manual pages.

Usage:
  python3 scripts/check_man_pages.py
  python3 scripts/check_man_pages.py --self-test
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

EXPECTED_MAN_PAGES = [
    "expanse.3",
    "expanse_set.3",
    "expanse_map.3",
    "expanse_strmap.3",
    "expanse_bytesmap.3",
    "expanse_sync.3",
    "expanse_blob_map.3",
    "Judy.3",
    "Judy1.3",
    "JudyL.3",
    "JudySL.3",
    "JudyHS.3",
]


def get_repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def check_formatting_hygiene(man_path: Path) -> list[str]:
    errors = []
    text = man_path.read_text(encoding="utf-8")
    lines = text.splitlines()

    in_nf = False
    has_th = False

    for idx, line in enumerate(lines, start=1):
        # Line length check (troff standard recommendation)
        if len(line.encode("utf-8")) > 80:
            errors.append(f"{man_path.name}:{idx}: line length {len(line.encode('utf-8'))} exceeds 80 bytes")

        if line.startswith(".TH"):
            has_th = True
        elif line.strip() == ".nf":
            # A toggle would read `.nf .nf .fi` as balanced; track nesting so a
            # dropped `.fi` is caught wherever it happens.
            if in_nf:
                errors.append(f"{man_path.name}:{idx}: nested .nf (previous block never closed with .fi)")
            in_nf = True
        elif line.strip() == ".fi":
            if not in_nf:
                errors.append(f"{man_path.name}:{idx}: .fi without a matching .nf")
            in_nf = False

    if not has_th:
        errors.append(f"{man_path.name}: missing .TH title macro")
    if in_nf:
        errors.append(f"{man_path.name}: unclosed .nf block (missing .fi)")

    return errors


def check_mandoc(man_path: Path) -> list[str]:
    mandoc_bin = shutil.which("mandoc")
    if not mandoc_bin:
        return []

    res = subprocess.run([mandoc_bin, "-Tlint", str(man_path)], capture_output=True, text=True)
    if res.returncode != 0:
        return [f"{man_path.name}: mandoc lint error:\n{res.stderr or res.stdout}"]
    return []


# A parser that silently matches nothing would report "100% coverage" over an
# empty set. These floors are well below the real counts (100 / 88) and exist
# only to turn a broken regex into a failure instead of a vacuous pass.
MIN_EXPANSE_SYMBOLS = 80
MIN_JUDY_SYMBOLS = 60


def parse_c_symbols(header_path: Path, prefix_pattern: str) -> set[str]:
    text = header_path.read_text(encoding="utf-8")
    # Matches functions or macros
    # e.g. expanse_set_insert(...) or Judy1Set(...)
    matches = re.findall(rf"\b({prefix_pattern}[A-Za-z0-9_]+)\s*\(", text)
    # Exclude C keywords / macro syntax
    symbols = {m for m in matches if not m.startswith("defined")}
    return symbols


def documents_symbol(man_text: str, symbol: str) -> bool:
    """True when `symbol` appears as a whole identifier.

    A plain substring test lets a longer name stand in for a shorter one —
    `expanse_map_get` would be "documented" by `expanse_map_get_batch` alone.
    Eight such prefix pairs exist in expanse.h today.
    """
    return re.search(rf"\b{re.escape(symbol)}(?![A-Za-z0-9_])", man_text) is not None


def check_symbol_coverage(root: Path) -> list[str]:
    errors = []
    expanse_h = root / "crates" / "expanse-capi" / "include" / "expanse.h"
    judy_h = root / "crates" / "expanse-capi" / "include" / "Judy.h"

    if not expanse_h.exists() or not judy_h.exists():
        return [f"Headers not found at {expanse_h} / {judy_h}"]

    expanse_symbols = parse_c_symbols(expanse_h, "expanse_")
    judy_symbols = parse_c_symbols(judy_h, r"(?:Judy|J1|JL|JSL|JHS)")

    if len(expanse_symbols) < MIN_EXPANSE_SYMBOLS:
        errors.append(
            f"only {len(expanse_symbols)} symbols parsed from {expanse_h.name} "
            f"(expected >= {MIN_EXPANSE_SYMBOLS}) — the header format likely changed and the "
            f"coverage check is no longer reading it; fix the parser rather than lowering the floor"
        )
    if len(judy_symbols) < MIN_JUDY_SYMBOLS:
        errors.append(
            f"only {len(judy_symbols)} symbols parsed from {judy_h.name} "
            f"(expected >= {MIN_JUDY_SYMBOLS}) — see above"
        )
    if errors:
        return errors

    # Read all man pages text
    man_dir = root / "man" / "man3"
    all_man_text = ""
    for f in man_dir.glob("*.3"):
        all_man_text += f.read_text(encoding="utf-8") + "\n"

    # Check expanse functions
    for sym in sorted(expanse_symbols):
        if not documents_symbol(all_man_text, sym):
            errors.append(f"C ABI symbol '{sym}' from expanse.h is not documented in any man page")

    # Check Judy functions & macros
    for sym in sorted(judy_symbols):
        if not documents_symbol(all_man_text, sym):
            errors.append(f"C ABI symbol/macro '{sym}' from Judy.h is not documented in any man page")

    return errors


def self_test() -> int:
    """Exercise the checkers against synthetic inputs.

    Re-running the real checks over the real man pages (what this did before)
    proves nothing about the checkers and duplicates the plain invocation that
    CI already runs; it would pass just as happily if a checker had been broken
    into a no-op. These assert both the true and the false positives.
    """
    import tempfile

    with tempfile.TemporaryDirectory() as td:
        good = Path(td) / "good.3"
        good.write_text('.TH GOOD 3 "d" "s" "m"\n.SH NAME\ngood \\- x\n.nf\ntext\n.fi\n')
        assert not check_formatting_hygiene(good), "clean page must pass"

        no_th = Path(td) / "no_th.3"
        no_th.write_text(".SH NAME\nx\n")
        assert any("missing .TH" in e for e in check_formatting_hygiene(no_th)), "missing .TH must fail"

        unclosed = Path(td) / "unclosed.3"
        unclosed.write_text('.TH U 3 "d" "s" "m"\n.nf\ntext\n')
        assert any("unclosed .nf" in e for e in check_formatting_hygiene(unclosed)), "unclosed .nf must fail"

        nested = Path(td) / "nested.3"
        nested.write_text('.TH N 3 "d" "s" "m"\n.nf\na\n.nf\nb\n.fi\n')
        assert any("nested .nf" in e for e in check_formatting_hygiene(nested)), "nested .nf must fail"

        long_line = Path(td) / "long.3"
        long_line.write_text('.TH L 3 "d" "s" "m"\n' + ("x" * 81) + "\n")
        assert any("exceeds 80 bytes" in e for e in check_formatting_hygiene(long_line)), "long line must fail"

    # Word-boundary coverage: a longer symbol must not satisfy a shorter one.
    assert documents_symbol("see expanse_map_get for details", "expanse_map_get")
    assert not documents_symbol("only expanse_map_get_batch here", "expanse_map_get"), \
        "a longer symbol must not count as documenting the shorter one"
    assert documents_symbol(".BI expanse_map_get(", "expanse_map_get"), "signature form must count"

    # The floors must actually be enforced against the live headers.
    root = get_repo_root()
    expanse_h = root / "crates" / "expanse-capi" / "include" / "expanse.h"
    if expanse_h.exists():
        n = len(parse_c_symbols(expanse_h, "expanse_"))
        assert n >= MIN_EXPANSE_SYMBOLS, f"parser regressed: {n} symbols < floor {MIN_EXPANSE_SYMBOLS}"

    print("check_man_pages.py --self-test: all checks passed")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Section 3 Man Pages Validator")
    parser.add_argument("--self-test", action="store_true", help="Run self-tests and exit")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    root = get_repo_root()
    man_dir = root / "man" / "man3"

    errors = []

    if not man_dir.exists():
        print(f"::error::man/man3 directory does not exist at {man_dir}")
        return 1

    # Cross-check both directions. The expected list catches a deleted page; the
    # reverse catches a page added to man/man3 that nobody wired into packaging
    # or this list — the drift the fuzz-target registration self-check exists to
    # prevent, and that the hand-listed nightly fuzz matrix actually suffered.
    on_disk = {p.name for p in man_dir.glob("*.3")}
    for extra in sorted(on_disk - set(EXPECTED_MAN_PAGES)):
        errors.append(
            f"man page '{extra}' exists on disk but is not in EXPECTED_MAN_PAGES — add it there "
            f"and to the deb/rpm packaging globs, or remove it"
        )

    for name in EXPECTED_MAN_PAGES:
        p = man_dir / name
        if not p.exists():
            errors.append(f"Missing expected manual page: {name}")
            continue

        errors.extend(check_formatting_hygiene(p))
        errors.extend(check_mandoc(p))

    errors.extend(check_symbol_coverage(root))

    if errors:
        print("::error::Man pages validation failed with errors:")
        for err in errors:
            print(f"  - {err}")
        return 1

    print(f"✓ All {len(EXPECTED_MAN_PAGES)} Section 3 man pages validated with 100% C ABI symbol coverage.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
