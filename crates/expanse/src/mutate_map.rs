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
    BRANCHB_UP, LEAF_CAP, LEAF1_CAP, LEAFB1_DOWN, branch_form_level, bump_pop0, decode_value,
    divergence_level, downgrade_b_to_l7, downgrade_l7_to_l3, downgrade_u_to_b, key_low,
    linear_insert_slot, linear_insert_slot_l3, linear_remove_slot, map_immed_max, read_packed,
    restore_decode, split_skip, sub_edges_size, sub_vals_size, upgrade_b_to_u, upgrade_l3_to_l7,
    upgrade_l7_to_b, wrap_skip_level, write_decode, write_packed, write_packed_fixed,
};
use crate::node::{BranchB, BranchL3, BranchL7, BranchU, Edge, LeafBitmapL};
use crate::types::{BRANCH_L3_CAP, BRANCH_L7_CAP, EdgeTag, EdgeType, ImmedType, Key, digit};

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

/// Runtime entry point with path tracking for multi-level sequential insert bypass.
///
/// # Safety
///
/// Same contract as [`map_insert`].
pub(crate) unsafe fn map_insert_path_dyn<const KEEP: bool>(
    a: &NodeAlloc,
    edge: &mut Edge,
    key: Key,
    val: u64,
    level: u8,
    path: &mut InsertPathMap,
) -> (Option<u64>, *mut u64) {
    // SAFETY: forwarded caller contract.
    unsafe {
        if a.occ_enabled() {
            map_insert_with_path::<KEEP, true>(a, edge, key, val, level, path)
        } else {
            map_insert_with_path::<KEEP, false>(a, edge, key, val, level, path)
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
    // SAFETY: forwarded contract.
    unsafe {
        map_insert_with_path::<KEEP, OCC>(a, edge, key, val, level, &mut InsertPathMap::empty())
    }
}

/// Inserts `key → val` in the subtree at `edge` while recording the descent path
/// for fast sequential bypass.
///
/// # Safety
///
/// Same contract as [`map_insert`].
pub(crate) unsafe fn map_insert_with_path<const KEEP: bool, const OCC: bool>(
    a: &NodeAlloc,
    edge: &mut Edge,
    key: Key,
    val: u64,
    level: u8,
    path: &mut InsertPathMap,
) -> (Option<u64>, *mut u64) {
    if OCC {
        // SAFETY: forwarded contract.
        unsafe { map_insert_with_path_occ::<KEEP, OCC>(a, edge, key, val, level, path) }
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
                        *edge =
                            Edge::new_node(node.as_ptr().cast(), EdgeType::branch_l3_tag(level));
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

            0x01 | 0x82..=0x88 => {
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
                        upgrade_l3_to_l7(a, &mut *edge);
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
                        upgrade_l7_to_b(a, &mut *edge);
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
                        upgrade_b_to_u(a, &mut *edge);
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
                                    EdgeType::branch_l3_tag(bl),
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
                        let vals = a.alloc_bytes(16).cast::<u64>();
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

/// Fallback version-bracketed recursive descent for OCC-enabled concurrent maps.
unsafe fn map_insert_with_path_occ<const KEEP: bool, const OCC: bool>(
    a: &NodeAlloc,
    edge: &mut Edge,
    key: Key,
    val: u64,
    mut level: u8,
    path: &mut InsertPathMap,
) -> (Option<u64>, *mut u64) {
    loop {
        debug_assert!((1..=8).contains(&level));
        let tag = edge.tag().expect("valid edge tag");
        match tag {
            EdgeTag::Structural(EdgeType::Null) => {
                path.clear();
                if level == 8 {
                    let node = a.alloc_node_zeroed::<BranchL3>();
                    // SAFETY: node is freshly allocated zeroed BranchL3 memory.
                    unsafe {
                        (*node.as_ptr()).hdr.level = level;
                    }
                    *edge = Edge::new_node(node.as_ptr().cast(), EdgeType::branch_l3_tag(level));
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
                *edge = Edge::NULL;
                edge.set_imm_bytes(val.to_le_bytes());
                edge.set_aux_bytes(aux);
                edge.set_tag(im.as_u8());
                // A single-entry immediate's value slot is the edge's word 0.
                return (None, (&raw mut *edge).cast::<u64>());
            }

            EdgeTag::Immed(im) => {
                debug_assert_eq!(im.key_bytes(), level);
                path.clear();
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
                            edge.set_imm_bytes(val.to_le_bytes());
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
                        let vals = a.alloc_bytes(16).cast::<u64>();
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
                        *edge = Edge::new_node(vals.as_ptr().cast(), 0);
                        edge.set_aux_bytes(new_aux);
                        edge.set_tag(new_im.as_u8());
                        // SAFETY: slot `pos` in the newly allocated value array.
                        return (None, unsafe { vals.as_ptr().add(pos) });
                    } else {
                        // Immediate max capacity is 1 (for kb in 4..=7): upgrade directly to linear leaf.
                        let entries = [(slot0_k, slot0_v), (slot1_k, slot1_v)];
                        build_map_leaf(a, edge, kb, &entries);
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
                            // SAFETY: slot is writable per contract.
                            unsafe { slot.write(val) };
                        }
                        return (Some(old), slot);
                    }
                    Err(p) => p,
                };
                let old_vals = edge.node_ptr().cast::<u64>();
                if n < map_immed_max(kb) {
                    let kb_usize = kb as usize;
                    if leaf::cap_class(n + 1) == leaf::cap_class(n) {
                        // Spare class capacity: shift values and keys in-place without reallocating!
                        // SAFETY: class capacity holds n + 1 entries; in-bounds shifts.
                        unsafe {
                            if pos < n {
                                core::ptr::copy(old_vals.add(pos), old_vals.add(pos + 1), n - pos);
                            }
                            old_vals.add(pos).write(val);
                        }
                        let mut new_aux = *edge.aux_bytes();
                        if pos < n {
                            new_aux.copy_within(pos * kb_usize..n * kb_usize, (pos + 1) * kb_usize);
                        }
                        // SAFETY: new_aux has capacity for n + 1 packed keys.
                        unsafe {
                            write_packed(new_aux.as_mut_ptr(), pos, kb_usize, k);
                        }
                        let new_im = ImmedType::new(kb, (n + 1) as u8).expect("immediate capacity");
                        edge.set_aux_bytes(new_aux);
                        edge.set_tag(new_im.as_u8());
                        // SAFETY: old_vals has capacity for n + 1 entries; pos is in-bounds.
                        return (None, unsafe { old_vals.add(pos) });
                    }
                    let new_vals = a.alloc_bytes(map_immed_val_size(n + 1)).cast::<u64>();
                    // SAFETY: copy n values around pos, write val at pos, free old array.
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
                            core::ptr::NonNull::new(edge.node_ptr()).expect("value array"),
                            map_immed_val_size(n),
                        );
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
                    *edge = Edge::new_node(new_vals.as_ptr().cast(), 0);
                    edge.set_aux_bytes(new_aux);
                    edge.set_tag(new_im.as_u8());
                    // SAFETY: slot pos in the newly allocated (n+1)-element value array.
                    return (None, unsafe { new_vals.as_ptr().add(pos) });
                }
                // Overflow immediate capacity -> build linear leaf.
                let mut entries = StackEntries32::new();
                let kb_usize = kb as usize;
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
                // SAFETY: old_vals points to live n*8 byte allocation.
                unsafe {
                    a.free_bytes(
                        core::ptr::NonNull::new(edge.node_ptr()).expect("value array"),
                        map_immed_val_size(n),
                    );
                }
                build_map_leaf(a, edge, kb, entries.as_slice());
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
                path.clear();
                let kb = t.leaf_key_bytes().expect("leaf tag");
                debug_assert!(kb <= level);
                let pop = edge.pop0(kb) as usize + 1;
                if kb < level && !crate::get::decode_matches(edge, key, kb, level) {
                    // Diverges inside the skipped prefix: branch out one
                    // level and retry.
                    split_skip(a, edge, key, level, pop as u64);
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
                        // SAFETY: in-place value swap within the live leaf.
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
                            // Fast path: spare class capacity — shift both
                            // areas in place.
                            // SAFETY: class capacity spare per the check.
                            unsafe { leaf::map_insert_at(base, kb, pop, pos, k, val) };
                            edge.set_pop0(kb, pop as u64);
                            if kb == 1 && level == 1 {
                                path.prefix = key >> 8;
                                path.leaf = core::ptr::null_mut();
                                path.leaf1 = base;
                                path.terminal_pop = (pop + 1) as u16;
                                path.edges[0] = edge as *mut Edge;
                                path.levels[0] = 1;
                                path.depth = 1;
                                path.pending_pop = 0;
                            } else {
                                path.clear();
                            }
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
                            if kb == 1 && level == 1 {
                                path.prefix = key >> 8;
                                path.leaf = core::ptr::null_mut();
                                path.leaf1 = new.as_ptr();
                                path.terminal_pop = (pop + 1) as u16;
                                path.edges[0] = edge as *mut Edge;
                                path.levels[0] = 1;
                                path.depth = 1;
                                path.pending_pop = 0;
                            } else {
                                path.clear();
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
                            build_bitmap_leaf_map(a, edge, &low);
                            write_decode(edge, 1, level, prefix_key);
                            if level == 1 {
                                path.prefix = key >> 8;
                                path.leaf = edge.node_ptr().cast::<LeafBitmapL>();
                                path.leaf1 = core::ptr::null_mut();
                                path.terminal_pop = entries.len() as u16;
                                path.edges[0] = edge as *mut Edge;
                                path.levels[0] = 1;
                                path.depth = 1;
                                path.pending_pop = 0;
                            }
                        } else {
                            // Cascade: an empty branch at the divergence level
                            // (a narrow pointer when that sits below the slot;
                            // level-8 slots always branch in place — the root
                            // edge has no room for decode bytes), re-insert.
                            let d =
                                divergence_level(entries[0].0, entries[entries.len() - 1].0, level);
                            let bl = if level <= 7 { d } else { level };
                            let node = a.alloc_node_zeroed::<BranchL3>();
                            // SAFETY: node is freshly allocated zeroed BranchL3 memory.
                            unsafe {
                                (*node.as_ptr()).hdr.level = bl;
                            }
                            *edge =
                                Edge::new_node(node.as_ptr().cast(), EdgeType::branch_l3_tag(bl));
                            if bl < level {
                                write_decode(edge, bl, level, entries[0].0);
                            }
                            for &(k, v) in &entries {
                                // SAFETY: freshly built branch subtree.
                                let prev = unsafe {
                                    map_insert_with_path::<KEEP, OCC>(a, edge, k, v, level, path)
                                };
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
                        return (None, slot);
                    }
                }
            }

            EdgeTag::Structural(EdgeType::LeafB1) => {
                if level > 1 && !crate::get::decode_matches(edge, key, 1, level) {
                    // Diverges inside the skipped prefix: branch out one
                    // level and retry.
                    let pop = edge.pop0(1) + 1;
                    path.clear();
                    split_skip(a, edge, key, level, pop);
                    continue;
                }
                let d = digit(key, 1);
                // Phase 7: a leaf carries no version of its own — readers
                // validate its payload against the parent branch's version,
                // whose bracket must still be open here.
                a.assert_bracketed();
                // SAFETY: live LeafBitmapL per contract.
                let node = unsafe { &mut *edge.node_ptr().cast::<LeafBitmapL>() };
                let sub = (d >> 5) as usize;
                if let Some(rank) = node.bitmap.test_and_subexpanse_rank(d) {
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
                let rank = node.bitmap.subexpanse_rank(d) as usize;
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
                if level == 1 {
                    path.prefix = key >> 8;
                    path.leaf = edge.node_ptr().cast::<LeafBitmapL>();
                    path.leaf1 = core::ptr::null_mut();
                    path.terminal_pop = (pop0 + 2) as u16;
                    path.edges[0] = edge as *mut Edge;
                    path.levels[0] = 1;
                    path.depth = 1;
                    path.pending_pop = 0;
                }
                // SAFETY: the value now lives at this subarray rank.
                return (None, unsafe { node.values[sub].add(rank) });
            }

            EdgeTag::Structural(EdgeType::FullExpanse) => {
                unreachable!("full-expanse edges are set-flavor only")
            }

            EdgeTag::Structural(
                t @ (EdgeType::BranchL3
                | EdgeType::BranchL7
                | EdgeType::BranchL3L2
                | EdgeType::BranchL3L3
                | EdgeType::BranchL3L4
                | EdgeType::BranchL3L5
                | EdgeType::BranchL3L6
                | EdgeType::BranchL3L7
                | EdgeType::BranchL3L8),
            ) => {
                debug_assert!(level >= 2);
                // SAFETY: live branch per contract.
                let bl = unsafe { branch_form_level(edge, t, level) };
                if bl < level && !crate::get::decode_matches(edge, key, bl, level) {
                    let pop = edge.pop0(bl) + 1;
                    path.clear();
                    split_skip(a, edge, key, level, pop);
                    continue;
                }
                let d = digit(key, bl);
                let is_l3 = t.is_branch_l3();
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
                    // The node's version brackets the descent (Phase 7 OCC).
                    let res = unsafe {
                        if is_l3 {
                            let b = &mut *edge.node_ptr().cast::<BranchL3>();
                            crate::occ::version_begin_if::<OCC>(a, &mut b.hdr.version);
                            let r = map_insert_with_path::<KEEP, OCC>(
                                a,
                                &mut b.edges[slot],
                                key,
                                val,
                                bl - 1,
                                path,
                            );
                            crate::occ::version_end_if::<OCC>(a, &mut b.hdr.version);
                            r
                        } else {
                            let b = &mut *edge.node_ptr().cast::<BranchL7>();
                            crate::occ::version_begin_if::<OCC>(a, &mut b.hdr.version);
                            let r = map_insert_with_path::<KEEP, OCC>(
                                a,
                                &mut b.edges[slot],
                                key,
                                val,
                                bl - 1,
                                path,
                            );
                            crate::occ::version_end_if::<OCC>(a, &mut b.hdr.version);
                            r
                        }
                    };
                    if res.0.is_none() {
                        // SAFETY: edge is a valid live edge.
                        unsafe { bump_pop0(edge, bl, 1) };
                        path.record_ancestor(edge as *mut Edge, level);
                    }
                    return res;
                }
                let cap = if is_l3 { BRANCH_L3_CAP } else { BRANCH_L7_CAP };
                if num == cap {
                    path.clear();
                    // SAFETY: upgrade rebuilds the node; subtree stays owned.
                    unsafe {
                        if is_l3 {
                            upgrade_l3_to_l7(a, edge);
                        } else {
                            upgrade_l7_to_b(a, edge);
                        }
                    }
                    continue;
                }
                // SAFETY: live branch; slot arithmetic bounded by capacity.
                let res = unsafe {
                    if is_l3 {
                        let b = &mut *edge.node_ptr().cast::<BranchL3>();
                        crate::occ::version_begin_if::<OCC>(a, &mut b.hdr.version);
                        let slot = linear_insert_slot_l3(&mut b.hdr.digits, &mut b.edges, num, d);
                        b.hdr.num += 1;
                        b.hdr.add_presence(d);
                        let r = map_insert_with_path::<KEEP, OCC>(
                            a,
                            &mut b.edges[slot],
                            key,
                            val,
                            bl - 1,
                            path,
                        );
                        crate::occ::version_end_if::<OCC>(a, &mut b.hdr.version);
                        r
                    } else {
                        let b = &mut *edge.node_ptr().cast::<BranchL7>();
                        crate::occ::version_begin_if::<OCC>(a, &mut b.hdr.version);
                        let slot = linear_insert_slot(&mut b.hdr.digits, &mut b.edges, num, d);
                        b.hdr.num += 1;
                        b.hdr.add_presence(d);
                        let r = map_insert_with_path::<KEEP, OCC>(
                            a,
                            &mut b.edges[slot],
                            key,
                            val,
                            bl - 1,
                            path,
                        );
                        crate::occ::version_end_if::<OCC>(a, &mut b.hdr.version);
                        r
                    }
                };
                debug_assert!(res.0.is_none());
                // SAFETY: edge is a valid live edge.
                unsafe { bump_pop0(edge, bl, 1) };
                path.record_ancestor(edge as *mut Edge, level);
                return (None, res.1);
            }

            EdgeTag::Structural(EdgeType::BranchB) => {
                debug_assert!(level >= 2);
                // SAFETY: live branch per contract.
                let bl = unsafe { branch_form_level(edge, EdgeType::BranchB, level) };
                if bl < level && !crate::get::decode_matches(edge, key, bl, level) {
                    let pop = edge.pop0(bl) + 1;
                    path.clear();
                    split_skip(a, edge, key, level, pop);
                    continue;
                }
                let slot_level = level;
                let d = digit(key, bl);
                // SAFETY: live BranchB per contract.
                let b = unsafe { &mut *edge.node_ptr().cast::<BranchB>() };
                if let Some(slot) = b.bitmap.test_and_subexpanse_rank(d) {
                    let sub = b.subarrays[(d >> 5) as usize];
                    // SAFETY: bitmap/subarray consistency invariant. The
                    // node's version brackets the descent (Phase 7 OCC).
                    crate::occ::version_begin_if::<OCC>(a, &mut b.version);
                    // SAFETY: bitmap/subarray consistency invariant.
                    let res = unsafe {
                        map_insert_with_path::<KEEP, OCC>(
                            a,
                            &mut *sub.add(slot),
                            key,
                            val,
                            bl - 1,
                            path,
                        )
                    };
                    crate::occ::version_end_if::<OCC>(a, &mut b.version);
                    if res.0.is_none() {
                        // SAFETY: edge is a valid live edge.
                        unsafe { bump_pop0(edge, bl, 1) };
                        path.record_ancestor(edge as *mut Edge, level);
                    }
                    return res;
                }
                if b.bitmap.count() as usize + 1 > BRANCHB_UP {
                    if bl < slot_level {
                        // BranchU cannot skip: materialize one chain level.
                        let pop = edge.pop0(bl) + 1;
                        path.clear();
                        wrap_skip_level(a, edge, bl + 1, slot_level, pop);
                        level = slot_level;
                        continue;
                    }
                    path.clear();
                    // SAFETY: upgrade rebuilds the node; subtree stays owned.
                    unsafe {
                        upgrade_b_to_u(a, edge);
                    }
                    level = slot_level;
                    continue;
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
                    map_insert_with_path::<KEEP, OCC>(
                        a,
                        &mut *b.subarrays[sub].add(rank),
                        key,
                        val,
                        bl - 1,
                        path,
                    )
                };
                crate::occ::version_end_if::<OCC>(a, &mut b.version);
                debug_assert!(res.0.is_none());
                // SAFETY: edge is a valid live edge.
                unsafe { bump_pop0(edge, bl, 1) };
                path.record_ancestor(edge as *mut Edge, level);
                return (None, res.1);
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
                    map_insert_with_path::<KEEP, OCC>(
                        a,
                        &mut b.edges[d as usize],
                        key,
                        val,
                        level - 1,
                        path,
                    )
                };
                crate::occ::version_end_if::<OCC>(a, &mut b.version);
                if res.0.is_none() {
                    // SAFETY: edge is a valid live edge.
                    unsafe { bump_pop0(edge, level, 1) };
                    path.record_ancestor(edge as *mut Edge, level);
                }
                return res;
            }
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
            let n = im.key_count() as usize;
            if n == 1 {
                // SAFETY: single-key map immediate holds 1 key in aux bytes.
                let existing_k = unsafe { read_packed(edge.aux_bytes().as_ptr(), 0, kb as usize) };
                if existing_k == k {
                    let old = u64::from_le_bytes(edge.imm_bytes());
                    *edge = Edge::NULL;
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
                // SAFETY: freeing the old 2-entry value array.
                unsafe {
                    a.free_bytes(
                        core::ptr::NonNull::new(edge.node_ptr()).expect("value array"),
                        map_immed_val_size(2),
                    );
                }
                *edge = Edge::NULL;
                edge.set_imm_bytes(remain_val.to_le_bytes());
                edge.set_aux_bytes(new_aux);
                edge.set_tag(new_im.as_u8());
                return Some(old);
            }
            let kb_usize = kb as usize;
            if leaf::cap_class(n - 1) == leaf::cap_class(n) {
                // Spare class capacity: shift values and keys in-place without reallocating!
                // SAFETY: shift surviving values left within the allocated array.
                unsafe {
                    if pos + 1 < n {
                        core::ptr::copy(vals.add(pos + 1), vals.add(pos), n - 1 - pos);
                    }
                }
                let mut new_aux = *edge.aux_bytes();
                new_aux.copy_within((pos + 1) * kb_usize..n * kb_usize, pos * kb_usize);
                new_aux[((n - 1) * kb_usize)..].fill(0);
                let new_im = ImmedType::new(kb, (n - 1) as u8).expect("immediate capacity");
                edge.set_aux_bytes(new_aux);
                edge.set_tag(new_im.as_u8());
                return Some(old);
            }
            // n > 2: copy values around pos and shift keys in aux.
            let new_vals = a.alloc_bytes(map_immed_val_size(n - 1)).cast::<u64>();
            // SAFETY: copy n-1 surviving values and free old array.
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
                a.free_bytes(
                    core::ptr::NonNull::new(edge.node_ptr()).expect("value array"),
                    map_immed_val_size(n),
                );
            }
            let mut new_aux = *edge.aux_bytes();
            new_aux.copy_within((pos + 1) * kb_usize..n * kb_usize, pos * kb_usize);
            new_aux[((n - 1) * kb_usize)..].fill(0);
            let new_im = ImmedType::new(kb, (n - 1) as u8).expect("immediate capacity");
            *edge = Edge::new_node(new_vals.as_ptr().cast(), 0);
            edge.set_aux_bytes(new_aux);
            edge.set_tag(new_im.as_u8());
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
            let keys_ptr = unsafe { base.add(leaf::map_keys_offset(pop)) };
            // SAFETY: keys_ptr points to `pop * kb` valid bytes.
            let pos = match unsafe { leaf::locate(keys_ptr, pop, kb, k) } {
                Ok(pos) => pos,
                Err(_) => return None,
            };
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
            // Slow path (conversion to immediate or null).
            let old_ptr = edge.node_ptr();
            let old_size = leaf::size_map(kb, pop);
            // SAFETY: pos < pop is within the live map leaf values.
            let old = unsafe { *base.cast::<u64>().add(pos) };
            if pop == 1 {
                // SAFETY: old leaf allocation no longer referenced.
                unsafe {
                    a.free_bytes(core::ptr::NonNull::new(old_ptr).expect("leaf"), old_size);
                }
                *edge = Edge::NULL;
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
            write_map_immed(a, edge, level, &entries[..rem_pop]);
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
            let sub = (d >> 5) as usize;
            let rank = node.bitmap.test_and_subexpanse_rank(d)?;
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
            Some(old)
        }

        EdgeTag::Structural(EdgeType::FullExpanse) => {
            unreachable!("full-expanse edges are set-flavor only")
        }

        EdgeTag::Structural(
            t @ (EdgeType::BranchL3
            | EdgeType::BranchL7
            | EdgeType::BranchL3L2
            | EdgeType::BranchL3L3
            | EdgeType::BranchL3L4
            | EdgeType::BranchL3L5
            | EdgeType::BranchL3L6
            | EdgeType::BranchL3L7
            | EdgeType::BranchL3L8),
        ) => {
            // SAFETY: live branch per contract.
            let bl = unsafe { branch_form_level(edge, t, level) };
            if bl < level && !crate::get::decode_matches(edge, key, bl, level) {
                return None;
            }
            let d = digit(key, bl);
            let is_l3 = t.is_branch_l3();
            // SAFETY: live branch per contract.
            let removed = unsafe {
                if is_l3 {
                    let b = &mut *edge.node_ptr().cast::<BranchL3>();
                    let slot = b.hdr.find(d)?;
                    crate::occ::version_begin_if::<OCC>(a, &mut b.hdr.version);
                    let r = map_remove::<OCC>(a, &mut b.edges[slot], key, bl - 1);
                    let child_null = r.is_some() && b.edges[slot].is_null();
                    if child_null {
                        linear_remove_slot(
                            &mut b.hdr.digits,
                            &mut b.edges,
                            b.hdr.num as usize,
                            slot,
                        );
                        b.hdr.num -= 1;
                        b.hdr.refresh_presence();
                    }
                    crate::occ::version_end_if::<OCC>(a, &mut b.hdr.version);
                    (r, child_null)
                } else {
                    let b = &mut *edge.node_ptr().cast::<BranchL7>();
                    let slot = b.hdr.find(d)?;
                    crate::occ::version_begin_if::<OCC>(a, &mut b.hdr.version);
                    let r = map_remove::<OCC>(a, &mut b.edges[slot], key, bl - 1);
                    let child_null = r.is_some() && b.edges[slot].is_null();
                    if child_null {
                        linear_remove_slot(
                            &mut b.hdr.digits,
                            &mut b.edges,
                            b.hdr.num as usize,
                            slot,
                        );
                        b.hdr.num -= 1;
                        b.hdr.refresh_presence();
                    }
                    crate::occ::version_end_if::<OCC>(a, &mut b.hdr.version);
                    (r, child_null)
                }
            };
            let (old_opt, child_null) = removed;
            let old = old_opt?;
            if child_null {
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
                                core::ptr::NonNull::new(edge.node_ptr().cast::<BranchL3>())
                                    .unwrap(),
                            );
                        } else {
                            a.free_node(
                                core::ptr::NonNull::new(edge.node_ptr().cast::<BranchL7>())
                                    .unwrap(),
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
            } else {
                // SAFETY: edge is a valid live edge.
                unsafe { bump_pop0(edge, bl, -1) };
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
            let rank = b.bitmap.test_and_subexpanse_rank(d)?;
            let sub = (d >> 5) as usize;
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
            if child_null {
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
                // SAFETY: edge is a valid live edge.
                unsafe { bump_pop0(edge, bl, -1) };
                if digits < BRANCH_L7_CAP {
                    // SAFETY: rebuild keeps the subtree owned.
                    unsafe { downgrade_b_to_l7(a, edge) };
                }
            } else {
                // SAFETY: edge is a valid live edge.
                unsafe { bump_pop0(edge, bl, -1) };
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
            let child_is_null = b.edges[d as usize].is_null();
            crate::occ::version_end_if::<OCC>(a, &mut b.version);
            if child_is_null {
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
                // SAFETY: edge is a valid live edge.
                unsafe { bump_pop0(edge, level, -1) };
                if digits < BRANCHB_UP {
                    // SAFETY: rebuild keeps the subtree owned.
                    unsafe { downgrade_u_to_b(a, edge, level) };
                }
            } else {
                // SAFETY: edge is a valid live edge.
                unsafe { bump_pop0(edge, level, -1) };
            }
            Some(old)
        }
    }
}
