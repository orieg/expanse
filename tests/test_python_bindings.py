"""Comprehensive pytest validation suite for Expanse Python bindings (`expanse-trie`).

Tests:
1. ExpanseSet: bitset/integer set mutations, ordered navigation, rank/select, range scans.
2. ExpanseMap: word-to-word map mutations, dict-like operators, ordered navigation, range scans.
3. ExpanseStrMap: variable-length string/bytes trie map, lexicographical scans.
4. ExpanseBytesMap: arbitrary bytes hash map.
5. SyncExpanseSet & SyncExpanseMap: multithreaded GIL-free concurrent reads and writes.
6. SyncExpanseStrMap, SyncExpanseBytesMap, SyncExpanseOrderedBytesMap, SyncExpanseBlobMap:
   the shared maps of the C `expanse_sync_*` handles (#1538).
"""

import threading
import time
import pytest

from expanse_trie import (
    ExpanseSet,
    ExpanseMap,
    ExpanseStrMap,
    ExpanseBytesMap,
    ExpanseBlobMap,
    ExpanseOrderedBytesMap,
    SyncExpanseSet,
    SyncExpanseMap,
    SyncExpanseStrMap,
    SyncExpanseBytesMap,
    SyncExpanseOrderedBytesMap,
    SyncExpanseBlobMap,
    __version__,
)


def test_version():
    # The module's __version__ must equal the installed distribution's version
    # (from pyproject.toml, kept in lockstep by scripts/bump_version.py). Never
    # a literal: a hardcoded "0.4.0" here broke the v0.4.1 release bump.
    from importlib.metadata import version as dist_version

    assert __version__ == dist_version("expanse-trie")


# ============================================================================
# 1. ExpanseSet Tests
# ============================================================================

def test_expanse_set_basic_mutation():
    s = ExpanseSet()
    assert s.is_empty()
    assert len(s) == 0
    assert s.mem_used() == 0

    # Insert elements
    assert s.insert(10) is True
    assert s.insert(20) is True
    assert s.insert(10) is False  # Already present
    assert len(s) == 2
    assert not s.is_empty()
    assert 10 in s
    assert 20 in s
    assert 30 not in s

    # Add method alias
    s.add(30)
    assert 30 in s
    assert len(s) == 3

    # Remove
    assert s.remove(20) is True
    assert s.remove(20) is False
    assert 20 not in s
    assert len(s) == 2

    # Discard alias
    assert s.discard(10) is True
    assert s.discard(10) is False
    assert len(s) == 1

    # Clear
    s.clear()
    assert len(s) == 0
    assert s.is_empty()


def test_expanse_set_initialization():
    initial = [42, 1, 999999, 100, 5]
    s = ExpanseSet(initial)
    assert len(s) == len(initial)
    for x in initial:
        assert x in s

    # Iteration order is strictly sorted ascending
    assert list(s) == sorted(initial)
    assert s.to_list() == sorted(initial)


def test_expanse_set_navigation_and_rank_select():
    keys = [10, 20, 30, 40, 50, 60, 70, 80, 90, 100]
    s = ExpanseSet(keys)

    # First & Last
    assert s.first() == 10
    assert s.last() == 100

    # Next
    assert s.next_at_or_after(25) == 30
    assert s.next_at_or_after(30) == 30
    assert s.next_after(30) == 40
    assert s.next_after(100) is None

    # Prev
    assert s.prev_at_or_before(35) == 30
    assert s.prev_at_or_before(30) == 30
    assert s.prev_before(30) == 20
    assert s.prev_before(10) is None

    # Rank (count_below) & Range count
    assert s.count_below(10) == 0
    assert s.count_below(35) == 3
    assert s.count_below(100) == 9
    assert s.count_below(101) == 10
    assert s.count_range(20, 50) == 4  # 20, 30, 40, 50
    assert s.count_range(25, 55) == 3  # 30, 40, 50

    # Select (by_count - 0-based index)
    assert s.by_count(0) == 10
    assert s.by_count(4) == 50
    assert s.by_count(9) == 100
    assert s.by_count(10) is None

    # Range query
    assert s.range(25, 75) == [30, 40, 50, 60, 70]
    assert s.range(None, 30) == [10, 20, 30]
    assert s.range(80, None) == [80, 90, 100]

def test_expanse_set_boundary_keys():
    s = ExpanseSet()
    MAX_U64 = (1 << 64) - 1
    MAX_I64 = (1 << 63) - 1
    keys = [0, 1, (1 << 53) - 1, 1 << 53, MAX_I64, MAX_U64]
    for k in keys:
        s.insert(k)
    for k in keys:
        assert k in s
    assert len(s) == len(keys)


def test_expanse_set_batch():
    keys = [10, 20, 30, 40, 50, 60, 70, 80]
    s = ExpanseSet(keys)
    res = s.contains_batch([10, 25, 30, 99, 80])
    assert res == [True, False, True, False, True]


# ============================================================================
# 2. ExpanseMap Tests
# ============================================================================

def test_expanse_map_basic_mutation():
    m = ExpanseMap()
    assert m.is_empty()
    assert len(m) == 0

    # Insert
    assert m.insert(1, 100) is None
    assert m.insert(2, 200) is None
    assert m.insert(1, 150) == 100  # Overwrite returns old value
    assert len(m) == 2

    # Get & getitem
    assert m.get(1) == 150
    assert m.get(2) == 200
    assert m.get(3) is None
    assert m.get(3, 999) == 999
    assert m[1] == 150
    assert m[2] == 200
    with pytest.raises(KeyError):
        _ = m[3]

    # Setitem
    m[3] = 300
    assert m[3] == 300
    assert len(m) == 3

    # Contains
    assert 1 in m
    assert 2 in m
    assert 3 in m
    assert 4 not in m
    assert m.contains_key(1) is True

    # Delitem & Remove & Pop
    del m[1]
    assert 1 not in m
    assert len(m) == 2
    with pytest.raises(KeyError):
        del m[1]

    assert m.remove(2) == 200
    assert m.remove(2) is None
    assert len(m) == 1

    assert m.pop(3) == 300
    assert len(m) == 0
    assert m.is_empty()
    assert m.pop(3, default=42) == 42
    with pytest.raises(KeyError):
        m.pop(3)


def test_expanse_map_initialization():
    d = {10: 100, 20: 200, 5: 50, 1000: 10000}
    m = ExpanseMap(d)
    assert len(m) == 4
    for k, v in d.items():
        assert m[k] == v

    # Iteration yields keys in ascending order
    assert list(m) == sorted(d.keys())
    assert m.keys() == sorted(d.keys())
    assert m.values() == [d[k] for k in sorted(d.keys())]
    assert m.items() == [(k, d[k]) for k in sorted(d.keys())]


def test_expanse_map_navigation_and_range():
    items = [(10, 100), (20, 200), (30, 300), (40, 400), (50, 500)]
    m = ExpanseMap(items)

    assert m.first() == (10, 100)
    assert m.last() == (50, 500)
    assert m.next_at_or_after(25) == (30, 300)
    assert m.next_after(30) == (40, 400)
    assert m.prev_at_or_before(35) == (30, 300)
    assert m.prev_before(30) == (20, 200)

    assert m.count_below(30) == 2
    assert m.count_range(20, 40) == 3
    assert m.by_count(1) == (20, 200)

    assert m.range(20, 40) == [(20, 200), (30, 300), (40, 400)]

def test_expanse_map_boundary_keys():
    m = ExpanseMap()
    MAX_U64 = (1 << 64) - 1
    MAX_I64 = (1 << 63) - 1
    keys = [0, 1, (1 << 53) - 1, 1 << 53, MAX_I64, MAX_U64]
    for k in keys:
        val = (k ^ 0x5555555555555555) & 0xFFFFFFFFFFFFFFFF
        m[k] = val
    for k in keys:
        expected = (k ^ 0x5555555555555555) & 0xFFFFFFFFFFFFFFFF
        assert m[k] == expected
    assert len(m) == len(keys)


def test_expanse_map_batch():
    m = ExpanseMap({10: 100, 20: 200, 30: 300, 40: 400})
    res = m.get_batch([10, 25, 30, 99])
    assert res == [100, None, 300, None]


# ============================================================================
# 3. ExpanseStrMap Tests
# ============================================================================

def test_expanse_strmap():
    sm = ExpanseStrMap()
    assert sm.is_empty()
    assert len(sm) == 0

    sm.insert("apple", 1)
    sm.insert("banana", 2)
    sm.insert("cherry", 3)
    sm["date"] = 4
    sm[b"elderberry"] = 5

    assert len(sm) == 5
    assert sm.get("banana") == 2
    assert sm["cherry"] == 3
    assert sm[b"apple"] == 1
    assert sm["date"] == 4
    assert sm["elderberry"] == 5
    assert "fig" not in sm
    assert sm.get("fig", 404) == 404

    # Navigation in lexicographical order
    assert sm.first() == ("apple", 1)
    assert sm.last() == ("elderberry", 5)
    assert sm.next_after("banana") == ("cherry", 3)
    assert sm.prev_before("cherry") == ("banana", 2)

    # Range query
    r = sm.range(start="banana", end="date")
    assert r == [("banana", 2), ("cherry", 3), ("date", 4)]

    # Deletion
    del sm["cherry"]
    assert "cherry" not in sm
    assert len(sm) == 4
    assert sm.remove("banana") == 2
    assert len(sm) == 3


# ============================================================================
# 4. ExpanseBytesMap Tests
# ============================================================================

def test_expanse_bytesmap():
    bm = ExpanseBytesMap()
    assert bm.is_empty()

    # Supports arbitrary byte sequences including embedded NUL bytes
    key1 = b"user\x00data\x001"
    key2 = b"user\x00data\x002"
    key3 = "regular_string_key"

    bm.insert(key1, 100)
    bm[key2] = 200
    bm[key3] = 300

    assert len(bm) == 3
    assert bm[key1] == 100
    assert bm[key2] == 200
    assert bm[key3] == 300
    assert key1 in bm
    assert b"unknown" not in bm

    del bm[key1]
    assert key1 not in bm
    assert len(bm) == 2


# ============================================================================
# 5. Multithreaded GIL-Free Concurrency Tests (SyncExpanseSet & SyncExpanseMap)
# ============================================================================

def test_sync_expanse_set_concurrent_readers():
    """Verify optimistic concurrent reads across multiple threads."""
    s = SyncExpanseSet()
    num_items = 10_000
    for i in range(num_items):
        s.insert(i)

    assert len(s) == num_items

    num_threads = 8
    reads_per_thread = 5_000
    errors = []

    def reader_worker(thread_id):
        try:
            for _ in range(reads_per_thread):
                key = (thread_id * 1337) % num_items
                if key not in s:
                    errors.append(f"Key {key} missing in thread {thread_id}")
                if s.contains(num_items + 1):
                    errors.append("Found non-existent key")
        except Exception as e:
            errors.append(f"Exception in reader: {e}")

    threads = [threading.Thread(target=reader_worker, args=(t,)) for t in range(num_threads)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()

    assert len(errors) == 0


def test_sync_expanse_map_concurrent_read_write_scaling():
    """Verify concurrent reads and writes with GIL release."""
    m = SyncExpanseMap()
    num_initial = 5_000
    for i in range(num_initial):
        m.insert(i, i * 10)

    num_reader_threads = 4
    num_writer_threads = 2
    duration_seconds = 0.5
    stop_event = threading.Event()
    read_counts = [0] * num_reader_threads
    write_counts = [0] * num_writer_threads
    errors = []

    def reader_loop(idx):
        cnt = 0
        while not stop_event.is_set():
            val = m.get(100)
            if val is not None and val != 1000:
                errors.append(f"Inconsistent value {val}")
            _ = len(m)
            cnt += 1
        read_counts[idx] = cnt

    def writer_loop(idx):
        cnt = 0
        start_key = 100_000 + idx * 50_000
        while not stop_event.is_set():
            k = start_key + (cnt % 1000)
            m.insert(k, k * 2)
            cnt += 1
        write_counts[idx] = cnt

    readers = [threading.Thread(target=reader_loop, args=(i,)) for i in range(num_reader_threads)]
    writers = [threading.Thread(target=writer_loop, args=(i,)) for i in range(num_writer_threads)]

    for t in readers + writers:
        t.start()

    time.sleep(duration_seconds)
    stop_event.set()

    for t in readers + writers:
        t.join()

    assert len(errors) == 0
    total_reads = sum(read_counts)
    total_writes = sum(write_counts)
    assert total_reads > 0
    assert total_writes > 0
    print(f"\nCompleted {total_reads:,} concurrent reads and {total_writes:,} writes in {duration_seconds}s")


# ============================================================================
# 6. Correctness fixes (PR: fix(bindings))
# ============================================================================

def test_sync_range_inclusive_matches_nonsync():
    """range() inclusive default is True across sync and non-sync (was False for sync)."""
    keys = [10, 20, 30, 40, 50]

    m = ExpanseMap()
    sm = SyncExpanseMap()
    for k in keys:
        m[k] = k * 10
        sm.insert(k, k * 10)

    # Default (inclusive) must include the endpoint 40 for BOTH.
    assert [k for k, _ in m.range(20, 40)] == [20, 30, 40]
    assert [k for k, _ in sm.range(20, 40)] == [20, 30, 40]
    # Explicit half-open agrees too.
    assert [k for k, _ in sm.range(20, 40, inclusive=False)] == [20, 30]

    s = ExpanseSet()
    ss = SyncExpanseSet()
    for k in keys:
        s.add(k)
        ss.add(k)
    assert list(s.range(20, 40)) == [20, 30, 40]
    assert list(ss.range(20, 40)) == [20, 30, 40]
    assert list(ss.range(20, 40, inclusive=False)) == [20, 30]


def test_sync_map_ordered_reads_match_nonsync():
    """SyncExpanseMap.first/last/next/prev answer like ExpanseMap on a quiescent map,
    including both ends of the key space, on the optimistic reader path (#900)."""
    top = 2**64 - 1
    m = ExpanseMap()
    sm = SyncExpanseMap()
    assert sm.first() is None and sm.last() is None
    assert sm.next(0, inclusive=True) is None and sm.prev(top, inclusive=True) is None

    for k in [0, 10, 20, 30, top]:
        m[k] = k ^ 0x5A5A
        sm.insert(k, k ^ 0x5A5A)
    assert sm.first() == m.first() == (0, 0x5A5A)
    assert sm.last() == m.last() == (top, top ^ 0x5A5A)
    for probe in [0, 5, 10, 15, 20, 25, 30, 31, top - 1, top]:
        assert sm.next(probe) == m.next_after(probe), probe
        assert sm.next(probe, inclusive=True) == m.next_at_or_after(probe), probe
        assert sm.prev(probe) == m.prev_before(probe), probe
        assert sm.prev(probe, inclusive=True) == m.prev_at_or_before(probe), probe
    assert sm.next(top) is None
    assert sm.prev(0) is None


def test_sync_map_ordered_reads_under_concurrent_writer():
    """Ordered reads return the right neighbour while a writer churns the keys between
    stable ones: even keys never change, odd keys come and go with a fixed value."""
    n = 2048
    sm = SyncExpanseMap()
    for k in range(0, n, 2):
        sm.insert(k, k * 3)
    stop = threading.Event()
    errors = []

    def writer():
        rnd = 0
        while not stop.is_set():
            for k in range(1, n, 2):
                if (k + rnd) % 3 == 0:
                    sm.remove(k)
                else:
                    sm.insert(k, k * 3)
            rnd += 1

    def reader(seed):
        for i in range(4000):
            even = ((i * 7919 + seed) % n) & ~1
            if sm.next(even, inclusive=True) != (even, even * 3):
                errors.append(("next inclusive", even))
            if sm.prev(even, inclusive=True) != (even, even * 3):
                errors.append(("prev inclusive", even))
            if even + 2 < n and sm.next(even) not in ((even + 1, (even + 1) * 3), (even + 2, (even + 2) * 3)):
                errors.append(("next", even))
            if even >= 2 and sm.prev(even) not in ((even - 1, (even - 1) * 3), (even - 2, (even - 2) * 3)):
                errors.append(("prev", even))
            if sm.first() != (0, 0):
                errors.append(("first", 0))

    w = threading.Thread(target=writer)
    readers = [threading.Thread(target=reader, args=(s,)) for s in range(4)]
    w.start()
    for t in readers:
        t.start()
    for t in readers:
        t.join()
    stop.set()
    w.join()
    assert errors == []


def test_sync_map_mem_used():
    """SyncExpanseMap.mem_used() is an int that grows with the population and
    matches ExpanseMap's accounting for the same entries inserted in the same order."""
    sm = SyncExpanseMap()
    m = ExpanseMap()
    empty = sm.mem_used()
    assert isinstance(empty, int) and empty >= 0
    for k in range(0, 20000, 3):
        sm.insert(k, k)
        m[k] = k
    assert sm.mem_used() > empty
    assert sm.mem_used() == m.mem_used()


def test_mem_held_and_shrink_to_fit():
    """A map, set or string map drained by remove keeps its freed blocks;
    shrink_to_fit() returns exactly mem_held() - mem_used(), after which the
    two agree."""
    n = 20000
    m = ExpanseMap()
    s = ExpanseSet()
    sm = ExpanseStrMap()
    for k in range(n):
        m[k * 7] = k
        s.insert(k * 7)
        sm[f"key/{k:08d}"] = k
    for c in (m, s, sm):
        assert c.mem_held() >= c.mem_used()
    for k in range(n):
        assert m.remove(k * 7) == k
        assert s.remove(k * 7) is True
        assert sm.remove(f"key/{k:08d}") == k
    for c, retains in ((m, True), (s, True), (sm, True)):
        held, used = c.mem_held(), c.mem_used()
        if retains:
            assert used == 0 and held > 0
        assert c.shrink_to_fit() == held - used
        assert c.mem_held() == c.mem_used() == used
        assert c.shrink_to_fit() == 0


def test_sync_mem_held_and_shrink_to_fit():
    """After a half drain a concurrent map holds more than it uses: its epoch
    collector keeps the freed blocks. shrink_to_fit() returns the ones past
    their grace period, and mem_held() falls by exactly what it reports;
    blocks still in their grace period stay held, so mem_held() is not
    asserted to reach mem_used()."""
    n = 50000
    sm = SyncExpanseMap()
    ss = SyncExpanseSet()
    for k in range(n):
        sm.insert(k * 0x9E3779B1, k)
        ss.insert(k * 0x9E3779B1)
    for k in range(0, n, 2):
        assert sm.remove(k * 0x9E3779B1) == k
        assert ss.remove(k * 0x9E3779B1) is True
    assert sm.mem_held() > sm.mem_used()
    for c in (sm, ss):
        held = c.mem_held()
        assert isinstance(held, int) and held > 0
        released = c.shrink_to_fit()
        assert isinstance(released, int) and released >= 0
        assert c.mem_held() == held - released
    for k in range(1, n, 2):
        assert sm.get(k * 0x9E3779B1) == k
        assert (k * 0x9E3779B1) in ss


def test_sync_remove_returns_value_not_keyerror():
    """SyncExpanseMap.remove -> Optional[int]; SyncExpanseSet.remove -> bool (mirror non-sync)."""
    sm = SyncExpanseMap()
    sm.insert(1, 100)
    assert sm.remove(1) == 100
    # Missing key returns None, does NOT raise (del/pop keep the KeyError semantics).
    assert sm.remove(999) is None
    with pytest.raises(KeyError):
        del sm[7]

    ss = SyncExpanseSet()
    ss.add(5)
    assert ss.remove(5) is True
    assert ss.remove(5) is False  # already gone -> False, no KeyError


def test_insert_many_rejects_bad_byte_buffers():
    """insert_many raises on non-multiple-of-8 buffers and length mismatches (no silent truncation)."""
    import struct

    s = ExpanseSet()
    good = struct.pack("<3Q", 1, 2, 3)  # 24 bytes = three u64
    assert s.insert_many(good) == 3
    assert sorted(s) == [1, 2, 3]

    # bytes whose length is not a multiple of 8 must raise, not fall to per-byte.
    with pytest.raises(ValueError):
        ExpanseSet().insert_many(b"\x01\x02\x03")

    # bytearray is treated as a packed u64 buffer, not iterated per byte.
    s2 = ExpanseSet()
    assert s2.insert_many(bytearray(struct.pack("<2Q", 7, 8))) == 2
    assert sorted(s2) == [7, 8]

    m = ExpanseMap()
    # Mismatched key/value buffer lengths must raise.
    with pytest.raises(ValueError):
        m.insert_many(struct.pack("<2Q", 1, 2), struct.pack("<1Q", 10))
    # Mismatched iterable lengths must raise too.
    with pytest.raises(ValueError):
        ExpanseMap().insert_many([1, 2, 3], [10, 20])


def test_insert_many_native_endian_roundtrip():
    """Packed byte keys are read in native endianness and round-trip through get()."""
    import struct
    import sys

    m = ExpanseMap()
    order = "<" if sys.byteorder == "little" else ">"
    keys = [1, 1 << 40, (1 << 64) - 1]
    vals = [100, 200, 300]
    m.insert_many(
        struct.pack(f"{order}{len(keys)}Q", *keys),
        struct.pack(f"{order}{len(vals)}Q", *vals),
    )
    for k, v in zip(keys, vals):
        assert m[k] == v


def test_strmap_non_utf8_keys_roundtrip_as_bytes():
    """Non-UTF-8 keys come back as bytes (not mangled by a lossy decode); UTF-8 keys as str."""
    m = ExpanseStrMap()
    utf8_key = "café"
    bad_key = b"\xff\xfe\xfd"  # 0xff/0xfe/0xfd are never valid UTF-8 lead bytes
    m[utf8_key] = 1
    m[bad_key] = 2

    # Readback via first/last/keys/items must preserve the exact bytes.
    keys = m.keys()
    assert utf8_key in keys
    assert bad_key in keys
    # The non-UTF-8 key is returned as bytes, and equals the original exactly.
    got = [k for k in keys if isinstance(k, bytes)]
    assert got == [bad_key]
    # str keys stay str.
    assert any(isinstance(k, str) and k == utf8_key for k in keys)

    # Navigation returns the same typed keys.
    for _, _ in m.items():
        pass
    first = m.first()
    last = m.last()
    assert {type(first[0]), type(last[0])} <= {str, bytes}


def test_blobmap_is_exported_and_usable():
    """ExpanseBlobMap is importable from the package (was missing from __init__)."""
    import expanse_trie

    assert "ExpanseBlobMap" in expanse_trie.__all__
    assert expanse_trie.ExpanseBlobMap is ExpanseBlobMap

    bm = ExpanseBlobMap()
    bm.insert(1, b"hello")
    bm.insert(2, b"a much larger arena-backed payload well over seven bytes")
    assert bm.get_bytes(1) == b"hello"
    assert bm[2].startswith(b"a much larger")
    assert bm.remove(1) is True
    assert len(bm) == 1


def test_blobmap_cap_switch_and_refusal_status():
    """#1300: the capacity cap, the reclaim switch, the arena stats, and a
    refusal with no compaction ("cap_refused") told from one after the
    insert's own compaction ("arena_full"). One 4 KiB chunk fits a 6000-byte cap."""
    default = ExpanseBlobMap().arena_stats()
    assert default["max_capacity"] == 1 << 30
    assert default["reclaim_at_cap"] is True

    bm = ExpanseBlobMap(4096, 6000)
    assert bm.arena_stats()["max_capacity"] == 6000
    for k in range(4):
        assert bm.insert_status(k, bytes([k]) * 1000, 1) == "ok"
    assert bm.insert_status(9, b"\x09" * 3100, 1) == "cap_refused"
    st = bm.arena_stats()
    assert (st["live_bytes"], st["allocated_bytes"]) == (4 * 1008, 4096)
    for k in range(1, 4):
        assert bm.remove(k)
    assert bm.insert_status(9, b"\x09" * 3100, 1) == "arena_full"
    assert len(bm) == 1
    assert bm.insert_status(1, b"x" * 100, 1 << 24) == "meta_overflow"

    off = ExpanseBlobMap(4096, 6000)
    off.set_reclaim_at_cap(False)
    assert off.reclaim_at_cap() is False
    assert off.arena_stats()["reclaim_at_cap"] is False


def test_blobmap_scan_filtered_reraises_callback_error():
    """A predicate/callback exception aborts the scan and is re-raised (not swallowed)."""
    bm = ExpanseBlobMap()
    for i in range(10):
        bm.insert(i, bytes([i]) * 4, hot_meta=i)

    # Callback that raises must propagate the error out of scan_filtered.
    seen = []

    def bad_callback(key, payload, meta):
        seen.append(key)
        if key == 3:
            raise RuntimeError("boom in callback")
        return True

    with pytest.raises(RuntimeError, match="boom in callback"):
        bm.scan_filtered(0, 9, None, bad_callback)
    assert 3 in seen  # it did reach the offending key before aborting

    # Predicate that raises must also propagate (was silently treated as "keep").
    def bad_predicate(key, meta):
        if key == 2:
            raise ValueError("boom in predicate")
        return True

    with pytest.raises(ValueError, match="boom in predicate"):
        bm.scan_filtered(0, 9, bad_predicate, None)


def test_sync_map_get_many_matches_per_call_get():
    """`get_many` must return exactly what repeated `get` returns, in order.

    It exists to amortise the GIL handoff across a batch (#554), which is only
    a valid optimisation if it computes the same thing. Mixed hits and misses,
    duplicates, and an empty batch are all included because a batched path is
    where an off-by-one or a silently dropped tail hides.
    """
    m = SyncExpanseMap()
    for k in range(0, 2000, 2):  # even keys only, so odds are real misses
        m.insert(k, k * 7)

    probes = [0, 1, 998, 999, 1000, 1999, 2000, 5000, 998]
    assert m.get_many(probes) == [m.get(k) for k in probes]

    # Every key present.
    hits = list(range(0, 200, 2))
    assert m.get_many(hits) == [k * 7 for k in hits]

    # Every key absent.
    assert m.get_many([1, 3, 5, 7]) == [None, None, None, None]

    # Empty batch is not an error.
    assert m.get_many([]) == []

    # A batch larger than the internal chunking still returns one result per
    # key, in order — the tail is where a chunked loop drops entries.
    big = list(range(0, 9000))
    out = m.get_many(big)
    assert len(out) == len(big)
    assert out == [m.get(k) for k in big]


def test_sync_map_compare_exchange_branches():
    """Every compare_exchange branch: absent/present expected, absent/present new, match and mismatch."""
    m = SyncExpanseMap()
    # absent == absent, nothing to store: succeeds, observes absent.
    assert m.compare_exchange(1, None, None) == (True, None)
    assert 1 not in m
    # insert-if-absent succeeds on an absent key.
    assert m.compare_exchange(1, None, 10) == (True, None)
    assert m.get(1) == 10
    # insert-if-absent fails on a present key and reports the word seen.
    assert m.compare_exchange(1, None, 99) == (False, 10)
    assert m.get(1) == 10
    # absent == absent against a present key fails.
    assert m.compare_exchange(1, None, None) == (False, 10)
    # expected present, wrong value: fails, value unchanged.
    assert m.compare_exchange(1, 11, 12) == (False, 10)
    assert m.get(1) == 10
    # expected present against an absent key: fails, observes absent.
    assert m.compare_exchange(2, 5, 6) == (False, None)
    assert 2 not in m
    # replace on match.
    assert m.compare_exchange(1, 10, 20) == (True, 10)
    assert m.get(1) == 20
    # remove-if-equals: mismatch leaves it, match removes it.
    assert m.compare_exchange(1, 21, None) == (False, 20)
    assert m.get(1) == 20
    assert m.compare_exchange(1, 20, None) == (True, 20)
    assert 1 not in m
    # Every u64 is a legal value, including 0 and the maximum.
    top = 2**64 - 1
    assert m.compare_exchange(3, None, 0) == (True, None)
    assert m.compare_exchange(3, 0, top) == (True, 0)
    assert m.get(3) == top
    with pytest.raises(OverflowError):
        m.compare_exchange(3, None, 2**64)


def test_sync_map_get_or_insert():
    m = SyncExpanseMap()
    assert m.get_or_insert(7, 70) is None  # stored
    assert m.get(7) == 70
    assert m.get_or_insert(7, 71) == 70  # present: existing value, untouched
    assert m.get(7) == 70
    assert m.get_or_insert(8, 0) is None  # zero is a value, not absence
    assert m.get_or_insert(8, 5) == 0


def test_sync_map_update_with_store_and_remove():
    m = SyncExpanseMap()
    seen = []

    def inc(cur):
        seen.append(cur)
        return (cur or 0) + 1

    assert m.update_with(5, inc) is None  # absent: replaced value is None
    assert m.update_with(5, inc) == 1
    assert m.get(5) == 2
    assert seen == [None, 1]
    # Returning None removes the key and reports the replaced value.
    assert m.update_with(5, lambda cur: None) == 2
    assert 5 not in m
    # Removing an absent key is a no-op that reports absent.
    assert m.update_with(5, lambda cur: None) is None
    assert 5 not in m
    # An exception from fn propagates and leaves the key unchanged.
    m.insert(6, 60)

    def boom(cur):
        raise RuntimeError("fn failed")

    with pytest.raises(RuntimeError):
        m.update_with(6, boom)
    assert m.get(6) == 60
    # A non-integer result is a TypeError, not a store.
    with pytest.raises(TypeError):
        m.update_with(6, lambda cur: "x")
    assert m.get(6) == 60


def test_sync_map_update_with_retries_on_interference():
    """fn runs again with the observed value when a concurrent writer wins."""
    m = SyncExpanseMap()
    m.insert(1, 100)
    calls = []

    def fn(cur):
        calls.append(cur)
        if len(calls) == 1:
            m.insert(1, 200)  # a competing writer lands between read and exchange
        return cur + 1

    assert m.update_with(1, fn) == 200
    assert calls == [100, 200]
    assert m.get(1) == 201


def test_sync_map_update_with_concurrent_counter_is_exact():
    m = SyncExpanseMap()
    threads, per_thread = 8, 2000

    def worker():
        for _ in range(per_thread):
            m.update_with(0, lambda cur: (cur or 0) + 1)

    ts = [threading.Thread(target=worker) for _ in range(threads)]
    for t in ts:
        t.start()
    for t in ts:
        t.join()
    assert m.get(0) == threads * per_thread

    # get_or_insert: exactly one of the racing callers stores.
    winners = []
    barrier = threading.Barrier(threads)

    def racer(i):
        barrier.wait()
        if m.get_or_insert(42, i) is None:
            winners.append(i)

    ts = [threading.Thread(target=racer, args=(i,)) for i in range(threads)]
    for t in ts:
        t.start()
    for t in ts:
        t.join()
    assert len(winners) == 1
    assert m.get(42) == winners[0]


# ============================================================================
# 6. ExpanseOrderedBytesMap Tests (C ABI expanse_ordered_bytesmap_*)
# ============================================================================


def test_ordered_bytesmap_roundtrip_and_mutation():
    om = ExpanseOrderedBytesMap()
    assert om.is_empty()
    assert len(om) == 0
    assert not om
    assert om.get(b"absent") is None
    assert om.get(b"absent", 7) == 7

    assert om.insert(b"alpha", 1) is None  # new key: no previous value
    assert om.insert(b"alpha", 2) == 1  # replace returns previous
    assert om[b"alpha"] == 2
    assert b"alpha" in om
    assert om.contains("alpha")  # str keys are encoded as UTF-8
    assert om.get("alpha") == 2  # str and bytes keys address the same entry
    om["beta"] = 5
    assert om[b"beta"] == 5
    assert len(om) == 2
    assert bool(om)

    with pytest.raises(KeyError):
        _ = om[b"missing"]
    assert om.remove(b"beta") == 5
    assert om.remove(b"beta") is None
    with pytest.raises(KeyError):
        del om[b"beta"]
    del om[b"alpha"]
    assert len(om) == 0
    assert repr(om) == "ExpanseOrderedBytesMap(len=0)"


def test_ordered_bytesmap_order_and_navigation():
    om = ExpanseOrderedBytesMap()
    keys = [b"b", b"a", b"ab", b"", b"\x00", b"\xff", b"abc", b"\x01"]
    for i, k in enumerate(keys):
        om.insert(k, i)

    # Ascending unsigned-byte lexicographic order; a key sorts before its extensions.
    expected = sorted(keys)
    assert list(om.keys()) == expected
    assert [k for k in om] == expected
    assert [v for _, v in om.items()] == [keys.index(k) for k in expected]
    assert list(om.values()) == [keys.index(k) for k in expected]

    assert om.first() == (b"", keys.index(b""))
    assert om.last() == (b"\xff", keys.index(b"\xff"))

    # Exact-hit and between-key navigation.
    assert om.next_at_or_after(b"a") == (b"a", keys.index(b"a"))
    assert om.next_after(b"a") == (b"ab", keys.index(b"ab"))
    assert om.next_at_or_after(b"aa") == (b"ab", keys.index(b"ab"))
    assert om.prev_at_or_before(b"aa") == (b"a", keys.index(b"a"))
    assert om.prev_at_or_before(b"ab") == (b"ab", keys.index(b"ab"))
    assert om.prev_before(b"ab") == (b"a", keys.index(b"a"))

    # Edges: nothing before the smallest key or after the largest.
    assert om.prev_before(b"") is None
    assert om.prev_at_or_before(b"") == (b"", keys.index(b""))
    assert om.next_after(b"\xff") is None
    assert om.next_at_or_after(b"\xff\xff") is None


def test_ordered_bytesmap_empty_key():
    om = ExpanseOrderedBytesMap()
    om.insert(b"", 42)
    om.insert(b"\x00", 1)
    assert om[b""] == 42
    assert b"" in om
    assert om.first() == (b"", 42)
    assert om.next_after(b"") == (b"\x00", 1)
    assert om.remove(b"") == 42
    assert b"" not in om
    assert om.first() == (b"\x00", 1)


def test_ordered_bytesmap_nul_and_ff_bytes_roundtrip():
    om = ExpanseOrderedBytesMap()
    raw = [b"\x00", b"\x00\x00", b"\x01", b"\x01\x00\x01", b"\xff", b"\xff\x00\xff", b"a\x00b"]
    for i, k in enumerate(raw):
        om.insert(k, 100 + i)
    for i, k in enumerate(raw):
        assert om[k] == 100 + i
    # Every key comes back byte-for-byte, with its embedded NUL/0x01/0xFF intact.
    assert list(om.keys()) == sorted(raw)
    assert all(type(k) is bytes for k in om.keys())
    assert om.last() == (b"\xff\x00\xff", 100 + raw.index(b"\xff\x00\xff"))


def test_ordered_bytesmap_long_key_beyond_navigation_buffer():
    # Longer than 8 bytes and longer than a 256-byte caller buffer would hold: navigation must
    # return the whole key, never a truncated prefix.
    long_key = bytes(range(256)) * 16  # 4096 bytes, ends in 0xFF, includes 0x00
    # Shares the first 4095 bytes; its last byte is 0x00, so it sorts before long_key.
    other = long_key[:-1] + b"\x00"
    assert other < long_key
    om = ExpanseOrderedBytesMap()
    om.insert(long_key, 9)
    om.insert(other, 8)
    assert om[long_key] == 9
    assert len(om.first()[0]) == len(other)
    assert om.first() == (other, 8)
    assert om.last() == (long_key, 9)
    assert om.next_at_or_after(long_key[:-1]) == (other, 8)
    assert om.next_after(other) == (long_key, 9)
    assert om.prev_before(long_key) == (other, 8)
    assert om.prev_at_or_before(long_key) == (long_key, 9)


def test_ordered_bytesmap_absent_key_and_empty_map():
    om = ExpanseOrderedBytesMap()
    assert om.first() is None
    assert om.last() is None
    assert om.next_at_or_after(b"x") is None
    assert om.prev_at_or_before(b"x") is None
    assert om.get(b"x") is None
    assert list(om.keys()) == []

    om.insert(b"m", 1)
    # An absent key navigates to its neighbours without matching.
    assert om.get(b"l") is None
    assert b"l" not in om
    assert om.next_at_or_after(b"l") == (b"m", 1)
    assert om.prev_at_or_before(b"n") == (b"m", 1)
    assert om.next_after(b"m") is None


def test_ordered_bytesmap_iteration_after_mutation():
    om = ExpanseOrderedBytesMap()
    for i, k in enumerate([b"a", b"c", b"e"]):
        om.insert(k, i)
    assert list(om.keys()) == [b"a", b"c", b"e"]

    # Mutations made between navigation calls are observed by the following calls.
    om.insert(b"b", 10)
    assert om.next_after(b"a") == (b"b", 10)
    om.remove(b"b")
    assert om.next_after(b"a") == (b"c", 1)

    # A fresh iterator reflects the current contents; an old one is a snapshot.
    snapshot = om.keys()
    om.insert(b"d", 3)
    assert list(om.keys()) == [b"a", b"c", b"d", b"e"]
    assert list(snapshot) == [b"a", b"c", b"e"]
    assert len(list(om.items())) == 4


def test_ordered_bytesmap_slot_and_ins_slot():
    om = ExpanseOrderedBytesMap()
    assert om.slot(b"k") is None  # absent: no slot value
    assert om.ins_slot(b"k") == 0  # absent: inserted with value 0
    assert b"k" in om
    om[b"k"] = 77
    assert om.ins_slot(b"k") == 77  # present: existing value is kept
    assert om.slot(b"k") == 77
    assert om[b"k"] == 77


def test_ordered_bytesmap_memory_accounting_and_clear():
    om = ExpanseOrderedBytesMap()
    assert om.mem_used() == 0
    for i in range(500):
        om.insert(i.to_bytes(4, "big"), i)
    assert len(om) == 500
    assert om.mem_used() > 0
    assert om.mem_held() >= om.mem_used()
    assert isinstance(om.shrink_to_fit(), int)
    om.clear()
    assert len(om) == 0
    assert om.first() is None
    assert om.mem_used() == 0


# ============================================================================
# 6. Shared string, bytes, ordered-bytes and blob maps (#1538)
# ============================================================================

# kind -> (class, key maker); the string map takes str keys, the others bytes.
WORD_MAPS = {
    "str": (SyncExpanseStrMap, lambda i: f"key-{i:06}"),
    "bytes": (SyncExpanseBytesMap, lambda i: f"key-{i:06}".encode()),
    "ordered": (SyncExpanseOrderedBytesMap, lambda i: f"key-{i:06}".encode()),
}


@pytest.fixture(params=sorted(WORD_MAPS))
def word_map(request):
    cls, mk = WORD_MAPS[request.param]
    return cls(), mk


def test_shared_word_map_point_ops(word_map):
    m, k = word_map
    assert len(m) == 0 and m.is_empty() and not m and m.empty
    assert m.get(k(1)) is None
    assert m.get(k(1), 7) == 7
    assert m.insert(k(1), 10) is None
    assert m.insert(k(1), 11) == 10  # previous value
    m[k(2)] = 20
    assert len(m) == 2 and m
    assert m[k(1)] == 11 and m.get(k(2)) == 20
    assert k(1) in m and m.contains(k(2)) and m.contains_key(k(1))
    assert k(3) not in m and not m.contains(k(3))
    assert m.remove(k(1)) == 11
    assert m.remove(k(1)) is None
    with pytest.raises(KeyError):
        m[k(1)]
    with pytest.raises(KeyError):
        del m[k(1)]
    del m[k(2)]
    assert len(m) == 0
    assert m.pop(k(9), 5) == 5
    with pytest.raises(KeyError):
        m.pop(k(9))
    m.insert(k(9), 90)
    assert m.pop(k(9)) == 90
    # u64 range is exact on both ends.
    m.insert(k(4), 2**64 - 1)
    assert m.get(k(4)) == 2**64 - 1
    with pytest.raises(OverflowError):
        m.insert(k(4), 2**64)


def test_shared_word_map_memory_accounting_and_clear(word_map):
    m, k = word_map
    for i in range(500):
        m.insert(k(i), i)
    assert len(m) == 500
    assert m.mem_used() > 0
    assert m.mem_held() >= m.mem_used()
    assert isinstance(m.shrink_to_fit(), int)
    m.clear()
    assert len(m) == 0 and m.get(k(1)) is None


def test_shared_word_map_compare_exchange_branches(word_map):
    m, k = word_map
    key = k(1)
    assert m.compare_exchange(key, None, None) == (True, None)  # absent check
    assert m.compare_exchange(key, None, 10) == (True, None)  # insert-if-absent
    assert m.compare_exchange(key, None, 99) == (False, 10)
    assert m.compare_exchange(key, None, None) == (False, 10)  # present check
    assert m.compare_exchange(key, 11, 12) == (False, 10)
    assert m.compare_exchange(k(2), 5, 6) == (False, None)
    assert m.compare_exchange(key, 10, 20) == (True, 10)
    assert m.compare_exchange(key, 21, None) == (False, 20)
    assert m.compare_exchange(key, 20, None) == (True, 20)  # remove-if-equals
    assert key not in m
    assert m.compare_exchange(key, None, 0) == (True, None)  # zero is a value
    assert m.compare_exchange(key, 0, 2**64 - 1) == (True, 0)
    with pytest.raises(OverflowError):
        m.compare_exchange(key, None, 2**64)


def test_shared_word_map_get_or_insert_and_update_with(word_map):
    m, k = word_map
    assert m.get_or_insert(k(1), 5) is None  # stored
    assert m.get_or_insert(k(1), 6) == 5  # present: untouched
    assert m[k(1)] == 5

    def inc(cur):
        return (cur or 0) + 1

    assert m.update_with(k(2), inc) is None  # absent -> 1
    assert m.update_with(k(2), inc) == 1
    assert m[k(2)] == 2
    assert m.update_with(k(2), lambda cur: None) == 2  # None removes
    assert k(2) not in m
    with pytest.raises(ZeroDivisionError):
        m.update_with(k(3), lambda cur: 1 // 0)
    assert k(3) not in m  # an exception leaves the key unchanged
    with pytest.raises(TypeError):
        m.update_with(k(3), lambda cur: "x")

    calls = []

    def interfered(cur):
        calls.append(cur)
        if len(calls) == 1:
            m.insert(k(1), 200)  # a competing writer lands between read and exchange
        return cur + 1

    assert m.update_with(k(1), interfered) == 200
    assert calls == [5, 200]
    assert m[k(1)] == 201


def test_shared_word_map_concurrent_counter_and_single_winner(word_map):
    m, k = word_map
    threads, per_thread = 8, 1500

    def worker():
        for _ in range(per_thread):
            m.update_with(k(0), lambda cur: (cur or 0) + 1)

    ts = [threading.Thread(target=worker) for _ in range(threads)]
    for t in ts:
        t.start()
    for t in ts:
        t.join()
    assert m[k(0)] == threads * per_thread  # no lost update

    # compare_exchange retry loop, the hand-written form.
    def cas_worker():
        for _ in range(per_thread // 3):
            seen = m.get(k(1))
            while True:
                stored, seen = m.compare_exchange(k(1), seen, (seen or 0) + 1)
                if stored:
                    break

    ts = [threading.Thread(target=cas_worker) for _ in range(threads)]
    for t in ts:
        t.start()
    for t in ts:
        t.join()
    assert m[k(1)] == threads * (per_thread // 3)

    # get_or_insert: exactly one of the racing callers stores.
    winners = []
    barrier = threading.Barrier(threads)

    def racer(i):
        barrier.wait()
        if m.get_or_insert(k(42), i) is None:
            winners.append(i)

    ts = [threading.Thread(target=racer, args=(i,)) for i in range(threads)]
    for t in ts:
        t.start()
    for t in ts:
        t.join()
    assert len(winners) == 1
    assert m[k(42)] == winners[0]


def test_shared_word_map_concurrent_readers_see_consistent_values(word_map):
    m, k = word_map
    for i in range(200):
        m.insert(k(i), i)
    stop = threading.Event()
    errors = []

    def writer():
        n = 0
        while not stop.is_set():
            m.insert(k(1000 + n % 50), n)
            m.remove(k(1000 + (n + 25) % 50))
            n += 1

    def reader():
        for _ in range(300):
            for i in range(200):
                if m.get(k(i)) != i:  # the stable keys never change
                    errors.append(i)

    w = threading.Thread(target=writer)
    rs = [threading.Thread(target=reader) for _ in range(4)]
    w.start()
    for t in rs:
        t.start()
    for t in rs:
        t.join()
    stop.set()
    w.join()
    assert errors == []


def test_shared_strmap_keys_and_ordered_reads():
    m = SyncExpanseStrMap()
    with pytest.raises(ValueError):
        m.insert("a\x00b", 1)  # NUL is refused
    with pytest.raises(ValueError):
        m.get(b"a\x00b")
    with pytest.raises(TypeError):
        m.insert(5, 1)
    assert m.first() is None and m.last() is None
    for i, key in enumerate(["apple", "apricot", "banana", "cherry"]):
        m.insert(key, i)
    m.insert(b"\xff\xfe", 99)  # non-UTF-8 round-trips as bytes
    assert m.get(bytearray(b"apple")) == 0  # bytearray and bytes keys are the same keys
    assert m.first() == ("apple", 0)
    assert m.last() == (b"\xff\xfe", 99)
    assert m.next("apple") == ("apricot", 1)
    assert m.next("apple", inclusive=True) == ("apple", 0)
    assert m.next("apricot") == ("banana", 2)
    assert m.next("cherry") == (b"\xff\xfe", 99)
    assert m.next(b"\xff\xfe") is None
    assert m.prev("banana") == ("apricot", 1)
    assert m.prev("banana", inclusive=True) == ("banana", 2)
    assert m.prev("apple") is None
    assert m.next("b") == ("banana", 2)  # key need not be present
    assert m.prev("b") == ("apricot", 1)
    # An ordered walk equals the sorted keys.
    walked, cur = [], m.first()
    while cur is not None:
        walked.append(cur[0])
        cur = m.next(cur[0])
    assert walked == ["apple", "apricot", "banana", "cherry", b"\xff\xfe"]


def test_shared_strmap_ordered_walk_under_concurrent_writer():
    m = SyncExpanseStrMap()
    stable = [f"s{i:04}" for i in range(300)]
    for i, key in enumerate(stable):
        m.insert(key, i)
    stop = threading.Event()

    def writer():
        n = 0
        while not stop.is_set():
            m.insert(f"t{n % 40:04}", n)
            m.remove(f"t{(n + 20) % 40:04}")
            n += 1

    w = threading.Thread(target=writer)
    w.start()
    try:
        for _ in range(30):
            seen, cur = [], m.first()
            while cur is not None:
                if cur[0].startswith("s"):
                    seen.append(cur[0])
                cur = m.next(cur[0])
            assert seen == stable  # every stable key, in order, exactly once
    finally:
        stop.set()
        w.join()


def test_shared_bytesmap_allows_any_byte():
    m = SyncExpanseBytesMap()
    keys = [b"", b"\x00", b"a\x00b", b"\xff" * 64, "text".encode()]
    for i, key in enumerate(keys):
        assert m.insert(key, i) is None
    for i, key in enumerate(keys):
        assert m[key] == i
    assert m.get("text") == 4  # str keys are UTF-8 encoded
    assert len(m) == len(keys)
    assert m.compare_exchange(b"a\x00b", 2, 22) == (True, 2)
    assert m[b"a\x00b"] == 22
    assert not hasattr(m, "first")  # unordered: no ordered reads


def test_shared_ordered_bytesmap_ordered_reads():
    m = SyncExpanseOrderedBytesMap()
    assert m.first() is None and m.last() is None
    keys = [b"", b"\x00", b"a", b"a\x00", b"ab", b"b", b"\xff", b"\xff\xff"]
    for i, key in enumerate(keys):
        m.insert(key, i)
    assert sorted(keys) == keys  # fixture is in byte order
    assert m.first() == (b"", 0)
    assert m.last() == (b"\xff\xff", 7)
    for i, key in enumerate(keys):
        assert m.next_at_or_after(key) == (key, i)
        assert m.prev_at_or_before(key) == (key, i)
        nxt = (keys[i + 1], i + 1) if i + 1 < len(keys) else None
        prv = (keys[i - 1], i - 1) if i > 0 else None
        assert m.next_after(key) == nxt
        assert m.prev_before(key) == prv
    assert m.next_at_or_after(b"aa") == (b"ab", 4)
    assert m.prev_at_or_before(b"aa") == (b"a\x00", 3)
    assert m.next_after(b"\xff\xff") is None
    assert m.prev_before(b"") is None
    walked, cur = [], m.first()
    while cur is not None:
        walked.append(cur[0])
        cur = m.next_after(cur[0])
    assert walked == keys


def test_shared_ordered_bytesmap_ordered_walk_under_concurrent_writer():
    m = SyncExpanseOrderedBytesMap()
    stable = [b"s%04d" % i for i in range(300)]
    for i, key in enumerate(stable):
        m.insert(key, i)
    stop = threading.Event()

    def writer():
        n = 0
        while not stop.is_set():
            m.insert(b"t%04d" % (n % 40), n)
            m.remove(b"t%04d" % ((n + 20) % 40))
            n += 1

    w = threading.Thread(target=writer)
    w.start()
    try:
        for _ in range(30):
            seen, cur = [], m.first()
            while cur is not None:
                if cur[0].startswith(b"s"):
                    seen.append(cur[0])
                cur = m.next_after(cur[0])
            assert seen == stable
    finally:
        stop.set()
        w.join()


# --- SyncExpanseBlobMap ------------------------------------------------------


def _payload(n, seed=0):
    return bytes((seed + i) % 251 for i in range(n))


# 0 and 1..7 bytes (slot word), 8 and 100 (arena), 64 KiB and 1 MiB (arena-sized).
BLOB_SIZES = [0, 1, 7, 8, 100, 65536, 1 << 20]


def test_shared_blobmap_point_ops_across_sizes():
    m = SyncExpanseBlobMap()
    assert len(m) == 0 and m.is_empty() and not m and m.empty
    assert m.get(1) is None and m.get_bytes(1) is None and m.get_meta(1) is None
    for i, n in enumerate(BLOB_SIZES):
        m.insert(i, _payload(n, i), 0x123456 if n > 7 else 0)
    assert len(m) == len(BLOB_SIZES)
    for i, n in enumerate(BLOB_SIZES):
        data, meta = m.get(i)
        assert data == _payload(n, i) and isinstance(data, bytes)
        assert meta == (0x123456 if n > 7 else 0)
        assert m.get_bytes(i) == data and m[i] == data
        assert m.get_meta(i) == meta
        assert i in m and m.contains(i) and m.contains_key(i)
    assert 99 not in m
    with pytest.raises(KeyError):
        m[99]
    # Replacing a value returns the new one; a larger replacement moves to the arena.
    m.insert(0, _payload(5000), 7)
    assert m.get(0) == (_payload(5000), 7)
    m[1] = b"xy"
    assert m.get_bytes(1) == b"xy"
    assert m.remove(1) is True and m.remove(1) is False
    with pytest.raises(KeyError):
        del m[1]
    del m[0]
    assert len(m) == len(BLOB_SIZES) - 2
    assert m.mem_used() > 0 and m.mem_used_estimate() > 0
    assert m.mem_held() >= 0 and isinstance(m.shrink_to_fit(), int)
    m.clear()
    assert len(m) == 0 and m.get(2) is None


def test_shared_blobmap_small_payload_keeps_metadata():
    # Issue #1548: a payload of 7 bytes or fewer keeps the hot_meta it was
    # inserted with, and metadata wider than 24 bits is refused for it.
    m = SyncExpanseBlobMap()
    m.insert(1, b"tiny", 0xABC)
    m.insert(2, b"x" * 64, 0xABC)
    m.insert(3, b"", 7)
    assert m.get(1) == (b"tiny", 0xABC)
    assert m.get_meta(1) == 0xABC
    assert m.get(2) == (b"x" * 64, 0xABC)
    assert m.get(3) == (b"", 7)
    assert m.insert_status(4, b"tiny", 1 << 24) == "meta_overflow"
    assert 4 not in m


def test_shared_blobmap_refusals():
    m = SyncExpanseBlobMap()
    assert m.insert_status(1, b"x" * 64, 1 << 24) == "meta_overflow"
    with pytest.raises(RuntimeError):
        m.insert(1, b"x" * 64, 1 << 24)
    assert m.insert_status(1, b"x" * 64, (1 << 24) - 1) == "ok"
    assert len(m) == 1
    with pytest.raises(TypeError):
        m.insert(2, 5)
    # A capacity cap too small for the payloads refuses, and the map is unchanged.
    capped = SyncExpanseBlobMap(chunk_size=4096, max_capacity=8192)
    capped.set_reclaim_at_cap(False)
    assert capped.reclaim_at_cap() is False
    statuses = {capped.insert_status(i, b"z" * 3000) for i in range(16)}
    assert "ok" in statuses and statuses & {"cap_refused", "arena_full"}
    n = len(capped)
    assert capped.insert_status(1000, b"z" * 3000) in {"cap_refused", "arena_full"}
    assert len(capped) == n


def test_shared_blobmap_compact_reclaims_removed_payloads():
    m = SyncExpanseBlobMap(chunk_size=4096)
    for i in range(200):
        m.insert(i, _payload(1000, i), 1)
    for i in range(0, 200, 2):
        m.remove(i)
    before, after, alloc_before, alloc_after = m.compact()
    assert after <= before and alloc_after <= alloc_before
    for i in range(1, 200, 2):
        assert m.get(i) == (_payload(1000, i), 1)
    assert len(m) == 100


def test_shared_blobmap_compare_exchange_branches():
    m = SyncExpanseBlobMap()
    big = _payload(100)
    big2 = _payload(100, 9)
    assert m.compare_exchange(1, None, None) == ("ok", None)  # absent check
    assert m.compare_exchange(1, None, (big, 5)) == ("ok", None)  # insert-if-absent
    assert m.compare_exchange(1, None, (big2, 6)) == ("mismatch", (big, 5))
    assert m.compare_exchange(1, None, None) == ("mismatch", (big, 5))
    assert m.compare_exchange(1, (big, 6), (big2, 6)) == ("mismatch", (big, 5))  # wrong meta
    assert m.compare_exchange(1, (big2, 5), (big2, 6)) == ("mismatch", (big, 5))  # wrong data
    assert m.compare_exchange(2, (big, 5), (big2, 6)) == ("mismatch", None)  # absent key
    status, observed = m.compare_exchange(1, (big, 5), (big2, 6))
    assert status == "ok" and observed == (big, 5)
    assert m.get(1) == (big2, 6)
    # A refusal is reported only when the comparison held, and changes nothing.
    assert m.compare_exchange(1, (big2, 6), (big, 1 << 24))[0] == "meta_overflow"
    assert m.get(1) == (big2, 6)
    assert m.compare_exchange(1, (big, 1), (big, 1 << 24)) == ("mismatch", (big2, 6))
    # remove-if-equals
    assert m.compare_exchange(1, (big, 6), None) == ("mismatch", (big2, 6))
    assert m.compare_exchange(1, (big2, 6), None)[0] == "ok"
    assert 1 not in m
    # small payloads, and str payloads encode as UTF-8
    assert m.compare_exchange(3, None, (b"hi", 0)) == ("ok", None)
    assert m.compare_exchange(3, (b"hi", 0), ("ho", 0))[0] == "ok"
    assert m.get(3) == (b"ho", 0)
    # a short payload keeps the metadata it was inserted with, so expecting it matches (#1548)
    m.insert(4, b"tiny", 9)
    assert m.compare_exchange(4, (b"tiny", 0), None) == ("mismatch", (b"tiny", 9))
    assert m.compare_exchange(4, (b"tiny", 9), None)[0] == "ok"


def test_shared_blobmap_compare_exchange_meta():
    m = SyncExpanseBlobMap()
    big = _payload(100)
    assert m.compare_exchange_meta(1, None, None) == ("ok", None)
    # metadata alone cannot create a key
    assert m.compare_exchange_meta(1, None, 1)[0] == "no_meta_field"
    assert 1 not in m
    m.insert(1, big, 10)
    assert m.compare_exchange_meta(1, 11, 12) == ("mismatch", 10)
    assert m.compare_exchange_meta(2, 0, 1) == ("mismatch", None)
    assert m.compare_exchange_meta(1, 10, 11)[0] == "ok"
    assert m.get(1) == (big, 11)  # payload untouched
    assert m.compare_exchange_meta(1, 11, 1 << 24)[0] == "meta_overflow"
    assert m.get_meta(1) == 11
    # a slot-word payload has no metadata field
    m.insert(2, b"abc", 0)
    assert m.compare_exchange_meta(2, 0, 5)[0] == "no_meta_field"
    assert m.compare_exchange_meta(2, 0, 0)[0] == "ok"
    assert m.compare_exchange_meta(1, 11, None)[0] == "ok"  # remove
    assert 1 not in m


def test_shared_blobmap_get_or_insert_and_update_with():
    m = SyncExpanseBlobMap()
    big = _payload(100)
    assert m.get_or_insert(1, big, 3) == ("ok", None)
    assert m.get_or_insert(1, b"other", 4) == ("mismatch", (big, 3))
    assert m.get(1) == (big, 3)
    assert m.get_or_insert(2, big, 1 << 24)[0] == "meta_overflow"  # absent but unstorable
    assert 2 not in m

    def bump(cur):
        if cur is None:
            return (b"c" * 16, 1)
        data, meta = cur
        return (data, meta + 1)

    assert m.update_with(5, bump) is None
    assert m.update_with(5, bump) == (b"c" * 16, 1)
    assert m.get(5) == (b"c" * 16, 2)
    assert m.update_with(5, lambda cur: None) == (b"c" * 16, 2)
    assert 5 not in m
    with pytest.raises(ZeroDivisionError):
        m.update_with(6, lambda cur: 1 // 0)
    assert 6 not in m
    with pytest.raises(RuntimeError):
        m.update_with(6, lambda cur: (big, 1 << 24))  # refusal of the new value

    calls = []

    def interfered(cur):
        calls.append(cur)
        if len(calls) == 1:
            m.insert(1, big, 8)  # a competing writer lands between read and exchange
        return (b"n" * 20, 9)

    assert m.update_with(1, interfered) == (big, 8)
    assert calls == [(big, 3), (big, 8)]
    assert m.get(1) == (b"n" * 20, 9)


def test_shared_blobmap_concurrent_counter_and_single_winner():
    m = SyncExpanseBlobMap()
    threads, per_thread = 8, 1000

    def bump(cur):
        n = int.from_bytes(cur[0], "big") if cur else 0
        return ((n + 1).to_bytes(16, "big"), 1)  # a 16-byte (arena) payload

    def worker():
        for _ in range(per_thread):
            m.update_with(0, bump)

    ts = [threading.Thread(target=worker) for _ in range(threads)]
    for t in ts:
        t.start()
    for t in ts:
        t.join()
    assert int.from_bytes(m.get_bytes(0), "big") == threads * per_thread  # no lost update

    # compare_exchange_meta as a sequence counter, hand-written retry loop.
    m.insert(1, b"s" * 32, 0)

    def meta_worker():
        for _ in range(per_thread):
            seen = m.get_meta(1)
            while True:
                status, seen = m.compare_exchange_meta(1, seen, seen + 1)
                if status != "mismatch":
                    assert status == "ok"
                    break

    ts = [threading.Thread(target=meta_worker) for _ in range(threads)]
    for t in ts:
        t.start()
    for t in ts:
        t.join()
    assert m.get_meta(1) == threads * per_thread

    winners = []
    barrier = threading.Barrier(threads)

    def racer(i):
        barrier.wait()
        status, _ = m.get_or_insert(42, bytes([i]) * 24, i)
        if status == "ok":
            winners.append(i)

    ts = [threading.Thread(target=racer, args=(i,)) for i in range(threads)]
    for t in ts:
        t.start()
    for t in ts:
        t.join()
    assert len(winners) == 1
    assert m.get(42) == (bytes([winners[0]]) * 24, winners[0])


def test_shared_blobmap_concurrent_readers_never_see_torn_values():
    m = SyncExpanseBlobMap()
    for i in range(64):
        m.insert(i, _payload(200, i), i)
    stop = threading.Event()
    errors = []

    def writer():
        n = 0
        while not stop.is_set():
            k = 1000 + n % 32
            m.insert(k, _payload(300, n), 1)
            m.remove(1000 + (n + 16) % 32)
            n += 1

    def reader():
        for _ in range(200):
            for i in range(64):
                if m.get(i) != (_payload(200, i), i):
                    errors.append(i)
            for k in range(1000, 1032):
                v = m.get(k)
                if v is not None and (len(v[0]) != 300 or v[1] != 1):
                    errors.append(k)

    w = threading.Thread(target=writer)
    rs = [threading.Thread(target=reader) for _ in range(4)]
    w.start()
    for t in rs:
        t.start()
    for t in rs:
        t.join()
    stop.set()
    w.join()
    assert errors == []


def test_shared_maps_reader_cache_survives_map_lifecycle():
    # Each thread caches a reader per map; a dropped map's address may be reused by
    # a new one, which must not answer from the dead map's reader.
    import gc

    for _ in range(200):
        s = SyncExpanseStrMap()
        s.insert("a", 1)
        assert s.get("a") == 1
        b = SyncExpanseBlobMap()
        b.insert(1, b"v" * 32, 2)
        assert b.get(1) == (b"v" * 32, 2)
        del s, b
        gc.collect()
        fresh = SyncExpanseStrMap()
        assert fresh.get("a") is None
        fresh_blob = SyncExpanseBlobMap()
        assert fresh_blob.get(1) is None
