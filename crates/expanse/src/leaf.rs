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
#[inline(always)]
#[must_use]
#[allow(dead_code)]
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

/// Locates `needle` in a linear leaf's packed keys.
/// Returns `Ok(pos)` if an exact match is found at `pos`.
/// Returns `Err(pos)` if absent, where `pos` is the insertion index.
///
/// # Safety
///
/// `keys` must be valid for reads of `key_bytes * pop` bytes.
#[inline(always)]
pub(crate) unsafe fn locate(
    keys: *const u8,
    pop: usize,
    key_bytes: u8,
    needle: u64,
) -> Result<usize, usize> {
    debug_assert!((1..=7).contains(&key_bytes));
    // SAFETY: forwarded contract; each arm's KB equals `key_bytes`.
    unsafe {
        match key_bytes {
            1 => locate_fixed::<1>(keys, pop, needle),
            2 => locate_fixed::<2>(keys, pop, needle),
            3 => locate_fixed::<3>(keys, pop, needle),
            4 => locate_fixed::<4>(keys, pop, needle),
            5 => locate_fixed::<5>(keys, pop, needle),
            6 => locate_fixed::<6>(keys, pop, needle),
            _ => locate_fixed::<7>(keys, pop, needle),
        }
    }
}

/// Locates `needle` at a compile-time key width.
///
/// # Safety
///
/// `keys` must be valid for reads of `KB * pop` bytes.
#[inline(always)]
pub(crate) unsafe fn locate_fixed<const KB: usize>(
    keys: *const u8,
    pop: usize,
    needle: u64,
) -> Result<usize, usize> {
    if pop == 0 {
        return Err(0);
    }
    // SAFETY: keys holds at least `pop * KB` bytes.
    let pos = unsafe { lower_bound_fixed::<KB>(keys, pop, needle) };
    // SAFETY: pos < pop is in bounds.
    if pos < pop && unsafe { crate::mutate::read_packed_fixed::<KB>(keys, pos) } == needle {
        Ok(pos)
    } else {
        Err(pos)
    }
}

/// Binary search at a compile-time key width: the whole probe — load,
/// widen, compare — becomes inline code instead of a call per step.
/// This is the innermost loop of every leaf insert (issue #1 item 3).
///
/// # Safety
///
/// `keys` must be valid for reads of `KB * pop` bytes.
#[inline(always)]
pub(crate) unsafe fn lower_bound_fixed<const KB: usize>(
    keys: *const u8,
    pop: usize,
    needle: u64,
) -> usize {
    // Every vectorized branch below must satisfy: cap_class(pop) * KB >=
    // the kernel's fixed load width (see `simd_gates_within_cap_class`).
    // cap_class rounds to multiples of FOUR, so pop 9..=12 only guarantees
    // 12 slots — gating those into the 16-byte kernel read out of bounds.
    if KB == 1 && (13..=16).contains(&pop) {
        // SAFETY: cap_class(pop >= 13) is 16, so keys holds at least 16 bytes.
        unsafe { crate::bits::lower_bound_16_u8(keys, pop, needle as u8) }
    } else if KB == 1 && (5..=8).contains(&pop) {
        // SAFETY: cap_class(pop >= 5) is 8, so keys holds at least 8 bytes.
        unsafe { crate::bits::lower_bound_8_u8(keys, pop, needle as u8) }
    } else if KB == 2 && (5..=8).contains(&pop) {
        // SAFETY: cap_class(pop >= 5) * 2 is 16, so keys holds at least 16 bytes.
        unsafe { crate::bits::lower_bound_8_u16(keys, pop, needle as u16) }
    } else if KB == 4 && (3..=4).contains(&pop) {
        // SAFETY: cap_class(pop >= 3) * 4 is 16, so keys holds at least 16 bytes.
        unsafe { crate::bits::lower_bound_4_u32(keys, pop, needle as u32) }
    } else if pop <= 32 {
        // Branchless parallel-load linear scan: issues all loads independently
        // without data-dependent serial pointer chasing, eliminating branch
        // mispredictions on uniform-random lookups (#480).
        let mut count = 0usize;
        let mut i = 0usize;
        while i + 4 <= pop {
            // SAFETY: i + 3 < pop, caller guarantees pop * KB readable bytes.
            unsafe {
                let k0 = crate::mutate::read_packed_fixed::<KB>(keys, i);
                let k1 = crate::mutate::read_packed_fixed::<KB>(keys, i + 1);
                let k2 = crate::mutate::read_packed_fixed::<KB>(keys, i + 2);
                let k3 = crate::mutate::read_packed_fixed::<KB>(keys, i + 3);
                count += (k0 < needle) as usize
                    + (k1 < needle) as usize
                    + (k2 < needle) as usize
                    + (k3 < needle) as usize;
            }
            i += 4;
        }
        while i < pop {
            // SAFETY: i < pop, caller guarantees pop * KB readable bytes.
            unsafe {
                let k = crate::mutate::read_packed_fixed::<KB>(keys, i);
                count += (k < needle) as usize;
            }
            i += 1;
        }
        count
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
#[inline(always)]
pub(crate) unsafe fn set_insert_at(base: *mut u8, key_bytes: u8, pop: usize, pos: usize, key: u64) {
    let kb = key_bytes as usize;
    // SAFETY: in-bounds shift within the class-sized allocation.
    unsafe {
        if pos < pop {
            core::ptr::copy(
                base.add(pos * kb),
                base.add((pos + 1) * kb),
                (pop - pos) * kb,
            );
        }
        crate::mutate::write_packed(base, pos, kb, key);
    }
}

/// The allocation must hold `cap_class(pop + 1)` slots and `pos <= pop`.
#[inline(always)]
pub(crate) unsafe fn set_insert_at_fixed<const KB: usize>(
    base: *mut u8,
    pop: usize,
    pos: usize,
    key: u64,
) {
    // SAFETY: in-bounds shift within the class-sized allocation.
    unsafe {
        if pos < pop {
            core::ptr::copy(
                base.add(pos * KB),
                base.add((pos + 1) * KB),
                (pop - pos) * KB,
            );
        }
        crate::mutate::write_packed_fixed::<KB>(base, pos, key);
    }
}

/// In-place removal from a set leaf: shifts keys `[pos + 1..pop)` left.
///
/// # Safety
///
/// `pos < pop`; the allocation holds `pop` keys.
#[inline(always)]
pub(crate) unsafe fn set_remove_at(base: *mut u8, key_bytes: u8, pop: usize, pos: usize) {
    let kb = key_bytes as usize;
    // SAFETY: in-bounds shift.
    unsafe {
        if pos + 1 < pop {
            core::ptr::copy(
                base.add((pos + 1) * kb),
                base.add(pos * kb),
                (pop - 1 - pos) * kb,
            );
        }
    }
}

/// In-place insert into a map leaf with spare class capacity: shifts both
/// the value and key areas (the key area's offset is class-stable here).
///
/// # Safety
///
/// `cap_class(pop) == cap_class(pop + 1)`, `pos <= pop`, live map leaf.
#[inline(always)]
pub(crate) unsafe fn map_insert_at(
    base: *mut u8,
    key_bytes: u8,
    pop: usize,
    pos: usize,
    key: u64,
    val: u64,
) {
    // SAFETY: caller guarantees live map leaf and in-bounds indices for key_bytes.
    unsafe {
        match key_bytes {
            1 => map_insert_at_fixed::<1>(base, pop, pos, key, val),
            2 => map_insert_at_fixed::<2>(base, pop, pos, key, val),
            3 => map_insert_at_fixed::<3>(base, pop, pos, key, val),
            4 => map_insert_at_fixed::<4>(base, pop, pos, key, val),
            5 => map_insert_at_fixed::<5>(base, pop, pos, key, val),
            6 => map_insert_at_fixed::<6>(base, pop, pos, key, val),
            _ => map_insert_at_fixed::<7>(base, pop, pos, key, val),
        }
    }
}

/// In-place insert into a map leaf with compile-time known key width.
///
/// # Safety
///
/// `cap_class(pop) == cap_class(pop + 1)`, `pos <= pop`, live map leaf.
#[inline(always)]
pub(crate) unsafe fn map_insert_at_fixed<const KB: usize>(
    base: *mut u8,
    pop: usize,
    pos: usize,
    key: u64,
    val: u64,
) {
    let keys = base.wrapping_add(map_keys_offset(pop));
    // SAFETY: in-bounds shifts within the class-sized areas.
    unsafe {
        let vals = base.cast::<u64>();
        if pos < pop {
            core::ptr::copy(vals.add(pos), vals.add(pos + 1), pop - pos);
            core::ptr::copy(
                keys.add(pos * KB),
                keys.add((pos + 1) * KB),
                (pop - pos) * KB,
            );
        }
        vals.add(pos).write(val);
        crate::mutate::write_packed_fixed::<KB>(keys, pos, key);
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
#[inline(always)]
pub(crate) unsafe fn set_realloc_insert(
    old: *const u8,
    new: *mut u8,
    key_bytes: u8,
    pop: usize,
    pos: usize,
    key: u64,
) {
    // SAFETY: caller guarantees old/new pointers and pop/pos bounds for key_bytes.
    unsafe {
        match key_bytes {
            1 => set_realloc_insert_fixed::<1>(old, new, pop, pos, key),
            2 => set_realloc_insert_fixed::<2>(old, new, pop, pos, key),
            3 => set_realloc_insert_fixed::<3>(old, new, pop, pos, key),
            4 => set_realloc_insert_fixed::<4>(old, new, pop, pos, key),
            5 => set_realloc_insert_fixed::<5>(old, new, pop, pos, key),
            6 => set_realloc_insert_fixed::<6>(old, new, pop, pos, key),
            _ => set_realloc_insert_fixed::<7>(old, new, pop, pos, key),
        }
    }
}

/// Copies a set leaf with compile-time known key width into `new`.
///
/// # Safety
///
/// `old` must be a live set leaf of `pop` keys of `KB` bytes;
/// `new` must be a fresh allocation of `size_set(KB, pop+1)` bytes; `pos <= pop`.
#[inline(always)]
pub(crate) unsafe fn set_realloc_insert_fixed<const KB: usize>(
    old: *const u8,
    new: *mut u8,
    pop: usize,
    pos: usize,
    key: u64,
) {
    // SAFETY: bounds per contract; the two allocations are disjoint.
    unsafe {
        if pos > 0 {
            core::ptr::copy_nonoverlapping(old, new, pos * KB);
        }
        crate::mutate::write_packed_fixed::<KB>(new, pos, key);
        if pos < pop {
            core::ptr::copy_nonoverlapping(
                old.add(pos * KB),
                new.add((pos + 1) * KB),
                (pop - pos) * KB,
            );
        }
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
#[inline(always)]
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
        if pos > 0 {
            core::ptr::copy_nonoverlapping(old, new, pos * kb);
        }
        if pos + 1 < pop {
            core::ptr::copy_nonoverlapping(
                old.add((pos + 1) * kb),
                new.add(pos * kb),
                (pop - 1 - pos) * kb,
            );
        }
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
#[inline(always)]
pub(crate) unsafe fn map_realloc_insert(
    old: *const u8,
    new: *mut u8,
    key_bytes: u8,
    pop: usize,
    pos: usize,
    key: u64,
    val: u64,
) {
    // SAFETY: caller guarantees old/new pointers and pop/pos bounds for key_bytes.
    unsafe {
        match key_bytes {
            1 => map_realloc_insert_fixed::<1>(old, new, pop, pos, key, val),
            2 => map_realloc_insert_fixed::<2>(old, new, pop, pos, key, val),
            3 => map_realloc_insert_fixed::<3>(old, new, pop, pos, key, val),
            4 => map_realloc_insert_fixed::<4>(old, new, pop, pos, key, val),
            5 => map_realloc_insert_fixed::<5>(old, new, pop, pos, key, val),
            6 => map_realloc_insert_fixed::<6>(old, new, pop, pos, key, val),
            _ => map_realloc_insert_fixed::<7>(old, new, pop, pos, key, val),
        }
    }
}

/// Copies a map leaf with compile-time known key width into `new`.
#[inline(always)]
pub(crate) unsafe fn map_realloc_insert_fixed<const KB: usize>(
    old: *const u8,
    new: *mut u8,
    pop: usize,
    pos: usize,
    key: u64,
    val: u64,
) {
    // SAFETY: bounds per contract; the two allocations are disjoint.
    unsafe {
        let ov = old.cast::<u64>();
        let nv = new.cast::<u64>();
        if pos > 0 {
            core::ptr::copy_nonoverlapping(ov, nv, pos);
        }
        nv.add(pos).write(val);
        if pos < pop {
            core::ptr::copy_nonoverlapping(ov.add(pos), nv.add(pos + 1), pop - pos);
        }
        let ok = old.add(map_keys_offset(pop));
        let nk = new.add(map_keys_offset(pop + 1));
        if pos > 0 {
            core::ptr::copy_nonoverlapping(ok, nk, pos * KB);
        }
        crate::mutate::write_packed_fixed::<KB>(nk, pos, key);
        if pos < pop {
            core::ptr::copy_nonoverlapping(
                ok.add(pos * KB),
                nk.add((pos + 1) * KB),
                (pop - pos) * KB,
            );
        }
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
#[inline(always)]
pub(crate) unsafe fn search_fixed<const KB: usize>(
    keys: *const u8,
    pop: usize,
    key: Key,
) -> Option<usize> {
    // SAFETY: caller guarantees keys holds at least KB * pop readable bytes in its class allocation.
    unsafe {
        if KB == 1 && pop == 16 {
            crate::bits::search_16_u8(keys, 16, key as u8)
        } else if KB == 1 && pop == 8 {
            crate::bits::search_8_u8(keys, 8, key as u8)
        } else if KB == 2 && pop == 8 {
            crate::bits::search_8_u16(keys, 8, key as u16)
        } else if KB == 4 && pop == 4 {
            crate::bits::search_4_u32(keys, 4, key as u32)
        } else {
            let needle = crate::mutate::key_low(key, KB as u8);
            let pos = lower_bound_fixed::<KB>(keys, pop, needle);
            if pos < pop && crate::mutate::read_packed_fixed::<KB>(keys, pos) == needle {
                Some(pos)
            } else {
                None
            }
        }
    }
}

/// Finds the slot of `key`'s low `key_bytes` bytes among `pop` packed keys.
///
/// # Safety
///
/// `keys` must be valid for reads of `key_bytes * pop` bytes.
#[inline(always)]
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
    fn simd_gate_safety() {
        // lower_bound_fixed / search_fixed have SIMD fast-paths.
        // For a pop in a gate range, `cap_class(pop) * KB` must be >= load width.
        for pop in 13..=16 {
            let kb = 1;
            assert!(cap_class(pop) * kb >= 16);
        }
        for pop in 5..=8 {
            let kb = 1;
            assert!(cap_class(pop) * kb >= 8);
        }
        for pop in 5..=8 {
            let kb = 2;
            assert!(cap_class(pop) * kb >= 16);
        }
        for pop in 3..=4 {
            let kb = 4;
            assert!(cap_class(pop) * kb >= 16);
        }

        // search_fixed (point queries)
        assert!(cap_class(16) >= 16);
        assert!(cap_class(8) >= 8);
        assert!(cap_class(8) * 2 >= 16);
        assert!(cap_class(4) * 4 >= 16);
    }

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
    fn simd_gates_within_cap_class() {
        // Mirrors the dispatch gates in `lower_bound_fixed`: (KB, pop
        // range, fixed load width in bytes). A gate whose smallest
        // cap_class-derived key area is narrower than its kernel's load
        // width reads out of bounds (the pop 9..=12 ASan overflow,
        // crash-7048e639). Update this table in lockstep with the gates.
        const GATES: &[(usize, core::ops::RangeInclusive<usize>, usize)] = &[
            (1, 13..=16, 16), // lower_bound_16_u8
            (1, 5..=8, 8),    // lower_bound_8_u8
            (2, 5..=8, 16),   // lower_bound_8_u16
            (4, 3..=4, 16),   // lower_bound_4_u32
        ];
        for (kb, pops, load_width) in GATES {
            for pop in pops.clone() {
                assert!(
                    cap_class(pop) * kb >= *load_width,
                    "gate KB={kb} pop={pop}: cap_class yields {} key bytes, kernel loads {load_width}",
                    cap_class(pop) * kb,
                );
            }
        }
    }

    #[test]
    fn lower_bound_fixed_parity_at_exact_capacity() {
        // Runs every population through `lower_bound_fixed::<1>` with the
        // key area allocated at exactly cap_class(pop) bytes — the same
        // guarantee real leaves provide — and checks parity against a
        // scalar reference. Covers the pop 9..=12 fallback path that the
        // vectorized gate must not claim.
        for pop in 0..=32usize {
            let cap = cap_class(pop).max(pop);
            let mut buf = vec![0u8; cap];
            for (i, b) in buf.iter_mut().enumerate().take(pop) {
                *b = (i * 7 + 3) as u8; // strictly increasing, sorted
            }
            for needle in 0..=255u64 {
                let expected = buf[..pop]
                    .iter()
                    .filter(|&&k| u64::from(k) < needle)
                    .count();
                // SAFETY: buf holds cap_class(pop) readable bytes, sorted.
                let got = unsafe { lower_bound_fixed::<1>(buf.as_ptr(), pop, needle) };
                assert_eq!(got, expected, "pop={pop} needle={needle}");
            }
        }
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
    fn locate_all_key_sizes() {
        for kb in 1u8..=7 {
            let max_val = if kb >= 8 {
                u64::MAX
            } else {
                (1u64 << (8 * kb)) - 1
            };
            for count in 0..=20 {
                let mut keys: Vec<u64> = (0..count).map(|i| (i * 7 + 10) & max_val).collect();
                keys.sort_unstable();
                keys.dedup();
                let pop = keys.len();
                let mut packed = Vec::new();
                for k in &keys {
                    packed.extend_from_slice(&k.to_le_bytes()[..kb as usize]);
                }
                // Pad to capacity class
                let cap = cap_class(pop);
                packed.resize(cap * kb as usize, 0);

                let ptr = if packed.is_empty() {
                    core::ptr::NonNull::<u8>::dangling().as_ptr()
                } else {
                    packed.as_ptr()
                };

                // Test existing keys
                for (idx, &k) in keys.iter().enumerate() {
                    // SAFETY: `ptr` holds at least `pop * kb` valid bytes.
                    let res = unsafe { locate(ptr, pop, kb, k) };
                    assert_eq!(res, Ok(idx), "kb={kb} pop={pop} key={k}");
                }

                // Test probe before, between, and after keys
                let probes = [0, 5, 15, 50, 100, max_val];
                for &needle in &probes {
                    let needle = needle & max_val;
                    // SAFETY: `ptr` holds at least `pop * kb` valid bytes.
                    let res = unsafe { locate(ptr, pop, kb, needle) };
                    let expected_idx = keys.binary_search(&needle);
                    assert_eq!(res, expected_idx, "kb={kb} pop={pop} probe={needle}");
                }
            }
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
