//! Phase 2: hardware-accelerated bit and byte-vector primitives.
//!
//! Two families live here:
//!
//! - **Byte-vector search** ([`find_byte_16`], [`find_byte_8`]): the digit
//!   lookup inside linear branches and 1-byte leaves. On x86-64 this uses
//!   the SSE2 splat/compare/movemask idiom (SSE2 is baseline, no runtime
//!   detection needed); on AArch64 the NEON equivalent (baseline likewise);
//!   elsewhere a portable implementation. The portable versions are always
//!   compiled and exported from [`portable`] so parity tests can assert
//!   bit-exact agreement with the accelerated paths.
//! - **[`Bitmap256`]**: the 256-bit membership set used by bitmap branches
//!   and bitmap leaves, with popcount-based rank (bit → packed-array slot)
//!   and select (rank → bit, the `ByCount` primitive), plus ordered
//!   neighbor scans for `First`/`Next`/`Last`/`Prev`.
//!
//! Scalar popcount/tzcnt/lzcnt need no wrappers: `u64::count_ones`,
//! `trailing_zeros`, and `leading_zeros` already lower to the hardware
//! instructions on every target this crate supports.

/// Portable reference implementations, always compiled.
///
/// These are the semantics; the accelerated paths in this module must agree
/// bit-for-bit (enforced by parity tests, see `docs/TESTING.md`).
pub mod portable {
    /// Finds the first index `i < len` with `hay[i] == needle`.
    #[must_use]
    pub const fn find_byte_16(hay: &[u8; 16], len: usize, needle: u8) -> Option<usize> {
        let len = if len < 16 { len } else { 16 };
        let mut i = 0;
        while i < len {
            if hay[i] == needle {
                return Some(i);
            }
            i += 1;
        }
        None
    }

    /// Finds the first index `i < len` with `hay[i] == needle`.
    #[must_use]
    pub const fn find_byte_8(hay: &[u8; 8], len: usize, needle: u8) -> Option<usize> {
        let len = if len < 8 { len } else { 8 };
        let mut i = 0;
        while i < len {
            if hay[i] == needle {
                return Some(i);
            }
            i += 1;
        }
        None
    }
}

/// Finds the first index `i < len` with `hay[i] == needle` in a 16-byte
/// buffer (linear-branch digit arrays; `len` is clamped to 16).
#[inline]
#[must_use]
pub fn find_byte_16(hay: &[u8; 16], len: usize, needle: u8) -> Option<usize> {
    #[cfg(target_arch = "x86_64")]
    {
        find_byte_16_sse2(hay, len, needle)
    }
    #[cfg(target_arch = "aarch64")]
    {
        find_byte_16_neon(hay, len, needle)
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        portable::find_byte_16(hay, len, needle)
    }
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn find_byte_16_sse2(hay: &[u8; 16], len: usize, needle: u8) -> Option<usize> {
    use core::arch::x86_64::{_mm_cmpeq_epi8, _mm_loadu_si128, _mm_movemask_epi8, _mm_set1_epi8};
    let len = len.min(16);
    // SAFETY: SSE2 is part of the x86-64 baseline, and the unaligned load
    // reads exactly 16 bytes from a `&[u8; 16]`.
    let eq = unsafe {
        let t = _mm_set1_epi8(needle as i8);
        let h = _mm_loadu_si128(hay.as_ptr().cast());
        _mm_movemask_epi8(_mm_cmpeq_epi8(t, h)) as u32
    };
    let mask = eq & ((1u32 << len) - 1);
    if mask == 0 {
        None
    } else {
        Some(mask.trailing_zeros() as usize)
    }
}

#[cfg(target_arch = "aarch64")]
#[inline]
fn find_byte_16_neon(hay: &[u8; 16], len: usize, needle: u8) -> Option<usize> {
    use core::arch::aarch64::{
        vceqq_u8, vdupq_n_u8, vget_lane_u64, vld1q_u8, vreinterpret_u64_u8, vreinterpretq_u16_u8,
        vshrn_n_u16,
    };
    let len = len.min(16);
    // SAFETY: NEON is part of the AArch64 baseline, and the load reads
    // exactly 16 bytes from a `&[u8; 16]`.
    let mask = unsafe {
        let t = vdupq_n_u8(needle);
        let h = vld1q_u8(hay.as_ptr());
        // Narrow the 128-bit compare result to a 64-bit mask, 4 bits per
        // byte lane (the standard AArch64 movemask substitute).
        let narrowed = vshrn_n_u16::<4>(vreinterpretq_u16_u8(vceqq_u8(t, h)));
        vget_lane_u64::<0>(vreinterpret_u64_u8(narrowed))
    };
    let mask = if len == 16 {
        mask
    } else {
        mask & ((1u64 << (len * 4)) - 1)
    };
    if mask == 0 {
        None
    } else {
        Some((mask.trailing_zeros() / 4) as usize)
    }
}

/// Finds the first index `i < len` with `hay[i] == needle` in an 8-byte
/// buffer (compact branch headers; `len` is clamped to 8).
///
/// Uses a branch-free SWAR zero-byte scan on one 64-bit word — as fast as
/// SIMD at this width and portable everywhere, so it needs no parity twin.
#[inline]
#[must_use]
pub const fn find_byte_8(hay: &[u8; 8], len: usize, needle: u8) -> Option<usize> {
    const LO: u64 = 0x0101_0101_0101_0101;
    const HI: u64 = 0x8080_8080_8080_8080;
    let len = if len < 8 { len } else { 8 };
    let x = u64::from_le_bytes(*hay) ^ (LO.wrapping_mul(needle as u64));
    // Standard zero-byte detector: the high bit of each byte lane is set
    // iff that lane of `x` is zero (i.e. matched the needle).
    let mut zeros = x.wrapping_sub(LO) & !x & HI;
    if len < 8 {
        zeros &= (1u64 << (len * 8)) - 1;
    }
    if zeros == 0 {
        None
    } else {
        Some((zeros.trailing_zeros() / 8) as usize)
    }
}

/// A 256-bit membership set over one decode byte (0x00..=0xFF): the core of
/// bitmap branches and bitmap leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Bitmap256 {
    words: [u64; 4],
}

impl Bitmap256 {
    /// The empty set.
    #[must_use]
    pub const fn new() -> Self {
        Self { words: [0; 4] }
    }

    /// The full set (all 256 members present).
    #[must_use]
    pub const fn full() -> Self {
        Self {
            words: [u64::MAX; 4],
        }
    }

    /// Membership test.
    #[inline]
    #[must_use]
    pub const fn test(&self, idx: u8) -> bool {
        self.words[(idx >> 6) as usize] & (1u64 << (idx & 63)) != 0
    }

    /// Inserts `idx`; returns `true` if it was newly inserted.
    #[inline]
    pub const fn set(&mut self, idx: u8) -> bool {
        let w = (idx >> 6) as usize;
        let bit = 1u64 << (idx & 63);
        let was = self.words[w] & bit != 0;
        self.words[w] |= bit;
        !was
    }

    /// Removes `idx`; returns `true` if it was present.
    #[inline]
    pub const fn clear(&mut self, idx: u8) -> bool {
        let w = (idx >> 6) as usize;
        let bit = 1u64 << (idx & 63);
        let was = self.words[w] & bit != 0;
        self.words[w] &= !bit;
        was
    }

    /// Number of members present.
    #[inline]
    #[must_use]
    pub const fn count(&self) -> u32 {
        self.words[0].count_ones()
            + self.words[1].count_ones()
            + self.words[2].count_ones()
            + self.words[3].count_ones()
    }

    /// True if no members are present.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        (self.words[0] | self.words[1] | self.words[2] | self.words[3]) == 0
    }

    /// Number of members strictly below `idx` — the packed-array slot of
    /// `idx` in a bitmap branch/leaf.
    #[inline]
    #[must_use]
    pub const fn rank(&self, idx: u8) -> u32 {
        let w = (idx >> 6) as usize;
        let mut r = 0;
        let mut i = 0;
        while i < w {
            r += self.words[i].count_ones();
            i += 1;
        }
        let below = (1u64 << (idx & 63)) - 1;
        r + (self.words[w] & below).count_ones()
    }

    /// The member with `n` members below it (0-based rank → bit), if any —
    /// the `ByCount` primitive. Inverse of [`Self::rank`] for present bits.
    #[must_use]
    pub const fn select(&self, n: u32) -> Option<u8> {
        let mut remaining = n;
        let mut w = 0;
        while w < 4 {
            let pop = self.words[w].count_ones();
            if remaining < pop {
                // Select the `remaining`-th set bit inside this word.
                let mut word = self.words[w];
                let mut k = remaining;
                while k > 0 {
                    word &= word - 1; // drop lowest set bit
                    k -= 1;
                }
                return Some(((w as u32 * 64) + word.trailing_zeros()) as u8);
            }
            remaining -= pop;
            w += 1;
        }
        None
    }

    /// Smallest member `>= from`, if any (`First`/`Next` navigation).
    #[must_use]
    pub const fn next_set(&self, from: u8) -> Option<u8> {
        let mut w = (from >> 6) as usize;
        let mut word = self.words[w] & (u64::MAX << (from & 63));
        loop {
            if word != 0 {
                return Some(((w as u32 * 64) + word.trailing_zeros()) as u8);
            }
            w += 1;
            if w == 4 {
                return None;
            }
            word = self.words[w];
        }
    }

    /// Largest member `<= from`, if any (`Last`/`Prev` navigation).
    #[must_use]
    pub const fn prev_set(&self, from: u8) -> Option<u8> {
        let mut w = (from >> 6) as usize;
        let keep = 63 - (from & 63);
        let mut word = (self.words[w] << keep) >> keep;
        loop {
            if word != 0 {
                return Some(((w as u32 * 64) + 63 - word.leading_zeros()) as u8);
            }
            if w == 0 {
                return None;
            }
            w -= 1;
            word = self.words[w];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic xorshift so parity sweeps are reproducible without an
    /// RNG dependency.
    struct XorShift(u64);
    impl XorShift {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
    }

    #[test]
    fn find_byte_16_basics() {
        let hay = *b"\x00\x11\x22\x33\x44\x55\x66\x77\x88\x99\xAA\xBB\xCC\xDD\xEE\xFF";
        assert_eq!(find_byte_16(&hay, 16, 0x00), Some(0));
        assert_eq!(find_byte_16(&hay, 16, 0xFF), Some(15));
        assert_eq!(find_byte_16(&hay, 16, 0x77), Some(7));
        assert_eq!(find_byte_16(&hay, 7, 0x77), None); // just past len
        assert_eq!(find_byte_16(&hay, 8, 0x77), Some(7)); // last in len
        assert_eq!(find_byte_16(&hay, 0, 0x00), None); // empty
        assert_eq!(find_byte_16(&hay, 16, 0x12), None); // absent
        // Duplicates: first match wins.
        let dup = [0xABu8; 16];
        assert_eq!(find_byte_16(&dup, 16, 0xAB), Some(0));
        // len beyond capacity clamps.
        assert_eq!(find_byte_16(&hay, 999, 0xFF), Some(15));
    }

    #[test]
    fn find_byte_8_basics() {
        let hay = *b"\x10\x20\x30\x40\x50\x60\x70\x80";
        assert_eq!(find_byte_8(&hay, 8, 0x10), Some(0));
        assert_eq!(find_byte_8(&hay, 8, 0x80), Some(7));
        assert_eq!(find_byte_8(&hay, 7, 0x80), None);
        assert_eq!(find_byte_8(&hay, 0, 0x10), None);
        assert_eq!(find_byte_8(&hay, 8, 0x00), None);
        let zeros = [0u8; 8];
        assert_eq!(find_byte_8(&zeros, 8, 0x00), Some(0));
        assert_eq!(find_byte_8(&zeros, 3, 0x00), Some(0));
        assert_eq!(find_byte_8(&hay, 999, 0x80), Some(7));
    }

    #[test]
    fn find_byte_parity_with_portable() {
        // Exhaustive over position × len × a set of needles, then a random
        // sweep. The accelerated and portable paths must agree bit-exactly.
        for pos in 0..16usize {
            for len in 0..=16usize {
                for needle in [0x00u8, 0x01, 0x7F, 0x80, 0xFE, 0xFF] {
                    let mut hay = [needle.wrapping_add(1); 16];
                    hay[pos] = needle;
                    assert_eq!(
                        find_byte_16(&hay, len, needle),
                        portable::find_byte_16(&hay, len, needle),
                        "pos={pos} len={len} needle={needle:#04x}"
                    );
                }
            }
        }
        let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
        for _ in 0..20_000 {
            let mut hay16 = [0u8; 16];
            for b in &mut hay16 {
                *b = rng.next() as u8;
            }
            let needle = rng.next() as u8;
            let len = (rng.next() % 17) as usize;
            assert_eq!(
                find_byte_16(&hay16, len, needle),
                portable::find_byte_16(&hay16, len, needle)
            );
            let hay8: [u8; 8] = hay16[..8].try_into().unwrap();
            let len8 = (rng.next() % 9) as usize;
            assert_eq!(
                find_byte_8(&hay8, len8, needle),
                portable::find_byte_8(&hay8, len8, needle)
            );
        }
    }

    #[test]
    fn bitmap_set_test_clear() {
        let mut bm = Bitmap256::new();
        assert!(bm.is_empty());
        for idx in [0u8, 63, 64, 127, 128, 191, 192, 255] {
            assert!(!bm.test(idx));
            assert!(bm.set(idx));
            assert!(!bm.set(idx), "second set of {idx} must report present");
            assert!(bm.test(idx));
        }
        assert_eq!(bm.count(), 8);
        assert!(bm.clear(64));
        assert!(!bm.clear(64), "second clear of 64 must report absent");
        assert!(!bm.test(64));
        assert_eq!(bm.count(), 7);
        assert!(!bm.is_empty());
        assert_eq!(Bitmap256::full().count(), 256);
    }

    #[test]
    fn bitmap_rank_select_roundtrip() {
        let members = [0u8, 1, 63, 64, 100, 128, 200, 254, 255];
        let mut bm = Bitmap256::new();
        for &m in &members {
            bm.set(m);
        }
        for (expected_rank, &m) in members.iter().enumerate() {
            assert_eq!(bm.rank(m), expected_rank as u32, "rank({m})");
            assert_eq!(bm.select(expected_rank as u32), Some(m));
        }
        assert_eq!(bm.select(members.len() as u32), None);
        assert_eq!(bm.rank(0), 0);
        assert_eq!(Bitmap256::new().rank(255), 0);
        assert_eq!(Bitmap256::full().rank(255), 255);
        assert_eq!(Bitmap256::full().select(255), Some(255));
        // rank counts strictly-below regardless of membership of idx itself.
        assert_eq!(bm.rank(65), 4);
    }

    #[test]
    fn bitmap_rank_matches_naive_random() {
        let mut rng = XorShift(0xDEAD_BEEF_CAFE_F00D);
        for _ in 0..200 {
            let mut bm = Bitmap256::new();
            let mut naive = [false; 256];
            for _ in 0..(rng.next() % 300) {
                let idx = rng.next() as u8;
                bm.set(idx);
                naive[idx as usize] = true;
            }
            let mut below = 0u32;
            let mut expected_next: [Option<u8>; 256] = [None; 256];
            for (idx, &present) in naive.iter().enumerate() {
                assert_eq!(bm.rank(idx as u8), below, "rank({idx})");
                if present {
                    assert_eq!(bm.select(below), Some(idx as u8));
                    below += 1;
                }
            }
            // next_set / prev_set against the naive model.
            let mut next: Option<u8> = None;
            let pairs = expected_next.iter_mut().zip(naive.iter());
            for (idx, (slot, &present)) in pairs.enumerate().rev() {
                if present {
                    next = Some(idx as u8);
                }
                *slot = next;
            }
            let mut prev: Option<u8> = None;
            for (idx, (&present, &exp_next)) in naive.iter().zip(expected_next.iter()).enumerate() {
                if present {
                    prev = Some(idx as u8);
                }
                assert_eq!(bm.prev_set(idx as u8), prev, "prev_set({idx})");
                assert_eq!(bm.next_set(idx as u8), exp_next, "next_set({idx})");
            }
        }
    }

    #[test]
    fn bitmap_navigation_edges() {
        let mut bm = Bitmap256::new();
        assert_eq!(bm.next_set(0), None);
        assert_eq!(bm.prev_set(255), None);
        bm.set(0);
        bm.set(255);
        assert_eq!(bm.next_set(0), Some(0));
        assert_eq!(bm.next_set(1), Some(255));
        assert_eq!(bm.next_set(255), Some(255));
        assert_eq!(bm.prev_set(255), Some(255));
        assert_eq!(bm.prev_set(254), Some(0));
        assert_eq!(bm.prev_set(0), Some(0));
    }
}
