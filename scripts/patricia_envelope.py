#!/usr/bin/env python3
"""
scripts/patricia_envelope.py — exact node census and descent-cost bounds for the
twins in `docs/benchmarks/patricia_comparison/`.

AGENTS.md §8.8 commit 1: the bounds the pre-registration reads are computed here,
in committed Python with pinned tests, not narrated.

What is derived, per key set:
- `patricia_tree` 0.10.2: exact allocation bytes and node count, and the mean
  number of nodes a successful `get` visits. Source: `src/node.rs` (MIT,
  https://github.com/sile/patricia_tree). A node is ONE allocation laid out as
  `flags: u8, label_len: u8, label: [u8], value?, child?, sibling?`, each
  optional field present only when set, built with `Layout::extend` and
  `pad_to_align` (`new_with_boundary`). Children are a byte-sorted singly
  linked sibling list; `Node::get` walks it linearly.
- `fast_radix_trie` 1.2.0: the same compressed tree with children in an inline
  array, so a hit visits the nodes on its root-to-value path. Its child count
  is a `u8` (`node.rs`: `children_len: self.children_len() as u8 + 1`), so any
  node with 256 children is unrepresentable: the default `realloc` build
  panics, and the build without it wraps the count to 0 and loses the children
  (both reproduced against the crate). `max_fanout == 256` therefore predicts
  an invalid cell.
- `qp-trie` 0.8.2: a crit-nybble trie; a hit passes one branch node per point
  on its path where the key set diverges at nybble granularity.

The byte census is exact only for keys of at most 255 bytes: longer keys split
labels at creation and `split_at` does not re-chunk, which makes the node set
insertion-order dependent. `census` refuses such keys, and the empty key.
Memory for `fast_radix_trie`, `qp-trie` and Expanse is not derived here; the
harness measures it.
"""

from __future__ import annotations

import sys

# A `Node<V>` is one non-null pointer, so each child/sibling slot is 8 bytes.
PTR = 8
VALUE = 8
ALIGN = 8
HEADER = 2  # flags + label_len
MAX_LABEL_LEN = 255
MASK64 = 2**64 - 1


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
    """Exact census of the compressed trie holding `keys` (duplicates ignored).

    Returns `patricia_tree` bytes and node count; `mean_hit_visits`, the nodes a
    `patricia_tree` hit visits (root, every child descent, every sibling step);
    `mean_path_nodes`, the nodes on the root-to-value path (what an inline child
    array visits); `max_fanout`, the largest child list.
    """
    ks = sorted(set(keys))
    if not ks:
        raise ValueError("census needs at least one key")
    if any(len(k) == 0 for k in ks):
        raise ValueError("the empty key lives on the root; not modelled")
    if any(len(k) > MAX_LABEL_LEN for k in ks):
        raise ValueError("keys longer than MAX_LABEL_LEN split labels order-dependently; not modelled")
    total = node_bytes(0, False, True, False)  # root: empty label, one child list
    nodes = 1
    visits = 0
    path = 0
    max_fanout = 0
    # Work item: sorted slice ks[lo:hi] sharing ks[lo][:depth]; `base` is the
    # visit number of the list's first sibling; `plen` the path length so far.
    stack = [(0, len(ks), 0, 2, 1)]
    while stack:
        lo, hi, depth, base, plen = stack.pop()
        groups = []
        i = lo
        while i < hi:
            b = ks[i][depth]
            j = i + 1
            while j < hi and ks[j][depth] == b:
                j += 1
            groups.append((i, j))
            i = j
        max_fanout = max(max_fanout, len(groups))
        for idx, (g_lo, g_hi) in enumerate(groups):
            here = base + idx  # siblings before it are walked first
            label = _lcp(ks[g_lo], ks[g_hi - 1], depth)  # sorted: first/last LCP = group LCP
            end = depth + label
            has_value = len(ks[g_lo]) == end  # sorted: a key ending here sorts first
            rest_lo = g_lo + 1 if has_value else g_lo
            has_child = rest_lo < g_hi
            total += node_bytes(label, has_value, has_child, idx + 1 < len(groups))
            nodes += 1
            if has_value:
                visits += here
                path += plen + 1
            if has_child:
                stack.append((rest_lo, g_hi, end, here + 1, plen + 1))
    n = len(ks)
    return {"bytes": total, "nodes": nodes, "keys": n, "bytes_per_key": total / n,
            "mean_hit_visits": visits / n, "mean_path_nodes": path / n, "max_fanout": max_fanout}


def branch_points(keys: list[bytes]) -> float:
    """Mean number of branch points (subtrees with at least two children) on a
    key's path in a trie over 4-bit digits taken low nybble first within each
    byte — the order `qp-trie` 0.8.2 branches in (its traversal of keys
    0, 1, …, 39 yields 0, 16, 32, 1, 17, 33, …)."""

    def digits(k: bytes) -> bytes:
        return bytes(d for b in k for d in (b & 0xF, b >> 4))

    ks = sorted({digits(k) for k in keys})
    if not ks:
        raise ValueError("branch_points needs at least one key")
    total = 0
    stack = [(0, len(ks), 0, 0)]
    while stack:
        lo, hi, depth, count = stack.pop()
        if hi - lo == 1:
            total += count
            continue
        d = depth + _lcp(ks[lo], ks[hi - 1], depth)
        i = lo
        while i < hi:
            j = i + 1
            while j < hi and ks[j][d] == ks[i][d]:
                j += 1
            stack.append((i, j, d + 1, count + 1))
            i = j
    return total / len(ks)


def be64(k: int) -> bytes:
    """The big-endian 8-byte encoding the harness feeds the twins."""
    return (k & MASK64).to_bytes(8, "big")


def expanse_max_levels(key_bytes: int = 8) -> int:
    """Upper bound on Expanse `u64` descent levels: one per key byte, most
    significant first (the digital-tree invariant, AGENTS.md §2.1 item 1;
    docs/ARCHITECTURE.md's 32-bit section states the 8-to-4 halving)."""
    return key_bytes


# ---------------------------------------------------------------------------
# Harness key streams, reproduced exactly (patricia_common::u64_dist / gen_paths)
# ---------------------------------------------------------------------------

SHARED_SEED = 0x0DDB_1A5E_5EED_0001
STRING_SEED = 0x5A71_C1A0_0000_0001


def xorshift(seed: int):
    x = seed
    while True:
        x ^= (x << 13) & MASK64
        x ^= x >> 7
        x ^= (x << 17) & MASK64
        yield x


def _dedupe(ks):
    seen, out = set(), []
    for k in ks:
        if k not in seen:
            seen.add(k)
            out.append(k)
    return out


def u64_dist(dist: str, n: int) -> list[int]:
    """`patricia_common::u64_dist` for every distribution but zipfian (whose
    float CDF is not reproduced bit for bit here)."""
    rng = xorshift(SHARED_SEED)
    if dist == "sequential":
        return list(range(n))
    if dist == "sparse_stride":
        return [(i << 32) & MASK64 for i in range(n)]
    if dist == "uniform_random":
        return _dedupe(next(rng) for _ in range(n))
    if dist == "clustered":
        out = []
        while len(out) < n:
            base = (next(rng) & 0x00FF_FFFF_FFFF_0000) ^ ((len(out) << 32) & MASK64)
            out.extend((base + i) & MASK64 for i in range(min(n - len(out), 1024)))
        return _dedupe(out)
    raise ValueError(f"not reproduced: {dist}")


def path_prefix(length: int) -> bytes:
    p = b"https://example.com/api/v2/objects/"
    while len(p) < length:
        p += b"seg/"
    return p[:length]


def gen_paths(n: int, prefix_len: int, seed: int = STRING_SEED) -> list[bytes]:
    prefix, rng, seen, out = path_prefix(prefix_len), xorshift(seed), set(), []
    while len(out) < n:
        i = next(rng) & 0xFFFF_FFFF_FFFF
        if i not in seen:
            seen.add(i)
            out.append(prefix + b"%012x" % i)
    return out


# Census values for the two pinned non-sequential cells; confirmed equal to the
# `patricia_memory` allocator hook (patricia_tree requested bytes / allocations).
UNIFORM_10K = (323_720, 10_945)
PATHS35_10K = (379_864, 13_746)


def test_bounds() -> None:
    # node layouts, derived by hand from node.rs
    assert node_bytes(0, False, True, False) == 16   # root: 2 -> pad 8, +8 child
    assert node_bytes(1, True, False, False) == 16   # last leaf
    assert node_bytes(1, True, False, True) == 24    # leaf with a sibling
    assert node_bytes(6, True, False, True) == 24    # 8-byte header still
    assert node_bytes(7, True, False, True) == 32    # 9 -> pad 16
    assert node_bytes(7, True, False, False) == 24
    assert node_bytes(0, False, False, False) == 2
    for bad in (lambda: node_bytes(256, False, False, False),
                lambda: census([b""]), lambda: census([b"x" * 256])):
        try:
            bad()
        except ValueError:
            pass
        else:
            raise AssertionError("must be rejected")

    # {0, 1}: root 16 + shared 7-zero-byte node (9 -> 16, +child) 24
    #          + leaf [0] with sibling 24 + leaf [1] 16 = 80
    c = census([be64(0), be64(1)])
    assert c["bytes"] == 80 and c["nodes"] == 4, c
    # visits: root, shared, leaf0 -> 3 ; root, shared, leaf0 (sibling step), leaf1 -> 4
    assert c["mean_hit_visits"] == 3.5, c
    assert c["mean_path_nodes"] == 3 and c["max_fanout"] == 2, c

    # dense 0..256: one 7-byte shared node, 256 one-byte leaves, 255 with sibling
    c = census([be64(k) for k in range(256)])
    assert c["bytes"] == 16 + 24 + 255 * 24 + 16, c
    assert c["mean_hit_visits"] == 2 + 1 + 127.5, c  # (0+...+255)/256 sibling steps
    assert c["max_fanout"] == 256, c                  # fast_radix_trie cannot hold it
    assert census([be64(k) for k in range(255)])["max_fanout"] == 255

    # a key that is a prefix of others: "ab" carries a value and a child list.
    # root 16; "ab" (2+2 -> pad 8, +value +child) 24; "c" (3 -> 8, +value
    # +sibling) 24; "d" (3 -> 8, +value) 16
    c = census([b"ab", b"abc", b"abd"])
    assert c["bytes"] == 16 + 24 + 24 + 16 and c["nodes"] == 4, c
    assert c["mean_path_nodes"] == (2 + 3 + 3) / 3, c

    # qp-trie branch points, low nybble first: {0x00, 0x01} branch once on the
    # low nybble. {0x00, 0x01, 0x02, 0x10}: the low nybble splits {0x00, 0x10}
    # from 0x01 and 0x02, then the high nybble splits 0x00 from 0x10 -> 2, 2, 1, 1.
    # High-nybble-first would give 2, 2, 2, 1, so this pins the order.
    assert branch_points([b"\x00", b"\x01"]) == 1.0
    assert branch_points([b"\x00", b"\x01", b"\x02", b"\x10"]) == 1.5

    # Pinned against the allocator hook: `patricia_memory` records exactly these
    # requested bytes and allocations for patricia_tree on the same key sets, in
    # both build orders. Three different shapes: dense 1-byte leaves; uniform
    # 64-bit keys with 7-byte leaf labels; shared-prefix paths with a 35-byte
    # label and 16-way hex lists.
    c = census([be64(k) for k in u64_dist("sequential", 10_000)])
    assert (c["bytes"], c["nodes"]) == (240_664, 10_042), c
    c = census([be64(k) for k in u64_dist("uniform_random", 10_000)])
    assert (c["bytes"], c["nodes"]) == UNIFORM_10K, c
    c = census(gen_paths(10_000, 35))
    assert (c["bytes"], c["nodes"]) == PATHS35_10K, c

    assert expanse_max_levels() == 8


def report(pops=(10_000, 100_000, 1_000_000)) -> None:
    print(f"{'n':>9} {'distribution':16} {'B/key':>7} {'visits/hit':>10} "
          f"{'path nodes':>10} {'qp branches':>11} {'max fanout':>10}")
    for n in pops:
        rows = [(d, [be64(k) for k in u64_dist(d, n)])
                for d in ("sequential", "clustered", "uniform_random", "sparse_stride")]
        if n <= 100_000:  # 1M path keys of up to 252 bytes is past this script's budget
            rows += [(f"path/{pl}", gen_paths(n, pl)) for pl in (8, 35, 128, 240)]
        for name, keys in rows:
            c = census(keys)
            print(f"{n:>9} {name:16} {c['bytes_per_key']:7.2f} {c['mean_hit_visits']:10.2f} "
                  f"{c['mean_path_nodes']:10.2f} {branch_points(keys):11.2f} {c['max_fanout']:10}")


def main() -> int:
    if "--pins" in sys.argv:
        for name, keys in (("uniform_random", [be64(k) for k in u64_dist("uniform_random", 10_000)]),
                           ("path/35", gen_paths(10_000, 35))):
            c = census(keys)
            print(name, c["bytes"], c["nodes"])
        return 0
    test_bounds()
    if "--report" in sys.argv:
        report()
    print("patricia_envelope: all bound tests passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
