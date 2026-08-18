//! Phase 5: variable-length linear-leaf layout and search.
//!
//! A linear leaf at level `L` stores `pop` packed key remainders of `L`
//! bytes each (1..=7), sorted ascending by numeric value, each key stored
//! little-endian. Leaves have **no header**: the population lives in the
//! parent edge's level-split `pop0` field and the OCC story goes through
//! the parent branch — exactly the original design's economy, where a
//! leaf is nothing but payload.
//!
//! - Set flavor: `[keys: L × pop]` — nothing else.
//! - Map flavor: `[values: u64 × pop][keys: L × pop]` in one allocation.
//!   Values first keeps them 8-aligned for free (allocations are
//!   cache-line aligned) with no padding arithmetic.
//!
//! Search is a scan over the packed keys; population caps (set by the
//! Phase 6 conversion ladder) keep leaves at a handful of cache lines, and
//! the Phase 8 bench pass decides whether a SIMD/binary variant earns its
//! complexity over this baseline.

use crate::types::Key;

/// Allocation size of a set-flavor leaf.
#[inline]
#[must_use]
pub const fn size_set(key_bytes: u8, pop: usize) -> usize {
    key_bytes as usize * pop
}

/// Allocation size of a map-flavor leaf (`pop` values then `pop` keys).
#[inline]
#[must_use]
pub const fn size_map(key_bytes: u8, pop: usize) -> usize {
    8 * pop + key_bytes as usize * pop
}

/// Offset of the packed key area inside a map-flavor leaf.
#[inline]
#[must_use]
pub const fn map_keys_offset(pop: usize) -> usize {
    8 * pop
}

/// Finds the slot of `key`'s low `key_bytes` bytes among `pop` packed keys.
///
/// # Safety
///
/// `keys` must be valid for reads of `key_bytes * pop` bytes.
#[inline]
#[must_use]
pub unsafe fn search(keys: *const u8, pop: usize, key_bytes: u8, key: Key) -> Option<usize> {
    let kb = key_bytes as usize;
    debug_assert!((1..=7).contains(&kb));
    let needle = &key.to_le_bytes()[..kb];
    // SAFETY: caller guarantees `key_bytes * pop` readable bytes.
    let packed = unsafe { core::slice::from_raw_parts(keys, kb * pop) };
    packed
        .chunks_exact(kb)
        .position(|candidate| candidate == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_and_offsets() {
        assert_eq!(size_set(1, 25), 25);
        assert_eq!(size_set(7, 2), 14);
        assert_eq!(size_map(1, 25), 8 * 25 + 25);
        assert_eq!(size_map(7, 2), 16 + 14);
        assert_eq!(map_keys_offset(3), 24);
    }

    #[test]
    fn search_all_key_sizes() {
        for kb in 1u8..=7 {
            // Keys chosen to differ in first, middle, and last bytes.
            let keys: Vec<u64> = vec![
                0,
                1,
                0xA5,
                1u64 << (8 * (kb - 1)),
                (1u64 << (8 * kb)) - 2,
                (1u64 << (8 * kb)) - 1,
            ];
            let mut packed = Vec::new();
            for k in &keys {
                packed.extend_from_slice(&k.to_le_bytes()[..kb as usize]);
            }
            for k in &keys {
                // Duplicated values (kb=1 collides two picks) match their
                // first slot.
                let first = keys.iter().position(|x| x == k).unwrap();
                // SAFETY: `packed` holds exactly pop × kb bytes.
                let got = unsafe { search(packed.as_ptr(), keys.len(), kb, *k) };
                assert_eq!(got, Some(first), "kb={kb} key={k:#x}");
            }
            for absent in [2u64, 0xA4, (1u64 << (8 * kb)) - 3] {
                // SAFETY: same buffer.
                let got = unsafe { search(packed.as_ptr(), keys.len(), kb, absent) };
                assert_eq!(got, None, "kb={kb} absent={absent:#x}");
            }
            // High bytes beyond kb must not affect matching.
            let with_garbage = keys[2] | (0xEEu64 << (8 * u32::from(kb)));
            // SAFETY: same buffer.
            let got = unsafe { search(packed.as_ptr(), keys.len(), kb, with_garbage) };
            assert_eq!(got, Some(2), "kb={kb}: high bytes must be ignored");
        }
    }

    #[test]
    fn search_empty_leaf() {
        // SAFETY: zero-length read from a dangling-but-aligned pointer is
        // valid for an empty slice.
        let got = unsafe { search(core::ptr::NonNull::<u8>::dangling().as_ptr(), 0, 3, 42) };
        assert_eq!(got, None);
    }
}
