//! Ordered navigation and rank/count over a subtree.
//!
//! These four primitives are the engine behind iteration, range counting,
//! and the future compat surface (`Judy1First/Next/Last/Prev/Count/ByCount`
//! and their JudyL twins):
//!
//! - [`next`]: smallest key `>= suffix` in the subtree (inclusive-forward
//!   search; "First" semantics — "Next" is this at `suffix + 1`).
//! - [`prev`]: largest key `<= suffix` (inclusive-backward; "Last"/"Prev").
//! - [`count_below`]: number of keys strictly below `suffix` — the rank
//!   primitive behind range counting, O(depth) thanks to the `pop0` fields
//!   every edge maintains.
//! - [`by_count`]: the key with `n` keys below it (0-based select).
//!
//! All four work on key *suffixes*: at `level`, only the low `level` bytes
//! of `suffix` are meaningful, and results are returned in the same space
//! (the caller composes higher digits). The map flavor (`MAP = true`)
//! returns each found key's value alongside it (the compat layer hands out
//! value pointers from `First`/`Next`, so navigation must locate values,
//! not just keys).
//!
//! Narrow pointers: a leaf child (linear or bitmap) may sit below its
//! slot level; its decode bytes define the sub-expanse holding its keys,
//! so navigation compares the suffix's skipped digits against the decode
//! value and composes results as `decode << 8·kb | low`.

use crate::leaf;
use crate::mutate::{key_low, pow256};
use crate::node::{BranchB, BranchL3, BranchL7, BranchU, Edge, LeafBitmap1, LeafBitmapL};
use crate::types::{EdgeTag, EdgeType, digit};

/// Population of a child subtree, read from its edge (children sit at
/// `level <= 7`, so the `pop0` field is always present; immediates carry
/// their count in the tag).
/// # Safety
///
/// Branch-tagged edges are dereferenced to read their own level, so the
/// usual live-tree contract applies.
unsafe fn edge_pop(edge: &Edge, level: u8) -> u64 {
    match edge.tag().expect("valid edge tag") {
        EdgeTag::Structural(EdgeType::Null) => 0,
        EdgeTag::Immed(im) => u64::from(im.key_count()),
        // Skipping forms store pop0 at their own level, not the slot's.
        EdgeTag::Structural(t) if t.leaf_key_bytes().is_some() => {
            edge.pop0(t.leaf_key_bytes().expect("leaf tag")) + 1
        }
        EdgeTag::Structural(EdgeType::LeafB1) => edge.pop0(1) + 1,
        EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7 | EdgeType::BranchB)) => {
            // SAFETY: live branch per this function's contract.
            let bl = unsafe { crate::mutate::branch_form_level(edge, t, level) };
            edge.pop0(bl) + 1
        }
        EdgeTag::Structural(_) => edge.pop0(level) + 1,
    }
}

/// A skipping leaf's expanse position: `(decode_value, shift)` such that
/// its full keys are `decode_value << shift | low`. Non-skipping leaves
/// return `(0, _)`, making the composition the identity.
fn skip_parts(edge: &Edge, kb: u8, level: u8) -> (u64, u32) {
    let dv = if kb < level {
        crate::mutate::decode_value(edge, kb, level)
    } else {
        0
    };
    (dv, 8 * u32::from(kb))
}

/// Reads the sorted key list of an immediate edge for either flavor
/// (stack-resident: immediates never exceed 15 keys).
fn immed_suffixes<const MAP: bool>(
    edge: &Edge,
    im: crate::types::ImmedType,
) -> crate::mutate::ImmedBuf<u64> {
    if MAP {
        crate::mutate::immed_map_keys(edge, im)
    } else {
        crate::mutate::immed_keys(edge, im)
    }
}

/// The value of slot `slot` in a map immediate.
///
/// # Safety
///
/// Multi-key map immediates must hold a live value array in word 0.
unsafe fn immed_value(edge: &Edge, im: crate::types::ImmedType, slot: usize) -> u64 {
    if im.key_count() == 1 {
        u64::from_le_bytes(edge.imm_bytes())
    } else {
        // SAFETY: live value array of key_count values per contract.
        unsafe { *edge.node_ptr().cast::<u64>().add(slot) }
    }
}

/// Key at `slot` of a linear leaf (either flavor).
///
/// # Safety
///
/// The edge must reference a live linear leaf of `pop` keys.
unsafe fn leaf_suffix<const MAP: bool>(edge: &Edge, kb: u8, pop: usize, slot: usize) -> u64 {
    let base = edge.node_ptr();
    let keys = if MAP {
        // SAFETY: map leaf = values then packed keys.
        unsafe { base.add(leaf::map_keys_offset(pop)) }
    } else {
        base
    };
    // SAFETY: slot < pop packed keys per contract.
    unsafe { crate::mutate::read_packed(keys, slot, kb as usize) }
}

/// Value at `slot` of a map linear leaf.
///
/// # Safety
///
/// The edge must reference a live map leaf of at least `slot + 1` values.
unsafe fn leaf_value(edge: &Edge, slot: usize) -> u64 {
    // SAFETY: values sit at the front of the map-leaf allocation.
    unsafe { *edge.node_ptr().cast::<u64>().add(slot) }
}

/// Binary-searches a leaf for the first slot with key `>= suffix`.
///
/// # Safety
///
/// Same contract as [`leaf_suffix`].
unsafe fn leaf_lower_bound<const MAP: bool>(edge: &Edge, kb: u8, pop: usize, suffix: u64) -> usize {
    let (mut lo, mut hi) = (0usize, pop);
    while lo < hi {
        let mid = (lo + hi) / 2;
        // SAFETY: mid < pop.
        if unsafe { leaf_suffix::<MAP>(edge, kb, pop, mid) } < suffix {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// Smallest key `>= suffix` in the subtree, with its value (0 for sets).
///
/// # Safety
///
/// Same tree contract as `get::test_set`, for the matching flavor.
pub(crate) unsafe fn next<const MAP: bool>(
    edge: &Edge,
    suffix: u64,
    level: u8,
) -> Option<(u64, u64)> {
    debug_assert!((1..=8).contains(&level));
    debug_assert!(level == 8 || suffix < pow256(level));
    match edge.tag().expect("valid edge tag") {
        EdgeTag::Structural(EdgeType::Null) => None,

        EdgeTag::Immed(im) => {
            let keys = immed_suffixes::<MAP>(edge, im);
            let slot = keys.partition_point(|&k| k < suffix);
            let k = *keys.get(slot)?;
            let v = if MAP {
                // SAFETY: live map immediate per contract.
                unsafe { immed_value(edge, im, slot) }
            } else {
                0
            };
            Some((k, v))
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
            let (dv, shift) = skip_parts(edge, kb, level);
            let low = match (suffix >> shift).cmp(&dv) {
                core::cmp::Ordering::Less => 0,
                core::cmp::Ordering::Equal => key_low(suffix, kb),
                core::cmp::Ordering::Greater => return None,
            };
            // SAFETY: live leaf of `pop` keys per contract.
            let slot = unsafe { leaf_lower_bound::<MAP>(edge, kb, pop, low) };
            if slot == pop {
                return None;
            }
            // SAFETY: slot < pop.
            let k = unsafe { leaf_suffix::<MAP>(edge, kb, pop, slot) };
            let v = if MAP {
                // SAFETY: map leaves hold pop values at the base.
                unsafe { leaf_value(edge, slot) }
            } else {
                0
            };
            Some(((dv << shift) | k, v))
        }

        EdgeTag::Structural(EdgeType::LeafB1) => {
            let (dv, shift) = skip_parts(edge, 1, level);
            let suffix = match (suffix >> shift).cmp(&dv) {
                core::cmp::Ordering::Less => 0,
                core::cmp::Ordering::Equal => key_low(suffix, 1),
                core::cmp::Ordering::Greater => return None,
            };
            if suffix > 255 {
                return None;
            }
            if MAP {
                // SAFETY: live LeafBitmapL per contract.
                let node = unsafe { &*edge.node_ptr().cast::<LeafBitmapL>() };
                let d = node.bitmap.next_set(suffix as u8)?;
                let sub = (d >> 5) as usize;
                let rank = node.bitmap.subexpanse_rank(d) as usize;
                // SAFETY: bit set → value subarray holds rank + 1 values.
                let v = unsafe { *node.values[sub].add(rank) };
                Some(((dv << shift) | u64::from(d), v))
            } else {
                // SAFETY: live LeafBitmap1 per contract.
                let node = unsafe { &*edge.node_ptr().cast::<LeafBitmap1>() };
                Some((
                    (dv << shift) | u64::from(node.bitmap.next_set(suffix as u8)?),
                    0,
                ))
            }
        }

        EdgeTag::Structural(EdgeType::FullExpanse) => {
            debug_assert!(!MAP);
            // Every key in the expanse is present.
            Some((suffix, 0))
        }

        EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
            // SAFETY: live branch per contract.
            let bl = unsafe { crate::mutate::branch_form_level(edge, t, level) };
            let (dv, shift): (u64, u32) = if bl < level {
                (
                    crate::mutate::decode_value(edge, bl, level),
                    8 * u32::from(bl),
                )
            } else {
                (0, 0)
            };
            let suffix = if bl < level {
                match (suffix >> shift).cmp(&dv) {
                    core::cmp::Ordering::Less => 0,
                    core::cmp::Ordering::Equal => key_low(suffix, bl),
                    core::cmp::Ordering::Greater => return None,
                }
            } else {
                suffix
            };
            let d = digit(suffix, bl);
            // SAFETY: live branch per contract.
            let (num, digits, edges_ptr) = unsafe {
                if matches!(t, EdgeType::BranchL3) {
                    let b = &*edge.node_ptr().cast::<BranchL3>();
                    (b.hdr.num as usize, b.hdr.digits, b.edges.as_ptr())
                } else {
                    let b = &*edge.node_ptr().cast::<BranchL7>();
                    (b.hdr.num as usize, b.hdr.digits, b.edges.as_ptr())
                }
            };
            for (slot, &bd) in digits.iter().enumerate().take(num) {
                if bd < d {
                    continue;
                }
                let rem = if bd == d { key_low(suffix, bl - 1) } else { 0 };
                // SAFETY: slot < num live child edges.
                let child = unsafe { &*edges_ptr.add(slot) };
                // SAFETY: child subtree per contract.
                if let Some((r, v)) = unsafe { next::<MAP>(child, rem, bl - 1) } {
                    return Some(((dv << shift) | compose(bd, r, bl), v));
                }
            }
            None
        }

        EdgeTag::Structural(EdgeType::BranchB) => {
            // SAFETY: live branch per contract.
            let bl = unsafe { crate::mutate::branch_form_level(edge, EdgeType::BranchB, level) };
            let (dv, shift): (u64, u32) = if bl < level {
                (
                    crate::mutate::decode_value(edge, bl, level),
                    8 * u32::from(bl),
                )
            } else {
                (0, 0)
            };
            let suffix = if bl < level {
                match (suffix >> shift).cmp(&dv) {
                    core::cmp::Ordering::Less => 0,
                    core::cmp::Ordering::Equal => key_low(suffix, bl),
                    core::cmp::Ordering::Greater => return None,
                }
            } else {
                suffix
            };
            let d = digit(suffix, bl);
            // SAFETY: live BranchB per contract.
            let b = unsafe { &*edge.node_ptr().cast::<BranchB>() };
            let mut cur = b.bitmap.next_set(d);
            while let Some(bd) = cur {
                let rem = if bd == d { key_low(suffix, bl - 1) } else { 0 };
                let sub = (bd >> 5) as usize;
                let rank = b.bitmap.subexpanse_rank(bd) as usize;
                // SAFETY: bit set → subarray holds rank + 1 edges.
                let child = unsafe { &*b.subarrays[sub].add(rank) };
                // SAFETY: child subtree per contract.
                if let Some((r, v)) = unsafe { next::<MAP>(child, rem, bl - 1) } {
                    return Some(((dv << shift) | compose(bd, r, bl), v));
                }
                cur = if bd == 255 {
                    None
                } else {
                    b.bitmap.next_set(bd + 1)
                };
            }
            None
        }

        EdgeTag::Structural(EdgeType::BranchU) => {
            let d = digit(suffix, level);
            // SAFETY: live BranchU per contract.
            let b = unsafe { &*edge.node_ptr().cast::<BranchU>() };
            for bd in d..=255u8 {
                let child = &b.edges[bd as usize];
                if child.is_null() {
                    continue;
                }
                let rem = if bd == d {
                    key_low(suffix, level - 1)
                } else {
                    0
                };
                // SAFETY: child subtree per contract.
                if let Some((r, v)) = unsafe { next::<MAP>(child, rem, level - 1) } {
                    return Some((compose(bd, r, level), v));
                }
            }
            None
        }
    }
}

/// Largest key `<= suffix` in the subtree, with its value (0 for sets).
///
/// # Safety
///
/// Same contract as [`next`].
pub(crate) unsafe fn prev<const MAP: bool>(
    edge: &Edge,
    suffix: u64,
    level: u8,
) -> Option<(u64, u64)> {
    debug_assert!((1..=8).contains(&level));
    debug_assert!(level == 8 || suffix < pow256(level));
    match edge.tag().expect("valid edge tag") {
        EdgeTag::Structural(EdgeType::Null) => None,

        EdgeTag::Immed(im) => {
            let keys = immed_suffixes::<MAP>(edge, im);
            let slot = keys.partition_point(|&k| k <= suffix).checked_sub(1)?;
            let v = if MAP {
                // SAFETY: live map immediate per contract.
                unsafe { immed_value(edge, im, slot) }
            } else {
                0
            };
            Some((keys[slot], v))
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
            let (dv, shift) = skip_parts(edge, kb, level);
            let low = match (suffix >> shift).cmp(&dv) {
                core::cmp::Ordering::Less => return None,
                core::cmp::Ordering::Equal => key_low(suffix, kb),
                core::cmp::Ordering::Greater => key_low(u64::MAX, kb),
            };
            let bound = if low == key_low(u64::MAX, kb) {
                pop
            } else {
                // SAFETY: live leaf of `pop` keys per contract.
                unsafe { leaf_lower_bound::<MAP>(edge, kb, pop, low + 1) }
            };
            let slot = bound.checked_sub(1)?;
            // SAFETY: slot < pop.
            let k = unsafe { leaf_suffix::<MAP>(edge, kb, pop, slot) };
            let v = if MAP {
                // SAFETY: map leaves hold pop values at the base.
                unsafe { leaf_value(edge, slot) }
            } else {
                0
            };
            Some(((dv << shift) | k, v))
        }

        EdgeTag::Structural(EdgeType::LeafB1) => {
            let (dv, shift) = skip_parts(edge, 1, level);
            let from = match (suffix >> shift).cmp(&dv) {
                core::cmp::Ordering::Less => return None,
                core::cmp::Ordering::Equal => key_low(suffix, 1) as u8,
                core::cmp::Ordering::Greater => 255,
            };
            if MAP {
                // SAFETY: live LeafBitmapL per contract.
                let node = unsafe { &*edge.node_ptr().cast::<LeafBitmapL>() };
                let d = node.bitmap.prev_set(from)?;
                let sub = (d >> 5) as usize;
                let rank = node.bitmap.subexpanse_rank(d) as usize;
                // SAFETY: bit set → value subarray holds rank + 1 values.
                let v = unsafe { *node.values[sub].add(rank) };
                Some(((dv << shift) | u64::from(d), v))
            } else {
                // SAFETY: live LeafBitmap1 per contract.
                let node = unsafe { &*edge.node_ptr().cast::<LeafBitmap1>() };
                Some(((dv << shift) | u64::from(node.bitmap.prev_set(from)?), 0))
            }
        }

        EdgeTag::Structural(EdgeType::FullExpanse) => {
            debug_assert!(!MAP);
            Some((suffix.min(pow256(level) - 1), 0))
        }

        EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
            // SAFETY: live branch per contract.
            let bl = unsafe { crate::mutate::branch_form_level(edge, t, level) };
            let (dv, shift): (u64, u32) = if bl < level {
                (
                    crate::mutate::decode_value(edge, bl, level),
                    8 * u32::from(bl),
                )
            } else {
                (0, 0)
            };
            let suffix = if bl < level {
                match (suffix >> shift).cmp(&dv) {
                    core::cmp::Ordering::Less => return None,
                    core::cmp::Ordering::Equal => key_low(suffix, bl),
                    core::cmp::Ordering::Greater => pow256(bl) - 1,
                }
            } else {
                suffix
            };
            let d = digit(suffix, bl);
            // SAFETY: live branch per contract.
            let (num, digits, edges_ptr) = unsafe {
                if matches!(t, EdgeType::BranchL3) {
                    let b = &*edge.node_ptr().cast::<BranchL3>();
                    (b.hdr.num as usize, b.hdr.digits, b.edges.as_ptr())
                } else {
                    let b = &*edge.node_ptr().cast::<BranchL7>();
                    (b.hdr.num as usize, b.hdr.digits, b.edges.as_ptr())
                }
            };
            for (slot, &bd) in digits.iter().enumerate().take(num).rev() {
                if bd > d {
                    continue;
                }
                let rem = if bd == d {
                    key_low(suffix, bl - 1)
                } else {
                    pow256(bl - 1) - 1
                };
                // SAFETY: slot < num live child edges.
                let child = unsafe { &*edges_ptr.add(slot) };
                // SAFETY: child subtree per contract.
                if let Some((r, v)) = unsafe { prev::<MAP>(child, rem, bl - 1) } {
                    return Some(((dv << shift) | compose(bd, r, bl), v));
                }
            }
            None
        }

        EdgeTag::Structural(EdgeType::BranchB) => {
            // SAFETY: live branch per contract.
            let bl = unsafe { crate::mutate::branch_form_level(edge, EdgeType::BranchB, level) };
            let (dv, shift): (u64, u32) = if bl < level {
                (
                    crate::mutate::decode_value(edge, bl, level),
                    8 * u32::from(bl),
                )
            } else {
                (0, 0)
            };
            let suffix = if bl < level {
                match (suffix >> shift).cmp(&dv) {
                    core::cmp::Ordering::Less => return None,
                    core::cmp::Ordering::Equal => key_low(suffix, bl),
                    core::cmp::Ordering::Greater => pow256(bl) - 1,
                }
            } else {
                suffix
            };
            let d = digit(suffix, bl);
            // SAFETY: live BranchB per contract.
            let b = unsafe { &*edge.node_ptr().cast::<BranchB>() };
            let mut cur = b.bitmap.prev_set(d);
            while let Some(bd) = cur {
                let rem = if bd == d {
                    key_low(suffix, bl - 1)
                } else {
                    pow256(bl - 1) - 1
                };
                let sub = (bd >> 5) as usize;
                let rank = b.bitmap.subexpanse_rank(bd) as usize;
                // SAFETY: bit set → subarray holds rank + 1 edges.
                let child = unsafe { &*b.subarrays[sub].add(rank) };
                // SAFETY: child subtree per contract.
                if let Some((r, v)) = unsafe { prev::<MAP>(child, rem, bl - 1) } {
                    return Some(((dv << shift) | compose(bd, r, bl), v));
                }
                cur = if bd == 0 {
                    None
                } else {
                    b.bitmap.prev_set(bd - 1)
                };
            }
            None
        }

        EdgeTag::Structural(EdgeType::BranchU) => {
            let d = digit(suffix, level);
            // SAFETY: live BranchU per contract.
            let b = unsafe { &*edge.node_ptr().cast::<BranchU>() };
            for bd in (0..=d).rev() {
                let child = &b.edges[bd as usize];
                if child.is_null() {
                    continue;
                }
                let rem = if bd == d {
                    key_low(suffix, level - 1)
                } else {
                    pow256(level - 1) - 1
                };
                // SAFETY: child subtree per contract.
                if let Some((r, v)) = unsafe { prev::<MAP>(child, rem, level - 1) } {
                    return Some((compose(bd, r, level), v));
                }
            }
            None
        }
    }
}

/// Number of keys strictly below `suffix` in the subtree (rank).
///
/// # Safety
///
/// Same contract as [`next`].
pub(crate) unsafe fn count_below<const MAP: bool>(edge: &Edge, suffix: u64, level: u8) -> u64 {
    debug_assert!((1..=8).contains(&level));
    debug_assert!(level == 8 || suffix < pow256(level));
    match edge.tag().expect("valid edge tag") {
        EdgeTag::Structural(EdgeType::Null) => 0,

        EdgeTag::Immed(im) => {
            let keys = immed_suffixes::<MAP>(edge, im);
            keys.partition_point(|&k| k < suffix) as u64
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
            let (dv, shift) = skip_parts(edge, kb, level);
            match (suffix >> shift).cmp(&dv) {
                core::cmp::Ordering::Less => 0,
                core::cmp::Ordering::Equal => {
                    // SAFETY: live leaf of `pop` keys per contract.
                    unsafe { leaf_lower_bound::<MAP>(edge, kb, pop, key_low(suffix, kb)) as u64 }
                }
                core::cmp::Ordering::Greater => pop as u64,
            }
        }

        EdgeTag::Structural(EdgeType::LeafB1) => {
            let (dv, shift) = skip_parts(edge, 1, level);
            match (suffix >> shift).cmp(&dv) {
                core::cmp::Ordering::Less => 0,
                core::cmp::Ordering::Equal => {
                    // Both flavors start with the bitmap at the node base.
                    // SAFETY: live bitmap leaf per contract.
                    let bitmap = unsafe { &(*edge.node_ptr().cast::<LeafBitmap1>()).bitmap };
                    u64::from(bitmap.rank(key_low(suffix, 1) as u8))
                }
                core::cmp::Ordering::Greater => edge.pop0(1) + 1,
            }
        }

        EdgeTag::Structural(EdgeType::FullExpanse) => {
            debug_assert!(!MAP);
            suffix
        }

        EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
            // SAFETY: live branch per contract.
            let bl = unsafe { crate::mutate::branch_form_level(edge, t, level) };
            let (dv, shift): (u64, u32) = if bl < level {
                (
                    crate::mutate::decode_value(edge, bl, level),
                    8 * u32::from(bl),
                )
            } else {
                (0, 0)
            };
            let _ = shift;
            let suffix = if bl < level {
                match (suffix >> shift).cmp(&dv) {
                    core::cmp::Ordering::Less => return 0,
                    core::cmp::Ordering::Equal => key_low(suffix, bl),
                    core::cmp::Ordering::Greater => return edge.pop0(bl) + 1,
                }
            } else {
                suffix
            };
            let d = digit(suffix, bl);
            // SAFETY: live branch per contract.
            let (num, digits, edges_ptr) = unsafe {
                if matches!(t, EdgeType::BranchL3) {
                    let b = &*edge.node_ptr().cast::<BranchL3>();
                    (b.hdr.num as usize, b.hdr.digits, b.edges.as_ptr())
                } else {
                    let b = &*edge.node_ptr().cast::<BranchL7>();
                    (b.hdr.num as usize, b.hdr.digits, b.edges.as_ptr())
                }
            };
            let mut below = 0;
            for (slot, &bd) in digits.iter().enumerate().take(num) {
                // SAFETY: slot < num live child edges.
                let child = unsafe { &*edges_ptr.add(slot) };
                if bd < d {
                    // SAFETY: live child per contract.
                    below += unsafe { edge_pop(child, bl - 1) };
                } else if bd == d {
                    // SAFETY: child subtree per contract.
                    below += unsafe { count_below::<MAP>(child, key_low(suffix, bl - 1), bl - 1) };
                }
            }
            below
        }

        EdgeTag::Structural(EdgeType::BranchB) => {
            // SAFETY: live branch per contract.
            let bl = unsafe { crate::mutate::branch_form_level(edge, EdgeType::BranchB, level) };
            let (dv, shift): (u64, u32) = if bl < level {
                (
                    crate::mutate::decode_value(edge, bl, level),
                    8 * u32::from(bl),
                )
            } else {
                (0, 0)
            };
            let _ = shift;
            let suffix = if bl < level {
                match (suffix >> shift).cmp(&dv) {
                    core::cmp::Ordering::Less => return 0,
                    core::cmp::Ordering::Equal => key_low(suffix, bl),
                    core::cmp::Ordering::Greater => return edge.pop0(bl) + 1,
                }
            } else {
                suffix
            };
            let d = digit(suffix, bl);
            // SAFETY: live BranchB per contract.
            let b = unsafe { &*edge.node_ptr().cast::<BranchB>() };
            let mut below = 0;
            let mut cur = b.bitmap.next_set(0);
            while let Some(bd) = cur {
                if bd > d {
                    break;
                }
                let sub = (bd >> 5) as usize;
                let rank = b.bitmap.subexpanse_rank(bd) as usize;
                // SAFETY: bit set → subarray holds rank + 1 edges.
                let child = unsafe { &*b.subarrays[sub].add(rank) };
                if bd < d {
                    // SAFETY: live child per contract.
                    below += unsafe { edge_pop(child, bl - 1) };
                } else {
                    // SAFETY: child subtree per contract.
                    below += unsafe { count_below::<MAP>(child, key_low(suffix, bl - 1), bl - 1) };
                }
                cur = if bd == 255 {
                    None
                } else {
                    b.bitmap.next_set(bd + 1)
                };
            }
            below
        }

        EdgeTag::Structural(EdgeType::BranchU) => {
            let d = digit(suffix, level);
            // SAFETY: live BranchU per contract.
            let b = unsafe { &*edge.node_ptr().cast::<BranchU>() };
            let mut below = 0;
            for bd in 0..=d {
                let child = &b.edges[bd as usize];
                if child.is_null() {
                    continue;
                }
                if bd < d {
                    // SAFETY: live child per contract.
                    below += unsafe { edge_pop(child, level - 1) };
                } else {
                    // SAFETY: child subtree per contract.
                    below +=
                        unsafe { count_below::<MAP>(child, key_low(suffix, level - 1), level - 1) };
                }
            }
            below
        }
    }
}

/// The key with `n` keys below it (0-based select), with its value.
/// `n` must be below the subtree population.
///
/// # Safety
///
/// Same contract as [`next`].
pub(crate) unsafe fn by_count<const MAP: bool>(edge: &Edge, n: u64, level: u8) -> (u64, u64) {
    debug_assert!((1..=8).contains(&level));
    match edge.tag().expect("valid edge tag") {
        EdgeTag::Structural(EdgeType::Null) => unreachable!("select into empty subtree"),

        EdgeTag::Immed(im) => {
            let keys = immed_suffixes::<MAP>(edge, im);
            let v = if MAP {
                // SAFETY: live map immediate per contract.
                unsafe { immed_value(edge, im, n as usize) }
            } else {
                0
            };
            (keys[n as usize], v)
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
            debug_assert!((n as usize) < pop);
            let (dv, shift) = skip_parts(edge, kb, level);
            // SAFETY: n < pop keys per contract.
            let k = unsafe { leaf_suffix::<MAP>(edge, kb, pop, n as usize) };
            let v = if MAP {
                // SAFETY: map leaves hold pop values at the base.
                unsafe { leaf_value(edge, n as usize) }
            } else {
                0
            };
            ((dv << shift) | k, v)
        }

        EdgeTag::Structural(EdgeType::LeafB1) => {
            let (dv, shift) = skip_parts(edge, 1, level);
            // SAFETY: both flavors start with the bitmap at the node base.
            let bitmap = unsafe { &(*edge.node_ptr().cast::<LeafBitmap1>()).bitmap };
            let d = bitmap.select(n as u32).expect("select within population");
            let v = if MAP {
                // SAFETY: live LeafBitmapL per contract.
                let node = unsafe { &*edge.node_ptr().cast::<LeafBitmapL>() };
                let sub = (d >> 5) as usize;
                let rank = node.bitmap.subexpanse_rank(d) as usize;
                // SAFETY: bit set → value subarray holds rank + 1 values.
                unsafe { *node.values[sub].add(rank) }
            } else {
                0
            };
            ((dv << shift) | u64::from(d), v)
        }

        EdgeTag::Structural(EdgeType::FullExpanse) => {
            debug_assert!(!MAP);
            debug_assert!(n < pow256(level));
            (n, 0)
        }

        EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
            // SAFETY: live branch per contract.
            let bl = unsafe { crate::mutate::branch_form_level(edge, t, level) };
            let (dv, shift): (u64, u32) = if bl < level {
                (
                    crate::mutate::decode_value(edge, bl, level),
                    8 * u32::from(bl),
                )
            } else {
                (0, 0)
            };
            // SAFETY: live branch per contract.
            let (num, digits, edges_ptr) = unsafe {
                if matches!(t, EdgeType::BranchL3) {
                    let b = &*edge.node_ptr().cast::<BranchL3>();
                    (b.hdr.num as usize, b.hdr.digits, b.edges.as_ptr())
                } else {
                    let b = &*edge.node_ptr().cast::<BranchL7>();
                    (b.hdr.num as usize, b.hdr.digits, b.edges.as_ptr())
                }
            };
            let mut n = n;
            for (slot, &bd) in digits.iter().enumerate().take(num) {
                // SAFETY: slot < num live child edges.
                let child = unsafe { &*edges_ptr.add(slot) };
                // SAFETY: live child per contract.
                let pop = unsafe { edge_pop(child, bl - 1) };
                if n < pop {
                    // SAFETY: child subtree per contract.
                    let (r, v) = unsafe { by_count::<MAP>(child, n, bl - 1) };
                    return ((dv << shift) | compose(bd, r, bl), v);
                }
                n -= pop;
            }
            unreachable!("select beyond branch population")
        }

        EdgeTag::Structural(EdgeType::BranchB) => {
            // SAFETY: live branch per contract.
            let bl = unsafe { crate::mutate::branch_form_level(edge, EdgeType::BranchB, level) };
            let (dv, shift): (u64, u32) = if bl < level {
                (
                    crate::mutate::decode_value(edge, bl, level),
                    8 * u32::from(bl),
                )
            } else {
                (0, 0)
            };
            // SAFETY: live BranchB per contract.
            let b = unsafe { &*edge.node_ptr().cast::<BranchB>() };
            let mut n = n;
            let mut cur = b.bitmap.next_set(0);
            while let Some(bd) = cur {
                let sub = (bd >> 5) as usize;
                let rank = b.bitmap.subexpanse_rank(bd) as usize;
                // SAFETY: bit set → subarray holds rank + 1 edges.
                let child = unsafe { &*b.subarrays[sub].add(rank) };
                // SAFETY: live child per contract.
                let pop = unsafe { edge_pop(child, bl - 1) };
                if n < pop {
                    // SAFETY: child subtree per contract.
                    let (r, v) = unsafe { by_count::<MAP>(child, n, bl - 1) };
                    return ((dv << shift) | compose(bd, r, bl), v);
                }
                n -= pop;
                cur = if bd == 255 {
                    None
                } else {
                    b.bitmap.next_set(bd + 1)
                };
            }
            unreachable!("select beyond branch population")
        }

        EdgeTag::Structural(EdgeType::BranchU) => {
            // SAFETY: live BranchU per contract.
            let b = unsafe { &*edge.node_ptr().cast::<BranchU>() };
            let mut n = n;
            for bd in 0..=255u8 {
                let child = &b.edges[bd as usize];
                if child.is_null() {
                    continue;
                }
                // SAFETY: live child per contract.
                let pop = unsafe { edge_pop(child, level - 1) };
                if n < pop {
                    // SAFETY: child subtree per contract.
                    let (r, v) = unsafe { by_count::<MAP>(child, n, level - 1) };
                    return (compose(bd, r, level), v);
                }
                n -= pop;
            }
            unreachable!("select beyond branch population")
        }
    }
}

/// Composes a branch digit with a child-level result suffix.
#[inline]
fn compose(d: u8, rem: u64, level: u8) -> u64 {
    (u64::from(d) << ((level - 1) * 8)) | rem
}
