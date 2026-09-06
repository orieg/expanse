#!/usr/bin/env python3
"""
scripts/masstree_envelope.py — Math-first bounds for the Masstree comparison arm
(orieg/expanse #661): node-layout floors, layering depth for shared-prefix keys,
the allocator census's slab quantum, and the smallest ratio the suite's interval
method can distinguish from parity.

Commit 1 of the three-commit cadence (AGENTS.md §8.8): every bound is a function
with a reference-pinned test, and the pre-registration in
docs/benchmarks/masstree_comparison/METHODOLOGY.md invokes these functions rather
than restating their arithmetic. A bound that lives only in prose is not a bound.

Reference constants are pinned to the Step 0 gate program's output on the
reference host (Masstree `kohler/masstree-beta` 1119842, `g++ 11.4 -O3 -std=c++17
-march=haswell -DNDEBUG`; workload `step0_masstree_gate`). They are layout facts
and node counts, not timings.
"""

from __future__ import annotations

import math

# ---------------------------------------------------------------------------
# 1. Pinned reference constants — Masstree node layout, nodeparams<15, 15>
# ---------------------------------------------------------------------------

# sizeof(Masstree::leaf<P>) with leaf_width = 15, uint64_t values, phantom epoch
# on: 312 B. Allocated through threadinfo::pool_allocate, which rounds to whole
# cache lines: 320 B. (measured: step0_masstree_gate, sizeof)
MASSTREE_LEAF_SIZEOF = 312
MASSTREE_LEAF_WIDTH = 15
# sizeof(Masstree::internode<P>): 272 B, pool-rounded to 320 B — the same
# size class as a leaf, so the two share one slab. (measured: sizeof)
MASSTREE_INTERNODE_SIZEOF = 272
MASSTREE_INTERNODE_WIDTH = 15
# kvthread.hh: pool_allocate rounds to CACHE_LINE_SIZE = 64.
MASSTREE_CACHE_LINE = 64
# masstree_key.hh: an ikey is one 8-byte slice; a layer descends one slice.
MASSTREE_IKEY_SIZE = 8
# configure.ac default: --enable-max-key-len=255. Keys longer than this are
# outside the library's contract (scan writes into MASSTREE_MAXKEYLEN buffers).
MASSTREE_MAXKEYLEN = 255
# kvthread.cc refill_pool: one slab per size class per thread, 2 MiB (the
# superpage size on the reference host and the fallback size without one).
MASSTREE_POOL_SLAB = 2 << 20
# kvthread.hh: pool_max_nlines = 20 size classes per thread.
MASSTREE_POOL_CLASSES = 20
# kvthread.cc threadinfo::make: `new(malloc(8192)) threadinfo`, plus the first
# limbo_group (sizeof 4072, allocated in the constructor). Per-thread constant.
MASSTREE_THREADINFO_BLOCK = 8192
MASSTREE_LIMBO_GROUP = 4072

# ---------------------------------------------------------------------------
# 2. Pinned reference constants — Step 0 node census, uniform random u64, N = 1M
# ---------------------------------------------------------------------------

# json_stats over a 1,000,000-key table built from the suite's XorShift64 stream
# (measured: step0_masstree_gate, reference host).
STEP0_RANDOM_1M_KEYS = 1_000_000
STEP0_RANDOM_1M_LEAVES = 94_381
STEP0_RANDOM_1M_INTERNODES = 9_144
STEP0_RANDOM_1M_STRUCTURAL_BYTES = 33_128_000
# Allocator census, build-only, single thread, table created inside the armed
# window: 16 slabs of 2 MiB. (derived from the gate's 15 counted + 1 that the
# gate created outside its window; the harness arms before `initialize`.)
STEP0_RANDOM_1M_SLABS = 16

# ---------------------------------------------------------------------------
# 3. Published Expanse anchors this arm's predictions lean on
# ---------------------------------------------------------------------------

# ExpanseMap B/key across expanse occupancy λ, allocator instrument
# (measured: reference host, docs/benchmarks/hot_comparison/README.md §1,
# results/baseline_memory_curve.json, workload hot_map_64bit).
EXPANSE_MAP_RANDOM_BPK_MIN = 16.26
EXPANSE_MAP_RANDOM_BPK_MAX = 24.71


# ---------------------------------------------------------------------------
# 4. Pure bound functions
# ---------------------------------------------------------------------------

def pool_round(size: int) -> int:
    """Bytes `threadinfo::pool_allocate` hands out for a `size`-byte request."""
    if size <= 0:
        raise ValueError("size must be positive")
    lines = -(-size // MASSTREE_CACHE_LINE)
    return lines * MASSTREE_CACHE_LINE


def structural_bytes(leaves: int, internodes: int, ksuf_capacity: int = 0,
                     overridden_ksuf_capacity: int = 0) -> int:
    """Bytes Masstree's nodes occupy, from its own `json_stats` counts.

    Leaves and internodes are pool allocations at their pool-rounded sizes;
    external key-suffix bags (`ksuf_capacity`) are `malloc`ed at capacity;
    `overridden_ksuf_capacity` is the extra a leaf was allocated beyond its
    minimum for an internal suffix bag. This is the engine-instrument column
    the arm publishes beside the allocator census (METHODOLOGY §3.3).
    """
    for name, v in (("leaves", leaves), ("internodes", internodes),
                    ("ksuf_capacity", ksuf_capacity),
                    ("overridden_ksuf_capacity", overridden_ksuf_capacity)):
        if v < 0:
            raise ValueError(f"{name} must be non-negative")
    return (leaves * pool_round(MASSTREE_LEAF_SIZEOF)
            + internodes * pool_round(MASSTREE_INTERNODE_SIZEOF)
            + ksuf_capacity + overridden_ksuf_capacity)


def leaf_fill(n_keys: int, leaves: int) -> float:
    """Mean leaf occupancy as a fraction of `leaf_width`."""
    if leaves <= 0 or n_keys < 0:
        raise ValueError("leaves must be positive and n_keys non-negative")
    return n_keys / (leaves * MASSTREE_LEAF_WIDTH)


def leaf_only_bpk(fill: float) -> float:
    """Bytes per key from leaves alone at occupancy `fill` — the floor no
    internode, suffix or slab cost can lower. 21.33 B/key at full leaves."""
    if not 0.0 < fill <= 1.0:
        raise ValueError("fill must be in (0, 1]")
    return pool_round(MASSTREE_LEAF_SIZEOF) / (MASSTREE_LEAF_WIDTH * fill)


def layers_for_shared_prefix(prefix_bytes: int) -> int:
    """Layers (B+-trees, one per 8-byte slice) a key descends before the first
    byte that can discriminate it, when every key shares `prefix_bytes` bytes.

    Each shared 8-byte slice becomes a one-key twig leaf in its own layer
    (masstree_insert.hh `make_new_layer`), so the descent is `prefix // 8`
    dependent node hops before the discriminating layer.
    """
    if prefix_bytes < 0:
        raise ValueError("prefix_bytes must be non-negative")
    return prefix_bytes // MASSTREE_IKEY_SIZE


def slab_slack_bound(size_classes_touched: int, threads: int = 1) -> int:
    """Upper bound on bytes the allocator census can hold beyond Masstree's
    structural bytes: every touched size class holds at most one partially
    used 2 MiB slab per thread, and the partial slab is at most one slab."""
    if size_classes_touched < 0 or threads < 1:
        raise ValueError("size_classes_touched non-negative, threads >= 1")
    if size_classes_touched > MASSTREE_POOL_CLASSES:
        raise ValueError(f"at most {MASSTREE_POOL_CLASSES} size classes exist")
    return size_classes_touched * threads * MASSTREE_POOL_SLAB


def census_quantum_dominated(alloc_bytes: int, structural_bytes: int) -> bool:
    """Whether the allocator column says more about the 2 MiB quantum than
    about the index: true when the measured slack — bytes the allocator holds
    beyond Masstree's own structural bytes — exceeds 25% of the
    structural bytes.

    A cell flagged here is still published — the allocator figure is what the
    process holds — but the pre-registration forbids reading it as a per-key
    cost of the index (METHODOLOGY §3.3). `slab_slack_bound` is the a-priori
    ceiling the measured slack must respect for the cell to be valid at all.
    """
    if alloc_bytes < 0 or structural_bytes < 0:
        raise ValueError("byte counts must be non-negative")
    if alloc_bytes < structural_bytes:
        raise ValueError("allocator bytes below structural bytes: the census is not seeing the arm")
    return (alloc_bytes - structural_bytes) * 4 > max(structural_bytes, 1)


def per_thread_constant() -> int:
    """Bytes one `threadinfo` costs before it allocates a node: its block plus
    its first limbo group. Counted by the census when the thread slot is
    created inside the armed window; independent of N."""
    return MASSTREE_THREADINFO_BLOCK + MASSTREE_LIMBO_GROUP


def min_detectable_ratio(relative_half_width: float) -> float:
    """Smallest ratio the suite's paired BCa intervals can separate from parity.

    Planning heuristic, not a gate: with each arm's per-round timings carrying a
    relative interval half-width `h`, the ratio of two independent arms carries
    roughly `sqrt(2)·h`, so a ratio inside `1 ± sqrt(2)·h` is expected to land
    as `BOUNDARY_RESULT`. The gate itself is the BCa interval computed from the
    run (AGENTS.md §8.4); this function says which registered predictions the
    instrument can resolve at all.
    """
    if not 0.0 <= relative_half_width < 1.0:
        raise ValueError("relative_half_width must be in [0, 1)")
    return 1.0 + math.sqrt(2.0) * relative_half_width


def projected_random_bpk_at_fill(fill: float, internodes_per_leaf: float) -> float:
    """Projected Masstree B/key for 8-byte keys: leaves plus internodes at the
    measured internode-to-leaf ratio, no suffixes (8-byte keys have none)."""
    if internodes_per_leaf < 0:
        raise ValueError("internodes_per_leaf must be non-negative")
    per_leaf = pool_round(MASSTREE_LEAF_SIZEOF) + internodes_per_leaf * pool_round(MASSTREE_INTERNODE_SIZEOF)
    return per_leaf / (MASSTREE_LEAF_WIDTH * fill) if fill > 0 else float("inf")


# ---------------------------------------------------------------------------
# 5. Reference-pinned tests
# ---------------------------------------------------------------------------

def _approx(a: float, b: float, tol: float = 1e-3) -> bool:
    return abs(a - b) <= tol * max(1.0, abs(b))


def test_bounds() -> None:
    # Layout constants pin the values the gate printed; if a Masstree update
    # moves them the pre-registration's arithmetic is stale and this fails.
    assert pool_round(MASSTREE_LEAF_SIZEOF) == 320, "leaf<15> pool-rounds to 320 B"
    assert pool_round(MASSTREE_INTERNODE_SIZEOF) == 320, "internode<15> pool-rounds to 320 B"
    assert pool_round(1) == 64 and pool_round(64) == 64 and pool_round(65) == 128
    assert per_thread_constant() == 12_264

    # Step 0 node census reproduces the gate's structural figure exactly.
    s = structural_bytes(STEP0_RANDOM_1M_LEAVES, STEP0_RANDOM_1M_INTERNODES)
    assert s == STEP0_RANDOM_1M_STRUCTURAL_BYTES, s
    assert _approx(s / STEP0_RANDOM_1M_KEYS, 33.128, 1e-4)

    # Leaf occupancy on the random build: 70.6% of 15 — the B+-tree random-fill
    # regime, above ln 2 because Masstree splits unevenly toward the insert.
    f = leaf_fill(STEP0_RANDOM_1M_KEYS, STEP0_RANDOM_1M_LEAVES)
    assert _approx(f, 0.7064, 1e-3), f
    assert _approx(leaf_only_bpk(1.0), 21.3333), "full-leaf floor"
    assert _approx(leaf_only_bpk(math.log(2)), 30.777, 1e-3), "ln 2 fill"
    # Projected B/key at the measured fill and internode ratio lands on the
    # gate's structural figure — the projection is consistent with the census.
    proj = projected_random_bpk_at_fill(f, STEP0_RANDOM_1M_INTERNODES / STEP0_RANDOM_1M_LEAVES)
    assert _approx(proj, 33.128, 1e-3), proj
    # And it sits above the whole published ExpanseMap band, which is what the
    # §6 memory prediction rests on.
    assert proj > EXPANSE_MAP_RANDOM_BPK_MAX > EXPANSE_MAP_RANDOM_BPK_MIN

    # Layering: the gate's key_by_layer placed every `prefixed` key (96-byte
    # shared prefix) at layer 12 and every `beyond` key (256-byte prefix) at 32.
    assert layers_for_shared_prefix(96) == 12
    assert layers_for_shared_prefix(256) == 32
    assert layers_for_shared_prefix(0) == 0 and layers_for_shared_prefix(7) == 0
    assert layers_for_shared_prefix(8) == 1

    # Census quantum. The gate saw 0 B counted at N = 1k and N = 10k: the whole
    # build fit in the one slab created before the window. At N = 1M the 16
    # slabs are within 1.3% of the structural bytes.
    assert slab_slack_bound(1) == 2 << 20
    assert slab_slack_bound(2, threads=8) == 32 << 20
    # One slab plus the per-thread constant is what a build that fits one slab
    # holds once the table is created inside the window.
    one_slab = MASSTREE_POOL_SLAB + per_thread_constant()
    assert census_quantum_dominated(one_slab, structural_bytes(92, 9)), "N = 1k is quantum-dominated"
    assert census_quantum_dominated(one_slab, structural_bytes(936, 93)), "N = 10k is quantum-dominated"
    slabs_bytes = STEP0_RANDOM_1M_SLABS * MASSTREE_POOL_SLAB
    assert not census_quantum_dominated(slabs_bytes + per_thread_constant(),
                                        STEP0_RANDOM_1M_STRUCTURAL_BYTES), "N = 1M is not"
    assert slabs_bytes >= STEP0_RANDOM_1M_STRUCTURAL_BYTES
    assert (slabs_bytes - STEP0_RANDOM_1M_STRUCTURAL_BYTES) / STEP0_RANDOM_1M_STRUCTURAL_BYTES < 0.02
    assert slabs_bytes - STEP0_RANDOM_1M_STRUCTURAL_BYTES <= slab_slack_bound(1)

    # Detectability heuristic: 1% half-widths (the HOT suite's typical) resolve
    # ratios beyond ~1.014; a 5% prediction is resolvable, a 1% one is not.
    assert _approx(min_detectable_ratio(0.01), 1.01414, 1e-4)
    assert min_detectable_ratio(0.0) == 1.0
    assert 1.05 > min_detectable_ratio(0.01)
    assert 1.01 < min_detectable_ratio(0.01)

    # Input validation fails loud.
    for bad in (lambda: pool_round(0), lambda: leaf_only_bpk(0.0), lambda: leaf_only_bpk(1.5),
                lambda: layers_for_shared_prefix(-1), lambda: slab_slack_bound(21),
                lambda: min_detectable_ratio(1.0), lambda: structural_bytes(-1, 0),
                lambda: census_quantum_dominated(10, 20)):
        try:
            bad()
        except ValueError:
            pass
        else:
            raise AssertionError("expected ValueError")


def main() -> int:
    test_bounds()
    f = leaf_fill(STEP0_RANDOM_1M_KEYS, STEP0_RANDOM_1M_LEAVES)
    print("masstree_envelope: all reference-pinned bounds hold")
    print(f"  leaf<15> {MASSTREE_LEAF_SIZEOF} B -> {pool_round(MASSTREE_LEAF_SIZEOF)} B pooled; "
          f"internode<15> {MASSTREE_INTERNODE_SIZEOF} B -> {pool_round(MASSTREE_INTERNODE_SIZEOF)} B pooled")
    print(f"  Step 0 random 1M: fill {f:.4f}, structural {STEP0_RANDOM_1M_STRUCTURAL_BYTES / STEP0_RANDOM_1M_KEYS:.3f} B/key; "
          f"full-leaf floor {leaf_only_bpk(1.0):.2f} B/key")
    print(f"  shared-prefix layers: 96 B -> {layers_for_shared_prefix(96)}, 256 B -> {layers_for_shared_prefix(256)}")
    print(f"  census slab quantum {MASSTREE_POOL_SLAB >> 20} MiB per size class per thread; "
          f"per-thread constant {per_thread_constant()} B")
    print(f"  min detectable ratio at 1% half-width: {min_detectable_ratio(0.01):.4f}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
