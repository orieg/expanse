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
    debug_assert!((1..=7).contains(&key_bytes));
    // SAFETY: forwarded contract; each arm's KB equals `key_bytes`.
    unsafe {
        match key_bytes {
            1 => lower_bound_fixed::<1>(keys, pop, needle),
            2 => lower_bound_fixed::<2>(keys, pop, needle),
            3 => lower_bound_fixed::<3>(keys, pop, needle),
            4 => lower_bound_fixed::<4>(keys, pop, needle),
            5 => lower_bound_fixed::<5>(keys, pop, needle),
            6 => lower_bound_fixed::<6>(keys, pop, needle),
            _ => lower_bound_fixed::<7>(keys, pop, needle),
        }
    }
}

/// Binary search at a compile-time key width: the whole probe — load,
/// widen, compare — becomes inline code instead of a call per step.
/// This is the innermost loop of every leaf insert (issue #1 item 3).
///
/// # Safety
///
/// `keys` must be valid for reads of `KB * pop` bytes.
#[inline]
unsafe fn lower_bound_fixed<const KB: usize>(keys: *const u8, pop: usize, needle: u64) -> usize {
    if KB == 1 && (13..=16).contains(&pop) {
        // SAFETY: cap_class(pop >= 13) is 16, so keys holds at least 16 bytes.
        unsafe { crate::bits::lower_bound_16_u8(keys, pop, needle as u8) }
    } else if pop <= 4 {
        if pop == 0 {
            0
        } else if pop == 1 {
            // SAFETY: pop == 1 guarantees slot 0 is in-bounds.
            (unsafe { crate::mutate::read_packed_fixed::<KB>(keys, 0) } < needle) as usize
        } else if pop == 2 {
            // SAFETY: pop == 2 guarantees slots 0 and 1 are in-bounds.
            let c0 = (unsafe { crate::mutate::read_packed_fixed::<KB>(keys, 0) } < needle) as usize;
            // SAFETY: pop == 2 guarantees slots 0 and 1 are in-bounds.
            let c1 = (unsafe { crate::mutate::read_packed_fixed::<KB>(keys, 1) } < needle) as usize;
            c0 + c1
        } else if pop == 3 {
            // SAFETY: pop == 3 guarantees slots 0..3 are in-bounds.
            let c0 = (unsafe { crate::mutate::read_packed_fixed::<KB>(keys, 0) } < needle) as usize;
            // SAFETY: pop == 3 guarantees slots 0..3 are in-bounds.
            let c1 = (unsafe { crate::mutate::read_packed_fixed::<KB>(keys, 1) } < needle) as usize;
            // SAFETY: pop == 3 guarantees slots 0..3 are in-bounds.
            let c2 = (unsafe { crate::mutate::read_packed_fixed::<KB>(keys, 2) } < needle) as usize;
            c0 + c1 + c2
        } else {
            // SAFETY: pop == 4 guarantees slots 0..4 are in-bounds.
            let c0 = (unsafe { crate::mutate::read_packed_fixed::<KB>(keys, 0) } < needle) as usize;
            // SAFETY: pop == 4 guarantees slots 0..4 are in-bounds.
            let c1 = (unsafe { crate::mutate::read_packed_fixed::<KB>(keys, 1) } < needle) as usize;
            // SAFETY: pop == 4 guarantees slots 0..4 are in-bounds.
            let c2 = (unsafe { crate::mutate::read_packed_fixed::<KB>(keys, 2) } < needle) as usize;
            // SAFETY: pop == 4 guarantees slots 0..4 are in-bounds.
            let c3 = (unsafe { crate::mutate::read_packed_fixed::<KB>(keys, 3) } < needle) as usize;
            c0 + c1 + c2 + c3
        }
    } else {
        let (mut lo, mut hi) = (0usize, pop);
        while lo < hi {
            let mid = (lo + hi) / 2;
            // SAFETY: mid < pop per the loop bounds and caller contract.
            if unsafe { crate::mutate::read_packed_fixed::<KB>(keys, mid) } < needle {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo
    }
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

/// Copies a set leaf into `new` (sized for `pop + 1` in its class) with
/// `key` inserted at `pos` — the class-crossing analogue of
/// [`set_insert_at`], and the set twin of [`map_realloc_insert`]: two
/// bulk copies and one packed write instead of materializing every key
/// into a heap `Vec` and repacking.
///
/// # Safety
///
/// `old` must be a live set leaf of `pop` keys of `key_bytes` bytes;
/// `new` must be a fresh allocation of `size_set(key_bytes, pop+1)`
/// bytes; `pos <= pop`.
pub(crate) unsafe fn set_realloc_insert(
    old: *const u8,
    new: *mut u8,
    key_bytes: u8,
    pop: usize,
    pos: usize,
    key: u64,
) {
    let kb = key_bytes as usize;
    // SAFETY: bounds per contract; the two allocations are disjoint.
    unsafe {
        core::ptr::copy_nonoverlapping(old, new, pos * kb);
        crate::mutate::write_packed(new, pos, kb, key);
        core::ptr::copy_nonoverlapping(
            old.add(pos * kb),
            new.add((pos + 1) * kb),
            (pop - pos) * kb,
        );
    }
}

/// Copies a set leaf into `new` (sized for `pop - 1` in its class) with
/// the key at `pos` removed — see [`set_realloc_insert`].
///
/// # Safety
///
/// `old` must be a live set leaf of `pop >= 2` keys of `key_bytes`
/// bytes; `new` must be a fresh allocation of `size_set(key_bytes,
/// pop-1)` bytes; `pos < pop`.
pub(crate) unsafe fn set_realloc_remove(
    old: *const u8,
    new: *mut u8,
    key_bytes: u8,
    pop: usize,
    pos: usize,
) {
    let kb = key_bytes as usize;
    // SAFETY: bounds per contract; the two allocations are disjoint.
    unsafe {
        core::ptr::copy_nonoverlapping(old, new, pos * kb);
        core::ptr::copy_nonoverlapping(
            old.add((pos + 1) * kb),
            new.add(pos * kb),
            (pop - 1 - pos) * kb,
        );
    }
}

/// Copies a map leaf into `new` (sized for `pop + 1` in its class) with
/// `key`/`val` inserted at `pos` — the **class-crossing** analogue of
/// [`map_insert_at`]. Four bulk copies and one packed write.
///
/// This replaces the materialize-into-`Vec` slow path for grows that stay
/// a linear leaf: the churn benchmark showed steady-state insert/remove
/// cycling across the exact 1↔2 capacity classes, paying a heap `Vec`,
/// a per-entry unpack and a per-entry repack on every crossing.
///
/// # Safety
///
/// `old` must be a live map leaf of `pop` entries with `key_bytes`-byte
/// keys; `new` must be a fresh allocation of `size_map(key_bytes, pop+1)`
/// bytes; `pos <= pop`.
pub(crate) unsafe fn map_realloc_insert(
    old: *const u8,
    new: *mut u8,
    key_bytes: u8,
    pop: usize,
    pos: usize,
    key: u64,
    val: u64,
) {
    let kb = key_bytes as usize;
    // SAFETY: bounds per contract; the two allocations are disjoint.
    unsafe {
        let ov = old.cast::<u64>();
        let nv = new.cast::<u64>();
        core::ptr::copy_nonoverlapping(ov, nv, pos);
        nv.add(pos).write(val);
        core::ptr::copy_nonoverlapping(ov.add(pos), nv.add(pos + 1), pop - pos);
        let ok = old.add(map_keys_offset(pop));
        let nk = new.add(map_keys_offset(pop + 1));
        core::ptr::copy_nonoverlapping(ok, nk, pos * kb);
        crate::mutate::write_packed(nk, pos, kb, key);
        core::ptr::copy_nonoverlapping(ok.add(pos * kb), nk.add((pos + 1) * kb), (pop - pos) * kb);
    }
}

/// Copies a map leaf into `new` (sized for `pop - 1` in its class) with
/// the entry at `pos` removed — the class-crossing analogue of
/// [`map_remove_at`]; see [`map_realloc_insert`].
///
/// # Safety
///
/// `old` must be a live map leaf of `pop >= 2` entries with
/// `key_bytes`-byte keys; `new` must be a fresh allocation of
/// `size_map(key_bytes, pop-1)` bytes; `pos < pop`.
pub(crate) unsafe fn map_realloc_remove(
    old: *const u8,
    new: *mut u8,
    key_bytes: u8,
    pop: usize,
    pos: usize,
) {
    let kb = key_bytes as usize;
    // SAFETY: bounds per contract; the two allocations are disjoint.
    unsafe {
        let ov = old.cast::<u64>();
        let nv = new.cast::<u64>();
        core::ptr::copy_nonoverlapping(ov, nv, pos);
        core::ptr::copy_nonoverlapping(ov.add(pos + 1), nv.add(pos), pop - 1 - pos);
        let ok = old.add(map_keys_offset(pop));
        let nk = new.add(map_keys_offset(pop - 1));
        core::ptr::copy_nonoverlapping(ok, nk, pos * kb);
        core::ptr::copy_nonoverlapping(
            ok.add((pos + 1) * kb),
            nk.add(pos * kb),
            (pop - 1 - pos) * kb,
        );
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

/// Scans `pop` packed keys of a **compile-time** width for `key`'s low
/// `KB` bytes.
///
/// Width-monomorphized on purpose: with a runtime width the slice
/// comparison lowers to a `memcmp` call — which on macOS goes through a
/// dynamic-linker stub, and showed up as ~6% of samples in the lookup
/// profile (`examples/lookup_profile.rs`). At a constant width the
/// candidate is a fixed-size array, so the comparison inlines to a
/// couple of loads and a compare with no call at all.
///
/// # Safety
///
/// `keys` must be valid for reads of `KB * pop` bytes.
#[inline]
unsafe fn search_fixed<const KB: usize>(keys: *const u8, pop: usize, key: Key) -> Option<usize> {
    if KB == 1 && (13..=16).contains(&pop) {
        // SAFETY: cap_class(pop >= 13) is 16, so keys holds at least 16 bytes.
        unsafe { crate::bits::search_16_u8(keys, pop, key as u8) }
    } else if KB == 2 && pop == 8 {
        // SAFETY: cap_class(8) * 2 is 16, so keys holds at least 16 bytes.
        unsafe { crate::bits::search_8_u16(keys, pop, key as u16) }
    } else if KB == 4 && pop == 4 {
        // SAFETY: cap_class(4) * 4 is 16, so keys holds at least 16 bytes.
        unsafe { crate::bits::search_4_u32(keys, pop, key as u32) }
    } else if pop <= 4 {
        let needle = crate::mutate::key_low(key, KB as u8);
        // SAFETY: pop >= 1 guarantees slot 0 is readable.
        if pop >= 1 && unsafe { crate::mutate::read_packed_fixed::<KB>(keys, 0) } == needle {
            return Some(0);
        }
        // SAFETY: pop >= 2 guarantees slot 1 is readable.
        if pop >= 2 && unsafe { crate::mutate::read_packed_fixed::<KB>(keys, 1) } == needle {
            return Some(1);
        }
        // SAFETY: pop >= 3 guarantees slot 2 is readable.
        if pop >= 3 && unsafe { crate::mutate::read_packed_fixed::<KB>(keys, 2) } == needle {
            return Some(2);
        }
        // SAFETY: pop >= 4 guarantees slot 3 is readable.
        if pop >= 4 && unsafe { crate::mutate::read_packed_fixed::<KB>(keys, 3) } == needle {
            return Some(3);
        }
        None
    } else {
        let needle = crate::mutate::key_low(key, KB as u8);
        // SAFETY: forwarded caller contract.
        let pos = unsafe { lower_bound_fixed::<KB>(keys, pop, needle) };
        // SAFETY: pos < pop is within the allocated `pop` keys.
        if pos < pop && unsafe { crate::mutate::read_packed_fixed::<KB>(keys, pos) } == needle {
            Some(pos)
        } else {
            None
        }
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
    debug_assert!((1..=7).contains(&key_bytes));
    // SAFETY: forwarded caller contract; each arm's `KB` equals
    // `key_bytes`, so the byte counts match.
    unsafe {
        match key_bytes {
            1 => search_fixed::<1>(keys, pop, key),
            2 => search_fixed::<2>(keys, pop, key),
            3 => search_fixed::<3>(keys, pop, key),
            4 => search_fixed::<4>(keys, pop, key),
            5 => search_fixed::<5>(keys, pop, key),
            6 => search_fixed::<6>(keys, pop, key),
            _ => search_fixed::<7>(keys, pop, key),
        }
    }
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
            let mut keys: Vec<u64> = vec![
                0,
                1,
                0xA5,
                1u64 << (8 * (kb - 1)),
                (1u64 << (8 * kb)) - 2,
                (1u64 << (8 * kb)) - 1,
            ];
            keys.sort_unstable();
            keys.dedup();
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
