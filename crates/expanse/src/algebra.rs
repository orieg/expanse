//! Native set-algebra kernels over the trie structure (`docs/ALGORITHMS.md`,
//! "Set-algebra kernels").
//!
//! `ExpanseSet` had no set-algebra kernel: an engine using it as a
//! posting-list backend had to compose `AND`/`OR`/`AND-NOT`/`XOR` from the
//! navigation primitives element by element, losing every Boolean cell to a
//! word-parallel container (issue #339, benchmarked in #337). This module
//! adds the missing kernel: a **cardinality** computed by descending both
//! tries in lockstep by expanse — skipping whole subtrees absent on one side
//! and counting whole subtrees present on only one side in `O(1)` from the
//! parent edge's `pop0`, and `AND`-ing bitmap leaves word-parallel with
//! `popcnt` (`Bitmap256::count_and`) instead of iterating up to 256 elements
//! per leaf.
//!
//! Only the intersection cardinality is computed structurally; the other
//! three cardinalities derive from it and the two populations (both `O(1)`):
//!
//! ```text
//! |A ∪ B| = |A| + |B| − |A ∩ B|
//! |A \ B| = |A|       − |A ∩ B|
//! |A △ B| = |A| + |B| − 2·|A ∩ B|
//! ```
//!
//! so `intersection_len` is the single load-bearing structural walk. The
//! result-materializing ops (`intersection`/`union`/… returning a new set)
//! are built in `set.rs` from an ordered merge of the two iterators; the
//! measured Boolean gate (#337 Pillar 1) is cardinality-only, which this
//! kernel serves directly with no allocation.
//!
//! Clean-room note (`AGENTS.md` §2): the kernel operates only on the
//! existing `Edge`/branch/leaf geometry — no new node form, no fat slot.

use crate::bits::Bitmap256;
use crate::get;
use crate::mutate;
use crate::node::{BranchB, BranchL3, BranchL7, BranchU, Edge, LeafBitmap1};
use crate::types::{EdgeTag, EdgeType};

extern crate alloc;
use alloc::vec::Vec;

/// True when `tag` names one of the four branch flavors.
#[inline(always)]
fn is_branch(tag: u8) -> bool {
    matches!(EdgeType::from_u8(tag), Some(e) if e.is_branch())
}

/// Population of the subtree rooted at `edge` (covering `level` undecoded
/// bytes), in `O(1)` — the parent-stored `pop0` for pointer forms, the key
/// count for immediates, `256^level` for a full expanse. `level` must be
/// `1..=7` (the root's population is tracked by the container, not an edge).
///
/// # Safety
///
/// `edge` must reference a live, well-formed node/leaf of its tagged type.
#[inline]
unsafe fn subtree_count(edge: &Edge, level: u8) -> u64 {
    match edge.tag() {
        Some(EdgeTag::Structural(EdgeType::Null)) => 0,
        Some(EdgeTag::Structural(EdgeType::FullExpanse)) => full_count(level),
        Some(EdgeTag::Structural(EdgeType::LeafB1)) => edge.pop0(1) + 1,
        Some(EdgeTag::Structural(t)) if t.is_leaf() => {
            let kb = t.leaf_key_bytes().expect("leaf tag");
            edge.pop0(kb) + 1
        }
        Some(EdgeTag::Structural(t)) if t.is_branch() => {
            // SAFETY: live branch of type `t` per contract.
            let bl = unsafe { mutate::branch_form_level(edge, t, level) };
            edge.pop0(bl) + 1
        }
        Some(EdgeTag::Immed(im)) => u64::from(im.key_count()),
        _ => 0,
    }
}

/// Number of keys in a full expanse covering `level` undecoded bytes
/// (`256^level`). A full expanse only ever forms at `level <= 7` (a level-8
/// full expanse would be the entire `2^64` universe); the `>= 8` arm is a
/// saturating guard that cannot be reached by a constructible set.
#[inline]
const fn full_count(level: u8) -> u64 {
    if level >= 8 {
        u64::MAX
    } else {
        1u64 << (8 * level as u32)
    }
}

/// If `edge`'s real level is 1 — a bitmap or 1-byte-remainder leaf, possibly
/// reached through a narrow-pointer skip — returns `(skipped_middle_digits,
/// final_byte_bitmap)`. Two such edges at the same slot level intersect over
/// their final byte iff their skipped middle digits agree.
///
/// # Safety
///
/// `edge` must reference a live node/leaf of its tagged type.
#[inline]
unsafe fn as_level1_bitmap(edge: &Edge, level: u8) -> Option<(u64, Bitmap256)> {
    match edge.tag() {
        Some(EdgeTag::Structural(EdgeType::LeafB1)) => {
            let dv = if level > 1 {
                mutate::decode_value(edge, 1, level)
            } else {
                0
            };
            // SAFETY: live LeafBitmap1 node pointer.
            let b = unsafe { &*edge.node_ptr().cast::<LeafBitmap1>() };
            Some((dv, b.bitmap))
        }
        Some(EdgeTag::Structural(EdgeType::Leaf1)) => {
            let dv = if level > 1 {
                mutate::decode_value(edge, 1, level)
            } else {
                0
            };
            let pop = (edge.pop0(1) + 1) as usize;
            let base = edge.node_ptr();
            let mut bm = Bitmap256::new();
            for slot in 0..pop {
                // SAFETY: a Leaf1 holds `pop` packed 1-byte keys.
                bm.set(unsafe { *base.add(slot) });
            }
            Some((dv, bm))
        }
        Some(EdgeTag::Immed(im)) if im.key_bytes() == 1 => {
            // A 1-byte-key immediate sits at slot level 1 (immediates store
            // the whole remainder, so key_bytes == slot level): no skip.
            let keys = mutate::immed_keys(edge, im);
            let mut bm = Bitmap256::new();
            for &k in keys.iter() {
                bm.set(k as u8);
            }
            Some((0, bm))
        }
        _ => None,
    }
}

/// The child edge of a branch `edge` for decode digit `d` at `level`, or the
/// null edge when that subexpanse is empty. A narrow-pointer branch (its node
/// level below `level`) has a single present digit — the skipped decode byte
/// at this level — whose child is the same edge viewed one level shallower.
///
/// # Safety
///
/// `edge` must reference a live branch node of its tagged type.
#[inline]
unsafe fn child_by_digit(edge: &Edge, level: u8, d: u8) -> Edge {
    let t = match edge.tag() {
        Some(EdgeTag::Structural(t)) if t.is_branch() => t,
        _ => return Edge::NULL,
    };
    match t {
        EdgeType::BranchU => {
            // BranchU never skips (no header level).
            // SAFETY: live BranchU pointer.
            let b = unsafe { &*edge.node_ptr().cast::<BranchU>() };
            b.edges[d as usize]
        }
        EdgeType::BranchB => {
            // SAFETY: live BranchB pointer.
            let b = unsafe { &*edge.node_ptr().cast::<BranchB>() };
            let bl = b.level;
            if bl < level {
                let pd = edge.decode_bytes(bl)[(level - bl - 1) as usize];
                return if d == pd { *edge } else { Edge::NULL };
            }
            match b.bitmap.test_and_subexpanse_rank_with_sub(d) {
                // SAFETY: d is set → subarrays[sub] is non-null with slot in range.
                Some((sub, slot)) => unsafe { *b.subarrays[sub].add(slot) },
                None => Edge::NULL,
            }
        }
        EdgeType::BranchL3 => {
            // SAFETY: live BranchL3 pointer.
            let b = unsafe { &*edge.node_ptr().cast::<BranchL3>() };
            let bl = b.hdr.level;
            if bl < level {
                let pd = edge.decode_bytes(bl)[(level - bl - 1) as usize];
                return if d == pd { *edge } else { Edge::NULL };
            }
            match b.hdr.find(d) {
                Some(slot) => b.edges[slot],
                None => Edge::NULL,
            }
        }
        EdgeType::BranchL7 => {
            // SAFETY: live BranchL7 pointer.
            let b = unsafe { &*edge.node_ptr().cast::<BranchL7>() };
            let bl = b.hdr.level;
            if bl < level {
                let pd = edge.decode_bytes(bl)[(level - bl - 1) as usize];
                return if d == pd { *edge } else { Edge::NULL };
            }
            match b.hdr.find(d) {
                Some(slot) => b.edges[slot],
                None => Edge::NULL,
            }
        }
        _ => Edge::NULL,
    }
}

/// The `level`-byte key remainders of a terminal (leaf / immediate) edge, or
/// `None` when `edge` is a branch (which has no cheap flat enumeration). Each
/// remainder carries the edge's skipped decode digits, so it can be probed
/// against a sibling subtree with [`get::test_set`] at the same `level`.
///
/// # Safety
///
/// `edge` must reference a live node/leaf of its tagged type.
#[inline]
unsafe fn terminal_remainders(edge: &Edge, level: u8) -> Option<Vec<u64>> {
    match edge.tag() {
        Some(EdgeTag::Immed(im)) => {
            // Immediates store the whole remainder (key_bytes == slot level).
            let keys = mutate::immed_keys(edge, im);
            Some(keys.to_vec())
        }
        Some(EdgeTag::Structural(EdgeType::LeafB1)) => {
            let dv = if level > 1 {
                mutate::decode_value(edge, 1, level)
            } else {
                0
            };
            let base = dv << 8;
            // SAFETY: live LeafBitmap1 node pointer.
            let b = unsafe { &*edge.node_ptr().cast::<LeafBitmap1>() };
            let mut out = Vec::with_capacity(b.bitmap.count() as usize);
            let mut from = b.bitmap.next_set(0);
            while let Some(bit) = from {
                out.push(base | u64::from(bit));
                from = if bit == 255 {
                    None
                } else {
                    b.bitmap.next_set(bit + 1)
                };
            }
            Some(out)
        }
        Some(EdgeTag::Structural(t)) if t.is_leaf() => {
            let kb = t.leaf_key_bytes().expect("leaf tag");
            let pop = (edge.pop0(kb) + 1) as usize;
            let dv = if kb < level {
                mutate::decode_value(edge, kb, level)
            } else {
                0
            };
            let base = dv << (8 * u32::from(kb));
            // SAFETY: live linear leaf of `pop` keys of `kb` bytes.
            let packed = unsafe { mutate::leaf_keys(edge, kb, pop) };
            Some(packed.into_iter().map(|k| base | k).collect())
        }
        _ => None,
    }
}

/// Counts how many of `keys` (remainders relative to `level`) are present in
/// the subtree rooted at `probe`.
///
/// # Safety
///
/// `probe` must reference a live, well-formed subtree covering `level` bytes.
#[inline]
unsafe fn probe_count(keys: &[u64], probe: &Edge, level: u8) -> u64 {
    let mut acc = 0u64;
    for &k in keys {
        // SAFETY: probe is a live subtree at `level`; k's high bits are zero
        // above `level`, which test_set ignores.
        if unsafe { get::test_set(probe, k, level) } {
            acc += 1;
        }
    }
    acc
}

/// Intersection cardinality of two branches at `level`: iterate the present
/// children of `eb` (the smaller side, per the caller's population ordering)
/// and probe each digit into `ea`, recursing only where both have a child.
/// Driving from `eb`'s actual children — rather than scanning all 256 digits —
/// makes the cost scale with the sparser operand, which is the whole point of
/// the skewed-`AND` case (a tiny list intersected into a huge one): subtrees
/// absent on one side are never visited.
///
/// # Safety
///
/// Both edges must reference live branch subtrees covering the same `level`.
#[inline]
unsafe fn intersection_branch(ea: &Edge, eb: &Edge, level: u8) -> u64 {
    let t = match eb.tag() {
        Some(EdgeTag::Structural(t)) if t.is_branch() => t,
        _ => return 0,
    };
    // SAFETY: eb is a live branch of type `t`.
    let bl = unsafe { mutate::branch_form_level(eb, t, level) };
    if bl < level {
        // eb skips this level: exactly one present digit (its decode byte).
        let pd = eb.decode_bytes(bl)[(level - bl - 1) as usize];
        // SAFETY: ea is a live branch at `level`.
        let ca = unsafe { child_by_digit(ea, level, pd) };
        if ca.is_null() {
            return 0;
        }
        // eb re-viewed one level shallower (still skipping, or now real).
        // SAFETY: ca and eb are live subtrees at level - 1.
        return unsafe { intersection_len(&ca, eb, level - 1) };
    }
    let mut acc = 0u64;
    match t {
        EdgeType::BranchU => {
            // SAFETY: live BranchU pointer.
            let b = unsafe { &*eb.node_ptr().cast::<BranchU>() };
            for (d, cb) in b.edges.iter().enumerate() {
                if cb.is_null() {
                    continue;
                }
                // SAFETY: ea is a live branch at `level`.
                let ca = unsafe { child_by_digit(ea, level, d as u8) };
                if !ca.is_null() {
                    // SAFETY: ca, cb are live subtrees at level - 1.
                    acc += unsafe { intersection_len(&ca, cb, level - 1) };
                }
            }
        }
        EdgeType::BranchB => {
            // SAFETY: live BranchB pointer.
            let b = unsafe { &*eb.node_ptr().cast::<BranchB>() };
            let mut from = b.bitmap.next_set(0);
            while let Some(d) = from {
                let sub = (d >> 5) as usize;
                let slot = b.bitmap.subexpanse_rank(d) as usize;
                // SAFETY: d is set → subarrays[sub] is non-null, slot in range.
                let cb = unsafe { *b.subarrays[sub].add(slot) };
                // SAFETY: ea is a live branch at `level`.
                let ca = unsafe { child_by_digit(ea, level, d) };
                if !ca.is_null() {
                    // SAFETY: ca, cb are live subtrees at level - 1.
                    acc += unsafe { intersection_len(&ca, &cb, level - 1) };
                }
                from = if d == 255 {
                    None
                } else {
                    b.bitmap.next_set(d + 1)
                };
            }
        }
        EdgeType::BranchL3 => {
            // SAFETY: live BranchL3 pointer.
            let b = unsafe { &*eb.node_ptr().cast::<BranchL3>() };
            for i in 0..b.hdr.num as usize {
                let d = b.hdr.digits[i];
                let cb = b.edges[i];
                // SAFETY: ea is a live branch at `level`.
                let ca = unsafe { child_by_digit(ea, level, d) };
                if !ca.is_null() {
                    // SAFETY: ca, cb are live subtrees at level - 1.
                    acc += unsafe { intersection_len(&ca, &cb, level - 1) };
                }
            }
        }
        EdgeType::BranchL7 => {
            // SAFETY: live BranchL7 pointer.
            let b = unsafe { &*eb.node_ptr().cast::<BranchL7>() };
            for i in 0..b.hdr.num as usize {
                let d = b.hdr.digits[i];
                let cb = b.edges[i];
                // SAFETY: ea is a live branch at `level`.
                let ca = unsafe { child_by_digit(ea, level, d) };
                if !ca.is_null() {
                    // SAFETY: ca, cb are live subtrees at level - 1.
                    acc += unsafe { intersection_len(&ca, &cb, level - 1) };
                }
            }
        }
        _ => {}
    }
    acc
}

/// Intersection cardinality where at least one side is a terminal (leaf or
/// immediate) that did not reduce to a shared level-1 bitmap: enumerate the
/// smaller terminal's keys and probe them into the other subtree.
///
/// # Safety
///
/// Both edges must reference live subtrees covering the same `level`; not both
/// are branches (that case is handled by [`intersection_branch`]).
#[inline]
unsafe fn intersection_terminal(ea: &Edge, eb: &Edge, level: u8) -> u64 {
    // SAFETY: forwarded contract.
    let ra = unsafe { terminal_remainders(ea, level) };
    // SAFETY: forwarded contract.
    let rb = unsafe { terminal_remainders(eb, level) };
    match (ra, rb) {
        (Some(ka), Some(kb)) => {
            // Both terminal: probe the smaller list into the larger subtree.
            if ka.len() <= kb.len() {
                // SAFETY: eb is a live subtree at `level`.
                unsafe { probe_count(&ka, eb, level) }
            } else {
                // SAFETY: ea is a live subtree at `level`.
                unsafe { probe_count(&kb, ea, level) }
            }
        }
        // SAFETY: the branch side is a live subtree at `level`.
        (Some(ka), None) => unsafe { probe_count(&ka, eb, level) },
        // SAFETY: the branch side is a live subtree at `level`.
        (None, Some(kb)) => unsafe { probe_count(&kb, ea, level) },
        (None, None) => 0,
    }
}

/// Intersection cardinality of two non-null subtrees covering the same
/// expanse of `level` undecoded bytes. `eb` should be the smaller side.
///
/// # Safety
///
/// Both edges must reference live, well-formed subtrees at `level`, each
/// pointer-tagged edge referencing a live node of its tagged type.
pub(crate) unsafe fn intersection_len(ea: &Edge, eb: &Edge, level: u8) -> u64 {
    debug_assert!(!ea.is_null() && !eb.is_null());
    let ta = ea.tag_byte();
    let tb = eb.tag_byte();

    // A full expanse contains every key of its expanse, so its intersection
    // with the other side is exactly the other side's population.
    if ta == EdgeType::FullExpanse as u8 {
        // SAFETY: eb is a live subtree at `level`.
        return unsafe { subtree_count(eb, level) };
    }
    if tb == EdgeType::FullExpanse as u8 {
        // SAFETY: ea is a live subtree at `level`.
        return unsafe { subtree_count(ea, level) };
    }

    // Both reduce to a final-byte bitmap (real level 1, incl. level == 1):
    // word-parallel popcnt when the skipped middle digits agree, else the
    // two expanses are disjoint.
    // SAFETY: both edges live at `level`.
    if let (Some((da, ba)), Some((db, bb))) = (unsafe { as_level1_bitmap(ea, level) }, unsafe {
        as_level1_bitmap(eb, level)
    }) {
        return if da == db {
            u64::from(ba.count_and(&bb))
        } else {
            0
        };
    }

    if is_branch(ta) && is_branch(tb) {
        // SAFETY: both are live branches at `level`.
        return unsafe { intersection_branch(ea, eb, level) };
    }

    // SAFETY: at least one terminal; both live at `level`.
    unsafe { intersection_terminal(ea, eb, level) }
}
