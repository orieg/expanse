//! Phase 6b: the map-flavor mutation engine (`ExpanseMap` core).
//!
//! Same least-compressed-form ladder and 1-index hysteresis as the
//! set-flavor engine in `mutate` (whose branch machinery — linear slot
//! insertion, node upgrades/downgrades, free/validate — this module
//! shares), with the map-specific terminal forms:
//!
//! - **Immediates** keep their keys in the 7 aux bytes; word 0 holds the
//!   value directly for one key, or a pointer to a packed value array for
//!   several (`mutate::map_immed_max` = `7 / key_bytes` keys).
//! - **Linear leaves** are `[values: u64×pop][keys]` in one allocation.
//! - **Level-1 overflow** goes to `LeafBitmapL` (bitmap + per-subexpanse
//!   value subarrays); there is no map full-expanse — values must exist.
//!
//! Inserts return the replaced value (`Some(old)`) or `None` when the key
//! is new; removes return the removed value.

use crate::alloc::NodeAlloc;
use crate::leaf;
use crate::mutate::{
    BRANCHB_UP, ImmedBuf, LEAF_CAP, LEAF1_CAP, branch_form_level, bump_pop0, decode_value,
    divergence_level, downgrade_b_to_l7, downgrade_l7_to_l3, downgrade_u_to_b, immed_map_keys,
    key_low, linear_insert_slot, linear_remove_slot, map_immed_max, read_packed, restore_decode,
    split_skip, sub_edges_size, sub_vals_size, upgrade_b_to_u, upgrade_l3_to_l7, upgrade_l7_to_b,
    wrap_skip_level, write_decode, write_packed,
};
use crate::node::{BranchB, BranchL3, BranchL7, BranchU, Edge, LeafBitmapL};
use crate::types::{BRANCH_L3_CAP, BRANCH_L7_CAP, EdgeTag, EdgeType, ImmedType, Key, digit};

/// Reads a map immediate's entries (sorted by key).
///
/// # Safety
///
/// For key counts above one, word 0 must point to a live value array.
unsafe fn read_map_immed(edge: &Edge, im: ImmedType) -> ImmedBuf<(u64, u64)> {
    let keys = immed_map_keys(edge, im);
    let n = im.key_count() as usize;
    let mut out = ImmedBuf::new();
    if n == 1 {
        out.push((keys.as_slice()[0], u64::from_le_bytes(edge.imm_bytes())));
    } else {
        let vals = edge.node_ptr().cast::<u64>();
        for (slot, &k) in keys.as_slice().iter().enumerate() {
            // SAFETY: value array holds `n` values per contract.
            out.push((k, unsafe { *vals.add(slot) }));
        }
    }
    out
}

/// Frees a map immediate's value array, if it owns one.
///
/// # Safety
///
/// Same contract as [`read_map_immed`]; the array is not used afterwards.
unsafe fn free_map_immed_storage(a: &NodeAlloc, edge: &Edge, im: ImmedType) {
    if im.key_count() > 1 {
        // SAFETY: live value array of key_count values per contract.
        unsafe {
            a.free_bytes(
                core::ptr::NonNull::new(edge.node_ptr()).expect("value array"),
                im.key_count() as usize * 8,
            );
        }
    }
}

/// Builds a fresh map immediate from sorted entries.
fn write_map_immed(a: &NodeAlloc, edge: &mut Edge, kb: u8, entries: &[(u64, u64)]) {
    let im = ImmedType::new(kb, entries.len() as u8).expect("immediate capacity");
    debug_assert!(entries.len() <= map_immed_max(kb));
    let mut aux = [0u8; 7];
    for (slot, &(k, _)) in entries.iter().enumerate() {
        aux[slot * kb as usize..(slot + 1) * kb as usize]
            .copy_from_slice(&k.to_le_bytes()[..kb as usize]);
    }
    if entries.len() == 1 {
        edge.set_imm_bytes(entries[0].1.to_le_bytes());
    } else {
        let vals = a.alloc_bytes(entries.len() * 8).cast::<u64>();
        for (slot, &(_, v)) in entries.iter().enumerate() {
            // SAFETY: fresh array of entries.len() slots.
            unsafe { vals.as_ptr().add(slot).write(v) };
        }
        *edge = Edge::new_node(vals.as_ptr().cast(), 0);
    }
    edge.set_aux_bytes(aux);
    edge.set_tag(im.as_u8());
}

/// Reads a map leaf's entries (sorted by key).
///
/// # Safety
///
/// The edge must reference a live map leaf of `pop` entries.
unsafe fn read_map_leaf(edge: &Edge, kb: u8, pop: usize) -> Vec<(u64, u64)> {
    let base = edge.node_ptr();
    // One slot of headroom: every insert-path caller does a mid-buffer
    // `insert` right after materializing, which used to force a growth
    // reallocation (a second malloc + copy + free) on every conversion.
    let mut out = Vec::with_capacity(pop + 1);
    // `extend` from a range keeps TrustedLen, so the fill elides the
    // per-element capacity check a manual push loop pays (measured:
    // +0.15% on map_remove/random, whose callers never use the
    // headroom slot).
    out.extend((0..pop).map(|slot| {
        // SAFETY: map leaf = pop values then pop packed keys, per layout.
        unsafe {
            let k = read_packed(base.add(leaf::map_keys_offset(pop)), slot, kb as usize);
            let v = *base.cast::<u64>().add(slot);
            (k, v)
        }
    }));
    out
}

/// Allocates a `LeafBitmapL` from sorted level-1 entries and points
/// `edge` at it (decode bytes, if any, are the caller's to restore).
fn build_bitmap_leaf_map(a: &NodeAlloc, edge: &mut Edge, entries: &[(u64, u64)]) {
    let ptr = a.alloc_node_zeroed::<LeafBitmapL>();
    unsafe {
        for &(k, _) in entries {
            (*ptr.as_ptr()).bitmap.set(k as u8);
        }
        // `entries` is sorted by key, so each 32-digit subexpanse is one
        // contiguous run: detect the run and write it straight into its
        // subarray. The previous version bucketed values through a
        // `[Vec<u64>; 8]` — eight heap allocations of scratch (plus growth
        // reallocations) per linear-leaf → bitmap-leaf conversion, to
        // regroup values that the sort order had already grouped.
        //
        // Scope note, measured: a wider rework replacing the materializer
        // `Vec`s (`read_map_leaf`, `leaf_keys`) with stack buffers was tried
        // twice and REGRESSED both times (+2.7% map_insert/small with
        // default-init backing; worse with MaybeUninit). Those conversions
        // run once per form change, not per op, and malloc's fast path is
        // cheaper than a 264-528-byte memset or the uninit bookkeeping.
        // This function is different: eight allocations per conversion for
        // a regrouping the input ordering already provides.
        let mut i = 0;
        while i < entries.len() {
            let sub = (entries[i].0 >> 5) as usize;
            let mut j = i + 1;
            while j < entries.len() && (entries[j].0 >> 5) as usize == sub {
                j += 1;
            }
            let arr = a.alloc_bytes(sub_vals_size(j - i)).cast::<u64>();
            for (slot, &(_, v)) in entries[i..j].iter().enumerate() {
                // SAFETY: fresh array of `j - i` slots.
                arr.as_ptr().add(slot).write(v);
            }
            (*ptr.as_ptr()).values[sub] = arr.as_ptr();
            i = j;
        }
    }
    *edge = Edge::new_node(ptr.as_ptr().cast(), EdgeType::LeafB1.as_u8());
    edge.set_pop0(1, entries.len() as u64 - 1);
}

/// Allocates a map leaf from sorted entries and points `edge` at it.
fn build_map_leaf(a: &NodeAlloc, edge: &mut Edge, kb: u8, entries: &[(u64, u64)]) {
    let pop = entries.len();
    let ptr = a.alloc_bytes(leaf::size_map(kb, pop));
    for (slot, &(k, v)) in entries.iter().enumerate() {
        // SAFETY: in-bounds writes of the fresh allocation.
        unsafe {
            ptr.as_ptr().cast::<u64>().add(slot).write(v);
            write_packed(
                ptr.as_ptr().add(leaf::map_keys_offset(pop)),
                slot,
                kb as usize,
                k,
            );
        }
    }
    let tag = match kb {
        1 => EdgeType::Leaf1,
        2 => EdgeType::Leaf2,
        3 => EdgeType::Leaf3,
        4 => EdgeType::Leaf4,
        5 => EdgeType::Leaf5,
        6 => EdgeType::Leaf6,
        _ => EdgeType::Leaf7,
    };
    *edge = Edge::new_node(ptr.as_ptr(), tag.as_u8());
    edge.set_pop0(kb, pop as u64 - 1);
}

/// Runtime entry point: resolves the OCC flag once per operation (see
/// `mutate::insert_dyn`).
///
/// # Safety
///
/// Same contract as [`map_insert`].
pub(crate) unsafe fn map_insert_dyn<const KEEP: bool>(
    a: &NodeAlloc,
    edge: &mut Edge,
    key: Key,
    val: u64,
    level: u8,
) -> (Option<u64>, *mut u64) {
    // SAFETY: forwarded caller contract.
    unsafe {
        if a.occ_enabled() {
            map_insert::<KEEP, true>(a, edge, key, val, level)
        } else {
            map_insert::<KEEP, false>(a, edge, key, val, level)
        }
    }
}

/// Runtime entry point for removal; see [`map_insert_dyn`].
///
/// # Safety
///
/// Same contract as [`map_remove`].
pub(crate) unsafe fn map_remove_dyn(
    a: &NodeAlloc,
    edge: &mut Edge,
    key: Key,
    level: u8,
) -> Option<u64> {
    // SAFETY: forwarded caller contract.
    unsafe {
        if a.occ_enabled() {
            map_remove::<true>(a, edge, key, level)
        } else {
            map_remove::<false>(a, edge, key, level)
        }
    }
}

/// Inserts `key → val` in the subtree at `edge`. Returns the previously
/// stored value (`None` if the key is new) and a **writable pointer to
/// the key's value slot** (the compat `JudyLIns` contract — valid until
/// the next structural mutation). With `KEEP = true` an existing value is
/// left untouched instead of replaced.
///
/// # Safety
///
/// Same contract as `mutate::insert`, for map-flavor trees.
pub(crate) unsafe fn map_insert<const KEEP: bool, const OCC: bool>(
    a: &NodeAlloc,
    edge: &mut Edge,
    key: Key,
    val: u64,
    level: u8,
) -> (Option<u64>, *mut u64) {
    debug_assert!((1..=8).contains(&level));
    let tag = edge.tag().expect("valid edge tag");
    match tag {
        EdgeTag::Structural(EdgeType::Null) => {
            if level == 8 {
                let node = a.alloc_node_zeroed::<BranchL3>();
                unsafe { (*node.as_ptr()).hdr.level = level; }
                *edge = Edge::new_node(node.as_ptr().cast(), EdgeType::BranchL3.as_u8());
                // SAFETY: forwarded contract; edge is now a branch.
                return unsafe { map_insert::<KEEP, OCC>(a, edge, key, val, level) };
            }
            write_map_immed(a, edge, level, &[(key_low(key, level), val)]);
            // A single-entry immediate's value slot is the edge's word 0.
            (None, (&raw mut *edge).cast::<u64>())
        }

        EdgeTag::Immed(im) => {
            debug_assert_eq!(im.key_bytes(), level);
            let kb = im.key_bytes();
            let k = key_low(key, kb);
            // SAFETY: live map immediate per contract.
            let mut entries = unsafe { read_map_immed(edge, im) };
            match entries.binary_search_by_key(&k, |e| e.0) {
                Ok(pos) => {
                    let old = entries[pos].1;
                    if im.key_count() == 1 {
                        if !KEEP {
                            edge.set_imm_bytes(val.to_le_bytes());
                        }
                        (Some(old), (&raw mut *edge).cast::<u64>())
                    } else {
                        // SAFETY: live value array per contract.
                        let slot = unsafe { edge.node_ptr().cast::<u64>().add(pos) };
                        if !KEEP {
                            // SAFETY: same slot.
                            unsafe { slot.write(val) };
                        }
                        (Some(old), slot)
                    }
                }
                Err(pos) => {
                    entries.insert(pos, (k, val));
                    // SAFETY: storage re-read above; safe to release.
                    unsafe { free_map_immed_storage(a, edge, im) };
                    let slot = if entries.len() <= map_immed_max(kb) {
                        write_map_immed(a, edge, kb, &entries);
                        if entries.len() == 1 {
                            (&raw mut *edge).cast::<u64>()
                        } else {
                            // SAFETY: fresh value array of entries.len().
                            unsafe { edge.node_ptr().cast::<u64>().add(pos) }
                        }
                    } else {
                        build_map_leaf(a, edge, kb, &entries);
                        // SAFETY: fresh map leaf; values at the base.
                        unsafe { edge.node_ptr().cast::<u64>().add(pos) }
                    };
                    (None, slot)
                }
            }
        }

        EdgeTag::Structural(
            t @ (EdgeType::Leaf1
            | EdgeType::Leaf2
            | EdgeType::Leaf3
            | EdgeType::Leaf4
            | EdgeType::Leaf5
            | EdgeType::Leaf6
            | EdgeType::Leaf7),
        ) => {
            let kb = t.leaf_key_bytes().expect("leaf tag");
            debug_assert!(kb <= level);
            let pop = edge.pop0(kb) as usize + 1;
            if kb < level && !crate::get::decode_matches(edge, key, kb, level) {
                // Diverges inside the skipped prefix: branch out one
                // level and retry.
                split_skip(a, edge, key, level, pop as u64);
                // SAFETY: forwarded contract; edge is now a branch.
                return unsafe { map_insert::<KEEP, OCC>(a, edge, key, val, level) };
            }
            let k = key_low(key, kb);
            // Phase 7 coverage invariant (see alloc::assert_bracketed):
            // linear leaves carry no version; the parent's bracket does.
            a.assert_bracketed();
            let base = edge.node_ptr();
            // SAFETY: live map leaf per contract (keys behind the values).
            let pos =
                unsafe { leaf::lower_bound(base.add(leaf::map_keys_offset(pop)), pop, kb, k) };
            // SAFETY: pos < pop is in bounds.
            let hit = pos < pop
                && unsafe {
                    read_packed(base.add(leaf::map_keys_offset(pop)), pos, kb as usize) == k
                };
            match if hit { Ok(pos) } else { Err(pos) } {
                Ok(pos) => {
                    // SAFETY: in-place value swap within the live leaf.
                    unsafe {
                        let slot = base.cast::<u64>().add(pos);
                        let old = *slot;
                        if !KEEP {
                            slot.write(val);
                        }
                        (Some(old), slot)
                    }
                }
                Err(pos) => {
                    let cap = if kb == 1 { LEAF1_CAP } else { LEAF_CAP };
                    if pop < cap && leaf::cap_class(pop + 1) == leaf::cap_class(pop) {
                        // Fast path: spare class capacity — shift both
                        // areas in place.
                        // SAFETY: class capacity spare per the check.
                        unsafe { leaf::map_insert_at(base, kb, pop, pos, k, val) };
                        edge.set_pop0(kb, pop as u64);
                        // SAFETY: freshly shifted value area.
                        return (None, unsafe { base.cast::<u64>().add(pos) });
                    }
                    if pop < cap {
                        // Class-crossing grow that stays this leaf: a
                        // direct copy with a gap. The materialize path
                        // below costs a heap `Vec`, a per-entry unpack
                        // and a per-entry repack; steady-state churn
                        // crosses the small classes on every cycle, and
                        // this was 48% of that arm. Aux (decode + pop0)
                        // is preserved wholesale — no conversion, no
                        // widening, the form and kb are unchanged.
                        let new = a.alloc_bytes(leaf::size_map(kb, pop + 1));
                        // SAFETY: live source leaf of `pop` entries;
                        // fresh destination sized for `pop + 1`;
                        // `pos <= pop` from lower_bound.
                        unsafe {
                            leaf::map_realloc_insert(base, new.as_ptr(), kb, pop, pos, k, val);
                        }
                        let saved_aux = *edge.aux_bytes();
                        *edge = Edge::new_node(new.as_ptr(), t.as_u8());
                        edge.set_aux_bytes(saved_aux);
                        edge.set_pop0(kb, pop as u64);
                        // SAFETY: the old leaf is unlinked above; freed
                        // (or retired) with its allocation size.
                        unsafe {
                            a.free_bytes(
                                core::ptr::NonNull::new(base).expect("leaf ptr"),
                                leaf::size_map(kb, pop),
                            );
                        }
                        // SAFETY: slot `pos` of the fresh value area.
                        return (None, unsafe { new.as_ptr().cast::<u64>().add(pos) });
                    }
                    // Slow path: materialize entries for the conversion.
                    // A skipping leaf's keys are widened to full
                    // `level`-byte suffixes with its decode prefix, so the
                    // conversions below can place the replacement form at
                    // the true divergence level.
                    // SAFETY: live map leaf per contract.
                    let mut entries = unsafe { read_map_leaf(edge, kb, pop) };
                    let old_ptr = edge.node_ptr();
                    let old_size = leaf::size_map(kb, pop);
                    let saved_aux = *edge.aux_bytes();
                    entries.insert(pos, (k, val));
                    if kb < level {
                        let prefix = decode_value(edge, kb, level) << (8 * u32::from(kb));
                        for e in &mut entries {
                            e.0 |= prefix;
                        }
                    }
                    if entries.len() <= cap {
                        build_map_leaf(a, edge, kb, &entries);
                        restore_decode(edge, kb, level, &saved_aux);
                    } else if kb == 1 {
                        // Level-1 overflow: linear map leaf → bitmap leaf
                        // (any narrow pointer carries over).
                        build_bitmap_leaf_map(a, edge, &entries);
                        restore_decode(edge, 1, level, &saved_aux);
                    } else if divergence_level(entries[0].0, entries[entries.len() - 1].0, level)
                        == 1
                    {
                        // Narrow-pointer synthesis: all keys share their
                        // digits at levels 2..=kb — one bitmap leaf with
                        // the shared prefix as decode bytes, no chain.
                        let low: Vec<(u64, u64)> =
                            entries.iter().map(|&(k, v)| (key_low(k, 1), v)).collect();
                        let prefix_key = entries[0].0;
                        build_bitmap_leaf_map(a, edge, &low);
                        write_decode(edge, 1, level, prefix_key);
                    } else {
                        // Cascade: an empty branch at the divergence level
                        // (a narrow pointer when that sits below the slot;
                        // level-8 slots always branch in place — the root
                        // edge has no room for decode bytes), re-insert.
                        let d = divergence_level(entries[0].0, entries[entries.len() - 1].0, level);
                        let bl = if level <= 7 { d } else { level };
                        let node = a.alloc_node_zeroed::<BranchL3>();
                        unsafe { (*node.as_ptr()).hdr.level = bl; }
                        *edge = Edge::new_node(node.as_ptr().cast(), EdgeType::BranchL3.as_u8());
                        if bl < level {
                            write_decode(edge, bl, level, entries[0].0);
                        }
                        for &(k, v) in &entries {
                            // SAFETY: freshly built branch subtree.
                            let prev = unsafe { map_insert::<KEEP, OCC>(a, edge, k, v, level) };
                            debug_assert!(prev.0.is_none());
                        }
                        // pop0 cannot express the transient empty branch;
                        // pin the true population (see mutate::insert).
                        edge.set_pop0(bl, entries.len() as u64 - 1);
                    }
                    // SAFETY: old leaf allocation no longer referenced.
                    unsafe {
                        a.free_bytes(core::ptr::NonNull::new(old_ptr).expect("leaf"), old_size);
                    }
                    // Slow-path conversions relocate the value; one extra
                    // locate walk here keeps every fast path single-walk.
                    // SAFETY: freshly rebuilt subtree owned by `a`.
                    let slot = unsafe { crate::get::locate_slot(&raw mut *edge, key, level) }
                        .expect("just-inserted key")
                        .as_ptr();
                    (None, slot)
                }
            }
        }

        EdgeTag::Structural(EdgeType::LeafB1) => {
            if level > 1 && !crate::get::decode_matches(edge, key, 1, level) {
                // Diverges inside the skipped prefix: branch out one
                // level and retry.
                let pop = edge.pop0(1) + 1;
                split_skip(a, edge, key, level, pop);
                // SAFETY: forwarded contract; edge is now a branch.
                return unsafe { map_insert::<KEEP, OCC>(a, edge, key, val, level) };
            }
            let d = digit(key, 1);
            // Phase 7: a leaf carries no version of its own — readers
            // validate its payload against the parent branch's version,
            // whose bracket must still be open here.
            a.assert_bracketed();
            // SAFETY: live LeafBitmapL per contract.
            let node = unsafe { &mut *edge.node_ptr().cast::<LeafBitmapL>() };
            let sub = (d >> 5) as usize;
            let rank = node.bitmap.subexpanse_rank(d) as usize;
            if node.bitmap.test(d) {
                // SAFETY: value subarray holds subexpanse_count values.
                unsafe {
                    let slot = node.values[sub].add(rank);
                    let old = *slot;
                    if !KEEP {
                        slot.write(val);
                    }
                    return (Some(old), slot);
                }
            }
            let old_n = node.bitmap.subexpanse_count(sub) as usize;
            if old_n > 0 && leaf::cap_class(old_n + 1) == leaf::cap_class(old_n) {
                // Fast path: spare class capacity — shift in place.
                // SAFETY: the subarray holds cap_class(old_n) slots.
                unsafe {
                    let arr = node.values[sub];
                    core::ptr::copy(arr.add(rank), arr.add(rank + 1), old_n - rank);
                    arr.add(rank).write(val);
                }
            } else {
                let new = a.alloc_bytes(sub_vals_size(old_n + 1)).cast::<u64>();
                // SAFETY: copy old_n values around the inserted rank; the
                // empty case touches no old pointer.
                unsafe {
                    if old_n > 0 {
                        let old = node.values[sub];
                        new.as_ptr().copy_from_nonoverlapping(old, rank);
                        new.as_ptr()
                            .add(rank + 1)
                            .copy_from_nonoverlapping(old.add(rank), old_n - rank);
                        a.free_bytes(
                            core::ptr::NonNull::new(old.cast()).expect("values"),
                            sub_vals_size(old_n),
                        );
                    }
                    new.as_ptr().add(rank).write(val);
                }
                node.values[sub] = new.as_ptr();
            }
            node.bitmap.set(d);
            let pop0 = edge.pop0(1);
            edge.set_pop0(1, pop0 + 1);
            // SAFETY: the value now lives at this subarray rank.
            (None, unsafe { node.values[sub].add(rank) })
        }

        EdgeTag::Structural(EdgeType::FullExpanse) => {
            unreachable!("full-expanse edges are set-flavor only")
        }

        EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
            debug_assert!(level >= 2);
            // SAFETY: live branch per contract.
            let bl = unsafe { branch_form_level(edge, t, level) };
            if bl < level && !crate::get::decode_matches(edge, key, bl, level) {
                let pop = edge.pop0(bl) + 1;
                split_skip(a, edge, key, level, pop);
                // SAFETY: forwarded contract; edge is now a branch.
                return unsafe { map_insert::<KEEP, OCC>(a, edge, key, val, level) };
            }
            let d = digit(key, bl);
            let is_l3 = matches!(t, EdgeType::BranchL3);
            // SAFETY: live branch per contract.
            let (found, num) = unsafe {
                if is_l3 {
                    let b = &*edge.node_ptr().cast::<BranchL3>();
                    (b.hdr.find(d), b.hdr.num as usize)
                } else {
                    let b = &*edge.node_ptr().cast::<BranchL7>();
                    (b.hdr.find(d), b.hdr.num as usize)
                }
            };
            if let Some(slot) = found {
                // SAFETY: slot within populated count; child well-formed.
                // The node's version brackets the descent (Phase 7 OCC).
                let res = unsafe {
                    if is_l3 {
                        let b = &mut *edge.node_ptr().cast::<BranchL3>();
                        crate::occ::version_begin_if::<OCC>(a, &mut b.hdr.version);
                        let r = map_insert::<KEEP, OCC>(a, &mut b.edges[slot], key, val, bl - 1);
                        crate::occ::version_end_if::<OCC>(a, &mut b.hdr.version);
                        r
                    } else {
                        let b = &mut *edge.node_ptr().cast::<BranchL7>();
                        crate::occ::version_begin_if::<OCC>(a, &mut b.hdr.version);
                        let r = map_insert::<KEEP, OCC>(a, &mut b.edges[slot], key, val, bl - 1);
                        crate::occ::version_end_if::<OCC>(a, &mut b.hdr.version);
                        r
                    }
                };
                if res.0.is_none() {
                    bump_pop0(edge, bl, 1);
                }
                return res;
            }
            let cap = if is_l3 { BRANCH_L3_CAP } else { BRANCH_L7_CAP };
            if num == cap {
                // SAFETY: upgrade rebuilds the node; subtree stays owned.
                unsafe {
                    if is_l3 {
                        upgrade_l3_to_l7(a, edge);
                    } else {
                        upgrade_l7_to_b(a, edge);
                    }
                    return map_insert::<KEEP, OCC>(a, edge, key, val, level);
                }
            }
            // SAFETY: live branch; slot arithmetic bounded by capacity.
            let res = unsafe {
                if is_l3 {
                    let b = &mut *edge.node_ptr().cast::<BranchL3>();
                    crate::occ::version_begin_if::<OCC>(a, &mut b.hdr.version);
                    let slot = linear_insert_slot(&mut b.hdr.digits, &mut b.edges, num, d);
                    b.hdr.num += 1;
                    let r = map_insert::<KEEP, OCC>(a, &mut b.edges[slot], key, val, bl - 1);
                    crate::occ::version_end_if::<OCC>(a, &mut b.hdr.version);
                    r
                } else {
                    let b = &mut *edge.node_ptr().cast::<BranchL7>();
                    crate::occ::version_begin_if::<OCC>(a, &mut b.hdr.version);
                    let slot = linear_insert_slot(&mut b.hdr.digits, &mut b.edges, num, d);
                    b.hdr.num += 1;
                    let r = map_insert::<KEEP, OCC>(a, &mut b.edges[slot], key, val, bl - 1);
                    crate::occ::version_end_if::<OCC>(a, &mut b.hdr.version);
                    r
                }
            };
            debug_assert!(res.0.is_none());
            bump_pop0(edge, bl, 1);
            (None, res.1)
        }

        EdgeTag::Structural(EdgeType::BranchB) => {
            debug_assert!(level >= 2);
            // SAFETY: live branch per contract.
            let bl = unsafe { branch_form_level(edge, EdgeType::BranchB, level) };
            if bl < level && !crate::get::decode_matches(edge, key, bl, level) {
                let pop = edge.pop0(bl) + 1;
                split_skip(a, edge, key, level, pop);
                // SAFETY: forwarded contract; edge is now a branch.
                return unsafe { map_insert::<KEEP, OCC>(a, edge, key, val, level) };
            }
            let slot_level = level;
            let d = digit(key, bl);
            // SAFETY: live BranchB per contract.
            let b = unsafe { &mut *edge.node_ptr().cast::<BranchB>() };
            if b.bitmap.test(d) {
                let slot = b.bitmap.subexpanse_rank(d) as usize;
                let sub = b.subarrays[(d >> 5) as usize];
                // SAFETY: bitmap/subarray consistency invariant. The
                // node's version brackets the descent (Phase 7 OCC).
                crate::occ::version_begin_if::<OCC>(a, &mut b.version);
                // SAFETY: bitmap/subarray consistency invariant.
                let res =
                    unsafe { map_insert::<KEEP, OCC>(a, &mut *sub.add(slot), key, val, bl - 1) };
                crate::occ::version_end_if::<OCC>(a, &mut b.version);
                if res.0.is_none() {
                    bump_pop0(edge, bl, 1);
                }
                return res;
            }
            if b.bitmap.count() as usize + 1 > BRANCHB_UP {
                if bl < slot_level {
                    // BranchU cannot skip: materialize one chain level.
                    let pop = edge.pop0(bl) + 1;
                    wrap_skip_level(a, edge, bl + 1, slot_level, pop);
                    // SAFETY: forwarded contract; edge is now a branch.
                    return unsafe { map_insert::<KEEP, OCC>(a, edge, key, val, slot_level) };
                }
                // SAFETY: upgrade rebuilds the node; subtree stays owned.
                unsafe {
                    upgrade_b_to_u(a, edge);
                    return map_insert::<KEEP, OCC>(a, edge, key, val, slot_level);
                }
            }
            let sub = (d >> 5) as usize;
            let old_n = b.pop_counts[sub] as usize;
            let rank = b.bitmap.subexpanse_rank(d) as usize;
            crate::occ::version_begin_if::<OCC>(a, &mut b.version);
            if old_n > 0 && leaf::cap_class(old_n + 1) == leaf::cap_class(old_n) {
                // Fast path: spare class capacity — shift in place.
                // SAFETY: the subarray holds cap_class(old_n) slots.
                unsafe {
                    let arr = b.subarrays[sub];
                    core::ptr::copy(arr.add(rank), arr.add(rank + 1), old_n - rank);
                    arr.add(rank).write(Edge::NULL);
                }
            } else {
                let new = a.alloc_bytes(sub_edges_size(old_n + 1)).cast::<Edge>();
                // SAFETY: copy old_n live edges around the inserted slot;
                // the empty case touches no old pointer.
                unsafe {
                    if old_n > 0 {
                        let old = b.subarrays[sub];
                        new.as_ptr().copy_from_nonoverlapping(old, rank);
                        new.as_ptr()
                            .add(rank + 1)
                            .copy_from_nonoverlapping(old.add(rank), old_n - rank);
                        a.free_bytes(
                            core::ptr::NonNull::new(old.cast()).expect("subarray"),
                            sub_edges_size(old_n),
                        );
                    }
                    new.as_ptr().add(rank).write(Edge::NULL);
                }
                b.subarrays[sub] = new.as_ptr();
            }
            b.pop_counts[sub] = (old_n + 1) as u16;
            b.bitmap.set(d);
            // SAFETY: fresh null child slot within the subarray.
            let res = unsafe {
                map_insert::<KEEP, OCC>(a, &mut *b.subarrays[sub].add(rank), key, val, bl - 1)
            };
            crate::occ::version_end_if::<OCC>(a, &mut b.version);
            debug_assert!(res.0.is_none());
            bump_pop0(edge, bl, 1);
            (None, res.1)
        }

        EdgeTag::Structural(EdgeType::BranchU) => {
            debug_assert!(level >= 2);
            let d = digit(key, level);
            // SAFETY: live BranchU per contract.
            let b = unsafe { &mut *edge.node_ptr().cast::<BranchU>() };
            // SAFETY: child subtree well-formed (or null) per contract.
            crate::occ::version_begin_if::<OCC>(a, &mut b.version);
            // SAFETY: child subtree well-formed (or null) per contract.
            let res = unsafe {
                map_insert::<KEEP, OCC>(a, &mut b.edges[d as usize], key, val, level - 1)
            };
            crate::occ::version_end_if::<OCC>(a, &mut b.version);
            if res.0.is_none() {
                bump_pop0(edge, level, 1);
            }
            res
        }
    }
}

/// Removes `key` from the subtree at `edge`; returns its value if present.
///
/// # Safety
///
/// Same contract as [`map_insert`].
pub(crate) unsafe fn map_remove<const OCC: bool>(
    a: &NodeAlloc,
    edge: &mut Edge,
    key: Key,
    level: u8,
) -> Option<u64> {
    debug_assert!((1..=8).contains(&level));
    let tag = edge.tag().expect("valid edge tag");
    match tag {
        EdgeTag::Structural(EdgeType::Null) => None,

        EdgeTag::Immed(im) => {
            let kb = im.key_bytes();
            debug_assert_eq!(kb, level);
            let k = key_low(key, kb);
            // SAFETY: live map immediate per contract.
            let mut entries = unsafe { read_map_immed(edge, im) };
            let Ok(pos) = entries.binary_search_by_key(&k, |e| e.0) else {
                return None;
            };
            let (_, old) = entries.remove(pos);
            // SAFETY: storage re-read above; safe to release.
            unsafe { free_map_immed_storage(a, edge, im) };
            if entries.is_empty() {
                *edge = Edge::NULL;
            } else {
                write_map_immed(a, edge, kb, &entries);
            }
            Some(old)
        }

        EdgeTag::Structural(
            t @ (EdgeType::Leaf1
            | EdgeType::Leaf2
            | EdgeType::Leaf3
            | EdgeType::Leaf4
            | EdgeType::Leaf5
            | EdgeType::Leaf6
            | EdgeType::Leaf7),
        ) => {
            let kb = t.leaf_key_bytes().expect("leaf tag");
            debug_assert!(kb <= level);
            if kb < level && !crate::get::decode_matches(edge, key, kb, level) {
                return None;
            }
            let pop = edge.pop0(kb) as usize + 1;
            let k = key_low(key, kb);
            // Phase 7 coverage invariant (see alloc::assert_bracketed):
            // linear leaves carry no version; the parent's bracket does.
            a.assert_bracketed();
            let base = edge.node_ptr();
            // SAFETY: live map leaf per contract.
            let pos =
                unsafe { leaf::lower_bound(base.add(leaf::map_keys_offset(pop)), pop, kb, k) };
            // SAFETY: pos < pop is in bounds.
            // SAFETY: pos < pop is in bounds.
            let miss = pos == pop
                || unsafe {
                    read_packed(base.add(leaf::map_keys_offset(pop)), pos, kb as usize) != k
                };
            if miss {
                return None;
            }
            if pop > map_immed_max(level) && leaf::cap_class(pop - 1) == leaf::cap_class(pop) {
                // Fast path: stays a leaf in the same class.
                // SAFETY: pos < pop; same-class allocation.
                unsafe {
                    let old = *base.cast::<u64>().add(pos);
                    leaf::map_remove_at(base, kb, pop, pos);
                    edge.set_pop0(kb, pop as u64 - 2);
                    return Some(old);
                }
            }
            if pop >= 2 && pop > map_immed_max(level) {
                // Class-crossing shrink that stays this leaf (the
                // hysteresis band keeps it one below `map_immed_max`):
                // direct copy with the slot elided — same rationale as
                // the grow twin in `map_insert`.
                // SAFETY: `pos < pop` (hit checked above); the old value
                // is read before the leaf is unlinked.
                let old = unsafe { *base.cast::<u64>().add(pos) };
                let new = a.alloc_bytes(leaf::size_map(kb, pop - 1));
                // SAFETY: live source leaf of `pop >= 2` entries; fresh
                // destination sized for `pop - 1`.
                unsafe { leaf::map_realloc_remove(base, new.as_ptr(), kb, pop, pos) };
                let saved_aux = *edge.aux_bytes();
                *edge = Edge::new_node(new.as_ptr(), t.as_u8());
                edge.set_aux_bytes(saved_aux);
                edge.set_pop0(kb, pop as u64 - 2);
                // SAFETY: the old leaf is unlinked above; freed (or
                // retired) with its allocation size.
                unsafe {
                    a.free_bytes(
                        core::ptr::NonNull::new(base).expect("leaf ptr"),
                        leaf::size_map(kb, pop),
                    );
                }
                return Some(old);
            }
            // Slow path (class boundary or conversion).
            // SAFETY: live map leaf per contract.
            let mut entries = unsafe { read_map_leaf(edge, kb, pop) };
            let (_, old) = entries.remove(pos);
            let old_ptr = edge.node_ptr();
            let old_size = leaf::size_map(kb, pop);
            let saved_aux = *edge.aux_bytes();
            // Hysteresis: back to an immediate one below the boundary —
            // at the *slot* level for skipping leaves, absorbing decode
            // bytes into full remainders. Wide keys (map_immed_max == 1)
            // can drain a leaf to zero entries: back to null.
            if entries.is_empty() {
                *edge = Edge::NULL;
            } else if entries.len() < map_immed_max(level) {
                let dv = if kb < level {
                    decode_value(edge, kb, level)
                } else {
                    0
                };
                let full: Vec<(u64, u64)> = entries
                    .iter()
                    .map(|&(low, v)| ((dv << (8 * u32::from(kb))) | low, v))
                    .collect();
                write_map_immed(a, edge, level, &full);
            } else {
                build_map_leaf(a, edge, kb, &entries);
                restore_decode(edge, kb, level, &saved_aux);
            }
            // SAFETY: old leaf allocation no longer referenced.
            unsafe {
                a.free_bytes(core::ptr::NonNull::new(old_ptr).expect("leaf"), old_size);
            }
            Some(old)
        }

        EdgeTag::Structural(EdgeType::LeafB1) => {
            if level > 1 && !crate::get::decode_matches(edge, key, 1, level) {
                return None;
            }
            let d = digit(key, 1);
            // Phase 7: a leaf carries no version of its own — readers
            // validate its payload against the parent branch's version,
            // whose bracket must still be open here.
            a.assert_bracketed();
            // SAFETY: live LeafBitmapL per contract.
            let node = unsafe { &mut *edge.node_ptr().cast::<LeafBitmapL>() };
            if !node.bitmap.test(d) {
                return None;
            }
            let sub = (d >> 5) as usize;
            let rank = node.bitmap.subexpanse_rank(d) as usize;
            let old_n = node.bitmap.subexpanse_count(sub) as usize;
            // SAFETY: value subarray holds old_n values.
            let old = unsafe { *node.values[sub].add(rank) };
            // SAFETY: shrink of the packed value subarray — in place
            // when the class holds, reallocating across boundaries.
            unsafe {
                let old_arr = node.values[sub];
                if old_n == 1 {
                    node.values[sub] = core::ptr::null_mut();
                    a.free_bytes(
                        core::ptr::NonNull::new(old_arr.cast()).expect("values"),
                        sub_vals_size(old_n),
                    );
                } else if leaf::cap_class(old_n - 1) == leaf::cap_class(old_n) {
                    core::ptr::copy(old_arr.add(rank + 1), old_arr.add(rank), old_n - 1 - rank);
                } else {
                    let new = a.alloc_bytes(sub_vals_size(old_n - 1)).cast::<u64>();
                    new.as_ptr().copy_from_nonoverlapping(old_arr, rank);
                    new.as_ptr()
                        .add(rank)
                        .copy_from_nonoverlapping(old_arr.add(rank + 1), old_n - 1 - rank);
                    node.values[sub] = new.as_ptr();
                    a.free_bytes(
                        core::ptr::NonNull::new(old_arr.cast()).expect("values"),
                        sub_vals_size(old_n),
                    );
                }
            }
            node.bitmap.clear(d);
            let pop = edge.pop0(1) as usize; // old pop - 1
            // Hysteresis: back to a linear map leaf one below the cap.
            if pop < LEAF1_CAP {
                let saved_aux = *edge.aux_bytes();
                let dv = if level > 1 {
                    decode_value(edge, 1, level)
                } else {
                    0
                };
                // SAFETY: live node; entries re-read for the rebuild.
                let entries: Vec<(u64, u64)> = unsafe {
                    let mut out = Vec::with_capacity(pop);
                    let mut dig = node.bitmap.next_set(0);
                    while let Some(g) = dig {
                        let s = (g >> 5) as usize;
                        let r = node.bitmap.subexpanse_rank(g) as usize;
                        out.push((u64::from(g), *node.values[s].add(r)));
                        dig = if g == 255 {
                            None
                        } else {
                            node.bitmap.next_set(g + 1)
                        };
                    }
                    out
                };
                // SAFETY: free subarrays + node after extraction.
                unsafe {
                    for sub in 0..8 {
                        let n = node.bitmap.subexpanse_count(sub) as usize;
                        if n > 0 {
                            a.free_bytes(
                                core::ptr::NonNull::new(node.values[sub].cast()).expect("values"),
                                sub_vals_size(n),
                            );
                        }
                    }
                    a.free_node(
                        core::ptr::NonNull::new(edge.node_ptr().cast::<LeafBitmapL>()).unwrap(),
                    );
                }
                if entries.len() < map_immed_max(level) {
                    let full: Vec<(u64, u64)> = entries
                        .iter()
                        .map(|&(low, v)| ((dv << 8) | low, v))
                        .collect();
                    write_map_immed(a, edge, level, &full);
                } else {
                    build_map_leaf(a, edge, 1, &entries);
                    restore_decode(edge, 1, level, &saved_aux);
                }
            } else {
                edge.set_pop0(1, pop as u64 - 1);
            }
            Some(old)
        }

        EdgeTag::Structural(EdgeType::FullExpanse) => {
            unreachable!("full-expanse edges are set-flavor only")
        }

        EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
            // SAFETY: live branch per contract.
            let bl = unsafe { branch_form_level(edge, t, level) };
            if bl < level && !crate::get::decode_matches(edge, key, bl, level) {
                return None;
            }
            let d = digit(key, bl);
            let is_l3 = matches!(t, EdgeType::BranchL3);
            // SAFETY: live branch per contract.
            let removed = unsafe {
                if is_l3 {
                    let b = &mut *edge.node_ptr().cast::<BranchL3>();
                    let slot = b.hdr.find(d)?;
                    crate::occ::version_begin_if::<OCC>(a, &mut b.hdr.version);
                    let r = map_remove::<OCC>(a, &mut b.edges[slot], key, bl - 1);
                    if r.is_some() && b.edges[slot].is_null() {
                        linear_remove_slot(
                            &mut b.hdr.digits,
                            &mut b.edges,
                            b.hdr.num as usize,
                            slot,
                        );
                        b.hdr.num -= 1;
                    }
                    crate::occ::version_end_if::<OCC>(a, &mut b.hdr.version);
                    r
                } else {
                    let b = &mut *edge.node_ptr().cast::<BranchL7>();
                    let slot = b.hdr.find(d)?;
                    crate::occ::version_begin_if::<OCC>(a, &mut b.hdr.version);
                    let r = map_remove::<OCC>(a, &mut b.edges[slot], key, bl - 1);
                    if r.is_some() && b.edges[slot].is_null() {
                        linear_remove_slot(
                            &mut b.hdr.digits,
                            &mut b.edges,
                            b.hdr.num as usize,
                            slot,
                        );
                        b.hdr.num -= 1;
                    }
                    crate::occ::version_end_if::<OCC>(a, &mut b.hdr.version);
                    r
                }
            };
            let old = removed?;
            // SAFETY: node rebuilds below keep the subtree owned.
            unsafe {
                let num = if is_l3 {
                    (*edge.node_ptr().cast::<BranchL3>()).hdr.num as usize
                } else {
                    (*edge.node_ptr().cast::<BranchL7>()).hdr.num as usize
                };
                if num == 0 {
                    if is_l3 {
                        a.free_node(
                            core::ptr::NonNull::new(edge.node_ptr().cast::<BranchL3>()).unwrap(),
                        );
                    } else {
                        a.free_node(
                            core::ptr::NonNull::new(edge.node_ptr().cast::<BranchL7>()).unwrap(),
                        );
                    }
                    *edge = Edge::NULL;
                    return Some(old);
                }
                bump_pop0(edge, bl, -1);
                if !is_l3 && num < BRANCH_L3_CAP {
                    downgrade_l7_to_l3(a, edge);
                }
            }
            Some(old)
        }

        EdgeTag::Structural(EdgeType::BranchB) => {
            // SAFETY: live branch per contract.
            let bl = unsafe { branch_form_level(edge, EdgeType::BranchB, level) };
            if bl < level && !crate::get::decode_matches(edge, key, bl, level) {
                return None;
            }
            let d = digit(key, bl);
            // SAFETY: live BranchB per contract.
            let b = unsafe { &mut *edge.node_ptr().cast::<BranchB>() };
            if !b.bitmap.test(d) {
                return None;
            }
            let sub = (d >> 5) as usize;
            let rank = b.bitmap.subexpanse_rank(d) as usize;
            // SAFETY: bitmap/subarray consistency invariant. Bracketed
            // through the subarray shrink below (Phase 7 OCC).
            crate::occ::version_begin_if::<OCC>(a, &mut b.version);
            // SAFETY: bitmap/subarray consistency invariant.
            let old = match unsafe {
                map_remove::<OCC>(a, &mut *b.subarrays[sub].add(rank), key, bl - 1)
            } {
                Some(v) => v,
                None => {
                    crate::occ::version_end_if::<OCC>(a, &mut b.version);
                    return None;
                }
            };
            // SAFETY: child slot checked/live per invariant.
            let child_null = unsafe { (*b.subarrays[sub].add(rank)).is_null() };
            if child_null {
                let old_n = b.pop_counts[sub] as usize;
                // SAFETY: shrink of the packed subarray — in place when
                // the class holds, reallocating across class boundaries.
                unsafe {
                    let old_arr = b.subarrays[sub];
                    if old_n == 1 {
                        b.subarrays[sub] = core::ptr::null_mut();
                        a.free_bytes(
                            core::ptr::NonNull::new(old_arr.cast()).unwrap(),
                            sub_edges_size(old_n),
                        );
                    } else if leaf::cap_class(old_n - 1) == leaf::cap_class(old_n) {
                        core::ptr::copy(old_arr.add(rank + 1), old_arr.add(rank), old_n - 1 - rank);
                    } else {
                        let new = a.alloc_bytes(sub_edges_size(old_n - 1)).cast::<Edge>();
                        new.as_ptr().copy_from_nonoverlapping(old_arr, rank);
                        new.as_ptr()
                            .add(rank)
                            .copy_from_nonoverlapping(old_arr.add(rank + 1), old_n - 1 - rank);
                        b.subarrays[sub] = new.as_ptr();
                        a.free_bytes(
                            core::ptr::NonNull::new(old_arr.cast()).unwrap(),
                            sub_edges_size(old_n),
                        );
                    }
                }
                b.pop_counts[sub] = (old_n - 1) as u16;
                b.bitmap.clear(d);
            }
            crate::occ::version_end_if::<OCC>(a, &mut b.version);
            let digits = b.bitmap.count() as usize;
            if digits == 0 {
                // SAFETY: empty node no longer referenced.
                unsafe {
                    a.free_node(
                        core::ptr::NonNull::new(edge.node_ptr().cast::<BranchB>()).unwrap(),
                    );
                }
                *edge = Edge::NULL;
                return Some(old);
            }
            bump_pop0(edge, bl, -1);
            if digits < BRANCH_L7_CAP {
                // SAFETY: rebuild keeps the subtree owned.
                unsafe { downgrade_b_to_l7(a, edge) };
            }
            Some(old)
        }

        EdgeTag::Structural(EdgeType::BranchU) => {
            let d = digit(key, level);
            // SAFETY: live BranchU per contract.
            let b = unsafe { &mut *edge.node_ptr().cast::<BranchU>() };
            // SAFETY: child subtree well-formed (or null) per contract.
            crate::occ::version_begin_if::<OCC>(a, &mut b.version);
            // SAFETY: child subtree well-formed (or null) per contract.
            let old =
                match unsafe { map_remove::<OCC>(a, &mut b.edges[d as usize], key, level - 1) } {
                    Some(v) => v,
                    None => {
                        crate::occ::version_end_if::<OCC>(a, &mut b.version);
                        return None;
                    }
                };
            crate::occ::version_end_if::<OCC>(a, &mut b.version);
            let digits = b.edges.iter().filter(|e| !e.is_null()).count();
            if digits == 0 {
                // SAFETY: empty node no longer referenced.
                unsafe {
                    a.free_node(
                        core::ptr::NonNull::new(edge.node_ptr().cast::<BranchU>()).unwrap(),
                    );
                }
                *edge = Edge::NULL;
                return Some(old);
            }
            bump_pop0(edge, level, -1);
            if digits < BRANCHB_UP {
                // SAFETY: rebuild keeps the subtree owned.
                unsafe { downgrade_u_to_b(a, edge, level) };
            }
            Some(old)
        }
    }
}
