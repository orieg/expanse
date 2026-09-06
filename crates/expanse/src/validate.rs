//! Defensive trie structure validation and statistics collection.

use crate::alloc::accounted_size;
use crate::mutate::{
    BRANCHB_UP, LEAF_CAP, LEAF1_CAP, LEAFB1_DOWN, branch_form_level, immed_keys, immed_map_keys,
    leaf_keys, map_immed_max, pow256, read_packed, sub_edges_size, sub_vals_size,
};
use crate::mutate_map::map_immed_val_size;
use crate::node::{BranchB, BranchL3, BranchL7, BranchU, Edge, LeafBitmap1, LeafBitmapL};
use crate::types::RAW_ALIGN;
use crate::types::{EdgeTag, EdgeType, ImmedType};
use core::mem::size_of;
#[cfg(not(feature = "std"))]
use core_alloc::format;
#[cfg(not(feature = "std"))]
use core_alloc::string::String;
#[cfg(not(feature = "std"))]
use core_alloc::vec::Vec;

const BRANCH_L3_CAP: usize = 3;
const BRANCH_L7_CAP: usize = 7;

/// Diagnostic statistics for an Expanse trie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpanseStats {
    /// Counts of nodes by their specific form.
    pub node_counts: NodeCounts,
    /// Histogram of node depths (0 to 8).
    pub depth_histogram: [usize; 9],
    /// Histogram of leaf populations (0 to 256).
    pub leaf_pop_histogram: [usize; 257],
    /// Heap bytes attributed to each node form; sums to `mem_used()`.
    pub node_bytes: NodeBytes,
    /// Branch nodes (any form) by the level of the slot holding them.
    /// `branch_depth_histogram[6]` is the number of 2-byte-prefix expanses
    /// that have cascaded past `LEAF_CAP`; `[5]` the sub-expanses below them
    /// that have cascaded in turn.
    pub branch_depth_histogram: [usize; 9],
    /// Linear and bitmap leaves (not immediates) by slot level; index 0 is
    /// the root leaf.
    pub leaf_depth_histogram: [usize; 9],
}

impl Default for ExpanseStats {
    fn default() -> Self {
        Self {
            node_counts: NodeCounts::default(),
            depth_histogram: [0; 9],
            leaf_pop_histogram: [0; 257],
            node_bytes: NodeBytes::default(),
            branch_depth_histogram: [0; 9],
            leaf_depth_histogram: [0; 9],
        }
    }
}

/// Heap bytes attributed to each node form.
///
/// Every allocation the engine makes is charged to the form that owns it:
/// a bitmap branch's packed edge subarrays count under `branch_b`, a
/// map-flavor bitmap leaf's value subarrays under `leaf_bitmap`, and the
/// value array behind a multi-key map immediate under `immed_values`. Edges
/// themselves live inside their parent and are charged there, so immediates
/// of the set flavor cost nothing here. The sum is exactly the engine's
/// `mem_used()` (asserted by `tests::node_bytes_sum_to_mem_used`), which is
/// what makes the breakdown a decomposition of a published number rather
/// than an estimate beside it.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NodeBytes {
    /// Value arrays behind map-flavor immediates holding two or more keys.
    pub immed_values: usize,
    /// Packed linear leaves (including a root leaf).
    pub leaf_linear: usize,
    /// Bitmap leaves, plus the value subarrays of the map flavor.
    pub leaf_bitmap: usize,
    /// Linear branches of up to 3 children.
    pub branch_l3: usize,
    /// Linear branches of up to 7 children.
    pub branch_l7: usize,
    /// Bitmap branches, plus their packed edge subarrays.
    pub branch_b: usize,
    /// Uncompressed branches.
    pub branch_u: usize,
}

/// What `alloc_bytes(bytes)` charges: the request rounded to [`RAW_ALIGN`].
const fn raw(bytes: usize) -> usize {
    accounted_size(bytes, RAW_ALIGN)
}

impl NodeBytes {
    /// Total bytes across every form.
    #[must_use]
    pub fn total(&self) -> usize {
        self.immed_values
            + self.leaf_linear
            + self.leaf_bitmap
            + self.branch_l3
            + self.branch_l7
            + self.branch_b
            + self.branch_u
    }
}

/// Counts of nodes by their structural or immediate form.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NodeCounts {
    /// NULL edges (empty slots).
    pub null: usize,
    /// Immed edges (key bytes stored inside the edge pointer).
    pub immed: usize,
    /// Packed linear leaf nodes.
    pub leaf_linear: usize,
    /// Bitmap leaf nodes.
    pub leaf_bitmap: usize,
    /// Linear branch nodes (up to 3 children).
    pub branch_l3: usize,
    /// Linear branch nodes (up to 7 children).
    pub branch_l7: usize,
    /// Bitmap branch nodes.
    pub branch_b: usize,
    /// Uncompressed branch nodes.
    pub branch_u: usize,
    /// Full expanse edges (set-flavor only).
    pub full_expanse: usize,
}

/// Recursively validates the subtree under `edge` at `level`, gathering statistics defensively.
///
/// Returns the total population (number of keys) in the subtree on success.
///
/// # Safety
///
/// Safe to call on corrupt trees: validates tag values, alignment, non-null,
/// level bounds, and limits depth to prevent stack overflow from cycles.
pub fn expanse_validate_and_stats<const MAP: bool>(
    edge: &Edge,
    level: u8,
    stats: &mut ExpanseStats,
    depth: usize,
) -> Result<u64, String> {
    if depth > 8 {
        return Err("depth limit exceeded (possible cycle)".into());
    }
    if !(1..=8).contains(&level) {
        return Err(format!("level {level} out of range"));
    }

    let tag = match edge.tag() {
        Some(t) => t,
        None => return Err("invalid edge tag byte".into()),
    };

    // Update stats for the edge tag form
    match tag {
        EdgeTag::Structural(EdgeType::Null) => stats.node_counts.null += 1,
        EdgeTag::Immed(_) => stats.node_counts.immed += 1,
        EdgeTag::Structural(
            EdgeType::Leaf1
            | EdgeType::Leaf2
            | EdgeType::Leaf3
            | EdgeType::Leaf4
            | EdgeType::Leaf5
            | EdgeType::Leaf6
            | EdgeType::Leaf7,
        ) => stats.node_counts.leaf_linear += 1,
        EdgeTag::Structural(EdgeType::LeafB1) => stats.node_counts.leaf_bitmap += 1,
        EdgeTag::Structural(EdgeType::FullExpanse) => stats.node_counts.full_expanse += 1,
        EdgeTag::Structural(EdgeType::BranchL3) => stats.node_counts.branch_l3 += 1,
        EdgeTag::Structural(EdgeType::BranchL7) => stats.node_counts.branch_l7 += 1,
        EdgeTag::Structural(EdgeType::BranchB) => stats.node_counts.branch_b += 1,
        EdgeTag::Structural(EdgeType::BranchU) => stats.node_counts.branch_u += 1,
    }

    stats.depth_histogram[level as usize] += 1;
    match tag {
        EdgeTag::Structural(
            EdgeType::BranchL3 | EdgeType::BranchL7 | EdgeType::BranchB | EdgeType::BranchU,
        ) => stats.branch_depth_histogram[level as usize] += 1,
        EdgeTag::Structural(
            EdgeType::Leaf1
            | EdgeType::Leaf2
            | EdgeType::Leaf3
            | EdgeType::Leaf4
            | EdgeType::Leaf5
            | EdgeType::Leaf6
            | EdgeType::Leaf7
            | EdgeType::LeafB1,
        ) => stats.leaf_depth_histogram[level as usize] += 1,
        _ => {}
    }

    // Branch form level: below the slot level only behind a narrow pointer
    let bl = match tag {
        EdgeTag::Structural(
            t @ (EdgeType::BranchL3 | EdgeType::BranchL7 | EdgeType::BranchB | EdgeType::BranchU),
        ) => {
            // SAFETY: validated branch tag
            let bl = unsafe { branch_form_level(edge, t, level) };
            if bl < 2 {
                return Err("branch below level 2".into());
            }
            if bl > level {
                return Err("branch form level above its slot level".into());
            }
            if level == 8 && bl != level {
                return Err("level-8 slots cannot skip".into());
            }
            if matches!(t, EdgeType::BranchU) && bl != level {
                return Err("uncompressed branches never skip".into());
            }
            bl
        }
        _ => level,
    };

    let pop = match tag {
        EdgeTag::Structural(EdgeType::Null) => 0,
        EdgeTag::Immed(im) => {
            if im.key_bytes() != level {
                return Err(format!(
                    "immediate key size {} must equal level {}",
                    im.key_bytes(),
                    level
                ));
            }
            let keys = if MAP {
                if im.key_count() as usize > map_immed_max(im.key_bytes()) {
                    return Err("map immediate above aux capacity".into());
                }
                // A one-key map immediate keeps its value in the edge; two or
                // more spill into a class-sized heap array.
                if im.key_count() >= 2 {
                    stats.node_bytes.immed_values +=
                        raw(map_immed_val_size(im.key_count() as usize));
                }
                immed_map_keys(edge, im)
            } else {
                immed_keys(edge, im)
            };
            if !keys.windows(2).all(|w| w[0] < w[1]) {
                return Err("immediate keys unsorted".into());
            }
            stats.leaf_pop_histogram[keys.len()] += 1;
            keys.len() as u64
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
            let kb = match t.leaf_key_bytes() {
                Some(k) => k,
                None => return Err("invalid leaf tag".into()),
            };
            if kb > level {
                return Err("leaf key size above its slot level".into());
            }
            let pop = edge.pop0(kb) + 1;
            let cap = if kb == 1 { LEAF1_CAP } else { LEAF_CAP };
            if pop as usize > cap {
                return Err(format!("linear leaf population {pop} above capacity {cap}"));
            }
            let floor = if MAP {
                map_immed_max(level)
            } else {
                ImmedType::max_count(level) as usize
            };
            if (pop as usize) < floor {
                return Err(format!(
                    "leaf population {pop} below immediate hysteresis floor {floor}"
                ));
            }
            let ptr = edge.node_ptr();
            if ptr.is_null() {
                return Err("leaf edge has null node pointer".into());
            }
            if !(ptr as usize).is_multiple_of(16) {
                return Err(format!("linear leaf pointer {ptr:p} not 16-byte aligned"));
            }
            let keys = if MAP {
                let offset = crate::leaf::map_keys_offset(pop as usize);
                // SAFETY: ptr is non-null, aligned, and offset is within the leaf layout.
                (0..pop as usize)
                    .map(|slot| unsafe { read_packed(ptr.add(offset), slot, kb as usize) })
                    .collect::<Vec<_>>()
            } else {
                // SAFETY: ptr is checked non-null and aligned.
                unsafe { leaf_keys(edge, kb, pop as usize) }
            };
            if !keys.windows(2).all(|w| w[0] < w[1]) {
                return Err("leaf keys unsorted".into());
            }
            stats.leaf_pop_histogram[pop as usize] += 1;
            stats.node_bytes.leaf_linear += raw(if MAP {
                crate::leaf::size_map(kb, pop as usize)
            } else {
                crate::leaf::size_set(kb, pop as usize)
            });
            pop
        }
        EdgeTag::Structural(EdgeType::LeafB1) => {
            let ptr = edge.node_ptr();
            if ptr.is_null() {
                return Err("LeafB1 has null node pointer".into());
            }
            let count = if MAP {
                if !(ptr as usize).is_multiple_of(64) {
                    return Err(format!("LeafBitmapL pointer {ptr:p} not 64-byte aligned"));
                }
                // SAFETY: ptr is non-null and 64-byte aligned LeafBitmapL.
                let node = unsafe { &*ptr.cast::<LeafBitmapL>() };
                stats.node_bytes.leaf_bitmap += size_of::<LeafBitmapL>();
                for sub in 0..8 {
                    let n = node.bitmap.subexpanse_count(sub) as usize;
                    if (node.values[sub].is_null()) != (n == 0) {
                        return Err("value subarray/bitmap disagreement in LeafBitmapL".into());
                    }
                    if n > 0 && !(node.values[sub] as usize).is_multiple_of(16) {
                        return Err(format!(
                            "LeafBitmapL value subarray {sub} pointer not 16-byte aligned"
                        ));
                    }
                    if n > 0 {
                        stats.node_bytes.leaf_bitmap += raw(sub_vals_size(n));
                    }
                }
                u64::from(node.bitmap.count())
            } else {
                if !(ptr as usize).is_multiple_of(64) {
                    return Err(format!("LeafBitmap1 pointer {ptr:p} not 64-byte aligned"));
                }
                // SAFETY: ptr is non-null and 64-byte aligned LeafBitmap1.
                let node = unsafe { &*ptr.cast::<LeafBitmap1>() };
                stats.node_bytes.leaf_bitmap += size_of::<LeafBitmap1>();
                u64::from(node.bitmap.count())
            };
            if edge.pop0(1) + 1 != count {
                return Err(format!(
                    "bitmap-leaf pop0 {} disagrees with bitmap count {}",
                    edge.pop0(1) + 1,
                    count
                ));
            }
            if (count as usize) < LEAFB1_DOWN {
                return Err(format!(
                    "bitmap leaf population {count} below hysteresis floor {LEAFB1_DOWN}"
                ));
            }
            stats.leaf_pop_histogram[count as usize] += 1;
            count
        }
        EdgeTag::Structural(EdgeType::FullExpanse) => {
            if MAP {
                return Err("full-expanse edges are set-flavor only".into());
            }
            pow256(level)
        }
        EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
            let is_l3 = matches!(t, EdgeType::BranchL3);
            let ptr = edge.node_ptr();
            if ptr.is_null() {
                return Err("branch edge has null node pointer".into());
            }
            if !(ptr as usize).is_multiple_of(64) {
                return Err(format!("linear branch pointer {ptr:p} not 64-byte aligned"));
            }
            // SAFETY: ptr is non-null and 64-byte aligned BranchL3/L7.
            let (num, digits, edges): (usize, [u8; 8], Vec<Edge>) = unsafe {
                if is_l3 {
                    let b = &*ptr.cast::<BranchL3>();
                    (b.hdr.num as usize, b.hdr.digits, b.edges.to_vec())
                } else {
                    let b = &*ptr.cast::<BranchL7>();
                    (b.hdr.num as usize, b.hdr.digits, b.edges.to_vec())
                }
            };
            let cap = if is_l3 { BRANCH_L3_CAP } else { BRANCH_L7_CAP };
            if is_l3 {
                stats.node_bytes.branch_l3 += size_of::<BranchL3>();
            } else {
                stats.node_bytes.branch_l7 += size_of::<BranchL7>();
            }
            if num < 1 || num > cap {
                return Err(format!(
                    "linear branch count {num} out of range (1..={cap})"
                ));
            }
            if !digits[..num].windows(2).all(|w| w[0] < w[1]) {
                return Err("linear branch digits unsorted".into());
            }
            let mut pop = 0;
            for child in edges.iter().take(num) {
                if child.is_null() {
                    return Err("linear branch holds a null child".into());
                }
                pop += expanse_validate_and_stats::<MAP>(child, bl - 1, stats, depth + 1)?;
            }
            pop
        }
        EdgeTag::Structural(EdgeType::BranchB) => {
            let ptr = edge.node_ptr();
            if ptr.is_null() {
                return Err("BranchB edge has null node pointer".into());
            }
            if !(ptr as usize).is_multiple_of(64) {
                return Err(format!("BranchB pointer {ptr:p} not 64-byte aligned"));
            }
            // SAFETY: ptr is non-null and 64-byte aligned BranchB.
            let b = unsafe { &*ptr.cast::<BranchB>() };
            stats.node_bytes.branch_b += size_of::<BranchB>();
            let digits = b.bitmap.count() as usize;
            if digits < BRANCH_L7_CAP {
                return Err(format!(
                    "bitmap branch population {digits} below hysteresis floor {BRANCH_L7_CAP}"
                ));
            }
            if digits > BRANCHB_UP {
                return Err(format!(
                    "bitmap branch population {digits} above uncompressed threshold {BRANCHB_UP}"
                ));
            }
            let mut pop = 0;
            for sub in 0..8usize {
                let expected = (0..32u8)
                    .filter(|i| b.bitmap.test((sub * 32) as u8 + i))
                    .count();
                if b.pop_counts[sub] as usize != expected {
                    return Err(format!(
                        "bitmap-branch rank cache {} disagrees with bitmap {}",
                        b.pop_counts[sub], expected
                    ));
                }
                if expected == 0 {
                    if !b.subarrays[sub].is_null() {
                        return Err(format!("empty subexpanse {sub} with non-null subarray"));
                    }
                } else {
                    let sub_ptr = b.subarrays[sub];
                    if sub_ptr.is_null() {
                        return Err(format!("non-empty subexpanse {sub} with null subarray"));
                    }
                    stats.node_bytes.branch_b += raw(sub_edges_size(expected));
                    if !(sub_ptr as usize).is_multiple_of(16) {
                        return Err(format!(
                            "BranchB subarray {sub} pointer not 16-byte aligned"
                        ));
                    }
                    for i in 0..expected {
                        // SAFETY: sub_ptr is non-null, 16-byte aligned, and index i is in bounds.
                        let child = unsafe { &*sub_ptr.add(i) };
                        if child.is_null() {
                            return Err(format!(
                                "bitmap branch subarray {sub} index {i} holds a null child"
                            ));
                        }
                        pop += expanse_validate_and_stats::<MAP>(child, bl - 1, stats, depth + 1)?;
                    }
                }
            }
            pop
        }
        EdgeTag::Structural(EdgeType::BranchU) => {
            let ptr = edge.node_ptr();
            if ptr.is_null() {
                return Err("BranchU edge has null node pointer".into());
            }
            if !(ptr as usize).is_multiple_of(64) {
                return Err(format!("BranchU pointer {ptr:p} not 64-byte aligned"));
            }
            // SAFETY: ptr is non-null and 64-byte aligned BranchU.
            let b = unsafe { &*ptr.cast::<BranchU>() };
            stats.node_bytes.branch_u += size_of::<BranchU>();
            let digits = b.edges.iter().filter(|e| !e.is_null()).count();
            if digits < BRANCHB_UP {
                return Err(format!(
                    "uncompressed branch population {digits} below hysteresis floor {BRANCHB_UP}"
                ));
            }
            let mut pop = 0;
            for child in &b.edges {
                if !child.is_null() {
                    pop += expanse_validate_and_stats::<MAP>(child, bl - 1, stats, depth + 1)?;
                }
            }
            pop
        }
    };

    if bl <= 7
        && matches!(
            tag,
            EdgeTag::Structural(
                EdgeType::BranchL3 | EdgeType::BranchL7 | EdgeType::BranchB | EdgeType::BranchU
            )
        )
        && edge.pop0(bl) + 1 != pop
    {
        return Err(format!(
            "branch pop0 disagrees with subtree: {} != {}",
            edge.pop0(bl) + 1,
            pop
        ));
    }
    Ok(pop)
}

#[cfg(test)]
mod tests {
    use crate::map::ExpanseMap;
    use crate::set::ExpanseSet;

    /// The `bytes_per_key` census generators, so every node form the
    /// committed density cells exercise (packed leaves, cascaded bitmap
    /// branches, uncompressed branches, bitmap leaves, root leaves) is
    /// covered by the exactness check.
    fn keys(dist: &str, n: usize) -> impl Iterator<Item = u64> {
        let mut rng = 0x0DDB_1A5E_5EED_0001u64;
        let mut base = 0u64;
        let mut next = move || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        let dist = dist.to_owned();
        (0..n as u64).map(move |i| match dist.as_str() {
            "sequential" => i,
            "random" => next(),
            "random62" => next() & ((1u64 << 62) - 1),
            "clustered" => {
                if i % 256 == 0 {
                    base = next() & !0xFF;
                }
                base + (i % 256)
            }
            "sparse" => i << 40,
            _ => unreachable!(),
        })
    }

    /// `NodeBytes` is a decomposition of `mem_used()`, not an estimate of it:
    /// the per-form sum must reproduce the allocator's live byte count exactly,
    /// on both flavors, across every distribution and across the root-leaf,
    /// packed-leaf and cascaded regimes. A drift here means an allocation the
    /// walk does not charge to any form.
    #[test]
    fn node_bytes_sum_to_mem_used() {
        for dist in ["sequential", "random", "random62", "clustered", "sparse"] {
            // The two largest populations exist for the branch forms that only
            // appear past ~50k keys; under Miri they are the whole cost of the
            // test (400k interpreted inserts per distribution), so the
            // interpreter stops at 5,000 and the native job runs all eleven.
            let sizes: &[usize] = if cfg!(miri) {
                &[0, 1, 2, 5, 31, 32, 33, 300, 5_000]
            } else {
                &[0, 1, 2, 5, 31, 32, 33, 300, 5_000, 60_000, 200_000]
            };
            for &n in sizes {
                let mut set = ExpanseSet::new();
                let mut map = ExpanseMap::new();
                for k in keys(dist, n) {
                    set.insert(k);
                    map.insert(k, !k);
                }
                let s = set.stats();
                assert_eq!(
                    s.node_bytes.total(),
                    set.mem_used(),
                    "set {dist} n={n}: {:?}",
                    s.node_bytes
                );
                let m = map.stats();
                assert_eq!(
                    m.node_bytes.total(),
                    map.mem_used(),
                    "map {dist} n={n}: {:?}",
                    m.node_bytes
                );
            }
        }
    }
}
