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
    BRANCHB_UP, LEAF_CAP, LEAF1_CAP, LEAFB1_DOWN, branch_form_level, bump_pop0, bump_pop0_dispatch,
    decode_value, divergence_level, downgrade_b_to_l7, downgrade_l7_to_l3, downgrade_u_to_b,
    free_branch_node, key_low, linear_insert_slot, linear_insert_slot_l3, linear_remove_slot,
    map_immed_max, read_packed, restore_decode, split_skip, sub_edges_size, sub_vals_size,
    upgrade_b_to_u, upgrade_l3_to_l7, upgrade_l7_to_b, wrap_skip_level, write_decode, write_packed,
    write_packed_fixed,
};
use crate::node::{BranchB, BranchL3, BranchL7, BranchU, Edge, LeafBitmapL};
use crate::occ::Cover;
use crate::types::{BRANCH_L3_CAP, BRANCH_L7_CAP, EdgeTag, EdgeType, ImmedType, Key, digit};
#[cfg(not(feature = "std"))]
use core_alloc::vec::Vec;

/// Allocation size of an immediate value array holding `n` values (`n >= 2`).
/// Sized with capacity classes so growing immediates shift in-place without reallocating.
#[inline(always)]
pub(crate) const fn map_immed_val_size(n: usize) -> usize {
    8 * crate::leaf::cap_class(n)
}

#[inline(always)]
pub(crate) unsafe fn read_packed_fixed(keys_ptr: *const u8, slot: usize, kb: u8) -> u64 {
    // SAFETY: forwarded contract.
    unsafe {
        match kb {
            1 => *keys_ptr.add(slot) as u64,
            2 => core::ptr::read_unaligned(keys_ptr.add(slot * 2).cast::<u16>()) as u64,
            3 => read_packed(keys_ptr, slot, 3),
            4 => core::ptr::read_unaligned(keys_ptr.add(slot * 4).cast::<u32>()) as u64,
            5 => read_packed(keys_ptr, slot, 5),
            6 => read_packed(keys_ptr, slot, 6),
            _ => read_packed(keys_ptr, slot, 7),
        }
    }
}

#[inline(always)]
pub(crate) unsafe fn leaf_locate_fixed(
    keys_ptr: *const u8,
    pop: usize,
    kb: u8,
    k: u64,
) -> Result<usize, usize> {
    // SAFETY: forwarded contract.
    unsafe {
        match kb {
            1 => leaf::locate_fixed::<1>(keys_ptr, pop, k),
            2 => leaf::locate_fixed::<2>(keys_ptr, pop, k),
            3 => leaf::locate_fixed::<3>(keys_ptr, pop, k),
            4 => leaf::locate_fixed::<4>(keys_ptr, pop, k),
            5 => leaf::locate_fixed::<5>(keys_ptr, pop, k),
            6 => leaf::locate_fixed::<6>(keys_ptr, pop, k),
            _ => leaf::locate_fixed::<7>(keys_ptr, pop, k),
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
        let vals = a
            .alloc_bytes(map_immed_val_size(entries.len()))
            .cast::<u64>();
        for (slot, &(_, v)) in entries.iter().enumerate() {
            // SAFETY: fresh array of entries.len() slots.
            unsafe { vals.as_ptr().add(slot).write(v) };
        }
        *edge = Edge::new_node(vals.as_ptr().cast(), 0);
    }
    edge.set_aux_bytes(aux);
    edge.set_tag(im.as_u8());
}

/// Fixed-size stack buffer for collecting up to 32 map entries during node
/// downgrades.
pub(crate) struct StackEntries32 {
    buf: [core::mem::MaybeUninit<(u64, u64)>; 32],
    len: usize,
}

impl StackEntries32 {
    #[inline(always)]
    pub(crate) fn new() -> Self {
        Self {
            buf: [core::mem::MaybeUninit::uninit(); 32],
            len: 0,
        }
    }

    #[inline(always)]
    pub(crate) fn push(&mut self, entry: (u64, u64)) {
        debug_assert!(self.len < 32);
        self.buf[self.len].write(entry);
        self.len += 1;
    }

    #[inline(always)]
    pub(crate) fn as_slice(&self) -> &[(u64, u64)] {
        // SAFETY: `len` elements have been written via `push`.
        unsafe { core::slice::from_raw_parts(self.buf.as_ptr().cast::<(u64, u64)>(), self.len) }
    }
}

/// Reads a map leaf's entries (sorted by key).
///
/// # Safety
///
/// The edge must reference a live map leaf of `pop` entries.
pub(crate) unsafe fn read_map_leaf(edge: &Edge, kb: u8, pop: usize) -> Vec<(u64, u64)> {
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
pub(crate) fn build_bitmap_leaf_map(a: &NodeAlloc, edge: &mut Edge, entries: &[(u64, u64)]) {
    let ptr = a.alloc_node_zeroed::<LeafBitmapL>();
    // SAFETY: ptr is freshly allocated zeroed LeafBitmapL memory.
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
    let vals = ptr.as_ptr().cast::<u64>();
    // SAFETY: freshly allocated leaf buffer holds keys at map_keys_offset.
    let keys = unsafe { ptr.as_ptr().add(leaf::map_keys_offset(pop)) };
    match kb {
        1 => {
            for (slot, &(k, v)) in entries.iter().enumerate() {
                // SAFETY: in-bounds writes of the fresh allocation.
                unsafe {
                    vals.add(slot).write(v);
                    write_packed_fixed::<1>(keys, slot, k);
                }
            }
        }
        2 => {
            for (slot, &(k, v)) in entries.iter().enumerate() {
                // SAFETY: in-bounds writes of the fresh allocation.
                unsafe {
                    vals.add(slot).write(v);
                    write_packed_fixed::<2>(keys, slot, k);
                }
            }
        }
        3 => {
            for (slot, &(k, v)) in entries.iter().enumerate() {
                // SAFETY: in-bounds writes of the fresh allocation.
                unsafe {
                    vals.add(slot).write(v);
                    write_packed_fixed::<3>(keys, slot, k);
                }
            }
        }
        4 => {
            for (slot, &(k, v)) in entries.iter().enumerate() {
                // SAFETY: in-bounds writes of the fresh allocation.
                unsafe {
                    vals.add(slot).write(v);
                    write_packed_fixed::<4>(keys, slot, k);
                }
            }
        }
        5 => {
            for (slot, &(k, v)) in entries.iter().enumerate() {
                // SAFETY: in-bounds writes of the fresh allocation.
                unsafe {
                    vals.add(slot).write(v);
                    write_packed_fixed::<5>(keys, slot, k);
                }
            }
        }
        6 => {
            for (slot, &(k, v)) in entries.iter().enumerate() {
                // SAFETY: in-bounds writes of the fresh allocation.
                unsafe {
                    vals.add(slot).write(v);
                    write_packed_fixed::<6>(keys, slot, k);
                }
            }
        }
        _ => {
            for (slot, &(k, v)) in entries.iter().enumerate() {
                // SAFETY: in-bounds writes of the fresh allocation.
                unsafe {
                    vals.add(slot).write(v);
                    write_packed_fixed::<7>(keys, slot, k);
                }
            }
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

/// Tracks the descent path of edges from the root to the active leaf
/// for fast multi-level sequential bypass.
#[derive(Clone, Copy)]
pub(crate) struct InsertPathMap {
    pub prefix: u64,
    pub edges: [*mut Edge; 8],
    pub levels: [u8; 8],
    pub depth: usize,
    pub leaf: *mut LeafBitmapL,
    pub leaf1: *mut u8,
    pub terminal_pop: u16,
    pub pending_pop: usize,
}

impl InsertPathMap {
    pub const fn empty() -> Self {
        Self {
            prefix: u64::MAX,
            edges: [core::ptr::null_mut(); 8],
            levels: [0; 8],
            depth: 0,
            leaf: core::ptr::null_mut(),
            leaf1: core::ptr::null_mut(),
            terminal_pop: 0,
            pending_pop: 0,
        }
    }

    #[inline(always)]
    pub fn record_ancestor(&mut self, edge: *mut Edge, level: u8) {
        if self.depth > 0 && self.depth < 8 {
            self.edges[self.depth] = edge;
            self.levels[self.depth] = level;
            self.depth += 1;
        }
    }

    #[inline(always)]
    pub unsafe fn flush(&mut self) {
        if self.pending_pop > 0 {
            let delta = self.pending_pop as i64;
            self.pending_pop = 0;
            for i in 1..self.depth {
                // SAFETY: path contains valid live edge pointers during active bypass.
                unsafe {
                    crate::mutate::bump_pop0(self.edges[i], self.levels[i], delta);
                }
            }
        }
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        if self.depth != 0 {
            if self.pending_pop > 0 {
                // SAFETY: flushing pending population before clearing path references.
                unsafe {
                    self.flush();
                }
            }
            self.prefix = u64::MAX;
            self.depth = 0;
            self.leaf = core::ptr::null_mut();
            self.leaf1 = core::ptr::null_mut();
            self.terminal_pop = 0;
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
pub(crate) unsafe fn map_insert<const KEEP: bool, const OCC: bool, const NESTED: bool>(
    a: &NodeAlloc,
    edge: &mut Edge,
    key: Key,
    val: u64,
    level: u8,
    cover: Cover,
) -> (Option<u64>, *mut u64) {
    // SAFETY: forwarded contract.
    unsafe {
        map_insert_with_path::<KEEP, OCC, NESTED>(
            a,
            edge,
            key,
            val,
            level,
            &mut InsertPathMap::empty(),
            cover,
        )
    }
}

/// Inserts `key → val` in the subtree at `edge` while recording the descent path
/// for fast sequential bypass.
///
/// # Safety
///
/// Same contract as [`map_insert`].
pub(crate) unsafe fn map_insert_with_path<const KEEP: bool, const OCC: bool, const NESTED: bool>(
    a: &NodeAlloc,
    edge: &mut Edge,
    key: Key,
    val: u64,
    level: u8,
    path: &mut InsertPathMap,
    cover: Cover,
) -> (Option<u64>, *mut u64) {
    if OCC {
        // SAFETY: forwarded contract.
        unsafe {
            map_insert_with_path_occ::<KEEP, OCC, NESTED>(a, edge, key, val, level, path, cover)
        }
    } else {
        // SAFETY: forwarded contract.
        unsafe { map_insert_with_path_flat::<KEEP>(a, edge as *mut Edge, key, val, level, path) }
    }
}

/// Iterative flat descent without function recursion or version checks for single-threaded maps.
#[inline(always)]
unsafe fn map_insert_with_path_flat<const KEEP: bool>(
    a: &NodeAlloc,
    mut edge: *mut Edge,
    key: Key,
    val: u64,
    mut level: u8,
    path: &mut InsertPathMap,
) -> (Option<u64>, *mut u64) {
    let mut ancestors: [(*mut Edge, u8); 8] = [(core::ptr::null_mut(), 0); 8];
    let mut anc_depth = 0;
    loop {
        debug_assert!((1..=8).contains(&level));
        // SAFETY: edge points to a live valid Edge in the trie.
        let tag = unsafe { (*edge).tag_byte() };
        match tag {
            0x00 => {
                path.clear();
                if level == 8 {
                    let node = a.alloc_node_zeroed::<BranchL3>();
                    // SAFETY: node is freshly allocated zeroed BranchL3 memory.
                    unsafe {
                        (*node.as_ptr()).hdr.level = level;
                        *edge = Edge::new_node(node.as_ptr().cast(), EdgeType::BranchL3.as_u8());
                    }
                    continue;
                }
                let kb = level;
                let k = key_low(key, kb);
                // SAFETY: write 1-key immediate; ancestors array is valid.
                unsafe {
                    *edge = Edge::new_immed_single_map(kb, k, val);
                    for &(anc, al) in ancestors.iter().take(anc_depth) {
                        bump_pop0(anc, al, 1);
                        path.record_ancestor(anc, al);
                    }
                    return (None, (&raw mut *edge).cast::<u64>());
                }
            }

            0x01 => {
                debug_assert!(level >= 2);
                // SAFETY: edge points to a live BranchL3 node; raw pointer derivations avoid creating unique references over parent edges.
                unsafe {
                    let b_ptr = (*edge).node_ptr().cast::<BranchL3>();
                    let bl = (*b_ptr).hdr.level;
                    if bl < level && !crate::get::decode_matches(&*edge, key, bl, level) {
                        let pop = (*edge).pop0(bl) + 1;
                        path.clear();
                        split_skip(a, &mut *edge, key, level, pop);
                        continue;
                    }
                    let d = digit(key, bl);
                    let num = (*b_ptr).hdr.num as usize;
                    let found = if num >= 1 && (*b_ptr).hdr.digits[0] == d {
                        Some(0)
                    } else if num >= 2 && (*b_ptr).hdr.digits[1] == d {
                        Some(1)
                    } else if num >= 3 && (*b_ptr).hdr.digits[2] == d {
                        Some(2)
                    } else {
                        None
                    };
                    if let Some(slot) = found {
                        ancestors[anc_depth] = (edge, bl);
                        anc_depth += 1;
                        edge = &raw mut (*b_ptr).edges[slot];
                        level = bl - 1;
                        continue;
                    }
                    if num == BRANCH_L3_CAP {
                        path.clear();
                        upgrade_l3_to_l7::<false>(a, &mut *edge);
                        continue;
                    }
                    let slot = linear_insert_slot_l3(
                        &mut (*b_ptr).hdr.digits,
                        &mut (*b_ptr).edges,
                        num,
                        d,
                    );
                    (*b_ptr).hdr.num += 1;
                    (*b_ptr).hdr.add_presence(d);
                    ancestors[anc_depth] = (edge, bl);
                    anc_depth += 1;
                    edge = &raw mut (*b_ptr).edges[slot];
                    level = bl - 1;
                    continue;
                }
            }

            0x02 => {
                debug_assert!(level >= 2);
                // SAFETY: edge points to a live BranchL7 node; raw pointer derivations avoid creating unique references over parent edges.
                unsafe {
                    let b_ptr = (*edge).node_ptr().cast::<BranchL7>();
                    let bl = (*b_ptr).hdr.level;
                    if bl < level && !crate::get::decode_matches(&*edge, key, bl, level) {
                        let pop = (*edge).pop0(bl) + 1;
                        path.clear();
                        split_skip(a, &mut *edge, key, level, pop);
                        continue;
                    }
                    let d = digit(key, bl);
                    if let Some(slot) = (*b_ptr).hdr.find(d) {
                        ancestors[anc_depth] = (edge, bl);
                        anc_depth += 1;
                        edge = &raw mut (*b_ptr).edges[slot];
                        level = bl - 1;
                        continue;
                    }
                    let num = (*b_ptr).hdr.num as usize;
                    if num == BRANCH_L7_CAP {
                        path.clear();
                        upgrade_l7_to_b::<false>(a, &mut *edge);
                        continue;
                    }
                    let slot =
                        linear_insert_slot(&mut (*b_ptr).hdr.digits, &mut (*b_ptr).edges, num, d);
                    (*b_ptr).hdr.num += 1;
                    (*b_ptr).hdr.add_presence(d);
                    ancestors[anc_depth] = (edge, bl);
                    anc_depth += 1;
                    edge = &raw mut (*b_ptr).edges[slot];
                    level = bl - 1;
                    continue;
                }
            }

            0x03 => {
                debug_assert!(level >= 2);
                // SAFETY: edge is a live BranchB node.
                let bl = unsafe { branch_form_level(&*edge, EdgeType::BranchB, level) };
                // SAFETY: edge is a live BranchB node.
                if bl < level && !unsafe { crate::get::decode_matches(&*edge, key, bl, level) } {
                    // SAFETY: edge is a live BranchB node.
                    let pop = unsafe { (*edge).pop0(bl) + 1 };
                    path.clear();
                    // SAFETY: split_skip maintains valid tree invariants.
                    unsafe { split_skip(a, &mut *edge, key, level, pop) };
                    continue;
                }
                let slot_level = level;
                let d = digit(key, bl);
                // SAFETY: edge is a live BranchB node.
                let b = unsafe { &mut *(*edge).node_ptr().cast::<BranchB>() };
                if let Some(slot) = b.bitmap.test_and_subexpanse_rank(d) {
                    let sub = b.subarrays[(d >> 5) as usize];
                    ancestors[anc_depth] = (edge, bl);
                    anc_depth += 1;
                    // SAFETY: sub points to valid live subarray; slot is in-bounds.
                    edge = unsafe { sub.add(slot) };
                    level = bl - 1;
                    continue;
                }
                if b.bitmap.count() as usize + 1 > BRANCHB_UP {
                    if bl < slot_level {
                        // SAFETY: edge is a live BranchB node.
                        let pop = unsafe { (*edge).pop0(bl) + 1 };
                        path.clear();
                        // SAFETY: wrap_skip_level maintains valid tree invariants.
                        unsafe { wrap_skip_level(a, &mut *edge, bl + 1, slot_level, pop) };
                        level = slot_level;
                        continue;
                    }
                    path.clear();
                    // SAFETY: upgrade_b_to_u upgrades live BranchB to BranchU.
                    unsafe {
                        upgrade_b_to_u::<false>(a, &mut *edge);
                    }
                    level = slot_level;
                    continue;
                }
                let sub = (d >> 5) as usize;
                let old_n = b.pop_counts[sub] as usize;
                let rank = b.bitmap.subexpanse_rank(d) as usize;
                if old_n > 0 && leaf::cap_class(old_n + 1) == leaf::cap_class(old_n) {
                    // SAFETY: spare class capacity; shift subarray in-place.
                    unsafe {
                        let arr = b.subarrays[sub];
                        core::ptr::copy(arr.add(rank), arr.add(rank + 1), old_n - rank);
                        arr.add(rank).write(Edge::NULL);
                    }
                } else {
                    let new = a.alloc_bytes(sub_edges_size(old_n + 1)).cast::<Edge>();
                    // SAFETY: allocate fresh subarray, copy old entries, write NULL, free old subarray.
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
                ancestors[anc_depth] = (edge, bl);
                anc_depth += 1;
                // SAFETY: sub is valid subarray with old_n + 1 edges; rank is in-bounds.
                edge = unsafe { b.subarrays[sub].add(rank) };
                level = bl - 1;
                continue;
            }

            0x04 => {
                debug_assert!(level >= 2);
                let d = digit(key, level);
                // SAFETY: edge is a live BranchU node.
                let b = unsafe { &mut *(*edge).node_ptr().cast::<BranchU>() };
                ancestors[anc_depth] = (edge, level);
                anc_depth += 1;
                edge = &raw mut b.edges[d as usize];
                level -= 1;
                continue;
            }

            0x0C => {
                // SAFETY: edge is a live LeafB1 edge.
                if level > 1 && !unsafe { crate::get::decode_matches(&*edge, key, 1, level) } {
                    // SAFETY: edge is a live LeafB1 edge.
                    let pop = unsafe { (*edge).pop0(1) + 1 };
                    path.clear();
                    // SAFETY: split_skip maintains valid tree invariants.
                    unsafe { split_skip(a, &mut *edge, key, level, pop) };
                    continue;
                }
                a.assert_bracketed();
                let d = digit(key, 1);
                let sub = (d >> 5) as usize;
                // SAFETY: edge points to a live LeafBitmapL node.
                let node = unsafe { &mut *(*edge).node_ptr().cast::<LeafBitmapL>() };
                if let Some(rank) = node.bitmap.test_and_subexpanse_rank(d) {
                    // SAFETY: rank is in-bounds for subarray; slot is readable/writable.
                    unsafe {
                        let slot = node.values[sub].add(rank);
                        let old = *slot;
                        if !KEEP {
                            slot.write(val);
                        }
                        return (Some(old), slot);
                    }
                }
                let rank = node.bitmap.subexpanse_rank(d) as usize;
                let old_n = node.bitmap.subexpanse_count(sub) as usize;
                if old_n > 0 && leaf::cap_class(old_n + 1) == leaf::cap_class(old_n) {
                    // SAFETY: spare class capacity; shift values in-place and write val.
                    unsafe {
                        let arr = node.values[sub];
                        core::ptr::copy(arr.add(rank), arr.add(rank + 1), old_n - rank);
                        arr.add(rank).write(val);
                    }
                } else {
                    let new = a.alloc_bytes(sub_vals_size(old_n + 1)).cast::<u64>();
                    // SAFETY: allocate fresh subarray, copy old entries, write val, free old subarray.
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
                // SAFETY: edge is a live LeafB1 edge.
                let pop0 = unsafe { (*edge).pop0(1) };
                // SAFETY: edge is a live LeafB1 edge.
                unsafe { (*edge).set_pop0(1, pop0 + 1) };
                if level == 1 {
                    path.prefix = key >> 8;
                    // SAFETY: edge points to a live LeafBitmapL node.
                    path.leaf = unsafe { (*edge).node_ptr().cast::<LeafBitmapL>() };
                    path.leaf1 = core::ptr::null_mut();
                    path.terminal_pop = (pop0 + 2) as u16;
                    path.edges[0] = edge;
                    path.levels[0] = 1;
                    path.depth = 1;
                    path.pending_pop = 0;
                }
                // SAFETY: node.values[sub] holds at least rank + 1 slots.
                let slot = unsafe { node.values[sub].add(rank) };
                // SAFETY: ancestors contains valid parent edges.
                unsafe {
                    for &(anc, al) in ancestors.iter().take(anc_depth) {
                        bump_pop0(anc, al, 1);
                        path.record_ancestor(anc, al);
                    }
                }
                return (None, slot);
            }

            0x05..=0x0B => {
                path.clear();
                let kb = tag - 0x04;
                debug_assert!(kb <= level);
                // SAFETY: edge is a live linear leaf edge.
                let pop = unsafe { (*edge).pop0(kb) as usize + 1 };
                // SAFETY: edge is a live linear leaf edge.
                if kb < level && !unsafe { crate::get::decode_matches(&*edge, key, kb, level) } {
                    // SAFETY: split_skip maintains valid tree invariants.
                    unsafe { split_skip(a, &mut *edge, key, level, pop as u64) };
                    continue;
                }
                let k = key_low(key, kb);
                // SAFETY: edge is a live linear leaf edge.
                let base = unsafe { (*edge).node_ptr() };
                // SAFETY: keys live behind values at map_keys_offset.
                let keys_ptr = unsafe { base.add(leaf::map_keys_offset(pop)) };
                let (hit, pos) = if pop > 0 {
                    // SAFETY: pop > 0 guarantees slot pop - 1 is in-bounds.
                    let last = unsafe { read_packed_fixed(keys_ptr, pop - 1, kb) };
                    if k > last {
                        (false, pop)
                    } else if k == last {
                        (true, pop - 1)
                    } else {
                        // SAFETY: keys_ptr holds pop packed keys.
                        match unsafe { leaf_locate_fixed(keys_ptr, pop, kb, k) } {
                            Ok(p) => (true, p),
                            Err(p) => (false, p),
                        }
                    }
                } else {
                    (false, 0)
                };
                match if hit { Ok(pos) } else { Err(pos) } {
                    Ok(pos) => {
                        // SAFETY: pos is within live leaf value slots.
                        unsafe {
                            let slot = base.cast::<u64>().add(pos);
                            let old = *slot;
                            if !KEEP {
                                slot.write(val);
                            }
                            return (Some(old), slot);
                        }
                    }
                    Err(pos) => {
                        let cap = if kb == 1 { LEAF1_CAP } else { LEAF_CAP };
                        if pop < cap && leaf::cap_class(pop + 1) == leaf::cap_class(pop) {
                            a.assert_bracketed();
                            // SAFETY: class capacity holds pop + 1 entries; in-place shifts and write.
                            unsafe {
                                leaf::map_insert_at(base, kb, pop, pos, k, val);
                                (*edge).set_pop0(kb, pop as u64);
                            }
                            // SAFETY: slot pos is in-bounds of live leaf value slots.
                            let slot = unsafe { base.cast::<u64>().add(pos) };
                            if kb == 1 && level == 1 {
                                path.prefix = key >> 8;
                                path.leaf = core::ptr::null_mut();
                                path.leaf1 = base;
                                path.terminal_pop = (pop + 1) as u16;
                                path.edges[0] = edge;
                                path.levels[0] = 1;
                                path.depth = 1;
                                path.pending_pop = 0;
                            } else {
                                path.clear();
                            }
                            // SAFETY: ancestors array is valid.
                            unsafe {
                                for &(anc, al) in ancestors.iter().take(anc_depth) {
                                    bump_pop0(anc, al, 1);
                                    path.record_ancestor(anc, al);
                                }
                            }
                            return (None, slot);
                        }
                        let old_ptr = base;
                        let old_size = leaf::size_map(kb, pop);
                        // SAFETY: edge is a live linear leaf edge.
                        let saved_aux = unsafe { *(*edge).aux_bytes() };
                        if pop < cap {
                            let new = a.alloc_bytes(leaf::size_map(kb, pop + 1));
                            // SAFETY: realloc_insert into fresh buffer, free old leaf; ancestors valid.
                            let slot = unsafe {
                                leaf::map_realloc_insert(base, new.as_ptr(), kb, pop, pos, k, val);
                                *edge = Edge::new_node(new.as_ptr(), (*edge).tag_byte());
                                (*edge).set_aux_bytes(saved_aux);
                                (*edge).set_pop0(kb, pop as u64);
                                a.free_bytes(
                                    core::ptr::NonNull::new(old_ptr).expect("leaf ptr"),
                                    old_size,
                                );
                                new.as_ptr().cast::<u64>().add(pos)
                            };
                            if kb == 1 && level == 1 {
                                path.prefix = key >> 8;
                                path.leaf = core::ptr::null_mut();
                                path.leaf1 = new.as_ptr();
                                path.terminal_pop = (pop + 1) as u16;
                                path.edges[0] = edge;
                                path.levels[0] = 1;
                                path.depth = 1;
                                path.pending_pop = 0;
                            } else {
                                path.clear();
                            }
                            // SAFETY: ancestors array is valid.
                            unsafe {
                                for &(anc, al) in ancestors.iter().take(anc_depth) {
                                    bump_pop0(anc, al, 1);
                                    path.record_ancestor(anc, al);
                                }
                            }
                            return (None, slot);
                        }
                        // SAFETY: edge is a live linear leaf edge of pop entries.
                        let mut entries = unsafe { read_map_leaf(&*edge, kb, pop) };
                        // SAFETY: edge is a live linear leaf edge.
                        let old_ptr = unsafe { (*edge).node_ptr() };
                        let old_size = leaf::size_map(kb, pop);
                        // SAFETY: edge is a live linear leaf edge.
                        let saved_aux = unsafe { *(*edge).aux_bytes() };
                        entries.insert(pos, (k, val));
                        if kb < level {
                            // SAFETY: edge is a live linear leaf edge.
                            let prefix =
                                unsafe { decode_value(&*edge, kb, level) } << (8 * u32::from(kb));
                            for e in &mut entries {
                                e.0 |= prefix;
                            }
                        }
                        if entries.len() <= cap {
                            // SAFETY: build fresh map leaf and restore decode.
                            unsafe {
                                build_map_leaf(a, &mut *edge, kb, &entries);
                                restore_decode(&mut *edge, kb, level, &saved_aux);
                            }
                        } else if kb == 1 {
                            // SAFETY: edge is a live valid edge.
                            build_bitmap_leaf_map(a, unsafe { &mut *edge }, &entries);
                            // SAFETY: edge is a live valid edge.
                            restore_decode(unsafe { &mut *edge }, 1, level, &saved_aux);
                        } else if divergence_level(
                            entries[0].0,
                            entries[entries.len() - 1].0,
                            level,
                        ) == 1
                        {
                            let low: Vec<(u64, u64)> =
                                entries.iter().map(|&(k, v)| (key_low(k, 1), v)).collect();
                            let prefix_key = entries[0].0;
                            // SAFETY: edge is a live valid edge.
                            build_bitmap_leaf_map(a, unsafe { &mut *edge }, &low);
                            // SAFETY: edge is a live valid edge.
                            write_decode(unsafe { &mut *edge }, 1, level, prefix_key);
                            if level == 1 {
                                path.prefix = key >> 8;
                                // SAFETY: edge points to newly allocated LeafBitmapL.
                                path.leaf = unsafe { (*edge).node_ptr().cast::<LeafBitmapL>() };
                                path.leaf1 = core::ptr::null_mut();
                                path.terminal_pop = entries.len() as u16;
                                path.edges[0] = edge;
                                path.levels[0] = 1;
                                path.depth = 1;
                                path.pending_pop = 0;
                            }
                        } else {
                            let d =
                                divergence_level(entries[0].0, entries[entries.len() - 1].0, level);
                            let bl = if level <= 7 { d } else { level };
                            let node = a.alloc_node_zeroed::<BranchL3>();
                            // SAFETY: populate fresh BranchL3, insert entries, set pop0.
                            unsafe {
                                (*node.as_ptr()).hdr.level = bl;
                                *edge = Edge::new_node(
                                    node.as_ptr().cast(),
                                    EdgeType::BranchL3.as_u8(),
                                );
                                if bl < level {
                                    write_decode(&mut *edge, bl, level, entries[0].0);
                                }
                                for &(k, v) in &entries {
                                    let prev = map_insert_with_path_flat::<KEEP>(
                                        a, edge, k, v, level, path,
                                    );
                                    debug_assert!(prev.0.is_none());
                                }
                                (*edge).set_pop0(bl, entries.len() as u64 - 1);
                            }
                        }
                        // SAFETY: free old linear leaf and update ancestors.
                        unsafe {
                            a.free_bytes(
                                core::ptr::NonNull::new(old_ptr).expect("leaf ptr"),
                                old_size,
                            );
                            for &(anc, al) in ancestors.iter().take(anc_depth) {
                                bump_pop0(anc, al, 1);
                                path.record_ancestor(anc, al);
                            }
                        }
                        // SAFETY: edge is a live valid subtree containing key.
                        let slot = unsafe { crate::get::locate_slot(&raw mut *edge, key, level) }
                            .expect("just-inserted key")
                            .as_ptr();
                        return (None, slot);
                    }
                }
            }

            0x7F => unreachable!("full-expanse edges are set-flavor only"),

            _ => {
                let im = ImmedType::from_u8(tag).expect("valid immediate tag");
                debug_assert_eq!(im.key_bytes(), level);
                path.clear();
                let kb = im.key_bytes();
                let kb_usize = kb as usize;
                let k = key_low(key, kb);
                let n = im.key_count() as usize;
                if n == 1 {
                    let mask = if kb >= 8 {
                        u64::MAX
                    } else {
                        (1u64 << (kb * 8)) - 1
                    };
                    // SAFETY: live 1-key immediate edge; read aux_word.
                    let existing_k = unsafe { (*edge).aux_word() } & mask;
                    if existing_k == k {
                        // SAFETY: live 1-key immediate edge; read word0.
                        let old = unsafe { (*edge).word0() };
                        if !KEEP {
                            // SAFETY: edge is a live 1-key immediate.
                            unsafe { (*edge).set_imm_bytes(val.to_le_bytes()) };
                        }
                        return (Some(old), (&raw mut *edge).cast::<u64>());
                    }
                    // SAFETY: live 1-key immediate edge; read word0.
                    let old_val = unsafe { (*edge).word0() };
                    let (slot0_k, slot0_v, slot1_k, slot1_v, pos) = if k < existing_k {
                        (k, val, existing_k, old_val, 0)
                    } else {
                        (existing_k, old_val, k, val, 1)
                    };
                    if map_immed_max(kb) >= 2 {
                        let vals = a.alloc_bytes(map_immed_val_size(2)).cast::<u64>();
                        // SAFETY: populate 2-entry array and install into edge; ancestors valid.
                        unsafe {
                            vals.as_ptr().write(slot0_v);
                            vals.as_ptr().add(1).write(slot1_v);
                            let mut new_aux = [0u8; 7];
                            write_packed(new_aux.as_mut_ptr(), 0, kb_usize, slot0_k);
                            write_packed(new_aux.as_mut_ptr(), 1, kb_usize, slot1_k);
                            let new_im = ImmedType::new(kb, 2).expect("immediate capacity");
                            *edge = Edge::new_node(vals.as_ptr().cast(), 0);
                            (*edge).set_aux_bytes(new_aux);
                            (*edge).set_tag(new_im.as_u8());
                            for &(anc, al) in ancestors.iter().take(anc_depth) {
                                bump_pop0(anc, al, 1);
                                path.record_ancestor(anc, al);
                            }
                            return (None, vals.as_ptr().add(pos));
                        }
                    } else {
                        let entries = [(slot0_k, slot0_v), (slot1_k, slot1_v)];
                        // SAFETY: build fresh map leaf and update ancestors.
                        unsafe {
                            build_map_leaf(a, &mut *edge, kb, &entries);
                            for &(anc, al) in ancestors.iter().take(anc_depth) {
                                bump_pop0(anc, al, 1);
                                path.record_ancestor(anc, al);
                            }
                            return (None, (*edge).node_ptr().cast::<u64>().add(pos));
                        }
                    }
                }
                // SAFETY: edge is a live immediate with n keys.
                let pos = match unsafe { leaf::locate((*edge).aux_bytes().as_ptr(), n, kb, k) } {
                    Ok(p) => {
                        // SAFETY: p is in-bounds of live value array.
                        let slot = unsafe { (*edge).node_ptr().cast::<u64>().add(p) };
                        // SAFETY: slot is in-bounds and readable.
                        let old = unsafe { *slot };
                        if !KEEP {
                            // SAFETY: slot is writable.
                            unsafe { slot.write(val) };
                        }
                        return (Some(old), slot);
                    }
                    Err(p) => p,
                };
                // SAFETY: edge points to live value array.
                let old_vals = unsafe { (*edge).node_ptr().cast::<u64>() };
                if n < map_immed_max(kb) {
                    let kb_usize = kb as usize;
                    if leaf::cap_class(n + 1) == leaf::cap_class(n) {
                        a.assert_bracketed();
                        // SAFETY: class capacity holds n + 1 entries; in-place shifts and write; ancestors valid.
                        unsafe {
                            if pos < n {
                                core::ptr::copy(old_vals.add(pos), old_vals.add(pos + 1), n - pos);
                            }
                            old_vals.add(pos).write(val);
                            let mut new_aux = *(*edge).aux_bytes();
                            if pos < n {
                                new_aux.copy_within(
                                    pos * kb_usize..n * kb_usize,
                                    (pos + 1) * kb_usize,
                                );
                            }
                            write_packed(new_aux.as_mut_ptr(), pos, kb_usize, k);
                            let new_im =
                                ImmedType::new(kb, (n + 1) as u8).expect("immediate capacity");
                            (*edge).set_aux_bytes(new_aux);
                            (*edge).set_tag(new_im.as_u8());
                            for &(anc, al) in ancestors.iter().take(anc_depth) {
                                bump_pop0(anc, al, 1);
                                path.record_ancestor(anc, al);
                            }
                            return (None, old_vals.add(pos));
                        }
                    }
                    let new_vals = a.alloc_bytes(map_immed_val_size(n + 1)).cast::<u64>();
                    // SAFETY: copy n values into fresh array, write val, free old array; ancestors valid.
                    unsafe {
                        if pos > 0 {
                            core::ptr::copy_nonoverlapping(old_vals, new_vals.as_ptr(), pos);
                        }
                        new_vals.as_ptr().add(pos).write(val);
                        if pos < n {
                            core::ptr::copy_nonoverlapping(
                                old_vals.add(pos),
                                new_vals.as_ptr().add(pos + 1),
                                n - pos,
                            );
                        }
                        a.free_bytes(
                            core::ptr::NonNull::new((*edge).node_ptr()).expect("value array"),
                            map_immed_val_size(n),
                        );
                        let mut new_aux = *(*edge).aux_bytes();
                        if pos < n {
                            new_aux.copy_within(pos * kb_usize..n * kb_usize, (pos + 1) * kb_usize);
                        }
                        write_packed(new_aux.as_mut_ptr(), pos, kb_usize, k);
                        let new_im = ImmedType::new(kb, (n + 1) as u8).expect("immediate capacity");
                        *edge = Edge::new_node(new_vals.as_ptr().cast(), 0);
                        (*edge).set_aux_bytes(new_aux);
                        (*edge).set_tag(new_im.as_u8());
                        for &(anc, al) in ancestors.iter().take(anc_depth) {
                            bump_pop0(anc, al, 1);
                            path.record_ancestor(anc, al);
                        }
                        return (None, new_vals.as_ptr().add(pos));
                    }
                }
                let ptr = a.alloc_bytes(leaf::size_map(kb, n + 1));
                let vals = ptr.as_ptr().cast::<u64>();
                // SAFETY: freshly allocated leaf buffer holds keys at map_keys_offset.
                let keys = unsafe { ptr.as_ptr().add(leaf::map_keys_offset(n + 1)) };
                let kb_usize = kb as usize;
                // SAFETY: ptr is freshly allocated with size for n + 1 entries; free old value array; ancestors valid.
                unsafe {
                    if pos > 0 {
                        core::ptr::copy_nonoverlapping(old_vals, vals, pos);
                        core::ptr::copy_nonoverlapping(
                            (*edge).aux_bytes().as_ptr(),
                            keys,
                            pos * kb_usize,
                        );
                    }
                    vals.add(pos).write(val);
                    write_packed(keys, pos, kb_usize, k);
                    if pos < n {
                        core::ptr::copy_nonoverlapping(
                            old_vals.add(pos),
                            vals.add(pos + 1),
                            n - pos,
                        );
                        core::ptr::copy_nonoverlapping(
                            (*edge).aux_bytes().as_ptr().add(pos * kb_usize),
                            keys.add((pos + 1) * kb_usize),
                            (n - pos) * kb_usize,
                        );
                    }
                    a.free_bytes(
                        core::ptr::NonNull::new((*edge).node_ptr()).expect("value array"),
                        map_immed_val_size(n),
                    );
                    *edge = Edge::new_node(ptr.as_ptr(), EdgeType::Leaf1 as u8 + (kb - 1));
                    (*edge).set_pop0(kb, n as u64);
                    for &(anc, al) in ancestors.iter().take(anc_depth) {
                        bump_pop0(anc, al, 1);
                        path.record_ancestor(anc, al);
                    }
                    return (None, vals.add(pos));
                }
            }
        }
    }
}

/// Recursive descent for shared (OCC) maps, bracketed by the covering
/// function (#568 PR 3): `cover` is the version word of the node whose slot
/// `edge` is (the tree word at the top). Every store this frame makes to
/// `*edge`, or to the leaf / immediate / subarray payload it points at, is
/// bracketed by `cover`; a branch child's frame is entered *unbracketed*
/// with this node's own word as its cover, so a node's word is odd only
/// while one frame stores into that node, never for the whole descent.
///
/// The insert-path cache is never consulted on a shared map (the owner's
/// bypass is compiled out at `OCC = true`), so it is cleared here and never
/// recorded.
unsafe fn map_insert_with_path_occ<const KEEP: bool, const OCC: bool, const NESTED: bool>(
    a: &NodeAlloc,
    edge: &mut Edge,
    key: Key,
    val: u64,
    mut level: u8,
    path: &mut InsertPathMap,
    cover: Cover,
) -> (Option<u64>, *mut u64) {
    path.clear();
    loop {
        debug_assert!((1..=8).contains(&level));
        let tag = edge.tag().expect("valid edge tag");
        match tag {
            EdgeTag::Structural(EdgeType::Null) => {
                if level == 8 {
                    let node = a.alloc_node_zeroed::<BranchL3>();
                    // SAFETY: node is freshly allocated zeroed BranchL3 memory.
                    unsafe {
                        (*node.as_ptr()).hdr.level = level;
                    }
                    cover.begin_if::<OCC, NESTED>(a);
                    *edge = Edge::new_node(node.as_ptr().cast(), EdgeType::BranchL3.as_u8());
                    cover.end_if::<OCC, NESTED>(a);
                    // SAFETY: forwarded contract; edge is now a branch.
                    continue;
                }
                let kb = level;
                let k = key_low(key, kb);
                let im = ImmedType::new(kb, 1).expect("immediate 1 key");
                let mut aux = [0u8; 7];
                // SAFETY: aux has 7 bytes; 1 key of kb bytes fits per kb <= 7.
                unsafe {
                    write_packed(aux.as_mut_ptr(), 0, kb as usize, k);
                }
                cover.begin_if::<OCC, NESTED>(a);
                *edge = Edge::NULL;
                edge.set_imm_bytes(val.to_le_bytes());
                edge.set_aux_bytes(aux);
                edge.set_tag(im.as_u8());
                cover.end_if::<OCC, NESTED>(a);
                // A single-entry immediate's value slot is the edge's word 0.
                return (None, (&raw mut *edge).cast::<u64>());
            }

            EdgeTag::Immed(im) => {
                debug_assert_eq!(im.key_bytes(), level);
                let kb = im.key_bytes();
                let kb_usize = kb as usize;
                let k = key_low(key, kb);
                let n = im.key_count() as usize;
                if n == 1 {
                    // SAFETY: single-key map immediate holds 1 key in aux bytes.
                    let existing_k = unsafe { read_packed(edge.aux_bytes().as_ptr(), 0, kb_usize) };
                    if existing_k == k {
                        let old = u64::from_le_bytes(edge.imm_bytes());
                        if !KEEP {
                            cover.begin_if::<OCC, NESTED>(a);
                            edge.set_imm_bytes(val.to_le_bytes());
                            cover.end_if::<OCC, NESTED>(a);
                        }
                        return (Some(old), (&raw mut *edge).cast::<u64>());
                    }
                    let old_val = u64::from_le_bytes(edge.imm_bytes());
                    let (slot0_k, slot0_v, slot1_k, slot1_v, pos) = if k < existing_k {
                        (k, val, existing_k, old_val, 0)
                    } else {
                        (existing_k, old_val, k, val, 1)
                    };
                    if map_immed_max(kb) >= 2 {
                        let vals = a.alloc_bytes(map_immed_val_size(2)).cast::<u64>();
                        // SAFETY: fresh 2-slot value array.
                        unsafe {
                            vals.as_ptr().write(slot0_v);
                            vals.as_ptr().add(1).write(slot1_v);
                        }
                        let mut new_aux = [0u8; 7];
                        // SAFETY: new_aux has 7 bytes; 2 keys of kb bytes fit per map_immed_max >= 2.
                        unsafe {
                            write_packed(new_aux.as_mut_ptr(), 0, kb_usize, slot0_k);
                            write_packed(new_aux.as_mut_ptr(), 1, kb_usize, slot1_k);
                        }
                        let new_im = ImmedType::new(kb, 2).expect("immediate capacity");
                        cover.begin_if::<OCC, NESTED>(a);
                        *edge = Edge::new_node(vals.as_ptr().cast(), 0);
                        edge.set_aux_bytes(new_aux);
                        edge.set_tag(new_im.as_u8());
                        cover.end_if::<OCC, NESTED>(a);
                        // SAFETY: slot `pos` in the newly allocated value array.
                        return (None, unsafe { vals.as_ptr().add(pos) });
                    } else {
                        // Immediate max capacity is 1 (for kb in 4..=7): upgrade directly to linear leaf.
                        let entries = [(slot0_k, slot0_v), (slot1_k, slot1_v)];
                        cover.begin_if::<OCC, NESTED>(a);
                        build_map_leaf(a, edge, kb, &entries);
                        cover.end_if::<OCC, NESTED>(a);
                        // SAFETY: build_map_leaf places value array at the base of the leaf.
                        return (None, unsafe { edge.node_ptr().cast::<u64>().add(pos) });
                    }
                }
                // SAFETY: aux bytes hold n packed keys of kb bytes.
                let pos = match unsafe { leaf::locate(edge.aux_bytes().as_ptr(), n, kb, k) } {
                    Ok(p) => {
                        // SAFETY: live value array per contract.
                        let slot = unsafe { edge.node_ptr().cast::<u64>().add(p) };
                        // SAFETY: slot is within the allocated n-value array.
                        let old = unsafe { *slot };
                        if !KEEP {
                            // A value store readers validate against the
                            // parent's word: bracketed like any other.
                            cover.begin_if::<OCC, NESTED>(a);
                            // SAFETY: slot is writable per contract.
                            unsafe { slot.write(val) };
                            cover.end_if::<OCC, NESTED>(a);
                        }
                        return (Some(old), slot);
                    }
                    Err(p) => p,
                };
                let old_vals = edge.node_ptr().cast::<u64>();
                if n < map_immed_max(kb) {
                    if leaf::cap_class(n + 1) == leaf::cap_class(n) {
                        // Spare class capacity: shift values and keys in
                        // place. Readers validate the value array against
                        // the parent's word.
                        let mut new_aux = *edge.aux_bytes();
                        if pos < n {
                            new_aux.copy_within(pos * kb_usize..n * kb_usize, (pos + 1) * kb_usize);
                        }
                        // SAFETY: new_aux has capacity for n + 1 packed keys.
                        unsafe {
                            write_packed(new_aux.as_mut_ptr(), pos, kb_usize, k);
                        }
                        let new_im = ImmedType::new(kb, (n + 1) as u8).expect("immediate capacity");
                        cover.begin_if::<OCC, NESTED>(a);
                        a.assert_bracketed_by(cover.addr(a));
                        // SAFETY: class capacity holds n + 1 entries; in-bounds shifts.
                        unsafe {
                            if pos < n {
                                core::ptr::copy(old_vals.add(pos), old_vals.add(pos + 1), n - pos);
                            }
                            old_vals.add(pos).write(val);
                        }
                        edge.set_aux_bytes(new_aux);
                        edge.set_tag(new_im.as_u8());
                        cover.end_if::<OCC, NESTED>(a);
                        // SAFETY: old_vals has capacity for n + 1 entries; pos is in-bounds.
                        return (None, unsafe { old_vals.add(pos) });
                    }
                    let new_vals = a.alloc_bytes(map_immed_val_size(n + 1)).cast::<u64>();
                    // SAFETY: copy n values around pos into the private array, write val at pos.
                    unsafe {
                        if pos > 0 {
                            core::ptr::copy_nonoverlapping(old_vals, new_vals.as_ptr(), pos);
                        }
                        new_vals.as_ptr().add(pos).write(val);
                        if pos < n {
                            core::ptr::copy_nonoverlapping(
                                old_vals.add(pos),
                                new_vals.as_ptr().add(pos + 1),
                                n - pos,
                            );
                        }
                    }
                    let mut new_aux = *edge.aux_bytes();
                    if pos < n {
                        new_aux.copy_within(pos * kb_usize..n * kb_usize, (pos + 1) * kb_usize);
                    }
                    // SAFETY: new_aux has 7 bytes; (n + 1) * kb_usize <= 7.
                    unsafe {
                        write_packed(new_aux.as_mut_ptr(), pos, kb_usize, k);
                    }
                    let new_im = ImmedType::new(kb, (n + 1) as u8).expect("immediate capacity");
                    cover.begin_if::<OCC, NESTED>(a);
                    *edge = Edge::new_node(new_vals.as_ptr().cast(), 0);
                    edge.set_aux_bytes(new_aux);
                    edge.set_tag(new_im.as_u8());
                    cover.end_if::<OCC, NESTED>(a);
                    // SAFETY: the old array is unlinked above; freed (or retired).
                    unsafe {
                        a.free_bytes(
                            core::ptr::NonNull::new(old_vals.cast::<u8>()).expect("value array"),
                            map_immed_val_size(n),
                        );
                    }
                    // SAFETY: slot pos in the newly allocated (n+1)-element value array.
                    return (None, unsafe { new_vals.as_ptr().add(pos) });
                }
                // Overflow immediate capacity -> build linear leaf.
                let mut entries = StackEntries32::new();
                for i in 0..pos {
                    // SAFETY: i < pos <= n <= 7; aux holds n packed keys.
                    let ki = unsafe { read_packed(edge.aux_bytes().as_ptr(), i, kb_usize) };
                    // SAFETY: i < pos <= n; old_vals holds n values.
                    let vi = unsafe { *old_vals.add(i) };
                    entries.push((ki, vi));
                }
                entries.push((k, val));
                for i in pos..n {
                    // SAFETY: i < n <= 7; aux holds n packed keys.
                    let ki = unsafe { read_packed(edge.aux_bytes().as_ptr(), i, kb_usize) };
                    // SAFETY: i < n; old_vals holds n values.
                    let vi = unsafe { *old_vals.add(i) };
                    entries.push((ki, vi));
                }
                cover.begin_if::<OCC, NESTED>(a);
                build_map_leaf(a, edge, kb, entries.as_slice());
                cover.end_if::<OCC, NESTED>(a);
                // SAFETY: old_vals points to the unlinked n*8 byte allocation.
                unsafe {
                    a.free_bytes(
                        core::ptr::NonNull::new(old_vals.cast::<u8>()).expect("value array"),
                        map_immed_val_size(n),
                    );
                }
                // SAFETY: build_map_leaf places value array at the base of the leaf.
                return (None, unsafe { edge.node_ptr().cast::<u64>().add(pos) });
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
                    // level and retry. The rewrite of this slot is the
                    // parent's store.
                    cover.begin_if::<OCC, NESTED>(a);
                    split_skip(a, edge, key, level, pop as u64);
                    cover.end_if::<OCC, NESTED>(a);
                    continue;
                }
                let k = key_low(key, kb);
                let base = edge.node_ptr();
                // SAFETY: map_keys_offset(pop) is within the live map leaf allocation.
                let keys_ptr = unsafe { base.add(leaf::map_keys_offset(pop)) };
                let (hit, pos) = if pop > 0 {
                    // SAFETY: pop > 0 guarantees slot pop - 1 is in-bounds.
                    let last = unsafe { read_packed_fixed(keys_ptr, pop - 1, kb) };
                    if k > last {
                        (false, pop)
                    } else if k == last {
                        (true, pop - 1)
                    } else {
                        // SAFETY: live map leaf per contract (keys behind the values).
                        match unsafe { leaf_locate_fixed(keys_ptr, pop, kb, k) } {
                            Ok(p) => (true, p),
                            Err(p) => (false, p),
                        }
                    }
                } else {
                    (false, 0)
                };
                match if hit { Ok(pos) } else { Err(pos) } {
                    Ok(pos) => {
                        // SAFETY: in-place value swap within the live leaf;
                        // linear leaves carry no version, so the parent's
                        // word brackets the store.
                        unsafe {
                            let slot = base.cast::<u64>().add(pos);
                            let old = *slot;
                            if !KEEP {
                                cover.begin_if::<OCC, NESTED>(a);
                                a.assert_bracketed_by(cover.addr(a));
                                slot.write(val);
                                cover.end_if::<OCC, NESTED>(a);
                            }
                            return (Some(old), slot);
                        }
                    }
                    Err(pos) => {
                        let cap = if kb == 1 { LEAF1_CAP } else { LEAF_CAP };
                        if pop < cap && leaf::cap_class(pop + 1) == leaf::cap_class(pop) {
                            // Fast path: spare class capacity — shift both
                            // areas in place under the parent's word.
                            cover.begin_if::<OCC, NESTED>(a);
                            a.assert_bracketed_by(cover.addr(a));
                            // SAFETY: class capacity spare per the check.
                            unsafe { leaf::map_insert_at(base, kb, pop, pos, k, val) };
                            edge.set_pop0(kb, pop as u64);
                            cover.end_if::<OCC, NESTED>(a);
                            // SAFETY: freshly shifted value area.
                            return (None, unsafe { base.cast::<u64>().add(pos) });
                        }
                        if pop < cap {
                            // Class-crossing grow that stays this leaf: a
                            // direct copy with a gap into a private
                            // allocation; only the slot rewrite is
                            // published. Aux (decode + pop0) is preserved
                            // wholesale — the form and kb are unchanged.
                            let new = a.alloc_bytes(leaf::size_map(kb, pop + 1));
                            // SAFETY: live source leaf of `pop` entries;
                            // fresh destination sized for `pop + 1`;
                            // `pos <= pop` from lower_bound.
                            unsafe {
                                leaf::map_realloc_insert(base, new.as_ptr(), kb, pop, pos, k, val);
                            }
                            let saved_aux = *edge.aux_bytes();
                            cover.begin_if::<OCC, NESTED>(a);
                            *edge = Edge::new_node(new.as_ptr(), t.as_u8());
                            edge.set_aux_bytes(saved_aux);
                            edge.set_pop0(kb, pop as u64);
                            cover.end_if::<OCC, NESTED>(a);
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
                        // the true divergence level. The replacement is built
                        // on a private edge and published with one store.
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
                        let mut tmp = Edge::NULL;
                        if entries.len() <= cap {
                            build_map_leaf(a, &mut tmp, kb, &entries);
                            restore_decode(&mut tmp, kb, level, &saved_aux);
                        } else if kb == 1 {
                            // Level-1 overflow: linear map leaf → bitmap leaf
                            // (any narrow pointer carries over).
                            build_bitmap_leaf_map(a, &mut tmp, &entries);
                            restore_decode(&mut tmp, 1, level, &saved_aux);
                        } else if divergence_level(
                            entries[0].0,
                            entries[entries.len() - 1].0,
                            level,
                        ) == 1
                        {
                            // Narrow-pointer synthesis: all keys share their
                            // digits at levels 2..=kb — one bitmap leaf with
                            // the shared prefix as decode bytes, no chain.
                            let low: Vec<(u64, u64)> =
                                entries.iter().map(|&(k, v)| (key_low(k, 1), v)).collect();
                            let prefix_key = entries[0].0;
                            build_bitmap_leaf_map(a, &mut tmp, &low);
                            write_decode(&mut tmp, 1, level, prefix_key);
                        } else {
                            // Cascade: an empty branch at the divergence level
                            // (a narrow pointer when that sits below the slot;
                            // level-8 slots always branch in place — the root
                            // edge has no room for decode bytes), re-insert.
                            // The private node's own word is the cover for
                            // the re-inserts: nothing can observe it yet.
                            let d =
                                divergence_level(entries[0].0, entries[entries.len() - 1].0, level);
                            let bl = if level <= 7 { d } else { level };
                            let node = a.alloc_node_zeroed::<BranchL3>();
                            // SAFETY: node is freshly allocated zeroed BranchL3 memory.
                            unsafe {
                                (*node.as_ptr()).hdr.level = bl;
                            }
                            tmp = Edge::new_node(node.as_ptr().cast(), EdgeType::BranchL3.as_u8());
                            if bl < level {
                                write_decode(&mut tmp, bl, level, entries[0].0);
                            }
                            // The subtree is private until the store below,
                            // so its re-inserts bracket a scratch word: not
                            // the parent's (it would flicker for nothing)
                            // and not the new node's own (an upgrade inside
                            // the cascade marks that node obsolete, which
                            // needs it even).
                            let mut scratch = 0u32;
                            let private = Cover::Node(&raw mut scratch);
                            for &(k, v) in &entries {
                                // SAFETY: freshly built branch subtree.
                                let prev = unsafe {
                                    map_insert_with_path::<KEEP, OCC, NESTED>(
                                        a, &mut tmp, k, v, level, path, private,
                                    )
                                };
                                debug_assert!(prev.0.is_none());
                            }
                            // pop0 cannot express the transient empty branch;
                            // pin the true population (see mutate::insert).
                            tmp.set_pop0(bl, entries.len() as u64 - 1);
                        }
                        cover.begin_if::<OCC, NESTED>(a);
                        *edge = tmp;
                        cover.end_if::<OCC, NESTED>(a);
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
                        return (None, slot);
                    }
                }
            }

            EdgeTag::Structural(EdgeType::LeafB1) => {
                if level > 1 && !crate::get::decode_matches(edge, key, 1, level) {
                    // Diverges inside the skipped prefix: branch out one
                    // level and retry.
                    let pop = edge.pop0(1) + 1;
                    cover.begin_if::<OCC, NESTED>(a);
                    split_skip(a, edge, key, level, pop);
                    cover.end_if::<OCC, NESTED>(a);
                    continue;
                }
                let d = digit(key, 1);
                // A bitmap leaf carries no version of its own — readers
                // validate its payload against the parent branch's word,
                // which brackets every store below.
                let node = edge.node_ptr().cast::<LeafBitmapL>();
                let sub = (d >> 5) as usize;
                // SAFETY: live LeafBitmapL per contract.
                if let Some(rank) = unsafe { (*node).bitmap.test_and_subexpanse_rank(d) } {
                    // SAFETY: value subarray holds subexpanse_count values.
                    unsafe {
                        let slot = (*node).values[sub].add(rank);
                        let old = *slot;
                        if !KEEP {
                            cover.begin_if::<OCC, NESTED>(a);
                            a.assert_bracketed_by(cover.addr(a));
                            slot.write(val);
                            cover.end_if::<OCC, NESTED>(a);
                        }
                        return (Some(old), slot);
                    }
                }
                // SAFETY: live LeafBitmapL per contract.
                let (rank, old_n) = unsafe {
                    (
                        (*node).bitmap.subexpanse_rank(d) as usize,
                        (*node).bitmap.subexpanse_count(sub) as usize,
                    )
                };
                cover.begin_if::<OCC, NESTED>(a);
                a.assert_bracketed_by(cover.addr(a));
                if old_n > 0 && leaf::cap_class(old_n + 1) == leaf::cap_class(old_n) {
                    // Fast path: spare class capacity — shift in place.
                    // SAFETY: the subarray holds cap_class(old_n) slots.
                    unsafe {
                        let arr = (*node).values[sub];
                        core::ptr::copy(arr.add(rank), arr.add(rank + 1), old_n - rank);
                        arr.add(rank).write(val);
                    }
                } else {
                    let new = a.alloc_bytes(sub_vals_size(old_n + 1)).cast::<u64>();
                    // SAFETY: copy old_n values around the inserted rank; the
                    // empty case touches no old pointer.
                    unsafe {
                        if old_n > 0 {
                            let old = (*node).values[sub];
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
                        (*node).values[sub] = new.as_ptr();
                    }
                }
                // SAFETY: live LeafBitmapL per contract.
                unsafe {
                    (*node).bitmap.set(d);
                }
                let pop0 = edge.pop0(1);
                edge.set_pop0(1, pop0 + 1);
                cover.end_if::<OCC, NESTED>(a);
                // SAFETY: the value now lives at this subarray rank.
                return (None, unsafe { (*node).values[sub].add(rank) });
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
                    cover.begin_if::<OCC, NESTED>(a);
                    split_skip(a, edge, key, level, pop);
                    cover.end_if::<OCC, NESTED>(a);
                    continue;
                }
                let d = digit(key, bl);
                let is_l3 = matches!(t, EdgeType::BranchL3);
                // SAFETY: live branch per contract.
                let (found, num) = unsafe {
                    if is_l3 {
                        let b = &*edge.node_ptr().cast::<BranchL3>();
                        let num = b.hdr.num as usize;
                        let found = if num >= 1 && b.hdr.digits[0] == d {
                            Some(0)
                        } else if num >= 2 && b.hdr.digits[1] == d {
                            Some(1)
                        } else if num >= 3 && b.hdr.digits[2] == d {
                            Some(2)
                        } else {
                            None
                        };
                        (found, num)
                    } else {
                        let b = &*edge.node_ptr().cast::<BranchL7>();
                        (b.hdr.find(d), b.hdr.num as usize)
                    }
                };
                if let Some(slot) = found {
                    // SAFETY: slot within populated count; child well-formed.
                    // The descent is not bracketed: the child frame brackets
                    // its own stores with this node's word (`inner`).
                    let res = unsafe {
                        if is_l3 {
                            let b = edge.node_ptr().cast::<BranchL3>();
                            let inner = Cover::Node(&raw mut (*b).hdr.version);
                            inner.nest_begin::<OCC, NESTED>(a);
                            let r = map_insert_with_path::<KEEP, OCC, NESTED>(
                                a,
                                &mut (*b).edges[slot],
                                key,
                                val,
                                bl - 1,
                                path,
                                inner,
                            );
                            inner.nest_end::<OCC, NESTED>(a);
                            r
                        } else {
                            let b = edge.node_ptr().cast::<BranchL7>();
                            let inner = Cover::Node(&raw mut (*b).hdr.version);
                            inner.nest_begin::<OCC, NESTED>(a);
                            let r = map_insert_with_path::<KEEP, OCC, NESTED>(
                                a,
                                &mut (*b).edges[slot],
                                key,
                                val,
                                bl - 1,
                                path,
                                inner,
                            );
                            inner.nest_end::<OCC, NESTED>(a);
                            r
                        }
                    };
                    if res.0.is_none() {
                        // The population lives in this node's incoming edge,
                        // which the parent's word covers.
                        cover.begin_if::<OCC, NESTED>(a);
                        // SAFETY: edge is a valid live edge.
                        unsafe { bump_pop0(edge, bl, 1) };
                        cover.end_if::<OCC, NESTED>(a);
                    }
                    return res;
                }
                let cap = if is_l3 { BRANCH_L3_CAP } else { BRANCH_L7_CAP };
                if num == cap {
                    // The rebuild rewrites this node's incoming edge: the
                    // parent's word covers it.
                    cover.begin_if::<OCC, NESTED>(a);
                    // SAFETY: upgrade rebuilds the node; subtree stays owned.
                    unsafe {
                        if is_l3 {
                            upgrade_l3_to_l7::<OCC>(a, edge);
                        } else {
                            upgrade_l7_to_b::<OCC>(a, edge);
                        }
                    }
                    cover.end_if::<OCC, NESTED>(a);
                    continue;
                }
                // Open the new slot under this node's own bracket, close it,
                // then let the child frame fill the slot under the same word.
                // SAFETY: live branch; slot arithmetic bounded by capacity.
                let res = unsafe {
                    if is_l3 {
                        let b = edge.node_ptr().cast::<BranchL3>();
                        let inner = Cover::Node(&raw mut (*b).hdr.version);
                        inner.nest_begin::<OCC, NESTED>(a);
                        inner.begin_if::<OCC, NESTED>(a);
                        let slot =
                            linear_insert_slot_l3(&mut (*b).hdr.digits, &mut (*b).edges, num, d);
                        (*b).hdr.num += 1;
                        (*b).hdr.add_presence(d);
                        inner.end_if::<OCC, NESTED>(a);
                        let r = map_insert_with_path::<KEEP, OCC, NESTED>(
                            a,
                            &mut (*b).edges[slot],
                            key,
                            val,
                            bl - 1,
                            path,
                            inner,
                        );
                        inner.nest_end::<OCC, NESTED>(a);
                        r
                    } else {
                        let b = edge.node_ptr().cast::<BranchL7>();
                        let inner = Cover::Node(&raw mut (*b).hdr.version);
                        inner.nest_begin::<OCC, NESTED>(a);
                        inner.begin_if::<OCC, NESTED>(a);
                        let slot =
                            linear_insert_slot(&mut (*b).hdr.digits, &mut (*b).edges, num, d);
                        (*b).hdr.num += 1;
                        (*b).hdr.add_presence(d);
                        inner.end_if::<OCC, NESTED>(a);
                        let r = map_insert_with_path::<KEEP, OCC, NESTED>(
                            a,
                            &mut (*b).edges[slot],
                            key,
                            val,
                            bl - 1,
                            path,
                            inner,
                        );
                        inner.nest_end::<OCC, NESTED>(a);
                        r
                    }
                };
                debug_assert!(res.0.is_none());
                cover.begin_if::<OCC, NESTED>(a);
                // SAFETY: edge is a valid live edge.
                unsafe { bump_pop0(edge, bl, 1) };
                cover.end_if::<OCC, NESTED>(a);
                return (None, res.1);
            }

            EdgeTag::Structural(EdgeType::BranchB) => {
                debug_assert!(level >= 2);
                // SAFETY: live branch per contract.
                let bl = unsafe { branch_form_level(edge, EdgeType::BranchB, level) };
                if bl < level && !crate::get::decode_matches(edge, key, bl, level) {
                    let pop = edge.pop0(bl) + 1;
                    cover.begin_if::<OCC, NESTED>(a);
                    split_skip(a, edge, key, level, pop);
                    cover.end_if::<OCC, NESTED>(a);
                    continue;
                }
                let slot_level = level;
                let d = digit(key, bl);
                let b = edge.node_ptr().cast::<BranchB>();
                // SAFETY: the node is live for the frame.
                let inner = Cover::Node(unsafe { &raw mut (*b).version });
                // SAFETY: live BranchB per contract.
                if let Some(slot) = unsafe { (*b).bitmap.test_and_subexpanse_rank(d) } {
                    inner.nest_begin::<OCC, NESTED>(a);
                    // SAFETY: bitmap/subarray consistency invariant. The
                    // descent is not bracketed in the brief mode; the child
                    // frame takes `inner` for its stores.
                    let res = unsafe {
                        let sub = (*b).subarrays[(d >> 5) as usize];
                        map_insert_with_path::<KEEP, OCC, NESTED>(
                            a,
                            &mut *sub.add(slot),
                            key,
                            val,
                            bl - 1,
                            path,
                            inner,
                        )
                    };
                    inner.nest_end::<OCC, NESTED>(a);
                    if res.0.is_none() {
                        cover.begin_if::<OCC, NESTED>(a);
                        // SAFETY: edge is a valid live edge.
                        unsafe { bump_pop0(edge, bl, 1) };
                        cover.end_if::<OCC, NESTED>(a);
                    }
                    return res;
                }
                // SAFETY: live BranchB per contract.
                if unsafe { (*b).bitmap.count() } as usize + 1 > BRANCHB_UP {
                    if bl < slot_level {
                        // BranchU cannot skip: materialize one chain level.
                        let pop = edge.pop0(bl) + 1;
                        cover.begin_if::<OCC, NESTED>(a);
                        wrap_skip_level(a, edge, bl + 1, slot_level, pop);
                        cover.end_if::<OCC, NESTED>(a);
                        level = slot_level;
                        continue;
                    }
                    cover.begin_if::<OCC, NESTED>(a);
                    // SAFETY: upgrade rebuilds the node; subtree stays owned.
                    unsafe {
                        upgrade_b_to_u::<OCC>(a, edge);
                    }
                    cover.end_if::<OCC, NESTED>(a);
                    level = slot_level;
                    continue;
                }
                let sub = (d >> 5) as usize;
                // SAFETY: live BranchB per contract.
                let (old_n, rank) = unsafe {
                    (
                        (*b).pop_counts[sub] as usize,
                        (*b).bitmap.subexpanse_rank(d) as usize,
                    )
                };
                // Open the new subarray slot under this node's own word,
                // close it, then the child frame fills the slot under it.
                inner.nest_begin::<OCC, NESTED>(a);
                inner.begin_if::<OCC, NESTED>(a);
                if old_n > 0 && leaf::cap_class(old_n + 1) == leaf::cap_class(old_n) {
                    // Fast path: spare class capacity — shift in place.
                    // SAFETY: the subarray holds cap_class(old_n) slots.
                    unsafe {
                        let arr = (*b).subarrays[sub];
                        core::ptr::copy(arr.add(rank), arr.add(rank + 1), old_n - rank);
                        arr.add(rank).write(Edge::NULL);
                    }
                } else {
                    let new = a.alloc_bytes(sub_edges_size(old_n + 1)).cast::<Edge>();
                    // SAFETY: copy old_n live edges around the inserted slot;
                    // the empty case touches no old pointer.
                    unsafe {
                        if old_n > 0 {
                            let old = (*b).subarrays[sub];
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
                        (*b).subarrays[sub] = new.as_ptr();
                    }
                }
                // SAFETY: live BranchB per contract.
                unsafe {
                    (*b).pop_counts[sub] = (old_n + 1) as u16;
                    (*b).bitmap.set(d);
                }
                inner.end_if::<OCC, NESTED>(a);
                inner.nest_end::<OCC, NESTED>(a);
                inner.nest_begin::<OCC, NESTED>(a);
                // SAFETY: fresh null child slot within the subarray.
                let res = unsafe {
                    map_insert_with_path::<KEEP, OCC, NESTED>(
                        a,
                        &mut *(*b).subarrays[sub].add(rank),
                        key,
                        val,
                        bl - 1,
                        path,
                        inner,
                    )
                };
                inner.nest_end::<OCC, NESTED>(a);
                debug_assert!(res.0.is_none());
                cover.begin_if::<OCC, NESTED>(a);
                // SAFETY: edge is a valid live edge.
                unsafe { bump_pop0(edge, bl, 1) };
                cover.end_if::<OCC, NESTED>(a);
                return (None, res.1);
            }

            EdgeTag::Structural(EdgeType::BranchU) => {
                debug_assert!(level >= 2);
                let d = digit(key, level);
                let b = edge.node_ptr().cast::<BranchU>();
                // SAFETY: the node is live for the frame.
                let inner = Cover::Node(unsafe { &raw mut (*b).version });
                inner.nest_begin::<OCC, NESTED>(a);
                // SAFETY: live BranchU per contract; child subtree well-formed
                // (or null). The descent is not bracketed; the child frame
                // takes `inner` for its stores into this node's slot.
                let res = unsafe {
                    map_insert_with_path::<KEEP, OCC, NESTED>(
                        a,
                        &mut (*b).edges[d as usize],
                        key,
                        val,
                        level - 1,
                        path,
                        inner,
                    )
                };
                inner.nest_end::<OCC, NESTED>(a);
                if res.0.is_none() {
                    cover.begin_if::<OCC, NESTED>(a);
                    // SAFETY: edge is a valid live edge.
                    unsafe { bump_pop0(edge, level, 1) };
                    cover.end_if::<OCC, NESTED>(a);
                }
                return res;
            }
        }
    }
}

/// `cover` is the version word of the node whose slot `edge` is (the tree
/// word at the top): the same covering function as
/// [`map_insert_with_path_occ`] — every store this frame makes to `*edge` or
/// to the payload it points at is bracketed by `cover`, and a branch child's
/// frame is entered unbracketed with this node's own word.
///
/// # Safety
///
/// Same contract as [`map_insert`].
pub(crate) unsafe fn map_remove<const OCC: bool, const NESTED: bool>(
    a: &NodeAlloc,
    edge: &mut Edge,
    key: Key,
    level: u8,
    cover: Cover,
) -> Option<u64> {
    debug_assert!((1..=8).contains(&level));
    let tag = edge.tag().expect("valid edge tag");
    match tag {
        EdgeTag::Structural(EdgeType::Null) => None,

        EdgeTag::Immed(im) => {
            let kb = im.key_bytes();
            debug_assert_eq!(kb, level);
            let k = key_low(key, kb);
            let n = im.key_count() as usize;
            if n == 1 {
                // SAFETY: single-key map immediate holds 1 key in aux bytes.
                let existing_k = unsafe { read_packed(edge.aux_bytes().as_ptr(), 0, kb as usize) };
                if existing_k == k {
                    let old = u64::from_le_bytes(edge.imm_bytes());
                    cover.begin_if::<OCC, NESTED>(a);
                    *edge = Edge::NULL;
                    cover.end_if::<OCC, NESTED>(a);
                    return Some(old);
                }
                return None;
            }
            // SAFETY: aux bytes hold n packed keys of kb bytes.
            let pos = match unsafe { leaf::locate(edge.aux_bytes().as_ptr(), n, kb, k) } {
                Ok(p) => p,
                Err(_) => return None,
            };
            let vals = edge.node_ptr().cast::<u64>();
            // SAFETY: pos < n is within the live value array.
            let old = unsafe { *vals.add(pos) };
            if n == 2 {
                let remain_slot = 1 - pos;
                // SAFETY: remain_slot < 2 is within the live value array.
                let remain_val = unsafe { *vals.add(remain_slot) };
                let mut new_aux = [0u8; 7];
                let kb_usize = kb as usize;
                new_aux[..kb_usize].copy_from_slice(
                    &edge.aux_bytes()[remain_slot * kb_usize..(remain_slot + 1) * kb_usize],
                );
                let new_im = ImmedType::new(kb, 1).expect("immediate capacity");
                cover.begin_if::<OCC, NESTED>(a);
                *edge = Edge::NULL;
                edge.set_imm_bytes(remain_val.to_le_bytes());
                edge.set_aux_bytes(new_aux);
                edge.set_tag(new_im.as_u8());
                cover.end_if::<OCC, NESTED>(a);
                // SAFETY: the old 2-entry value array is unlinked above.
                unsafe {
                    a.free_bytes(
                        core::ptr::NonNull::new(vals.cast::<u8>()).expect("value array"),
                        map_immed_val_size(2),
                    );
                }
                return Some(old);
            }
            let kb_usize = kb as usize;
            if leaf::cap_class(n - 1) == leaf::cap_class(n) {
                // Spare class capacity: shift values and keys in place;
                // readers validate the value array against the parent's word.
                let mut new_aux = *edge.aux_bytes();
                new_aux.copy_within((pos + 1) * kb_usize..n * kb_usize, pos * kb_usize);
                new_aux[((n - 1) * kb_usize)..].fill(0);
                let new_im = ImmedType::new(kb, (n - 1) as u8).expect("immediate capacity");
                cover.begin_if::<OCC, NESTED>(a);
                a.assert_bracketed_by(cover.addr(a));
                // SAFETY: shift surviving values left within the allocated array.
                unsafe {
                    if pos + 1 < n {
                        core::ptr::copy(vals.add(pos + 1), vals.add(pos), n - 1 - pos);
                    }
                }
                edge.set_aux_bytes(new_aux);
                edge.set_tag(new_im.as_u8());
                cover.end_if::<OCC, NESTED>(a);
                return Some(old);
            }
            // n > 2: copy values around pos into a private array and shift
            // keys in aux; publish with the slot rewrite.
            let new_vals = a.alloc_bytes(map_immed_val_size(n - 1)).cast::<u64>();
            // SAFETY: copy n-1 surviving values.
            unsafe {
                if pos > 0 {
                    core::ptr::copy_nonoverlapping(vals, new_vals.as_ptr(), pos);
                }
                if pos + 1 < n {
                    core::ptr::copy_nonoverlapping(
                        vals.add(pos + 1),
                        new_vals.as_ptr().add(pos),
                        n - 1 - pos,
                    );
                }
            }
            let mut new_aux = *edge.aux_bytes();
            new_aux.copy_within((pos + 1) * kb_usize..n * kb_usize, pos * kb_usize);
            new_aux[((n - 1) * kb_usize)..].fill(0);
            let new_im = ImmedType::new(kb, (n - 1) as u8).expect("immediate capacity");
            cover.begin_if::<OCC, NESTED>(a);
            *edge = Edge::new_node(new_vals.as_ptr().cast(), 0);
            edge.set_aux_bytes(new_aux);
            edge.set_tag(new_im.as_u8());
            cover.end_if::<OCC, NESTED>(a);
            // SAFETY: the old array is unlinked above.
            unsafe {
                a.free_bytes(
                    core::ptr::NonNull::new(vals.cast::<u8>()).expect("value array"),
                    map_immed_val_size(n),
                );
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
            let base = edge.node_ptr();
            // SAFETY: live map leaf per contract.
            let keys_ptr = unsafe { base.add(leaf::map_keys_offset(pop)) };
            // SAFETY: keys_ptr points to `pop * kb` valid bytes.
            let pos = match unsafe { leaf::locate(keys_ptr, pop, kb, k) } {
                Ok(pos) => pos,
                Err(_) => return None,
            };
            // SAFETY: pos < pop is within the live map leaf values.
            let old = unsafe { *base.cast::<u64>().add(pos) };
            if pop > map_immed_max(level) && leaf::cap_class(pop - 1) == leaf::cap_class(pop) {
                // Fast path: stays a leaf in the same class. Linear leaves
                // carry no version; the parent's word brackets the shift.
                cover.begin_if::<OCC, NESTED>(a);
                a.assert_bracketed_by(cover.addr(a));
                // SAFETY: pos < pop; same-class allocation.
                unsafe {
                    leaf::map_remove_at(base, kb, pop, pos);
                }
                edge.set_pop0(kb, pop as u64 - 2);
                cover.end_if::<OCC, NESTED>(a);
                return Some(old);
            }
            if pop >= 2 && pop > map_immed_max(level) {
                // Class-crossing shrink that stays this leaf (the
                // hysteresis band keeps it one below `map_immed_max`):
                // direct copy with the slot elided into a private
                // allocation; only the slot rewrite is published.
                let new = a.alloc_bytes(leaf::size_map(kb, pop - 1));
                // SAFETY: live source leaf of `pop >= 2` entries; fresh
                // destination sized for `pop - 1`.
                unsafe { leaf::map_realloc_remove(base, new.as_ptr(), kb, pop, pos) };
                let saved_aux = *edge.aux_bytes();
                cover.begin_if::<OCC, NESTED>(a);
                *edge = Edge::new_node(new.as_ptr(), t.as_u8());
                edge.set_aux_bytes(saved_aux);
                edge.set_pop0(kb, pop as u64 - 2);
                cover.end_if::<OCC, NESTED>(a);
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
            // Slow path (conversion to immediate or null).
            let old_ptr = edge.node_ptr();
            let old_size = leaf::size_map(kb, pop);
            if pop == 1 {
                cover.begin_if::<OCC, NESTED>(a);
                *edge = Edge::NULL;
                cover.end_if::<OCC, NESTED>(a);
                // SAFETY: old leaf allocation no longer referenced.
                unsafe {
                    a.free_bytes(core::ptr::NonNull::new(old_ptr).expect("leaf"), old_size);
                }
                return Some(old);
            }
            let rem_pop = pop - 1;
            let dv = if kb < level {
                decode_value(edge, kb, level)
            } else {
                0
            };
            let mut entries = [(0u64, 0u64); 8];
            let mut idx = 0;
            for slot in 0..pop {
                if slot == pos {
                    continue;
                }
                // SAFETY: map leaf = pop values then pop packed keys; slot < pop.
                let low_k =
                    unsafe { read_packed(base.add(leaf::map_keys_offset(pop)), slot, kb as usize) };
                let k = (dv << (8 * u32::from(kb))) | low_k;
                // SAFETY: slot < pop is within the live map leaf values.
                let v = unsafe { *base.cast::<u64>().add(slot) };
                entries[idx] = (k, v);
                idx += 1;
            }
            cover.begin_if::<OCC, NESTED>(a);
            write_map_immed(a, edge, level, &entries[..rem_pop]);
            cover.end_if::<OCC, NESTED>(a);
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
            let node = edge.node_ptr().cast::<LeafBitmapL>();
            let sub = (d >> 5) as usize;
            // SAFETY: live LeafBitmapL per contract.
            let rank = unsafe { (*node).bitmap.test_and_subexpanse_rank(d) }?;
            // SAFETY: live LeafBitmapL per contract; value subarray holds
            // old_n values.
            let (old_n, old) = unsafe {
                let old_n = (*node).bitmap.subexpanse_count(sub) as usize;
                (old_n, *(*node).values[sub].add(rank))
            };
            // A bitmap leaf carries no version of its own — readers
            // validate its payload against the parent branch's word,
            // which brackets every store below.
            cover.begin_if::<OCC, NESTED>(a);
            a.assert_bracketed_by(cover.addr(a));
            // SAFETY: shrink of the packed value subarray — in place
            // when the class holds, reallocating across boundaries.
            unsafe {
                let old_arr = (*node).values[sub];
                if old_n == 1 {
                    (*node).values[sub] = core::ptr::null_mut();
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
                    (*node).values[sub] = new.as_ptr();
                    a.free_bytes(
                        core::ptr::NonNull::new(old_arr.cast()).expect("values"),
                        sub_vals_size(old_n),
                    );
                }
                (*node).bitmap.clear(d);
            }
            let pop = edge.pop0(1) as usize; // old pop - 1
            // Hysteresis: back to a linear map leaf when pop drops below the floor.
            if pop < LEAFB1_DOWN {
                let saved_aux = *edge.aux_bytes();
                let dv = if level > 1 {
                    decode_value(edge, 1, level)
                } else {
                    0
                };
                // SAFETY: live node; entries re-read for the rebuild.
                let entries = unsafe {
                    let mut out = StackEntries32::new();
                    let bitmap = &(*node).bitmap;
                    let mut dig = bitmap.next_set(0);
                    while let Some(g) = dig {
                        let s = (g >> 5) as usize;
                        let r = bitmap.subexpanse_rank(g) as usize;
                        out.push((u64::from(g), *(*node).values[s].add(r)));
                        dig = if g == 255 {
                            None
                        } else {
                            bitmap.next_set(g + 1)
                        };
                    }
                    out
                };
                // SAFETY: free subarrays + node after extraction.
                unsafe {
                    for sub in 0..8 {
                        let n = (*node).bitmap.subexpanse_count(sub) as usize;
                        if n > 0 {
                            a.free_bytes(
                                core::ptr::NonNull::new((*node).values[sub].cast())
                                    .expect("values"),
                                sub_vals_size(n),
                            );
                        }
                    }
                    a.free_node(core::ptr::NonNull::new(node).unwrap());
                }
                if entries.len < map_immed_max(level) {
                    let mut full = StackEntries32::new();
                    for &(low, v) in entries.as_slice() {
                        full.push(((dv << 8) | low, v));
                    }
                    write_map_immed(a, edge, level, full.as_slice());
                } else {
                    build_map_leaf(a, edge, 1, entries.as_slice());
                    restore_decode(edge, 1, level, &saved_aux);
                }
            } else {
                edge.set_pop0(1, pop as u64 - 1);
            }
            cover.end_if::<OCC, NESTED>(a);
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
            // SAFETY: live branch per contract. The descent is not
            // bracketed: the child frame brackets its own stores with this
            // node's word (`inner`); the slot removal below is this frame's
            // own store into the node and takes the same word.
            let (old_opt, child_null) = unsafe {
                if is_l3 {
                    let b = edge.node_ptr().cast::<BranchL3>();
                    let slot = (*b).hdr.find(d)?;
                    let inner = Cover::Node(&raw mut (*b).hdr.version);
                    inner.nest_begin::<OCC, NESTED>(a);
                    let r = map_remove::<OCC, NESTED>(a, &mut (*b).edges[slot], key, bl - 1, inner);
                    inner.nest_end::<OCC, NESTED>(a);
                    let child_null = r.is_some() && (*b).edges[slot].is_null();
                    if child_null {
                        inner.nest_begin::<OCC, NESTED>(a);
                        inner.begin_if::<OCC, NESTED>(a);
                        linear_remove_slot(
                            &mut (*b).hdr.digits,
                            &mut (*b).edges,
                            (*b).hdr.num as usize,
                            slot,
                        );
                        (*b).hdr.num -= 1;
                        (*b).hdr.refresh_presence();
                        inner.end_if::<OCC, NESTED>(a);
                        inner.nest_end::<OCC, NESTED>(a);
                    }
                    (r, child_null)
                } else {
                    let b = edge.node_ptr().cast::<BranchL7>();
                    let slot = (*b).hdr.find(d)?;
                    let inner = Cover::Node(&raw mut (*b).hdr.version);
                    inner.nest_begin::<OCC, NESTED>(a);
                    let r = map_remove::<OCC, NESTED>(a, &mut (*b).edges[slot], key, bl - 1, inner);
                    inner.nest_end::<OCC, NESTED>(a);
                    let child_null = r.is_some() && (*b).edges[slot].is_null();
                    if child_null {
                        inner.nest_begin::<OCC, NESTED>(a);
                        inner.begin_if::<OCC, NESTED>(a);
                        linear_remove_slot(
                            &mut (*b).hdr.digits,
                            &mut (*b).edges,
                            (*b).hdr.num as usize,
                            slot,
                        );
                        (*b).hdr.num -= 1;
                        (*b).hdr.refresh_presence();
                        inner.end_if::<OCC, NESTED>(a);
                        inner.nest_end::<OCC, NESTED>(a);
                    }
                    (r, child_null)
                }
            };
            let old = old_opt?;
            if child_null {
                // SAFETY: node rebuilds below keep the subtree owned.
                unsafe {
                    let num = if is_l3 {
                        (*edge.node_ptr().cast::<BranchL3>()).hdr.num as usize
                    } else {
                        (*edge.node_ptr().cast::<BranchL7>()).hdr.num as usize
                    };
                    // This node's incoming edge is the parent's slot: its
                    // word covers the stores below. The node's own bracket
                    // is closed, as `free_branch_node`'s obsolete mark needs.
                    cover.begin_if::<OCC, NESTED>(a);
                    if num == 0 {
                        free_branch_node::<OCC>(a, edge, is_l3);
                        *edge = Edge::NULL;
                        cover.end_if::<OCC, NESTED>(a);
                        return Some(old);
                    }
                    bump_pop0_dispatch::<OCC>(edge, bl, -1);
                    if !is_l3 && num < BRANCH_L3_CAP {
                        downgrade_l7_to_l3::<OCC>(a, edge);
                    }
                    cover.end_if::<OCC, NESTED>(a);
                }
            } else {
                cover.begin_if::<OCC, NESTED>(a);
                // SAFETY: edge is a valid live edge.
                unsafe { bump_pop0_dispatch::<OCC>(edge, bl, -1) };
                cover.end_if::<OCC, NESTED>(a);
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
            let b = edge.node_ptr().cast::<BranchB>();
            // SAFETY: live BranchB per contract.
            let rank = unsafe { (*b).bitmap.test_and_subexpanse_rank(d) }?;
            let sub = (d >> 5) as usize;
            // SAFETY: the node is live for the frame.
            let inner = Cover::Node(unsafe { &raw mut (*b).version });
            inner.nest_begin::<OCC, NESTED>(a);
            // SAFETY: bitmap/subarray consistency invariant. The descent is
            // not bracketed; the child frame takes `inner` for its stores.
            let old = match unsafe {
                map_remove::<OCC, NESTED>(
                    a,
                    &mut *(*b).subarrays[sub].add(rank),
                    key,
                    bl - 1,
                    inner,
                )
            } {
                Some(v) => v,
                None => {
                    inner.nest_end::<OCC, NESTED>(a);
                    return None;
                }
            };
            inner.nest_end::<OCC, NESTED>(a);
            // SAFETY: child slot checked/live per invariant.
            let child_null = unsafe { (*(*b).subarrays[sub].add(rank)).is_null() };
            if child_null {
                // SAFETY: shrink of the packed subarray — in place when
                // the class holds, reallocating across class boundaries.
                // A reader may be inside the subarray: bracketed by this
                // node's word.
                let digits = unsafe {
                    let old_n = (*b).pop_counts[sub] as usize;
                    inner.nest_begin::<OCC, NESTED>(a);
                    inner.begin_if::<OCC, NESTED>(a);
                    let old_arr = (*b).subarrays[sub];
                    if old_n == 1 {
                        (*b).subarrays[sub] = core::ptr::null_mut();
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
                        (*b).subarrays[sub] = new.as_ptr();
                        a.free_bytes(
                            core::ptr::NonNull::new(old_arr.cast()).unwrap(),
                            sub_edges_size(old_n),
                        );
                    }
                    (*b).pop_counts[sub] = (old_n - 1) as u16;
                    (*b).bitmap.clear(d);
                    inner.end_if::<OCC, NESTED>(a);
                    inner.nest_end::<OCC, NESTED>(a);
                    (*b).bitmap.count() as usize
                };
                cover.begin_if::<OCC, NESTED>(a);
                if digits == 0 {
                    // SAFETY: empty node no longer referenced; marked
                    // obsolete first (its own bracket is closed).
                    unsafe {
                        crate::occ::version_obsolete_if::<OCC>(&raw mut (*b).version);
                        a.free_node(core::ptr::NonNull::new(b).unwrap());
                    }
                    *edge = Edge::NULL;
                    cover.end_if::<OCC, NESTED>(a);
                    return Some(old);
                }
                // SAFETY: edge is a valid live edge.
                unsafe { bump_pop0_dispatch::<OCC>(edge, bl, -1) };
                if digits < BRANCH_L7_CAP {
                    // SAFETY: rebuild keeps the subtree owned.
                    unsafe { downgrade_b_to_l7::<OCC>(a, edge) };
                }
                cover.end_if::<OCC, NESTED>(a);
            } else {
                cover.begin_if::<OCC, NESTED>(a);
                // SAFETY: edge is a valid live edge.
                unsafe { bump_pop0_dispatch::<OCC>(edge, bl, -1) };
                cover.end_if::<OCC, NESTED>(a);
            }
            Some(old)
        }

        EdgeTag::Structural(EdgeType::BranchU) => {
            let d = digit(key, level);
            let b = edge.node_ptr().cast::<BranchU>();
            // SAFETY: the node is live for the frame.
            let inner = Cover::Node(unsafe { &raw mut (*b).version });
            inner.nest_begin::<OCC, NESTED>(a);
            // SAFETY: live BranchU per contract; child subtree well-formed
            // (or null). The descent is not bracketed; the child frame
            // takes `inner` for its stores into this node's slot.
            let old = match unsafe {
                map_remove::<OCC, NESTED>(a, &mut (*b).edges[d as usize], key, level - 1, inner)
            } {
                Some(v) => v,
                None => {
                    inner.nest_end::<OCC, NESTED>(a);
                    return None;
                }
            };
            inner.nest_end::<OCC, NESTED>(a);
            // SAFETY: live BranchU per contract.
            let child_is_null = unsafe { (*b).edges[d as usize].is_null() };
            if child_is_null {
                // SAFETY: live BranchU per contract.
                let digits = unsafe { (*b).edges.iter().filter(|e| !e.is_null()).count() };
                cover.begin_if::<OCC, NESTED>(a);
                if digits == 0 {
                    // SAFETY: empty node no longer referenced; marked
                    // obsolete first (its own bracket is closed).
                    unsafe {
                        crate::occ::version_obsolete_if::<OCC>(&raw mut (*b).version);
                        a.free_node(core::ptr::NonNull::new(b).unwrap());
                    }
                    *edge = Edge::NULL;
                    cover.end_if::<OCC, NESTED>(a);
                    return Some(old);
                }
                // SAFETY: edge is a valid live edge.
                unsafe { bump_pop0_dispatch::<OCC>(edge, level, -1) };
                if digits < BRANCHB_UP {
                    // SAFETY: rebuild keeps the subtree owned.
                    unsafe { downgrade_u_to_b::<OCC>(a, edge, level) };
                }
                cover.end_if::<OCC, NESTED>(a);
            } else {
                cover.begin_if::<OCC, NESTED>(a);
                // SAFETY: edge is a valid live edge.
                unsafe { bump_pop0_dispatch::<OCC>(edge, level, -1) };
                cover.end_if::<OCC, NESTED>(a);
            }
            Some(old)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::ptr::NonNull;
    use std::sync::Arc;

    struct TestAllocGuard<'a> {
        alloc: &'a NodeAlloc,
        ptr: NonNull<u8>,
        size: usize,
    }

    impl Drop for TestAllocGuard<'_> {
        fn drop(&mut self) {
            // SAFETY: ptr was allocated with size on alloc and is freed once on drop/unwind.
            unsafe { self.alloc.free_bytes(self.ptr, self.size) };
        }
    }

    /// Negative control (§2.3, #479): exercises `a.assert_bracketed()` at
    /// `map_insert_with_path_flat` (case 0x05..=0x0B, Leaf1..Leaf7 in-place shift).
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "node interior mutated outside any version bracket")]
    fn negative_control_map_flat_leaf1_inplace_panics_unbracketed() {
        let alloc = NodeAlloc::new();
        alloc.defer_to(Arc::new(crate::occ::Collector::new()));

        let size = leaf::size_map(1, 3);
        let ptr = alloc.alloc_bytes(size);
        let _guard = TestAllocGuard {
            alloc: &alloc,
            ptr,
            size,
        };

        // SAFETY: freshly allocated map leaf has capacity for 3 entries.
        unsafe {
            let vals = ptr.as_ptr().cast::<u64>();
            vals.add(0).write(10);
            vals.add(1).write(20);
            vals.add(2).write(30);
            let keys = ptr.as_ptr().add(leaf::map_keys_offset(3));
            *keys.add(0) = 0x02;
            *keys.add(1) = 0x05;
            *keys.add(2) = 0x09;
        }
        let mut edge = Edge::new_node(ptr.as_ptr(), EdgeType::Leaf1.as_u8());
        edge.set_pop0(1, 2); // pop0 = 2 => pop = 3

        let mut path = InsertPathMap::empty();
        // SAFETY: edge is a valid Leaf1 map node.
        unsafe { map_insert_with_path_flat::<false>(&alloc, &mut edge, 0x07, 25, 1, &mut path) };
    }

    /// Positive companion: covered map Leaf1 in-place mutation succeeds quietly.
    #[test]
    #[cfg(debug_assertions)]
    fn positive_control_map_flat_leaf1_inplace_quiet_when_covered() {
        let alloc = NodeAlloc::new();
        alloc.defer_to(Arc::new(crate::occ::Collector::new()));

        let size = leaf::size_map(1, 3);
        let ptr = alloc.alloc_bytes(size);
        let _guard = TestAllocGuard {
            alloc: &alloc,
            ptr,
            size,
        };

        // SAFETY: freshly allocated map leaf has capacity for 3 entries.
        unsafe {
            let vals = ptr.as_ptr().cast::<u64>();
            vals.add(0).write(10);
            vals.add(1).write(20);
            vals.add(2).write(30);
            let keys = ptr.as_ptr().add(leaf::map_keys_offset(3));
            *keys.add(0) = 0x02;
            *keys.add(1) = 0x05;
            *keys.add(2) = 0x09;
        }
        let mut edge = Edge::new_node(ptr.as_ptr(), EdgeType::Leaf1.as_u8());
        edge.set_pop0(1, 2); // pop0 = 2 => pop = 3

        let mut path = InsertPathMap::empty();
        alloc.bracket_enter_any();
        // SAFETY: edge is a valid Leaf1 map node.
        let (old, _slot) = unsafe {
            map_insert_with_path_flat::<false>(&alloc, &mut edge, 0x07, 25, 1, &mut path)
        };
        alloc.bracket_leave_any();
        assert_eq!(old, None);
    }

    /// Negative control (§2.3, #479): exercises `a.assert_bracketed()` at
    /// `map_insert_with_path_flat` (map immediate in-place shift).
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "node interior mutated outside any version bracket")]
    fn negative_control_map_flat_immed_inplace_panics_unbracketed() {
        let alloc = NodeAlloc::new();
        alloc.defer_to(Arc::new(crate::occ::Collector::new()));

        let size = map_immed_val_size(3);
        let val_ptr = alloc.alloc_bytes(size).cast::<u64>();
        let _guard = TestAllocGuard {
            alloc: &alloc,
            ptr: val_ptr.cast(),
            size,
        };

        // SAFETY: freshly allocated value array holds 3 entries.
        unsafe {
            val_ptr.as_ptr().add(0).write(10);
            val_ptr.as_ptr().add(1).write(20);
            val_ptr.as_ptr().add(2).write(30);
        }
        let im = ImmedType::new(1, 3).expect("immed");
        let mut edge = Edge::new_node(val_ptr.as_ptr().cast(), im.as_u8());
        let mut aux = [0u8; 7];
        aux[0] = 0x02;
        aux[1] = 0x05;
        aux[2] = 0x09;
        edge.set_aux_bytes(aux);

        let mut path = InsertPathMap::empty();
        // SAFETY: edge is a valid immediate node.
        unsafe { map_insert_with_path_flat::<false>(&alloc, &mut edge, 0x07, 25, 1, &mut path) };
    }

    /// Positive companion: covered map flat immediate in-place shift succeeds quietly.
    #[test]
    #[cfg(debug_assertions)]
    fn positive_control_map_flat_immed_inplace_quiet_when_covered() {
        let alloc = NodeAlloc::new();
        alloc.defer_to(Arc::new(crate::occ::Collector::new()));

        let size = map_immed_val_size(3);
        let val_ptr = alloc.alloc_bytes(size).cast::<u64>();
        let _guard = TestAllocGuard {
            alloc: &alloc,
            ptr: val_ptr.cast(),
            size,
        };

        // SAFETY: freshly allocated value array holds 3 entries.
        unsafe {
            val_ptr.as_ptr().add(0).write(10);
            val_ptr.as_ptr().add(1).write(20);
            val_ptr.as_ptr().add(2).write(30);
        }
        let im = ImmedType::new(1, 3).expect("immed");
        let mut edge = Edge::new_node(val_ptr.as_ptr().cast(), im.as_u8());
        let mut aux = [0u8; 7];
        aux[0] = 0x02;
        aux[1] = 0x05;
        aux[2] = 0x09;
        edge.set_aux_bytes(aux);

        let mut path = InsertPathMap::empty();
        alloc.bracket_enter_any();
        // SAFETY: edge is a valid immediate node.
        let (old, _slot) = unsafe {
            map_insert_with_path_flat::<false>(&alloc, &mut edge, 0x07, 25, 1, &mut path)
        };
        alloc.bracket_leave_any();
        assert_eq!(old, None);
    }

    /// The shared-map engine brackets its own stores with the cover it is
    /// handed (#568 PR 3): an in-place immediate shift with no bracket open
    /// around the call leaves the word it was given bumped by exactly one
    /// begin/end pair, even, and the debug bracket stack empty.
    #[test]
    fn map_occ_immed_inplace_brackets_its_cover() {
        let alloc = NodeAlloc::new();
        alloc.defer_to(Arc::new(crate::occ::Collector::new()));

        let size = map_immed_val_size(3);
        let val_ptr = alloc.alloc_bytes(size).cast::<u64>();
        let _guard = TestAllocGuard {
            alloc: &alloc,
            ptr: val_ptr.cast(),
            size,
        };

        // SAFETY: freshly allocated value array holds 3 entries.
        unsafe {
            val_ptr.as_ptr().add(0).write(10);
            val_ptr.as_ptr().add(1).write(20);
            val_ptr.as_ptr().add(2).write(30);
        }
        let im = ImmedType::new(1, 3).expect("immed");
        let mut edge = Edge::new_node(val_ptr.as_ptr().cast(), im.as_u8());
        let mut aux = [0u8; 7];
        aux[0] = 0x02;
        aux[1] = 0x05;
        aux[2] = 0x09;
        edge.set_aux_bytes(aux);

        let mut word: u32 = 0;
        let mut path = InsertPathMap::empty();
        // SAFETY: edge is a valid immediate node; `word` outlives the call.
        let (old, _slot) = unsafe {
            map_insert_with_path_occ::<false, true, false>(
                &alloc,
                &mut edge,
                0x07,
                25,
                1,
                &mut path,
                Cover::Node(&raw mut word),
            )
        };
        assert_eq!(old, None);
        assert_eq!(word, 2, "one begin/end pair on the cover word");
        #[cfg(debug_assertions)]
        assert!(
            crate::alloc::bracket_stack::open().is_empty(),
            "no bracket left open"
        );
        // Under the immediate the value slot moved: the shift is visible.
        // SAFETY: the array still holds 4 values after the in-place shift.
        let vals: Vec<u64> = (0..4)
            .map(|i| unsafe { *val_ptr.as_ptr().add(i) })
            .collect();
        assert_eq!(vals, vec![10, 20, 25, 30]);
    }

    /// Same instrument on a linear map leaf's in-place shift (`Leaf1`,
    /// same capacity class): the leaf carries no version, so the store is
    /// bracketed by the parent's word the frame was handed.
    #[test]
    fn map_occ_leaf1_inplace_brackets_its_cover() {
        let alloc = NodeAlloc::new();
        alloc.defer_to(Arc::new(crate::occ::Collector::new()));

        let size = leaf::size_map(1, 3);
        let ptr = alloc.alloc_bytes(size);
        let _guard = TestAllocGuard {
            alloc: &alloc,
            ptr,
            size,
        };

        // SAFETY: freshly allocated map leaf has capacity for 3 entries.
        unsafe {
            let vals = ptr.as_ptr().cast::<u64>();
            vals.add(0).write(10);
            vals.add(1).write(20);
            vals.add(2).write(30);
            let keys = ptr.as_ptr().add(leaf::map_keys_offset(3));
            *keys.add(0) = 0x02;
            *keys.add(1) = 0x05;
            *keys.add(2) = 0x09;
        }
        let mut edge = Edge::new_node(ptr.as_ptr(), EdgeType::Leaf1.as_u8());
        edge.set_pop0(1, 2); // pop0 = 2 => pop = 3
        assert_eq!(
            leaf::cap_class(3),
            leaf::cap_class(4),
            "the fourth entry must stay in the class for the in-place path"
        );

        let mut word: u32 = 0;
        let mut path = InsertPathMap::empty();
        // SAFETY: edge is a valid Leaf1 map node; `word` outlives the call.
        let (old, _slot) = unsafe {
            map_insert_with_path_occ::<false, true, false>(
                &alloc,
                &mut edge,
                0x07,
                25,
                1,
                &mut path,
                Cover::Node(&raw mut word),
            )
        };
        assert_eq!(old, None);
        assert_eq!(word, 2, "one begin/end pair on the cover word");
        assert_eq!(edge.pop0(1), 3);
        #[cfg(debug_assertions)]
        assert!(crate::alloc::bracket_stack::open().is_empty());
    }
}
