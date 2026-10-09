#!/usr/bin/env python3
"""Mathematical bounds for validated ordered reads and batch cursor on StrReader (#1143).

This module implements pure bound functions with unit tests pinning known
reference values (AGENTS.md §8.8 commit 1, §1.3), reconciled with the concurrency
and optimistic lock coupling model in `scripts/olc_bounds.py`, `scripts/batch_cursor_bounds.py`,
and the concrete engine structures in `crates/expanse/src/strmap.rs` and `crates/expanse/src/sync_cursor.rs`.
The pre-registration in `docs/benchmarks/concurrency/METHODOLOGY.md` §33 invokes
these functions rather than restating hand arithmetic.

The problem:
  `StrReader` exposes only point lookup `get` and `contains` (`crates/expanse/src/sync.rs:13207-13256`).
  An ordered scan on a shared string map today must go through `SyncExpanseStrMap::with_locked`
  (`sync.rs:12980` -> `Shared::with_locked`, `sync.rs:3032`). That takes `fallback_mutex`, quiesces
  the writer gate and takes the writer mutex, so a scan serialises against writers and against other
  `with_locked` callers. It does NOT hold the tree word for the closure and does not block optimistic
  `get` readers: the tree word is bracketed only around the dirty-digit fold republish
  (`sync.rs:3054-3061`), and the string map never runs that fold because `SharedTree for
  ExpanseStrMap` (`sync.rs:797-835`) keeps the null `root_top_ptr` default (`sync.rs:684-686`).

Concrete trie structure & descent model (`crates/expanse/src/strmap.rs`):
  1. Key chunking: String keys are decomposed into chunks of at most 8 bytes (`CHUNK_BYTES = 8`,
     `strmap.rs:117-122`). A key of length K terminates with NUL; `chunk_at(key, off)` (`strmap.rs:591-598`)
     yields (chunk: u64, terminal: bool). Before tail collapse a key of length K has
     C(K) = floor(K / 8) + 1 chunk stages.
  2. Each chunk is looked up in a `StrNode`'s `MapCore` (`strmap.rs:384-392, 2309-2327`).
     `StrNode` heads with `cover: u32` (offset 0), followed by `dirty: u32` (offset 4) and
     `map: MapCore` (`strmap.rs:397-407`).
     Inside `MapCore`, a 64-bit chunk descent traverses up to 7 branch levels (levels 8 down to 2;
     level 1 is leaf). A sub-map in leaf state (at most `ROOT_LEAF_CAP = 31` entries) has no branch.
  3. Depth is the number of `StrNode` levels on a path, NOT the key length. Tail collapse moves a key's
     remainder into a `StrSuffix` once it is unique, so the depth of a path is the number of shared
     8-byte prefix chunks plus one: floor(shared_prefix_bytes / 8) + 1 (`str_strnode_depth`). `paths`
     keys share 35 bytes, so depth 5.
  4. Read set (`sync_nav.rs` / `sync_cursor.rs`, `READ_SET_CAP = 16`):
     - `ReadSet::sample` pushes one entry per call with no de-duplication and no truncation on
       backtrack (`sync_nav.rs:69-79`; `backtrack` shrinks the set only under `cfg(test)`, as a
       negative control, `sync_nav.rs:758-771`), so every seek in a walk adds its own path.
     - The ported string walk runs two sub-map seeks per level, from the single-threaded walk it
       mirrors: `next_at_or_after(target)` (`strmap.rs:957`), then `next_after(target)` when a suffix
       compares below the remainder (`strmap.rs:977`) or `next_after(parent_target)` when unwinding
       from a failed child (`strmap.rs:1001`).
     - `str_point_read_set_bound` and `str_cursor_attempt_read_set_bound` derive the maximum by
       ENUMERATION of that walk (`enumerate_point_read_sets`), under three seek-count assumptions.
       The two-seek assumption matches the code; the other two are kept because earlier reviews used
       them. The cursor "14 <= 16" figure holds only under the one-seek assumption.
     - Overflow policy: `ReadSet::sample` returns `Err(Retry)` when `len == READ_SET_CAP`
       (`sync_nav.rs:72-73`, `sync_cursor.rs:82-83`). For u64 that is sound because an overflow implies
       a moving tree. For strings a depth overflow is deterministic, so a `Retry` repeats until
       `MAX_RETRIES = 64` (`sync.rs:165`) is spent. The string policy classifies an overflow by
       `len == READ_SET_CAP` AFTER `validate_all`: a failed validation is a moving tree (`Retry`); a
       passed validation with a full set is a depth overflow, which goes to `read_locked` immediately
       and is counted separately. This is a description of the target design; no Rust changes here.
  5. Variable-length key buffering & byte budget:
     - `BATCH_CAP = 256` entries (matching `sync_cursor.rs:51` and LeafB1 capacity).
     - `KEY_BUFFER_BUDGET_BYTES = 4096` bytes (4 KiB arena for variable-length key bytes).
     - `ENTRY_DESCRIPTOR_BYTES = 16` (8 bytes value + 4 bytes offset + 4 bytes length).
     - Total cursor footprint: 4 KiB keys + 4 KiB descriptors = 8 KiB (2 pages, resides in L1 cache).
     - Oversized key policy (K > 4096 bytes): If buffer is non-empty, close and validate current batch;
       if buffer is empty, allocate a dedicated spill buffer sized to K, copy and emit as a 1-item batch
       (N = 1), validate under cover, and resume from root by key on the next batch. Normal keys <= 4 KiB
       pay zero allocations.
  6. Callgrind prediction derivation:
     Measured `sync_strmap_scan_locked` baseline in CI (run 37249211829, commit dce444783437):
       - `paths`: 9,199,722 ins / 50k = 183.99444 ins/key (measured: CI 37249211829, dce444783437).
       - `paths_dense`: 13,661,632 ins / 50k = 273.23264 ins/key (measured: CI 37249211829, dce444783437).
     The batch cursor overhead is a hand estimate, not a measurement: 40 + 15 + 35 = 90 ins/batch
     (projected), an assumed batch of 64 keys (projected), and 3.0 ins/key of buffering (projected),
     giving ~4.4 ins/key (projected) and ratios ~1.024 / ~1.016 (projected). Only the Callgrind
     `instruction-counts` job decides P33.2; target ceiling <= 1.15 (target).

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

# Hand-estimated batch cursor costs. None of these was measured: they are (projected) until
# the `instruction-counts` job reports `sync_strmap_scan` (P33.2 decides, not this script).
PROJECTED_SYNC_INS_PER_BATCH: float = 40.0 + 15.0 + 35.0  # pin/unpin + cover + path versions (projected)
PROJECTED_BATCH_SIZE: int = 64  # assumed keys per batch (projected)
PROJECTED_BUFFER_INS_PER_KEY: float = 3.0  # buffer write + read/dispatch (projected)

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
    """Pre-collapse ceiling on versioned node depth, indexed by key LENGTH.

    Not the depth of a real path: tail collapse makes depth the number of shared prefix
    chunks (`str_strnode_depth`). Kept as an upper bound for keys with no shared prefix.

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


def str_strnode_depth(shared_prefix_bytes: int) -> int:
    """`StrNode` levels on a path whose keys share `shared_prefix_bytes` leading bytes.

    Tail collapse (`strmap.rs` module doc, `TAG_SUFFIX`) moves a key's remainder into a
    `StrSuffix` once it is unique, so depth is the number of shared whole 8-byte chunks
    plus the level where the keys diverge: floor(shared_prefix_bytes / 8) + 1. It is NOT
    indexed by key length (`str_chunk_count` is only the pre-collapse ceiling).
    `paths` keys share 35 bytes (`https://example.com/api/v2/objects/`, 35 bytes,
    `instructions.rs` `path_keys`), so depth 5.

    Raises:
        ValueError: If shared_prefix_bytes < 0.
    """
    if shared_prefix_bytes < 0:
        raise ValueError(f"shared_prefix_bytes must be >= 0, got {shared_prefix_bytes}")
    return shared_prefix_bytes // CHUNK_BYTES + 1


# --- Enumeration of the retained read set of the validated string walk -------------------
#
# Model of `StrNode::next_at_or_after` (`strmap.rs:947-1008`) as it would run under the
# retained-read-set protocol (`sync_nav.rs` module doc). A sub-map seek retains the
# versions it sampled and nothing is de-duplicated or dropped (`sync_nav.rs:69-79`):
#   HIT / EMPTY / min-descent: the descent crosses r branches, r in 0..=MAX_BRANCH_DEPTH
#     (r = 0 is a sub-map in leaf state, which has no branch).
#   FOUND: the seek answered, possibly after a backtrack at level l in 2..=8, which retains
#     `olc_bounds.ordered_read_set_branches(l)` branches (maximum 13).
# Each visited `StrNode` also retains its `cover` word (COVER = 1).
#
# Seek-count assumptions (the three figures earlier reviews produced):
#   two_seek: the code. Per descended level the walk seeks `next_at_or_after(target)`
#     (`strmap.rs:957`, exact hit) and, on the way back up, `next_after(parent_target)`
#     (`strmap.rs:1001`); a deepest level whose suffix compares below the remainder seeks
#     `next_after(target)` (`strmap.rs:977`). Both seeks are retained.
#   one_seek: one seek per level (the resume reuses the first seek's path).
#   ordered_step_per_failed_level: every failed level is charged a full ordered step
#     (1 cover + 13). Not reachable (a failed level found nothing) and kept only because
#     the scoping document used it.
ASSUME_TWO_SEEK = "two_seek"
ASSUME_ONE_SEEK = "one_seek"
ASSUME_ORDERED_STEP = "ordered_step_per_failed_level"
ASSUMPTIONS = (ASSUME_TWO_SEEK, ASSUME_ONE_SEEK, ASSUME_ORDERED_STEP)
CODE_ASSUMPTION = ASSUME_TWO_SEEK

COVER = 1
_DESCENT: frozenset[int] = frozenset(range(0, MAX_BRANCH_DEPTH + 1))
_FOUND: frozenset[int] = _DESCENT | frozenset(
    olc_bounds.ordered_read_set_branches(level)
    for level in range(olc_bounds.BRANCH_MIN_LEVEL, olc_bounds.BRANCH_TOP_LEVEL + 1)
)


def _sumset(*parts: frozenset[int]) -> frozenset[int]:
    """Every total reachable by taking one value from each part."""
    totals = frozenset({0})
    for part in parts:
        totals = frozenset(t + v for t in totals for v in part)
    return totals


def _level_costs(kind: str, assumption: str) -> frozenset[int]:
    """Versions one `StrNode` level can retain, by role in the walk.

    Kinds: `above` (descended through an exact continuation, answer is not here),
    `answer` (the level whose sibling answers), `failed_mid` (descended, child failed,
    its resume seek found nothing), `deepest_miss` (deepest level, first seek empty),
    `deepest_suffix_miss` (deepest level, suffix below the remainder, second seek empty),
    `descent` (a level crossed by the min/max descent into the answer's subtree).
    """
    cover = frozenset({COVER})
    if assumption == ASSUME_ORDERED_STEP:
        table = {
            "above": _sumset(cover, _DESCENT),
            "answer": _sumset(cover, _FOUND),
            "failed_mid": _sumset(cover, _FOUND),
            "deepest_miss": _sumset(cover, _FOUND),
            "deepest_suffix_miss": _sumset(cover, _FOUND),
            "descent": _sumset(cover, _DESCENT),
        }
    elif assumption == ASSUME_ONE_SEEK:
        table = {
            "above": _sumset(cover, _DESCENT),
            "answer": _sumset(cover, _FOUND),
            "failed_mid": _sumset(cover, _DESCENT),
            "deepest_miss": _sumset(cover, _DESCENT),
            "deepest_suffix_miss": _sumset(cover, _DESCENT),
            "descent": _sumset(cover, _DESCENT),
        }
    elif assumption == ASSUME_TWO_SEEK:
        table = {
            "above": _sumset(cover, _DESCENT),
            "answer": _sumset(cover, _DESCENT, _FOUND),
            "failed_mid": _sumset(cover, _DESCENT, _DESCENT),
            "deepest_miss": _sumset(cover, _DESCENT),
            "deepest_suffix_miss": _sumset(cover, _DESCENT, _DESCENT),
            "descent": _sumset(cover, _DESCENT),
        }
    else:
        raise ValueError(f"unknown assumption {assumption!r}; expected one of {ASSUMPTIONS}")
    return table[kind]


def enumerate_point_read_sets(
    probe_depth: int,
    answer_depth: int = 0,
    assumption: str = CODE_ASSUMPTION,
    max_depth: int | None = None,
) -> list[tuple[str, frozenset[int]]]:
    """Every shape of a validated ordered point read, with the totals it can retain.

    The probe descends `probe_depth` `StrNode` levels (0..probe_depth-1). The answer is
    taken at level j (a sibling found after the levels below j failed), or there is no
    answer. An answer whose sibling is a child subtree adds up to `answer_depth` descent
    levels, limited by `j + a <= max_depth - 1` (the answer's child sits at level j + 1).
    `max_depth` is the total `StrNode` depth D of the tree, default
    max(probe_depth, answer_depth + 1).

    Returns:
        List of (shape label, set of achievable retained-version totals).

    Raises:
        ValueError: On probe_depth < 1, answer_depth < 0, or an unknown assumption.
    """
    if probe_depth < 1:
        raise ValueError(f"probe_depth must be >= 1, got {probe_depth}")
    if answer_depth < 0:
        raise ValueError(f"answer_depth must be >= 0, got {answer_depth}")
    if assumption not in ASSUMPTIONS:
        raise ValueError(f"unknown assumption {assumption!r}; expected one of {ASSUMPTIONS}")
    if max_depth is None:
        max_depth = max(probe_depth, answer_depth + 1)
    deepest = probe_depth - 1
    shapes: list[tuple[str, frozenset[int]]] = []

    def tail(answer_level: int | None) -> list[tuple[str, frozenset[int]]]:
        # Levels below the answer level (all of them, when there is no answer).
        first = 0 if answer_level is None else answer_level + 1
        if first > deepest:
            return [("", frozenset({0}))]
        mids = [_level_costs("failed_mid", assumption)] * (deepest - first)
        out: list[tuple[str, frozenset[int]]] = []
        for last in ("deepest_miss", "deepest_suffix_miss"):
            label = f"{deepest - first} failed_mid + {last}"
            out.append((label, _sumset(*mids, _level_costs(last, assumption))))
        return out

    for j in [None, *range(probe_depth)]:
        if j is None:
            for label, totals in tail(None):
                shapes.append((f"no answer; {label}", totals))
            continue
        a = min(answer_depth, max_depth - 1 - j)
        if a < 0:
            continue
        head = _sumset(
            *([_level_costs("above", assumption)] * j),
            _level_costs("answer", assumption),
            *([_level_costs("descent", assumption)] * a),
        )
        for label, totals in tail(j):
            shapes.append((f"answer at level {j}, descent {a}; {label}", _sumset(head, totals)))
    return shapes


def str_point_read_set_bound(
    probe_depth: int,
    answer_depth: int = 0,
    assumption: str = CODE_ASSUMPTION,
    max_depth: int | None = None,
) -> int:
    """Maximum versions a validated ordered point read retains: the enumeration's maximum.

    Args:
        probe_depth: `StrNode` levels the probe descends.
        answer_depth: `StrNode` levels the answer's min/max descent crosses below its
            sibling's level.
        assumption: seek-count assumption (default: the one matching the code).
        max_depth: total tree depth D (see `enumerate_point_read_sets`).
    """
    shapes = enumerate_point_read_sets(probe_depth, answer_depth, assumption, max_depth)
    return max(max(totals) for _, totals in shapes)


def str_point_read_set_closed_form(probe_depth: int, answer_depth: int, assumption: str = CODE_ASSUMPTION) -> int:
    """Closed forms the enumeration reduces to at the maximising shape (answer at level 0).

    two_seek: 15d + 6 + 8a; one_seek: 8d + 6 + 8a; ordered_step_per_failed_level: 14d + 8a.
    Used only to cross-check the enumeration; the enumeration is the derivation.
    """
    d, a = probe_depth, answer_depth
    if assumption == ASSUME_TWO_SEEK:
        return 15 * d + 6 + 8 * a
    if assumption == ASSUME_ONE_SEEK:
        return 8 * d + 6 + 8 * a
    if assumption == ASSUME_ORDERED_STEP:
        return 14 * d + 8 * a
    raise ValueError(f"unknown assumption {assumption!r}")


def enumerate_cursor_attempt_read_sets(
    depth: int = 1,
    assumption: str = CODE_ASSUMPTION,
    ancestors_leaf_state: bool = False,
) -> list[tuple[str, frozenset[int]]]:
    """Shapes of one cursor attempt that drains a terminal at `StrNode` level depth-1.

    The attempt descends `depth - 1` ancestor levels through exact continuations, each
    retaining its cover and its descent path (`above`), then works in the final level
    (an answer, or a miss that ends the attempt; a failed child restarts by key, so no
    failed level is retained across attempts). With `ancestors_leaf_state` the ancestors
    are sub-maps in leaf state and retain the cover only.
    """
    if depth < 1:
        raise ValueError(f"depth must be >= 1, got {depth}")
    if assumption not in ASSUMPTIONS:
        raise ValueError(f"unknown assumption {assumption!r}; expected one of {ASSUMPTIONS}")
    ancestor = frozenset({COVER}) if ancestors_leaf_state else _level_costs("above", assumption)
    head = _sumset(*([ancestor] * (depth - 1)))
    return [
        (f"{depth - 1} ancestors + {kind}", _sumset(head, _level_costs(kind, assumption)))
        for kind in ("answer", "deepest_miss", "deepest_suffix_miss")
    ]


def str_cursor_attempt_read_set_bound(
    depth: int = 1,
    assumption: str = CODE_ASSUMPTION,
    ancestors_leaf_state: bool = False,
) -> int:
    """Maximum versions one cursor attempt retains, including its ancestor covers."""
    shapes = enumerate_cursor_attempt_read_sets(depth, assumption, ancestors_leaf_state)
    return max(max(totals) for _, totals in shapes)


def str_single_node_max_read_set(assumption: str = ASSUME_ONE_SEEK) -> int:
    """Maximum versions retained at one `StrNode` level (depth 1 cursor attempt).

    14 under one_seek (1 cover + 13 branches, `olc_bounds.max_ordered_read_set_branches()`);
    21 under two_seek (1 + 7 + 13). Only the one_seek figure fits `READ_SET_CAP = 16`.
    """
    return str_cursor_attempt_read_set_bound(1, assumption)


def classify_read_set_overflow(read_set_len: int, cap: int, validated: bool) -> str:
    """Outcome of a full read set, classified AFTER `validate_all`.

    `ReadSet::sample` returns `Err(Retry)` at `len == cap` (`sync_nav.rs:72-73`,
    `sync_cursor.rs:82-83`). Sound for u64, where overflow implies a moving tree. For
    strings a deep chain overflows deterministically, so a `Retry` burns `MAX_RETRIES`
    (`sync.rs:165`) attempts before `read_locked`. The target policy:
      - validation failed          -> "retry" (the tree moved).
      - validated and len == cap   -> "fallback_now" (depth overflow; go to `read_locked`).
      - otherwise                  -> "ok".
    A walk that completes with exactly `cap` entries is classed "fallback_now" as well:
    conservative, and it costs one `read_locked`.
    """
    if cap < 1 or read_set_len < 0 or read_set_len > cap:
        raise ValueError(f"need 0 <= read_set_len <= cap and cap >= 1, got {read_set_len}, {cap}")
    if not validated:
        return "retry"
    return "fallback_now" if read_set_len == cap else "ok"


def str_read_set_policy() -> dict[str, str | int | bool]:
    """Reconciliation of multi-level read set scaling with `READ_SET_CAP = 16`.

    Returns:
        Policy dictionary defining single-node conformance, resume-by-key, and fallback.
    """
    one = str_single_node_max_read_set(ASSUME_ONE_SEEK)
    two = str_single_node_max_read_set(ASSUME_TWO_SEEK)
    return {
        "read_set_cap": READ_SET_CAP,
        "single_node_max": one,
        "single_node_fits_cap": one <= READ_SET_CAP,
        "single_node_max_two_seek": two,
        "single_node_fits_cap_two_seek": two <= READ_SET_CAP,
        "scoped_terminal_drain": (
            "Batches drain terminal chunks/leaves within the active StrNode under its "
            "cover. One attempt retains the ancestor covers and descent paths plus the final "
            f"level: {one} <= {READ_SET_CAP} at depth 1 under one seek per level, {two} under "
            "the two seeks per level the ported walk runs."
        ),
        "resume_by_key_policy": (
            "Across StrNodes and across batches, the cursor validates the retained "
            "versions, unpins, and resumes from root by key (next_at_or_after), ensuring "
            "no raw node pointers are held across unpins."
        ),
        "overflow_fallback": (
            "Overflow is classified by len == READ_SET_CAP after validate_all: a failed "
            "validation is a moving tree and retries; a passed validation with a full set is "
            "a depth overflow and goes to read_locked immediately, counted separately. "
            "ReadSet::sample returns Err(Retry) at the cap today (sync_nav.rs:72-73, "
            "sync_cursor.rs:82-83), which repeats until MAX_RETRIES = 64 (sync.rs:165) for a "
            "deep chain; the Rust is unchanged in this PR."
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


def strmap_batch_amortized_overhead_per_key(workload: str, avg_batch_size: int = PROJECTED_BATCH_SIZE) -> float:
    """Amortized synchronization and buffering overhead per key in `sync_strmap_scan` (projected).

    Every component below is a hand estimate, not a measurement:
      - Epoch pin/unpin per batch: ~40 instructions (projected).
      - StrNode cover sample/validate per batch: ~15 instructions (projected).
      - Active path branch versions sample/validate: ~35 instructions (projected).
      - Total sync overhead per batch: ~90 instructions (projected).
      - Amortized sync per key: 90 / avg_batch_size; the batch size of 64 is an assumption
        (projected), ~1.4 ins/key (projected).
      - Buffer write + read/dispatch overhead per key: ~3.0 instructions (projected).
      - Total overhead per key: ~4.4 instructions/key (projected).
    The read set of a deep attempt (`str_cursor_attempt_read_set_bound`) validates more
    versions than the 35-instruction path term assumes; that term is not re-derived here.

    Args:
        workload: 'paths' or 'paths_dense'.
        avg_batch_size: Expected average batch size (> 0).

    Returns:
        Overhead in instructions per key.
    """
    if avg_batch_size <= 0:
        raise ValueError(f"avg_batch_size must be > 0, got {avg_batch_size}")
    sync_per_key = PROJECTED_SYNC_INS_PER_BATCH / avg_batch_size
    return sync_per_key + PROJECTED_BUFFER_INS_PER_KEY


def strmap_predicted_callgrind_ratio(workload: str, avg_batch_size: int = PROJECTED_BATCH_SIZE) -> float:
    """Projected Callgrind instruction ratio of `sync_strmap_scan` vs `sync_strmap_scan_locked`.

    Calculated as: (I_baseline + delta_I_overhead) / I_baseline.

    Args:
        workload: 'paths' or 'paths_dense'.
        avg_batch_size: Average batch size.

    Returns:
        Ratio (projected; ~1.02 for the assumed batch).
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

    def test_strnode_depth_is_shared_chunks_not_key_length(self) -> None:
        """Depth follows shared 8-byte prefix chunks (tail collapse), not key length."""
        self.assertEqual(str_strnode_depth(0), 1)
        self.assertEqual(str_strnode_depth(7), 1)
        self.assertEqual(str_strnode_depth(8), 2)
        self.assertEqual(str_strnode_depth(31), 4)
        self.assertEqual(str_strnode_depth(32), 5)
        # `paths`: 35 shared bytes, 47-byte keys. Key length would say 6.
        self.assertEqual(str_strnode_depth(35), 5)
        self.assertEqual(str_chunk_count(47), 6)
        with self.assertRaises(ValueError):
            str_strnode_depth(-1)

    def test_point_read_set_bound_reference_values(self) -> None:
        """Pin the enumeration for D = 1..5 with answer depth D - 1, per seek-count assumption."""
        depths = range(1, 6)
        two = [str_point_read_set_bound(d, d - 1, ASSUME_TWO_SEEK) for d in depths]
        one = [str_point_read_set_bound(d, d - 1, ASSUME_ONE_SEEK) for d in depths]
        step = [str_point_read_set_bound(d, d - 1, ASSUME_ORDERED_STEP) for d in depths]
        self.assertEqual(two, [21, 44, 67, 90, 113])  # 15d + 6 + 8a: matches the code
        self.assertEqual(one, [14, 30, 46, 62, 78])  # 16D - 2
        self.assertEqual(step, [14, 36, 58, 80, 102])  # 22D - 8, failed levels over-charged
        self.assertEqual(str_point_read_set_bound(1), 21)  # default assumption is the code's
        self.assertEqual(CODE_ASSUMPTION, ASSUME_TWO_SEEK)

    def test_point_read_set_enumeration_equals_closed_form(self) -> None:
        """The enumeration reduces to its closed form over the whole depth grid."""
        for assumption in ASSUMPTIONS:
            for d in range(1, 6):
                for a in range(0, 5):
                    self.assertEqual(
                        str_point_read_set_bound(d, a, assumption),
                        str_point_read_set_closed_form(d, a, assumption),
                        (d, a, assumption),
                    )

    def test_point_read_set_maximum_is_answer_at_top_level(self) -> None:
        """The maximising shape takes the answer at level 0 and no shape beats it."""
        shapes = enumerate_point_read_sets(4, 3, ASSUME_TWO_SEEK)
        best = max(shapes, key=lambda s: max(s[1]))
        self.assertTrue(best[0].startswith("answer at level 0"), best[0])
        # An answer at level 3 (the deepest) retains less: 3 above + answer, descent limited to 0.
        deepest = [s for s in shapes if s[0].startswith("answer at level 3")]
        self.assertTrue(all(max(t) < 90 for _, t in deepest))
        # A probe with no answer is bounded by the answered shape.
        no_answer = [s for s in shapes if s[0].startswith("no answer")]
        self.assertTrue(no_answer)
        self.assertLess(max(max(t) for _, t in no_answer), 90)

    def test_point_read_set_bound_rejects_bad_inputs(self) -> None:
        with self.assertRaises(ValueError):
            str_point_read_set_bound(0)
        with self.assertRaises(ValueError):
            str_point_read_set_bound(1, -1)
        with self.assertRaises(ValueError):
            str_point_read_set_bound(1, 0, "three_seek")

    def test_cursor_attempt_read_set_bound_reference_values(self) -> None:
        """Pin the cursor attempt bound, ancestor covers included."""
        depths = range(1, 6)
        one = [str_cursor_attempt_read_set_bound(d, ASSUME_ONE_SEEK) for d in depths]
        two = [str_cursor_attempt_read_set_bound(d, ASSUME_TWO_SEEK) for d in depths]
        self.assertEqual(one, [14, 22, 30, 38, 46])  # 8d + 6
        self.assertEqual(two, [21, 29, 37, 45, 53])  # 8d + 13
        # Ancestors that are sub-maps in leaf state retain the cover only: `paths` reaches
        # 18 at depth 5 under one seek (projected: the leaf-state chain is derived from the
        # key shape, not counted on a built map).
        leaf_one = [str_cursor_attempt_read_set_bound(d, ASSUME_ONE_SEEK, True) for d in depths]
        leaf_two = [str_cursor_attempt_read_set_bound(d, ASSUME_TWO_SEEK, True) for d in depths]
        self.assertEqual(leaf_one, [14, 15, 16, 17, 18])
        self.assertEqual(leaf_two, [21, 22, 23, 24, 25])
        self.assertEqual(str_cursor_attempt_read_set_bound(), 21)
        with self.assertRaises(ValueError):
            str_cursor_attempt_read_set_bound(0)

    def test_multi_level_read_set_exceeds_cap(self) -> None:
        """A multi-level attempt exceeds READ_SET_CAP; the 14 <= 16 claim omits covers and a seek."""
        self.assertEqual(str_cursor_attempt_read_set_bound(1, ASSUME_ONE_SEEK), 14)
        self.assertLessEqual(str_cursor_attempt_read_set_bound(1, ASSUME_ONE_SEEK), READ_SET_CAP)
        self.assertEqual(str_cursor_attempt_read_set_bound(2, ASSUME_ONE_SEEK), 22)
        self.assertGreater(str_cursor_attempt_read_set_bound(2, ASSUME_ONE_SEEK), READ_SET_CAP)
        self.assertEqual(str_cursor_attempt_read_set_bound(1, ASSUME_TWO_SEEK), 21)
        self.assertGreater(str_cursor_attempt_read_set_bound(1, ASSUME_TWO_SEEK), READ_SET_CAP)
        self.assertEqual(str_point_read_set_bound(2, 1, ASSUME_TWO_SEEK), 44)
        self.assertGreater(str_point_read_set_bound(2, 1, ASSUME_TWO_SEEK), READ_SET_CAP)
        self.assertEqual(str_point_read_set_bound(3, 2, ASSUME_ONE_SEEK), 46)
        self.assertGreater(str_point_read_set_bound(3, 2, ASSUME_ONE_SEEK), READ_SET_CAP)

    def test_single_node_read_set_fits_cap(self) -> None:
        """One level: 14 under one seek (fits the cap), 21 under the code's two seeks."""
        self.assertEqual(str_single_node_max_read_set(), 14)
        self.assertLessEqual(str_single_node_max_read_set(ASSUME_ONE_SEEK), READ_SET_CAP)
        self.assertEqual(str_single_node_max_read_set(ASSUME_TWO_SEEK), 21)
        self.assertGreater(str_single_node_max_read_set(ASSUME_TWO_SEEK), READ_SET_CAP)

    def test_overflow_classification(self) -> None:
        """A full set is a depth overflow only after validate_all passed."""
        self.assertEqual(classify_read_set_overflow(16, 16, validated=False), "retry")
        self.assertEqual(classify_read_set_overflow(16, 16, validated=True), "fallback_now")
        self.assertEqual(classify_read_set_overflow(15, 16, validated=True), "ok")
        self.assertEqual(classify_read_set_overflow(3, 16, validated=False), "retry")
        with self.assertRaises(ValueError):
            classify_read_set_overflow(17, 16, validated=True)

    def test_read_set_policy_structure(self) -> None:
        """Verify read set policy dictionary contents."""
        p = str_read_set_policy()
        self.assertEqual(p["read_set_cap"], 16)
        self.assertEqual(p["single_node_max"], 14)
        self.assertTrue(p["single_node_fits_cap"])
        self.assertEqual(p["single_node_max_two_seek"], 21)
        self.assertFalse(p["single_node_fits_cap_two_seek"])
        self.assertIn("terminal chunks/leaves", str(p["scoped_terminal_drain"]))
        self.assertIn("resumes from root by key", str(p["resume_by_key_policy"]))
        overflow = str(p["overflow_fallback"])
        self.assertIn("len == READ_SET_CAP after validate_all", overflow)
        self.assertIn("read_locked immediately", overflow)
        self.assertNotIn("triggering", overflow)

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

    def test_projected_constants_are_pinned_and_tagged(self) -> None:
        """The overhead inputs are hand estimates; pin them and keep the tag in the docs."""
        self.assertEqual(PROJECTED_SYNC_INS_PER_BATCH, 90.0)
        self.assertEqual(PROJECTED_BATCH_SIZE, 64)
        self.assertEqual(PROJECTED_BUFFER_INS_PER_KEY, 3.0)
        self.assertAlmostEqual(strmap_batch_amortized_overhead_per_key("paths"), 90 / 64 + 3.0)
        doc = strmap_batch_amortized_overhead_per_key.__doc__ or ""
        self.assertGreaterEqual(doc.count("(projected)"), 7)
        self.assertIn("projected", strmap_predicted_callgrind_ratio.__doc__ or "")

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
    print("1. StrNode depth (tail collapse: shared 8-byte prefix chunks + 1, not key length):")
    for shared in (0, 7, 8, 32, 35):
        print(f"   {shared:>2} shared prefix bytes: depth {str_strnode_depth(shared)}")
    print()
    print(f"2. Retained read set by ENUMERATION of the walk (READ_SET_CAP = {READ_SET_CAP}):")
    print("   point op, probe depth d = D, answer depth a = D - 1 (maximum over answer level):")
    print("     assumption                          D=1    2    3    4    5")
    for assumption in (ASSUME_TWO_SEEK, ASSUME_ONE_SEEK, ASSUME_ORDERED_STEP):
        row = [str_point_read_set_bound(d, d - 1, assumption) for d in range(1, 6)]
        tag = "  <- matches the code" if assumption == CODE_ASSUMPTION else ""
        print(f"     {assumption:<34}  {row[0]:>3} {row[1]:>4} {row[2]:>4} {row[3]:>4} {row[4]:>4}{tag}")
    print("   cursor attempt, ancestor covers included:")
    for assumption in (ASSUME_TWO_SEEK, ASSUME_ONE_SEEK):
        worst = [str_cursor_attempt_read_set_bound(d, assumption) for d in range(1, 6)]
        leaf = [str_cursor_attempt_read_set_bound(d, assumption, True) for d in range(1, 6)]
        print(f"     {assumption:<34}  worst {worst}")
        print(f"     {'':<34}  leaf-state ancestors (projected, not counted on a built map) {leaf}")
    p = str_read_set_policy()
    print(
        f"   Single StrNode: {p['single_node_max']} (one seek, fits cap) / {p['single_node_max_two_seek']} (two seeks, exceeds cap)"
    )
    print(f"   Overflow: {p['overflow_fallback']}")
    print(f"   Policy: {p['scoped_terminal_drain']}")
    print(f"           {p['resume_by_key_policy']}")
    print()
    print("3. Variable-Length Key Buffer Budget & Oversized Key Policy:")
    bp = str_oversized_key_policy()
    print(f"   Key bytes arena budget:           {bp['key_buffer_budget_bytes']} bytes (4 KiB)")
    print(
        f"   Entry descriptors:                {bp['batch_cap_entries']} entries * {bp['entry_descriptor_bytes']} bytes = 4096 bytes (4 KiB)"
    )
    print(f"   Total cursor buffer size:         {bp['total_cursor_buffer_bytes']} bytes (8 KiB, fits L1 cache)")
    print("   Batch entry capacity:")
    for k in (8, 32, 64, 4096):
        cap = str_batch_capacity_bound(k)
        print(f"     Avg key length K = {k:<4} bytes -> batch capacity = {cap} entries")
    print(f"   Oversized key policy: {bp['oversized_key_rule']}")
    print()
    print("4. Callgrind Instruction Predictions vs sync_strmap_scan_locked (projected):")
    for wl, ceiling in TARGET_CALLGRIND_RATIOS.items():
        base = strmap_baseline_instructions_per_key(wl)
        ovh = strmap_batch_amortized_overhead_per_key(wl)
        pred = strmap_predicted_callgrind_ratio(wl)
        meta = STRMAP_BASELINE_MEASURED[wl]
        print(
            f"   {wl:<12}: baseline {base:.1f} ins/key (measured: CI run {meta['run_id']}, {meta['commit'][:12]}) "
            f"+ ~{ovh:.1f} ins/key overhead (projected, batch {PROJECTED_BATCH_SIZE} assumed) "
            f"-> ratio {pred:.3f} (projected); ceiling {ceiling:.2f} (target)"
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
