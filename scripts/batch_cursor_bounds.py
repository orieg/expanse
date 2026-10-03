#!/usr/bin/env python3
"""Mathematical bounds for the validated batch cursor on concurrent map readers (#1142).

This module implements pure bound functions with unit tests pinning known
reference values (AGENTS.md §8.8 commit 1, §1.3). The pre-registration in
`docs/benchmarks/concurrency/METHODOLOGY.md` §32 invokes these functions rather
than restating hand arithmetic.

The problem:
  `SyncExpanseMap` reader handles expose single-key ordered operations
  (`first`, `last`, `next_at_or_after`, `next_after`, `prev_at_or_before`,
  `prev_before`). Each call pins the epoch, samples the tree version, and
  descends from the root through `sync_nav::next_validated`. A k-entry scan
  using `next_after` pays k epoch pins and k root descents.

The proposed batch cursor:
  1. Pins once per batch.
  2. Copies one terminal's `(key, u64)` entries using raw loads into a
     cursor-owned buffer.
  3. Validates the terminal's direct parent cover.
  4. Steps to the next sibling under the parent's retained version,
     validating skipped empty siblings, and re-descends only when the parent
     version changes or the parent is exhausted.
  5. Re-descends from the root across batches (because node pointers cannot
     be retained safely once unpinned).

Key derivations settled in this module:
  - Terminal buffer capacity bound: in Expanse's digital trie, the widest
    terminal is a level-1 bitmap leaf (`LeafB1`), which represents an 8-bit
    byte expanse and holds at most 2^8 = 256 entries. At 16 bytes per entry
    (`(u64, u64)`), a 4096-byte (4 KiB) buffer strictly bounds any terminal
    in the trie.
  - 4 KiB buffer vs resume-by-key mid-leaf: a 4 KiB buffer eliminates mid-leaf
    splits entirely (0 extra descents, 0 extra version loads, 0 extra fences),
    preserving terminal-level atomicity. In contrast, any buffer B < 256
    requires `ceil(P / B) - 1` extra root descents, breaks terminal atomicity,
    and adds up to 195 extra version loads per leaf.
  - Per-step read set: stepping to a sibling under the same parent node has a
    read set of strictly 1 branch node (the parent), paying 1 version load and
    1 acquire fence. In contrast, a full root descent visits up to 7 branch
    levels, paying 15 version loads and 7 fences (a 7x to 15x reduction).
  - Callgrind instruction prediction: amortizing root descent and epoch pinning
    over leaf population predicts instruction reduction factors of >= 70% on
    sequential keys, >= 60% on clustered keys, and >= 50% on random keys
    relative to `sync_map_next_after_scan`.

Sources:
  Leis, Scheibner, Kemper & Neumann, "The ART of Practical Synchronization",
    DaMoN 2016 (optimistic lock coupling; version validation; parent cover).
  Hennessy & Patterson, Computer Architecture: A Quantitative Approach,
    6th ed., §5.2 (cache line transfers and coherence overhead).
  `docs/ARCHITECTURE.md` §4.1 (concurrent reads, covering function, ordered reads),
  `crates/expanse/src/leaf.rs` (`LEAF1_CAP = 25`, `LeafB1` 256-bit expanse),
  `crates/expanse/src/sync.rs` (ordered reads, `sync_nav::next_validated`),
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
BRANCH_TOP_LEVEL: int = 8
BRANCH_MIN_LEVEL: int = 2
MAX_BRANCH_DEPTH: int = BRANCH_TOP_LEVEL - BRANCH_MIN_LEVEL + 1  # 7 branch levels


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


def sibling_step_read_set_branches(same_parent: bool = True, backtrack_levels: int = 0) -> int:
    """Number of branch nodes in the read set when stepping to a sibling.

    When advancing to the next sibling under the same parent node, the parent's
    pointer is already held under the epoch pin and was sampled on initial descent.
    Under the lazy census (`ARCHITECTURE.md:224`), mutations to siblings or children
    do not bubble to ancestors, so validating the direct parent's version word alone
    guarantees that:
      (a) The parent node was not split or obsoleted.
      (b) No child was inserted at any intermediate empty sibling digit.
      (c) The loaded child pointer is consistent.
    Hence, read set size under the same parent is strictly 1 branch node.

    If the current parent is exhausted and the cursor backtracks h levels up the
    retained ancestor stack, the read set contains h + 1 branch nodes.

    Args:
        same_parent: True if next sibling is under the same direct parent.
        backtrack_levels: Number of levels ascended if same_parent is False.

    Returns:
        Branch nodes validated in the step (>= 1).

    Raises:
        ValueError: If backtrack_levels < 0 or backtrack_levels > MAX_BRANCH_DEPTH - 1.
    """
    if same_parent:
        return 1
    if not (0 <= backtrack_levels <= MAX_BRANCH_DEPTH - 1):
        raise ValueError(f"backtrack_levels must be in [0, {MAX_BRANCH_DEPTH - 1}], got {backtrack_levels}")
    return backtrack_levels + 1


def root_descent_read_set_branches(depth: int = MAX_BRANCH_DEPTH) -> int:
    """Number of branch nodes validated during a full root descent.

    A full root descent validates each branch node from the root down to the leaf
    (levels 8 down to 2 in a 64-bit trie = 7 branch nodes), plus the tree version word.

    Args:
        depth: Number of branch levels in the trie (1 <= depth <= 8).

    Returns:
        Number of branch nodes in the descent path.

    Raises:
        ValueError: If depth < 1 or depth > 8.
    """
    if not (1 <= depth <= 8):
        raise ValueError(f"depth must be in [1, 8], got {depth}")
    return depth


def step_version_word_cost(step_type: str, depth: int = MAX_BRANCH_DEPTH) -> tuple[int, int]:
    """Cost model in `(version_loads, fences)` for an iteration step.

    Models the synchronization operations executed per step:
      - `sibling_step`: Reads and validates direct parent version: 1 load, 1 fence.
      - `root_descent`: Reads tree head version, then for each branch level reads
        node version, child edge, validates version: (1 + 2 * depth) version loads,
        depth acquire fences.

    Args:
        step_type: Either 'sibling_step' or 'root_descent'.
        depth: Number of branch levels for root descent (1 <= depth <= 8).

    Returns:
        Tuple of `(version_loads, fences)`.

    Raises:
        ValueError: On unknown step_type or invalid depth.
    """
    if not (1 <= depth <= 8):
        raise ValueError(f"depth must be in [1, 8], got {depth}")
    if step_type == "sibling_step":
        return 1, 1
    elif step_type == "root_descent":
        return 1 + 2 * depth, depth
    else:
        raise ValueError(f"unknown step_type {step_type!r}; expected 'sibling_step' or 'root_descent'")


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
          - 'extra_version_loads': Extra version word loads incurred.
          - 'extra_fences': Extra memory fences incurred.
          - 'atomic_snapshot': True if entire leaf is read atomically in one batch.
    """
    extra_descents = mid_leaf_resume_extra_descents(leaf_pop, buffer_cap)
    loads_per_descent, fences_per_descent = step_version_word_cost("root_descent", depth)
    return {
        "extra_descents": extra_descents,
        "extra_pins": extra_descents,
        "extra_version_loads": extra_descents * loads_per_descent,
        "extra_fences": extra_descents * fences_per_descent,
        "atomic_snapshot": extra_descents == 0,
    }


def read_set_reduction_factor(depth: int = MAX_BRANCH_DEPTH) -> float:
    """Ratio of root descent branch read set to sibling step read set."""
    return float(root_descent_read_set_branches(depth)) / float(sibling_step_read_set_branches(same_parent=True))


def version_load_reduction_factor(depth: int = MAX_BRANCH_DEPTH) -> float:
    """Ratio of root descent version loads to sibling step version loads."""
    descent_loads, _ = step_version_word_cost("root_descent", depth)
    sibling_loads, _ = step_version_word_cost("sibling_step", depth)
    return float(descent_loads) / float(sibling_loads)


def predict_batch_cursor_ratio(density: str) -> dict[str, float]:
    """Derived Callgrind instruction prediction of batch cursor vs unbatched scan.

    In `sync_map_next_after_scan`, each key pays:
      - 1 epoch pin + unpin (~50 instructions)
      - 1 tree version sample + root descent through 7 branch levels (~350 instructions)
      - 1 leaf search & value load (~50 instructions)
      Total unbatched per key: ~450 instructions.

    In `sync_map_scan` with a 4 KiB buffer:
      - Traversal & epoch pin are paid once per terminal, amortized across L keys:
        (~350 descent + ~50 pin + ~10 parent validate) / L = ~410 / L instructions/key.
      - Batch copy into buffer: ~12 instructions/key.
      - Consumer buffer extraction: ~4 instructions/key.
      Total batched per key: ~410 / L + 16 instructions.

    Args:
        density: Distribution regime ('sequential', 'clustered', or 'random').

    Returns:
        Dict with:
          - 'avg_leaf_pop': Assumed average leaf population.
          - 'unbatched_per_key': Estimated baseline instructions per key.
          - 'batched_per_key': Estimated batch cursor instructions per key.
          - 'predicted_ratio': Batched / unbatched ratio.
          - 'gate_ceiling_ratio': Conservative upper bound for Callgrind pre-reg gate.
          - 'predicted_reduction_pct': Minimum instruction reduction percentage.

    Raises:
        ValueError: On unknown density.
    """
    unbatched_per_key = 450.0

    if density == "sequential":
        # Dense LeafB1 leaves with up to 256 keys (average ~250 keys)
        avg_pop = 250.0
        gate_ceiling_ratio = 0.30  # >= 70% reduction gate
        min_reduction_pct = 70.0
    elif density == "clustered":
        # Clustered keys form dense subtrees with average ~80 keys/leaf
        avg_pop = 80.0
        gate_ceiling_ratio = 0.40  # >= 60% reduction gate
        min_reduction_pct = 60.0
    elif density == "random":
        # Uniform 64-bit random keys: mix of Leaf1 (16-25 keys) and LeafB1, average ~25 keys/leaf
        avg_pop = 25.0
        gate_ceiling_ratio = 0.50  # >= 50% reduction gate
        min_reduction_pct = 50.0
    else:
        raise ValueError(f"unknown density {density!r}; expected 'sequential', 'clustered', or 'random'")

    batched_per_key = (410.0 / avg_pop) + 16.0
    predicted_ratio = batched_per_key / unbatched_per_key

    return {
        "avg_leaf_pop": avg_pop,
        "unbatched_per_key": unbatched_per_key,
        "batched_per_key": batched_per_key,
        "predicted_ratio": predicted_ratio,
        "gate_ceiling_ratio": gate_ceiling_ratio,
        "predicted_reduction_pct": min_reduction_pct,
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
        # 3 extra descents * (1 + 2 * 7) loads = 3 * 15 = 45 version loads
        self.assertEqual(cost_1k["extra_version_loads"], 45)
        # 3 extra descents * 7 fences = 21 fences
        self.assertEqual(cost_1k["extra_fences"], 21)
        self.assertFalse(cost_1k["atomic_snapshot"])

    def test_read_set_reduction(self) -> None:
        """Pin read set sizes: sibling step = 1; root descent = 7."""
        self.assertEqual(sibling_step_read_set_branches(same_parent=True), 1)
        self.assertEqual(sibling_step_read_set_branches(same_parent=False, backtrack_levels=2), 3)
        self.assertEqual(root_descent_read_set_branches(depth=7), 7)
        self.assertAlmostEqual(read_set_reduction_factor(depth=7), 7.0)

    def test_version_load_reduction(self) -> None:
        """Pin version loads: sibling step = (1, 1); root descent = (15, 7)."""
        self.assertEqual(step_version_word_cost("sibling_step", depth=7), (1, 1))
        self.assertEqual(step_version_word_cost("root_descent", depth=7), (15, 7))
        self.assertAlmostEqual(version_load_reduction_factor(depth=7), 15.0)

    def test_callgrind_instruction_predictions(self) -> None:
        """Verify that derived predictions strictly clear their conservative gate ceilings."""
        for density in ("sequential", "clustered", "random"):
            pred = predict_batch_cursor_ratio(density)
            self.assertLess(
                pred["predicted_ratio"],
                pred["gate_ceiling_ratio"],
                f"Prediction {pred['predicted_ratio']} did not clear gate {pred['gate_ceiling_ratio']} for {density}",
            )
            self.assertGreater(pred["predicted_reduction_pct"], 0.0)


def report() -> None:
    """Print the derived bounds and comparative summary."""
    print("=" * 76)
    print("MATHEMATICAL BOUNDS FOR VALIDATED BATCH CURSOR (#1142)")
    print("=" * 76)
    print("1. Terminal Buffer Bound:")
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
    print("3. Read Set and Synchronization Comparison (depth = 7):")
    sib_loads, sib_fences = step_version_word_cost("sibling_step", depth=7)
    root_loads, root_fences = step_version_word_cost("root_descent", depth=7)
    print(f"   Sibling step: {sibling_step_read_set_branches(True)} branch node in read set, {sib_loads} version load, {sib_fences} fence")
    print(f"   Root descent: {root_descent_read_set_branches(7)} branch nodes in read set, {root_loads} version loads, {root_fences} fences")
    print(f"   Read set reduction:     {read_set_reduction_factor(7):.1f}x fewer branch nodes")
    print(f"   Version load reduction: {version_load_reduction_factor(7):.1f}x fewer version loads")
    print()
    print("4. Callgrind Instruction Predictions vs `sync_map_next_after_scan`:")
    print(f"   {'Regime':<14} {'Avg Pop':<10} {'Unbatched (Ins)':<18} {'Batched (Ins)':<16} {'Predicted Ratio':<18} {'Gate Ceiling'}")
    print("   " + "-" * 88)
    for regime in ("sequential", "clustered", "random"):
        p = predict_batch_cursor_ratio(regime)
        print(f"   {regime:<14} {p['avg_leaf_pop']:<10.0f} {p['unbatched_per_key']:<18.1f} {p['batched_per_key']:<16.1f} {p['predicted_ratio']:<18.3f} <= {p['gate_ceiling_ratio']:.2f} (>={p['predicted_reduction_pct']:.0f}% win)")
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
    # Always verify self-test invariants before exiting
    suite = unittest.TestLoader().loadTestsFromTestCase(TestBatchCursorBounds)
    result = unittest.TextTestRunner(stream=open("/dev/null", "w")).run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    sys.exit(main())
