#!/usr/bin/env python3
"""scripts/shrink_rss_bounds.py — Mathematical bounds and derivations for slab region carving
and shrink_to_fit RSS recovery (Issue #1108).

Enforces AGENTS.md §8.8 commit 1 (Math-first validation in committed Python
with reference-pinned unit tests) and AGENTS.md §8.8 (Commit 1 Step 0).

Primary sources:
  - Linux Programmer's Manual, madvise(2): MADV_DONTNEED behaviour on private anonymous memory.
  - Linux Programmer's Manual, mallopt(3):
      "Nowadays, glibc uses a dynamic mmap threshold by default. The initial value of the
       threshold is 128*1024, but when blocks larger than the current threshold and less
       than or equal to DEFAULT_MMAP_THRESHOLD_MAX are freed, the threshold is adjusted
       upward to the size of the freed block."
  - The GNU C Library Reference Manual:
      * §3.2.2.4 "Aligned Memory Blocks" (posix_memalign chunk splitting and free chunk returns)
      * §3.2.2.8 "Malloc Tunable Parameters" (dynamic M_MMAP_THRESHOLD)
  - Linux Programmer's Manual, mmap(2) & posix_memalign(3).
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

    total_offsets = os_page_size // chunk_align
    if chunk_size == os_page_size:
        favourable = 1
    else:
        slack = chunk_size - os_page_size
        favourable = min(total_offsets, 1 + slack // chunk_align)
    return float(favourable) / float(total_offsets)


def glibc_mmap_threshold_padding(
    region_size: int,
    mmap_threshold: int = 131072,  # 128 KiB glibc default
    page_size: int = 4096,
    malloc_alignment: int = 16,
) -> int:
    """Computes maximum glibc alignment padding for an allocation of `region_size` with `page_size` alignment.

    Primary Source:
      The GNU C Library Reference Manual §3.2.2.8 & mallopt(3):
      Allocations >= M_MMAP_THRESHOLD are satisfied via mmap(2) rather than brk(2).
      mmap(2) maps anonymous memory page-aligned (4096-aligned) by the OS kernel,
      resulting in EXACTLY 0 bytes of alignment padding.

      However, mallopt(3) notes that glibc dynamically adjusts M_MMAP_THRESHOLD upward
      when large blocks are freed. To completely eliminate dynamic thresholding and
      guarantee zero alignment padding across all shrink/grow cycles, Linux directly
      invokes mmap(MAP_PRIVATE | MAP_ANONYMOUS).
    """
    if region_size <= 0 or mmap_threshold <= 0 or page_size <= 0 or malloc_alignment <= 0:
        raise ValueError("parameters must be strictly positive")
    if region_size >= mmap_threshold:
        return 0
    return page_size - malloc_alignment


def region_alignment_waste_bound(
    region_pages: int,
    page_size: int = 4096,
    mmap_threshold: int = 131072,
) -> float:
    """Computes upper bound on amortized alignment padding overhead per page.

    For region_size >= mmap_threshold (e.g. 128 KiB = 32 pages): 0.0 B/page.
    For region_size < mmap_threshold (e.g. 64 KiB = 16 pages): up to 4080 / region_pages B/page.
    """
    if region_pages <= 0 or page_size <= 0:
        raise ValueError("invalid region or page parameters")
    region_size = region_pages * page_size
    pad = glibc_mmap_threshold_padding(region_size, mmap_threshold, page_size)
    return float(pad) / float(region_pages)


def region_header_overhead_ratio(
    region_pages: int = 32,
    page_size: int = 4096,
    header_size: int = 64,
) -> float:
    """Calculates memory overhead ratio of the intrusive per-region metadata header.

    The Region header (next, next_released, first_released, region_base, pages_carved,
    released_mask, classes[32]) occupies header_size (<= 64 B) at offset 0 of Page 0.
    """
    if region_pages <= 0 or page_size <= 0 or header_size <= 0:
        raise ValueError("invalid region parameters")
    total_region_bytes = region_pages * page_size
    return float(header_size) / float(total_region_bytes)


def region_tail_overhead_bound(
    region_pages: int = 32, page_size: int = 4096
) -> int:
    """Maximum uncarved tail memory in the active region (bytes).

    If an active region has carved at least 1 page, at most (region_pages - 1)
    pages remain uncarved in that region.
    """
    if region_pages <= 0 or page_size <= 0:
        raise ValueError("invalid region or page parameters")
    return (region_pages - 1) * page_size


def uncarved_tail_demand_paging_rss(
    uncarved_tail_bytes: int,
    is_mmap_backed: bool = True,
) -> int:
    """Computes resident physical RSS consumed by uncarved tail pages.

    Primary Source: Linux man 2 mmap / vm_area_struct demand paging:
    Anonymous mmap pages are demand-zero-paged. They are mapped virtually but
    consume ZERO physical memory (RSS = 0) until touched/written.
    Since uncarved tail pages are never written to, their RSS is 0.
    """
    if uncarved_tail_bytes < 0:
        raise ValueError("tail bytes must be non-negative")
    if is_mmap_backed:
        return 0
    return uncarved_tail_bytes


def pre_shrink_rss_inflation_ratio(
    padding_bytes: int,
    uncarved_rss_bytes: int,
    total_tree_resident_bytes: int,
) -> float:
    """Calculates upper bound on pre-shrink RSS inflation ratio.

    Gate G1 requires: no shape's pre-shrink RSS worse than main by more than 1% (0.01).
    """
    if padding_bytes < 0 or uncarved_rss_bytes < 0 or total_tree_resident_bytes <= 0:
        raise ValueError("invalid byte counts")
    total_overhead = padding_bytes + uncarved_rss_bytes
    return float(total_overhead) / float(total_tree_resident_bytes)


def min_slab_release_ratio_for_g1(
    target_fraction: float = 0.75,
    unaligned_coalesce_ratio: float = 0.343387,  # modelling assumption from census baseline
    slab_return_ratio: float = 1.0,
) -> float:
    """Calculates minimum fraction of released mem_held that must come from slab pages
    in order for the overall RSS drop to clear Gate G1 (>= target_fraction, 75%).

    Falsifiable condition:
      F = S_slab * slab_return_ratio + (1 - S_slab) * unaligned_coalesce_ratio >= target_fraction
      S_slab >= (target_fraction - unaligned_coalesce_ratio) / (slab_return_ratio - unaligned_coalesce_ratio)
    """
    if not (0.0 <= target_fraction <= 1.0) or not (0.0 <= unaligned_coalesce_ratio < slab_return_ratio <= 1.0):
        raise ValueError("invalid ratio parameters")
    return (target_fraction - unaligned_coalesce_ratio) / (slab_return_ratio - unaligned_coalesce_ratio)


def expected_rss_drop(
    released_held: float,
    slab_released_ratio: float = 0.90,
    unaligned_coalesce_ratio: float = 0.343387,  # modelling assumption from baseline
    is_region_aligned: bool = True,
) -> float:
    """Computes expected RSS drop following shrink_to_fit().

    If is_region_aligned=True, fully free slab pages are 4096-aligned and returned
    via madvise(MADV_DONTNEED), guaranteeing 1.0 (100%) physical RSS return for slab pages.
    Any non-slab released memory recovers RSS at the unaligned coalescence ratio (modelling assumption).
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

        # 128 KiB region with 4096 alignment:
        p_region = page_containment_probability(131072, 4096, 4096)
        self.assertEqual(p_region, 1.0)

    def test_glibc_mmap_threshold_and_padding_waste(self) -> None:
        # Region size 128 KiB (32 pages) >= M_MMAP_THRESHOLD (128 KiB):
        # mmap-served: 0 padding waste!
        pad_128k = glibc_mmap_threshold_padding(131072, mmap_threshold=131072)
        self.assertEqual(pad_128k, 0)
        waste_128k = region_alignment_waste_bound(32, 4096, mmap_threshold=131072)
        self.assertEqual(waste_128k, 0.0)

        # Region size 64 KiB (16 pages) < M_MMAP_THRESHOLD (128 KiB):
        # brk-served: up to 4080 B padding waste, 255 B/page (6.225%).
        pad_64k = glibc_mmap_threshold_padding(65536, mmap_threshold=131072)
        self.assertEqual(pad_64k, 4080)
        waste_64k = region_alignment_waste_bound(16, 4096, mmap_threshold=131072)
        self.assertAlmostEqual(waste_64k, 255.0, places=1)

    def test_region_header_overhead(self) -> None:
        # Region header is 64 B at Page 0 of a 128 KiB region (32 pages = 131,072 B).
        # Overhead = 64 / 131072 = 0.00048828... (< 0.05%).
        overhead = region_header_overhead_ratio(32, 4096, 64)
        self.assertLess(overhead, 0.0005)

    def test_uncarved_tail_demand_paging(self) -> None:
        # 32-page (128 KiB) region leaves at most 31 uncarved pages = 126,976 B.
        tail_bytes = region_tail_overhead_bound(32, 4096)
        self.assertEqual(tail_bytes, 126976)

        # Under anonymous mmap, uncarved pages are demand-zero-paged and untouched: RSS is 0.
        tail_rss = uncarved_tail_demand_paging_rss(tail_bytes, is_mmap_backed=True)
        self.assertEqual(tail_rss, 0)

        # Pre-shrink RSS inflation for 128 KiB region (0 pad, 0 uncarved RSS):
        # inflation ratio is 0.0, strictly clearing Gate G1's 1.0% limit.
        inflation = pre_shrink_rss_inflation_ratio(0, 0, 91_000_000)
        self.assertEqual(inflation, 0.0)

    def test_min_slab_release_ratio_for_gate_g1(self) -> None:
        # Falsifiable condition: using baseline coalesce ratio from results/allocator_overhead_a4b03ad5.txt:
        # unaligned_coalesce_ratio = 1.48 / 4.31 = 0.343387... (modelling assumption)
        # Target return fraction = 0.75 (Gate G1 floor)
        # S_slab >= (0.75 - 0.343387) / (1.0 - 0.343387) = 0.61927... (61.93%).
        min_slab = min_slab_release_ratio_for_g1(0.75, 0.343387, 1.0)
        self.assertAlmostEqual(min_slab, 0.6193, places=3)
        self.assertLess(min_slab, 0.65)

    def test_commit_a4b03ad5_random_1e7_empirical_pins(self) -> None:
        # Ground truth measured values from results/allocator_overhead_a4b03ad5.txt:
        # shape: random, N = 10,000,000 keys
        # mem_held = 32.70 B/key, held/shr = 28.39 B/key -> released_held = 4.31 B/key
        # trimmed RSS = 33.72 B/key, RSS/shr = 32.24 B/key -> delta_rss = 1.48 B/key
        released_held = 32.70 - 28.39  # 4.31 B/key
        self.assertAlmostEqual(released_held, 4.31, places=2)

        actual_old_drop = 33.72 - 32.24  # 1.48 B/key
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

        # Falsifiable condition check (S_slab >= S_min = 61.93%):
        # Case A: Boundary condition S_slab = 61.93%:
        drop_bound = expected_rss_drop(
            released_held=4.31,
            slab_released_ratio=0.6193,
            unaligned_coalesce_ratio=0.343387,
            is_region_aligned=True,
        )
        self.assertAlmostEqual(drop_bound / 4.31, 0.75, places=3)  # exactly 75%

        # Case B: Projected / assumed condition S_slab = 90% (projected, assumed):
        drop_projected_90 = expected_rss_drop(
            released_held=4.31,
            slab_released_ratio=0.90,
            unaligned_coalesce_ratio=0.343387,
            is_region_aligned=True,
        )
        self.assertAlmostEqual(drop_projected_90, 4.0270, places=3)  # 4.03 B/key drop
        self.assertGreater(drop_projected_90 / 4.31, 0.93)  # 93.4% return fraction

        # Both clear Gate G1 ceiling:
        self.assertLess(33.72 - drop_bound, g1_ceiling + 1e-4)
        self.assertLess(33.72 - drop_projected_90, g1_ceiling)

    def test_invalid_parameter_rejections(self) -> None:
        with self.assertRaises(ValueError):
            page_containment_probability(-4096, 64)
        with self.assertRaises(ValueError):
            page_containment_probability(4096, 63)
        with self.assertRaises(ValueError):
            glibc_mmap_threshold_padding(0)
        with self.assertRaises(ValueError):
            region_alignment_waste_bound(0, 4096)
        with self.assertRaises(ValueError):
            region_header_overhead_ratio(0, 4096)
        with self.assertRaises(ValueError):
            expected_rss_drop(-1.0)
        with self.assertRaises(ValueError):
            expected_rss_drop(1.0, slab_released_ratio=1.5)
        with self.assertRaises(ValueError):
            min_slab_release_ratio_for_g1(1.5)
        with self.assertRaises(ValueError):
            gate_g1_rss_ceiling(-1.0, 1.0)


if __name__ == "__main__":
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(TestShrinkRssBounds)
    runner = unittest.TextTestRunner(verbosity=2)
    result = runner.run(suite)
    if not result.wasSuccessful():
        sys.exit(1)
    print("All shrink RSS mathematical bounds verified and pinned.")
