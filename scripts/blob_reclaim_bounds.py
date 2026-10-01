#!/usr/bin/env python3
"""Bounds behind engine-driven blob-arena reclamation (#1290, AGENTS.md §8.8 commit 1).

The rule (`docs/design/large-values.md` §6.3.1, amended by METHODOLOGY §28a):
when an insert cannot grow the arena because it would cross `max_capacity` (or
`MAX_ARENA_CHUNKS`), the engine compacts once and retries the insert once,
only if both hold:

(A) the copy budget: the live bytes, which the compaction copies, are at most
    `RECLAIM_COPY_PER_GROWTH` times the bytes it can have freed since its
    previous compaction (or since the arena was created or cleared): the chunk
    bytes the arena grew by, plus the net drop in live bytes. Growth alone
    would never fire again for an arena filled with live records and then
    emptied by removals, since at the cap it cannot grow;
(B) the waste guard (§28a): the live bytes are under half the allocated chunk
    bytes, `2 * live_bytes < total_allocated`. An arena filled with live
    records never passes it, so a bulk load that reaches the cap fails without
    a copy that frees nothing, and no automatic copy reaches half the cap.

Otherwise the insert fails with `ArenaError::OffsetOverflow`.

The functions below answer, for a single-size workload that overwrites a fixed
set of live keys forever (the shape that fills the arena in #1280):

1. how records pack into chunks (`records_per_chunk`, `compacted_chunks`),
   mirroring `ArenaChunk::alloc` and `BlobArena::alloc_blob`;
2. whether the rule keeps such a workload running at the cap indefinitely
   (`sustains_overwrite`), and the largest live set for which it does
   (`max_sustained_live_records`);
3. what it costs: bytes copied per byte appended in the steady state
   (`copy_per_append`), the largest single compaction (`stall_copy_bytes`,
   under `max_stall_bytes` by (B)), and the transient memory a compaction
   holds (`compaction_peak_bytes`).

By construction a compaction copies at most `RECLAIM_COPY_PER_GROWTH` bytes
per byte of growth or removal since the previous one, and both reset at every
compaction, so no workload can make the rule compact twice without inserting
or removing in between. `simulate` is a
record-level model of the allocator, independent of the closed forms, and the
self-test checks the two against each other on small arenas.

Sources
-------
Engine constants, read by the self-test from `crates/expanse/src/blobmap.rs`
  so a change there fails it: `DEFAULT_CHUNK_SIZE`, `DEFAULT_ARENA_CAPACITY`,
  `MAX_ARENA_CHUNKS`, `ARENA_ALIGN`, `RECLAIM_COPY_PER_GROWTH`, and the rule's
  lines in `BlobArena::reclaim_allowed`. The record layout — an 8-byte header, the
  payload, the chunk cursor rounded up to 16 bytes after each record, a record
  fitting when `cursor + 8 + len <= capacity`, and a new chunk refused when
  `total_allocated + chunk_size > max_capacity` — is `ArenaChunk::alloc` and
  `BlobArena::alloc_blob` in the same file. Compaction packs every live record
  into a fresh arena in index order (`BlobArena::compact_with_index`), which
  for a single record size is the dense prefix `simulate` models.
The amortization argument is the standard one for rebuild-on-threshold
  structures: charge each rebuild to the insertions since the previous one
  (Cormen, Leiserson, Rivest, Stein, "Introduction to Algorithms", 3rd ed.,
  §17.4, dynamic tables).

Usage:
    python3 scripts/blob_reclaim_bounds.py             # the bounds, then the self-test
    python3 scripts/blob_reclaim_bounds.py --self-test # the self-test only
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
BLOBMAP_RS = REPO_ROOT / "crates" / "expanse" / "src" / "blobmap.rs"
# METHODOLOGY §27's build D runs: each blob arm's in-window compactions of the
# harness's 200,000 live records, timed (`compact_ns`) and counted per window.
S27_D_ARTIFACTS = tuple(
    REPO_ROOT / "docs" / "benchmarks" / "concurrency" / "results" / f"gate_1280_compaction_d_run{r}.json"
    for r in (1, 2)
)
S27_LIVE_RECORDS = 200_000
# METHODOLOGY §29's live sets: the #1280 harness's, and one near the rule's
# sustained maximum (3,830,069 at the default geometry) that the waste guard
# admits (2 * 3,600,000 * 136 < 2^30).
STALL_LIVE_SETS = (200_000, 3_600_000)
# README §29.1's Step 0: per-phase compaction timings from a diagnostic build
# (`probe.patch` beside it), one `PHASE` line per compaction and, on the
# concurrent map, one `SYNC` line per exclusive section.
STEP0_PROBE_LOG = (REPO_ROOT / "docs" / "benchmarks" / "concurrency" / "results"
                   / "step0_1300_phases" / "probe.log")

RECORD_HEADER_BYTES = 8
ARENA_ALIGN = 16
DEFAULT_CHUNK_SIZE = 2 * 1024 * 1024
DEFAULT_ARENA_CAPACITY = 1 << 30
MAX_ARENA_CHUNKS = 1 << 16
# The proposed rule's one parameter: a compaction may copy at most this many
# bytes per byte of chunk growth since the previous compaction.
RECLAIM_COPY_PER_GROWTH = 1
# The #1280 harness's blob workload: 200,000 live 128-byte payloads.
HARNESS_LIVE = 200_000
HARNESS_LEN = 128


def _nonneg_int(name: str, value: int) -> None:
    if not isinstance(value, int) or value < 0:
        raise ValueError(f"{name} must be a non-negative integer, got {value!r}")


def _pos_int(name: str, value: int) -> None:
    if not isinstance(value, int) or value <= 0:
        raise ValueError(f"{name} must be a positive integer, got {value!r}")


def record_needed(payload_len: int) -> int:
    """Bytes `live_bytes` charges for one record: header plus payload, unaligned."""
    _nonneg_int("payload_len", payload_len)
    return RECORD_HEADER_BYTES + payload_len


def record_stride(payload_len: int) -> int:
    """Cursor advance per record: `needed` rounded up to `ARENA_ALIGN`."""
    n = record_needed(payload_len)
    return -(-n // ARENA_ALIGN) * ARENA_ALIGN


def records_per_chunk(payload_len: int, chunk: int) -> int:
    """Records of one size a chunk holds: the largest i with (i-1)*stride + needed <= chunk."""
    _pos_int("chunk", chunk)
    needed = record_needed(payload_len)
    if needed > chunk:
        raise ValueError("a record larger than a chunk is refused (AllocationFailed)")
    return (chunk - needed) // record_stride(payload_len) + 1


def max_chunks(chunk: int, cap: int) -> int:
    """Chunks the arena may hold: growth is refused once total + chunk > cap, or at MAX_ARENA_CHUNKS."""
    _pos_int("chunk", chunk)
    _pos_int("cap", cap)
    return min(cap // chunk, MAX_ARENA_CHUNKS)


def compacted_chunks(live_records: int, payload_len: int, chunk: int) -> int:
    """Chunks a compaction leaves: the live records packed densely from chunk 0."""
    _nonneg_int("live_records", live_records)
    per = records_per_chunk(payload_len, chunk)
    return -(-live_records // per)


def sustains_overwrite(live_records: int, payload_len: int, chunk: int, cap: int,
                       k: int = RECLAIM_COPY_PER_GROWTH) -> bool:
    """Whether the rule keeps an overwrite-forever workload running at the cap.

    After the first compaction every cycle is the same: the arena grows from
    `compacted_chunks` to `max_chunks`, and at the next refused growth the rule
    compacts iff live_bytes <= k * (max_chunks - compacted_chunks) * chunk
    (an overwrite leaves the live bytes unchanged, so the removal term is
    zero) and 2 * live_bytes < max_chunks * chunk (the waste guard).
    The first compaction's growth is the whole arena, so the steady state is
    the binding cycle. The compacted layout must also leave room for one more
    record, or the retry fails.
    """
    _pos_int("k", k)
    mc = max_chunks(chunk, cap)
    per = records_per_chunk(payload_len, chunk)
    if live_records >= mc * per:
        return False
    cc = compacted_chunks(live_records, payload_len, chunk)
    growth = (mc - cc) * chunk
    live = live_records * record_needed(payload_len)
    return live <= k * growth and 2 * live < mc * chunk


def max_sustained_live_records(payload_len: int, chunk: int, cap: int,
                               k: int = RECLAIM_COPY_PER_GROWTH) -> int:
    """The largest live set `sustains_overwrite` accepts (monotone in live_records)."""
    lo, hi = 0, max_chunks(chunk, cap) * records_per_chunk(payload_len, chunk)
    if not sustains_overwrite(0, payload_len, chunk, cap, k):
        return -1
    while lo < hi:
        mid = (lo + hi + 1) // 2
        if sustains_overwrite(mid, payload_len, chunk, cap, k):
            lo = mid
        else:
            hi = mid - 1
    return lo


def live_share_of_cap(live_records: int, payload_len: int, chunk: int, cap: int) -> float:
    """Live records as a share of the records the capped arena can hold at all."""
    return live_records / (max_chunks(chunk, cap) * records_per_chunk(payload_len, chunk))


def copy_per_append(live_records: int, payload_len: int, chunk: int, cap: int) -> float:
    """Steady-state bytes a compaction copies per byte appended since the previous one."""
    total = max_chunks(chunk, cap) * records_per_chunk(payload_len, chunk)
    appends = total - live_records
    if appends <= 0:
        raise ValueError("the live records fill the arena; nothing is appended")
    return live_records / appends


def stall_copy_bytes(live_records: int, payload_len: int) -> int:
    """Bytes the single compaction inside a triggering insert copies: every live record."""
    return live_records * record_needed(payload_len)


def max_stall_bytes(cap: int) -> int:
    """Largest live-byte copy an automatic compaction can make: the waste guard
    requires 2 * live_bytes < total_allocated <= cap, so live_bytes < cap / 2."""
    _pos_int("cap", cap)
    return (cap - 1) // 2


def s27_ns_per_record(paths: tuple = S27_D_ARTIFACTS, live_records: int = S27_LIVE_RECORDS) -> list[float]:
    """Nanoseconds per live record of one compaction, per §27 blob cell at 50 % read.

    Each cell's summed `compact_ns` over its compaction count, divided by the
    live records each compaction copied. Measured on the reference host by the
    committed §27 runs; a cell without compactions is skipped.
    """
    _pos_int("live_records", live_records)
    out = []
    for path in paths:
        art = json.loads(Path(path).read_text())
        for cell in art["throughput"]:
            if cell["engine_key"] in ("blob", "blob_mutex") and cell["read_pct"] == 50:
                rows = cell["rounds_raw"]
                n = sum(w.get("compactions", 0) for w in rows)
                if n:
                    out.append(sum(w.get("compact_ns", 0) for w in rows) / n / live_records)
    if not out:
        raise ValueError("no §27 cell carries compactions")
    return out


def predicted_stall_ns(live_records: int, ns_per_record: float) -> float:
    """The stall a compaction of `live_records` predicts if its cost is linear in them."""
    _nonneg_int("live_records", live_records)
    if not ns_per_record > 0:
        raise ValueError("ns_per_record must be positive")
    return live_records * ns_per_record


def step0_sync_sections(path: Path = STEP0_PROBE_LOG) -> dict[int, list[dict]]:
    """README §29.1's concurrent exclusive sections, keyed by live records.

    Each entry pairs one compaction's `PHASE` timings with the `SYNC` line of
    the exclusive section that ran it: `inside_ns` is the time the tree
    bracket was open, so every read that started in it waited.
    """
    out: dict[int, list[dict]] = {}
    live = phase = None
    is_sync = False
    for line in Path(path).read_text().splitlines():
        if line.startswith("== "):
            fields = dict(kv.split("=", 1) for kv in line[3:].split())
            live, is_sync, phase = int(fields["live"]), fields["map"] == "sync", None
        elif line.startswith("PHASE "):
            phase = json.loads(line[6:])
        elif line.startswith("SYNC ") and is_sync:
            if phase is None:
                raise ValueError("a SYNC line with no PHASE line before it")
            out.setdefault(live, []).append({**phase, **json.loads(line[5:])})
            phase = None
    if not out:
        raise ValueError(f"no concurrent exclusive section in {path}")
    return out


def reader_wait_bound_ns(section: dict) -> int:
    """The reader wait left if only the index rewrite and the table publish hold
    the tree bracket (#1300 item 2): phase 2 plus `republish_table`. The
    collect and the copy write nothing a reader validates against."""
    for k in ("phase2_ns", "publish_ns"):
        _nonneg_int(k, section[k])
    return section["phase2_ns"] + section["publish_ns"]


def reader_wait_ratio_bound(section: dict) -> float:
    """`reader_wait_bound_ns` over the section's measured bracket time."""
    _pos_int("inside_ns", section["inside_ns"])
    return reader_wait_bound_ns(section) / section["inside_ns"]


def compaction_peak_bytes(total_allocated: int, live_records: int, payload_len: int,
                          chunk: int, index_entries: int) -> int:
    """Bytes a compaction holds at its peak, beside the index.

    The old chunk set (`total_allocated`) and the new one (the live records
    repacked) coexist until the new table is published, and the relocation
    list holds one 16-byte `(Key, ValueSlot)` per index entry, reserved once
    (`compact_with_index`, amended by §28a from two growing vectors to one
    reserved vector). On `SyncExpanseBlobMap` the old set then stays allocated
    until the epoch collector frees it, which pinned readers delay.
    """
    for name, v in (("total_allocated", total_allocated), ("live_records", live_records),
                    ("index_entries", index_entries)):
        _nonneg_int(name, v)
    if index_entries < live_records:
        raise ValueError("every arena record is an index entry")
    return total_allocated + compacted_chunks(live_records, payload_len, chunk) * chunk + 16 * index_entries


class _ArenaModel:
    """Record-level model of one arena under the rule, for one payload size.

    Mirrors `BlobArena::alloc_blob` (cursor rounding, chunk and cap checks) and
    `compact_with_index` (live records repacked densely from a fresh arena).
    The rule's state is the chunk count and live bytes right after the last
    compaction, zero for a new arena.
    """

    def __init__(self, payload_len: int, chunk: int, cap: int, k: int) -> None:
        _pos_int("k", k)
        self.needed, self.stride = record_needed(payload_len), record_stride(payload_len)
        if self.needed > chunk:
            raise ValueError("a record larger than a chunk is refused (AllocationFailed)")
        self.chunk, self.max_chunks, self.k = chunk, max_chunks(chunk, cap), k
        self.chunks = self.cursor = self.live_bytes = 0
        self.floor_chunks = self.floor_live = 0
        self.compactions = self.copied = 0

    def alloc(self) -> bool:
        if self.chunks and self.cursor + self.needed <= self.chunk:
            self.cursor = -(-(self.cursor + self.needed) // ARENA_ALIGN) * ARENA_ALIGN
            return True
        if self.chunks + 1 > self.max_chunks:
            return False
        self.chunks += 1
        self.cursor = self.stride
        return True

    def insert(self) -> bool:
        """Place one record: allocate, or compact once and retry once. Live bytes are the caller's."""
        if self.alloc():
            return True
        freed = (self.chunks - self.floor_chunks) * self.chunk + max(0, self.floor_live - self.live_bytes)
        if self.live_bytes > self.k * freed:
            return False
        if 2 * self.live_bytes >= self.chunks * self.chunk:
            return False  # (B), the waste guard
        self.compactions += 1
        self.copied += self.live_bytes
        self.chunks = self.cursor = 0
        for _ in range(self.live_bytes // self.needed):
            assert self.alloc(), "live records that fit before a compaction fit after it"
        self.floor_chunks, self.floor_live = self.chunks, self.live_bytes
        return self.alloc()


def simulate(live_records: int, payload_len: int, chunk: int, cap: int, appends: int,
             k: int = RECLAIM_COPY_PER_GROWTH) -> dict[str, int]:
    """An overwrite-forever workload under the rule, record by record.

    Independent of the closed forms. The live set is inserted first, then
    `appends` overwrites follow. An overwrite charges the old record dead only
    after the new one is placed (`insert`, then `record_deleted_slot`), so the
    rule sees the live set without the new record, and live bytes stay fixed.
    """
    for name, v in (("live_records", live_records), ("appends", appends)):
        _nonneg_int(name, v)
    a = _ArenaModel(payload_len, chunk, cap, k)
    for _ in range(live_records):
        if not a.alloc():
            raise ValueError("the live set alone overflows the arena")
        a.live_bytes += a.needed
    appended = errors = 0
    for _ in range(appends):
        if a.insert():
            appended += a.needed
        else:
            errors += 1
    return {"compactions": a.compactions, "copied": a.copied, "appended": appended, "errors": errors}


def simulate_fill_then_remove(payload_len: int, chunk: int, cap: int, keep_share: float,
                              k: int = RECLAIM_COPY_PER_GROWTH) -> dict[str, int]:
    """Distinct keys until an insert fails, one more insert, then remove all but
    `keep_share` of the keys and insert once more. Record-level, as `simulate`.
    """
    if not 0.0 <= keep_share <= 1.0:
        raise ValueError("keep_share must lie in [0, 1]")
    a = _ArenaModel(payload_len, chunk, cap, k)
    while a.insert():
        a.live_bytes += a.needed
    filled = a.live_bytes // a.needed
    compactions_at_fill = a.compactions
    # Nothing changed since the refusal, so the rule refuses again.
    retry_refused = not a.insert()
    a.live_bytes = int(filled * keep_share) * a.needed
    ok = a.insert()
    return {"filled": filled, "compactions_at_fill": compactions_at_fill,
            "retry_refused": int(retry_refused), "compactions": a.compactions,
            "insert_after_removals": int(ok)}


def _read_const(pattern: str) -> int:
    m = re.search(pattern, BLOBMAP_RS.read_text(), re.MULTILINE)
    if not m:
        raise AssertionError(f"pattern not found in {BLOBMAP_RS}: {pattern}")
    expr = m.group(1).replace("_", "")
    if not re.fullmatch(r"[0-9 *<()]+", expr):
        raise AssertionError(f"unexpected constant expression {expr!r}")
    return int(eval(expr, {"__builtins__": {}}))  # digits, *, << and parentheses only


def self_test() -> int:
    # Engine constants this module mirrors.
    assert _read_const(r"^pub const DEFAULT_CHUNK_SIZE: usize = (.+);") == DEFAULT_CHUNK_SIZE
    assert _read_const(r"^pub const DEFAULT_ARENA_CAPACITY: usize = (.+);") == DEFAULT_ARENA_CAPACITY
    assert _read_const(r"^pub const MAX_ARENA_CHUNKS: usize = (.+);") == MAX_ARENA_CHUNKS
    assert _read_const(r"^pub const ARENA_ALIGN: usize = (.+);") == ARENA_ALIGN
    src = BLOBMAP_RS.read_text()
    assert "self.cursor = (next_cursor + 15) & !15;" in src, "ArenaChunk::alloc's cursor rounding moved"
    assert "self.total_allocated.saturating_add(self.chunk_size) > self.max_capacity" in src
    # The rule itself, as the engine states it (`BlobArena::reclaim_allowed`).
    assert _read_const(r"^pub\(crate\) const RECLAIM_COPY_PER_GROWTH: usize = (.+);") == RECLAIM_COPY_PER_GROWTH
    for line in ("let grown = self.total_allocated.saturating_sub(self.compacted_total);",
                 "let dropped = self.compacted_live.saturating_sub(self.live_bytes);",
                 "self.live_bytes <= RECLAIM_COPY_PER_GROWTH.saturating_mul(grown.saturating_add(dropped))",
                 "&& self.live_bytes.saturating_mul(2) < self.total_allocated"):
        assert line in src, f"BlobArena::reclaim_allowed no longer reads: {line}"

    # Record geometry: 128 B payload -> 136 charged, 144 stride (as blob_mixed_bounds.py).
    assert record_needed(128) == 136 and record_stride(128) == 144
    assert record_stride(9) == 32 and record_stride(8) == 16
    # (2 MiB - 136) // 144 + 1 = 14,562 + 1.
    assert records_per_chunk(128, DEFAULT_CHUNK_SIZE) == 14_563
    assert max_chunks(DEFAULT_CHUNK_SIZE, DEFAULT_ARENA_CAPACITY) == 512
    assert max_chunks(4096, DEFAULT_ARENA_CAPACITY) == MAX_ARENA_CHUNKS  # the chunk count binds
    assert compacted_chunks(HARNESS_LIVE, 128, DEFAULT_CHUNK_SIZE) == 14  # ceil(200,000 / 14,563)

    # The #1280 harness's live set is sustained at the default cap with a wide margin.
    assert sustains_overwrite(HARNESS_LIVE, HARNESS_LEN, DEFAULT_CHUNK_SIZE, DEFAULT_ARENA_CAPACITY)
    m = max_sustained_live_records(HARNESS_LEN, DEFAULT_CHUNK_SIZE, DEFAULT_ARENA_CAPACITY)
    # Pinned: the largest L with L * 136 <= (512 - ceil(L / 14,563)) * 2 MiB.
    # L = 3,830,069 fills 263 chunks exactly: 520,889,384 <= 249 * 2 MiB =
    # 522,190,848. One more record opens a 264th: 520,889,520 > 520,093,696.
    # The waste guard allows up to 2 * L * 136 < 2^30, L <= 3,947,580, so (A)
    # binds here and the amendment leaves this figure unchanged.
    assert m == 3_830_069, m
    assert sustains_overwrite(m, 128, DEFAULT_CHUNK_SIZE, DEFAULT_ARENA_CAPACITY)
    assert not sustains_overwrite(m + 1, 128, DEFAULT_CHUNK_SIZE, DEFAULT_ARENA_CAPACITY)
    assert 0.51 < live_share_of_cap(m, 128, DEFAULT_CHUNK_SIZE, DEFAULT_ARENA_CAPACITY) < 0.52
    # A larger k admits more live data (monotone in k).
    assert max_sustained_live_records(128, DEFAULT_CHUNK_SIZE, DEFAULT_ARENA_CAPACITY, k=2) > m

    # Cost at the harness's live set: about 0.027 bytes copied per byte appended.
    c = copy_per_append(HARNESS_LIVE, 128, DEFAULT_CHUNK_SIZE, DEFAULT_ARENA_CAPACITY)
    assert abs(c - 200_000 / (512 * 14_563 - 200_000)) < 1e-12 and c < 0.03
    # At the sustained maximum the copy per appended byte stays near k (here k = 1).
    assert copy_per_append(m, 128, DEFAULT_CHUNK_SIZE, DEFAULT_ARENA_CAPACITY) < 1.1
    assert stall_copy_bytes(HARNESS_LIVE, 128) == 27_200_000
    # (B) bounds every automatic copy below half the cap: 512 MiB - 1 byte at
    # the default cap, whatever the live set or the payload size.
    assert max_stall_bytes(DEFAULT_ARENA_CAPACITY) == (1 << 29) - 1
    for plen in (8, 9, 128, 1_048_577):
        mm = max_sustained_live_records(plen, DEFAULT_CHUNK_SIZE, DEFAULT_ARENA_CAPACITY)
        assert stall_copy_bytes(mm, plen) <= max_stall_bytes(DEFAULT_ARENA_CAPACITY), plen
    # Transient peak at the harness's live set and at the sustained maximum:
    # old arena at the cap + repacked live set + 16 B per index entry.
    assert compaction_peak_bytes(DEFAULT_ARENA_CAPACITY, HARNESS_LIVE, 128, DEFAULT_CHUNK_SIZE,
                                 HARNESS_LIVE) == (1 << 30) + 14 * (2 << 20) + 3_200_000
    assert compaction_peak_bytes(DEFAULT_ARENA_CAPACITY, m, 128, DEFAULT_CHUNK_SIZE, m) == \
        (1 << 30) + 263 * (2 << 20) + 16 * 3_830_069

    # The closed forms against the record-level model on small arenas, over
    # payload sizes that exercise alignment waste (9 B), dense packing (8 B),
    # the harness shape (128 B) and a record just over half a chunk.
    chunk, cap = 4096, 64 * 1024
    for plen in (8, 9, 128, 2100):
        per = records_per_chunk(plen, chunk)
        full = max_chunks(chunk, cap) * per
        for live in sorted({1, full // 4, full // 2, max_sustained_live_records(plen, chunk, cap),
                            max_sustained_live_records(plen, chunk, cap) + 1, full - 1}):
            if live <= 0 or live >= full:
                continue
            sim = simulate(live, plen, chunk, cap, appends=4 * full)
            sustained = sustains_overwrite(live, plen, chunk, cap)
            assert (sim["errors"] == 0) == sustained, (plen, live, sim, sustained)
            # The rule's invariant, for every workload: copied <= k * freed, and
            # an overwrite workload can free at most the whole cap between compactions.
            assert sim["copied"] <= RECLAIM_COPY_PER_GROWTH * cap * max(sim["compactions"], 1)
            if sustained:
                assert sim["compactions"] >= 3, (plen, live, sim)
            else:
                # A refused compaction is not retried until the arena grows again:
                # at most one compaction, the first (its growth is the whole arena).
                assert sim["compactions"] <= 1, (plen, live, sim)
            # (B): no automatic copy reaches half the cap.
            assert sim["copied"] <= max(sim["compactions"], 1) * max_stall_bytes(cap), (plen, live, sim)

    # An arena filled with distinct live keys fails at the first refusal with
    # no compaction: its live bytes are more than half its chunk bytes, so the
    # waste guard (B) refuses a copy that would free nothing (§28a; before it,
    # the rule compacted once here). After half the keys are removed the guard
    # passes, and the budget (A) counts the whole arena grown since creation,
    # so one compaction admits the insert.
    for plen in (9, 128, 2100):
        r = simulate_fill_then_remove(plen, 4096, 64 * 1024, keep_share=0.5)
        assert r == {"filled": max_chunks(4096, 64 * 1024) * records_per_chunk(plen, 4096),
                     "compactions_at_fill": 0, "retry_refused": 1, "compactions": 1,
                     "insert_after_removals": 1}, (plen, r)
        # Keeping nearly everything: 128 B records leave the arena more than
        # half live, so nothing is copied. For 9 B records (15 bytes of padding
        # each) and 2,100 B records (one per chunk) the charged live bytes fall
        # under half the chunk bytes, and the compaction does free chunks
        # (1,843 records repack into 15; 14 records into 14): it admits the insert.
        r = simulate_fill_then_remove(plen, 4096, 64 * 1024, keep_share=0.9)
        want = (0, 0) if plen == 128 else (1, 1)
        assert (r["compactions"], r["insert_after_removals"]) == want, (plen, r)
    # No compaction the guard admits in these sequences is futile: every one
    # is followed by a successful insert.
    for plen in (8, 9, 128, 2100):
        for keep in (0.0, 0.25, 0.5, 0.75, 0.9, 1.0):
            r = simulate_fill_then_remove(plen, 4096, 64 * 1024, keep_share=keep)
            assert r["compactions_at_fill"] == 0, (plen, keep, r)
            assert r["compactions"] <= r["insert_after_removals"], (plen, keep, r)

    # §29's predictions: the per-record compaction cost the §27 runs measured,
    # carried linearly to §29's live sets. 12 cells (two arms, three thread
    # counts, two runs) between 53.07 and 58.80 ns per live record.
    c = s27_ns_per_record()
    assert len(c) == 12, len(c)
    assert 53.07 <= min(c) < 53.08 and 58.79 < max(c) <= 58.80, (min(c), max(c))
    lo, hi = min(c), max(c)
    assert 10.61e6 < predicted_stall_ns(200_000, lo) < 10.62e6
    assert 11.75e6 < predicted_stall_ns(200_000, hi) < 11.76e6
    assert 191.0e6 < predicted_stall_ns(3_600_000, lo) < 191.1e6
    assert 211.6e6 < predicted_stall_ns(3_600_000, hi) < 211.7e6
    # Both live sets are sustained and pass the waste guard at the default geometry.
    for live in STALL_LIVE_SETS:
        assert sustains_overwrite(live, HARNESS_LEN, DEFAULT_CHUNK_SIZE, MAX_ARENA_CAPACITY), live
        assert 2 * live * record_needed(HARNESS_LEN) < MAX_ARENA_CAPACITY, live

    # §30's bound (README §29.1): with the bracket held only for the index
    # rewrite and the publish, a reader's wait is 9.93-10.45 % of today's at
    # 200,000 live and 9.00-9.14 % at 3,600,000, six sections each.
    secs = step0_sync_sections()
    assert sorted(secs) == [200_000, 3_600_000] and all(len(v) == 6 for v in secs.values())
    q = {live: [reader_wait_ratio_bound(x) for x in v] for live, v in secs.items()}
    assert 0.0993 < min(q[200_000]) and max(q[200_000]) < 0.1046, q[200_000]
    assert 0.0899 < min(q[3_600_000]) and max(q[3_600_000]) < 0.0914, q[3_600_000]
    assert 1_481_000 < min(map(reader_wait_bound_ns, secs[200_000])) < 1_482_000
    assert 26_464_000 < max(map(reader_wait_bound_ns, secs[3_600_000])) < 26_465_000

    # Invalid inputs fail loudly.
    for bad in (lambda: records_per_chunk(5000, 4096), lambda: record_needed(-1),
                lambda: max_chunks(0, 10), lambda: sustains_overwrite(1, 8, 4096, 65536, k=0),
                lambda: copy_per_append(512 * 14_563, 128, DEFAULT_CHUNK_SIZE, DEFAULT_ARENA_CAPACITY),
                lambda: reader_wait_ratio_bound({"phase2_ns": 1, "publish_ns": 0, "inside_ns": 0}),
                lambda: reader_wait_bound_ns({"phase2_ns": -1, "publish_ns": 0})):
        try:
            bad()
        except ValueError:
            pass
        else:
            raise AssertionError("invalid input accepted")
    print("blob_reclaim_bounds.py self-test PASSED")
    return 0


def report() -> None:
    chunk, cap = DEFAULT_CHUNK_SIZE, DEFAULT_ARENA_CAPACITY
    print(f"rule: compact on refused growth iff live_bytes <= {RECLAIM_COPY_PER_GROWTH} x (chunk growth + live drop) "
          "since the previous compaction, and 2 x live_bytes < allocated chunk bytes")
    print(f"{'payload B':>10} {'rec/chunk':>10} {'max live recs':>14} {'share of cap':>13} "
          f"{'copy/append at max':>19} {'stall MiB at max':>17}")
    for plen in (8, 9, 64, 128, 1024, 65536, 1_048_577):
        m = max_sustained_live_records(plen, chunk, cap)
        print(f"{plen:>10} {records_per_chunk(plen, chunk):>10} {m:>14,} "
              f"{live_share_of_cap(m, plen, chunk, cap):>13.3f} "
              f"{copy_per_append(m, plen, chunk, cap):>19.3f} "
              f"{stall_copy_bytes(m, plen) / 2**20:>17.1f}")
    print(f"#1280 harness ({HARNESS_LIVE:,} x {HARNESS_LEN} B): sustained="
          f"{sustains_overwrite(HARNESS_LIVE, HARNESS_LEN, chunk, cap)}, copy/append="
          f"{copy_per_append(HARNESS_LIVE, HARNESS_LEN, chunk, cap):.4f}, stall="
          f"{stall_copy_bytes(HARNESS_LIVE, HARNESS_LEN) / 2**20:.1f} MiB")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if not args.self_test:
        report()
    return self_test()


if __name__ == "__main__":
    sys.exit(main())
