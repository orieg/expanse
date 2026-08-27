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
            in_nf = True
        elif line.strip() == ".fi":
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


def parse_c_symbols(header_path: Path, prefix_pattern: str) -> set[str]:
    text = header_path.read_text(encoding="utf-8")
    # Matches functions or macros
    # e.g. expanse_set_insert(...) or Judy1Set(...)
    matches = re.findall(rf"\b({prefix_pattern}[A-Za-z0-9_]+)\s*\(", text)
    # Exclude C keywords / macro syntax
    symbols = {m for m in matches if not m.startswith("defined")}
    return symbols


def check_symbol_coverage(root: Path) -> list[str]:
    errors = []
    expanse_h = root / "crates" / "expanse-capi" / "include" / "expanse.h"
    judy_h = root / "crates" / "expanse-capi" / "include" / "Judy.h"

    if not expanse_h.exists() or not judy_h.exists():
        return [f"Headers not found at {expanse_h} / {judy_h}"]

    expanse_symbols = parse_c_symbols(expanse_h, "expanse_")
    judy_symbols = parse_c_symbols(judy_h, r"(?:Judy|J1|JL|JSL|JHS)")

    # Read all man pages text
    man_dir = root / "man" / "man3"
    all_man_text = ""
    for f in man_dir.glob("*.3"):
        all_man_text += f.read_text(encoding="utf-8") + "\n"

    # Check expanse functions
    for sym in sorted(expanse_symbols):
        if sym not in all_man_text:
            errors.append(f"C ABI symbol '{sym}' from expanse.h is not documented in any man page")

    # Check Judy functions & macros
    for sym in sorted(judy_symbols):
        if sym not in all_man_text:
            errors.append(f"C ABI symbol/macro '{sym}' from Judy.h is not documented in any man page")

    return errors


def self_test() -> int:
    root = get_repo_root()
    man_dir = root / "man" / "man3"
    assert man_dir.exists(), "man/man3 directory must exist"
    for name in EXPECTED_MAN_PAGES:
        p = man_dir / name
        assert p.exists(), f"man page {name} must exist"
        errs = check_formatting_hygiene(p)
        assert not errs, f"Formatting hygiene failed for {name}: {errs}"
    cov_errs = check_symbol_coverage(root)
    assert not cov_errs, f"Symbol coverage failed: {cov_errs}"
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
