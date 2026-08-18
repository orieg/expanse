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

/// Slot-capacity class: leaf and subarray allocations are sized in
/// multiples of four slots, so runs of consecutive inserts and deletes
/// shift in place instead of reallocating (they reallocate only when the
/// population crosses a class boundary). Allocation sizes stay derivable
/// from the current population alone — `free` needs no stored capacity.
#[inline]
#[must_use]
pub const fn cap_class(pop: usize) -> usize {
    // Tiny populations stay exact — wide-key map leaves of one or two
    // entries are common under random keys, and rounding them to four
    // slots measurably regressed bytes/key.
    if pop <= 2 { pop } else { (pop + 3) & !3 }
}

/// Allocation size of a set-flavor leaf.
#[inline]
#[must_use]
pub const fn size_set(key_bytes: u8, pop: usize) -> usize {
    key_bytes as usize * cap_class(pop)
}

/// Allocation size of a map-flavor leaf (`pop` values then `pop` keys,
/// both areas class-sized).
#[inline]
#[must_use]
pub const fn size_map(key_bytes: u8, pop: usize) -> usize {
    8 * cap_class(pop) + key_bytes as usize * cap_class(pop)
}

/// Offset of the packed key area inside a map-flavor leaf.
#[inline]
#[must_use]
pub const fn map_keys_offset(pop: usize) -> usize {
    8 * cap_class(pop)
}

/// Binary search over packed little-endian keys: first slot whose key is
/// `>= needle` (`needle` already masked to `key_bytes`).
///
/// # Safety
///
/// `keys` must be valid for reads of `key_bytes * pop` bytes.
#[inline]
#[must_use]
pub(crate) unsafe fn lower_bound(keys: *const u8, pop: usize, key_bytes: u8, needle: u64) -> usize {
    let (mut lo, mut hi) = (0usize, pop);
    while lo < hi {
        let mid = (lo + hi) / 2;
        // SAFETY: mid < pop per the loop bounds and caller contract.
        if unsafe { crate::mutate::read_packed(keys, mid, key_bytes as usize) } < needle {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// In-place insert into a set leaf with spare class capacity: shifts keys
/// `[pos..pop)` right one slot and writes `key` at `pos`.
///
/// # Safety
///
/// The allocation must hold `cap_class(pop + 1)` slots (i.e.
/// `cap_class(pop) == cap_class(pop + 1)`), and `pos <= pop`.
pub(crate) unsafe fn set_insert_at(base: *mut u8, key_bytes: u8, pop: usize, pos: usize, key: u64) {
    let kb = key_bytes as usize;
    // SAFETY: in-bounds shift within the class-sized allocation.
    unsafe {
        core::ptr::copy(
            base.add(pos * kb),
            base.add((pos + 1) * kb),
            (pop - pos) * kb,
        );
        crate::mutate::write_packed(base, pos, kb, key);
    }
}

/// In-place removal from a set leaf: shifts keys `[pos + 1..pop)` left.
///
/// # Safety
///
/// `pos < pop`; the allocation holds `pop` keys.
pub(crate) unsafe fn set_remove_at(base: *mut u8, key_bytes: u8, pop: usize, pos: usize) {
    let kb = key_bytes as usize;
    // SAFETY: in-bounds shift.
    unsafe {
        core::ptr::copy(
            base.add((pos + 1) * kb),
            base.add(pos * kb),
            (pop - 1 - pos) * kb,
        );
    }
}

/// In-place insert into a map leaf with spare class capacity: shifts both
/// the value and key areas (the key area's offset is class-stable here).
///
/// # Safety
///
/// `cap_class(pop) == cap_class(pop + 1)`, `pos <= pop`, live map leaf.
pub(crate) unsafe fn map_insert_at(
    base: *mut u8,
    key_bytes: u8,
    pop: usize,
    pos: usize,
    key: u64,
    val: u64,
) {
    let kb = key_bytes as usize;
    let keys = base.wrapping_add(map_keys_offset(pop));
    // SAFETY: in-bounds shifts within the class-sized areas.
    unsafe {
        let vals = base.cast::<u64>();
        core::ptr::copy(vals.add(pos), vals.add(pos + 1), pop - pos);
        vals.add(pos).write(val);
        core::ptr::copy(
            keys.add(pos * kb),
            keys.add((pos + 1) * kb),
            (pop - pos) * kb,
        );
        crate::mutate::write_packed(keys, pos, kb, key);
    }
}

/// In-place removal from a map leaf (class-stable, see [`map_insert_at`]).
///
/// # Safety
///
/// `cap_class(pop) == cap_class(pop - 1)`, `pos < pop`, live map leaf.
pub(crate) unsafe fn map_remove_at(base: *mut u8, key_bytes: u8, pop: usize, pos: usize) {
    let kb = key_bytes as usize;
    let keys = base.wrapping_add(map_keys_offset(pop));
    // SAFETY: in-bounds shifts.
    unsafe {
        let vals = base.cast::<u64>();
        core::ptr::copy(vals.add(pos + 1), vals.add(pos), pop - 1 - pos);
        core::ptr::copy(
            keys.add((pos + 1) * kb),
            keys.add(pos * kb),
            (pop - 1 - pos) * kb,
        );
    }
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
        // Class-based sizing: allocations round the population up to a
        // multiple of four slots.
        assert_eq!(cap_class(1), 1);
        assert_eq!(cap_class(2), 2);
        assert_eq!(cap_class(3), 4);
        assert_eq!(cap_class(4), 4);
        assert_eq!(cap_class(5), 8);
        assert_eq!(cap_class(25), 28);
        assert_eq!(size_set(1, 25), 28);
        assert_eq!(size_set(7, 2), 14);
        assert_eq!(size_map(1, 25), 8 * 28 + 28);
        assert_eq!(size_map(7, 2), 8 * 2 + 7 * 2);
        assert_eq!(map_keys_offset(3), 32);
        assert_eq!(map_keys_offset(4), 32);
        assert_eq!(map_keys_offset(5), 64);
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
