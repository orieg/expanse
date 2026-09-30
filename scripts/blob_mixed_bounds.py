#!/usr/bin/env python3
"""Bounds behind the `SyncExpanseBlobMap` mixed-workload work (#1280, AGENTS.md §8.8 commit 1).

Committed, unit-tested arithmetic for two things the pre-registration
(`docs/benchmarks/concurrency/METHODOLOGY.md` §26) relies on:

1. The repaired `core_concurrency` blob arms stay under the arena's capacity
   cap by construction. Every insert appends a record; `BLOB_COMPACT_APPENDS`
   bounds how many appends accumulate between compactions, and each writer
   slot may hold one private chunk besides. `arena_high_water_bytes` is that
   ceiling and `cap_margin` its distance from the cap.
2. What a serialised removal can cost a mixed cell. Under the utilization law
   a section that every one of a share `f` of operations must hold alone, for
   `s` seconds each, caps throughput at `1 / (f * s)` whatever the thread
   count. The mixed workload's write bit fixes `f` per build: every removal
   serialises on the parent build, only removals of a present key under the
   miss short-circuit (M), and only fallbacks under the optimistic removal
   (C). `s` is not derivable; it is the census's measured quiesce section, an
   empirical residual the pre-registration reads from a run.

Sources
-------
Denning, Buzen, "The Operational Analysis of Queueing Network Models", ACM
  Computing Surveys 10(3), 1978. The utilization law U = X * S: a device busy
  S per completion cannot complete more than 1 / S per unit time. Applied here
  to a section serialised across all threads and visited by a share f of
  operations: U = X * f * s <= 1.
Engine and harness constants, read by the self-test from the files below so a
  change there fails it: `DEFAULT_CHUNK_SIZE`, `MAX_ARENA_CAPACITY`,
  `ARENA_ALIGN` (crates/expanse/src/blobmap.rs), `MAX_WRITER_SLOTS`
  (crates/expanse/src/occ.rs, the 64-bit value), `BLOB_POP`, `BLOB_LEN`,
  `BLOB_COMPACT_APPENDS` (crates/expanse/benches/concurrency.rs). The record
  layout — an 8-byte header, the payload, the private cursor rounded to 16
  bytes — is `SyncExpanseBlobMap::alloc_private` (crates/expanse/src/sync.rs).

Usage:
    python3 scripts/blob_mixed_bounds.py             # the bounds, then the self-test
    python3 scripts/blob_mixed_bounds.py --self-test # the self-test only
"""

from __future__ import annotations

import argparse
import math
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
BLOBMAP_RS = REPO_ROOT / "crates" / "expanse" / "src" / "blobmap.rs"
OCC_RS = REPO_ROOT / "crates" / "expanse" / "src" / "occ.rs"
SYNC_RS = REPO_ROOT / "crates" / "expanse" / "src" / "sync.rs"
CONCURRENCY_RS = REPO_ROOT / "crates" / "expanse" / "benches" / "concurrency.rs"

RECORD_HEADER_BYTES = 8
ARENA_ALIGN = 16
DEFAULT_CHUNK_SIZE = 2 * 1024 * 1024
MAX_ARENA_CAPACITY = 1 << 30
MAX_WRITER_SLOTS = 64
BLOB_POP = 200_000
BLOB_LEN = 128
BLOB_COMPACT_APPENDS = BLOB_POP
# The mixed workloads' write bit: a write inserts with this probability and
# removes otherwise, on a uniform key.
INSERT_PROBABILITY = 0.5
# The registered margin: the high-water ceiling must fit under the cap twice.
REQUIRED_CAP_MARGIN = 2.0


def _positive(name: str, value: float) -> None:
    if not (isinstance(value, (int, float)) and math.isfinite(value) and value > 0):
        raise ValueError(f"{name} must be a positive finite number, got {value!r}")


def _share(name: str, value: float) -> None:
    if not (isinstance(value, (int, float)) and 0.0 <= value <= 1.0):
        raise ValueError(f"{name} must lie in [0, 1], got {value!r}")


def record_bytes(payload_len: int, header: int = RECORD_HEADER_BYTES, align: int = ARENA_ALIGN) -> int:
    """Arena bytes one record occupies: header plus payload, rounded up to `align`."""
    if payload_len < 0 or header < 0:
        raise ValueError("payload_len and header must be non-negative")
    _positive("align", align)
    raw = header + payload_len
    return -(-raw // align) * align


def appends_to_cap(cap_bytes: int, live_records: int, payload_len: int) -> int:
    """Inserts an arena holding `live_records` takes before it reaches `cap_bytes`, never compacted."""
    _positive("cap_bytes", cap_bytes)
    if live_records < 0:
        raise ValueError("live_records must be non-negative")
    rec = record_bytes(payload_len)
    free = cap_bytes - live_records * rec
    if free < 0:
        raise ValueError("the live records alone exceed the cap")
    return free // rec


def arena_high_water_bytes(live_records: int, compact_appends: int, payload_len: int,
                           writer_slots: int, chunk_bytes: int) -> int:
    """Ceiling on arena bytes under the harness's compaction trigger.

    Between compactions at most `compact_appends` records are appended beside
    the live ones, and each writer slot may additionally hold one private chunk
    whose unwritten tail is charged but not yet filled.
    """
    if min(live_records, compact_appends, writer_slots) < 0:
        raise ValueError("counts must be non-negative")
    _positive("chunk_bytes", chunk_bytes)
    rec = record_bytes(payload_len)
    return (live_records + compact_appends) * rec + writer_slots * chunk_bytes


def cap_margin(cap_bytes: int, high_water_bytes: int) -> float:
    """How many times the ceiling fits under the cap."""
    _positive("cap_bytes", cap_bytes)
    _positive("high_water_bytes", high_water_bytes)
    return cap_bytes / high_water_bytes


def equilibrium_occupancy(insert_probability: float) -> float:
    """Steady-state share of a uniform keyspace present when each write inserts
    with probability p and removes otherwise, on a uniform key.

    Per write aimed at a key it enters with probability p * (1 - x) and leaves
    with probability (1 - p) * x; the two balance at x = p.
    """
    _share("insert_probability", insert_probability)
    return insert_probability


def remove_hit_share(insert_probability: float, occupancy: float) -> float:
    """Share of writes that remove a present key."""
    _share("insert_probability", insert_probability)
    _share("occupancy", occupancy)
    return (1.0 - insert_probability) * occupancy


def serialised_share(read_share: float, insert_probability: float, occupancy: float, build: str) -> float:
    """Share of all operations that take the blob removal's serialised section, by build.

    `parent`: every removal, hit or miss. `miss_shortcut`: removals of a
    present key only. `optimistic`: none by construction; its fallbacks are a
    measured residual, not a derivable share.
    """
    _share("read_share", read_share)
    _share("insert_probability", insert_probability)
    _share("occupancy", occupancy)
    removes = (1.0 - read_share) * (1.0 - insert_probability)
    if build == "parent":
        return removes
    if build == "miss_shortcut":
        return removes * occupancy
    if build == "optimistic":
        return 0.0
    raise ValueError(f"unknown build {build!r}")


def utilization_ceiling(serial_share: float, section_s: float) -> float:
    """Upper bound on total ops/s when a share of operations each hold one
    all-thread serialised section for `section_s` seconds (U = X * f * s <= 1)."""
    _share("serial_share", serial_share)
    _positive("section_s", section_s)
    if serial_share == 0.0:
        return math.inf
    return 1.0 / (serial_share * section_s)


def compactions_per_s(total_ops_s: float, read_share: float, insert_probability: float,
                      compact_appends: int) -> float:
    """Compactions per second the harness trigger fires at a given throughput."""
    _positive("compact_appends", compact_appends)
    if total_ops_s < 0:
        raise ValueError("total_ops_s must be non-negative")
    _share("read_share", read_share)
    _share("insert_probability", insert_probability)
    return total_ops_s * (1.0 - read_share) * insert_probability / compact_appends


def _int_expr(expr: str) -> int:
    """An integer literal, a product of literals, or `a << b`, as the Rust sources spell them."""
    expr = expr.replace("_", "").strip()
    if "<<" in expr:
        a, b = expr.split("<<")
        return int(a) << int(b)
    out = 1
    for part in expr.split("*"):
        out *= int(part)
    return out


def _read_const(path: Path, pattern: str) -> int:
    m = re.search(pattern, path.read_text(), re.MULTILINE)
    if not m:
        raise AssertionError(f"{path.relative_to(REPO_ROOT)}: pattern {pattern!r} not found")
    return _int_expr(m.group(1))


def self_test() -> int:
    def raises(fn, *args) -> None:
        try:
            fn(*args)
        except ValueError:
            return
        raise AssertionError(f"{fn.__name__}{args} did not raise")

    # The mirrored constants still match the code.
    assert _read_const(BLOBMAP_RS, r"^pub const DEFAULT_CHUNK_SIZE: usize = ([0-9_ *]+);") == DEFAULT_CHUNK_SIZE
    assert _read_const(BLOBMAP_RS, r"^pub const MAX_ARENA_CAPACITY: usize = ([0-9_ <]+);") == MAX_ARENA_CAPACITY
    assert _read_const(BLOBMAP_RS, r"^pub const ARENA_ALIGN: usize = ([0-9_]+);") == ARENA_ALIGN
    assert MAX_WRITER_SLOTS in [
        _int_expr(v) for v in re.findall(r"const MAX_WRITER_SLOTS: usize = ([0-9_]+);", OCC_RS.read_text())
    ]
    assert _read_const(CONCURRENCY_RS, r"^const BLOB_POP: u64 = ([0-9_]+);") == BLOB_POP
    assert _read_const(CONCURRENCY_RS, r"^const BLOB_LEN: usize = ([0-9_]+);") == BLOB_LEN
    assert re.search(r"^const BLOB_COMPACT_APPENDS: u64 = BLOB_POP;", CONCURRENCY_RS.read_text(), re.M)
    assert "(off + needed + 15) & !15" in SYNC_RS.read_text()
    assert _int_expr("2 * 1024 * 1024") == DEFAULT_CHUNK_SIZE and _int_expr("1 << 30") == MAX_ARENA_CAPACITY

    # Record size: 8 + 128 = 136, rounded to 144.
    assert record_bytes(128) == 144
    assert record_bytes(8) == 16 and record_bytes(0) == 16 and record_bytes(9) == 32
    raises(record_bytes, -1)

    # The pre-#1280 harness: 200k live records under the 1 GiB cap and no
    # compaction. (2^30 - 200000 * 144) / 144 = 7,256,540.4 inserts, about
    # 14.5 M writes at 50/50 insert/remove.
    assert appends_to_cap(MAX_ARENA_CAPACITY, BLOB_POP, BLOB_LEN) == 7_256_540
    raises(appends_to_cap, 1000, 100, BLOB_LEN)

    # The repaired harness: (200k + 200k) * 144 + 64 * 2 MiB = 57,600,000 + 134,217,728.
    hw = arena_high_water_bytes(BLOB_POP, BLOB_COMPACT_APPENDS, BLOB_LEN, MAX_WRITER_SLOTS, DEFAULT_CHUNK_SIZE)
    assert hw == 191_817_728
    margin = cap_margin(MAX_ARENA_CAPACITY, hw)
    assert abs(margin - 5.5977) < 1e-3, margin
    assert margin >= REQUIRED_CAP_MARGIN

    # The write bit holds half the keyspace, and a quarter of writes remove a present key.
    occ = equilibrium_occupancy(INSERT_PROBABILITY)
    assert occ == 0.5
    assert remove_hit_share(INSERT_PROBABILITY, occ) == 0.25
    raises(remove_hit_share, 1.5, 0.5)

    # Serialised shares at 50 % read: every removal on the parent (0.25 of
    # ops), present-key removals under the miss short-circuit (0.125), none
    # by construction under the optimistic removal.
    assert serialised_share(0.5, INSERT_PROBABILITY, occ, "parent") == 0.25
    assert serialised_share(0.5, INSERT_PROBABILITY, occ, "miss_shortcut") == 0.125
    assert serialised_share(0.5, INSERT_PROBABILITY, occ, "optimistic") == 0.0
    raises(serialised_share, 0.5, 0.5, 0.5, "other")

    # Utilization law: a quarter of ops each holding a 400 ns section caps throughput at 10 M ops/s.
    assert abs(utilization_ceiling(0.25, 400e-9) - 10e6) < 1e-3
    assert utilization_ceiling(0.0, 1e-6) == math.inf
    raises(utilization_ceiling, 0.25, 0.0)

    # Compaction rate: 20 M ops/s at 50/50, half the writes inserting, K = 200k: 25 per second.
    assert compactions_per_s(20e6, 0.5, INSERT_PROBABILITY, BLOB_COMPACT_APPENDS) == 25.0
    print("blob_mixed_bounds.py self-test PASSED")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("--self-test", action="store_true", help="run the self-test only")
    args = ap.parse_args()
    if not args.self_test:
        hw = arena_high_water_bytes(BLOB_POP, BLOB_COMPACT_APPENDS, BLOB_LEN, MAX_WRITER_SLOTS, DEFAULT_CHUNK_SIZE)
        occ = equilibrium_occupancy(INSERT_PROBABILITY)
        print(f"record bytes ({BLOB_LEN} B payload): {record_bytes(BLOB_LEN)}")
        print(f"inserts to the cap without compaction: {appends_to_cap(MAX_ARENA_CAPACITY, BLOB_POP, BLOB_LEN):,}")
        print(f"arena high-water ceiling under the trigger: {hw:,} B, cap margin "
              f"{cap_margin(MAX_ARENA_CAPACITY, hw):.2f}x")
        print(f"equilibrium occupancy {occ}, remove-hit share of writes {remove_hit_share(INSERT_PROBABILITY, occ)}")
        for build in ("parent", "miss_shortcut", "optimistic"):
            print(f"serialised share of ops at 50% read, {build}: "
                  f"{serialised_share(0.5, INSERT_PROBABILITY, occ, build)}")
    return self_test()


if __name__ == "__main__":
    sys.exit(main())
