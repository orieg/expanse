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
//! - Map flavor: `[values: u64 × cap][keys: L × cap]` in one allocation.
//!   Values first keeps them 8-aligned for free (raw allocations are
//!   `RAW_ALIGN` = 16 aligned) with no padding arithmetic.
//!
//! Both areas are sized by `cap_class(pop)`, not `pop`, so an insert or
//! removal that stays inside a capacity class shifts in place. Search is
//! per key width (`search_fixed`, `lower_bound_fixed`): a 128-bit SIMD
//! or 64-bit-load kernel at the populations whose capacity class covers the
//! bytes that load reads, unrolled compares at `pop <= 4`, and binary
//! search otherwise.

use crate::types::Key;

/// Slot-capacity class: leaf and subarray allocations follow a tail-coarsened
/// schedule: populations of 1 or 2 stay exact, populations 3..=16 round to
/// multiples of 4 slots (4, 8, 12, 16), and mature populations 17..=32 round
/// in steps of 8 slots (24, 32). This halves mature-leaf reallocation
/// boundaries while staying strictly within the 1M random memory budget gate
/// (< 9.0 B/key set, < 18.0 B/key map). Allocation sizes stay derivable from
/// the current population alone — `free` needs no stored capacity.
#[inline]
#[must_use]
pub const fn cap_class(pop: usize) -> usize {
    if pop <= 2 {
        pop
    } else if pop <= 16 {
        (pop + 3) & !3
    } else if pop <= 24 {
        24
    } else if pop <= 32 {
        32
    } else {
        (pop + 3) & !3
    }
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
#[inline]
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
#[inline]
pub(crate) unsafe fn lower_bound_fixed<const KB: usize>(
    keys: *const u8,
    pop: usize,
    needle: u64,
) -> usize {
    // Every vectorized branch below must satisfy: cap_class(pop) * KB >=
    // the kernel's fixed load width (see `simd_gates_within_cap_class`).
    // cap_class rounds to multiples of FOUR over the range these gates cover
    // (3..=16), so pop 9..=12 only guarantees 12 slots — gating those into the
    // 16-byte kernel read out of bounds. Above 16 the ladder coarsens to
    // {24, 32} (#826), which only ever *raises* the slot count a population is
    // guaranteed, so every bound below stays satisfied by construction.
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
    } else if pop <= 4 {
        if pop == 0 {
            0
        } else if KB == 1 {
            let n = needle as u8;
            // SAFETY: keys is valid for pop bytes.
            let c0 = unsafe { (*keys < n) as usize };
            if pop == 1 {
                c0
            } else if pop == 2 {
                // SAFETY: keys holds 2 bytes.
                c0 + unsafe { (*keys.add(1) < n) as usize }
            } else if pop == 3 {
                // SAFETY: keys holds 3 bytes.
                c0 + unsafe { (*keys.add(1) < n) as usize } + unsafe { (*keys.add(2) < n) as usize }
            } else {
                // SAFETY: keys holds 4 bytes.
                c0 + unsafe { (*keys.add(1) < n) as usize }
                    + unsafe { (*keys.add(2) < n) as usize }
                    + unsafe { (*keys.add(3) < n) as usize }
            }
        } else if KB == 2 {
            let n = needle as u16;
            // SAFETY: keys holds at least 2 bytes.
            let c0 = unsafe { ((keys as *const u16).read_unaligned() < n) as usize };
            if pop == 1 {
                c0
            } else if pop == 2 {
                // SAFETY: keys holds 4 bytes.
                c0 + unsafe { ((keys.add(2) as *const u16).read_unaligned() < n) as usize }
            } else if pop == 3 {
                // SAFETY: keys holds 6 bytes.
                c0 + unsafe { ((keys.add(2) as *const u16).read_unaligned() < n) as usize }
                    + unsafe { ((keys.add(4) as *const u16).read_unaligned() < n) as usize }
            } else {
                // SAFETY: keys holds 8 bytes.
                c0 + unsafe { ((keys.add(2) as *const u16).read_unaligned() < n) as usize }
                    + unsafe { ((keys.add(4) as *const u16).read_unaligned() < n) as usize }
                    + unsafe { ((keys.add(6) as *const u16).read_unaligned() < n) as usize }
            }
        } else if KB == 4 {
            let n = needle as u32;
            // SAFETY: keys holds at least 4 bytes.
            let c0 = unsafe { ((keys as *const u32).read_unaligned() < n) as usize };
            if pop == 1 {
                c0
            } else if pop == 2 {
                // SAFETY: keys holds 8 bytes.
                c0 + unsafe { ((keys.add(4) as *const u32).read_unaligned() < n) as usize }
            } else if pop == 3 {
                // SAFETY: keys holds 12 bytes.
                c0 + unsafe { ((keys.add(4) as *const u32).read_unaligned() < n) as usize }
                    + unsafe { ((keys.add(8) as *const u32).read_unaligned() < n) as usize }
            } else {
                // SAFETY: keys holds 16 bytes.
                c0 + unsafe { ((keys.add(4) as *const u32).read_unaligned() < n) as usize }
                    + unsafe { ((keys.add(8) as *const u32).read_unaligned() < n) as usize }
                    + unsafe { ((keys.add(12) as *const u32).read_unaligned() < n) as usize }
            }
        } else {
            // SAFETY: keys holds pop * KB bytes.
            let c0 = unsafe { (crate::mutate::read_packed_fixed::<KB>(keys, 0) < needle) as usize };
            if pop == 1 {
                c0
            } else if pop == 2 {
                // SAFETY: keys holds 2 entries.
                c0 + unsafe { (crate::mutate::read_packed_fixed::<KB>(keys, 1) < needle) as usize }
            } else if pop == 3 {
                // SAFETY: keys holds 3 entries.
                c0 + unsafe { (crate::mutate::read_packed_fixed::<KB>(keys, 1) < needle) as usize }
                    + unsafe { (crate::mutate::read_packed_fixed::<KB>(keys, 2) < needle) as usize }
            } else {
                // SAFETY: keys holds 4 entries.
                c0 + unsafe { (crate::mutate::read_packed_fixed::<KB>(keys, 1) < needle) as usize }
                    + unsafe { (crate::mutate::read_packed_fixed::<KB>(keys, 2) < needle) as usize }
                    + unsafe { (crate::mutate::read_packed_fixed::<KB>(keys, 3) < needle) as usize }
            }
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
        } else if pop <= 4 {
            let needle = crate::mutate::key_low(key, KB as u8);
            if KB == 1 {
                let n = needle as u8;
                if pop >= 1 && *keys == n {
                    return Some(0);
                }
                if pop >= 2 && *keys.add(1) == n {
                    return Some(1);
                }
                if pop >= 3 && *keys.add(2) == n {
                    return Some(2);
                }
                if pop >= 4 && *keys.add(3) == n {
                    return Some(3);
                }
                return None;
            }
            if KB == 2 {
                let n = needle as u16;
                if pop >= 1 && (keys as *const u16).read_unaligned() == n {
                    return Some(0);
                }
                if pop >= 2 && (keys.add(2) as *const u16).read_unaligned() == n {
                    return Some(1);
                }
                if pop >= 3 && (keys.add(4) as *const u16).read_unaligned() == n {
                    return Some(2);
                }
                if pop >= 4 && (keys.add(6) as *const u16).read_unaligned() == n {
                    return Some(3);
                }
                return None;
            }
            if KB == 4 {
                let n = needle as u32;
                if pop >= 1 && (keys as *const u32).read_unaligned() == n {
                    return Some(0);
                }
                if pop >= 2 && (keys.add(4) as *const u32).read_unaligned() == n {
                    return Some(1);
                }
                if pop >= 3 && (keys.add(8) as *const u32).read_unaligned() == n {
                    return Some(2);
                }
                if pop >= 4 && (keys.add(12) as *const u32).read_unaligned() == n {
                    return Some(3);
                }
                return None;
            }
            if pop >= 1 && crate::mutate::read_packed_fixed::<KB>(keys, 0) == needle {
                return Some(0);
            }
            if pop >= 2 && crate::mutate::read_packed_fixed::<KB>(keys, 1) == needle {
                return Some(1);
            }
            if pop >= 3 && crate::mutate::read_packed_fixed::<KB>(keys, 2) == needle {
                return Some(2);
            }
            if pop >= 4 && crate::mutate::read_packed_fixed::<KB>(keys, 3) == needle {
                return Some(3);
            }
            None
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

/// Packed leaf keys on a shared tree's paths (#1086, class 1, stage (d);
/// `docs/ARCHITECTURE.md` §4.2). A shared tree's readers search a leaf's key
/// area while its writer shifts it, so both sides access the area as the
/// aligned words that cover it: a key is extracted from one or two word
/// loads by shift, and an in-place insert or removal rewrites the covering
/// words. A word wholly inside the area is one 8-byte atomic
/// (`bits::shared_word`); the area's last, partial word is 4-, 2- and
/// 1-byte atomics at aligned offsets ([`tail_load`], [`tail_store`]), so no
/// access reaches past the area, which ends the allocation. Both sides
/// derive the area from the leaf's capacity class, `kb * cap_class(pop)`,
/// which an in-place edit never changes, so every race is between accesses
/// of one size at one address. No SIMD: stable Rust has no 128-bit atomic.
/// The plain paths keep [`search`], [`lower_bound`] and the in-place helpers
/// above, and leaf sizes are the same on plain and shared trees.
///
/// A key area starts 8-aligned: a set leaf's at the allocation, a map
/// leaf's at [`map_keys_offset`], a multiple of 8.
pub(crate) mod shared_keys {
    use crate::bits::shared_word;

    use core::sync::atomic::{AtomicU8, AtomicU16, AtomicU32, Ordering::Relaxed};

    /// The low `kb` bytes of a word.
    #[cfg(feature = "std")]
    #[inline(always)]
    const fn mask(kb: usize) -> u64 {
        (1u64 << (kb * 8)) - 1
    }

    /// Bytes of the key area of a leaf of `pop` keys of `kb` bytes: its
    /// capacity class's, which an in-place edit never changes.
    #[inline(always)]
    fn area(kb: usize, pop: usize) -> usize {
        kb * super::cap_class(pop)
    }

    /// The area's last, partial word: `t` (1..=7) bytes at the 8-aligned
    /// `p`, loaded as 4-, 2- and 1-byte atomics in that order.
    ///
    /// # Safety
    ///
    /// `p` is 8-aligned with `t` readable bytes and write permission.
    #[inline]
    pub(crate) unsafe fn tail_load(p: *const u8, t: usize) -> u64 {
        let p = p.cast_mut();
        let (mut v, mut o) = (0u64, 0usize);
        // SAFETY: caller contract; each piece is aligned to its size, since
        // `p` is 8-aligned and the pieces go 4, 2, 1.
        unsafe {
            if t & 4 != 0 {
                v |= u64::from(AtomicU32::from_ptr(p.cast::<u32>()).load(Relaxed));
                o = 4;
            }
            if t & 2 != 0 {
                v |=
                    u64::from(AtomicU16::from_ptr(p.add(o).cast::<u16>()).load(Relaxed)) << (o * 8);
                o += 2;
            }
            if t & 1 != 0 {
                v |= u64::from(AtomicU8::from_ptr(p.add(o)).load(Relaxed)) << (o * 8);
            }
        }
        v
    }

    /// [`tail_load`]'s store: the low `t` bytes of `v` at `p`, in the same
    /// pieces.
    ///
    /// # Safety
    ///
    /// As [`tail_load`], and the caller is the area's one writer.
    #[inline]
    pub(crate) unsafe fn tail_store(p: *mut u8, t: usize, v: u64) {
        let mut o = 0usize;
        // SAFETY: as in `tail_load`.
        unsafe {
            if t & 4 != 0 {
                AtomicU32::from_ptr(p.cast::<u32>()).store(v as u32, Relaxed);
                o = 4;
            }
            if t & 2 != 0 {
                AtomicU16::from_ptr(p.add(o).cast::<u16>()).store((v >> (o * 8)) as u16, Relaxed);
                o += 2;
            }
            if t & 1 != 0 {
                AtomicU8::from_ptr(p.add(o)).store((v >> (o * 8)) as u8, Relaxed);
            }
        }
    }

    /// Word `w` of an area of `area` bytes: one 8-byte atomic when the word
    /// lies inside the area, the tail's pieces otherwise.
    ///
    /// # Safety
    ///
    /// `keys` is an 8-aligned area of `area` bytes with write permission, and
    /// `w * 8 < area`.
    #[inline(always)]
    unsafe fn word(keys: *const u8, w: usize, area: usize) -> u64 {
        let p = keys.wrapping_add(w * 8);
        if w * 8 + 8 <= area {
            // SAFETY: caller contract; a whole word inside the area.
            unsafe { shared_word::load::<true>(p.cast::<u64>().cast_mut()) }
        } else {
            // SAFETY: caller contract; the `area - w * 8` bytes that remain.
            unsafe { tail_load(p, area - w * 8) }
        }
    }

    /// Key `i` of `kb` bytes in an area of `area` bytes.
    ///
    /// # Safety
    ///
    /// As [`word`], and key `i` lies inside the area.
    #[cfg(feature = "std")]
    #[inline(always)]
    unsafe fn read_in(keys: *const u8, i: usize, kb: usize, area: usize) -> u64 {
        let off = i * kb;
        let w = off / 8;
        let sh = (off & 7) * 8;
        // SAFETY: caller contract; the word holding the key's first byte.
        let lo = unsafe { word(keys, w, area) } >> sh;
        let v = if sh + kb * 8 > 64 {
            // SAFETY: the key's last byte is in the next word, inside the
            // area.
            lo | (unsafe { word(keys, w + 1, area) } << (64 - sh))
        } else {
            lo
        };
        v & mask(kb)
    }

    /// Key `i` of `kb` bytes of a leaf of `pop` keys.
    ///
    /// # Safety
    ///
    /// `keys` is the 8-aligned key area of a live leaf of `pop` keys, with
    /// write permission (the loads form atomic references), and `i < pop`.
    #[cfg(feature = "std")]
    #[inline(always)]
    pub(crate) unsafe fn read(keys: *const u8, i: usize, kb: usize, pop: usize) -> u64 {
        // SAFETY: forwarded.
        unsafe { read_in(keys, i, kb, area(kb, pop)) }
    }

    /// Key `i` of `KB` bytes in an area whose words are all whole (`FULL`:
    /// `area` is a multiple of 8, so no word is a tail) or not.
    ///
    /// # Safety
    ///
    /// As [`read_in`].
    #[cfg(feature = "std")]
    #[inline(always)]
    unsafe fn read_fixed<const KB: usize, const FULL: bool>(
        keys: *const u8,
        i: usize,
        area: usize,
    ) -> u64 {
        if !FULL {
            // SAFETY: forwarded.
            return unsafe { read_in(keys, i, KB, area) };
        }
        let off = i * KB;
        let w = keys.wrapping_add(off & !7).cast::<u64>().cast_mut();
        let sh = (off & 7) * 8;
        // SAFETY: caller contract; every word of the area is whole.
        let lo = unsafe { shared_word::load::<true>(w) } >> sh;
        let v = if sh + KB * 8 > 64 {
            // SAFETY: as above; the key's last byte is in the next word.
            lo | (unsafe { shared_word::load::<true>(w.add(1)) } << (64 - sh))
        } else {
            lo
        };
        v & mask(KB)
    }

    /// First slot of `pop` sorted keys `>= needle`, and whether that slot
    /// holds `needle`: one search per key width, with the tail test taken
    /// once per call and a linear scan for up to four keys, as the plain
    /// [`super::search`] specialises.
    ///
    /// # Safety
    ///
    /// As [`read`], for `pop` keys; `needle` is masked to `KB` bytes.
    #[cfg(feature = "std")]
    #[inline(always)]
    unsafe fn seek_fixed<const KB: usize, const FULL: bool>(
        keys: *const u8,
        pop: usize,
        area: usize,
        needle: u64,
    ) -> (usize, bool) {
        if pop <= 4 {
            for i in 0..pop {
                // SAFETY: `i < pop`.
                let v = unsafe { read_fixed::<KB, FULL>(keys, i, area) };
                if v >= needle {
                    return (i, v == needle);
                }
            }
            return (pop, false);
        }
        let (mut lo, mut hi) = (0usize, pop);
        while lo < hi {
            let mid = (lo + hi) / 2;
            // SAFETY: `mid < pop`.
            let v = unsafe { read_fixed::<KB, FULL>(keys, mid, area) };
            if v < needle {
                lo = mid + 1;
            } else if v == needle {
                // Keys are unique: the first slot `>= needle`.
                return (mid, true);
            } else {
                hi = mid;
            }
        }
        (lo, false)
    }

    /// [`seek_fixed`] over an area whose last word is partial. The keys
    /// that lie wholly inside the area's whole words are searched by
    /// whole-word reads; the keys that reach the partial word, at most
    /// seven, are scanned after them, when the needle is above the last of
    /// the others. The tail test is then taken once per search instead of on
    /// every read (#1191).
    ///
    /// # Safety
    ///
    /// As [`seek_fixed`].
    #[cfg(feature = "std")]
    #[inline(always)]
    unsafe fn seek_split<const KB: usize>(
        keys: *const u8,
        pop: usize,
        area: usize,
        needle: u64,
    ) -> (usize, bool) {
        if pop <= 4 {
            // SAFETY: forwarded.
            return unsafe { seek_fixed::<KB, false>(keys, pop, area, needle) };
        }
        // Keys `0..whole_keys` end at or below the area's last whole word.
        let whole_keys = ((area & !7) / KB).min(pop);
        if whole_keys > 0 {
            // SAFETY: `whole_keys - 1 < pop`, and its bytes lie in whole
            // words of the area.
            let v = unsafe { read_fixed::<KB, true>(keys, whole_keys - 1, area) };
            if v >= needle {
                // SAFETY: every key below `whole_keys` lies in whole words.
                return unsafe { seek_fixed::<KB, true>(keys, whole_keys, area, needle) };
            }
        }
        for i in whole_keys..pop {
            // SAFETY: `i < pop`.
            let v = unsafe { read_in(keys, i, KB, area) };
            if v >= needle {
                return (i, v == needle);
            }
        }
        (pop, false)
    }

    /// [`seek_fixed`] for a key width known only at run time.
    ///
    /// # Safety
    ///
    /// As [`seek_fixed`], with `1 <= kb <= 7`.
    #[cfg(feature = "std")]
    #[inline(always)]
    unsafe fn seek(keys: *const u8, pop: usize, kb: usize, needle: u64) -> (usize, bool) {
        let area = area(kb, pop);
        macro_rules! by_width {
            ($($k:literal)*) => {
                match kb {
                    $($k => {
                        // SAFETY: forwarded.
                        unsafe {
                            if area & 7 == 0 {
                                seek_fixed::<$k, true>(keys, pop, area, needle)
                            } else {
                                seek_split::<$k>(keys, pop, area, needle)
                            }
                        }
                    })*
                    _ => unreachable!("a packed leaf key is 1..=7 bytes"),
                }
            };
        }
        by_width!(1 2 3 4 5 6 7)
    }

    /// First slot of `pop` sorted keys whose key is `>= needle` (masked).
    ///
    /// # Safety
    ///
    /// As [`read`], for `pop` keys.
    #[cfg(feature = "std")]
    #[inline]
    pub(crate) unsafe fn lower_bound(keys: *const u8, pop: usize, kb: usize, needle: u64) -> usize {
        // SAFETY: forwarded.
        unsafe { seek(keys, pop, kb, needle) }.0
    }

    /// The slot of `key`'s low `kb` bytes, as [`super::search`].
    ///
    /// # Safety
    ///
    /// As [`lower_bound`].
    #[cfg(feature = "std")]
    #[inline]
    pub(crate) unsafe fn find(keys: *const u8, pop: usize, kb: usize, key: u64) -> Option<usize> {
        // SAFETY: forwarded.
        let (at, hit) = unsafe { seek(keys, pop, kb, key & mask(kb)) };
        hit.then_some(at)
    }

    /// The low `n` bytes of a word, for `n` in `0..=8`.
    #[inline(always)]
    const fn low_bytes(n: usize) -> u64 {
        if n >= 8 { !0 } else { (1u64 << (n * 8)) - 1 }
    }

    /// The bytes of the word at area byte `base` that lie below area byte
    /// `lim`.
    #[inline(always)]
    const fn below(lim: usize, base: usize) -> u64 {
        low_bytes(lim.saturating_sub(base))
    }

    /// Word `w` of an area whose first `whole` words are whole: one 8-byte
    /// atomic below `whole`, the tail's pieces at `whole` itself. The
    /// callers walk the words upwards, so only their last word can be the
    /// tail, and the test is against a bound held in a register.
    ///
    /// # Safety
    ///
    /// As [`word`], with `whole == area / 8`.
    #[inline(always)]
    unsafe fn load_w(keys: *const u8, w: usize, whole: usize, area: usize) -> u64 {
        let p = keys.wrapping_add(w * 8);
        if w < whole {
            // SAFETY: caller contract; a whole word inside the area.
            unsafe { shared_word::load::<true>(p.cast::<u64>().cast_mut()) }
        } else {
            // SAFETY: caller contract; the `area - w * 8` bytes that remain.
            unsafe { tail_load(p, area - w * 8) }
        }
    }

    /// [`load_w`]'s store.
    ///
    /// # Safety
    ///
    /// As [`load_w`], and the caller is the area's one writer.
    #[inline(always)]
    unsafe fn store_w(keys: *mut u8, w: usize, whole: usize, area: usize, v: u64) {
        let p = keys.wrapping_add(w * 8);
        if w < whole {
            // SAFETY: as in `load_w`.
            unsafe { shared_word::store::<true>(p.cast::<u64>(), v) }
        } else {
            // SAFETY: as in `load_w`.
            unsafe { tail_store(p, area - w * 8, v) }
        }
    }

    /// Inserts `key` at `pos` among `pop` keys, shifting the tail up.
    ///
    /// Each word that covers bytes `pos * kb..(pop + 1) * kb` is loaded
    /// once, rebuilt in a register from itself, the word below it and the
    /// key, and stored once: the loads and stores of a copy through a
    /// buffer, word for word, without the buffer (#1191).
    ///
    /// # Safety
    ///
    /// The area holds `pop + 1` keys (the class is unchanged), `pos <= pop`,
    /// and the caller is its one writer.
    #[inline]
    pub(crate) unsafe fn insert_at(keys: *mut u8, kb: usize, pop: usize, pos: usize, key: u64) {
        let area = area(kb, pop);
        let whole = area / 8;
        let at = pos * kb;
        let kend = at + kb;
        let end = (pop + 1) * kb;
        debug_assert!(end <= area, "an insert past the key area");
        let s = kb * 8;
        // The key's bit offset in its first word; it spills into the next
        // word when `o + s > 64`.
        let o = (at & 7) * 8;
        let key = key & low_bytes(kb);
        let w0 = at / 8;
        // The old word below the current one. The first word takes no byte
        // from below: its shifted bytes start at `kend`, a key above `base`.
        let mut prev = 0u64;
        for w in w0..end.div_ceil(8) {
            // SAFETY: caller contract; `w * 8 < end <= area`.
            let old = unsafe { load_w(keys, w, whole, area) };
            let base = w * 8;
            // Every byte moved up by one key.
            let shifted = (old << s) | (prev >> (64 - s));
            // The key's bytes in this word (masked to the key's range below).
            let kp = if w == w0 {
                key << o
            } else if o == 0 {
                0
            } else {
                key >> (64 - o)
            };
            let mb = below(at, base);
            let mk = below(kend, base);
            let me = below(end, base);
            let new = (old & (mb | !me)) | (kp & mk & !mb) | (shifted & me & !mk);
            // SAFETY: as the load; the caller is the area's one writer.
            unsafe { store_w(keys, w, whole, area, new) };
            prev = old;
        }
    }

    /// Removes the key at `pos` among `pop` keys, shifting the tail down.
    /// The twin of [`insert_at`]: each word is rebuilt from itself and the
    /// word above it, which is loaded before the word is stored.
    ///
    /// # Safety
    ///
    /// `pos < pop`, and the caller is the area's one writer.
    #[inline]
    pub(crate) unsafe fn remove_at(keys: *mut u8, kb: usize, pop: usize, pos: usize) {
        if pos + 1 >= pop {
            return;
        }
        let area = area(kb, pop);
        let whole = area / 8;
        let at = pos * kb;
        let end = pop * kb;
        // Bytes `at..lim` take the bytes one key above them; the bytes from
        // `lim` to `end` keep the last key, as the plain removal leaves them.
        let lim = end - kb;
        let s = kb * 8;
        let w0 = at / 8;
        // SAFETY: caller contract; `at < end <= area`.
        let mut cur = unsafe { load_w(keys, w0, whole, area) };
        for w in w0..lim.div_ceil(8) {
            let base = w * 8;
            // The word above is read only when it holds a byte below `end`.
            let next = if base + 8 < end {
                // SAFETY: `(w + 1) * 8 < end <= area`.
                unsafe { load_w(keys, w + 1, whole, area) }
            } else {
                0
            };
            let shifted = (cur >> s) | (next << (64 - s));
            let mb = below(at, base);
            let ml = below(lim, base);
            let new = (cur & (mb | !ml)) | (shifted & ml & !mb);
            // SAFETY: `w * 8 < lim < end <= area`; the caller is the writer.
            unsafe { store_w(keys, w, whole, area, new) };
            cur = next;
        }
    }

    /// Every key of a set leaf of `pop` keys, with one slot of headroom (as
    /// `mutate::leaf_keys`).
    ///
    /// # Safety
    ///
    /// As [`read`], for `pop` keys.
    #[cfg(feature = "std")]
    pub(crate) unsafe fn keys_vec(
        base: *const u8,
        kb: u8,
        pop: usize,
    ) -> core_alloc::vec::Vec<u64> {
        let mut out = core_alloc::vec::Vec::with_capacity(pop + 1);
        // SAFETY: forwarded.
        out.extend((0..pop).map(|i| unsafe { read(base, i, kb as usize, pop) }));
        out
    }

    /// Every `(key, value)` of a map leaf of `pop` entries, with one slot of
    /// headroom (as `mutate_map::read_map_leaf`).
    ///
    /// # Safety
    ///
    /// As [`read`], for a live map leaf of `pop` entries.
    #[cfg(feature = "std")]
    pub(crate) unsafe fn map_entries(
        base: *const u8,
        kb: u8,
        pop: usize,
    ) -> core_alloc::vec::Vec<(u64, u64)> {
        let keys = base.wrapping_add(super::map_keys_offset(pop));
        let mut out = core_alloc::vec::Vec::with_capacity(pop + 1);
        // SAFETY: forwarded; `pop` values then `pop` keys.
        out.extend((0..pop).map(|i| unsafe {
            (
                read(keys, i, kb as usize, pop),
                shared_word::load::<true>(base.cast::<u64>().add(i).cast_mut()),
            )
        }));
        out
    }

    /// [`super::set_insert_at`] for a shared tree.
    ///
    /// # Safety
    ///
    /// As [`super::set_insert_at`].
    #[inline]
    pub(crate) unsafe fn set_insert_at(base: *mut u8, kb: u8, pop: usize, pos: usize, key: u64) {
        // SAFETY: forwarded.
        unsafe { insert_at(base, kb as usize, pop, pos, key) }
    }

    /// [`super::set_remove_at`] for a shared tree.
    ///
    /// # Safety
    ///
    /// As [`super::set_remove_at`].
    #[inline]
    pub(crate) unsafe fn set_remove_at(base: *mut u8, kb: u8, pop: usize, pos: usize) {
        // SAFETY: forwarded.
        unsafe { remove_at(base, kb as usize, pop, pos) }
    }

    /// [`super::map_insert_at`] for a shared tree: values as atomic words,
    /// keys as above.
    ///
    /// # Safety
    ///
    /// As [`super::map_insert_at`].
    #[inline]
    pub(crate) unsafe fn map_insert_at(
        base: *mut u8,
        kb: u8,
        pop: usize,
        pos: usize,
        key: u64,
        val: u64,
    ) {
        // SAFETY: forwarded; the class is unchanged, so both areas keep
        // their offsets and hold `pop + 1` entries.
        unsafe {
            let vals = base.cast::<u64>();
            shared_word::shift_up::<true>(vals, pos, pop - pos);
            shared_word::store::<true>(vals.add(pos), val);
            insert_at(
                base.add(super::map_keys_offset(pop)),
                kb as usize,
                pop,
                pos,
                key,
            );
        }
    }

    /// [`super::map_remove_at`] for a shared tree.
    ///
    /// # Safety
    ///
    /// As [`super::map_remove_at`].
    #[inline]
    pub(crate) unsafe fn map_remove_at(base: *mut u8, kb: u8, pop: usize, pos: usize) {
        // SAFETY: forwarded.
        unsafe {
            let vals = base.cast::<u64>();
            shared_word::shift_down::<true>(vals, pos, pop - 1 - pos);
            remove_at(base.add(super::map_keys_offset(pop)), kb as usize, pop, pos);
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
        // Class-based sizing: populations of 1 or 2 stay exact; populations
        // 3..=16 round to multiples of 4 (4, 8, 12, 16); populations 17..=32
        // round in steps of 8 (24, 32).
        assert_eq!(cap_class(0), 0);
        assert_eq!(cap_class(1), 1);
        assert_eq!(cap_class(2), 2);
        assert_eq!(cap_class(3), 4);
        assert_eq!(cap_class(4), 4);
        assert_eq!(cap_class(5), 8);
        assert_eq!(cap_class(8), 8);
        assert_eq!(cap_class(9), 12);
        assert_eq!(cap_class(12), 12);
        assert_eq!(cap_class(13), 16);
        assert_eq!(cap_class(16), 16);
        assert_eq!(cap_class(17), 24);
        assert_eq!(cap_class(20), 24);
        assert_eq!(cap_class(24), 24);
        assert_eq!(cap_class(25), 32);
        assert_eq!(cap_class(28), 32);
        assert_eq!(cap_class(32), 32);
        assert_eq!(size_set(1, 25), 32);
        // Key areas are exact, on shared trees too: the shared key helpers
        // cover a partial last word with narrower atomics (#1086).
        assert_eq!(size_set(7, 2), 14);
        assert_eq!(size_set(7, 12), 84);
        assert_eq!(size_map(1, 25), 8 * 32 + 32);
        assert_eq!(size_map(7, 2), 8 * 2 + 7 * 2);
        assert_eq!(size_map(3, 12), 8 * 12 + 36);
        assert_eq!(map_keys_offset(3), 32);
        assert_eq!(map_keys_offset(4), 32);
        assert_eq!(map_keys_offset(5), 64);
        assert_eq!(map_keys_offset(17), 192);
        assert_eq!(map_keys_offset(25), 256);
    }

    /// The shared key helpers agree with the plain ones on every width, pop
    /// and position: reads, lower bounds and searches over an area, and
    /// in-place inserts and removals leave the same bytes. In the Tier-1
    /// Miri lane (`leaf::`), so the atomic word accesses are checked there.
    /// An allocation of exactly `n` bytes at 8-byte alignment, freed on drop
    /// (so a panicking assertion does not leak it under LeakSanitizer).
    struct ExactArea(*mut u8, core::alloc::Layout);

    impl ExactArea {
        fn new(n: usize) -> Self {
            let layout = core::alloc::Layout::from_size_align(n.max(1), 8).expect("layout");
            // SAFETY: non-zero size.
            let p = unsafe { std::alloc::alloc_zeroed(layout) };
            assert!(!p.is_null(), "allocation failed");
            Self(p, layout)
        }

        fn free(self) {
            drop(self);
        }
    }

    impl Drop for ExactArea {
        fn drop(&mut self) {
            // SAFETY: allocated in `new` with this layout, freed once.
            unsafe { std::alloc::dealloc(self.0, self.1) };
        }
    }

    /// The shared in-place insert and removal, at every position of every
    /// population up to 32 keys, over an allocation of exactly the key area,
    /// leave the bytes the plain ones leave. Each word is rebuilt in a
    /// register (#1191), so this pins the masks at the key's first and last
    /// word and at the area's partial last word; an access past the area
    /// is out of bounds, which the Tier-1 Miri run over `leaf::` reports.
    #[test]
    fn shared_rewrite_matches_plain_in_exact_area() {
        use super::shared_keys as sk;
        for kb in 1..=7usize {
            let m = (1u64 << (kb * 8)) - 1;
            for pop in 1..=32usize {
                let exact = kb * cap_class(pop);
                // Keys whose bytes vary with the slot, so a byte moved to
                // the wrong place shows. Order does not matter to a shift.
                let keys: Vec<u64> = (0..pop as u64)
                    .map(|i| (0x0101_0101_0101_0101u64.wrapping_mul(2 * i + 1) ^ i) & m)
                    .collect();
                let mut plain = vec![0u8; exact + 16];
                for (i, &k) in keys.iter().enumerate() {
                    // SAFETY: in bounds of `plain`.
                    unsafe { crate::mutate::write_packed(plain.as_mut_ptr(), i, kb, k) };
                }
                let load = |buf: &ExactArea| {
                    // SAFETY: `buf` holds `exact` bytes.
                    unsafe { core::slice::from_raw_parts(buf.0, exact) }.to_vec()
                };
                let fresh = || {
                    let buf = ExactArea::new(exact);
                    // SAFETY: both hold at least `exact` bytes.
                    unsafe { core::ptr::copy_nonoverlapping(plain.as_ptr(), buf.0, exact) };
                    buf
                };
                if cap_class(pop + 1) == cap_class(pop) {
                    for pos in 0..=pop {
                        let key = 0xA5A5_A5A5_A5A5_A5A5u64 & m;
                        let mut want = plain.clone();
                        let got = fresh();
                        // SAFETY: the class holds `pop + 1` keys; `pos <= pop`.
                        unsafe {
                            set_insert_at(want.as_mut_ptr(), kb as u8, pop, pos, key);
                            sk::set_insert_at(got.0, kb as u8, pop, pos, key);
                        }
                        let got_bytes = load(&got);
                        got.free();
                        assert_eq!(
                            got_bytes[..(pop + 1) * kb],
                            want[..(pop + 1) * kb],
                            "insert kb {kb} pop {pop} pos {pos}"
                        );
                    }
                }
                for pos in 0..pop {
                    let mut want = plain.clone();
                    let got = fresh();
                    // SAFETY: `pos < pop`.
                    unsafe {
                        set_remove_at(want.as_mut_ptr(), kb as u8, pop, pos);
                        sk::set_remove_at(got.0, kb as u8, pop, pos);
                    }
                    let got_bytes = load(&got);
                    got.free();
                    // The whole area: the bytes past the survivors are left
                    // as the plain removal leaves them too.
                    assert_eq!(
                        got_bytes,
                        want[..exact],
                        "remove kb {kb} pop {pop} pos {pos}"
                    );
                }
            }
        }
    }

    /// The shared search answers what the plain one answers, for every
    /// population up to 32 keys of every width, over an allocation of
    /// exactly the key area: a partial last word is searched apart from the
    /// whole ones (#1191), so every needle position relative to the keys
    /// that reach it is probed.
    #[test]
    fn shared_search_matches_plain_in_exact_area() {
        use super::shared_keys as sk;
        for kb in 1..=7usize {
            let m = (1u64 << (kb * 8)) - 1;
            for pop in 1..=32usize {
                let exact = kb * cap_class(pop);
                // Ascending: every byte of key `i` is `7 * i + 1`.
                let keys: Vec<u64> = (0..pop as u64)
                    .map(|i| (7 * i + 1).wrapping_mul(0x0101_0101_0101_0101) & m)
                    .collect();
                let mut plain = vec![0u8; exact + 16];
                for (i, &k) in keys.iter().enumerate() {
                    // SAFETY: in bounds of `plain`.
                    unsafe { crate::mutate::write_packed(plain.as_mut_ptr(), i, kb, k) };
                }
                let buf = ExactArea::new(exact);
                // SAFETY: both hold at least `exact` bytes.
                unsafe { core::ptr::copy_nonoverlapping(plain.as_ptr(), buf.0, exact) };
                let mut needles = vec![0, m];
                for &k in &keys {
                    needles.extend([k.saturating_sub(1), k, (k + 1) & m]);
                }
                for n in needles {
                    // SAFETY: `pop` keys in both areas.
                    unsafe {
                        assert_eq!(
                            sk::lower_bound(buf.0, pop, kb, n),
                            lower_bound(plain.as_ptr(), pop, kb as u8, n),
                            "lower_bound kb {kb} pop {pop} n {n:#x}"
                        );
                        assert_eq!(
                            sk::find(buf.0, pop, kb, n),
                            search(plain.as_ptr(), pop, kb as u8, n),
                            "find kb {kb} pop {pop} n {n:#x}"
                        );
                    }
                }
                buf.free();
            }
        }
    }

    #[test]
    fn shared_keys_match_plain() {
        use super::shared_keys as sk;
        for kb in 1..=7usize {
            for pop in [3usize, 4, 7, 11, 12, 15, 23, 31] {
                let area = size_set(kb as u8, pop + 1);
                let mut keys: Vec<u64> = (0..pop as u64)
                    .map(|i| (i * 37 + 5) & ((1u64 << (kb * 8)) - 1))
                    .collect();
                keys.sort_unstable();
                keys.dedup();
                let pop = keys.len();
                let mut plain = vec![0u64; area.div_ceil(8) + 1];
                let mut shared = plain.clone();
                for (i, &k) in keys.iter().enumerate() {
                    // SAFETY: in bounds of the word buffers.
                    unsafe {
                        crate::mutate::write_packed(plain.as_mut_ptr().cast(), i, kb, k);
                        crate::mutate::write_packed(shared.as_mut_ptr().cast(), i, kb, k);
                    }
                }
                let sp = shared.as_mut_ptr().cast::<u8>();
                for (i, &k) in keys.iter().enumerate() {
                    // SAFETY: `i < pop`.
                    let got = unsafe { sk::read(sp, i, kb, pop) };
                    assert_eq!(got, k, "read kb {kb} pop {pop} i {i}");
                }
                // The same reads, and an in-place insert and removal, over an
                // allocation of exactly the key area: a covering word that
                // reached past it would be an out-of-bounds access, which the
                // Tier-1 Miri run over `leaf::` reports.
                {
                    let exact = kb * cap_class(pop);
                    let buf = ExactArea::new(exact);
                    // SAFETY: both buffers hold at least `exact` bytes.
                    unsafe {
                        core::ptr::copy_nonoverlapping(plain.as_ptr().cast::<u8>(), buf.0, exact);
                    }
                    for (i, &k) in keys.iter().enumerate() {
                        // SAFETY: `i < pop`, inside the exact area.
                        let got = unsafe { sk::read(buf.0, i, kb, pop) };
                        assert_eq!(got, k, "exact read kb {kb}");
                    }
                    // SAFETY: `pop` keys in the exact area.
                    let hit = unsafe { sk::find(buf.0, pop, kb, keys[pop - 1]) };
                    assert_eq!(hit, Some(pop - 1), "exact find kb {kb} pop {pop}");
                    if cap_class(pop + 1) == cap_class(pop) {
                        // SAFETY: the class holds `pop + 1` keys.
                        unsafe { sk::set_insert_at(buf.0, kb as u8, pop, 0, 0) };
                        // SAFETY: `pop + 1` keys now.
                        unsafe { sk::set_remove_at(buf.0, kb as u8, pop + 1, 0) };
                    }
                    for (i, &k) in keys.iter().enumerate() {
                        // SAFETY: as above.
                        let got = unsafe { sk::read(buf.0, i, kb, pop) };
                        assert_eq!(got, k, "exact round trip kb {kb}");
                    }
                    buf.free();
                }
                for needle in [0u64, 1, 5, 42, 200, (1u64 << (kb * 8)) - 1] {
                    let n = needle & ((1u64 << (kb * 8)) - 1);
                    // SAFETY: `pop` keys in both buffers.
                    unsafe {
                        assert_eq!(
                            sk::lower_bound(sp, pop, kb, n),
                            lower_bound(plain.as_ptr().cast(), pop, kb as u8, n),
                            "lower_bound kb {kb} pop {pop} n {n}"
                        );
                        assert_eq!(
                            sk::find(sp, pop, kb, needle),
                            search(plain.as_ptr().cast(), pop, kb as u8, needle),
                            "search kb {kb} pop {pop}"
                        );
                    }
                }
                // In place only within a class, as the engine inserts.
                let same_class = cap_class(pop + 1) == cap_class(pop);
                for pos in (0..=pop).filter(|_| same_class) {
                    let (mut p2, mut s2) = (plain.clone(), shared.clone());
                    // SAFETY: the buffers hold `pop + 1` keys.
                    unsafe {
                        set_insert_at(p2.as_mut_ptr().cast(), kb as u8, pop, pos, 0x55);
                        sk::set_insert_at(s2.as_mut_ptr().cast(), kb as u8, pop, pos, 0x55);
                    }
                    assert_eq!(p2, s2, "insert kb {kb} pop {pop} pos {pos}");
                }
                for pos in 0..pop {
                    let (mut p2, mut s2) = (plain.clone(), shared.clone());
                    // SAFETY: `pos < pop`.
                    unsafe {
                        set_remove_at(p2.as_mut_ptr().cast(), kb as u8, pop, pos);
                        sk::set_remove_at(s2.as_mut_ptr().cast(), kb as u8, pop, pos);
                    }
                    assert_eq!(
                        p2[..pop * kb / 8],
                        s2[..pop * kb / 8],
                        "remove kb {kb} pop {pop} pos {pos}"
                    );
                }
            }
        }
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
