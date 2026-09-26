#!/usr/bin/env python3
"""
scripts/compact_bounds.py — the bounds behind an explicit `compact()` for the
64-bit `ExpanseSet` and `ExpanseMap`
(docs/benchmarks/remove_retention/METHODOLOGY.md §12), as committed,
unit-tested code (AGENTS.md §8.8 commit 1).

`compact()` builds a fresh tree of the surviving keys into a new allocator from
the ordered iteration, swaps it in and drops the old one. What that holds, what
it peaks at and what it may cost follow from how the allocator carves slab
pages (crates/expanse/src/alloc.rs, `NodeAlloc::alloc_raw` and
`NodeAlloc::census`):

  * `no_free_held` — the `mem_held()` of an allocator that has only ever
    allocated. A slab class carves a new 4 KiB page only when its freelist is
    empty, so with no free every class has exactly ceil(live / blocks_per_page)
    pages, and the system-served classes hold their live bytes and no free
    block. This is the census "dense floor" plus the system-served live bytes,
    and it is a floor for any allocator holding the same live blocks.
  * `predicted_compact_over_fresh` — the G-held prediction: a builder that
    never frees, emitting the fresh build's per-class live blocks, holds
    `no_free_held(fresh census)`, so its ratio to the fresh build's
    `mem_held()` is at most 1.
  * `peak_no_free`, `peak_ceiling` — the G-peak bound: the old tree's held
    bytes plus the new tree's, when the new allocator never frees during the
    build and the old tree is dropped after it; the gate's ceiling is
    `held_before + slack * held_fresh`.
  * `sort_buffer_bytes` — the transient key buffer `compact()` collects outside
    both allocators (8 B per key for a set, 16 B per entry for a map), which
    G-peak does not count and the pre-registration reports as an expected loss.
  * `per_key`, `cost_gate_met` — G-cost's ceiling is the matching
    `*_rebuild_drained` Callgrind arm in the same CI run. No instruction model
    is derived here: the cost is an empirical residual.

What is not derivable, and stays an empirical residual (METHODOLOGY §12.8):
that the map bulk builder emits the insert path's per-class node census (its
total `mem_used()` is checked by a unit test, the per-class split by the
instrument); the builder's own scratch; and every instruction count.

`test_engine_sync` reads the slab page size, the page header and the census's
`held` body from alloc.rs and fails when the engine moves (with a negative
control per check). `test_artifact_pins` holds `no_free_held` to the committed
census artifact: the set's rebuild is `from_sorted_iter`, which never frees, so
its held bytes must equal the formula on every set cell, and every census must
hold at least its floor.

Usage:
  python3 scripts/compact_bounds.py            # run the pinned tests, then print the prediction table
  python3 scripts/compact_bounds.py --self-test
"""

from __future__ import annotations

import json
import math
import re
import sys
from pathlib import Path

SLAB_PAGE_SIZE = 4096
CACHE_LINE = 64
SLAB_HEADER = CACHE_LINE
FLAVORS = ("set", "map")
# The G-held and G-peak slack (METHODOLOGY §12.5), a pre-registered constant.
G_SLACK = 1.10
# Bytes per surviving key in the sorted buffer `compact()` collects:
# `Vec<u64>` for a set, `Vec<(u64, u64)>` for a map, at exact capacity.
BUFFER_BYTES_PER_KEY = {"set": 8, "map": 16}

REPO = Path(__file__).resolve().parent.parent
ALLOC_RS = "crates/expanse/src/alloc.rs"
CENSUS_ARTIFACT = "docs/benchmarks/remove_retention/results/census_rebuild.json"


def slab_blocks(block: int) -> int:
    """Blocks carved from one slab page for a class whose accounted block size
    is `block` (`alloc::slab_blocks`)."""
    if block <= 0 or block > SLAB_PAGE_SIZE - SLAB_HEADER:
        raise ValueError("block in 1..=page size minus header")
    return (SLAB_PAGE_SIZE - SLAB_HEADER) // block


def floor_pages(live_blocks: int, blocks_per_page: int) -> int:
    """The fewest slab pages `live_blocks` of one class fit on."""
    if live_blocks < 0 or blocks_per_page <= 0:
        raise ValueError("live_blocks >= 0, blocks_per_page > 0")
    return -(-live_blocks // blocks_per_page)


def no_free_held(classes, system_live_bytes: int) -> int:
    """`mem_held()` of an allocator that never freed a block.

    `classes` is an iterable of (live_blocks, blocks_per_page), one per slab
    class. With no free, a class's freelist holds only the uncarved rest of its
    newest page, and a page is carved only when that freelist is empty
    (`NodeAlloc::alloc_raw`), so the class holds ceil(live / blocks_per_page)
    pages. The system-served classes hold their live bytes and, with no free,
    no freelist block."""
    if system_live_bytes < 0:
        raise ValueError("system_live_bytes >= 0")
    pages = sum(floor_pages(live, bpp) for live, bpp in classes)
    return pages * SLAB_PAGE_SIZE + system_live_bytes


def census_floor(census: dict) -> int:
    """`no_free_held` of the live blocks a census records (the artifact's
    `census.*` objects: `classes[].live_blocks`, `classes[].blocks_per_page`,
    `system_live_bytes`)."""
    return no_free_held(
        ((c["live_blocks"], c["blocks_per_page"]) for c in census["classes"]),
        census["system_live_bytes"],
    )


def census_held(census: dict) -> int:
    """`AllocCensus::held`: slab pages at 4 KiB plus the system-served live and
    free bytes."""
    return census["slab_pages"] * SLAB_PAGE_SIZE + census["system_live_bytes"] + census["system_free_bytes"]


def predicted_compact_over_fresh(fresh_census: dict, held_fresh: int) -> float:
    """Predicted `mem_held()` after `compact()` over the fresh build's
    `mem_held()`, for a builder that never frees and emits the fresh build's
    per-class live blocks. At most 1: the fresh build holds at least the floor
    of its own live blocks."""
    if held_fresh <= 0:
        raise ValueError("held_fresh > 0")
    return census_floor(fresh_census) / held_fresh


def peak_no_free(held_before: int, held_after: int) -> int:
    """Peak bytes held by the two tree allocators during `compact()`: the old
    tree is only read until the new one is complete, and the new allocator
    never frees during the build, so its held bytes rise monotonically to
    `held_after`."""
    if held_before < 0 or held_after < 0:
        raise ValueError("held bytes >= 0")
    return held_before + held_after


def peak_ceiling(held_before: int, held_fresh: int, slack: float = G_SLACK) -> float:
    """G-peak's ceiling: held_before + slack * held_fresh."""
    if held_before < 0 or held_fresh <= 0 or slack < 1.0:
        raise ValueError("held_before >= 0, held_fresh > 0, slack >= 1")
    return held_before + slack * held_fresh


def sort_buffer_bytes(n: int, flavor: str) -> int:
    """Requested bytes of the sorted key buffer `compact()` collects outside the
    tree allocators, at exact capacity (`Vec::with_capacity(len)`)."""
    if n < 0 or flavor not in FLAVORS:
        raise ValueError("n >= 0, flavor set or map")
    return n * BUFFER_BYTES_PER_KEY[flavor]


def per_key(ir: int, keys: int) -> float:
    """Instructions per surviving key."""
    if ir < 0 or keys <= 0:
        raise ValueError("ir >= 0, keys > 0")
    return ir / keys


def cost_gate_met(compact_ir: int, rebuild_ir: int) -> bool:
    """G-cost: the compact arm is at or below the matching rebuild arm measured
    in the same CI run. Exact counts, so equality meets the gate."""
    if compact_ir <= 0 or rebuild_ir <= 0:
        raise ValueError("instruction counts must be positive")
    return compact_ir <= rebuild_ir


# ---------------------------------------------------------------------------
# Engine source sync
# ---------------------------------------------------------------------------
def _strip_comments(text: str) -> str:
    return re.sub(r"//[^\n]*", "", text)


def _fn_body(text: str, name: str) -> str | None:
    m = re.search(rf"\bfn {name}\b[^{{]*\{{", text)
    if not m:
        return None
    depth, i = 1, m.end()
    while depth and i < len(text):
        depth += {"{": 1, "}": -1}.get(text[i], 0)
        i += 1
    return re.sub(r"\s+", "", _strip_comments(text[m.end():i - 1]))


ENGINE_BODIES = (
    ("slab_blocks", "let(bytes,align)=CLASS_SPECS[class];(SLAB_PAGE_SIZE-SLAB_HEADER)/accounted_size(bytes,align)"),
    ("held", "self.slab.iter().map(|c|c.pages*SLAB_PAGE_SIZE).sum::<usize>()+self.system_live_bytes"
             "+self.system_free_bytes()"),
)


def engine_source_problems(text: str) -> list[str]:
    """Every mismatch between this module and alloc.rs, as text. Takes the file
    contents so the self-test can hand in a mutated copy."""
    problems = []
    src = _strip_comments(text)
    for name, want in (("SLAB_PAGE_SIZE", SLAB_PAGE_SIZE),):
        m = re.search(rf"^const {name}: usize = (\d+);", src, re.M)
        if not m:
            problems.append(f"alloc.rs: `const {name}: usize = <literal>;` not found")
        elif int(m.group(1)) != want:
            problems.append(f"alloc.rs: {name} = {m.group(1)}, model has {want}")
    if not re.search(r"^const SLAB_HEADER: usize = CACHE_LINE;", src, re.M):
        problems.append("alloc.rs: SLAB_HEADER is no longer CACHE_LINE")
    for fn, want in ENGINE_BODIES:
        got = _fn_body(text, fn)
        if got != want:
            problems.append(f"alloc.rs: `fn {fn}` body is {got!r}, model mirrors {want!r}")
    # A page is carved only once the class freelist is empty: the carve sits
    # after the freelist pop returns, inside `alloc_raw`.
    body = _fn_body(text, "alloc_raw") or ""
    pop = body.find("self.freelists[class].store(next,Ordering::Relaxed);")
    carve = body.find("alloc_zeroed(page_layout)")
    if pop < 0 or carve < 0 or carve < pop:
        problems.append("alloc.rs: `alloc_raw` no longer pops the freelist before carving a page")
    return problems


def test_engine_sync() -> None:
    text = (REPO / ALLOC_RS).read_text()
    problems = engine_source_problems(text)
    assert not problems, "\n".join(problems)
    mutations = (
        ("const SLAB_PAGE_SIZE: usize = 4096;", "const SLAB_PAGE_SIZE: usize = 8192;", "SLAB_PAGE_SIZE"),
        ("const SLAB_HEADER: usize = CACHE_LINE;", "const SLAB_HEADER: usize = 2 * CACHE_LINE;", "SLAB_HEADER"),
        ("(SLAB_PAGE_SIZE - SLAB_HEADER) / accounted_size(bytes, align)",
         "(SLAB_PAGE_SIZE - 2 * SLAB_HEADER) / accounted_size(bytes, align)", "slab_blocks"),
        ("+ self.system_free_bytes()\n", "\n", "fn held"),
        ("let page_raw = unsafe { alloc_zeroed(page_layout) };",
         "let page_raw = unsafe { core::ptr::null_mut::<u8>() };", "alloc_raw"),
    )
    for old, new, expect in mutations:
        assert old in text, f"negative control out of date: {old!r} not in alloc.rs"
        found = engine_source_problems(text.replace(old, new, 1))
        assert any(expect in p for p in found), f"mutation {old!r} -> {new!r} not reported: {found}"


# ---------------------------------------------------------------------------
# Pins
# ---------------------------------------------------------------------------
def _cells() -> list[dict]:
    path = REPO / CENSUS_ARTIFACT
    if not path.is_file():
        raise FileNotFoundError(f"{path}: committed census artifact not found")
    return json.loads(path.read_text())["cells"]


def test_artifact_pins() -> None:
    """`no_free_held` against the committed census (measured: Apple M1,
    `a154bc57`). The set's rebuild is `from_sorted_iter`, which never frees:
    its held bytes equal the formula on every set cell. The map's rebuild is an
    ascending insert, which frees as leaves step up their size classes: it
    holds at least the formula. Every census holds at least its floor, and the
    predicted ratio is at most 1 everywhere."""
    cells = _cells()
    assert len(cells) == 42, len(cells)
    for c in cells:
        for tree in ("drained", "shrunk", "fresh", "rebuilt"):
            cs = c["census"][tree]
            assert census_held(cs) >= census_floor(cs), (c["cell"], c["flavor"], tree)
        rb = c["census"]["rebuilt"]
        assert census_held(rb) == c["held_rebuilt"], (c["cell"], c["flavor"])
        if c["flavor"] == "set":
            assert census_floor(rb) == c["held_rebuilt"], (c["cell"], "set rebuild is no-free")
        else:
            assert census_floor(rb) <= c["held_rebuilt"], (c["cell"], "map floor")
        assert census_held(c["census"]["fresh"]) == c["held_fresh"]
        p = predicted_compact_over_fresh(c["census"]["fresh"], c["held_fresh"])
        assert 0.0 < p <= 1.0, (c["cell"], c["flavor"], p)
    by = {(c["cell"], c["flavor"]): c for c in cells}
    # Reference values, computed from the artifact by the functions above.
    pins = {
        ("headline", "set"): 0.6595,
        ("headline", "map"): 0.7180,
        ("r64_range", "set"): 0.8100,
        ("r64_range", "map"): 0.8394,
        ("sparse_shuffled", "map"): 0.9982,
        ("seq_range", "set"): 0.5770,
    }
    for key, want in pins.items():
        c = by[key]
        got = predicted_compact_over_fresh(c["census"]["fresh"], c["held_fresh"])
        assert round(got, 4) == want, (key, got)
    worst = max(predicted_compact_over_fresh(c["census"]["fresh"], c["held_fresh"]) for c in cells)
    assert round(worst, 4) == 0.9982, worst
    # G-peak on the headline set with the rebuild as the stand-in for the new
    # tree: 94,650,432 B held drained (no shrink) + 8,368,192 B rebuilt.
    h = by[("headline", "set")]
    assert h["held_drained"] == 94_650_432 and h["held_rebuilt"] == 8_368_192
    assert peak_no_free(h["held_drained"], h["held_rebuilt"]) == 103_018_624
    assert peak_no_free(h["held_drained"], h["held_rebuilt"]) <= peak_ceiling(h["held_drained"], h["held_fresh"])


def test_pins() -> None:
    assert slab_blocks(16) == 252
    assert slab_blocks(128) == 31
    assert slab_blocks(256) == 15
    assert slab_blocks(32) == 126
    assert floor_pages(0, 31) == 0
    assert floor_pages(31, 31) == 1
    assert floor_pages(32, 31) == 2
    # One class at 31 blocks per page with 62 live, one at 252 with 1 live,
    # 4,160 B of system-served live bytes: 3 pages + 4,160 B.
    assert no_free_held([(62, 31), (1, 252)], 4160) == 3 * 4096 + 4160
    assert no_free_held([], 0) == 0
    assert peak_no_free(100, 40) == 140
    assert peak_ceiling(100, 40) == 100 + 1.10 * 40
    assert math.isclose(peak_ceiling(0, 10, 1.0), 10.0)
    assert sort_buffer_bytes(1_000_000, "set") == 8_000_000
    assert sort_buffer_bytes(1_000_000, "map") == 16_000_000
    assert sort_buffer_bytes(62_500, "map") == 1_000_000
    # The rebuild arms at `f9782220` (README §3.4): 271.4 and 645.3 per key.
    assert round(per_key(16_960_355, 62_500), 1) == 271.4
    assert round(per_key(40_329_898, 62_500), 1) == 645.3
    assert cost_gate_met(40_329_898, 40_329_898)
    assert cost_gate_met(16_960_354, 16_960_355)
    assert not cost_gate_met(16_960_356, 16_960_355)
    for bad in (
        lambda: slab_blocks(0),
        lambda: slab_blocks(4096),
        lambda: floor_pages(-1, 1),
        lambda: floor_pages(1, 0),
        lambda: no_free_held([], -1),
        lambda: peak_no_free(-1, 0),
        lambda: peak_ceiling(0, 0),
        lambda: peak_ceiling(0, 1, 0.9),
        lambda: sort_buffer_bytes(-1, "set"),
        lambda: sort_buffer_bytes(1, "strmap"),
        lambda: per_key(1, 0),
        lambda: cost_gate_met(0, 1),
        lambda: predicted_compact_over_fresh({"classes": [], "system_live_bytes": 0}, 0),
    ):
        try:
            bad()
        except ValueError:
            pass
        else:
            raise AssertionError("invalid input must raise")


def main(argv: list[str]) -> int:
    test_engine_sync()
    test_pins()
    test_artifact_pins()
    if "--self-test" in argv:
        print("compact_bounds: self-test OK")
        return 0
    print(f"predicted mem_held after compact() / held_fresh (no-free builder, fresh census); "
          f"G-held ceiling {G_SLACK}")
    print(f"source: {CENSUS_ARTIFACT}")
    print(f"{'cell':<18} {'fl':<3} {'predicted':>9} {'peak / held_before':>18} {'buffer (B)':>11}")
    for c in _cells():
        p = predicted_compact_over_fresh(c["census"]["fresh"], c["held_fresh"])
        after = census_floor(c["census"]["fresh"])
        peak = peak_no_free(c["held_drained"], after) / c["held_drained"]
        buf = sort_buffer_bytes(c["m"], c["flavor"])
        print(f"{c['cell']:<18} {c['flavor']:<3} {p:>9.4f} {peak:>18.3f} {buf:>11,}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
