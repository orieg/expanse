#!/usr/bin/env python3
"""
scripts/dns_name_bounds.py — key encodings, exact string-map census and
competitor bounds for DNS-shaped keys (orieg/expanse #1800).

AGENTS.md §8.8 commit 1: the bounds the pre-registration will read are computed
here, in committed Python with pinned tests, not narrated. No harness and no
measurement lives in this file.

What is derived:

1. Key encodings of a domain name, and their order.
   RFC 4034 §6.1 (https://www.rfc-editor.org/rfc/rfc4034.txt) orders owner
   names by their labels, most significant (rightmost) first, each label an
   unsigned left-justified octet string with US-ASCII uppercase folded to
   lowercase, the absence of an octet sorting before a zero octet.
   `canonical_sort_key` is that rule; the three encodings below are byte
   strings whose plain lexicographic order equals it (pinned on the RFC's own
   nine-name example and on generated names with arbitrary label octets).
   - `key_nul`: reversed labels, each followed by a 0x00 terminator, label
     octets 0x00 and 0x01 escaped as (0x01 0x01) and (0x01 0x02). The form an
     application hands `ExpanseOrderedBytesMap`.
   - `key_ordered_bytes_stored`: `key_nul` after that map's own escape
     (`crates/expanse/src/ordered_bytesmap.rs:13-16`), which is what its inner
     string map stores. Every terminator becomes two bytes.
   - `key_direct`: a NUL-free form for `ExpanseStrMap` with a 0x01 terminator
     and label octets 0x00, 0x01, 0x02 escaped as (0x02 0x02), (0x02 0x03),
     (0x02 0x04). One byte per terminator.

2. An exact census of the `ExpanseStrMap` holding a key set
   (`crates/expanse/src/strmap.rs`, `ins_slot_impl` and `chunk_at`): a key is
   consumed in 8-byte chunks (`CHUNK_BYTES`); a key with fewer than 8 bytes
   left at a node is a terminal entry of that node's sub-map; a key that is
   alone on its next chunk is one suffix leaf holding the bytes after that
   chunk; two or more keys sharing the chunk share a child node. The census
   gives the node (shell) count, the suffix-leaf count and bytes, each
   sub-map's population, and the number of nodes a hit visits. The structure
   is a function of the key set, not of insertion order.
   Bytes: a shell is `SHELL_ACCOUNTED`; a suffix leaf is its 16-byte header
   plus its bytes, rounded to `RAW_ALIGN` (`alloc.rs::accounted_size`); a
   sub-map of at most `ROOT_LEAF_CAP` entries is one root leaf of parallel
   key and value words, 16 bytes per `cap_class` slot (`common.rs:16`).
   A sub-map above `ROOT_LEAF_CAP` is a `u64` trie whose bytes are NOT derived
   here: `mem_used_lower_bound` counts it as zero and the census reports how
   many such sub-maps there are and how many entries they hold. Those bytes
   are an empirical residual the harness measures. On a key set with no such
   sub-map the bound is exact.
   `crates/expanse/tests/test_dns_name_bounds.rs` builds the pinned key sets
   in the engine and asserts the same constants this file's self-test pins.

3. Competitor bounds.
   - `std::collections::BTreeMap<Vec<u8>, u64>`: node sizes from
     `library/alloc/src/collections/btree/node.rs` (`B = 6`, so 11 entries per
     node; read at rustc 1.98.1), giving a lower and an upper bound on bytes
     per key around the key bytes themselves.
   - A byte-wise compressed radix trie: nodes on a root-to-value path, from
     `scripts/patricia_envelope.py::census`, reused here as the path length a
     string-map descent is compared with.
   - `qp-trie`: nothing is derived. `patricia_envelope.branch_points` assumes
     no key is a prefix of another, and a zone breaks that at every name with
     a child (`com.example.` is a prefix of `com.example.www.`). Its path
     length and its bytes are left to the harness.

The name generator (`gen_names`) is SYNTHETIC: labels are uniform draws from
[a-z0-9], which share fewer prefixes than registered names do. It exists so the
bounds can be evaluated per population before a dataset is chosen, and so the
engine pin test has a reproducible key set. It is not the suite's dataset;
#1800 leaves that to the pre-registration.

  python3 scripts/dns_name_bounds.py --self-test   # run the pinned tests
  python3 scripts/dns_name_bounds.py --report      # per-population table
  python3 scripts/dns_name_bounds.py --pins        # print the pinned census values
"""

from __future__ import annotations

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import patricia_envelope  # noqa: E402  (sibling module: shared PRNG, radix path lengths)

# ---------------------------------------------------------------------------
# Engine constants (each pinned against the engine by test_dns_name_bounds.rs)
# ---------------------------------------------------------------------------

CHUNK_BYTES = 8  # strmap.rs `CHUNK_BYTES`
ROOT_LEAF_CAP = 31  # types.rs `ROOT_LEAF_CAP`
RAW_ALIGN = 16  # types.rs `RAW_ALIGN`
SUFFIX_HEADER = 16  # strmap.rs `StrSuffix { value: u64, len: usize }`
SHELL_ACCOUNTED = 48  # strmap.rs `SHELL_ACCOUNTED`: `size_of::<StrNode>()` rounded to RAW_ALIGN
ROOT_LEAF_SLOT = 16  # one u64 key word and one u64 value word per cap_class slot

MAX_LABEL = 63  # RFC 1035 §2.3.4
MAX_NAME_WIRE = 255  # RFC 1035 §2.3.4

# ---------------------------------------------------------------------------
# 1. Names, canonical order, encodings
# ---------------------------------------------------------------------------


def _lower(label: bytes) -> bytes:
    """US-ASCII uppercase folded to lowercase; every other octet unchanged
    (RFC 4034 §6.1). `bytes.lower` does exactly this."""
    return label.lower()


def check_name(labels: tuple[bytes, ...]) -> None:
    """Raises `ValueError` unless `labels` (wire order, root excluded) is a
    valid domain name under RFC 1035 §2.3.4."""
    if not labels:
        raise ValueError("the root name has no labels; not modelled")
    for label in labels:
        if not 1 <= len(label) <= MAX_LABEL:
            raise ValueError(f"label length must be in [1, {MAX_LABEL}], got {len(label)}")
    wire = sum(len(label) + 1 for label in labels) + 1
    if wire > MAX_NAME_WIRE:
        raise ValueError(f"name is {wire} octets on the wire, above {MAX_NAME_WIRE}")


def canonical_sort_key(labels: tuple[bytes, ...]) -> tuple[bytes, ...]:
    """RFC 4034 §6.1 order as a Python sort key: the lowercased labels, most
    significant first. Tuple comparison takes labels in turn and `bytes`
    comparison is unsigned with a proper prefix sorting first, which is the
    RFC's rule."""
    check_name(labels)
    return tuple(_lower(label) for label in reversed(labels))


def key_nul(labels: tuple[bytes, ...]) -> bytes:
    """Reversed-label key with 0x00 terminators; label octets 0x00 and 0x01
    escaped as (0x01 0x01) and (0x01 0x02)."""
    out = bytearray()
    for label in canonical_sort_key(labels):
        for b in label:
            if b == 0:
                out += b"\x01\x01"
            elif b == 1:
                out += b"\x01\x02"
            else:
                out.append(b)
        out.append(0)
    return bytes(out)


def ordered_bytes_escape(key: bytes) -> bytes:
    """`ExpanseOrderedBytesMap`'s escape (`ordered_bytesmap.rs:13-16`):
    0x00 -> (0x01 0x01), 0x01 -> (0x01 0x02), every other byte unchanged."""
    out = bytearray()
    for b in key:
        if b == 0:
            out += b"\x01\x01"
        elif b == 1:
            out += b"\x01\x02"
        else:
            out.append(b)
    return bytes(out)


def key_ordered_bytes_stored(labels: tuple[bytes, ...]) -> bytes:
    """What the string map inside `ExpanseOrderedBytesMap` stores for
    `key_nul(labels)`."""
    return ordered_bytes_escape(key_nul(labels))


def key_direct(labels: tuple[bytes, ...]) -> bytes:
    """NUL-free reversed-label key with 0x01 terminators; label octets 0x00,
    0x01, 0x02 escaped as (0x02 0x02), (0x02 0x03), (0x02 0x04)."""
    out = bytearray()
    for label in canonical_sort_key(labels):
        for b in label:
            if b <= 2:
                out += bytes((2, b + 2))
            else:
                out.append(b)
        out.append(1)
    return bytes(out)


def encoded_len(labels: tuple[bytes, ...], encoding: str) -> int:
    """Stored key length in bytes, in closed form (pinned equal to the length
    of the encoder's output). `low(t)` counts label octets at or below `t`."""
    check_name(labels)
    octets = sum(len(label) for label in labels)

    def low(t: int) -> int:
        return sum(1 for label in labels for b in label if b <= t)

    n = len(labels)
    if encoding == "nul":
        return octets + low(1) + n
    if encoding == "ordered_bytes_stored":
        # A 0x00 octet is (01 01) in `key_nul` and each 0x01 doubles again:
        # four bytes. A 0x01 octet is (01 02), then (01 02 02): three bytes.
        # Each terminator doubles.
        return octets + 3 * low(0) + 2 * (low(1) - low(0)) + 2 * n
    if encoding == "direct":
        return octets + low(2) + n
    raise ValueError(f"unknown encoding: {encoding}")


ENCODERS = {"nul": key_nul, "ordered_bytes_stored": key_ordered_bytes_stored, "direct": key_direct}

# ---------------------------------------------------------------------------
# 2. ExpanseStrMap census
# ---------------------------------------------------------------------------


def cap_class(pop: int) -> int:
    """`crates/expanse/src/common.rs::cap_class`."""
    if pop < 0:
        raise ValueError("population must be non-negative")
    if pop <= 2:
        return pop
    if pop <= 16:
        return (pop + 3) & ~3
    if pop <= 24:
        return 24
    if pop <= 32:
        return 32
    return (pop + 3) & ~3


def accounted(nbytes: int) -> int:
    """`alloc.rs::accounted_size(nbytes, RAW_ALIGN)`."""
    return (nbytes + RAW_ALIGN - 1) // RAW_ALIGN * RAW_ALIGN


def suffix_leaf_bytes(suffix_len: int) -> int:
    """Accounted bytes of one suffix leaf holding `suffix_len` key bytes."""
    if suffix_len < 0:
        raise ValueError("suffix length must be non-negative")
    return accounted(SUFFIX_HEADER + suffix_len)


def root_leaf_bytes(pop: int) -> int:
    """Bytes of a sub-map in root-leaf state."""
    if not 1 <= pop <= ROOT_LEAF_CAP:
        raise ValueError(f"a root leaf holds 1..={ROOT_LEAF_CAP} entries, got {pop}")
    return ROOT_LEAF_SLOT * cap_class(pop)


def strmap_census(keys: list[bytes]) -> dict:
    """Exact census of the `ExpanseStrMap` holding `keys` (duplicates ignored).

    `levels` is the number of string-map nodes a hit on a key visits. Each
    visit is one sub-map lookup; a suffix hit adds one byte comparison.
    """
    ks = sorted(set(keys))
    if not ks:
        raise ValueError("census needs at least one key")
    if any(0 in k for k in ks):
        raise ValueError("ExpanseStrMap keys are NUL-free")
    shells = suffixes = suffix_bytes = terminals = 0
    leaf_submap_bytes = over_submaps = over_entries = 0
    max_pop = level_sum = max_levels = 0
    stack = [(0, len(ks), 0, 1)]  # sorted slice [lo, hi) sharing ks[lo][:off]; node depth + 1
    while stack:
        lo, hi, off, level = stack.pop()
        shells += 1
        pop = 0
        i = lo
        while i < hi:
            k = ks[i]
            if len(k) - off < CHUNK_BYTES:  # chunk_at's terminal rule
                terminals += 1
                pop += 1
                level_sum += level
                max_levels = max(max_levels, level)
                i += 1
                continue
            chunk = k[off:off + CHUNK_BYTES]
            j = i + 1
            # Sorted keys sharing ks[lo][:off]: those with one next chunk are contiguous.
            while j < hi and ks[j][off:off + CHUNK_BYTES] == chunk and len(ks[j]) - off >= CHUNK_BYTES:
                j += 1
            pop += 1
            if j - i == 1:
                suffixes += 1
                suffix_bytes += suffix_leaf_bytes(len(k) - off - CHUNK_BYTES)
                level_sum += level
                max_levels = max(max_levels, level)
            else:
                stack.append((i, j, off + CHUNK_BYTES, level + 1))
            i = j
        max_pop = max(max_pop, pop)
        if pop <= ROOT_LEAF_CAP:
            leaf_submap_bytes += root_leaf_bytes(pop)
        else:
            over_submaps += 1
            over_entries += pop
    n = len(ks)
    return {
        "keys": n,
        "shells": shells,
        "suffixes": suffixes,
        "terminals": terminals,
        "shell_bytes": shells * SHELL_ACCOUNTED,
        "suffix_bytes": suffix_bytes,
        "leaf_submap_bytes": leaf_submap_bytes,
        "over_submaps": over_submaps,
        "over_entries": over_entries,
        "max_submap_pop": max_pop,
        "mean_levels": level_sum / n,
        "max_levels": max_levels,
    }


def mem_used_lower_bound(census: dict) -> int:
    """A lower bound on `ExpanseStrMap::mem_used()`: shells, suffix leaves and
    the sub-maps in root-leaf state. Exact when `census["over_submaps"] == 0`;
    otherwise the sub-maps above `ROOT_LEAF_CAP` are counted as zero bytes."""
    return census["shell_bytes"] + census["suffix_bytes"] + census["leaf_submap_bytes"]


# ---------------------------------------------------------------------------
# 3. Competitor bounds
# ---------------------------------------------------------------------------

BTREE_CAPACITY = 11  # node.rs: `CAPACITY = 2 * B - 1`, `B = 6`
BTREE_MIN_LEN = 5  # node.rs: `MIN_LEN_AFTER_SPLIT = B - 1`
VEC_HEADER = 24  # `Vec<u8>`: pointer, capacity, length
# `LeafNode<Vec<u8>, u64>`: parent pointer, parent_idx: u16, len: u16, 11 keys, 11 values,
# aligned to 8. `InternalNode` adds 12 child pointers.
BTREE_LEAF_NODE = (8 + 2 + 2 + BTREE_CAPACITY * VEC_HEADER + BTREE_CAPACITY * 8 + 7) // 8 * 8
BTREE_INTERNAL_NODE = BTREE_LEAF_NODE + (BTREE_CAPACITY + 1) * 8


def btreemap_bytes_bounds(n: int, key_bytes_total: int) -> tuple[int, int]:
    """Lower and upper bound on the requested heap bytes of a
    `BTreeMap<Vec<u8>, u64>` holding `n` keys of `key_bytes_total` bytes,
    each key a `Vec` of exact capacity.

    Lower: every node holds at most `BTREE_CAPACITY` entries and is at least a
    leaf node. Upper: every node but the root holds at least `BTREE_MIN_LEN`
    entries and is at most an internal node. Where a real tree sits between
    them depends on insertion order, which is why #1800 measures both orders.
    """
    if n < 1 or key_bytes_total < 0:
        raise ValueError("need n >= 1 and a non-negative key byte total")
    lo_nodes = -(-n // BTREE_CAPACITY)
    hi_nodes = 1 + n // BTREE_MIN_LEN
    return (lo_nodes * BTREE_LEAF_NODE + key_bytes_total,
            hi_nodes * BTREE_INTERNAL_NODE + key_bytes_total)


def radix_path_nodes(keys: list[bytes]) -> float:
    """Mean number of nodes on a root-to-value path of the byte-wise
    compressed radix trie over `keys` (`scripts/patricia_envelope.py::census`,
    which handles a key that is a prefix of another)."""
    return patricia_envelope.census(keys)["mean_path_nodes"]


# ---------------------------------------------------------------------------
# Synthetic names (reproduced in test_dns_name_bounds.rs)
# ---------------------------------------------------------------------------

NAME_SEED = 0xD05_5EED_0000_0001
TLDS = (b"com", b"net", b"org", b"io", b"de", b"uk")
HOSTS = (b"www", b"mail", b"ns1")
ALPHABET = b"abcdefghijklmnopqrstuvwxyz0123456789"


def gen_names(n: int, seed: int = NAME_SEED) -> list[tuple[bytes, ...]]:
    """`n` distinct SYNTHETIC names in wire label order: registrations
    `<sld>.<tld>` with a uniform [a-z0-9] label of 3..=16 octets, each
    followed by its first 0..=3 names from `HOSTS`."""
    if n < 1:
        raise ValueError("need n >= 1")
    rng = patricia_envelope.xorshift(seed)
    seen: set[tuple[bytes, bytes]] = set()
    out: list[tuple[bytes, ...]] = []
    while len(out) < n:
        r = next(rng)
        tld = TLDS[r % len(TLDS)]
        length = 3 + (r >> 8) % 14
        hosts = (r >> 16) % 4
        sld = bytes(ALPHABET[next(rng) % len(ALPHABET)] for _ in range(length))
        if (sld, tld) in seen:
            continue
        seen.add((sld, tld))
        out.append((sld, tld))
        out.extend((h, sld, tld) for h in HOSTS[:hosts])
    return out[:n]


# ---------------------------------------------------------------------------
# Pinned tests
# ---------------------------------------------------------------------------

# RFC 4034 §6.1's example, in the order the RFC lists it. `\001` and `\200`
# are decimal escapes (RFC 1035 §5.1): octets 1 and 200.
RFC4034_SORTED: tuple[tuple[bytes, ...], ...] = (
    (b"example",),
    (b"a", b"example"),
    (b"yljkjljk", b"a", b"example"),
    (b"Z", b"a", b"example"),
    (b"zABC", b"a", b"EXAMPLE"),
    (b"z", b"example"),
    (b"\x01", b"z", b"example"),
    (b"*", b"z", b"example"),
    (b"\xc8", b"z", b"example"),
)

# Census of `gen_names(n)` under `key_direct` and `key_ordered_bytes_stored`:
# (shells, suffixes, terminals, suffix_bytes, leaf_submap_bytes, over_submaps,
#  over_entries, max_levels). test_dns_name_bounds.rs asserts the engine's
# `stats()` reports the same shells, suffixes and suffix bytes, and that
# `mem_used()` is at least the lower bound, with equality at n = 24.
PINS: dict[tuple[int, str], tuple[int, ...]] = {
    (24, "direct"): (7, 11, 13, 336, 496, 0, 0, 3),
    (24, "ordered_bytes_stored"): (10, 7, 17, 208, 576, 0, 0, 3),
    (2000, "direct"): (829, 785, 1215, 21840, 34432, 1, 862, 3),
    (2000, "ordered_bytes_stored"): (934, 1046, 954, 30720, 37088, 1, 811, 3),
}


def _pin_tuple(c: dict) -> tuple[int, ...]:
    return (c["shells"], c["suffixes"], c["terminals"], c["suffix_bytes"],
            c["leaf_submap_bytes"], c["over_submaps"], c["over_entries"], c["max_levels"])


def _arbitrary_names(count: int, seed: int) -> list[tuple[bytes, ...]]:
    """Names over a small alphabet that includes octets 0, 1, 2 and both
    letter cases, so the escapes and the case fold are exercised."""
    rng = patricia_envelope.xorshift(seed)
    alphabet = bytes((0, 1, 2, 3, 0x2A, 0x41, 0x42, 0x61, 0x62, 0x7A, 0xC8, 0xFF))
    out = []
    for _ in range(count):
        labels = tuple(
            bytes(alphabet[next(rng) % len(alphabet)] for _ in range(1 + next(rng) % 4))
            for _ in range(1 + next(rng) % 4))
        out.append(labels)
    return out


def test_bounds() -> None:
    # --- RFC 4034 §6.1 example: every encoding sorts it as the RFC lists it ---
    shuffled = list(reversed(RFC4034_SORTED))
    assert tuple(sorted(shuffled, key=canonical_sort_key)) == RFC4034_SORTED
    for name, enc in ENCODERS.items():
        assert tuple(sorted(shuffled, key=enc)) == RFC4034_SORTED, name

    # --- encodings, by hand ---
    api = (b"api", b"v2", b"example", b"com")
    assert key_nul(api) == b"com\x00example\x00v2\x00api\x00"
    assert key_direct(api) == b"com\x01example\x01v2\x01api\x01"
    assert key_ordered_bytes_stored(api) == b"com\x01\x01example\x01\x01v2\x01\x01api\x01\x01"
    assert key_nul((b"A\x00\x01",)) == b"a\x01\x01\x01\x02\x00"
    assert key_direct((b"A\x00\x01\x02",)) == b"a\x02\x02\x02\x03\x02\x04\x01"
    assert (encoded_len(api, "nul"), encoded_len(api, "direct"),
            encoded_len(api, "ordered_bytes_stored")) == (19, 19, 23)

    # --- order and injectivity on names with arbitrary octets ---
    names = _arbitrary_names(4000, 0xA5A5_0001)
    canon = {canonical_sort_key(nm) for nm in names}
    for name, enc in ENCODERS.items():
        by_key = {}
        for nm in names:
            k = enc(nm)
            assert len(k) == encoded_len(nm, name), (name, nm)
            assert by_key.setdefault(k, canonical_sort_key(nm)) == canonical_sort_key(nm), name
        assert len(by_key) == len(canon), f"{name} is not injective on canonical names"
        assert [by_key[k] for k in sorted(by_key)] == sorted(canon), f"{name} does not preserve order"
        if name != "nul":
            assert all(0 not in k for k in by_key), f"{name} must be NUL-free"

    # --- name validation ---
    for bad in ((), (b"",), (b"a" * 64,), tuple(b"a" * 63 for _ in range(4))):
        try:
            check_name(bad)
        except ValueError:
            pass
        else:
            raise AssertionError(f"check_name accepted {bad!r}")

    # --- engine byte rules ---
    assert [cap_class(p) for p in (0, 1, 2, 3, 4, 5, 16, 17, 24, 25, 31, 32, 33)] == \
        [0, 1, 2, 4, 4, 8, 16, 24, 24, 32, 32, 32, 36]
    assert [suffix_leaf_bytes(s) for s in (0, 1, 16, 17)] == [16, 32, 32, 48]
    assert (root_leaf_bytes(1), root_leaf_bytes(31)) == (16, 512)

    # --- census, by hand ---
    # One 7-byte key: a terminal in the root, nothing else.
    c = strmap_census([b"abcdefg"])
    assert _pin_tuple(c) == (1, 0, 1, 0, 16, 0, 0, 1) and mem_used_lower_bound(c) == 48 + 16
    # One 8-byte key: not terminal (no room for the NUL), so a suffix leaf of 0 bytes.
    c = strmap_census([b"abcdefgh"])
    assert _pin_tuple(c) == (1, 1, 0, 16, 16, 0, 0, 1)
    # Two keys sharing a chunk: a child holding one terminal (the empty
    # remainder) and one terminal "X".
    c = strmap_census([b"abcdefgh", b"abcdefghX"])
    assert _pin_tuple(c) == (2, 0, 2, 0, 16 + 32, 0, 0, 2) and c["mean_levels"] == 2.0
    # 16 shared bytes then divergence: a chain of three nodes, two suffixes.
    c = strmap_census([b"0123456789abcdefAAAAAAAAtail", b"0123456789abcdefBBBBBBBBtail"])
    assert _pin_tuple(c) == (3, 2, 0, 2 * 32, 16 + 16 + 32, 0, 0, 3)
    # 40 distinct first chunks: the root sub-map leaves root-leaf state.
    c = strmap_census([b"%08d-rest" % i for i in range(40)])
    assert (c["over_submaps"], c["over_entries"], c["leaf_submap_bytes"], c["suffixes"]) == (1, 40, 0, 40)
    for bad_keys in ([], [b"a\x00b"]):
        try:
            strmap_census(bad_keys)
        except ValueError:
            pass
        else:
            raise AssertionError("strmap_census accepted an invalid key set")

    # --- the escape pass costs one byte per label, and what that does to the census ---
    names = gen_names(2000)
    assert len(set(names)) == 2000
    direct = [key_direct(nm) for nm in names]
    stored = [key_ordered_bytes_stored(nm) for nm in names]
    assert sum(map(len, stored)) - sum(map(len, direct)) == sum(len(nm) for nm in names)

    # --- pinned census values (also asserted against the engine) ---
    for (n, enc), want in PINS.items():
        got = _pin_tuple(strmap_census([ENCODERS[enc](nm) for nm in gen_names(n)]))
        assert got == want, f"pin ({n}, {enc}): {got} != {want}"
    assert PINS, "the census pins must not be empty"
    assert PINS[(24, "direct")][5] == 0, "the n = 24 cell must be exact (no sub-map above ROOT_LEAF_CAP)"

    # --- BTreeMap node sizes and bounds ---
    assert (BTREE_LEAF_NODE, BTREE_INTERNAL_NODE) == (368, 464)
    assert btreemap_bytes_bounds(11, 100) == (368 + 100, 3 * 464 + 100)
    lo, hi = btreemap_bytes_bounds(1_000_000, 0)
    assert (lo, hi) == (90_910 * 368, 200_001 * 464)
    for bad_n in (0, -1):
        try:
            btreemap_bytes_bounds(bad_n, 0)
        except ValueError:
            pass
        else:
            raise AssertionError("btreemap_bytes_bounds accepted n < 1")

    # --- radix path lengths reuse the patricia envelope ---
    assert radix_path_nodes([b"ab\x01", b"ac\x01", b"b\x01"]) == (3 + 3 + 2) / 3
    # A name and its child: the parent is a prefix of the child, which sits one node deeper.
    assert radix_path_nodes([b"com\x01ab\x01", b"com\x01ab\x01www\x01", b"net\x01"]) == (2 + 3 + 2) / 3


def report(pops=(1_000, 10_000, 100_000, 1_000_000)) -> None:
    print("SYNTHETIC names (uniform [a-z0-9] labels); see the module docstring.")
    print(f"{'n':>9} {'encoding':21} {'B/key':>6} {'shells':>8} {'suffixes':>8} {'levels':>6} "
          f"{'max':>3} {'>31 maps':>8} {'entries in them':>15} {'lower B/name':>12} "
          f"{'radix nodes':>11} {'btree B/name':>15}")
    for n in pops:
        names = gen_names(n)
        for enc in ("direct", "ordered_bytes_stored"):
            keys = [ENCODERS[enc](nm) for nm in names]
            c = strmap_census(keys)
            total = sum(map(len, keys))
            lo, hi = btreemap_bytes_bounds(n, total)
            qp = f"{radix_path_nodes(keys):11.2f}"
            print(f"{n:>9} {enc:21} {total / n:6.2f} {c['shells']:>8} {c['suffixes']:>8} "
                  f"{c['mean_levels']:6.2f} {c['max_levels']:>3} {c['over_submaps']:>8} "
                  f"{c['over_entries']:>15} {mem_used_lower_bound(c) / n:12.2f} {qp} "
                  f"{lo / n:7.1f}-{hi / n:<7.1f}")


def main() -> int:
    if "--pins" in sys.argv:
        for n in (24, 2000):
            for enc in ("direct", "ordered_bytes_stored"):
                c = strmap_census([ENCODERS[enc](nm) for nm in gen_names(n)])
                print(f"    ({n}, \"{enc}\"): {_pin_tuple(c)},  # lower bound {mem_used_lower_bound(c)}")
        return 0
    test_bounds()
    if "--report" in sys.argv:
        report()
    print("dns_name_bounds: all bound tests passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
