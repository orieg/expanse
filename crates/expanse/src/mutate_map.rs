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
    BRANCHB_UP, LEAF_CAP, LEAF1_CAP, bump_pop0, downgrade_b_to_l7, downgrade_l7_to_l3,
    downgrade_u_to_b, immed_map_keys, key_low, linear_insert_slot, linear_remove_slot,
    map_immed_max, read_packed, upgrade_b_to_u, upgrade_l3_to_l7, upgrade_l7_to_b, write_packed,
};
use crate::node::{BranchB, BranchL3, BranchL7, BranchU, Edge, LeafBitmapL};
use crate::types::{BRANCH_L3_CAP, BRANCH_L7_CAP, EdgeTag, EdgeType, ImmedType, Key, digit};

/// Reads a map immediate's entries (sorted by key).
///
/// # Safety
///
/// For key counts above one, word 0 must point to a live value array.
unsafe fn read_map_immed(edge: &Edge, im: ImmedType) -> Vec<(u64, u64)> {
    let keys = immed_map_keys(edge, im);
    let n = im.key_count() as usize;
    if n == 1 {
        vec![(keys[0], u64::from_le_bytes(edge.imm_bytes()))]
    } else {
        let vals = edge.node_ptr().cast::<u64>();
        keys.iter()
            .enumerate()
            // SAFETY: value array holds `n` values per contract.
            .map(|(slot, &k)| (k, unsafe { *vals.add(slot) }))
            .collect()
    }
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
    // SAFETY: map leaf = pop values then pop packed keys, per layout.
    (0..pop)
        .map(|slot| unsafe {
            let k = read_packed(base.add(leaf::map_keys_offset(pop)), slot, kb as usize);
            let v = *base.cast::<u64>().add(slot);
            (k, v)
        })
        .collect()
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

/// Inserts or replaces `key → val` in the subtree at `edge`; returns the
/// replaced value, or `None` if the key is new.
///
/// # Safety
///
/// Same contract as `mutate::insert`, for map-flavor trees.
pub(crate) unsafe fn map_insert(
    a: &NodeAlloc,
    edge: &mut Edge,
    key: Key,
    val: u64,
    level: u8,
) -> Option<u64> {
    debug_assert!((1..=8).contains(&level));
    let tag = edge.tag().expect("valid edge tag");
    match tag {
        EdgeTag::Structural(EdgeType::Null) => {
            if level == 8 {
                let node = a.alloc_node(BranchL3::new());
                *edge = Edge::new_node(node.as_ptr().cast(), EdgeType::BranchL3.as_u8());
                // SAFETY: forwarded contract; edge is now a branch.
                return unsafe { map_insert(a, edge, key, val, level) };
            }
            write_map_immed(a, edge, level, &[(key_low(key, level), val)]);
            None
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
                        edge.set_imm_bytes(val.to_le_bytes());
                    } else {
                        // SAFETY: live value array per contract.
                        unsafe { edge.node_ptr().cast::<u64>().add(pos).write(val) };
                    }
                    Some(old)
                }
                Err(pos) => {
                    entries.insert(pos, (k, val));
                    // SAFETY: storage re-read above; safe to release.
                    unsafe { free_map_immed_storage(a, edge, im) };
                    if entries.len() <= map_immed_max(kb) {
                        write_map_immed(a, edge, kb, &entries);
                    } else {
                        build_map_leaf(a, edge, kb, &entries);
                    }
                    None
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
            debug_assert_eq!(kb, level);
            let pop = edge.pop0(kb) as usize + 1;
            let k = key_low(key, kb);
            // SAFETY: live map leaf per contract.
            let mut entries = unsafe { read_map_leaf(edge, kb, pop) };
            match entries.binary_search_by_key(&k, |e| e.0) {
                Ok(pos) => {
                    let old = entries[pos].1;
                    // SAFETY: in-place value write within the live leaf.
                    unsafe { edge.node_ptr().cast::<u64>().add(pos).write(val) };
                    Some(old)
                }
                Err(pos) => {
                    let old_ptr = edge.node_ptr();
                    let old_size = leaf::size_map(kb, pop);
                    entries.insert(pos, (k, val));
                    let cap = if kb == 1 { LEAF1_CAP } else { LEAF_CAP };
                    if entries.len() <= cap {
                        build_map_leaf(a, edge, kb, &entries);
                    } else if kb == 1 {
                        // Level-1 overflow: linear map leaf → bitmap leaf.
                        let mut node = LeafBitmapL::new();
                        let mut per_sub: [Vec<u64>; 8] = Default::default();
                        for &(k, v) in &entries {
                            node.bitmap.set(k as u8);
                            per_sub[(k >> 5) as usize].push(v);
                        }
                        for (sub, vals) in per_sub.iter().enumerate() {
                            if !vals.is_empty() {
                                let arr = a.alloc_bytes(vals.len() * 8).cast::<u64>();
                                for (i, &v) in vals.iter().enumerate() {
                                    // SAFETY: fresh array of vals.len().
                                    unsafe { arr.as_ptr().add(i).write(v) };
                                }
                                node.values[sub] = arr.as_ptr();
                            }
                        }
                        let ptr = a.alloc_node(node);
                        *edge = Edge::new_node(ptr.as_ptr().cast(), EdgeType::LeafB1.as_u8());
                        edge.set_pop0(1, entries.len() as u64 - 1);
                    } else {
                        // Cascade: empty branch, then re-insert everything.
                        let node = a.alloc_node(BranchL3::new());
                        *edge = Edge::new_node(node.as_ptr().cast(), EdgeType::BranchL3.as_u8());
                        for &(k, v) in &entries {
                            // SAFETY: freshly built branch subtree.
                            let prev = unsafe { map_insert(a, edge, k, v, level) };
                            debug_assert!(prev.is_none());
                        }
                        // pop0 cannot express the transient empty branch;
                        // pin the true population (see mutate::insert).
                        edge.set_pop0(level, entries.len() as u64 - 1);
                    }
                    // SAFETY: old leaf allocation no longer referenced.
                    unsafe {
                        a.free_bytes(core::ptr::NonNull::new(old_ptr).expect("leaf"), old_size);
                    }
                    None
                }
            }
        }

        EdgeTag::Structural(EdgeType::LeafB1) => {
            debug_assert_eq!(level, 1);
            let d = digit(key, 1);
            // SAFETY: live LeafBitmapL per contract.
            let node = unsafe { &mut *edge.node_ptr().cast::<LeafBitmapL>() };
            let sub = (d >> 5) as usize;
            let rank = node.bitmap.subexpanse_rank(d) as usize;
            if node.bitmap.test(d) {
                // SAFETY: value subarray holds subexpanse_count values.
                unsafe {
                    let slot = node.values[sub].add(rank);
                    let old = *slot;
                    slot.write(val);
                    return Some(old);
                }
            }
            let old_n = node.bitmap.subexpanse_count(sub) as usize;
            let new = a.alloc_bytes((old_n + 1) * 8).cast::<u64>();
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
                        old_n * 8,
                    );
                }
                new.as_ptr().add(rank).write(val);
            }
            node.values[sub] = new.as_ptr();
            node.bitmap.set(d);
            let pop0 = edge.pop0(1);
            edge.set_pop0(1, pop0 + 1);
            None
        }

        EdgeTag::Structural(EdgeType::FullExpanse) => {
            unreachable!("full-expanse edges are set-flavor only")
        }

        EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
            debug_assert!(level >= 2);
            let d = digit(key, level);
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
                let prev = unsafe {
                    if is_l3 {
                        let b = &mut *edge.node_ptr().cast::<BranchL3>();
                        map_insert(a, &mut b.edges[slot], key, val, level - 1)
                    } else {
                        let b = &mut *edge.node_ptr().cast::<BranchL7>();
                        map_insert(a, &mut b.edges[slot], key, val, level - 1)
                    }
                };
                if prev.is_none() {
                    bump_pop0(edge, level, 1);
                }
                return prev;
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
                    return map_insert(a, edge, key, val, level);
                }
            }
            // SAFETY: live branch; slot arithmetic bounded by capacity.
            let prev = unsafe {
                if is_l3 {
                    let b = &mut *edge.node_ptr().cast::<BranchL3>();
                    let slot = linear_insert_slot(&mut b.hdr.digits, &mut b.edges, num, d);
                    b.hdr.num += 1;
                    map_insert(a, &mut b.edges[slot], key, val, level - 1)
                } else {
                    let b = &mut *edge.node_ptr().cast::<BranchL7>();
                    let slot = linear_insert_slot(&mut b.hdr.digits, &mut b.edges, num, d);
                    b.hdr.num += 1;
                    map_insert(a, &mut b.edges[slot], key, val, level - 1)
                }
            };
            debug_assert!(prev.is_none());
            bump_pop0(edge, level, 1);
            None
        }

        EdgeTag::Structural(EdgeType::BranchB) => {
            debug_assert!(level >= 2);
            let d = digit(key, level);
            // SAFETY: live BranchB per contract.
            let b = unsafe { &mut *edge.node_ptr().cast::<BranchB>() };
            if b.bitmap.test(d) {
                let slot = b.bitmap.subexpanse_rank(d) as usize;
                let sub = b.subarrays[(d >> 5) as usize];
                // SAFETY: bitmap/subarray consistency invariant.
                let prev = unsafe { map_insert(a, &mut *sub.add(slot), key, val, level - 1) };
                if prev.is_none() {
                    bump_pop0(edge, level, 1);
                }
                return prev;
            }
            if b.bitmap.count() as usize + 1 > BRANCHB_UP {
                // SAFETY: upgrade rebuilds the node; subtree stays owned.
                unsafe {
                    upgrade_b_to_u(a, edge);
                    return map_insert(a, edge, key, val, level);
                }
            }
            let sub = (d >> 5) as usize;
            let old_n = b.pop_counts[sub] as usize;
            let rank = b.bitmap.subexpanse_rank(d) as usize;
            let new = a
                .alloc_bytes((old_n + 1) * size_of::<Edge>())
                .cast::<Edge>();
            // SAFETY: copy old_n live edges around the inserted slot; the
            // empty case touches no old pointer.
            unsafe {
                if old_n > 0 {
                    let old = b.subarrays[sub];
                    new.as_ptr().copy_from_nonoverlapping(old, rank);
                    new.as_ptr()
                        .add(rank + 1)
                        .copy_from_nonoverlapping(old.add(rank), old_n - rank);
                    a.free_bytes(
                        core::ptr::NonNull::new(old.cast()).expect("subarray"),
                        old_n * size_of::<Edge>(),
                    );
                }
                new.as_ptr().add(rank).write(Edge::NULL);
            }
            b.subarrays[sub] = new.as_ptr();
            b.pop_counts[sub] = (old_n + 1) as u16;
            b.bitmap.set(d);
            // SAFETY: fresh null child slot within the new subarray.
            let prev = unsafe { map_insert(a, &mut *new.as_ptr().add(rank), key, val, level - 1) };
            debug_assert!(prev.is_none());
            bump_pop0(edge, level, 1);
            None
        }

        EdgeTag::Structural(EdgeType::BranchU) => {
            debug_assert!(level >= 2);
            let d = digit(key, level);
            // SAFETY: live BranchU per contract.
            let b = unsafe { &mut *edge.node_ptr().cast::<BranchU>() };
            // SAFETY: child subtree well-formed (or null) per contract.
            let prev = unsafe { map_insert(a, &mut b.edges[d as usize], key, val, level - 1) };
            if prev.is_none() {
                bump_pop0(edge, level, 1);
            }
            prev
        }
    }
}

/// Removes `key` from the subtree at `edge`; returns its value if present.
///
/// # Safety
///
/// Same contract as [`map_insert`].
pub(crate) unsafe fn map_remove(
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
            debug_assert_eq!(kb, level);
            let pop = edge.pop0(kb) as usize + 1;
            let k = key_low(key, kb);
            // SAFETY: live map leaf per contract.
            let mut entries = unsafe { read_map_leaf(edge, kb, pop) };
            let Ok(pos) = entries.binary_search_by_key(&k, |e| e.0) else {
                return None;
            };
            let (_, old) = entries.remove(pos);
            let old_ptr = edge.node_ptr();
            let old_size = leaf::size_map(kb, pop);
            // Hysteresis: back to an immediate one below the boundary.
            // Wide keys (map_immed_max == 1) can drain a leaf to zero
            // entries, which has no immediate form: back to null.
            if entries.is_empty() {
                *edge = Edge::NULL;
            } else if entries.len() < map_immed_max(kb) {
                write_map_immed(a, edge, kb, &entries);
            } else {
                build_map_leaf(a, edge, kb, &entries);
            }
            // SAFETY: old leaf allocation no longer referenced.
            unsafe {
                a.free_bytes(core::ptr::NonNull::new(old_ptr).expect("leaf"), old_size);
            }
            Some(old)
        }

        EdgeTag::Structural(EdgeType::LeafB1) => {
            debug_assert_eq!(level, 1);
            let d = digit(key, 1);
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
            // SAFETY: shrink-copy of the packed value subarray.
            unsafe {
                let old_arr = node.values[sub];
                if old_n == 1 {
                    node.values[sub] = core::ptr::null_mut();
                } else {
                    let new = a.alloc_bytes((old_n - 1) * 8).cast::<u64>();
                    new.as_ptr().copy_from_nonoverlapping(old_arr, rank);
                    new.as_ptr()
                        .add(rank)
                        .copy_from_nonoverlapping(old_arr.add(rank + 1), old_n - 1 - rank);
                    node.values[sub] = new.as_ptr();
                }
                a.free_bytes(
                    core::ptr::NonNull::new(old_arr.cast()).expect("values"),
                    old_n * 8,
                );
            }
            node.bitmap.clear(d);
            let pop = edge.pop0(1) as usize; // old pop - 1
            // Hysteresis: back to a linear map leaf one below the cap.
            if pop < LEAF1_CAP {
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
                                n * 8,
                            );
                        }
                    }
                    a.free_node(
                        core::ptr::NonNull::new(edge.node_ptr().cast::<LeafBitmapL>()).unwrap(),
                    );
                }
                if entries.len() < map_immed_max(1) {
                    write_map_immed(a, edge, 1, &entries);
                } else {
                    build_map_leaf(a, edge, 1, &entries);
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
            let d = digit(key, level);
            let is_l3 = matches!(t, EdgeType::BranchL3);
            // SAFETY: live branch per contract.
            let removed = unsafe {
                if is_l3 {
                    let b = &mut *edge.node_ptr().cast::<BranchL3>();
                    let slot = b.hdr.find(d)?;
                    let r = map_remove(a, &mut b.edges[slot], key, level - 1);
                    if r.is_some() && b.edges[slot].is_null() {
                        linear_remove_slot(
                            &mut b.hdr.digits,
                            &mut b.edges,
                            b.hdr.num as usize,
                            slot,
                        );
                        b.hdr.num -= 1;
                    }
                    r
                } else {
                    let b = &mut *edge.node_ptr().cast::<BranchL7>();
                    let slot = b.hdr.find(d)?;
                    let r = map_remove(a, &mut b.edges[slot], key, level - 1);
                    if r.is_some() && b.edges[slot].is_null() {
                        linear_remove_slot(
                            &mut b.hdr.digits,
                            &mut b.edges,
                            b.hdr.num as usize,
                            slot,
                        );
                        b.hdr.num -= 1;
                    }
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
                bump_pop0(edge, level, -1);
                if !is_l3 && num < BRANCH_L3_CAP {
                    downgrade_l7_to_l3(a, edge);
                }
            }
            Some(old)
        }

        EdgeTag::Structural(EdgeType::BranchB) => {
            let d = digit(key, level);
            // SAFETY: live BranchB per contract.
            let b = unsafe { &mut *edge.node_ptr().cast::<BranchB>() };
            if !b.bitmap.test(d) {
                return None;
            }
            let sub = (d >> 5) as usize;
            let rank = b.bitmap.subexpanse_rank(d) as usize;
            // SAFETY: bitmap/subarray consistency invariant.
            let old = unsafe { map_remove(a, &mut *b.subarrays[sub].add(rank), key, level - 1)? };
            // SAFETY: child slot checked/live per invariant.
            let child_null = unsafe { (*b.subarrays[sub].add(rank)).is_null() };
            if child_null {
                let old_n = b.pop_counts[sub] as usize;
                // SAFETY: shrink-copy of the packed subarray.
                unsafe {
                    let old_arr = b.subarrays[sub];
                    if old_n == 1 {
                        b.subarrays[sub] = core::ptr::null_mut();
                    } else {
                        let new = a
                            .alloc_bytes((old_n - 1) * size_of::<Edge>())
                            .cast::<Edge>();
                        new.as_ptr().copy_from_nonoverlapping(old_arr, rank);
                        new.as_ptr()
                            .add(rank)
                            .copy_from_nonoverlapping(old_arr.add(rank + 1), old_n - 1 - rank);
                        b.subarrays[sub] = new.as_ptr();
                    }
                    a.free_bytes(
                        core::ptr::NonNull::new(old_arr.cast()).unwrap(),
                        old_n * size_of::<Edge>(),
                    );
                }
                b.pop_counts[sub] = (old_n - 1) as u16;
                b.bitmap.clear(d);
            }
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
            bump_pop0(edge, level, -1);
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
            let old = unsafe { map_remove(a, &mut b.edges[d as usize], key, level - 1)? };
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
                unsafe { downgrade_u_to_b(a, edge) };
            }
            Some(old)
        }
    }
}
