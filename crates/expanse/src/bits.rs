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
//! **x86-64 caveat**: `popcnt` is not in the base x86-64 target
//! (`fxsr,sse,sse2`), so without `-C target-cpu=x86-64-v2` or
//! `-C target-feature=+popcnt` the calls below lower to a SWAR sequence,
//! not one instruction. On AArch64 `count_ones` lowers to the NEON
//! `CNT`+`ADDV` path (per-byte popcount plus horizontal add), not a scalar
//! instruction — base A64 has no scalar popcount (scalar `CNT` needs
//! FEAT_CSSC, Armv8.9+; see docs/HARDWARE.md §2.2). No target-cpu is set in
//! this workspace today.
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
    // Under Miri the portable path runs instead of the intrinsics (Miri's
    // core::arch coverage is incomplete); the parity tests prove the two
    // agree bit-for-bit, so this loses no checking power.
    #[cfg(all(target_arch = "x86_64", not(miri)))]
    {
        find_byte_16_sse2(hay, len, needle)
    }
    #[cfg(all(target_arch = "aarch64", not(miri)))]
    {
        find_byte_16_neon(hay, len, needle)
    }
    #[cfg(any(miri, not(any(target_arch = "x86_64", target_arch = "aarch64"))))]
    {
        portable::find_byte_16(hay, len, needle)
    }
}

#[cfg(all(target_arch = "x86_64", not(miri)))]
#[inline]
fn find_byte_16_sse2(hay: &[u8; 16], len: usize, needle: u8) -> Option<usize> {
    use core::arch::x86_64::{_mm_cmpeq_epi8, _mm_loadu_si128, _mm_movemask_epi8, _mm_set1_epi8};
    let len = len.min(16);
    // SAFETY: SSE2 is guaranteed present by the x86-64 psABI / Rust target
    // spec (base `x86_64-*` enables `+fxsr,+sse,+sse2`; the SysV/MS calling
    // conventions require SSE2), not by the CPU manuals — those present SSE2
    // as CPUID-detectable. See docs/HARDWARE.md §1.1. The unaligned load
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

#[cfg(all(target_arch = "aarch64", not(miri)))]
#[inline]
fn find_byte_16_neon(hay: &[u8; 16], len: usize, needle: u8) -> Option<usize> {
    use core::arch::aarch64::{
        vceqq_u8, vdupq_n_u8, vget_lane_u64, vld1q_u8, vreinterpret_u64_u8, vreinterpretq_u16_u8,
        vshrn_n_u16,
    };
    let len = len.min(16);
    // SAFETY: the unconditional NEON path is sound because Rust's `aarch64-*`
    // targets baseline `+neon,+fp-armv8` and Advanced SIMD is universally
    // present on OS-hosting AArch64 parts — FEAT_AdvSIMD is architecturally
    // OPTIONAL from Armv8.0 (Arm ARM §A2.2), not mandatory. See
    // docs/HARDWARE.md §2.1. The load reads exactly 16 bytes from a `&[u8; 16]`.
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
#[inline]
#[must_use]
pub fn find_byte_8(hay: &[u8; 8], len: usize, needle: u8) -> Option<usize> {
    let len = len.min(8);
    if len <= 3 {
        if len >= 1 && hay[0] == needle {
            return Some(0);
        }
        if len >= 2 && hay[1] == needle {
            return Some(1);
        }
        if len >= 3 && hay[2] == needle {
            return Some(2);
        }
        return None;
    }
    #[cfg(all(target_arch = "x86_64", not(miri)))]
    {
        use core::arch::x86_64::{
            _mm_cmpeq_epi8, _mm_loadl_epi64, _mm_movemask_epi8, _mm_set1_epi8,
        };
        // SAFETY: SSE2 is guaranteed by the x86-64 psABI / Rust target, not
        // the CPU manual (docs/HARDWARE.md §1.1); reads 8 bytes from hay.
        let eq = unsafe {
            let t = _mm_set1_epi8(needle as i8);
            let h = _mm_loadl_epi64(hay.as_ptr().cast());
            _mm_movemask_epi8(_mm_cmpeq_epi8(t, h)) as u32
        };
        let mask = eq & ((1u32 << len) - 1);
        if mask == 0 {
            None
        } else {
            Some(mask.trailing_zeros() as usize)
        }
    }
    #[cfg(all(target_arch = "aarch64", not(miri)))]
    {
        use core::arch::aarch64::{
            vceq_u8, vdup_n_u8, vget_lane_u64, vld1_u8, vreinterpret_u64_u8,
        };
        // SAFETY: NEON guaranteed by Rust's aarch64 `+neon` baseline —
        // FEAT_AdvSIMD is architecturally optional but universally present
        // (docs/HARDWARE.md §2.1); reads 8 bytes from hay.
        let mask = unsafe {
            let t = vdup_n_u8(needle);
            let h = vld1_u8(hay.as_ptr());
            let eq = vceq_u8(t, h);
            vget_lane_u64::<0>(vreinterpret_u64_u8(eq))
        };
        let active_mask = if len < 8 {
            mask & ((1u64 << (len * 8)) - 1)
        } else {
            mask
        };
        if active_mask == 0 {
            None
        } else {
            Some((active_mask.trailing_zeros() / 8) as usize)
        }
    }
    #[cfg(any(miri, not(any(target_arch = "x86_64", target_arch = "aarch64"))))]
    {
        for i in 0..len {
            if hay[i] == needle {
                return Some(i);
            }
        }
        None
    }
}

/// Finds the first index `i < len` where `hay[i] == needle` in a 16-byte buffer.
///
/// # Safety
///
/// `hay` must point to at least 16 readable bytes.
#[inline]
pub(crate) unsafe fn search_16_u8(hay: *const u8, len: usize, needle: u8) -> Option<usize> {
    #[cfg(all(target_arch = "x86_64", not(miri)))]
    {
        use core::arch::x86_64::{
            _mm_cmpeq_epi8, _mm_loadu_si128, _mm_movemask_epi8, _mm_set1_epi8,
        };
        // SAFETY: caller guarantees 16 readable bytes.
        unsafe {
            let t = _mm_set1_epi8(needle as i8);
            let h = _mm_loadu_si128(hay.cast());
            let mask = _mm_movemask_epi8(_mm_cmpeq_epi8(t, h)) as u32 & ((1u32 << len.min(16)) - 1);
            if mask == 0 {
                None
            } else {
                Some(mask.trailing_zeros() as usize)
            }
        }
    }
    #[cfg(all(target_arch = "aarch64", not(miri)))]
    {
        // SAFETY: caller guarantees 16 readable bytes.
        unsafe { find_byte_16_neon(&*(hay as *const [u8; 16]), len, needle) }
    }
    #[cfg(any(miri, not(any(target_arch = "x86_64", target_arch = "aarch64"))))]
    {
        for i in 0..len.min(16) {
            // SAFETY: i < 16 is within the 16 readable bytes.
            if unsafe { *hay.add(i) } == needle {
                return Some(i);
            }
        }
        None
    }
}

/// Finds the first index `i < len` where `hay[i] == needle` in an 8-byte buffer.
///
/// # Safety
///
/// `hay` must point to at least 8 readable bytes.
#[inline]
pub(crate) unsafe fn search_8_u8(hay: *const u8, len: usize, needle: u8) -> Option<usize> {
    // SAFETY: caller guarantees 8 readable bytes.
    unsafe { find_byte_8(&*(hay as *const [u8; 8]), len, needle) }
}

/// Finds the lower bound index `i <= len` where `hay[i] >= needle` in an 8-byte sorted buffer.
///
/// # Safety
///
/// `hay` must point to at least 8 readable bytes with sorted contents.
#[inline]
pub(crate) unsafe fn lower_bound_8_u8(hay: *const u8, len: usize, needle: u8) -> usize {
    #[cfg(all(target_arch = "x86_64", not(miri)))]
    {
        use core::arch::x86_64::{
            _mm_cmplt_epi8, _mm_cvtsi64_si128, _mm_movemask_epi8, _mm_set1_epi8, _mm_xor_si128,
        };
        // SAFETY: caller guarantees 8 readable bytes.
        unsafe {
            let bias = _mm_set1_epi8(0x80u8 as i8);
            let raw_64 = (hay as *const i64).read_unaligned();
            let v = _mm_cvtsi64_si128(raw_64);
            let v_signed = _mm_xor_si128(v, bias);
            let n_signed = _mm_set1_epi8((needle ^ 0x80) as i8);
            let lt = _mm_cmplt_epi8(v_signed, n_signed);
            let mask = _mm_movemask_epi8(lt) as u32 & ((1u32 << len.min(8)) - 1);
            mask.count_ones() as usize
        }
    }
    #[cfg(any(miri, not(target_arch = "x86_64")))]
    {
        for i in 0..len.min(8) {
            // SAFETY: i < 8 is within the 8 readable bytes.
            if unsafe { *hay.add(i) } >= needle {
                return i;
            }
        }
        len.min(8)
    }
}

/// Finds the lower bound index `i <= len` where `hay[i] >= needle` in a 16-byte sorted buffer.
///
/// # Safety
///
/// `hay` must point to at least 16 readable bytes with sorted contents.
#[inline]
pub(crate) unsafe fn lower_bound_16_u8(hay: *const u8, len: usize, needle: u8) -> usize {
    #[cfg(all(target_arch = "x86_64", not(miri)))]
    {
        use core::arch::x86_64::{
            _mm_cmplt_epi8, _mm_loadu_si128, _mm_movemask_epi8, _mm_set1_epi8, _mm_xor_si128,
        };
        // SAFETY: caller guarantees 16 readable bytes.
        unsafe {
            let bias = _mm_set1_epi8(0x80u8 as i8);
            let v = _mm_loadu_si128(hay.cast());
            let v_signed = _mm_xor_si128(v, bias);
            let n_signed = _mm_set1_epi8((needle ^ 0x80) as i8);
            let lt = _mm_cmplt_epi8(v_signed, n_signed);
            let mask = _mm_movemask_epi8(lt) as u32 & ((1u32 << len.min(16)) - 1);
            mask.count_ones() as usize
        }
    }
    #[cfg(any(miri, not(target_arch = "x86_64")))]
    {
        for i in 0..len.min(16) {
            // SAFETY: i < 16 is within the 16 readable bytes.
            if unsafe { *hay.add(i) } >= needle {
                return i;
            }
        }
        len.min(16)
    }
}

/// Finds the first index `i < len` where `hay[i] == needle` in an 8-element `u16` buffer.
///
/// # Safety
///
/// `hay` must point to at least 16 readable bytes (8 x u16).
#[inline]
#[allow(dead_code)]
pub(crate) unsafe fn search_8_u16(hay: *const u8, len: usize, needle: u16) -> Option<usize> {
    #[cfg(all(target_arch = "x86_64", not(miri)))]
    {
        use core::arch::x86_64::{
            _mm_cmpeq_epi16, _mm_loadu_si128, _mm_movemask_epi8, _mm_set1_epi16,
        };
        // SAFETY: caller guarantees 16 readable bytes.
        unsafe {
            let t = _mm_set1_epi16(needle as i16);
            let h = _mm_loadu_si128(hay.cast());
            let mask =
                _mm_movemask_epi8(_mm_cmpeq_epi16(t, h)) as u32 & ((1u32 << (len.min(8) * 2)) - 1);
            if mask == 0 {
                None
            } else {
                Some((mask.trailing_zeros() / 2) as usize)
            }
        }
    }
    #[cfg(any(miri, not(target_arch = "x86_64")))]
    {
        for i in 0..len.min(8) {
            // SAFETY: i < 8 is within the 16 readable bytes.
            let val = unsafe { (hay.add(i * 2) as *const u16).read_unaligned() };
            if val == needle {
                return Some(i);
            }
        }
        None
    }
}

/// Finds the lower bound index `i <= len` where `hay[i] >= needle` in an 8-element `u16` buffer.
///
/// # Safety
///
/// `hay` must point to at least 16 readable bytes (8 x u16) with sorted contents.
#[inline]
pub(crate) unsafe fn lower_bound_8_u16(hay: *const u8, len: usize, needle: u16) -> usize {
    #[cfg(all(target_arch = "x86_64", not(miri)))]
    {
        use core::arch::x86_64::{
            _mm_cmplt_epi16, _mm_loadu_si128, _mm_movemask_epi8, _mm_set1_epi16, _mm_xor_si128,
        };
        // SAFETY: caller guarantees 16 readable bytes.
        unsafe {
            let bias = _mm_set1_epi16(0x8000u16 as i16);
            let v = _mm_loadu_si128(hay.cast());
            let v_signed = _mm_xor_si128(v, bias);
            let n_signed = _mm_set1_epi16((needle ^ 0x8000) as i16);
            let lt = _mm_cmplt_epi16(v_signed, n_signed);
            let mask = _mm_movemask_epi8(lt) as u32 & ((1u32 << (len.min(8) * 2)) - 1);
            (mask.count_ones() / 2) as usize
        }
    }
    #[cfg(any(miri, not(target_arch = "x86_64")))]
    {
        for i in 0..len.min(8) {
            // SAFETY: i < 8 is within the 16 readable bytes.
            let val = unsafe { (hay.add(i * 2) as *const u16).read_unaligned() };
            if val >= needle {
                return i;
            }
        }
        len.min(8)
    }
}

/// Finds the first index `i < len` where `hay[i] == needle` in a 4-element `u32` buffer.
///
/// # Safety
///
/// `hay` must point to at least 16 readable bytes (4 x u32).
#[inline]
#[allow(dead_code)]
pub(crate) unsafe fn search_4_u32(hay: *const u8, len: usize, needle: u32) -> Option<usize> {
    #[cfg(all(target_arch = "x86_64", not(miri)))]
    {
        use core::arch::x86_64::{
            _mm_cmpeq_epi32, _mm_loadu_si128, _mm_movemask_epi8, _mm_set1_epi32,
        };
        // SAFETY: caller guarantees 16 readable bytes.
        unsafe {
            let t = _mm_set1_epi32(needle as i32);
            let h = _mm_loadu_si128(hay.cast());
            let mask =
                _mm_movemask_epi8(_mm_cmpeq_epi32(t, h)) as u32 & ((1u32 << (len.min(4) * 4)) - 1);
            if mask == 0 {
                None
            } else {
                Some((mask.trailing_zeros() / 4) as usize)
            }
        }
    }
    #[cfg(any(miri, not(target_arch = "x86_64")))]
    {
        for i in 0..len.min(4) {
            // SAFETY: i < 4 is within the 16 readable bytes.
            let val = unsafe { (hay.add(i * 4) as *const u32).read_unaligned() };
            if val == needle {
                return Some(i);
            }
        }
        None
    }
}

/// Finds the lower bound index `i <= len` where `hay[i] >= needle` in a 4-element `u32` buffer.
///
/// # Safety
///
/// `hay` must point to at least 16 readable bytes (4 x u32) with sorted contents.
#[inline]
pub(crate) unsafe fn lower_bound_4_u32(hay: *const u8, len: usize, needle: u32) -> usize {
    #[cfg(all(target_arch = "x86_64", not(miri)))]
    {
        use core::arch::x86_64::{
            _mm_cmplt_epi32, _mm_loadu_si128, _mm_movemask_epi8, _mm_set1_epi32, _mm_xor_si128,
        };
        // SAFETY: caller guarantees 16 readable bytes.
        unsafe {
            let bias = _mm_set1_epi32(0x8000_0000u32 as i32);
            let v = _mm_loadu_si128(hay.cast());
            let v_signed = _mm_xor_si128(v, bias);
            let n_signed = _mm_set1_epi32((needle ^ 0x8000_0000) as i32);
            let lt = _mm_cmplt_epi32(v_signed, n_signed);
            let mask = _mm_movemask_epi8(lt) as u32 & ((1u32 << (len.min(4) * 4)) - 1);
            (mask.count_ones() / 4) as usize
        }
    }
    #[cfg(any(miri, not(target_arch = "x86_64")))]
    {
        for i in 0..len.min(4) {
            // SAFETY: i < 4 is within the 16 readable bytes.
            let val = unsafe { (hay.add(i * 4) as *const u32).read_unaligned() };
            if val >= needle {
                return i;
            }
        }
        len.min(4)
    }
}

/// A 256-bit membership set over one decode byte (0x00..=0xFF): the core of
/// bitmap branches and bitmap leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Bitmap256 {
    /// 4 x 64-bit words representing the 256 membership bits.
    pub words: [u64; 4],
}

/// Runtime `popcnt` dispatch (x86-64 only).
///
/// The x86-64 baseline target does not include `popcnt`, so
/// `u64::count_ones` lowers to a ~12-instruction SWAR sequence — paid
/// four times per `Bitmap256::count`, once per word in `rank`, once per
/// `subexpanse_rank`. Dense distributions terminate in bitmap leaves
/// exclusively (see docs/BENCHMARKING.md's terminal-form census), so
/// every dense lookup pays this on its hot path.
///
/// Detection is cached in a relaxed atomic: 0 = unknown, 1 = absent,
/// 2 = present. The per-call cost after the first is one load and one
/// perfectly predicted branch. Detection granularity is the whole
/// bitmap *operation* (`#[target_feature]` clones whose bodies adopt the
/// feature via `#[inline(always)]` — plain `#[inline]` is advisory
/// within one crate and does not guarantee the adoption), per issue #1's
/// review: a function pointer per `count_ones` would cost more than the
/// SWAR it replaces.
///
/// Non-x86-64 targets compile the portable bodies directly with no
/// dispatch: on AArch64 `u64::count_ones` lowers to the NEON `CNT`+`ADDV`
/// path (per-byte popcount plus horizontal add), not a scalar instruction —
/// base A64 has no scalar popcount (scalar `CNT` needs FEAT_CSSC, Armv8.9+;
/// see docs/HARDWARE.md §2.2).
#[cfg(target_arch = "x86_64")]
pub(crate) mod popcnt_rt {
    use core::sync::atomic::{AtomicU8, Ordering};

    static STATE: AtomicU8 = AtomicU8::new(0);

    #[inline(always)]
    pub(crate) fn available() -> bool {
        let s = STATE.load(Ordering::Relaxed);
        if s == 2 {
            true
        } else if s == 1 {
            false
        } else {
            detect()
        }
    }

    /// First-call CPUID probe, outlined so the dispatchers' fast path is
    /// exactly one load and one predicted branch — the
    /// `is_x86_feature_detected!` expansion would otherwise be inlined
    /// into every lookup entry (review finding, PR #19).
    #[cold]
    #[inline(never)]
    fn detect() -> bool {
        let yes = std::arch::is_x86_feature_detected!("popcnt");
        STATE.store(if yes { 2 } else { 1 }, Ordering::Relaxed);
        yes
    }
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
    #[inline(always)]
    #[must_use]
    pub const fn test(&self, idx: u8) -> bool {
        let w = (idx >> 6) as usize;
        let bit = 1u64 << (idx & 63);
        (self.words[w] & bit) != 0
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
    ///
    /// `inline(always)`: this must fold into its caller so that a
    /// `#[target_feature(enable = "popcnt")]` caller compiles the
    /// `count_ones` calls with the feature — dispatch lives at walk
    /// granularity (see `get::walk`), NOT here. A per-operation clone
    /// was measured and REGRESSED every arm: `target_feature` functions
    /// cannot inline into feature-less callers, so each rank became a
    /// real call in what had been a fused descent.
    #[inline(always)]
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
    #[inline(always)]
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

    /// Number of members below `idx` **within its own 32-digit subexpanse**
    /// (`idx & !31 ..= idx - 1`) — the packed-subarray slot in bitmap
    /// branches and bitmap map-leaves, whose child/value arrays are one per
    /// 32-digit subexpanse.
    #[inline(always)]
    #[must_use]
    pub const fn subexpanse_rank(&self, idx: u8) -> u32 {
        let sub = (idx >> 5) as usize;
        let bit = (idx & 31) as u32;
        // SAFETY: `self.words` contains 4 `u64`s (32 bytes), aligned to 8 bytes.
        // `sub < 8` accesses a valid `u32` within the 8 contiguous `u32` subwords.
        let sub_word = unsafe { *self.words.as_ptr().cast::<u32>().add(sub) };
        let below = (1u32 << bit) - 1;
        (sub_word & below).count_ones()
    }

    /// Tests if `idx` is present, and if so returns its subexpanse rank.
    /// Fuses bit testing and rank calculation with a single direct 32-bit subword load.
    #[inline(always)]
    #[must_use]
    pub const fn test_and_subexpanse_rank(&self, idx: u8) -> Option<usize> {
        let sub = (idx >> 5) as usize;
        let bit = (idx & 31) as u32;
        // SAFETY: `self.words` contains 4 `u64`s (32 bytes), aligned to 8 bytes.
        // `sub < 8` accesses a valid `u32` within the 8 contiguous `u32` subwords.
        let sub_word = unsafe { *self.words.as_ptr().cast::<u32>().add(sub) };
        let bit_mask = 1u32 << bit;
        if (sub_word & bit_mask) == 0 {
            None
        } else {
            let below = bit_mask - 1;
            Some((sub_word & below).count_ones() as usize)
        }
    }

    /// Tests if `idx` is present, and if so returns its `(subexpanse_index, subexpanse_rank)` tuple.
    /// Fuses subexpanse index calculation, bit testing, and rank computation.
    #[inline(always)]
    #[must_use]
    pub const fn test_and_subexpanse_rank_with_sub(&self, idx: u8) -> Option<(usize, usize)> {
        let sub = (idx >> 5) as usize;
        let bit = (idx & 31) as u32;
        // SAFETY: `self.words` contains 4 `u64`s (32 bytes), aligned to 8 bytes.
        // `sub < 8` accesses a valid `u32` within the 8 contiguous `u32` subwords.
        let sub_word = unsafe { *self.words.as_ptr().cast::<u32>().add(sub) };
        let bit_mask = 1u32 << bit;
        if (sub_word & bit_mask) == 0 {
            None
        } else {
            let below = bit_mask - 1;
            Some((sub, (sub_word & below).count_ones() as usize))
        }
    }

    /// Number of members inside one 32-digit subexpanse (`sub` in 0..8) —
    /// the length of that subexpanse's packed child/value array.
    #[inline(always)]
    #[must_use]
    pub const fn subexpanse_count(&self, sub: usize) -> u32 {
        debug_assert!(sub < 8);
        // SAFETY: `self.words` contains 4 `u64`s (32 bytes), aligned to 8 bytes.
        // `sub < 8` accesses a valid `u32` within the 8 contiguous `u32` subwords.
        let sub_word = unsafe { *self.words.as_ptr().cast::<u32>().add(sub) };
        sub_word.count_ones()
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

    /// SIMD/intrinsic parity rule (docs/TESTING.md): the `popcnt`
    /// dispatch path must agree with the portable body on every
    /// operation, for a spread of bitmap densities and every index.
    /// On a CPU with `popcnt` this exercises the `#[target_feature]`
    /// clones against the SWAR bodies; without one, both sides are the
    /// portable body and the test degenerates to a tautology — which is
    /// the correct behaviour, not a gap (there is no second path to
    /// diverge).
    #[test]
    fn popcnt_dispatch_parity() {
        let mut rng = 0x00C0_FFEEu64;
        let mut bitmaps: Vec<Bitmap256> = Vec::new();
        bitmaps.push(Bitmap256::new());
        let mut full = Bitmap256::new();
        let mut i = 0u32;
        while i < 256 {
            full.set(i as u8);
            i += 1;
        }
        bitmaps.push(full);
        for density in [4u32, 32, 128, 250] {
            let mut b = Bitmap256::new();
            for _ in 0..density {
                rng ^= rng << 13;
                rng ^= rng >> 7;
                rng ^= rng << 17;
                b.set((rng & 0xFF) as u8);
            }
            bitmaps.push(b);
        }
        for b in &bitmaps {
            let naive_count: u32 = (0u32..256).filter(|&i| b.test(i as u8)).count() as u32;
            assert_eq!(b.count(), naive_count);
            let step = if cfg!(miri) { 16 } else { 1 };
            for idx in (0..=255u8).step_by(step) {
                let naive_rank = (0..idx).filter(|&i| b.test(i)).count() as u32;
                assert_eq!(b.rank(idx), naive_rank, "rank({idx})");
                let base = idx & !31;
                let naive_sub = (base..idx).filter(|&i| b.test(i)).count() as u32;
                assert_eq!(b.subexpanse_rank(idx), naive_sub, "subexpanse_rank({idx})");
            }
            for sub in 0..8usize {
                let lo = (sub * 32) as u16;
                let naive = (lo..lo + 32).filter(|&i| b.test(i as u8)).count() as u32;
                assert_eq!(b.subexpanse_count(sub), naive);
            }
        }
    }
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
        for _ in 0..if cfg!(miri) { 100 } else { 20_000 } {
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
        for _ in 0..if cfg!(miri) { 5 } else { 200 } {
            let mut bm = Bitmap256::new();
            let mut naive = [false; 256];
            for _ in 0..(if cfg!(miri) { 30 } else { rng.next() % 300 }) {
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
    fn bitmap_subexpanse_rank_matches_naive() {
        let mut rng = XorShift(0x1234_5678_9ABC_DEF1);
        for _ in 0..if cfg!(miri) { 5 } else { 200 } {
            let mut bm = Bitmap256::new();
            let mut naive = [false; 256];
            for _ in 0..(if cfg!(miri) { 30 } else { rng.next() % 300 }) {
                let idx = rng.next() as u8;
                bm.set(idx);
                naive[idx as usize] = true;
            }
            for idx in 0..256usize {
                let base = idx & !31;
                let expected = naive[base..idx].iter().filter(|&&b| b).count() as u32;
                assert_eq!(bm.subexpanse_rank(idx as u8), expected, "idx={idx}");
                if naive[idx] {
                    assert_eq!(
                        bm.test_and_subexpanse_rank(idx as u8),
                        Some(expected as usize),
                        "idx={idx}"
                    );
                } else {
                    assert_eq!(bm.test_and_subexpanse_rank(idx as u8), None, "idx={idx}");
                }
            }
        }
        // Boundary bits of each word/subexpanse.
        let full = Bitmap256::full();
        for idx in [0u8, 31, 32, 63, 64, 95, 96, 127, 128, 255] {
            assert_eq!(full.subexpanse_rank(idx), u32::from(idx) % 32);
            assert_eq!(
                full.test_and_subexpanse_rank(idx),
                Some((u32::from(idx) % 32) as usize)
            );
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

    #[test]
    fn simd_leaf_search_and_lower_bound_parity() {
        let mut rng = XorShift(0xCAFE_BABE_1234_5678);
        for _ in 0..if cfg!(miri) { 100 } else { 10_000 } {
            let mut buf = [0u8; 16];
            for b in &mut buf {
                *b = rng.next() as u8;
            }
            let needle_u8 = rng.next() as u8;
            let len = (rng.next() % 17) as usize;

            // search_16_u8
            // SAFETY: buf is 16 bytes.
            let actual_search = unsafe { search_16_u8(buf.as_ptr(), len, needle_u8) };
            let expected_search = buf[..len.min(16)].iter().position(|&b| b == needle_u8);
            assert_eq!(actual_search, expected_search, "search_16_u8 parity");

            // search_8_u8
            let len_8 = (rng.next() % 9) as usize;
            // SAFETY: buf is at least 8 bytes.
            let actual_search_8 = unsafe { search_8_u8(buf.as_ptr(), len_8, needle_u8) };
            let expected_search_8 = buf[..len_8.min(8)].iter().position(|&b| b == needle_u8);
            assert_eq!(actual_search_8, expected_search_8, "search_8_u8 parity");

            // sorted lower_bound_16_u8
            buf.sort_unstable();
            // SAFETY: buf is 16 bytes.
            let actual_lb = unsafe { lower_bound_16_u8(buf.as_ptr(), len, needle_u8) };
            let expected_lb = buf[..len.min(16)]
                .iter()
                .position(|&b| b >= needle_u8)
                .unwrap_or(len.min(16));
            assert_eq!(actual_lb, expected_lb, "lower_bound_16_u8 parity");

            // sorted lower_bound_8_u8
            // SAFETY: buf is at least 8 bytes sorted.
            let actual_lb_8 = unsafe { lower_bound_8_u8(buf.as_ptr(), len_8, needle_u8) };
            let expected_lb_8 = buf[..len_8.min(8)]
                .iter()
                .position(|&b| b >= needle_u8)
                .unwrap_or(len_8.min(8));
            assert_eq!(actual_lb_8, expected_lb_8, "lower_bound_8_u8 parity");

            // search_8_u16
            let needle_u16 = rng.next() as u16;
            let len_u16 = (rng.next() % 9) as usize;
            // SAFETY: buf is 16 bytes (8 x u16).
            let actual_u16 = unsafe { search_8_u16(buf.as_ptr(), len_u16, needle_u16) };
            let mut expected_u16 = None;
            for i in 0..len_u16.min(8) {
                let val = u16::from_ne_bytes([buf[i * 2], buf[i * 2 + 1]]);
                if val == needle_u16 {
                    expected_u16 = Some(i);
                    break;
                }
            }
            assert_eq!(actual_u16, expected_u16, "search_8_u16 parity");

            // sorted lower_bound_8_u16
            let mut u16_arr = [0u16; 8];
            for i in 0..8 {
                u16_arr[i] = u16::from_ne_bytes([buf[i * 2], buf[i * 2 + 1]]);
            }
            u16_arr.sort_unstable();
            let mut sorted_buf = [0u8; 16];
            for i in 0..8 {
                let bytes = u16_arr[i].to_ne_bytes();
                sorted_buf[i * 2] = bytes[0];
                sorted_buf[i * 2 + 1] = bytes[1];
            }
            // SAFETY: sorted_buf is 16 bytes (8 x u16) sorted.
            let actual_lb_u16 =
                unsafe { lower_bound_8_u16(sorted_buf.as_ptr(), len_u16, needle_u16) };
            let expected_lb_u16 = u16_arr[..len_u16.min(8)]
                .iter()
                .position(|&v| v >= needle_u16)
                .unwrap_or(len_u16.min(8));
            assert_eq!(actual_lb_u16, expected_lb_u16, "lower_bound_8_u16 parity");

            // search_4_u32
            let needle_u32 = rng.next() as u32;
            let len_u32 = (rng.next() % 5) as usize;
            // SAFETY: buf is 16 bytes (4 x u32).
            let actual_u32 = unsafe { search_4_u32(buf.as_ptr(), len_u32, needle_u32) };
            let mut expected_u32 = None;
            for i in 0..len_u32.min(4) {
                let val = u32::from_ne_bytes([
                    buf[i * 4],
                    buf[i * 4 + 1],
                    buf[i * 4 + 2],
                    buf[i * 4 + 3],
                ]);
                if val == needle_u32 {
                    expected_u32 = Some(i);
                    break;
                }
            }
            assert_eq!(actual_u32, expected_u32, "search_4_u32 parity");

            // sorted lower_bound_4_u32
            let mut u32_arr = [0u32; 4];
            for i in 0..4 {
                u32_arr[i] = u32::from_ne_bytes([
                    buf[i * 4],
                    buf[i * 4 + 1],
                    buf[i * 4 + 2],
                    buf[i * 4 + 3],
                ]);
            }
            u32_arr.sort_unstable();
            let mut sorted_buf_32 = [0u8; 16];
            for i in 0..4 {
                let bytes = u32_arr[i].to_ne_bytes();
                sorted_buf_32[i * 4] = bytes[0];
                sorted_buf_32[i * 4 + 1] = bytes[1];
                sorted_buf_32[i * 4 + 2] = bytes[2];
                sorted_buf_32[i * 4 + 3] = bytes[3];
            }
            // SAFETY: sorted_buf_32 is 16 bytes (4 x u32) sorted.
            let actual_lb_u32 =
                unsafe { lower_bound_4_u32(sorted_buf_32.as_ptr(), len_u32, needle_u32) };
            let expected_lb_u32 = u32_arr[..len_u32.min(4)]
                .iter()
                .position(|&v| v >= needle_u32)
                .unwrap_or(len_u32.min(4));
            assert_eq!(actual_lb_u32, expected_lb_u32, "lower_bound_4_u32 parity");
        }
    }
}
