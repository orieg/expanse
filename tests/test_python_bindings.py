"""Comprehensive pytest validation suite for Expanse Python bindings (`expanse-trie`).

Tests:
1. ExpanseSet: bitset/integer set mutations, ordered navigation, rank/select, range scans.
2. ExpanseMap: word-to-word map mutations, dict-like operators, ordered navigation, range scans.
3. ExpanseStrMap: variable-length string/bytes trie map, lexicographical scans.
4. ExpanseBytesMap: arbitrary bytes hash map.
5. SyncExpanseSet & SyncExpanseMap: multithreaded GIL-free concurrent reads and writes.
"""

import concurrent.futures
import threading
import time
import pytest

from expanse_trie import (
    ExpanseSet,
    ExpanseMap,
    ExpanseStrMap,
    ExpanseBytesMap,
    ExpanseBlobMap,
    SyncExpanseSet,
    SyncExpanseMap,
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
    """Verify lock-free concurrent reads across multiple threads."""
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
