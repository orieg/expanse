//! Direct-emission bulk builder and materialization for set algebra (#348).
//!
//! Split out of `algebra.rs` so the cardinality walk (`intersection_len` and
//! its helpers) keeps the module it shipped in for #339/#347 — the builder and
//! the materializing ops are the only additions here, sharing `algebra`'s
//! read-only descent helpers (`child_by_digit`, `as_level1_bitmap`,
//! `subtree_count`, `terminal_remainders`, `full_count`, `is_branch`).

use crate::algebra::{
    as_level1_bitmap, child_by_digit, full_count, is_branch, subtree_count, terminal_remainders_buf,
};
use crate::alloc::NodeAlloc;
use crate::bits::Bitmap256;
use crate::get;
use crate::mutate;
use crate::node::{BranchB, BranchL3, BranchL7, BranchU, Edge, LeafBitmap1};
use crate::types::{BRANCH_L3_CAP, BRANCH_L7_CAP, EdgeTag, EdgeType, ImmedType, digit};

use core::mem::MaybeUninit;

extern crate alloc;
use alloc::vec::Vec;

// ===========================================================================
// Bulk builder — direct-emission construction of a canonical subtree from a
// sorted, distinct key set (issue #348). The forms it emits mirror the
// mutation ladder's convergent shapes (immediate ≤ `max_count`, linear leaf ≤
// cap, bitmap leaf / full-expanse / branch by population and divergence), so a
// tree built here is content-equivalent to the insert path's, canonical, and
// never less compact — it may pick the more-compact `FullExpanse` where an
// ascending insert leaves a full bitmap leaf. All node construction reuses the
// mutation engine's constructors, so the invariants validator, Miri, and the
// fuzzers certify one construction path, not two.
// ===========================================================================

/// A `FullExpanse` edge covering a `level`-byte expanse (`256^level` keys).
/// `pop0` is stored at `level` for `level <= 7`; a level-8 full expanse (the
/// whole universe) carries no `pop0` (the owner tracks the total).
#[inline]
fn full_edge(level: u8) -> Edge {
    let mut e = Edge::NULL;
    e.set_tag(EdgeType::FullExpanse.as_u8());
    if level <= 7 {
        e.set_pop0(level, full_count(level) - 1);
    }
    e
}

/// Immediate hysteresis floor / capacity: the largest set-flavor immediate at
/// `level` holds `15 / level` full-remainder keys.
#[inline]
fn immed_max(level: u8) -> usize {
    ImmedType::max_count(level) as usize
}

/// Writes the narrow-pointer decode bytes of a `kb == 1` terminal that skips
/// up to `level`, taken from the shared middle digits `dv` (LSB = level-2
/// digit). A no-op at `level == 1`.
#[inline]
fn set_skip_decode(edge: &mut Edge, level: u8, dv: u64) {
    if level > 1 {
        // `write_decode` reads digits at levels `2..=level` of the key; the
        // final byte is irrelevant, so `dv << 8` is a valid representative.
        mutate::write_decode(edge, 1, level, dv << 8);
    }
}

/// Emits the canonical terminal for the final-byte membership `bm` inside a
/// `level`-byte expanse whose shared middle digits are `dv` (0 at level 1):
/// full-expanse (only at level 1), immediate, linear leaf, or bitmap leaf, by
/// population — the same ladder the mutation engine converges to. Returns
/// `None` for an empty bitmap.
///
/// # Safety
///
/// `a` owns the tree the returned edge is spliced into.
unsafe fn build_leaf_from_bitmap(
    a: &NodeAlloc,
    bm: &Bitmap256,
    level: u8,
    dv: u64,
) -> Option<Edge> {
    let count = bm.count() as usize;
    if count == 0 {
        return None;
    }
    if count == 256 && level == 1 {
        return Some(full_edge(1));
    }
    // Enumerate set bits in ascending order (the terminal's keys are the
    // shared prefix `dv` followed by each present final byte).
    let base = dv << 8;
    if count <= immed_max(level) {
        // Immediate: full `level`-byte remainders, no skip.
        let mut keys = Vec::with_capacity(count);
        let mut from = bm.next_set(0);
        while let Some(b) = from {
            keys.push(base | u64::from(b));
            from = if b == 255 { None } else { bm.next_set(b + 1) };
        }
        let mut e = Edge::NULL;
        mutate::write_immed(&mut e, level, &keys);
        return Some(e);
    }
    if count <= mutate::LEAF1_CAP {
        // Linear leaf over the final byte (`kb == 1`), skip carried in decode.
        let mut finals = Vec::with_capacity(count);
        let mut from = bm.next_set(0);
        while let Some(b) = from {
            finals.push(u64::from(b));
            from = if b == 255 { None } else { bm.next_set(b + 1) };
        }
        let mut e = Edge::NULL;
        mutate::build_leaf(a, &mut e, 1, &finals);
        set_skip_decode(&mut e, level, dv);
        return Some(e);
    }
    // Bitmap leaf: copy the whole word-parallel bitmap in one shot.
    let ptr = a.alloc_node_zeroed::<LeafBitmap1>();
    // SAFETY: `ptr` is a fresh, zeroed, 64-byte-aligned LeafBitmap1.
    unsafe {
        (*ptr.as_ptr()).bitmap = *bm;
    }
    let mut e = Edge::new_node(ptr.as_ptr().cast(), EdgeType::LeafB1.as_u8());
    e.set_pop0(1, count as u64 - 1);
    set_skip_decode(&mut e, level, dv);
    Some(e)
}

/// Emits the canonical terminal for `keys` that diverge only in their final
/// byte (they share every digit at levels `2..=level`). Routes through the
/// final-byte bitmap so a single ladder decides the form.
///
/// # Safety
///
/// `keys` is non-empty, sorted, distinct, sharing digits above the final byte;
/// `a` owns the tree.
unsafe fn build_terminal(a: &NodeAlloc, keys: &[u64], level: u8) -> Edge {
    let mut bm = Bitmap256::new();
    for &k in keys {
        bm.set(digit(k, 1));
    }
    let dv = if level > 1 {
        (keys[0] >> 8) & ((1u64 << (8 * u32::from(level - 1))) - 1)
    } else {
        0
    };
    // SAFETY: non-empty bitmap; `a` owns the tree.
    unsafe { build_leaf_from_bitmap(a, &bm, level, dv) }.expect("non-empty terminal")
}

/// Assembles a branch node at `level` from `digits`/`children` (sorted by
/// digit, `1..=256` of them), picking the form the population implies:
/// L3 ≤ 3, L7 ≤ 7, bitmap ≤ `BRANCHB_UP`, else uncompressed — the mutation
/// ladder's thresholds. Sets no `pop0`/decode (the caller owns the slot).
///
/// # Safety
///
/// Every child edge is a live subtree at `level - 1`; `a` owns the tree.
unsafe fn build_branch(a: &NodeAlloc, level: u8, digits: &[u8], children: &[Edge]) -> Edge {
    let m = digits.len();
    debug_assert!((1..=256).contains(&m));
    debug_assert_eq!(m, children.len());
    if m <= BRANCH_L3_CAP {
        let node = a.alloc_node_zeroed::<BranchL3>();
        // SAFETY: `node` is a fresh, exclusively-owned zeroed BranchL3.
        unsafe {
            let b = &mut *node.as_ptr();
            b.hdr.level = level;
            b.hdr.num = m as u8;
            b.hdr.digits[..m].copy_from_slice(digits);
            b.edges[..m].copy_from_slice(children);
            b.hdr.refresh_presence();
        }
        Edge::new_node(node.as_ptr().cast(), EdgeType::branch_l3_tag(level))
    } else if m <= BRANCH_L7_CAP {
        let node = a.alloc_node_zeroed::<BranchL7>();
        // SAFETY: `node` is a fresh, exclusively-owned zeroed BranchL7.
        unsafe {
            let b = &mut *node.as_ptr();
            b.hdr.level = level;
            b.hdr.num = m as u8;
            b.hdr.digits[..m].copy_from_slice(digits);
            b.edges[..m].copy_from_slice(children);
            b.hdr.refresh_presence();
        }
        Edge::new_node(node.as_ptr().cast(), EdgeType::BranchL7.as_u8())
    } else if m <= mutate::BRANCHB_UP {
        let node = a.alloc_node_zeroed::<BranchB>();
        // SAFETY: fresh zeroed BranchB; subarrays sized to per-subexpanse counts.
        unsafe {
            let b = &mut *node.as_ptr();
            b.level = level;
            for &d in digits {
                b.pop_counts[(d >> 5) as usize] += 1;
            }
            for sub in 0..8 {
                let n = b.pop_counts[sub] as usize;
                if n > 0 {
                    b.subarrays[sub] = a
                        .alloc_bytes(mutate::sub_edges_size(n))
                        .cast::<Edge>()
                        .as_ptr();
                }
            }
            let mut filled = [0usize; 8];
            for i in 0..m {
                let d = digits[i];
                let sub = (d >> 5) as usize;
                b.bitmap.set(d);
                // SAFETY: `filled[sub] < pop_counts[sub]` slots allocated.
                b.subarrays[sub].add(filled[sub]).write(children[i]);
                filled[sub] += 1;
            }
        }
        Edge::new_node(node.as_ptr().cast(), EdgeType::BranchB.as_u8())
    } else {
        let node = a.alloc_node_zeroed::<BranchU>();
        // SAFETY: fresh zeroed BranchU; direct-indexed by digit.
        unsafe {
            let b = &mut *node.as_ptr();
            for i in 0..m {
                b.edges[digits[i] as usize] = children[i];
            }
        }
        Edge::new_node(node.as_ptr().cast(), EdgeType::BranchU.as_u8())
    }
}

/// Builds a branch at form-level `bl` from `keys` (which span ≥ 2 digits at
/// `bl`), sitting in a slot at `slot_level` (a narrow pointer when
/// `bl < slot_level`). Partitions the sorted keys by their `bl` digit,
/// recurses each run at `bl - 1`, and stamps the edge's `pop0`/decode.
///
/// # Safety
///
/// `keys` is sorted, distinct, non-empty, sharing digits above `slot_level`;
/// `a` owns the tree.
unsafe fn build_branch_from_keys(a: &NodeAlloc, keys: &[u64], slot_level: u8, bl: u8) -> Edge {
    let n = keys.len();
    let mut digits: Vec<u8> = Vec::new();
    let mut children: Vec<Edge> = Vec::new();
    let mut i = 0;
    while i < n {
        let d = digit(keys[i], bl);
        let start = i;
        while i < n && digit(keys[i], bl) == d {
            i += 1;
        }
        // SAFETY: the run shares digits above `bl - 1`; `a` owns the tree.
        let child = unsafe { build_subtree(a, &keys[start..i], bl - 1) };
        digits.push(d);
        children.push(child);
    }
    // SAFETY: children are live subtrees at `bl - 1`.
    let mut e = unsafe { build_branch(a, bl, &digits, &children) };
    if bl <= 7 {
        e.set_pop0(bl, n as u64 - 1);
    }
    if bl < slot_level {
        if e.tag_byte() == EdgeType::BranchU as u8 {
            // An uncompressed branch has no header level and cannot skip: host
            // it under a narrow one-child linear branch that carries the skip.
            // SAFETY: `e` is the freshly built non-skipping BranchU at `bl`.
            e = unsafe { wrap_u_narrow(a, e, bl, slot_level, keys[0], n as u64) };
        } else {
            mutate::write_decode(&mut e, bl, slot_level, keys[0]);
        }
    }
    e
}

/// Hosts a non-skipping `BranchU` at level `bl` under a narrow one-child
/// `BranchL3` at level `bl + 1` that carries the skip up to `slot_level`
/// (levels `bl + 2..=slot_level` in its decode). The one-child wrapper is the
/// canonical way a full fanout sits below a shared-prefix skip — an
/// uncompressed branch itself never skips.
///
/// # Safety
///
/// `child_u` is a live non-skipping `BranchU` at `bl` with population `n`,
/// owned by `a`; `bl < slot_level`.
unsafe fn wrap_u_narrow(
    a: &NodeAlloc,
    child_u: Edge,
    bl: u8,
    slot_level: u8,
    rep_key: u64,
    n: u64,
) -> Edge {
    let node = a.alloc_node_zeroed::<BranchL3>();
    let d = digit(rep_key, bl + 1);
    // SAFETY: fresh zeroed BranchL3; one child.
    unsafe {
        (*node.as_ptr()).hdr.level = bl + 1;
        (*node.as_ptr()).hdr.num = 1;
        (*node.as_ptr()).hdr.digits[0] = d;
        (*node.as_ptr()).hdr.refresh_presence();
        (*node.as_ptr()).edges[0] = child_u;
    }
    let mut e = Edge::new_node(node.as_ptr().cast(), EdgeType::branch_l3_tag(bl + 1));
    if bl + 1 < slot_level {
        mutate::write_decode(&mut e, bl + 1, slot_level, rep_key);
    }
    // `bl < slot_level <= 8` gives `bl + 1 <= 8`; the level-8 top never skips.
    if bl < 7 {
        e.set_pop0(bl + 1, n - 1);
    }
    e
}

/// Builds the canonical subtree for `keys` inside a `level`-byte expanse.
///
/// Preconditions: `keys` is sorted ascending, distinct, non-empty, and every
/// key shares its digits above `level`. At `level == 8` the result is always a
/// branch (level-8 slots cannot skip); below that it is the minimal form the
/// mutation ladder converges to.
///
/// # Safety
///
/// The preconditions above hold and `a` owns the tree the edge is spliced into.
pub(crate) unsafe fn build_subtree(a: &NodeAlloc, keys: &[u64], level: u8) -> Edge {
    let n = keys.len();
    debug_assert!(n >= 1);

    if level == 8 {
        // The top can never skip: force a branch at level 8, even single-child.
        // SAFETY: forwarded contract.
        return unsafe { build_branch_from_keys(a, keys, 8, 8) };
    }

    if n == 1 {
        // Single key: a full-remainder immediate, no skip.
        let mut e = Edge::NULL;
        mutate::write_immed(&mut e, level, &[mutate::key_low(keys[0], level)]);
        return e;
    }

    // Divergence level of the whole (sorted) set: min and max bound it, so all
    // keys share every digit above `bl` and span ≥ 2 digits at `bl`.
    let bl = mutate::divergence_level(keys[0], keys[n - 1], level);

    // Fits a single terminal? Immediate / linear leaf keep the whole set flat.
    if n <= immed_max(level) {
        // Immediate: full `level`-byte remainders.
        let rem: Vec<u64> = keys.iter().map(|&k| mutate::key_low(k, level)).collect();
        let mut e = Edge::NULL;
        mutate::write_immed(&mut e, level, &rem);
        return e;
    }
    let leaf_cap = if level == 1 {
        mutate::LEAF1_CAP
    } else {
        mutate::LEAF_CAP
    };
    if n <= leaf_cap {
        // Linear leaf at `kb == level` (full remainders, no skip) — the shape
        // an immediate overflows into and grows in place until it splits.
        let rem: Vec<u64> = keys.iter().map(|&k| mutate::key_low(k, level)).collect();
        let mut e = Edge::NULL;
        mutate::build_leaf(a, &mut e, level, &rem);
        return e;
    }

    // Overflows the linear leaf. Diverging only at the final byte (`bl == 1`)
    // is a bitmap leaf / full expanse; otherwise a branch at `bl`.
    if bl == 1 {
        // SAFETY: forwarded contract; keys share digits `2..=level`.
        return unsafe { build_terminal(a, keys, level) };
    }
    // SAFETY: forwarded contract.
    unsafe { build_branch_from_keys(a, keys, level, bl) }
}

/// Deep-copies the subtree rooted at `edge` (covering `level` bytes) into `a`,
/// preserving every form and the edge's own `pop0`/decode aux bytes.
///
/// # Safety
///
/// `edge` references a live, well-formed subtree at `level`; `a` owns the copy.
pub(crate) unsafe fn clone_subtree(a: &NodeAlloc, edge: &Edge, level: u8) -> Edge {
    let aux = *edge.aux_bytes();
    match edge.tag() {
        Some(EdgeTag::Structural(EdgeType::Null)) => Edge::NULL,
        // Immediates and full expanses are self-contained edge values.
        Some(EdgeTag::Immed(_)) | Some(EdgeTag::Structural(EdgeType::FullExpanse)) => *edge,
        Some(EdgeTag::Structural(EdgeType::LeafB1)) => {
            // SAFETY: live LeafBitmap1.
            let src = unsafe { &*edge.node_ptr().cast::<LeafBitmap1>() };
            let ptr = a.alloc_node_zeroed::<LeafBitmap1>();
            // SAFETY: fresh node.
            unsafe {
                (*ptr.as_ptr()).bitmap = src.bitmap;
            }
            let mut e = Edge::new_node(ptr.as_ptr().cast(), EdgeType::LeafB1.as_u8());
            e.set_aux_bytes(aux);
            e
        }
        Some(EdgeTag::Structural(t)) if t.is_leaf() => {
            let kb = t.leaf_key_bytes().expect("leaf tag");
            let pop = (edge.pop0(kb) + 1) as usize;
            let size = crate::leaf::size_set(kb, pop);
            let dst = a.alloc_bytes(size);
            // SAFETY: source leaf holds `size` payload bytes; dst is fresh and
            // class-sized identically.
            unsafe {
                core::ptr::copy_nonoverlapping(edge.node_ptr(), dst.as_ptr(), size);
            }
            let mut e = Edge::new_node(dst.as_ptr(), t.as_u8());
            e.set_aux_bytes(aux);
            e
        }
        Some(EdgeTag::Structural(t)) if t.is_branch() => {
            // SAFETY: live branch of type `t`.
            let bl = unsafe { mutate::branch_form_level(edge, t, level) };
            // SAFETY: forwarded contract.
            let e = unsafe { clone_branch(a, edge, t, bl) };
            let mut e = e;
            e.set_aux_bytes(aux);
            e
        }
        _ => Edge::NULL,
    }
}

/// Collects every key of the subtree at `edge` (covering `level` bytes) into
/// `out`, offset by `base` (the digits already decoded above `level`). Used to
/// re-canonicalize a small structural result through [`build_subtree`], so it
/// is only ever called on subtrees of at most a linear-leaf's worth of keys.
///
/// # Safety
///
/// `edge` references a live, well-formed subtree at `level`.
unsafe fn collect_subtree_keys(edge: &Edge, level: u8, base: u64, out: &mut Vec<u64>) {
    // SAFETY: forwarded contract; branch children recurse one level down.
    unsafe {
        let mut buf = [0u64; 256];
        if let Some(n) = terminal_remainders_buf(edge, level, &mut buf) {
            for &k in &buf[..n] {
                out.push(base | k);
            }
            return;
        }
        match edge.tag() {
            Some(EdgeTag::Structural(EdgeType::FullExpanse)) => {
                let span = full_count(level);
                for k in 0..span {
                    out.push(base | k);
                }
            }
            Some(EdgeTag::Structural(t)) if t.is_branch() => {
                let acc = branch_accessor(edge, level);
                let bm = acc.populated_bitmap();
                for w in 0..4 {
                    let mut word = bm.words[w];
                    while word != 0 {
                        let bit = word.trailing_zeros();
                        word &= word - 1;
                        let d = ((w as u32 * 64) + bit) as u8;
                        let child = acc.child(edge, d);
                        if !child.is_null() {
                            let child_base = base | (u64::from(d) << (8 * u32::from(level - 1)));
                            collect_subtree_keys(&child, level - 1, child_base, out);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Clones an entire branch subtree.
///
/// # Safety
///
/// `edge` references a live branch of type `t` at form-level `bl`.
unsafe fn clone_branch(a: &NodeAlloc, edge: &Edge, t: EdgeType, bl: u8) -> Edge {
    match t {
        t if t.is_branch_l3() => {
            // SAFETY: live BranchL3.
            let src = unsafe { &*edge.node_ptr().cast::<BranchL3>() };
            let node = a.alloc_node_zeroed::<BranchL3>();
            // SAFETY: fresh node; clone each populated child.
            unsafe {
                (*node.as_ptr()).hdr = src.hdr;
                for i in 0..src.hdr.num as usize {
                    (*node.as_ptr()).edges[i] = clone_subtree(a, &src.edges[i], bl - 1);
                }
            }
            Edge::new_node(node.as_ptr().cast(), t.as_u8())
        }
        EdgeType::BranchL7 => {
            // SAFETY: live BranchL7.
            let src = unsafe { &*edge.node_ptr().cast::<BranchL7>() };
            let node = a.alloc_node_zeroed::<BranchL7>();
            // SAFETY: fresh node; clone each populated child.
            unsafe {
                (*node.as_ptr()).hdr = src.hdr;
                for i in 0..src.hdr.num as usize {
                    (*node.as_ptr()).edges[i] = clone_subtree(a, &src.edges[i], bl - 1);
                }
            }
            Edge::new_node(node.as_ptr().cast(), t.as_u8())
        }
        EdgeType::BranchB => {
            // SAFETY: live BranchB.
            let src = unsafe { &*edge.node_ptr().cast::<BranchB>() };
            let node = a.alloc_node_zeroed::<BranchB>();
            // SAFETY: fresh node; clone each subarray and its children.
            unsafe {
                let b = &mut *node.as_ptr();
                b.level = src.level;
                b.bitmap = src.bitmap;
                b.pop_counts = src.pop_counts;
                for sub in 0..8 {
                    let ncnt = src.pop_counts[sub] as usize;
                    if ncnt > 0 {
                        let arr = a.alloc_bytes(mutate::sub_edges_size(ncnt)).cast::<Edge>();
                        for j in 0..ncnt {
                            let child = clone_subtree(a, &*src.subarrays[sub].add(j), bl - 1);
                            arr.as_ptr().add(j).write(child);
                        }
                        b.subarrays[sub] = arr.as_ptr();
                    }
                }
            }
            Edge::new_node(node.as_ptr().cast(), t.as_u8())
        }
        EdgeType::BranchU => {
            // SAFETY: live BranchU.
            let src = unsafe { &*edge.node_ptr().cast::<BranchU>() };
            let node = a.alloc_node_zeroed::<BranchU>();
            // SAFETY: fresh node; clone each non-null child.
            unsafe {
                let b = &mut *node.as_ptr();
                for d in 0..256usize {
                    if !src.edges[d].is_null() {
                        b.edges[d] = clone_subtree(a, &src.edges[d], bl - 1);
                    }
                }
            }
            Edge::new_node(node.as_ptr().cast(), t.as_u8())
        }
        _ => Edge::NULL,
    }
}

// ===========================================================================
// Direct-emission materialization (issue #348). Instead of merging the two
// ordered iterators and re-inserting every surviving key, the result tree is
// emitted from the same lockstep walk that computes the cardinality:
//   * full expanses resolve structurally (clone / full / complement), never
//     key by key;
//   * aligned final-byte bitmaps combine word-parallel and emit one leaf;
//   * branch pairs recurse per digit and assemble the parent bottom-up;
//   * small results are re-canonicalized through `build_subtree`, so every
//     emitted tree is a valid, insert-equivalent shape (validator + fuzz).
// ===========================================================================

/// Total population of a root trie (a level-8 branch): the sum of its present
/// children's populations, `O(fanout)` rather than `O(keys)`.
///
/// # Safety
///
/// `top` references a live level-8 branch.
pub(crate) unsafe fn tree_pop(top: &Edge) -> u64 {
    match top.tag() {
        Some(EdgeTag::Structural(t)) if t.is_branch_l3() => {
            // SAFETY: live BranchL3 pointer.
            let b = unsafe { &*top.node_ptr().cast::<BranchL3>() };
            let mut pop = 0u64;
            for i in 0..b.hdr.num as usize {
                // SAFETY: child is a live subtree at level 7.
                pop += unsafe { subtree_count(&b.edges[i], 7) };
            }
            pop
        }
        Some(EdgeTag::Structural(EdgeType::BranchL7)) => {
            // SAFETY: live BranchL7 pointer.
            let b = unsafe { &*top.node_ptr().cast::<BranchL7>() };
            let mut pop = 0u64;
            for i in 0..b.hdr.num as usize {
                // SAFETY: child is a live subtree at level 7.
                pop += unsafe { subtree_count(&b.edges[i], 7) };
            }
            pop
        }
        Some(EdgeTag::Structural(EdgeType::BranchB)) => {
            // SAFETY: live BranchB pointer.
            let b = unsafe { &*top.node_ptr().cast::<BranchB>() };
            let mut pop = 0u64;
            for sub in 0..8 {
                let ncnt = b.pop_counts[sub] as usize;
                for j in 0..ncnt {
                    // SAFETY: child is a live subtree at level 7.
                    pop += unsafe { subtree_count(&*b.subarrays[sub].add(j), 7) };
                }
            }
            pop
        }
        Some(EdgeTag::Structural(EdgeType::BranchU)) => {
            // SAFETY: live BranchU pointer.
            let b = unsafe { &*top.node_ptr().cast::<BranchU>() };
            let mut pop = 0u64;
            for d in 0..256 {
                if !b.edges[d].is_null() {
                    // SAFETY: child is a live subtree at level 7.
                    pop += unsafe { subtree_count(&b.edges[d], 7) };
                }
            }
            pop
        }
        // SAFETY: non-branch form covering level 8.
        _ => unsafe { subtree_count(top, 8) },
    }
}

/// A materializing set operation.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Op {
    /// `A ∩ B`.
    And,
    /// `A ∪ B`.
    Or,
    /// `A \ B` (asymmetric: keys of the first operand not in the second).
    Diff,
    /// `A △ B`.
    Xor,
}

/// Above this result population a small subtree is emitted structurally; at or
/// below it the (few) surviving keys are re-canonicalized through
/// [`build_subtree`] into a flat immediate/leaf — the insert-equivalent shape.
const MATERIALIZE_SMALL: u64 = mutate::LEAF_CAP as u64;

/// Frees a temporary subtree built during a structural op that is then
/// discarded in favor of a re-canonicalized rebuild.
///
/// # Safety
///
/// `edge` owns a live subtree allocated in `a`, no longer referenced elsewhere.
#[inline]
unsafe fn free_temp(a: &NodeAlloc, mut edge: Edge) {
    // SAFETY: forwarded contract; set-flavor subtree.
    unsafe { mutate::free_subtree::<false>(a, &mut edge) };
}

/// Wraps a single surviving child as the canonical edge for "only sub-expanse
/// `d0` of this `level`-expanse is populated, by `child`". Leaves/bitmaps/
/// branches absorb `d0` as an extra decode digit (a narrow pointer); immediates
/// and full expanses — which cannot skip — sit under a one-child branch.
///
/// # Safety
///
/// `child` is a live canonical subtree at `level - 1` with population
/// `child_pop`; `a` owns it.
unsafe fn wrap_single_child(a: &NodeAlloc, level: u8, d0: u8, child: Edge, child_pop: u64) -> Edge {
    match child.tag() {
        // Immediates and full expanses cannot carry decode; an uncompressed
        // branch has no header level to skip from — all sit under a one-child
        // branch (valid, as `remove` keeps single-child linear branches).
        Some(EdgeTag::Immed(_))
        | Some(EdgeTag::Structural(EdgeType::FullExpanse | EdgeType::BranchU)) => {
            // SAFETY: single live child at `level - 1`.
            let mut e = unsafe { build_branch(a, level, &[d0], &[child]) };
            if level <= 7 {
                e.set_pop0(level, child_pop - 1);
            }
            e
        }
        _ => {
            // The child's `pop0` sits at its own form level (unchanged); adding
            // the `level` decode digit re-hosts it one level up as a skip.
            let mut e = child;
            e.set_decode_bytes(level - 1, &[d0]);
            e
        }
    }
}

/// Assembles the result of a branch × branch op at `level` from surviving
/// `(digit, child)` pairs (each child a canonical subtree at `level - 1`).
/// Empty → `None`; a tiny total is rebuilt flat; a lone child becomes a skip;
/// otherwise a fresh branch. Frees the temporary children it discards.
///
/// # Safety
///
/// Every child is a live subtree at `level - 1` owned by `a`.
unsafe fn assemble(a: &NodeAlloc, level: u8, digits: &[u8], children: &[Edge]) -> Option<Edge> {
    let m = digits.len();
    if m == 0 {
        return None;
    }
    // SAFETY: each child is a live subtree at `level - 1`.
    let total: u64 = children
        .iter()
        .map(|c| unsafe { subtree_count(c, level - 1) })
        .sum();
    if total == 0 {
        for &c in children {
            // SAFETY: discarding a live (empty) temporary subtree.
            unsafe { free_temp(a, c) };
        }
        return None;
    }
    if total <= MATERIALIZE_SMALL {
        // Re-canonicalize: enumerate the few keys and rebuild flat, freeing the
        // temporary children.
        let mut keys = Vec::with_capacity(total as usize);
        for (i, &c) in children.iter().enumerate() {
            let base = u64::from(digits[i]) << (8 * u32::from(level - 1));
            // SAFETY: live small child at `level - 1`.
            unsafe { collect_subtree_keys(&c, level - 1, base, &mut keys) };
        }
        for &c in children {
            // SAFETY: temporary child, now superseded by the rebuild.
            unsafe { free_temp(a, c) };
        }
        keys.sort_unstable();
        // SAFETY: keys share digits above `level`; `a` owns the rebuild.
        return Some(unsafe { build_subtree(a, &keys, level) });
    }
    if m == 1 && level < 8 {
        // A lone child collapses to a narrow pointer — except at level 8, which
        // cannot skip and keeps the (single-child) branch.
        // SAFETY: single live child at `level - 1`.
        return Some(unsafe { wrap_single_child(a, level, digits[0], children[0], total) });
    }
    // SAFETY: ≥ 1 live children at `level - 1` (single child only at level 8).
    let mut e = unsafe { build_branch(a, level, digits, children) };
    if level <= 7 {
        e.set_pop0(level, total - 1);
    }
    Some(e)
}

///// Read accessor over a live branch node, caching the form and skipping status
/// to avoid repeated dispatch and tag decoding during digit-by-digit materialization.
enum BranchAccessor<'e> {
    U(&'e BranchU),
    B {
        b: &'e BranchB,
        skipping_pd: Option<u8>,
    },
    L3 {
        b: &'e BranchL3,
        skipping_pd: Option<u8>,
    },
    L7 {
        b: &'e BranchL7,
        skipping_pd: Option<u8>,
    },
}

impl<'e> BranchAccessor<'e> {
    /// Extracts the 256-bit set of populated digits for this branch edge.
    #[inline(always)]
    fn populated_bitmap(&self) -> Bitmap256 {
        match self {
            BranchAccessor::U(u) => {
                let mut bm = Bitmap256::new();
                for d in 0..256 {
                    if !u.edges[d].is_null() {
                        bm.set(d as u8);
                    }
                }
                bm
            }
            BranchAccessor::B { b, skipping_pd } => {
                if let Some(pd) = skipping_pd {
                    let mut bm = Bitmap256::new();
                    bm.set(*pd);
                    bm
                } else {
                    b.bitmap
                }
            }
            BranchAccessor::L3 { b, skipping_pd } => {
                let mut bm = Bitmap256::new();
                if let Some(pd) = skipping_pd {
                    bm.set(*pd);
                } else {
                    for &d in &b.hdr.digits[..b.hdr.num as usize] {
                        bm.set(d);
                    }
                }
                bm
            }
            BranchAccessor::L7 { b, skipping_pd } => {
                let mut bm = Bitmap256::new();
                if let Some(pd) = skipping_pd {
                    bm.set(*pd);
                } else {
                    for &d in &b.hdr.digits[..b.hdr.num as usize] {
                        bm.set(d);
                    }
                }
                bm
            }
        }
    }

    /// The child edge for decode digit `d` at `level`.
    ///
    /// # Safety
    ///
    /// `edge` is the parent edge whose node this accessor views.
    #[inline(always)]
    unsafe fn child(&self, edge: &Edge, d: u8) -> Edge {
        match self {
            BranchAccessor::U(u) => u.edges[d as usize],
            BranchAccessor::B { b, skipping_pd } => {
                if let Some(pd) = skipping_pd {
                    if d == *pd { *edge } else { Edge::NULL }
                } else {
                    match b.bitmap.test_and_subexpanse_rank_with_sub(d) {
                        // SAFETY: d is set in b.bitmap -> subarrays[sub] is non-null.
                        Some((sub, slot)) => unsafe { *b.subarrays[sub].add(slot) },
                        None => Edge::NULL,
                    }
                }
            }
            BranchAccessor::L3 { b, skipping_pd } => {
                if let Some(pd) = skipping_pd {
                    if d == *pd { *edge } else { Edge::NULL }
                } else {
                    match b.hdr.find(d) {
                        Some(slot) => b.edges[slot],
                        None => Edge::NULL,
                    }
                }
            }
            BranchAccessor::L7 { b, skipping_pd } => {
                if let Some(pd) = skipping_pd {
                    if d == *pd { *edge } else { Edge::NULL }
                } else {
                    match b.hdr.find(d) {
                        Some(slot) => b.edges[slot],
                        None => Edge::NULL,
                    }
                }
            }
        }
    }
}

/// Creates a [`BranchAccessor`] for `edge` covering `level`.
///
/// # Safety
///
/// `edge` references a live branch node.
#[inline(always)]
unsafe fn branch_accessor<'e>(edge: &'e Edge, level: u8) -> BranchAccessor<'e> {
    match edge.tag() {
        Some(EdgeTag::Structural(EdgeType::BranchU)) => {
            // SAFETY: live BranchU pointer (BranchU never skips).
            BranchAccessor::U(unsafe { &*edge.node_ptr().cast::<BranchU>() })
        }
        Some(EdgeTag::Structural(EdgeType::BranchB)) => {
            // SAFETY: live BranchB pointer.
            let b = unsafe { &*edge.node_ptr().cast::<BranchB>() };
            let skipping_pd = if b.level < level {
                Some(edge.decode_bytes(b.level)[(level - b.level - 1) as usize])
            } else {
                None
            };
            BranchAccessor::B { b, skipping_pd }
        }
        Some(EdgeTag::Structural(t)) if t.is_branch_l3() => {
            // SAFETY: live BranchL3 pointer.
            let b = unsafe { &*edge.node_ptr().cast::<BranchL3>() };
            let skipping_pd = if b.hdr.level < level {
                Some(edge.decode_bytes(b.hdr.level)[(level - b.hdr.level - 1) as usize])
            } else {
                None
            };
            BranchAccessor::L3 { b, skipping_pd }
        }
        Some(EdgeTag::Structural(EdgeType::BranchL7)) => {
            // SAFETY: live BranchL7 pointer.
            let b = unsafe { &*edge.node_ptr().cast::<BranchL7>() };
            let skipping_pd = if b.hdr.level < level {
                Some(edge.decode_bytes(b.hdr.level)[(level - b.hdr.level - 1) as usize])
            } else {
                None
            };
            BranchAccessor::L7 { b, skipping_pd }
        }
        _ => unreachable!("branch_accessor called on non-branch edge"),
    }
}

/// Specialized fast merge when both branches are non-skipping linear branches.
#[inline]
unsafe fn materialize_linear_pair(
    a: &NodeAlloc,
    da_slice: &[u8],
    ea_slice: &[Edge],
    db_slice: &[u8],
    eb_slice: &[Edge],
    level: u8,
    op: Op,
) -> Option<Edge> {
    let mut digits: [MaybeUninit<u8>; 14] = [MaybeUninit::uninit(); 14];
    let mut children: [MaybeUninit<Edge>; 14] = [MaybeUninit::uninit(); 14];
    let mut count = 0usize;

    let (mut ia, mut ib) = (0usize, 0usize);
    let (na, nb) = (da_slice.len(), db_slice.len());

    match op {
        Op::And => {
            while ia < na && ib < nb {
                let da = da_slice[ia];
                let db = db_slice[ib];
                match da.cmp(&db) {
                    core::cmp::Ordering::Equal => {
                        let child_opt =
                            // SAFETY: live children at level - 1.
                            unsafe { materialize(a, &ea_slice[ia], &eb_slice[ib], level - 1, Op::And) };
                        if let Some(child) = child_opt {
                            digits[count].write(da);
                            children[count].write(child);
                            count += 1;
                        }
                        ia += 1;
                        ib += 1;
                    }
                    core::cmp::Ordering::Less => ia += 1,
                    core::cmp::Ordering::Greater => ib += 1,
                }
            }
        }
        Op::Diff => {
            while ia < na {
                let da = da_slice[ia];
                while ib < nb && db_slice[ib] < da {
                    ib += 1;
                }
                if ib < nb && db_slice[ib] == da {
                    let child_opt =
                        // SAFETY: live children at level - 1.
                        unsafe { materialize(a, &ea_slice[ia], &eb_slice[ib], level - 1, Op::Diff) };
                    if let Some(child) = child_opt {
                        digits[count].write(da);
                        children[count].write(child);
                        count += 1;
                    }
                    ia += 1;
                    ib += 1;
                } else {
                    // Present in A, absent in B -> clone child A.
                    // SAFETY: live child at level - 1.
                    let child = unsafe { clone_subtree(a, &ea_slice[ia], level - 1) };
                    digits[count].write(da);
                    children[count].write(child);
                    count += 1;
                    ia += 1;
                }
            }
        }
        Op::Or | Op::Xor => {
            while ia < na || ib < nb {
                let (take_a, take_b, d) = if ia == na {
                    let d = db_slice[ib];
                    ib += 1;
                    (false, true, d)
                } else if ib == nb {
                    let d = da_slice[ia];
                    ia += 1;
                    (true, false, d)
                } else {
                    let da = da_slice[ia];
                    let db = db_slice[ib];
                    match da.cmp(&db) {
                        core::cmp::Ordering::Less => {
                            ia += 1;
                            (true, false, da)
                        }
                        core::cmp::Ordering::Greater => {
                            ib += 1;
                            (false, true, db)
                        }
                        core::cmp::Ordering::Equal => {
                            ia += 1;
                            ib += 1;
                            (true, true, da)
                        }
                    }
                };
                let child_opt = match (take_a, take_b) {
                    (true, true) => {
                        // SAFETY: live children at level - 1.
                        unsafe {
                            materialize(a, &ea_slice[ia - 1], &eb_slice[ib - 1], level - 1, op)
                        }
                    }
                    (true, false) => {
                        // SAFETY: live child at level - 1.
                        Some(unsafe { clone_subtree(a, &ea_slice[ia - 1], level - 1) })
                    }
                    (false, true) => {
                        // SAFETY: live child at level - 1.
                        Some(unsafe { clone_subtree(a, &eb_slice[ib - 1], level - 1) })
                    }
                    (false, false) => unreachable!(),
                };
                if let Some(child) = child_opt {
                    digits[count].write(d);
                    children[count].write(child);
                    count += 1;
                }
            }
        }
    }

    // SAFETY: first `count` entries in digits and children were written.
    let digits_slice: &[u8] =
        unsafe { core::slice::from_raw_parts(digits.as_ptr().cast::<u8>(), count) };
    // SAFETY: first `count` entries in digits and children were written.
    let children_slice: &[Edge] =
        unsafe { core::slice::from_raw_parts(children.as_ptr().cast::<Edge>(), count) };
    // SAFETY: children are live subtrees at level - 1.
    unsafe { assemble(a, level, digits_slice, children_slice) }
}

/// Structural branch × branch materialization: recurse each digit present on
/// either side and assemble the survivors.
///
/// # Safety
///
/// `ea`, `eb` are live branches covering the same `level`-expanse; `a` owns
/// the result.
unsafe fn materialize_branch(
    a: &NodeAlloc,
    ea: &Edge,
    eb: &Edge,
    level: u8,
    op: Op,
) -> Option<Edge> {
    // SAFETY: ea and eb are live branches per function contract.
    let acc_a = unsafe { branch_accessor(ea, level) };
    // SAFETY: eb is a live branch per function contract.
    let acc_b = unsafe { branch_accessor(eb, level) };

    // Fast path: both non-skipping linear branches.
    let l_a = match acc_a {
        BranchAccessor::L3 {
            b,
            skipping_pd: None,
        } => Some((
            &b.hdr.digits[..b.hdr.num as usize],
            &b.edges[..b.hdr.num as usize],
        )),
        BranchAccessor::L7 {
            b,
            skipping_pd: None,
        } => Some((
            &b.hdr.digits[..b.hdr.num as usize],
            &b.edges[..b.hdr.num as usize],
        )),
        _ => None,
    };
    let l_b = match acc_b {
        BranchAccessor::L3 {
            b,
            skipping_pd: None,
        } => Some((
            &b.hdr.digits[..b.hdr.num as usize],
            &b.edges[..b.hdr.num as usize],
        )),
        BranchAccessor::L7 {
            b,
            skipping_pd: None,
        } => Some((
            &b.hdr.digits[..b.hdr.num as usize],
            &b.edges[..b.hdr.num as usize],
        )),
        _ => None,
    };
    if let (Some((da, ea_slice)), Some((db, eb_slice))) = (l_a, l_b) {
        // SAFETY: both non-skipping linear branches covering level.
        return unsafe { materialize_linear_pair(a, da, ea_slice, db, eb_slice, level, op) };
    }

    // Fast path for AND: linear branch vs any other branch.
    if op == Op::And {
        if let Some((da_slice, ea_slice)) = l_a {
            let mut digits: [MaybeUninit<u8>; 7] = [MaybeUninit::uninit(); 7];
            let mut children: [MaybeUninit<Edge>; 7] = [MaybeUninit::uninit(); 7];
            let mut count = 0usize;
            for (i, &da) in da_slice.iter().enumerate() {
                // SAFETY: acc_b views live eb.
                let cb = unsafe { acc_b.child(eb, da) };
                if !cb.is_null() {
                    let child_opt =
                        // SAFETY: live subtrees at level - 1.
                        unsafe { materialize(a, &ea_slice[i], &cb, level - 1, Op::And) };
                    if let Some(child) = child_opt {
                        digits[count].write(da);
                        children[count].write(child);
                        count += 1;
                    }
                }
            }
            // SAFETY: first `count` entries were initialized.
            let digits_slice: &[u8] =
                unsafe { core::slice::from_raw_parts(digits.as_ptr().cast::<u8>(), count) };
            // SAFETY: first `count` entries were initialized.
            let children_slice: &[Edge] =
                unsafe { core::slice::from_raw_parts(children.as_ptr().cast::<Edge>(), count) };
            // SAFETY: children are live subtrees at level - 1.
            return unsafe { assemble(a, level, digits_slice, children_slice) };
        }
        if let Some((db_slice, eb_slice)) = l_b {
            let mut digits: [MaybeUninit<u8>; 7] = [MaybeUninit::uninit(); 7];
            let mut children: [MaybeUninit<Edge>; 7] = [MaybeUninit::uninit(); 7];
            let mut count = 0usize;
            for (i, &db) in db_slice.iter().enumerate() {
                // SAFETY: acc_a views live ea.
                let ca = unsafe { acc_a.child(ea, db) };
                if !ca.is_null() {
                    let child_opt =
                        // SAFETY: live subtrees at level - 1.
                        unsafe { materialize(a, &ca, &eb_slice[i], level - 1, Op::And) };
                    if let Some(child) = child_opt {
                        digits[count].write(db);
                        children[count].write(child);
                        count += 1;
                    }
                }
            }
            // SAFETY: first `count` entries were initialized.
            let digits_slice: &[u8] =
                unsafe { core::slice::from_raw_parts(digits.as_ptr().cast::<u8>(), count) };
            // SAFETY: first `count` entries were initialized.
            let children_slice: &[Edge] =
                unsafe { core::slice::from_raw_parts(children.as_ptr().cast::<Edge>(), count) };
            // SAFETY: children are live subtrees at level - 1.
            return unsafe { assemble(a, level, digits_slice, children_slice) };
        }
    }

    let bm_a = acc_a.populated_bitmap();
    let bm_b = acc_b.populated_bitmap();

    let mut digits: [MaybeUninit<u8>; 256] = [MaybeUninit::uninit(); 256];
    let mut children: [MaybeUninit<Edge>; 256] = [MaybeUninit::uninit(); 256];
    let mut count = 0usize;

    for w in 0..4 {
        let wa = bm_a.words[w];
        let wb = bm_b.words[w];
        let mut active_w = match op {
            Op::And => wa & wb,
            Op::Diff => wa,
            Op::Or | Op::Xor => wa | wb,
        };
        while active_w != 0 {
            let bit = active_w.trailing_zeros();
            active_w &= active_w - 1;
            let d = ((w as u32 * 64) + bit) as u8;
            let in_a = (wa & (1u64 << bit)) != 0;
            let in_b = (wb & (1u64 << bit)) != 0;

            let child_opt: Option<Edge> = match (in_a, in_b) {
                (true, false) => {
                    // SAFETY: acc_a views live ea; child at level - 1.
                    Some(unsafe { clone_subtree(a, &acc_a.child(ea, d), level - 1) })
                }
                (false, true) => match op {
                    Op::Diff => None,
                    Op::Or | Op::Xor => {
                        // SAFETY: acc_b views live eb; child at level - 1.
                        Some(unsafe { clone_subtree(a, &acc_b.child(eb, d), level - 1) })
                    }
                    Op::And => unreachable!(),
                },
                (true, true) => {
                    // SAFETY: acc_a views live ea.
                    let ca = unsafe { acc_a.child(ea, d) };
                    // SAFETY: acc_b views live eb.
                    let cb = unsafe { acc_b.child(eb, d) };
                    // SAFETY: ca, cb are live subtrees at level - 1.
                    unsafe { materialize(a, &ca, &cb, level - 1, op) }
                }
                (false, false) => unreachable!(),
            };
            if let Some(child) = child_opt {
                digits[count].write(d);
                children[count].write(child);
                count += 1;
            }
        }
    }
    // SAFETY: first `count` entries were initialized.
    let digits_slice: &[u8] =
        unsafe { core::slice::from_raw_parts(digits.as_ptr().cast::<u8>(), count) };
    // SAFETY: first `count` entries were initialized.
    let children_slice: &[Edge] =
        unsafe { core::slice::from_raw_parts(children.as_ptr().cast::<Edge>(), count) };
    // SAFETY: children are live subtrees at level - 1.
    unsafe { assemble(a, level, digits_slice, children_slice) }
}

/// Materializes an operation between a branch and a terminal key list.
///
/// Partitions `term_keys` by top-digit at `level`, matching against the branch's
/// populated children in a single top-down pass rather than cloning the branch
/// and performing sequential dynamic tree insertions/removals.
///
/// # Safety
///
/// `branch` is a live branch edge at `level` (`level >= 2`), `term_keys` is a sorted,
/// distinct slice of keys covering the `level`-expanse; `a` owns the result.
unsafe fn materialize_branch_terminal(
    a: &NodeAlloc,
    branch: &Edge,
    term_keys: &[u64],
    level: u8,
    op: Op,
    term_is_a: bool,
) -> Option<Edge> {
    // SAFETY: branch is a live branch covering level per function contract.
    let acc = unsafe { branch_accessor(branch, level) };
    let bm_branch = acc.populated_bitmap();

    let mut term_digits = [0u8; 256];
    let mut term_ranges = [(0usize, 0usize); 256];
    let mut num_groups = 0usize;
    let mut bm_term = Bitmap256::new();

    let mut i = 0usize;
    while i < term_keys.len() {
        let d = digit(term_keys[i], level);
        let start = i;
        while i < term_keys.len() && digit(term_keys[i], level) == d {
            i += 1;
        }
        term_digits[num_groups] = d;
        term_ranges[num_groups] = (start, i);
        bm_term.set(d);
        num_groups += 1;
    }

    let mut digits: [MaybeUninit<u8>; 256] = [MaybeUninit::uninit(); 256];
    let mut children: [MaybeUninit<Edge>; 256] = [MaybeUninit::uninit(); 256];
    let mut count = 0usize;

    for w in 0..4 {
        let wb = bm_branch.words[w];
        let wt = bm_term.words[w];
        let mut active_w = match op {
            Op::Diff if !term_is_a => wb,
            _ => wb | wt,
        };
        while active_w != 0 {
            let bit = active_w.trailing_zeros();
            active_w &= active_w - 1;
            let d = ((w as u32 * 64) + bit) as u8;
            let in_b = (wb & (1u64 << bit)) != 0;
            let in_t = (wt & (1u64 << bit)) != 0;

            let child_opt: Option<Edge> = match (in_b, in_t) {
                (true, false) => {
                    // SAFETY: acc views live branch; child at level - 1.
                    Some(unsafe { clone_subtree(a, &acc.child(branch, d), level - 1) })
                }
                (false, true) => {
                    if op == Op::Diff && !term_is_a {
                        None
                    } else {
                        let g_idx = term_digits[..num_groups]
                            .iter()
                            .position(|&x| x == d)
                            .expect("digit in term_digits");
                        let (start, end) = term_ranges[g_idx];
                        // SAFETY: keys share digits above level - 1; a owns rebuild.
                        Some(unsafe { build_subtree(a, &term_keys[start..end], level - 1) })
                    }
                }
                (true, true) => {
                    let g_idx = term_digits[..num_groups]
                        .iter()
                        .position(|&x| x == d)
                        .expect("digit in term_digits");
                    let (start, end) = term_ranges[g_idx];
                    // SAFETY: acc views live branch.
                    let child_b = unsafe { acc.child(branch, d) };
                    // SAFETY: keys share digits above level - 1; a owns rebuild.
                    let child_t = unsafe { build_subtree(a, &term_keys[start..end], level - 1) };
                    let (ca, cb) = if term_is_a {
                        (&child_t, &child_b)
                    } else {
                        (&child_b, &child_t)
                    };
                    // SAFETY: ca, cb are live subtrees at level - 1; a owns result.
                    let res = unsafe { materialize(a, ca, cb, level - 1, op) };
                    // SAFETY: child_t is a temporary subtree allocated in a.
                    unsafe { free_temp(a, child_t) };
                    res
                }
                (false, false) => unreachable!(),
            };
            if let Some(child) = child_opt {
                digits[count].write(d);
                children[count].write(child);
                count += 1;
            }
        }
    }

    // SAFETY: first `count` entries were initialized.
    let digits_slice: &[u8] =
        unsafe { core::slice::from_raw_parts(digits.as_ptr().cast::<u8>(), count) };
    // SAFETY: first `count` entries were initialized.
    let children_slice: &[Edge] =
        unsafe { core::slice::from_raw_parts(children.as_ptr().cast::<Edge>(), count) };
    // SAFETY: children are live subtrees at level - 1.
    unsafe { assemble(a, level, digits_slice, children_slice) }
}

/// Terminal-driven materialization for the mixed cases (at least one side a
/// terminal, neither a full expanse, not both aligned final-byte bitmaps): a
/// key-list merge when both are terminals, otherwise a form-aware top-down
/// partition merge between the branch and the terminal's key list.
///
/// # Safety
///
/// `ea`, `eb` are live non-null subtrees covering the same `level`-expanse,
/// not both branches; `a` owns the result.
unsafe fn materialize_mixed(
    a: &NodeAlloc,
    ea: &Edge,
    eb: &Edge,
    level: u8,
    op: Op,
) -> Option<Edge> {
    let mut ka_buf = [0u64; 256];
    let mut kb_buf = [0u64; 256];
    // SAFETY: forwarded contract.
    let ra = unsafe { terminal_remainders_buf(ea, level, &mut ka_buf) };
    // SAFETY: forwarded contract.
    let rb = unsafe { terminal_remainders_buf(eb, level, &mut kb_buf) };

    // Both terminals: merge the two small key lists per op and rebuild flat.
    if let (Some(na), Some(nb)) = (ra, rb) {
        let ka = &ka_buf[..na];
        let kb = &kb_buf[..nb];
        let mut out_buf = [0u64; 512];
        let mut out_len = 0usize;
        let (mut i, mut j) = (0usize, 0usize);
        while i < ka.len() || j < kb.len() {
            let take = if i == ka.len() {
                let v = kb[j];
                j += 1;
                (false, true, v)
            } else if j == kb.len() {
                let v = ka[i];
                i += 1;
                (true, false, v)
            } else {
                match ka[i].cmp(&kb[j]) {
                    core::cmp::Ordering::Less => {
                        let v = ka[i];
                        i += 1;
                        (true, false, v)
                    }
                    core::cmp::Ordering::Greater => {
                        let v = kb[j];
                        j += 1;
                        (false, true, v)
                    }
                    core::cmp::Ordering::Equal => {
                        let v = ka[i];
                        i += 1;
                        j += 1;
                        (true, true, v)
                    }
                }
            };
            let (in_a, in_b, v) = take;
            let keep = match op {
                Op::And => in_a && in_b,
                Op::Or => true,
                Op::Diff => in_a && !in_b,
                Op::Xor => in_a != in_b,
            };
            if keep {
                out_buf[out_len] = v;
                out_len += 1;
            }
        }
        if out_len == 0 {
            return None;
        }
        // SAFETY: `out` is sorted/distinct and shares digits above `level`.
        return Some(unsafe { build_subtree(a, &out_buf[..out_len], level) });
    }

    // One terminal, one branch.
    let a_is_term = ra.is_some();
    let (term_keys, branch, term_is_a) = if a_is_term {
        (&ka_buf[..ra.unwrap()], eb, true)
    } else {
        (&kb_buf[..rb.unwrap()], ea, false)
    };

    match op {
        Op::And => {
            // Result ⊆ the terminal's keys: probe each into the branch.
            let mut out_buf = [0u64; 256];
            let mut out_len = 0;
            for &k in term_keys {
                // SAFETY: branch is a live subtree at `level`.
                if unsafe { get::test_set(branch, k, level) } {
                    out_buf[out_len] = k;
                    out_len += 1;
                }
            }
            if out_len == 0 {
                return None;
            }
            // SAFETY: sorted/distinct, sharing digits above `level`.
            Some(unsafe { build_subtree(a, &out_buf[..out_len], level) })
        }
        Op::Diff if term_is_a => {
            // terminal \ branch: keep terminal keys absent from the branch.
            let mut out_buf = [0u64; 256];
            let mut out_len = 0;
            for &k in term_keys {
                // SAFETY: branch live at `level`.
                if !unsafe { get::test_set(branch, k, level) } {
                    out_buf[out_len] = k;
                    out_len += 1;
                }
            }
            if out_len == 0 {
                return None;
            }
            // SAFETY: sorted/distinct, sharing digits above `level`.
            Some(unsafe { build_subtree(a, &out_buf[..out_len], level) })
        }
        _ => {
            // Op::Or, Op::Xor, or Op::Diff (branch \ terminal).
            // SAFETY: branch is a live branch at `level`, term_keys are sorted distinct keys in level expanse.
            unsafe { materialize_branch_terminal(a, branch, term_keys, level, op, term_is_a) }
        }
    }
}

/// Complement of `edge` within a `level`-expanse (`FullExpanse \ edge`): every
/// key of the expanse not in `edge`. Structural — never enumerates keys — by
/// complementing per digit (an absent child becomes a full sub-expanse). The
/// result of complementing a sparse subtree is legitimately dense.
///
/// # Safety
///
/// `edge` is a live-or-null subtree at `level`; `a` owns the result.
unsafe fn complement(a: &NodeAlloc, edge: &Edge, level: u8) -> Option<Edge> {
    if edge.is_null() {
        return Some(full_edge(level));
    }
    if edge.tag_byte() == EdgeType::FullExpanse as u8 {
        return None;
    }
    // Today `complement` is only ever entered at level 1: nothing constructs a
    // level ≥ 2 `FullExpanse` (insert and the builder promote to full only at
    // level 1, and materialize never produces a level ≥ 2 full from level-1-only
    // inputs), so `materialize`'s full arms fire only at level 1. The `level > 1`
    // branch below is dead-but-correct — it becomes live if an upward full
    // canonicalization or the level-2 bitmap leaf lands.
    debug_assert!(
        level == 1,
        "level>=2 FullExpanse is currently unreachable; complement's level>1 branch is dead"
    );
    if level == 1 {
        // SAFETY: at level 1 every non-full form reduces to a final-byte bitmap.
        let (_dv, bm) = unsafe { as_level1_bitmap(edge, 1) }.expect("level-1 bitmap");
        let comp = Bitmap256::full().xor(&bm);
        // SAFETY: `a` owns the result.
        return unsafe { build_leaf_from_bitmap(a, &comp, 1, 0) };
    }
    // level > 1: complement each of the 256 sub-expanses.
    let mut digits = [0u8; 256];
    let mut children = [Edge::NULL; 256];
    let mut count = 0usize;
    for d in 0..256u32 {
        let d = d as u8;
        // SAFETY: child covering top-digit d of `edge`'s expanse.
        let child = unsafe { child_at_top(edge, level, d) };
        // SAFETY: child is a live-or-null subtree at `level - 1`.
        if let Some(c) = unsafe { complement(a, &child, level - 1) } {
            digits[count] = d;
            children[count] = c;
            count += 1;
        }
    }
    // SAFETY: children are live subtrees at `level - 1`.
    unsafe { assemble(a, level, &digits[..count], &children[..count]) }
}

/// The child subtree covering top-digit `d` of `edge`'s `level`-expanse,
/// treating a terminal reached through a skip as living entirely under its one
/// shared top digit. Used by [`complement`], which must descend non-branch
/// forms too.
///
/// # Safety
///
/// `edge` is a live non-null, non-full subtree at `level` (`level >= 2`).
unsafe fn child_at_top(edge: &Edge, level: u8, d: u8) -> Edge {
    let tag = edge.tag_byte();
    if is_branch(tag) {
        // SAFETY: live branch at `level`.
        return unsafe { child_by_digit(edge, level, d) };
    }
    // A terminal reached via a skip shares one top digit (its decode byte for
    // `level`, stored at `aux[level - 1]` = `decode_bytes(level - 1)[0]`); it
    // covers digit d only when d equals that byte, viewed one level shallower.
    // (`decode_bytes(level)` would be `aux[level..]`, empty at level 7, and
    // would disagree with the `clear_aux_byte(level - 1)` below.)
    let shared = edge.decode_bytes(level - 1)[0];
    if d == shared {
        // Re-host the same terminal one level down by dropping this decode byte.
        let mut c = *edge;
        c.clear_aux_byte(level as usize - 1);
        c
    } else {
        Edge::NULL
    }
}

/// Direct-emission materialization of `op` over two subtrees covering the same
/// `level`-expanse. Returns the result subtree, or `None` when empty.
///
/// # Safety
///
/// `ea`, `eb` are live-or-null, well-formed subtrees at `level`; every
/// pointer-tagged edge references a live node of its type; `a` owns the result.
pub(crate) unsafe fn materialize(
    a: &NodeAlloc,
    ea: &Edge,
    eb: &Edge,
    level: u8,
    op: Op,
) -> Option<Edge> {
    let na = ea.is_null();
    let nb = eb.is_null();
    if na && nb {
        return None;
    }
    if na {
        // Only B present.
        return match op {
            Op::And | Op::Diff => None,
            // SAFETY: eb live at `level`.
            Op::Or | Op::Xor => Some(unsafe { clone_subtree(a, eb, level) }),
        };
    }
    if nb {
        // Only A present.
        return match op {
            Op::And => None,
            // SAFETY: ea live at `level`.
            Op::Or | Op::Xor | Op::Diff => Some(unsafe { clone_subtree(a, ea, level) }),
        };
    }

    // A `FullExpanse` operand only ever occurs at level 1 today (nothing
    // constructs a level ≥ 2 full — see `complement`), so this block fires at
    // level 1 in practice; the arms are written for any `level` so they stay
    // correct (dead-but-plausible above level 1) if that ever changes.
    let fa = ea.tag_byte() == EdgeType::FullExpanse as u8;
    let fb = eb.tag_byte() == EdgeType::FullExpanse as u8;
    if fa || fb {
        return match (fa, fb, op) {
            // full ∩ full, full ∪ full → full; full \ full, full △ full → empty.
            (true, true, Op::And | Op::Or) => Some(full_edge(level)),
            (true, true, Op::Diff | Op::Xor) => None,
            // full on the left.
            // SAFETY: eb live at `level`.
            (true, false, Op::And) => Some(unsafe { clone_subtree(a, eb, level) }),
            (true, false, Op::Or) => Some(full_edge(level)),
            // SAFETY: eb live at `level`.
            (true, false, Op::Diff | Op::Xor) => unsafe { complement(a, eb, level) },
            // full on the right.
            // SAFETY: ea live at `level`.
            (false, true, Op::And) => Some(unsafe { clone_subtree(a, ea, level) }),
            (false, true, Op::Or) => Some(full_edge(level)),
            (false, true, Op::Diff) => None,
            // SAFETY: ea live at `level`.
            (false, true, Op::Xor) => unsafe { complement(a, ea, level) },
            (false, false, _) => unreachable!("fa || fb"),
        };
    }

    // Both reduce to an aligned final-byte bitmap: combine word-parallel.
    // SAFETY: ea is live at `level`.
    let la = unsafe { as_level1_bitmap(ea, level) };
    // SAFETY: eb is live at `level`.
    let lb = unsafe { as_level1_bitmap(eb, level) };
    if let (Some((da, ba)), Some((db, bb))) = (la, lb)
        && da == db
    {
        let res = match op {
            Op::And => ba.and(&bb),
            Op::Or => ba.or(&bb),
            Op::Diff => ba.andnot(&bb),
            Op::Xor => ba.xor(&bb),
        };
        // SAFETY: `a` owns the result; `da` is the shared skip.
        return unsafe { build_leaf_from_bitmap(a, &res, level, da) };
    }
    // Disjoint final-byte clusters at different skip positions: fall through
    // to the terminal merge (both are terminals).

    let both_branches = is_branch(ea.tag_byte()) && is_branch(eb.tag_byte());
    if both_branches {
        // SAFETY: both live branches at `level`.
        return unsafe { materialize_branch(a, ea, eb, level, op) };
    }
    // SAFETY: at least one terminal; both live at `level`.
    unsafe { materialize_mixed(a, ea, eb, level, op) }
}
