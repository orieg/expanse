#!/usr/bin/env python3
"""Checks that a libexpanse build starts every Expanse function on a 64-byte boundary.

The x86_64 Linux release library is built with
`-C llvm-args=-align-all-functions=6` (#1684): the 1 Ki find of the stress-ng
suite moved by 0.96x-1.14x of stock between library builds with identical
instructions, with the hot function's offset inside a 64-byte line
(docs/benchmarks/stressng_comparison/README.md, "Diagnostic: layout-only
builds"). Aligning every function removes that dependence on link order. The
flag reaches only crates compiled in the build: the precompiled standard
library keeps its own alignment, so only symbols of this repository's code are
checked — those whose mangled name contains `expanse`, and the `Judy*` and
`expanse_*` C entry points.

Fails loudly (AGENTS.md section 8.1) on a missing tool, an unreadable file, no
matching symbol, or any matching function off a 64-byte boundary.

Usage:
  python3 scripts/check_function_alignment.py <libexpanse.so> [<more.so> ...]
  python3 scripts/check_function_alignment.py --self-test
"""
from __future__ import annotations

import re
import subprocess
import sys

ALIGN = 64
# `readelf -sW` symbol rows: Num: Value Size Type Bind Vis Ndx Name
ROW = re.compile(r"^\s*\d+:\s+([0-9a-fA-F]+)\s+(\d+)\s+FUNC\s+\S+\s+\S+\s+(\S+)\s+(\S+)")


def ours(name: str) -> bool:
    return "expanse" in name or name.startswith("Judy")


def misaligned(readelf_text: str) -> tuple[int, list[str]]:
    """Counts this repository's defined functions and lists those off ALIGN."""
    seen, bad = set(), []
    for line in readelf_text.splitlines():
        m = ROW.match(line)
        if not m:
            continue
        addr, size, ndx, name = int(m.group(1), 16), int(m.group(2)), m.group(3), m.group(4)
        name = name.split("@")[0]
        if ndx == "UND" or size == 0 or not ours(name) or (addr, name) in seen:
            continue
        seen.add((addr, name))
        if addr % ALIGN:
            bad.append(f"{name} at {addr:#x} ({addr % ALIGN} bytes past a {ALIGN}-byte boundary)")
    return len(seen), bad


def check(path: str) -> bool:
    try:
        out = subprocess.run(["readelf", "-sW", path], check=True, capture_output=True,
                             text=True).stdout
    except FileNotFoundError:
        sys.exit("check_function_alignment: readelf not found (binutils)")
    except subprocess.CalledProcessError as e:
        sys.exit(f"check_function_alignment: readelf failed on {path}: {e.stderr.strip()}")
    n, bad = misaligned(out)
    if n == 0:
        sys.exit(f"check_function_alignment: {path} defines no Expanse function symbol")
    if bad:
        print(f"{path}: {len(bad)} of {n} Expanse functions are not {ALIGN}-byte aligned:")
        for b in bad[:20]:
            print(f"  {b}")
        return False
    print(f"{path}: all {n} Expanse functions start on a {ALIGN}-byte boundary")
    return True


def self_test() -> None:
    good = """
    10: 0000000000043f00   398 FUNC    GLOBAL DEFAULT   12 JudyLGet
    11: 000000000022b640  1614 FUNC    LOCAL  DEFAULT   12 _RNvNtCs1_12expanse_trie3get18locate_slot_popcnt
    12: 0000000000012345    40 FUNC    LOCAL  DEFAULT   12 _ZN4core3fmt5write17h0
    13: 0000000000000000     0 FUNC    GLOBAL DEFAULT  UND malloc@GLIBC_2.2.5
    14: 0000000000043f40    16 FUNC    GLOBAL DEFAULT   12 expanse_map_get@@EXPANSE_1
"""
    n, bad = misaligned(good)
    assert (n, bad) == (3, []), (n, bad)
    # The two addresses #1684 measured: JudyLGet and the hot find function of
    # main's library at 6ec29a656, 48 and 16 bytes past a boundary.
    off = good.replace("0000000000043f00", "0000000000043fb0").replace(
        "000000000022b640", "000000000022b650")
    n, bad = misaligned(off)
    assert n == 3 and len(bad) == 2, (n, bad)
    assert "JudyLGet at 0x43fb0 (48 bytes" in bad[0] and "16 bytes" in bad[1], bad
    # Standard-library and undefined symbols never count.
    n, _ = misaligned("    12: 0000000000012345    40 FUNC    LOCAL  DEFAULT   12 _ZN4core3fmt5write17h0\n")
    assert n == 0
    print("check_function_alignment.py --self-test: all checks passed")


def main(argv: list[str]) -> None:
    if argv == ["--self-test"]:
        self_test()
        return
    if not argv:
        sys.exit(__doc__)
    if not all([check(p) for p in argv]):
        sys.exit(1)


if __name__ == "__main__":
    main(sys.argv[1:])
