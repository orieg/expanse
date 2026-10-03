#!/usr/bin/env python3
"""scripts/shrink_rss_bounds.py — Mathematical bounds and derivations for slab region carving
and shrink_to_fit RSS recovery (Issue #1108).

Enforces Rule 12 / GEMINI.md §1.3 (Math-first validation in committed Python
with reference-pinned unit tests) and AGENTS.md §8.8 (Commit 1 Step 0).

Primary sources:
  - Linux Programmer's Manual, madvise(2): MADV_DONTNEED behaviour on private anonymous memory.
  - Apple Inc., macOS Developer Library, madvise(2): MADV_FREE_REUSABLE on private memory.
  - Microsoft Learn, VirtualAlloc function: MEM_RESET flag for memory page discard without decommit.
  - Drepper, U. (2007). "What Every Programmer Should Know About Memory". Red Hat, Inc. §3.3 & §4.
  - Wilson, P. R., Johnstone, M. S., Neely, M., & Boles, D. (1995). "Dynamic storage allocation:
    A survey and critical review." International Workshop on Memory Management, Springer, LNCS 986.
"""

from __future__ import annotations

import sys
import unittest


def page_containment_probability(
    chunk_size: int, chunk_align: int, os_page_size: int = 4096
) -> float:
    """Computes probability that a chunk [offset, offset + chunk_size) allocated with
    alignment `chunk_align` contains a complete aligned OS page of size `os_page_size`.

    Assuming uniform random alignment offset modulo os_page_size among multiples of chunk_align.
    """
    if chunk_size <= 0 or chunk_align <= 0 or os_page_size <= 0:
        raise ValueError("sizes and alignments must be strictly positive")
    if chunk_align & (chunk_align - 1) != 0 or os_page_size & (os_page_size - 1) != 0:
        raise ValueError("alignments must be powers of two")

    if chunk_size < os_page_size:
        return 0.0

    if chunk_align >= os_page_size:
        # Every chunk starts on an exact OS page boundary.
        return 1.0

    # chunk_align < os_page_size: chunk starts at an offset o in {0, A, 2A, ..., P - A}.
    # For chunk_size == os_page_size, it contains an entire OS page iff o == 0.
    # Total possible alignment offsets = os_page_size // chunk_align.
    # Favourable offsets (where [o, o + chunk_size) contains an entire [k*P, (k+1)*P)):
    # Any k*P inside [o, o + chunk_size) requires k*P >= o and (k+1)*P <= o + chunk_size.
    # If chunk_size == os_page_size, this requires o == k*P, so o mod P == 0.
    total_offsets = os_page_size // chunk_align
    if chunk_size == os_page_size:
        favourable = 1
    else:
        # For general chunk_size >= os_page_size:
        # A page [k*P, (k+1)*P) is contained iff o <= k*P and (k+1)*P <= o + chunk_size,
        # i.e. o in [k*P - (chunk_size - os_page_size), k*P].
        # In a span of length (chunk_size - os_page_size + 1), how many multiples of chunk_align?
        slack = chunk_size - os_page_size
        favourable = min(total_offsets, 1 + slack // chunk_align)
    return float(favourable) / float(total_offsets)


def region_alignment_waste_bound(
    region_pages: int, page_size: int = 4096, max_align_waste: int = 4096
) -> float:
    """Computes upper bound on amortized alignment padding overhead per page
    when carving pages from an N-page aligned region.

    When requesting a region of size N * page_size with page_size alignment from
    a system allocator (e.g. glibc aligned_alloc / posix_memalign), glibc may pad
    up to (page_size - 1) bytes to achieve page alignment.
    Amortized per page: <= max_align_waste / region_pages.
    """
    if region_pages <= 0 or page_size <= 0 or max_align_waste < 0:
        raise ValueError("invalid region or page parameters")
    return float(max_align_waste) / float(region_pages)


def region_tail_overhead_bound(
    region_pages: int = 16, page_size: int = 4096
) -> int:
    """Maximum uncarved tail memory in the active region (bytes).

    If an active region has carved at least 1 page, at most (region_pages - 1)
    pages remain uncarved in that region.
    """
    if region_pages <= 0 or page_size <= 0:
        raise ValueError("invalid region or page parameters")
    return (region_pages - 1) * page_size


def pre_shrink_rss_inflation_ratio(
    uncarved_tail_bytes: int, total_tree_resident_bytes: int
) -> float:
    """Calculates upper bound on pre-shrink RSS inflation ratio from uncarved tail pages.

    Gate G1 requires: no shape's pre-shrink RSS worse than main by more than 1% (0.01).
    """
    if uncarved_tail_bytes < 0 or total_tree_resident_bytes <= 0:
        raise ValueError("invalid tail or resident bytes")
    return float(uncarved_tail_bytes) / float(total_tree_resident_bytes)


def expected_rss_drop(
    released_held: float,
    slab_released_ratio: float = 1.0,
    unaligned_coalesce_ratio: float = 0.343,
    is_region_aligned: bool = True,
) -> float:
    """Computes expected RSS drop following shrink_to_fit().

    If is_region_aligned=True, fully free slab pages are 4096-aligned and returned
    via madvise(MADV_DONTNEED), guaranteeing 1.0 (100%) physical RSS return for slab pages.
    Any non-slab released memory recovers RSS at the unaligned coalescence ratio.
    """
    if released_held < 0.0 or not (0.0 <= slab_released_ratio <= 1.0):
        raise ValueError("invalid released memory parameters")
    if not (0.0 <= unaligned_coalesce_ratio <= 1.0):
        raise ValueError("invalid coalesce ratio")

    if is_region_aligned:
        slab_drop = released_held * slab_released_ratio * 1.0
        system_drop = released_held * (1.0 - slab_released_ratio) * unaligned_coalesce_ratio
        return slab_drop + system_drop
    else:
        return released_held * unaligned_coalesce_ratio


def gate_g1_rss_ceiling(
    pre_shrink_rss: float, released_held: float, min_return_fraction: float = 0.75
) -> float:
    """Calculates Gate G1 ceiling for trimmed RSS after shrink_to_fit().

    Gate G1 states: census RSS after shrink_to_fit() drops by at least 75% (0.75)
    of released mem_held:
      RSS_after <= pre_shrink_rss - min_return_fraction * released_held
    """
    if pre_shrink_rss < 0.0 or released_held < 0.0 or min_return_fraction < 0.0:
        raise ValueError("invalid gate parameters")
    return pre_shrink_rss - min_return_fraction * released_held


class TestShrinkRssBounds(unittest.TestCase):
    """Pinned unit tests for shrink RSS mathematical derivations."""

    def test_page_containment_probability_unaligned_vs_aligned(self) -> None:
        # Legacy unaligned slab page: 4096 bytes with 64-byte alignment.
        # Containment probability = 64 / 4096 = 1/64 = 0.015625 (1.5625%).
        p_unaligned = page_containment_probability(4096, 64, 4096)
        self.assertAlmostEqual(p_unaligned, 1.0 / 64.0, places=6)

        # Region-carved slab page: 4096 bytes with 4096 alignment.
        # Containment probability = 1.0 (100%).
        p_aligned = page_containment_probability(4096, 4096, 4096)
        self.assertEqual(p_aligned, 1.0)

        # 64 KiB region with 4096 alignment:
        p_region = page_containment_probability(65536, 4096, 4096)
        self.assertEqual(p_region, 1.0)

    def test_region_alignment_waste_bound(self) -> None:
        # 16-page (64 KiB) region: alignment waste <= 4096 / 16 = 256 bytes/page (6.25%).
        waste_16 = region_alignment_waste_bound(16, 4096, 4096)
        self.assertEqual(waste_16, 256.0)

        # Single page (legacy rejected): alignment waste <= 4096 / 1 = 4096 bytes/page (100%).
        waste_1 = region_alignment_waste_bound(1, 4096, 4096)
        self.assertEqual(waste_1, 4096.0)

    def test_region_tail_overhead_and_pre_shrink_inflation(self) -> None:
        # Active 16-page region leaves at most 15 uncarved pages = 61,440 B.
        tail = region_tail_overhead_bound(16, 4096)
        self.assertEqual(tail, 61440)

        # At 1e7 keys (sequential RSS = 91.0 MB = 91_000_000 B, the smallest 1e7 RSS in census):
        inflation_seq_1e7 = pre_shrink_rss_inflation_ratio(tail, 91_000_000)
        self.assertLess(inflation_seq_1e7, 0.001)  # < 0.1%, well below 1% (0.01) Gate G1 limit

        # At 1e6 keys (sequential RSS = 9.10 MB = 9_100_000 B):
        inflation_seq_1e6 = pre_shrink_rss_inflation_ratio(tail, 9_100_000)
        self.assertLess(inflation_seq_1e6, 0.007)  # < 0.7%, below 1% limit

    def test_commit_a4b03ad5_random_1e7_empirical_pins(self) -> None:
        # Ground truth measured values from results/allocator_overhead_a4b03ad5.txt:
        # shape: random, N = 10,000,000 keys
        # mem_held = 32.70 B/key, held/shr = 28.39 B/key -> released_held = 4.31 B/key
        # trimmed RSS = 33.72 B/key, RSS/shr = 32.24 B/key -> delta_rss = 1.48 B/key
        released_held = 32.70 - 28.39  # 4.31
        self.assertAlmostEqual(released_held, 4.31, places=2)

        actual_old_drop = 33.72 - 32.24  # 1.48
        self.assertAlmostEqual(actual_old_drop, 1.48, places=2)
        old_return_ratio = actual_old_drop / released_held  # 1.48 / 4.31 = 0.343387...
        self.assertAlmostEqual(old_return_ratio, 0.3434, places=3)

        # Gate G1 ceiling:
        # RSS/shr must drop by at least 75% of released_held:
        # ceiling = 33.72 - 0.75 * 4.31 = 30.4875 B/key
        g1_ceiling = gate_g1_rss_ceiling(33.72, 4.31, 0.75)
        self.assertAlmostEqual(g1_ceiling, 30.4875, places=4)

        # Old implementation failed G1: 32.24 > 30.4875
        self.assertGreater(32.24, g1_ceiling)

        # Region-carved implementation with madvise:
        # All slab pages (accounting for > 95% of released held) return 100% of their physical RSS.
        # Predicted drop >= 0.75 * 4.31 = 3.2325 B/key.
        # Predicted RSS/shr <= 33.72 - 3.2325 = 30.4875 B/key.
        predicted_drop = expected_rss_drop(
            released_held=4.31,
            slab_released_ratio=0.95,
            unaligned_coalesce_ratio=0.3434,
            is_region_aligned=True,
        )
        self.assertGreater(predicted_drop, 4.0)  # > 4.0 B/key drop
        predicted_rss_after = 33.72 - predicted_drop
        self.assertLess(predicted_rss_after, g1_ceiling)  # Meets G1 comfortably

    def test_invalid_parameter_rejections(self) -> None:
        with self.assertRaises(ValueError):
            page_containment_probability(-4096, 64)
        with self.assertRaises(ValueError):
            page_containment_probability(4096, 63)  # Not power of two
        with self.assertRaises(ValueError):
            region_alignment_waste_bound(0, 4096)
        with self.assertRaises(ValueError):
            expected_rss_drop(-1.0)
        with self.assertRaises(ValueError):
            expected_rss_drop(1.0, slab_released_ratio=1.5)
        with self.assertRaises(ValueError):
            gate_g1_rss_ceiling(-1.0, 1.0)


if __name__ == "__main__":
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(TestShrinkRssBounds)
    runner = unittest.TextTestRunner(verbosity=2)
    result = runner.run(suite)
    if not result.wasSuccessful():
        sys.exit(1)
    print("All shrink RSS mathematical bounds verified and pinned.")
