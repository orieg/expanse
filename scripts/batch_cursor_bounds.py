#!/usr/bin/env python3
"""Mathematical bounds for the validated batch cursor on concurrent map readers (#1142).

This module implements pure bound functions with unit tests pinning known
reference values (AGENTS.md §8.8 commit 1, §1.3), reconciled with the concurrency
and optimistic lock coupling model in `scripts/olc_bounds.py` and `crates/expanse/src/sync_nav.rs`.
The pre-registration in `docs/benchmarks/concurrency/METHODOLOGY.md` §32 invokes
these functions rather than restating hand arithmetic.

The problem:
  `SyncExpanseMap` reader handles expose single-key ordered operations
  (`first`, `last`, `next_at_or_after`, `next_after`, `prev_at_or_before`,
  `prev_before`). Each call pins the epoch, samples the tree version, and
  descends from the root through `sync_nav::next_validated`. A K-entry scan
  using `next_after` pays K epoch pins and K root descents.

The validated batch cursor architecture:
  1. Pins once per batch.
  2. Copies one terminal's `(key, u64)` entries using raw atomic loads into a
     cursor-owned buffer.
  3. Validates the terminal's direct parent cover. In Expanse, terminal leaves
     carry no version word of their own (`docs/ARCHITECTURE.md:589`,
     `crates/expanse/src/sync_nav.rs:26-29`); their payloads are covered by their
     parent branch's version word (`Holder::Node(vp, snap)`, `sync_nav.rs:101-114`),
     which brackets every in-place store (`crates/expanse/src/mutate_map.rs:1339, 1485, 1500, 1684`).
     Draining a terminal leaf under its covering parent's version word validates
     strictly 1 branch node (`terminal_drain_read_set_branches() = 1`), paying 2 version
     loads and 1 acquire fence (`olc_bounds.version_word_cost(1, retained=False) = (2, 1)`).
  4. Steps to the next sibling under the parent's retained version.
     Under the lazy census rollup (`docs/ARCHITECTURE.md:591`), child mutations do not
     move parent or ancestor version words. However, filling an empty slot in a branch
     rewrites the branch's digit and edge arrays inside that branch's own bracket
     (`crates/expanse/src/mutate_map.rs:1868, 1904, 2000, 2140`). Therefore, the parent branch's
     version word covers the absence of any skipped empty sibling.
     If navigation enters a non-empty child subtree, that child has its own version word
     and may mutate without moving the parent; hence any entered branch is retained in
     the read set (`sync_nav.rs:407, 442, 490`), bounded by `ordered_read_set_branches(l) <= 13`
     (`scripts/olc_bounds.py`), within `READ_SET_CAP = 16` (`sync_nav.rs:46`), and re-validated
     at the end (`sync_nav.rs:209`).
  5. Re-descends from the root across batches by key (`next_at_or_after`). Node pointers
     cannot be retained safely once unpinned, as epoch reclamation permits retired nodes
     to be freed or reused.

Key derivations settled in this module:
  - Terminal buffer capacity bound: in Expanse's digital trie, the widest terminal is
    a level-1 bitmap leaf (`LeafB1`), which represents an 8-bit byte expanse and holds
    at most 2^8 = 256 entries (`docs/ARCHITECTURE.md:46`, `crates/expanse/src/leaf.rs`).
    At 16 bytes per entry (`(u64, u64)`), a 4096-byte (4 KiB) buffer strictly bounds
    any terminal in the trie.
  - 4 KiB buffer vs narrow buffer: a 4 KiB buffer eliminates mid-leaf splits entirely
    (0 extra descents across all valid populations P in [0, 256]), preserving terminal-level
    atomicity under the parent bracket. In contrast, any buffer B < 256 requires
    `ceil(P / B) - 1` extra root descents, breaks terminal atomicity across batches,
    and incurs additional epoch pins, tree descents, and version validations.
  - Terminal drain vs unbatched scan: draining all P entries of a terminal leaf performs
    1 leaf validation under the covering branch version (read set 1, 2 version loads, 1 fence).
    An unbatched `next_after` scan unpins after each key and performs P full root descents
    (each traversing 7 branch nodes from levels 8 down to 2, or backtracks up to 13 branches).

Sources:
  Leis, Scheibner, Kemper & Neumann, "The ART of Practical Synchronization",
    DaMoN 2016 (optimistic lock coupling; version validation; parent cover).
  `docs/ARCHITECTURE.md` §4.1 (concurrent reads, covering function, ordered reads),
  `crates/expanse/src/sync_nav.rs` (retained read set, ordered search, READ_SET_CAP),
  `crates/expanse/src/mutate_map.rs` (in-place leaf stores, branch slot insertions),
  `scripts/olc_bounds.py` (read set sizes and version word cost model),
  Issue #1142.

Usage:
  python3 scripts/batch_cursor_bounds.py             # report bounds and derivations
  python3 scripts/batch_cursor_bounds.py --self-test # run unit tests
"""

from __future__ import annotations

import argparse
import math
import sys
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT / "scripts"))
import olc_bounds  # Reconciled OLC concurrency and read set model

# Expanse digital trie physical constants (ARCHITECTURE.md, leaf.rs, sync.rs)
KEY_BYTES: int = 8
VALUE_BYTES: int = 8
ENTRY_BYTES: int = KEY_BYTES + VALUE_BYTES  # 16 bytes per (u64, u64) pair

# Level 1 byte expanse: 2^8 = 256 possible byte values
LEVEL1_BYTE_EXPANSE: int = 256
LEAFB1_MAX_POP: int = 256
LEAF1_MAX_POP: int = 25  # LEAF1_CAP in crates/expanse/src/leaf.rs
IMMEDIATE_MAX_POP: int = 7  # Maximum entries packed in 7 aux bytes

# Branch levels: 64-bit trie has levels 8 down to 2 for branches; level 1 is leaf
BRANCH_TOP_LEVEL: int = olc_bounds.BRANCH_TOP_LEVEL  # 8
BRANCH_MIN_LEVEL: int = olc_bounds.BRANCH_MIN_LEVEL  # 2
MAX_BRANCH_DEPTH: int = BRANCH_TOP_LEVEL - BRANCH_MIN_LEVEL + 1  # 7 branch levels

# Retained read set capacity in sync_nav.rs:46
READ_SET_CAP: int = 16

# Pre-registered Callgrind target ceilings (target) for P32.2 relative to sync_map_next_after_scan
TARGET_CALLGRIND_RATIOS: dict[str, float] = {
    "sequential": 0.30,  # >= 70% instruction reduction (target)
    "clustered": 0.40,   # >= 60% instruction reduction (target)
    "random": 0.50,      # >= 50% instruction reduction (target)
}


def terminal_buffer_capacity_bound() -> int:
    """Maximum entries that any single terminal node in Expanse can contain.

    In Expanse's digital trie, keys are partitioned by digit (8 bits per level).
    Level 1 represents the final key byte. The widest leaf is `LeafB1`, which
    covers a full byte expanse of 2^8 = 256 distinct values. Other terminals
    hold fewer: `Leaf1` holds up to 25 entries (`LEAF1_CAP`), and immediates
    hold up to 7 entries.

    Returns:
        256 (entries).
    """
    return LEAFB1_MAX_POP


def terminal_buffer_byte_bound(entry_bytes: int = ENTRY_BYTES) -> int:
    """Exact byte size required for an internal cursor buffer to hold any terminal.

    Args:
        entry_bytes: Byte size of each key-value pair. Defaults to 16.

    Returns:
        Exact buffer size in bytes (4096 bytes = 4 KiB for 16-byte pairs).

    Raises:
        ValueError: If entry_bytes <= 0.
    """
    if entry_bytes <= 0:
        raise ValueError(f"entry_bytes must be > 0, got {entry_bytes}")
    return terminal_buffer_capacity_bound() * entry_bytes


def mid_leaf_resume_extra_descents(leaf_pop: int, buffer_cap: int) -> int:
    """Number of extra root descents incurred if buffer capacity cannot hold leaf_pop.

    If buffer_cap >= leaf_pop, the entire leaf is copied in 1 batch, so 0 extra
    descents occur. If buffer_cap < leaf_pop, the cursor must unpin between
    batches, invalidate all node pointers, and resume by key from the root
    `ceil(leaf_pop / buffer_cap) - 1` times.

    Args:
        leaf_pop: Number of entries in the leaf (0 <= leaf_pop <= 256).
        buffer_cap: Buffer capacity in entries (> 0).

    Returns:
        Number of additional root descents (>= 0).

    Raises:
        ValueError: If leaf_pop < 0, leaf_pop > 256, or buffer_cap <= 0.
    """
    if not (0 <= leaf_pop <= LEAFB1_MAX_POP):
        raise ValueError(f"leaf_pop must be in [0, {LEAFB1_MAX_POP}], got {leaf_pop}")
    if buffer_cap <= 0:
        raise ValueError(f"buffer_cap must be > 0, got {buffer_cap}")
    if leaf_pop == 0:
        return 0
    batches = math.ceil(leaf_pop / buffer_cap)
    return max(0, batches - 1)


def terminal_drain_read_set_branches() -> int:
    """Branch nodes in the read set to drain one terminal leaf.

    In Expanse, terminal leaves (Leaf1..Leaf7, LeafB1, immediates) carry no version
    word of their own (docs/ARCHITECTURE.md:589, crates/expanse/src/sync_nav.rs:26-29).
    Their stores are bracketed by the parent branch's version word (mutate_map.rs:1339,
    1485, 1500, 1684). Draining all entries of a single terminal leaf into the cursor buffer
    validates strictly that covering parent branch (Holder::Node, sync_nav.rs:424).

    Returns:
        1 branch node.
    """
    return 1


def terminal_drain_version_cost() -> tuple[int, int]:
    """Cost in `(version_loads, fences)` to drain one terminal leaf under its covering parent.

    Uses `olc_bounds.version_word_cost(1, retained=False)`:
    - 1 version load to sample the parent version (`node_sample`, occ.rs:44).
    - 1 acquire fence and 1 version load to validate after copying (`node_validate`, occ.rs:60).

    Returns:
        `(2, 1)`: 2 version loads, 1 fence.
    """
    return olc_bounds.version_word_cost(terminal_drain_read_set_branches(), retained=False)


def point_lookup_read_set_branches() -> int:
    """Branch versions a point lookup validates at most: one per level from levels 8 down to 2.

    Reconciles with `olc_bounds.get_read_set_branches()`.

    Returns:
        7 branch nodes.
    """
    return olc_bounds.get_read_set_branches()


def point_lookup_version_cost() -> tuple[int, int]:
    """Cost in `(version_loads, fences)` for a point lookup descent (hand-over-hand).

    Reconciles with `olc_bounds.version_word_cost(7, retained=False)`.

    Returns:
        `(14, 7)`: 14 version loads, 7 fences.
    """
    return olc_bounds.version_word_cost(point_lookup_read_set_branches(), retained=False)


def ordered_search_read_set_branches(backtrack_level: int) -> int:
    """Branch versions an ordered search validates at most for a backtrack at `backtrack_level`.

    Reconciles with `olc_bounds.ordered_read_set_branches(backtrack_level)`:
    `(BRANCH_TOP_LEVEL - backtrack_level + 1) + 2 * (backtrack_level - BRANCH_MIN_LEVEL)`.
    For backtrack_level in [2, 8], bounds the retained read set to <= 13 branch nodes.

    Args:
        backtrack_level: Branch level where backtrack occurs (2 <= backtrack_level <= 8).

    Returns:
        Branch nodes in the retained read set.
    """
    return olc_bounds.ordered_read_set_branches(backtrack_level)


def max_ordered_search_read_set_branches() -> int:
    """The largest `ordered_search_read_set_branches` across all backtrack levels.

    Reconciles with `olc_bounds.max_ordered_read_set_branches()`.

    Returns:
        13 branch nodes.
    """
    return olc_bounds.max_ordered_read_set_branches()


def ordered_search_max_version_cost() -> tuple[int, int]:
    """Cost in `(version_loads, fences)` to validate the maximum retained read set (13 branches).

    Reconciles with `olc_bounds.version_word_cost(13, retained=True)`.

    Returns:
        `(39, 14)`: 39 version loads, 14 fences.
    """
    return olc_bounds.version_word_cost(max_ordered_search_read_set_branches(), retained=True)


def retained_read_set_capacity() -> int:
    """Maximum capacity of the retained read set in `sync_nav.rs:46` (`READ_SET_CAP`).

    Strictly exceeds the maximum consistent-read bound of 13 branch nodes.

    Returns:
        16 entries.
    """
    return READ_SET_CAP


def point_scan_descents_per_leaf(leaf_pop: int) -> int:
    """Root descents incurred by unbatched `next_after` across a leaf of population `leaf_pop`.

    Because single-key `next_after` unpins between calls, each key lookup executes
    a separate call to `sync_nav::next_validated`, descending the tree from the root.

    Args:
        leaf_pop: Leaf population (0 <= leaf_pop <= 256).

    Returns:
        leaf_pop descents.
    """
    if not (0 <= leaf_pop <= LEAFB1_MAX_POP):
        raise ValueError(f"leaf_pop must be in [0, {LEAFB1_MAX_POP}], got {leaf_pop}")
    return leaf_pop


def mid_leaf_resume_extra_cost(leaf_pop: int, buffer_cap: int, depth: int = MAX_BRANCH_DEPTH) -> dict[str, int]:
    """Total synchronization penalty of a buffer narrower than leaf_pop across one leaf.

    Args:
        leaf_pop: Leaf population (0 <= leaf_pop <= 256).
        buffer_cap: Buffer capacity in entries (> 0).
        depth: Branch depth for root descents (1 <= depth <= 8).

    Returns:
        Dict containing:
          - 'extra_descents': Extra root descents incurred.
          - 'extra_pins': Extra epoch pin/unpin pairs incurred.
          - 'extra_version_loads': Extra version word loads incurred (under point lookup descent).
          - 'extra_fences': Extra memory fences incurred.
          - 'atomic_snapshot': True if entire leaf is read atomically in one batch.
    """
    extra_descents = mid_leaf_resume_extra_descents(leaf_pop, buffer_cap)
    loads_per_descent, fences_per_descent = olc_bounds.version_word_cost(depth, retained=False)
    return {
        "extra_descents": extra_descents,
        "extra_pins": extra_descents,
        "extra_version_loads": extra_descents * loads_per_descent,
        "extra_fences": extra_descents * fences_per_descent,
        "atomic_snapshot": extra_descents == 0,
    }


class TestBatchCursorBounds(unittest.TestCase):
    """Unit tests pinning mathematical bounds and invariants."""

    def test_terminal_buffer_capacity(self) -> None:
        """Pin maximum terminal capacity to exactly 256 entries."""
        self.assertEqual(terminal_buffer_capacity_bound(), 256)

    def test_terminal_buffer_bytes(self) -> None:
        """Pin 4 KiB buffer byte bound for 16-byte (u64, u64) entries."""
        self.assertEqual(terminal_buffer_byte_bound(16), 4096)
        self.assertEqual(terminal_buffer_byte_bound(8), 2048)
        with self.assertRaises(ValueError):
            terminal_buffer_byte_bound(0)
        with self.assertRaises(ValueError):
            terminal_buffer_byte_bound(-1)

    def test_4kib_buffer_eliminates_mid_leaf_descents(self) -> None:
        """Confirm that a 4 KiB (256-entry) buffer incurs 0 extra descents across all pops."""
        cap = terminal_buffer_capacity_bound()
        for pop in range(0, 257):
            self.assertEqual(mid_leaf_resume_extra_descents(pop, cap), 0)

    def test_narrow_buffer_incurs_extra_descents(self) -> None:
        """Pin extra descent counts for buffers smaller than 256."""
        # 64-entry buffer on 256-entry leaf: ceil(256 / 64) - 1 = 3 extra descents
        self.assertEqual(mid_leaf_resume_extra_descents(256, 64), 3)
        # 16-entry buffer on 256-entry leaf: ceil(256 / 16) - 1 = 15 extra descents
        self.assertEqual(mid_leaf_resume_extra_descents(256, 16), 15)
        # 128-entry buffer on 256-entry leaf: ceil(256 / 128) - 1 = 1 extra descent
        self.assertEqual(mid_leaf_resume_extra_descents(256, 128), 1)
        # 64-entry buffer on 65-entry leaf: ceil(65 / 64) - 1 = 1 extra descent
        self.assertEqual(mid_leaf_resume_extra_descents(65, 64), 1)
        # 64-entry buffer on 64-entry leaf: 0 extra descents
        self.assertEqual(mid_leaf_resume_extra_descents(64, 64), 0)

    def test_mid_leaf_resume_bounds_validation(self) -> None:
        """Ensure invalid inputs to mid_leaf_resume_extra_descents fail loudly."""
        with self.assertRaises(ValueError):
            mid_leaf_resume_extra_descents(-1, 64)
        with self.assertRaises(ValueError):
            mid_leaf_resume_extra_descents(257, 64)
        with self.assertRaises(ValueError):
            mid_leaf_resume_extra_descents(100, 0)
        with self.assertRaises(ValueError):
            mid_leaf_resume_extra_descents(100, -10)

    def test_reconciled_olc_bounds(self) -> None:
        """Reconcile bounds with olc_bounds and sync_nav invariants."""
        # Point lookup: 7 branch nodes (levels 8 down to 2)
        self.assertEqual(point_lookup_read_set_branches(), 7)
        self.assertEqual(point_lookup_version_cost(), (14, 7))

        # Terminal drain: 1 covering parent branch node
        self.assertEqual(terminal_drain_read_set_branches(), 1)
        self.assertEqual(terminal_drain_version_cost(), (2, 1))

        # Ordered search: backtrack at level 2 -> (8 - 2 + 1) + 2 * (2 - 2) = 7
        self.assertEqual(ordered_search_read_set_branches(2), 7)
        # backtrack at level 8 -> (8 - 8 + 1) + 2 * (8 - 2) = 1 + 12 = 13
        self.assertEqual(ordered_search_read_set_branches(8), 13)
        self.assertEqual(max_ordered_search_read_set_branches(), 13)
        self.assertEqual(ordered_search_max_version_cost(), (39, 14))

        # Retained read set capacity strictly bounds max consistent search
        self.assertEqual(retained_read_set_capacity(), 16)
        self.assertGreater(retained_read_set_capacity(), max_ordered_search_read_set_branches())

    def test_mid_leaf_cost_comparison(self) -> None:
        """Verify synchronization penalty dict for 4 KiB vs 1 KiB (64 entries)."""
        cost_4k = mid_leaf_resume_extra_cost(256, 256, depth=7)
        self.assertEqual(cost_4k["extra_descents"], 0)
        self.assertEqual(cost_4k["extra_version_loads"], 0)
        self.assertEqual(cost_4k["extra_fences"], 0)
        self.assertTrue(cost_4k["atomic_snapshot"])

        cost_1k = mid_leaf_resume_extra_cost(256, 64, depth=7)
        self.assertEqual(cost_1k["extra_descents"], 3)
        self.assertEqual(cost_1k["extra_pins"], 3)
        # 3 extra descents * (2 * 7) loads = 3 * 14 = 42 version loads
        self.assertEqual(cost_1k["extra_version_loads"], 42)
        # 3 extra descents * 7 fences = 21 fences
        self.assertEqual(cost_1k["extra_fences"], 21)
        self.assertFalse(cost_1k["atomic_snapshot"])

    def test_point_scan_descents(self) -> None:
        """Pin point-scan root descents to leaf population."""
        self.assertEqual(point_scan_descents_per_leaf(256), 256)
        self.assertEqual(point_scan_descents_per_leaf(25), 25)
        self.assertEqual(point_scan_descents_per_leaf(0), 0)
        with self.assertRaises(ValueError):
            point_scan_descents_per_leaf(-1)
        with self.assertRaises(ValueError):
            point_scan_descents_per_leaf(257)

    def test_target_callgrind_ratios(self) -> None:
        """Confirm pre-registered target ratios are ordered and valid probabilities."""
        self.assertEqual(TARGET_CALLGRIND_RATIOS["sequential"], 0.30)
        self.assertEqual(TARGET_CALLGRIND_RATIOS["clustered"], 0.40)
        self.assertEqual(TARGET_CALLGRIND_RATIOS["random"], 0.50)
        for density, ratio in TARGET_CALLGRIND_RATIOS.items():
            self.assertGreater(ratio, 0.0)
            self.assertLess(ratio, 1.0)
        self.assertLess(TARGET_CALLGRIND_RATIOS["sequential"], TARGET_CALLGRIND_RATIOS["clustered"])
        self.assertLess(TARGET_CALLGRIND_RATIOS["clustered"], TARGET_CALLGRIND_RATIOS["random"])


def report() -> None:
    """Print the derived bounds and comparative summary."""
    print("=" * 76)
    print("MATHEMATICAL BOUNDS FOR VALIDATED BATCH CURSOR (#1142)")
    print("=" * 76)
    print("1. Terminal Buffer Sizing Bound:")
    print(f"   Max terminal entries (LeafB1 byte expanse): {terminal_buffer_capacity_bound()} entries")
    print(f"   Required cursor buffer size:                {terminal_buffer_byte_bound()} bytes (4 KiB)")
    print()
    print("2. 4 KiB Buffer vs Narrow Buffer (LeafB1 pop = 256 entries):")
    print(f"   {'Buffer Config':<20} {'Extra Descents':<16} {'Extra Loads':<14} {'Extra Fences':<14} {'Atomic?':<8}")
    print("   " + "-" * 72)
    for cap, label in ((256, "4 KiB (256 entries)"), (128, "2 KiB (128 entries)"),
                       (64, "1 KiB (64 entries)"), (16, "256 B (16 entries)")):
        c = mid_leaf_resume_extra_cost(256, cap, depth=7)
        print(f"   {label:<20} {c['extra_descents']:<16} {c['extra_version_loads']:<14} {c['extra_fences']:<14} {str(c['atomic_snapshot']):<8}")
    print()
    print("3. Read Set and Synchronization Reconciled Model (olc_bounds.py & sync_nav.rs):")
    td_loads, td_fences = terminal_drain_version_cost()
    pl_loads, pl_fences = point_lookup_version_cost()
    os_loads, os_fences = ordered_search_max_version_cost()
    print(f"   Terminal leaf drain:      {terminal_drain_read_set_branches()} covering branch in read set, {td_loads} version loads, {td_fences} fence")
    print(f"   Point lookup (depth 7):   {point_lookup_read_set_branches()} branch nodes in read set, {pl_loads} version loads, {pl_fences} fences")
    print(f"   Max ordered search (l=8): {max_ordered_search_read_set_branches()} branch nodes in read set, {os_loads} version loads, {os_fences} fences")
    print(f"   Retained read set cap:    {retained_read_set_capacity()} entries (sync_nav.rs:46 READ_SET_CAP)")
    print()
    print("4. Pre-registered Callgrind Target Ceilings (target) vs sync_map_next_after_scan:")
    for regime, ceiling in TARGET_CALLGRIND_RATIOS.items():
        reduction = (1.0 - ceiling) * 100.0
        print(f"   {regime:<14} ratio <= {ceiling:.2f} (target) (>= {reduction:.0f}% instruction reduction target)")
    print("=" * 76)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n", 1)[0])
    parser.add_argument("--self-test", action="store_true", help="Run unit test suite")
    args = parser.parse_args()

    if args.self_test:
        suite = unittest.TestLoader().loadTestsFromTestCase(TestBatchCursorBounds)
        runner = unittest.TextTestRunner(verbosity=2)
        result = runner.run(suite)
        return 0 if result.wasSuccessful() else 1

    report()
    suite = unittest.TestLoader().loadTestsFromTestCase(TestBatchCursorBounds)
    result = unittest.TextTestRunner(stream=open("/dev/null", "w")).run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    sys.exit(main())
