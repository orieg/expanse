#!/usr/bin/env python3
"""
scripts/patricia_envelope.py — exact node census and descent-cost bounds for the
`patricia_tree` crate (v0.10.2), the Patricia-trie twin in
`docs/benchmarks/patricia_comparison/`.

AGENTS.md §8.8 commit 1: the bounds the pre-registration reads are computed here,
in committed Python with pinned tests, not narrated.

Primary sources:
- Morrison, "PATRICIA — Practical Algorithm To Retrieve Information Coded in
  Alphanumeric", J. ACM 15(4), 1968, doi:10.1145/321479.321481 (the structure).
- `patricia_tree` 0.10.2, `src/node.rs` (MIT, https://github.com/sile/patricia_tree):
  a node is ONE allocation laid out as `flags: u8, label_len: u8, label: [u8],
  value: Option<V>, child: Option<Node>, sibling: Option<Node>`, each optional
  field present only when set, built with `Layout::extend` and `pad_to_align`
  (node.rs `new_with_boundary`). Children are a singly linked, byte-sorted
  sibling list; `Node::get` walks it linearly and stops early once the probe's
  first byte is below a sibling's first label byte.

The census below rebuilds that exact node set for a key set and sums the layout
sizes, so it predicts the bytes the `patricia_memory` harness's GlobalAlloc hook
records (requested `Layout::size`, allocator overhead excluded on both arms).
"""

from __future__ import annotations

import sys

# `Option<Node<V>>` is one pointer (niche on a non-null `*mut u8`); u64 values.
PTR = 8
VALUE = 8
ALIGN = 8
HEADER = 2  # flags + label_len
MAX_LABEL_LEN = 255


def _pad(off: int, align: int) -> int:
    return (off + align - 1) // align * align


def node_bytes(label_len: int, has_value: bool, has_child: bool, has_sibling: bool) -> int:
    """Allocation size of one `patricia_tree::Node<u64>` (node.rs `new_with_boundary`)."""
    if not 0 <= label_len <= MAX_LABEL_LEN:
        raise ValueError(f"label_len must be in [0, {MAX_LABEL_LEN}], got {label_len}")
    size, align = HEADER + label_len, 1
    for present, width in ((has_value, VALUE), (has_child, PTR), (has_sibling, PTR)):
        if present:
            size = _pad(size, ALIGN) + width
            align = ALIGN
    return _pad(size, align)


def _lcp(a: bytes, b: bytes, start: int) -> int:
    i = start
    while i < len(a) and i < len(b) and a[i] == b[i]:
        i += 1
    return i - start


def census(keys: list[bytes]) -> dict:
    """Exact node census of the patricia tree holding `keys` (duplicates ignored).

    Returns total bytes, node count, and the mean number of nodes a successful
    `get` visits (child descents plus sibling steps), which is the count of
    dependent pointer loads on a hit.
    """
    ks = sorted(set(keys))
    if not ks:
        raise ValueError("census needs at least one key")
    if any(len(k) > MAX_LABEL_LEN for k in ks):
        raise ValueError("keys longer than MAX_LABEL_LEN split labels; not modelled")
    total = 0
    nodes = 0
    visits = 0

    # Iterative build of sibling lists. Work item: (lo, hi, depth, visits_so_far)
    # over the sorted slice ks[lo:hi], all sharing ks[lo][:depth].
    stack = [(0, len(ks), 0, 2)]  # root is visit 1, first top-level sibling visit 2
    # Root: empty label, no value, one child list.
    total += node_bytes(0, False, True, False)
    nodes += 1
    while stack:
        lo, hi, depth, base = stack.pop()
        # Partition by the byte at `depth` (keys ending at depth are values of the
        # parent and never reach here for prefix-free sets; see check below).
        groups = []
        i = lo
        while i < hi:
            b = ks[i][depth]
            j = i + 1
            while j < hi and ks[j][depth] == b:
                j += 1
            groups.append((i, j))
            i = j
        for idx, (g_lo, g_hi) in enumerate(groups):
            here = base + idx  # siblings before it are walked first
            label = _lcp(ks[g_lo], ks[g_hi - 1], depth)  # sorted: first/last LCP = group LCP
            end = depth + label
            has_value = len(ks[g_lo]) == end
            rest_lo = g_lo + 1 if has_value else g_lo
            has_child = rest_lo < g_hi
            has_sibling = idx + 1 < len(groups)
            total += node_bytes(label, has_value, has_child, has_sibling)
            nodes += 1
            if has_value:
                visits += here
            if has_child:
                stack.append((rest_lo, g_hi, end, here + 1))
    return {"bytes": total, "nodes": nodes, "keys": len(ks),
            "bytes_per_key": total / len(ks), "mean_hit_visits": visits / len(ks)}


def be64(k: int) -> bytes:
    """The big-endian 8-byte encoding the harness feeds `PatriciaMap`."""
    return (k & (2**64 - 1)).to_bytes(8, "big")


def expanse_max_levels(key_bytes: int = 8) -> int:
    """Upper bound on Expanse descent levels: one per key byte (docs/ARCHITECTURE.md §2)."""
    return key_bytes


def test_bounds() -> None:
    # node layouts, derived by hand from node.rs
    assert node_bytes(0, False, True, False) == 16   # root: 2 -> pad 8, +8 child
    assert node_bytes(1, True, False, False) == 16   # last leaf
    assert node_bytes(1, True, False, True) == 24    # leaf with a sibling
    assert node_bytes(6, True, False, True) == 24    # 8-byte header still
    assert node_bytes(7, True, False, True) == 32    # 9 -> pad 16
    assert node_bytes(0, False, False, False) == 2
    try:
        node_bytes(256, False, False, False)
    except ValueError:
        pass
    else:
        raise AssertionError("label_len 256 must be rejected")

    # {0, 1}: root 16 + shared 7-zero-byte node (9 -> 16, +child) 24
    #          + leaf [0] with sibling 24 + leaf [1] 16 = 80
    c = census([be64(0), be64(1)])
    assert c["bytes"] == 80 and c["nodes"] == 4, c
    # visits: root, shared, leaf0 -> 3 ; root, shared, leaf0(sibling step), leaf1 -> 4
    assert c["mean_hit_visits"] == 3.5, c

    # dense 0..256: one 7-byte shared node, 256 one-byte leaves, 255 with sibling
    c = census([be64(k) for k in range(256)])
    assert c["bytes"] == 16 + 24 + 255 * 24 + 16, c
    # mean sibling walk over a full 256-way list = (0+...+255)/256 = 127.5
    assert c["mean_hit_visits"] == 2 + 1 + 127.5, c

    # Pinned against the allocator hook: `patricia_memory` records exactly these
    # counts for sequential n=10,000 (`patricia_live_bytes` / `_allocs`); the
    # census is only usable as a prediction while the two agree.
    c = census([be64(k) for k in range(10_000)])
    assert (c["bytes"], c["nodes"]) == (240_664, 10_042), c

    assert expanse_max_levels() == 8


def main() -> int:
    test_bounds()
    if "--report" in sys.argv:
        from itertools import islice

        def xorshift(seed: int):
            x = seed
            while True:
                x ^= (x << 13) & (2**64 - 1)
                x ^= x >> 7
                x ^= (x << 17) & (2**64 - 1)
                yield x

        seed = 0x0DDB_1A5E_5EED_0001  # art_common::SHARED_SEED
        for n in (10_000, 100_000, 1_000_000):
            rows = {
                "sequential": [be64(k) for k in range(n)],
                "sparse_stride": [be64(k << 32) for k in range(n)],
                "uniform_random": [be64(k) for k in islice(xorshift(seed), n)],
            }
            for name, keys in rows.items():
                c = census(keys)
                print(f"n={n:>9} {name:15} {c['bytes_per_key']:6.2f} B/key  "
                      f"{c['mean_hit_visits']:7.2f} nodes/hit")
    print("patricia_envelope: all bound tests passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
