#!/usr/bin/env python3
"""Mathematical bounds for validated ordered reads and batch cursor on StrReader (#1143).

This module implements pure bound functions with unit tests pinning known
reference values (AGENTS.md §8.8 commit 1, §1.3), reconciled with the concurrency
and optimistic lock coupling model in `scripts/olc_bounds.py`, `scripts/batch_cursor_bounds.py`,
and the concrete engine structures in `crates/expanse/src/strmap.rs` and `crates/expanse/src/sync_cursor.rs`.
The pre-registration in `docs/benchmarks/concurrency/METHODOLOGY.md` §33 invokes
these functions rather than restating hand arithmetic.

The problem:
  `StrReader` exposes only point lookup `get` and `contains` (`crates/expanse/src/sync.rs:13030-13095`).
  An ordered scan on a shared string map today must go through `SyncExpanseStrMap::with_locked`
  (`sync.rs:12808`), which holds the writer mutex, quiescing optimistic writers and blocking
  concurrent readers that fall back. A scan therefore serializes against writers and readers.

Concrete trie structure & descent model (`crates/expanse/src/strmap.rs`):
  1. Key chunking: String keys are decomposed into chunks of at most 8 bytes (`CHUNK_BYTES = 8`,
     `strmap.rs:117-122`). A key of length K terminates with NUL; `chunk_at(key, off)` (`strmap.rs:591-598`)
     yields (chunk: u64, terminal: bool). The number of chunk stages is C(K) = floor(K / 8) + 1.
  2. Each chunk is looked up in a `StrNode`'s `MapCore` (`strmap.rs:384-392, 2309-2327`).
     `StrNode` heads with `cover: u32` (offset 0), followed by `dirty: u32` (offset 4) and
     `map: MapCore` (`strmap.rs:397-407`).
     Inside `MapCore`, a 64-bit chunk descent traverses up to 7 branch levels (levels 8 down to 2;
     level 1 is leaf).
     Total versioned node depth per chunk stage is at most 1 StrNode cover + 7 MapCore branches = 8.
     Total trie depth across C(K) stages: D(K) <= 8 * (floor(K / 8) + 1).
  3. Read set scaling & `READ_SET_CAP = 16` (`crates/expanse/src/sync_cursor.rs:54`):
     - Within a single `StrNode`'s `MapCore`, an ordered step or backtrack retains at most 13 branch
       nodes (`scripts/olc_bounds.py`) plus the 1 StrNode cover, totalling at most 14 versions.
       This strictly fits within `READ_SET_CAP = 16` (14 <= 16).
     - Across multiple StrNode levels (K >= 8, C(K) >= 2), unconstrained multi-level backtracking
       would require 14 + 8 * (C(K) - 1) >= 22 versions, exceeding `READ_SET_CAP = 16`.
     - Policy: Scoped terminal drains + resume-by-key. Batches are drained at terminal chunks/leaves
       within the active StrNode. Across StrNodes and across batches, the cursor validates the
       retained versions, unpins, and resumes from root by key (`next_at_or_after`). If an active
       search ever exceeds `READ_SET_CAP = 16`, `ReadSet::sample` returns `Err(Retry)`
       (`sync_cursor.rs:80-82`), triggering validation, unpin, and root re-descent by key.
  4. Variable-length key buffering & byte budget:
     - `BATCH_CAP = 256` entries (matching `sync_cursor.rs:51` and LeafB1 capacity).
     - `KEY_BUFFER_BUDGET_BYTES = 4096` bytes (4 KiB arena for variable-length key bytes).
     - `ENTRY_DESCRIPTOR_BYTES = 16` (8 bytes value + 4 bytes offset + 4 bytes length).
     - Total cursor footprint: 4 KiB keys + 4 KiB descriptors = 8 KiB (2 pages, resides in L1 cache).
     - Oversized key policy (K > 4096 bytes): If buffer is non-empty, close and validate current batch;
       if buffer is empty, allocate a dedicated spill buffer sized to K, copy and emit as a 1-item batch
       (N = 1), validate under cover, and resume from root by key on the next batch. Normal keys <= 4 KiB
       pay zero allocations.
  5. Callgrind prediction derivation:
     Measured `sync_strmap_scan_locked` baseline in CI (run 37249211829, commit dce444783437):
       - `paths`: 9,199,722 ins / 50k = 183.99444 ins/key (measured: CI 37249211829, dce444783437).
       - `paths_dense`: 13,661,632 ins / 50k = 273.23264 ins/key (measured: CI 37249211829, dce444783437).
     The batch cursor pays amortized epoch pinning, version validation (~90 ins/batch over ~64 entries = ~1.4 ins/key),
     and cursor buffer copy/iteration overhead (~3.0 ins/key), resulting in ~4.4 ins/key overhead.
     Predicted ratios: ~1.024 on `paths`, ~1.016 on `paths_dense`.
     Target ratio ceiling: <= 1.15 (target) on both `paths` and `paths_dense`.

Sources:
  `docs/ARCHITECTURE.md` §4.2 ("The string wrapper"),
  `crates/expanse/src/strmap.rs` (StrNode, StrSuffix, StrCursor, Walk, chunk_at),
  `crates/expanse/src/sync.rs` (StrReader, SyncExpanseStrMap),
  `crates/expanse/src/sync_cursor.rs` (READ_SET_CAP, BATCH_CAP, ReadSet, Holder),
  `scripts/olc_bounds.py` (read set sizes and version word cost model),
  `scripts/batch_cursor_bounds.py` (batch cursor sizing and terminal drain model),
  Issues #1142, #1143.

Usage:
  python3 scripts/str_cursor_bounds.py             # report bounds and derivations
  python3 scripts/str_cursor_bounds.py --self-test # run unit tests
"""

from __future__ import annotations

import argparse
import sys
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT / "scripts"))
import olc_bounds  # Reconciled OLC concurrency and read set model

# Chunk constants from crates/expanse/src/strmap.rs:117-122
CHUNK_BYTES: int = 8

# Maximum branch depth in MapCore (levels 8 down to 2; level 1 is leaf)
MAX_BRANCH_DEPTH: int = olc_bounds.BRANCH_TOP_LEVEL - olc_bounds.BRANCH_MIN_LEVEL + 1  # 7

# Retained read set capacity in crates/expanse/src/sync_cursor.rs:54
READ_SET_CAP: int = 16

# Terminal batch buffer sizing constants
BATCH_CAP: int = 256  # crates/expanse/src/sync_cursor.rs:51
KEY_BUFFER_BUDGET_BYTES: int = 4096  # 4 KiB key byte arena
ENTRY_DESCRIPTOR_BYTES: int = 16  # 8 bytes value + 4 bytes offset + 4 bytes length
TOTAL_CURSOR_BUFFER_BYTES: int = KEY_BUFFER_BUDGET_BYTES + BATCH_CAP * ENTRY_DESCRIPTOR_BYTES  # 8 KiB

# Measured baseline instruction counts from Callgrind runner:
# CI Run ID: 37249211829, commit dce4447834374b562fc8923e19d60a091a5d923b (PR #1373)
# N = 50,000 ops (keys)
STRMAP_BASELINE_MEASURED: dict[str, dict[str, float | int | str]] = {
    "paths": {
        "total_instructions": 9_199_722,
        "ops": 50_000,
        "instructions_per_key": 9_199_722 / 50_000,  # 183.99444
        "est_cycles": 14_320_176,
        "run_id": 37249211829,
        "commit": "dce4447834374b562fc8923e19d60a091a5d923b",
    },
    "paths_dense": {
        "total_instructions": 13_661_632,
        "ops": 50_000,
        "instructions_per_key": 13_661_632 / 50_000,  # 273.23264
        "est_cycles": 20_429_261,
        "run_id": 37249211829,
        "commit": "dce4447834374b562fc8923e19d60a091a5d923b",
    },
}

# Pre-registered Callgrind target ceilings (target) for P33.1 relative to sync_strmap_scan_locked
TARGET_CALLGRIND_RATIOS: dict[str, float] = {
    "paths": 1.15,
    "paths_dense": 1.15,
}


def str_chunk_count(key_bytes: int) -> int:
    """Number of 8-byte chunk stages for a string key of length `key_bytes`.

    In `crates/expanse/src/strmap.rs:591-598`, `chunk_at(key, off)` advances by
    `CHUNK_BYTES = 8` bytes per stage. Keys terminate with NUL. If `rest.len() < CHUNK`,
    the chunk is terminal:
      - K in [0, 7]: 1 chunk (terminal).
      - K = 8: 2 chunks (chunk 0: 8 bytes, non-terminal; chunk 1: 0 bytes, terminal).
      - General: floor(K / 8) + 1.

    Args:
        key_bytes: Key length in bytes (>= 0).

    Returns:
        Number of chunk stages (>= 1).

    Raises:
        ValueError: If key_bytes < 0.
    """
    if key_bytes < 0:
        raise ValueError(f"key_bytes must be >= 0, got {key_bytes}")
    return (key_bytes // CHUNK_BYTES) + 1


def str_trie_depth_bound(key_bytes: int) -> int:
    """Maximum versioned node depth traversed from root StrNode to terminal leaf.

    At each chunk stage, navigation touches:
      - 1 `StrNode`'s `cover: u32` OCC version word (offset 0, strmap.rs:387).
      - Up to 7 `MapCore` branch nodes (levels 8 down to 2; level 1 is leaf).
    Across C(K) chunk stages, maximum versioned node depth is:
      D(K) <= C(K) * (1 + MAX_BRANCH_DEPTH) = 8 * (floor(K / 8) + 1).

    Args:
        key_bytes: Key length in bytes (>= 0).

    Returns:
        Maximum node depth (>= 8).

    Raises:
        ValueError: If key_bytes < 0.
    """
    return str_chunk_count(key_bytes) * (1 + MAX_BRANCH_DEPTH)


def str_single_node_max_read_set() -> int:
    """Maximum version words retained for an ordered backtrack within a single StrNode.

    Inside a single `StrNode`:
      - 1 `StrNode.cover` version word.
      - At most 13 branch nodes in `MapCore` for backtrack at level 8
        (`olc_bounds.max_ordered_read_set_branches() = 13`).
    Total versions retained: 1 + 13 = 14 versions.
    Reconciles with `READ_SET_CAP = 16` in `sync_cursor.rs:54`: 14 <= 16.

    Returns:
        14 versions.
    """
    return 1 + olc_bounds.max_ordered_read_set_branches()


def str_multi_level_read_set_bound(key_bytes: int) -> int:
    """Maximum version words if an unconstrained search retained versions across all levels.

    For C(K) chunk levels, retaining all parent covers and intermediate descent paths yields:
      R(K) <= str_single_node_max_read_set() + (C(K) - 1) * (1 + MAX_BRANCH_DEPTH)
           = 14 + 8 * (C(K) - 1).

    For K >= 8 (C(K) >= 2), R(K) >= 22 > 16 (`READ_SET_CAP`), proving that a multi-level
    search cannot retain all versions in a fixed 16-entry array without a resume-by-key policy.

    Args:
        key_bytes: Key length in bytes (>= 0).

    Returns:
        Upper bound on retained versions if unconstrained.
    """
    chunks = str_chunk_count(key_bytes)
    return str_single_node_max_read_set() + (chunks - 1) * (1 + MAX_BRANCH_DEPTH)


def str_read_set_policy() -> dict[str, str | int | bool]:
    """Reconciliation of multi-level read set scaling with `READ_SET_CAP = 16`.

    Returns:
        Policy dictionary defining single-node conformance, resume-by-key, and fallback.
    """
    single_node_max = str_single_node_max_read_set()
    return {
        "read_set_cap": READ_SET_CAP,
        "single_node_max": single_node_max,
        "single_node_fits_cap": single_node_max <= READ_SET_CAP,
        "scoped_terminal_drain": (
            "Batches drain terminal chunks/leaves within the active StrNode under its "
            f"cover, retaining at most {single_node_max} <= {READ_SET_CAP} versions."
        ),
        "resume_by_key_policy": (
            "Across StrNodes and across batches, the cursor validates the retained "
            "versions, unpins, and resumes from root by key (next_at_or_after), ensuring "
            "no raw node pointers are held across unpins and stack stays strictly O(1)."
        ),
        "overflow_fallback": (
            f"If any search path during descent exceeds READ_SET_CAP = {READ_SET_CAP}, "
            "ReadSet::sample returns Err(Retry) (sync_cursor.rs:80-82), triggering "
            "validation, unpin, and root re-descent by key."
        ),
    }


def str_batch_capacity_bound(avg_key_bytes: int) -> int:
    """Maximum entries a batch holds given average key length before filling 4 KiB budget.

    Args:
        avg_key_bytes: Average key length in bytes (> 0).

    Returns:
        Entry capacity (>= 1, <= BATCH_CAP = 256).

    Raises:
        ValueError: If avg_key_bytes <= 0.
    """
    if avg_key_bytes <= 0:
        raise ValueError(f"avg_key_bytes must be > 0, got {avg_key_bytes}")
    max_by_bytes = KEY_BUFFER_BUDGET_BYTES // avg_key_bytes
    return max(1, min(BATCH_CAP, max_by_bytes))


def str_oversized_key_policy() -> dict[str, int | str]:
    """Policy for variable-length key buffering and keys exceeding the byte budget.

    Returns:
        Dict specifying buffer sizes and the oversized key policy.
    """
    return {
        "key_buffer_budget_bytes": KEY_BUFFER_BUDGET_BYTES,
        "batch_cap_entries": BATCH_CAP,
        "entry_descriptor_bytes": ENTRY_DESCRIPTOR_BYTES,
        "total_cursor_buffer_bytes": TOTAL_CURSOR_BUFFER_BYTES,
        "oversized_key_rule": (
            f"If a single key exceeds {KEY_BUFFER_BUDGET_BYTES} bytes, any pending batch "
            "is closed and validated. The oversized key is loaded into a dedicated spill "
            "buffer, validated under the node cover, and emitted as a 1-item batch (N = 1). "
            "Subsequent batches resume from root via next_at_or_after."
        ),
        "heap_allocation_guarantee": (
            f"Zero heap allocations for all keys <= {KEY_BUFFER_BUDGET_BYTES} bytes during "
            "batch cursor scanning; strictly bounded L1 cache footprint (8 KiB total)."
        ),
    }


def strmap_baseline_instructions_per_key(workload: str) -> float:
    """Measured baseline instruction count per key for single-threaded in-place walk.

    Matches `sync_strmap_scan_locked` (50,000 keys) measured in CI Callgrind run:
      - `paths`: 9,199,722 total ins / 50,000 = 183.99444 ins/key (measured: CI 37249211829, dce444783437).
      - `paths_dense`: 13,661,632 total ins / 50,000 = 273.23264 ins/key (measured: CI 37249211829, dce444783437).

    Args:
        workload: 'paths' or 'paths_dense'.

    Returns:
        Instructions per key (> 0).

    Raises:
        ValueError: If unknown workload.
    """
    if workload in STRMAP_BASELINE_MEASURED:
        return float(STRMAP_BASELINE_MEASURED[workload]["instructions_per_key"])
    raise ValueError(f"Unknown workload: {workload}")


def strmap_batch_amortized_overhead_per_key(workload: str, avg_batch_size: int = 64) -> float:
    """Amortized synchronization and buffering overhead per key in `sync_strmap_scan`.

    Components:
      - Epoch pin/unpin per batch: ~40 instructions.
      - StrNode cover sample/validate per batch: ~15 instructions.
      - Active path branch versions sample/validate: ~35 instructions.
      - Total sync overhead per batch: ~90 instructions.
      - Amortized sync per key: 90 / avg_batch_size (~1.4 ins/key for N = 64).
      - Buffer write + read/dispatch overhead per key: ~3.0 instructions.
      - Total overhead per key: ~4.4 instructions/key.

    Args:
        workload: 'paths' or 'paths_dense'.
        avg_batch_size: Expected average batch size (> 0).

    Returns:
        Overhead in instructions per key.
    """
    if avg_batch_size <= 0:
        raise ValueError(f"avg_batch_size must be > 0, got {avg_batch_size}")
    sync_per_batch = 40.0 + 15.0 + 35.0  # 90.0 instructions
    sync_per_key = sync_per_batch / avg_batch_size
    buffering_per_key = 3.0
    return sync_per_key + buffering_per_key


def strmap_predicted_callgrind_ratio(workload: str, avg_batch_size: int = 64) -> float:
    """Derived Callgrind instruction ratio of `sync_strmap_scan` vs `sync_strmap_scan_locked`.

    Calculated as: (I_baseline + delta_I_overhead) / I_baseline.

    Args:
        workload: 'paths' or 'paths_dense'.
        avg_batch_size: Average batch size.

    Returns:
        Ratio (e.g. ~1.10 - 1.12).
    """
    base = strmap_baseline_instructions_per_key(workload)
    overhead = strmap_batch_amortized_overhead_per_key(workload, avg_batch_size)
    return (base + overhead) / base


class TestStrCursorBounds(unittest.TestCase):
    """Unit tests pinning mathematical bounds and invariants."""

    def test_str_chunk_count(self) -> None:
        """Pin chunk counts for key lengths."""
        self.assertEqual(str_chunk_count(0), 1)
        self.assertEqual(str_chunk_count(1), 1)
        self.assertEqual(str_chunk_count(7), 1)
        self.assertEqual(str_chunk_count(8), 2)
        self.assertEqual(str_chunk_count(9), 2)
        self.assertEqual(str_chunk_count(15), 2)
        self.assertEqual(str_chunk_count(16), 3)
        self.assertEqual(str_chunk_count(64), 9)
        with self.assertRaises(ValueError):
            str_chunk_count(-1)

    def test_str_trie_depth_bound(self) -> None:
        """Pin versioned node depth bounds from root to leaf."""
        self.assertEqual(str_trie_depth_bound(0), 8)
        self.assertEqual(str_trie_depth_bound(7), 8)
        self.assertEqual(str_trie_depth_bound(8), 16)
        self.assertEqual(str_trie_depth_bound(16), 24)
        self.assertEqual(str_trie_depth_bound(64), 72)
        with self.assertRaises(ValueError):
            str_trie_depth_bound(-1)

    def test_single_node_read_set_fits_cap(self) -> None:
        """Confirm single StrNode ordered search fits READ_SET_CAP = 16."""
        single_node = str_single_node_max_read_set()
        self.assertEqual(single_node, 14)
        self.assertLessEqual(single_node, READ_SET_CAP)

    def test_multi_level_read_set_exceeds_cap(self) -> None:
        """Demonstrate that unconstrained multi-level search exceeds READ_SET_CAP."""
        self.assertEqual(str_multi_level_read_set_bound(0), 14)
        self.assertEqual(str_multi_level_read_set_bound(8), 22)
        self.assertGreater(str_multi_level_read_set_bound(8), READ_SET_CAP)
        self.assertEqual(str_multi_level_read_set_bound(16), 30)
        self.assertGreater(str_multi_level_read_set_bound(16), READ_SET_CAP)

    def test_read_set_policy_structure(self) -> None:
        """Verify read set policy dictionary contents."""
        p = str_read_set_policy()
        self.assertEqual(p["read_set_cap"], 16)
        self.assertEqual(p["single_node_max"], 14)
        self.assertTrue(p["single_node_fits_cap"])
        self.assertIn("terminal chunks/leaves", str(p["scoped_terminal_drain"]))
        self.assertIn("resumes from root by key", str(p["resume_by_key_policy"]))
        self.assertIn("Err(Retry)", str(p["overflow_fallback"]))

    def test_buffer_budget_and_sizing(self) -> None:
        """Pin buffer constants and entry capacity across key lengths."""
        self.assertEqual(KEY_BUFFER_BUDGET_BYTES, 4096)
        self.assertEqual(BATCH_CAP, 256)
        self.assertEqual(TOTAL_CURSOR_BUFFER_BYTES, 8192)

        # 8-byte keys fill 4096 B with 512 entries, capped at BATCH_CAP = 256
        self.assertEqual(str_batch_capacity_bound(8), 256)
        # 32-byte keys: 4096 / 32 = 128 entries
        self.assertEqual(str_batch_capacity_bound(32), 128)
        # 64-byte keys: 4096 / 64 = 64 entries
        self.assertEqual(str_batch_capacity_bound(64), 64)
        # 4096-byte key: 4096 / 4096 = 1 entry
        self.assertEqual(str_batch_capacity_bound(4096), 1)

        with self.assertRaises(ValueError):
            str_batch_capacity_bound(0)

    def test_oversized_key_policy_structure(self) -> None:
        """Verify oversized key policy specifies dedicated spill buffer and zero-alloc."""
        p = str_oversized_key_policy()
        self.assertEqual(p["key_buffer_budget_bytes"], 4096)
        self.assertEqual(p["batch_cap_entries"], 256)
        self.assertIn("dedicated spill buffer", str(p["oversized_key_rule"]))
        self.assertIn("Zero heap allocations", str(p["heap_allocation_guarantee"]))

    def test_strmap_baseline_measured_pinned(self) -> None:
        """Pin measured Callgrind baseline instruction counts from CI.

        Cites CI run ID 37249211829 on commit dce4447834374b562fc8923e19d60a091a5d923b (PR #1373):
          - paths: 9,199,722 instructions over 50,000 keys = 183.99444 ins/key.
          - paths_dense: 13,661,632 instructions over 50,000 keys = 273.23264 ins/key.
        """
        paths_meta = STRMAP_BASELINE_MEASURED["paths"]
        self.assertEqual(paths_meta["total_instructions"], 9_199_722)
        self.assertEqual(paths_meta["ops"], 50_000)
        self.assertEqual(paths_meta["run_id"], 37249211829)
        self.assertEqual(paths_meta["commit"], "dce4447834374b562fc8923e19d60a091a5d923b")
        self.assertAlmostEqual(strmap_baseline_instructions_per_key("paths"), 183.99444, places=4)

        dense_meta = STRMAP_BASELINE_MEASURED["paths_dense"]
        self.assertEqual(dense_meta["total_instructions"], 13_661_632)
        self.assertEqual(dense_meta["ops"], 50_000)
        self.assertEqual(dense_meta["run_id"], 37249211829)
        self.assertEqual(dense_meta["commit"], "dce4447834374b562fc8923e19d60a091a5d923b")
        self.assertAlmostEqual(strmap_baseline_instructions_per_key("paths_dense"), 273.23264, places=4)

    def test_callgrind_predicted_ratios(self) -> None:
        """Confirm derived Callgrind ratios sit strictly below the 1.15 ceiling."""
        ratio_paths = strmap_predicted_callgrind_ratio("paths", avg_batch_size=64)
        ratio_dense = strmap_predicted_callgrind_ratio("paths_dense", avg_batch_size=64)

        # Overhead: 90 / 64 + 3.0 = 4.40625 ins/key
        # paths: (183.99444 + 4.40625) / 183.99444 ~= 1.0240
        self.assertGreater(ratio_paths, 1.0)
        self.assertLess(ratio_paths, TARGET_CALLGRIND_RATIOS["paths"])
        self.assertAlmostEqual(ratio_paths, (183.99444 + 4.40625) / 183.99444, places=3)
        self.assertEqual(TARGET_CALLGRIND_RATIOS["paths"], 1.15)

        # paths_dense: (273.23264 + 4.40625) / 273.23264 ~= 1.0161
        self.assertGreater(ratio_dense, 1.0)
        self.assertLess(ratio_dense, TARGET_CALLGRIND_RATIOS["paths_dense"])
        self.assertAlmostEqual(ratio_dense, (273.23264 + 4.40625) / 273.23264, places=3)
        self.assertEqual(TARGET_CALLGRIND_RATIOS["paths_dense"], 1.15)

        with self.assertRaises(ValueError):
            strmap_baseline_instructions_per_key("invalid_workload")
        with self.assertRaises(ValueError):
            strmap_batch_amortized_overhead_per_key("paths", avg_batch_size=0)


def report() -> None:
    """Print the derived bounds and comparative summary."""
    print("=" * 76)
    print("MATHEMATICAL BOUNDS FOR STRREADER ORDERED READS & BATCH CURSOR (#1143)")
    print("=" * 76)
    print("1. Real Trie Descent Depth Bound (C(K) chunk stages, MapCore depth 7):")
    for k in (0, 7, 8, 16, 64):
        c = str_chunk_count(k)
        d = str_trie_depth_bound(k)
        print(f"   Key length K = {k:<2} bytes: {c} chunk stage(s), max trie depth D(K) <= {d} versioned nodes")
    print()
    print("2. Read Set Scaling vs READ_SET_CAP = 16 (sync_cursor.rs:54):")
    p = str_read_set_policy()
    print(f"   Single StrNode max read set:      {p['single_node_max']} versions (1 StrNode cover + 13 MapCore branches)")
    print(f"   READ_SET_CAP in sync_cursor.rs:  {p['read_set_cap']} versions (single node fits: {p['single_node_fits_cap']})")
    print(f"   Multi-level (K=8, 2 chunks):      {str_multi_level_read_set_bound(8)} versions (exceeds {READ_SET_CAP})")
    print(f"   Multi-level (K=64, 9 chunks):     {str_multi_level_read_set_bound(64)} versions (exceeds {READ_SET_CAP})")
    print(f"   Policy: {p['scoped_terminal_drain']}")
    print(f"           {p['resume_by_key_policy']}")
    print()
    print("3. Variable-Length Key Buffer Budget & Oversized Key Policy:")
    bp = str_oversized_key_policy()
    print(f"   Key bytes arena budget:           {bp['key_buffer_budget_bytes']} bytes (4 KiB)")
    print(f"   Entry descriptors:                {bp['batch_cap_entries']} entries * {bp['entry_descriptor_bytes']} bytes = 4096 bytes (4 KiB)")
    print(f"   Total cursor buffer size:         {bp['total_cursor_buffer_bytes']} bytes (8 KiB, fits L1 cache)")
    print("   Batch entry capacity:")
    for k in (8, 32, 64, 4096):
        cap = str_batch_capacity_bound(k)
        print(f"     Avg key length K = {k:<4} bytes -> batch capacity = {cap} entries")
    print(f"   Oversized key policy: {bp['oversized_key_rule']}")
    print()
    print("4. Callgrind Instruction Predictions vs sync_strmap_scan_locked (target):")
    for wl, ceiling in TARGET_CALLGRIND_RATIOS.items():
        base = strmap_baseline_instructions_per_key(wl)
        ovh = strmap_batch_amortized_overhead_per_key(wl, 64)
        pred = strmap_predicted_callgrind_ratio(wl, 64)
        meta = STRMAP_BASELINE_MEASURED[wl]
        print(
            f"   {wl:<12}: measured {base:.1f} ins/key (CI run {meta['run_id']}, {meta['commit'][:12]}) "
            f"+ ~{ovh:.1f} ins/key ovh -> pred {pred:.3f} <= ceiling {ceiling:.2f} (target)"
        )
    print("=" * 76)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n", 1)[0])
    parser.add_argument("--self-test", action="store_true", help="Run unit test suite")
    args = parser.parse_args()

    if args.self_test:
        suite = unittest.TestLoader().loadTestsFromTestCase(TestStrCursorBounds)
        runner = unittest.TextTestRunner(verbosity=2)
        result = runner.run(suite)
        return 0 if result.wasSuccessful() else 1

    report()
    suite = unittest.TestLoader().loadTestsFromTestCase(TestStrCursorBounds)
    result = unittest.TextTestRunner(stream=open("/dev/null", "w")).run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    sys.exit(main())
