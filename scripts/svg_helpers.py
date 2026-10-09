#!/usr/bin/env python3
"""Shared helpers for the SVG chart generators.

`esc` and `save_svg` were copied into several `generate_charts.py` files and
`scripts/generate_asset_svgs.py`. The copies below were byte-for-byte equal in
behaviour, and every generated SVG stays byte-identical after sharing them.

Not shared on purpose: the `esc` in `hot_comparison` and `masstree_comparison`
coerces its argument with `str(s)` first, which accepts non-string cells. This
`esc` does not, so those two keep their own copy.

A generator under `docs/benchmarks/<suite>/scripts/` imports this module with
the same `sys.path.insert(0, <repo>/scripts)` line the other generators use for
`bench_provenance` and `embedded_envelope`.

    python3 scripts/svg_helpers.py --self-test
"""

from __future__ import annotations

import sys
import tempfile
import xml.etree.ElementTree as ET
from pathlib import Path


def esc(s: str) -> str:
    """Escape `&`, `<` and `>` for SVG text content (str input only)."""
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def save_svg(filepath: Path, content: str) -> None:
    """Write `content` to `filepath` after checking it is well-formed XML.

    A malformed document raises `ET.ParseError` and writes nothing.
    """
    try:
        ET.fromstring(content)
    except ET.ParseError as err:
        print(f"XML validation error in {filepath.name}: {err}")
        raise
    with open(filepath, "w", encoding="utf-8") as f:
        f.write(content)
    print(f"Generated & validated: {filepath}")


def _self_test() -> int:
    assert esc("a & b < c > d") == "a &amp; b &lt; c &gt; d"
    # `&` is escaped first, so an existing entity is escaped, not preserved.
    assert esc("&lt;") == "&amp;lt;"
    assert esc("plain") == "plain"
    with tempfile.TemporaryDirectory() as d:
        ok = Path(d) / "ok.svg"
        save_svg(ok, '<svg xmlns="http://www.w3.org/2000/svg"/>')
        assert ok.read_text(encoding="utf-8") == '<svg xmlns="http://www.w3.org/2000/svg"/>'
        bad = Path(d) / "bad.svg"
        try:
            save_svg(bad, "<svg><unclosed></svg>")
        except ET.ParseError:
            pass
        else:
            raise AssertionError("malformed SVG was accepted")
        assert not bad.exists(), "malformed SVG must not be written"
    print("svg_helpers self-test: ok")
    return 0


if __name__ == "__main__":
    if sys.argv[1:] == ["--self-test"]:
        sys.exit(_self_test())
    sys.exit("usage: svg_helpers.py --self-test")
