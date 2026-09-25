//! Subtree condensation on remove: the `subtree-condense` spike
//! (`docs/benchmarks/remove_retention/METHODOLOGY.md` §5).
//!
//! The remove walks step a branch down its own ladder and free it when it
//! empties, but never rebuild it into a leaf. Under this feature a branch frame
//! whose subtree population `P` lands on one of the arm's evaluation points
//! (§5.2) compares the subtree's accounted bytes with the same `P` keys packed
//! as one leaf (or immediate) at the edge's slot level, and replaces the
//! subtree only when the packed form is **strictly smaller** (§5.1). `P` is
//! the branch edge's `pop0`, which the walk already maintains.
//!
//! The arm is selected by its threshold: `subtree-condense` alone is `H1`
//! (`LEAF_CAP - 1`), `subtree-condense-wide` is `wide` (`LEAF_CAP - 8`). The
//! root's top edge (level 8) is never condensed here; the root-leaf condense
//! already covers it.
//!
//! Shared trees (§5.3): the optimistic write paths in `sync.rs` never reach
//! this code. The engine walks call it only in their `CONDENSE` instantiation:
//! every plain tree, and a serialized removal under the writer lock whose top
//! digit is clean in `DirtyDigits` (`SyncExpanseSet::remove`,
//! `SyncExpanseMap::remove`). [`sweep`] is the `shrink_to_fit` pass, run after
//! the wrapper folds its dirty digits.

use crate::alloc::{NodeAlloc, accounted_size};
use crate::leaf;
use crate::mutate::{StackKeys32, sub_edges_size, sub_vals_size, write_immed};
use crate::mutate_map::{StackEntries32, map_immed_val_size};
use crate::node::{BranchB, BranchL3, BranchL7, BranchU, Edge, LeafBitmap1, LeafBitmapL};
use crate::occ::Cover;
use crate::types::{EdgeTag, EdgeType, ImmedType, LEAF_CAP, RAW_ALIGN};

/// The `H1` arm's threshold: Judy's one index of hysteresis below
/// `LEAF_CAP` (METHODOLOGY §5.2).
pub const THRESHOLD_H1: usize = LEAF_CAP - 1;

/// The `wide` arm's threshold: the 24-slot class (METHODOLOGY §5.2).
pub const THRESHOLD_WIDE: usize = LEAF_CAP - 8;

/// The arm this build runs.
#[cfg(not(feature = "subtree-condense-wide"))]
pub const THRESHOLD: usize = THRESHOLD_H1;

/// The arm this build runs.
#[cfg(feature = "subtree-condense-wide")]
pub const THRESHOLD: usize = THRESHOLD_WIDE;

/// Whether a subtree left at population `p` is evaluated: the threshold
/// itself, then every slot-class top below it (`cap_class(p) == p`),
/// METHODOLOGY §5.2.
#[inline(always)]
#[must_use]
pub const fn is_evaluation_point(p: usize) -> bool {
    p == THRESHOLD || (p >= 1 && p < THRESHOLD && leaf::cap_class(p) == p)
}

/// Accounted bytes of `p` keys packed at slot level `level` the way a fresh
/// build places them: an immediate when they fit one, else a linear leaf of
/// `level`-byte keys (a leaf is not narrowed below its parent, METHODOLOGY
/// §4). The edge itself is excluded: it is the same edge either way.
#[must_use]
pub(crate) const fn packed_bytes<const MAP: bool>(p: usize, level: u8) -> usize {
    if MAP {
        if p <= crate::mutate::map_immed_max(level) {
            if p >= 2 {
                accounted_size(map_immed_val_size(p), RAW_ALIGN)
            } else {
                0
            }
        } else {
            accounted_size(leaf::size_map(level, p), RAW_ALIGN)
        }
    } else if p <= ImmedType::max_count(level) as usize {
        0
    } else {
        accounted_size(leaf::size_set(level, p), RAW_ALIGN)
    }
}

/// Accounted heap bytes of the subtree under `edge` (edge excluded), the
/// same attribution `validate::NodeBytes` sums to `mem_used()`. Stops as
/// soon as the running total exceeds `budget`, returning a value above it.
///
/// # Safety
///
/// `edge` is a live, well-formed edge of the engine's flavour.
pub(crate) unsafe fn subtree_bytes<const MAP: bool>(edge: &Edge, budget: usize) -> usize {
    let raw = |n: usize| accounted_size(n, RAW_ALIGN);
    let Some(tag) = edge.tag() else { return 0 };
    // SAFETY: live nodes of the tagged types per the contract; children are
    // sizes do not depend on the slot level.
    unsafe {
        match tag {
            EdgeTag::Structural(EdgeType::Null | EdgeType::FullExpanse) => 0,
            EdgeTag::Immed(im) => {
                if MAP && im.key_count() >= 2 {
                    raw(map_immed_val_size(im.key_count() as usize))
                } else {
                    0
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
                let pop = edge.pop0(kb) as usize + 1;
                raw(if MAP {
                    leaf::size_map(kb, pop)
                } else {
                    leaf::size_set(kb, pop)
                })
            }
            EdgeTag::Structural(EdgeType::LeafB1) => {
                if MAP {
                    let node = &*edge.node_ptr().cast::<LeafBitmapL>();
                    let mut total = size_of::<LeafBitmapL>();
                    for sub in 0..8 {
                        let n = node.bitmap.subexpanse_count(sub) as usize;
                        if n > 0 {
                            total += raw(sub_vals_size(n));
                        }
                    }
                    total
                } else {
                    size_of::<LeafBitmap1>()
                }
            }
            EdgeTag::Structural(EdgeType::BranchL3) => {
                let b = &*edge.node_ptr().cast::<BranchL3>();
                let mut total = size_of::<BranchL3>();
                for i in 0..b.hdr.num as usize {
                    if total > budget {
                        break;
                    }
                    total += subtree_bytes::<MAP>(&b.edges[i], budget - total);
                }
                total
            }
            EdgeTag::Structural(EdgeType::BranchL7) => {
                let b = &*edge.node_ptr().cast::<BranchL7>();
                let mut total = size_of::<BranchL7>();
                for i in 0..b.hdr.num as usize {
                    if total > budget {
                        break;
                    }
                    total += subtree_bytes::<MAP>(&b.edges[i], budget - total);
                }
                total
            }
            EdgeTag::Structural(EdgeType::BranchB) => {
                let b = &*edge.node_ptr().cast::<BranchB>();
                let mut total = size_of::<BranchB>();
                for sub in 0..8 {
                    let n = b.pop_counts[sub] as usize;
                    if n == 0 {
                        continue;
                    }
                    total += raw(sub_edges_size(n));
                    for i in 0..n {
                        if total > budget {
                            return total;
                        }
                        total += subtree_bytes::<MAP>(&*b.subarrays[sub].add(i), budget - total);
                    }
                }
                total
            }
            EdgeTag::Structural(EdgeType::BranchU) => {
                let b = &*edge.node_ptr().cast::<BranchU>();
                let mut total = size_of::<BranchU>();
                for child in &b.edges {
                    if total > budget {
                        break;
                    }
                    total += subtree_bytes::<MAP>(child, budget - total);
                }
                total
            }
        }
    }
}

/// Marks every branch node under `edge` obsolete (OCC only), so a reader
/// still inside the replaced subtree fails its next validation rather than
/// trusting a node about to be retired (see `occ::version_obsolete_if`).
///
/// # Safety
///
/// `edge` is a live, well-formed edge; every branch below it has its own
/// bracket closed.
unsafe fn mark_obsolete<const OCC: bool>(edge: &Edge) {
    if !OCC {
        return;
    }
    let Some(tag) = edge.tag() else { return };
    // SAFETY: live nodes of the tagged types per the contract.
    unsafe {
        match tag {
            EdgeTag::Structural(EdgeType::BranchL3) => {
                let b = edge.node_ptr().cast::<BranchL3>();
                for i in 0..(*b).hdr.num as usize {
                    mark_obsolete::<OCC>(&(*b).edges[i]);
                }
                crate::occ::version_obsolete_if::<OCC>(&raw mut (*b).hdr.version);
            }
            EdgeTag::Structural(EdgeType::BranchL7) => {
                let b = edge.node_ptr().cast::<BranchL7>();
                for i in 0..(*b).hdr.num as usize {
                    mark_obsolete::<OCC>(&(*b).edges[i]);
                }
                crate::occ::version_obsolete_if::<OCC>(&raw mut (*b).hdr.version);
            }
            EdgeTag::Structural(EdgeType::BranchB) => {
                let b = edge.node_ptr().cast::<BranchB>();
                for sub in 0..8 {
                    for i in 0..(*b).pop_counts[sub] as usize {
                        mark_obsolete::<OCC>(&*(*b).subarrays[sub].add(i));
                    }
                }
                crate::occ::version_obsolete_if::<OCC>(&raw mut (*b).version);
            }
            EdgeTag::Structural(EdgeType::BranchU) => {
                let b = edge.node_ptr().cast::<BranchU>();
                for i in 0..256 {
                    mark_obsolete::<OCC>(&(*b).edges[i]);
                }
                crate::occ::version_obsolete_if::<OCC>(&raw mut (*b).version);
            }
            _ => {}
        }
    }
}

/// The remove walks' hook, called on a branch frame's success exit once the
/// frame's own `pop0` is decremented: condenses the subtree if its
/// population is an evaluation point and the packed form is smaller.
///
/// # Safety
///
/// Same contract as the remove walk frame that calls it: `edge` is the live
/// branch edge at slot `level`, `bl` its form level, and `cover` the word of
/// the node holding `edge`, with every bracket of this frame closed.
#[inline(always)]
pub(crate) unsafe fn after_branch_remove<const OCC: bool, const NESTED: bool, const MAP: bool>(
    a: &NodeAlloc,
    edge: &mut Edge,
    level: u8,
    bl: u8,
    cover: Cover,
) {
    if level >= 8 {
        return;
    }
    let p = edge.pop0(bl) as usize + 1;
    if !is_evaluation_point(p) {
        return;
    }
    // SAFETY: forwarded contract.
    unsafe { condense::<OCC, NESTED, MAP>(a, edge, level, p, cover) };
}

/// Condenses the branch subtree at `edge` (slot `level`, `p` keys) into a
/// packed leaf or immediate when that is strictly smaller. Returns whether
/// it did.
///
/// # Safety
///
/// As [`after_branch_remove`]; `p` is the subtree's exact population.
#[cold]
#[inline(never)]
pub(crate) unsafe fn condense<const OCC: bool, const NESTED: bool, const MAP: bool>(
    a: &NodeAlloc,
    edge: &mut Edge,
    level: u8,
    p: usize,
    cover: Cover,
) -> bool {
    debug_assert!((2..8).contains(&level));
    debug_assert!((1..LEAF_CAP).contains(&p));
    let packed = packed_bytes::<MAP>(p, level);
    // SAFETY: live subtree per contract.
    if unsafe { subtree_bytes::<MAP>(edge, packed) } <= packed {
        return false;
    }
    let mut fresh = Edge::NULL;
    if MAP {
        let mut entries = StackEntries32::new();
        let mut from = Some(0u64);
        // SAFETY: live map subtree; `nav::next` works in the slot's suffix
        // space and returns each key with its value word.
        while let Some(f) = from {
            // SAFETY: as above.
            let Some((k, v)) = (unsafe { crate::nav::next::<true>(edge, f, level) }) else {
                break;
            };
            entries.push((k, v));
            from = k
                .checked_add(1)
                .filter(|&n| n < crate::mutate::pow256(level));
        }
        debug_assert_eq!(entries.len, p);
        if p <= crate::mutate::map_immed_max(level) {
            crate::mutate_map::write_map_immed::<OCC>(a, &mut fresh, level, entries.as_slice());
        } else {
            crate::mutate_map::build_map_leaf::<OCC>(a, &mut fresh, level, entries.as_slice());
        }
    } else {
        let mut keys = StackKeys32::new();
        let mut from = Some(0u64);
        // SAFETY: live set subtree; `nav::next` works in the slot's suffix
        // space.
        while let Some(f) = from {
            // SAFETY: as above.
            let Some((k, _)) = (unsafe { crate::nav::next::<false>(edge, f, level) }) else {
                break;
            };
            keys.push(k);
            from = k
                .checked_add(1)
                .filter(|&n| n < crate::mutate::pow256(level));
        }
        debug_assert_eq!(keys.len, p);
        if p <= ImmedType::max_count(level) as usize {
            write_immed(&mut fresh, level, keys.as_slice());
        } else {
            crate::mutate::build_leaf::<OCC>(a, &mut fresh, level, keys.as_slice());
        }
    }
    crate::occ_stats::note_branch_replacement();
    let mut old = *edge;
    // The slot is the parent's: its word covers the store. The replaced
    // branches are marked obsolete inside the same bracket, before a reader
    // can load the new edge.
    cover.begin_if::<OCC, NESTED>(a);
    // SAFETY: the replaced subtree is live until freed below.
    unsafe { mark_obsolete::<OCC>(&old) };
    *edge = fresh;
    cover.end_if::<OCC, NESTED>(a);
    // SAFETY: unlinked above; every node freed exactly once (retired through
    // the collector on a shared tree). Values moved into `fresh`, so the map
    // flavour frees nodes and value arrays only.
    unsafe { crate::mutate::free_subtree::<OCC, MAP>(a, &mut old) };
    true
}

/// The `shrink_to_fit` pass: condenses every branch subtree below the root's
/// top edge whose population is at most [`THRESHOLD`] and whose packed form
/// is smaller, bottom-up. Returns the number of subtrees condensed.
///
/// A sweep has no removal to land on an evaluation point, so it applies the
/// byte rule at every population up to the threshold (METHODOLOGY §5.3:
/// "condense every subtree the rule accepts").
///
/// # Safety
///
/// `edge` is a live, well-formed edge at `level` whose ancestors' and own
/// `pop0` counts are exact (a shared tree's dirty digits folded), with no
/// other writer running; `cover` is the word of the node holding it.
pub(crate) unsafe fn sweep<const OCC: bool, const NESTED: bool, const MAP: bool>(
    a: &NodeAlloc,
    edge: &mut Edge,
    level: u8,
    cover: Cover,
) -> usize {
    let Some(tag) = edge.tag() else { return 0 };
    let mut done = 0usize;
    // SAFETY: live nodes per contract; a child frame takes its node's word
    // as cover, as the remove walk does.
    unsafe {
        let bl = match tag {
            EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
                let bl = crate::mutate::branch_form_level(edge, t, level);
                if matches!(t, EdgeType::BranchL3) {
                    let b = edge.node_ptr().cast::<BranchL3>();
                    let inner = Cover::Node(&raw mut (*b).hdr.version);
                    for i in 0..(*b).hdr.num as usize {
                        inner.nest_begin::<OCC, NESTED>(a);
                        done += sweep::<OCC, NESTED, MAP>(a, &mut (*b).edges[i], bl - 1, inner);
                        inner.nest_end::<OCC, NESTED>(a);
                    }
                } else {
                    let b = edge.node_ptr().cast::<BranchL7>();
                    let inner = Cover::Node(&raw mut (*b).hdr.version);
                    for i in 0..(*b).hdr.num as usize {
                        inner.nest_begin::<OCC, NESTED>(a);
                        done += sweep::<OCC, NESTED, MAP>(a, &mut (*b).edges[i], bl - 1, inner);
                        inner.nest_end::<OCC, NESTED>(a);
                    }
                }
                bl
            }
            EdgeTag::Structural(EdgeType::BranchB) => {
                let bl = crate::mutate::branch_form_level(edge, EdgeType::BranchB, level);
                let b = edge.node_ptr().cast::<BranchB>();
                let inner = Cover::Node(&raw mut (*b).version);
                for sub in 0..8 {
                    for i in 0..(*b).pop_counts[sub] as usize {
                        inner.nest_begin::<OCC, NESTED>(a);
                        done += sweep::<OCC, NESTED, MAP>(
                            a,
                            &mut *(*b).subarrays[sub].add(i),
                            bl - 1,
                            inner,
                        );
                        inner.nest_end::<OCC, NESTED>(a);
                    }
                }
                bl
            }
            EdgeTag::Structural(EdgeType::BranchU) => {
                let b = edge.node_ptr().cast::<BranchU>();
                let inner = Cover::Node(&raw mut (*b).version);
                for i in 0..256 {
                    if (*b).edges[i].is_null() {
                        continue;
                    }
                    inner.nest_begin::<OCC, NESTED>(a);
                    done += sweep::<OCC, NESTED, MAP>(a, &mut (*b).edges[i], level - 1, inner);
                    inner.nest_end::<OCC, NESTED>(a);
                }
                level
            }
            _ => return 0,
        };
        if level < 8 {
            let p = edge.pop0(bl) as usize + 1;
            if p <= THRESHOLD && condense::<OCC, NESTED, MAP>(a, edge, level, p, cover) {
                done += 1;
            }
        }
    }
    done
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `packed_bytes` against `scripts/condense_bounds.py`'s pinned values
    /// (METHODOLOGY §4): a 32-key `Leaf6` is 192 B (set) / 448 B (map), a
    /// fresh 20-key leaf 144 / 336 B, and a set immediate costs no heap.
    #[test]
    fn packed_bytes_match_the_model_pins() {
        assert_eq!(packed_bytes::<false>(32, 6), 192);
        assert_eq!(packed_bytes::<true>(32, 6), 448);
        assert_eq!(packed_bytes::<false>(20, 6), 144);
        assert_eq!(packed_bytes::<true>(20, 6), 336);
        assert_eq!(packed_bytes::<false>(2, 6), 0);
        assert_eq!(packed_bytes::<true>(1, 6), 0);
        // Two map keys of one byte fit an immediate whose values spill to a
        // class-sized array: 16 B.
        assert_eq!(packed_bytes::<true>(2, 2), 16);
    }

    #[test]
    fn evaluation_points_are_the_threshold_then_class_tops() {
        let points: Vec<usize> = (1..=LEAF_CAP).filter(|&p| is_evaluation_point(p)).collect();
        let tops: Vec<usize> = (1..THRESHOLD)
            .filter(|&p| leaf::cap_class(p) == p)
            .collect();
        assert_eq!(points.last(), Some(&THRESHOLD));
        assert_eq!(&points[..points.len() - 1], tops.as_slice());
        assert!(!is_evaluation_point(0));
        assert!(!is_evaluation_point(LEAF_CAP));
    }

    /// One condense on a plain set small enough for Miri: a cascaded level-6
    /// expanse drained onto the threshold becomes one leaf.
    #[test]
    fn a_small_set_condenses() {
        let mut s = crate::set::ExpanseSet::new();
        for t in 0..40u64 {
            s.insert((0x80 + t) << 56);
        }
        let ks: Vec<u64> = (0..=LEAF_CAP as u64)
            .map(|j| (0x07u64 << 48) | (j << 40))
            .collect();
        for &k in &ks {
            s.insert(k);
        }
        for &k in ks[THRESHOLD..].iter().rev() {
            assert!(s.remove(k));
        }
        s.validate();
        assert_eq!(s.stats().leaf_pop_histogram[THRESHOLD], 1);
        for &k in &ks[..THRESHOLD] {
            assert!(s.contains(k));
        }
    }

    /// The map twin, with values checked after the move.
    #[test]
    fn a_small_map_condenses() {
        let mut m = crate::map::ExpanseMap::new();
        for t in 0..40u64 {
            m.insert((0x80 + t) << 56, t);
        }
        let ks: Vec<u64> = (0..=LEAF_CAP as u64)
            .map(|j| (0x07u64 << 48) | (j << 40))
            .collect();
        for &k in &ks {
            m.insert(k, !k);
        }
        for &k in ks[THRESHOLD..].iter().rev() {
            assert_eq!(m.remove(k), Some(!k));
        }
        m.validate();
        assert_eq!(m.stats().leaf_pop_histogram[THRESHOLD], 1);
        for &k in &ks[..THRESHOLD] {
            assert_eq!(m.get(k), Some(!k));
        }
    }
}
