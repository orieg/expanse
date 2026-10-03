//! Defensive trie structure validation and statistics collection.

use crate::alloc::accounted_size;
use crate::mutate::{
    BRANCHB_UP, LEAF_CAP, LEAF1_CAP, LEAFB1_DOWN, branch_form_level, immed_keys, immed_map_keys,
    leaf_keys, map_immed_max, pow256, read_packed, sub_edges_size, sub_vals_size,
};
use crate::mutate_map::map_immed_val_size;
use crate::node::{BranchB, BranchL3, BranchL7, BranchU, Edge, LeafBitmap1, LeafBitmapL};
use crate::types::{
    BRANCH_L3_CAP, BRANCH_L7_CAP, BRANCHB_TO_L7_DOWN, BRANCHU_TO_B_DOWN, RAW_ALIGN,
};
use crate::types::{EdgeTag, EdgeType, ImmedType};
use core::mem::size_of;
#[cfg(not(feature = "std"))]
use core_alloc::format;
#[cfg(not(feature = "std"))]
use core_alloc::string::String;
#[cfg(not(feature = "std"))]
use core_alloc::vec::Vec;

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

impl core::ops::AddAssign<&NodeBytes> for NodeBytes {
    fn add_assign(&mut self, rhs: &NodeBytes) {
        self.immed_values += rhs.immed_values;
        self.leaf_linear += rhs.leaf_linear;
        self.leaf_bitmap += rhs.leaf_bitmap;
        self.branch_l3 += rhs.branch_l3;
        self.branch_l7 += rhs.branch_l7;
        self.branch_b += rhs.branch_b;
        self.branch_u += rhs.branch_u;
    }
}

impl core::ops::AddAssign<NodeBytes> for NodeBytes {
    fn add_assign(&mut self, rhs: NodeBytes) {
        *self += &rhs;
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

impl core::ops::AddAssign<&NodeCounts> for NodeCounts {
    fn add_assign(&mut self, rhs: &NodeCounts) {
        self.null += rhs.null;
        self.immed += rhs.immed;
        self.leaf_linear += rhs.leaf_linear;
        self.leaf_bitmap += rhs.leaf_bitmap;
        self.branch_l3 += rhs.branch_l3;
        self.branch_l7 += rhs.branch_l7;
        self.branch_b += rhs.branch_b;
        self.branch_u += rhs.branch_u;
        self.full_expanse += rhs.full_expanse;
    }
}

impl core::ops::AddAssign<NodeCounts> for NodeCounts {
    fn add_assign(&mut self, rhs: NodeCounts) {
        *self += &rhs;
    }
}

/// Diagnostic statistics for an [`crate::strmap::ExpanseStrMap`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrMapStats {
    /// Counts of nodes by their specific form across all sub-maps.
    pub node_counts: NodeCounts,
    /// Total count of `StrNode` shells.
    pub str_nodes: usize,
    /// Total count of suffix leaves.
    pub suffix_leaves: usize,
    /// Histogram of sub-map node depths (0 to 8).
    pub depth_histogram: [usize; 9],
    /// Histogram of sub-map leaf populations (0 to 256).
    pub leaf_pop_histogram: [usize; 257],
    /// Heap bytes attributed to each component; sums to `mem_used()`.
    pub node_bytes: StrMapNodeBytes,
    /// Branch nodes by the level of the slot holding them across all sub-maps.
    pub branch_depth_histogram: [usize; 9],
    /// Linear and bitmap leaves by slot level across all sub-maps.
    pub leaf_depth_histogram: [usize; 9],
}

impl Default for StrMapStats {
    fn default() -> Self {
        Self {
            node_counts: NodeCounts::default(),
            str_nodes: 0,
            suffix_leaves: 0,
            depth_histogram: [0; 9],
            leaf_pop_histogram: [0; 257],
            node_bytes: StrMapNodeBytes::default(),
            branch_depth_histogram: [0; 9],
            leaf_depth_histogram: [0; 9],
        }
    }
}

/// Heap bytes attributed to each component of an [`crate::strmap::ExpanseStrMap`].
///
/// Sums exactly to `mem_used()`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StrMapNodeBytes {
    /// Heap bytes attributed to each node form across all sub-maps.
    pub sub_maps: NodeBytes,
    /// Heap bytes of the `StrNode` shells (`size_of::<StrNode>()` each).
    pub node_shells: usize,
    /// Heap bytes of suffix leaves.
    pub suffixes: usize,
}

impl StrMapNodeBytes {
    /// Total bytes across every component; equals `mem_used()`.
    #[must_use]
    pub fn total(&self) -> usize {
        self.sub_maps.total() + self.node_shells + self.suffixes
    }

    /// Suffix leaf bytes (alias for [`Self::suffixes`]).
    #[must_use]
    pub fn suffix_bytes(&self) -> usize {
        self.suffixes
    }

    /// Node shell bytes (alias for [`Self::node_shells`]).
    #[must_use]
    pub fn node_shell_bytes(&self) -> usize {
        self.node_shells
    }
}

/// Diagnostic statistics for an [`crate::bytesmap::ExpanseBytesMap`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytesMapStats {
    /// Counts of nodes by their specific form in the hash trie.
    pub node_counts: NodeCounts,
    /// Total count of collision buckets (hash trie entries).
    pub buckets: usize,
    /// Total count of key-value entries across all buckets.
    pub entries: u64,
    /// Histogram of hash trie node depths (0 to 8).
    pub depth_histogram: [usize; 9],
    /// Histogram of hash trie leaf populations (0 to 256).
    pub leaf_pop_histogram: [usize; 257],
    /// Heap bytes attributed to each component; sums to `mem_used()`.
    pub node_bytes: BytesMapNodeBytes,
    /// Branch nodes by the level of the slot holding them in the hash trie.
    pub branch_depth_histogram: [usize; 9],
    /// Linear and bitmap leaves by slot level in the hash trie.
    pub leaf_depth_histogram: [usize; 9],
}

impl Default for BytesMapStats {
    fn default() -> Self {
        Self {
            node_counts: NodeCounts::default(),
            buckets: 0,
            entries: 0,
            depth_histogram: [0; 9],
            leaf_pop_histogram: [0; 257],
            node_bytes: BytesMapNodeBytes::default(),
            branch_depth_histogram: [0; 9],
            leaf_depth_histogram: [0; 9],
        }
    }
}

/// Heap bytes attributed to each component of an [`crate::bytesmap::ExpanseBytesMap`].
///
/// Sums exactly to `mem_used()`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BytesMapNodeBytes {
    /// Heap bytes attributed to each node form of the hash trie.
    pub sub_maps: NodeBytes,
    /// Heap bytes of the collision bucket shells.
    pub node_shells: usize,
    /// Heap bytes of entry structures and key buffers in collision buckets.
    pub terminal_bytes: usize,
}

impl BytesMapNodeBytes {
    /// Total bytes across every component; equals `mem_used()`.
    #[must_use]
    pub fn total(&self) -> usize {
        self.sub_maps.total() + self.node_shells + self.terminal_bytes
    }

    /// Bucket shell bytes (alias for [`Self::node_shells`]).
    #[must_use]
    pub fn bucket_shells(&self) -> usize {
        self.node_shells
    }

    /// Terminal bytes (alias for [`Self::terminal_bytes`]).
    #[must_use]
    pub fn terminals(&self) -> usize {
        self.terminal_bytes
    }

    /// Node shell bytes (alias for [`Self::node_shells`]).
    #[must_use]
    pub fn node_shell_bytes(&self) -> usize {
        self.node_shells
    }

    /// Hash trie node bytes (alias for [`Self::sub_maps`]).
    #[must_use]
    pub fn map(&self) -> &NodeBytes {
        &self.sub_maps
    }

    /// Hash trie node bytes (alias for [`Self::sub_maps`]).
    #[must_use]
    pub fn trie(&self) -> &NodeBytes {
        &self.sub_maps
    }
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
            if digits <= BRANCHB_TO_L7_DOWN {
                return Err(format!(
                    "bitmap branch population {digits} at or below demotion threshold \
                     {BRANCHB_TO_L7_DOWN}"
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
            // The header child count, when the ablation keeps one (#1202).
            #[cfg(feature = "ablation-branchu-header-count")]
            {
                // SAFETY: the live branch above; the validator runs with no
                // writer, so the plain load races nothing.
                let w = unsafe { BranchU::child_count::<false>(ptr.cast()) };
                let count = (w & !crate::mutate::BRANCH_U_COUNT_VALID) as usize;
                // A branch that no shared writer has counted yet has no count.
                if w & crate::mutate::BRANCH_U_COUNT_VALID != 0 && count != digits {
                    return Err(format!(
                        "uncompressed branch header child count {count} differs from its \
                         {digits} non-null slots"
                    ));
                }
            }
            if crate::mutate::branch_u_below_floor(digits) {
                return Err(format!(
                    "uncompressed branch population {digits} at or below demotion threshold \
                     {BRANCHU_TO_B_DOWN}"
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

// ---------------------------------------------------------------------------
// Dependent node visits per probe (Refs #1249)
// ---------------------------------------------------------------------------

const IMM_MASKS: [u64; 8] = [
    0,
    0x0000_0000_0000_00FF,
    0x0000_0000_0000_FFFF,
    0x0000_0000_00FF_FFFF,
    0x0000_0000_FFFF_FFFF,
    0x0000_00FF_FFFF_FFFF,
    0x0000_FFFF_FFFF_FFFF,
    0x00FF_FFFF_FFFF_FFFF,
];

#[inline(always)]
fn immed_find(im: ImmedType, payload: &[u8], key: u64) -> Option<usize> {
    match im.key_bytes() {
        1 => immed_find_fixed::<1>(im, payload, key),
        2 => immed_find_fixed::<2>(im, payload, key),
        3 => immed_find_fixed::<3>(im, payload, key),
        4 => immed_find_fixed::<4>(im, payload, key),
        5 => immed_find_fixed::<5>(im, payload, key),
        6 => immed_find_fixed::<6>(im, payload, key),
        _ => immed_find_fixed::<7>(im, payload, key),
    }
}

#[inline(always)]
fn immed_find_fixed<const KB: usize>(im: ImmedType, payload: &[u8], key: u64) -> Option<usize> {
    let n = im.key_count() as usize;
    let needle = crate::mutate::key_low(key, KB as u8);
    let ptr = payload.as_ptr();
    // SAFETY: payload holds at least n * KB readable bytes per ImmedType invariant and i < n.
    (0..n).find(|&i| unsafe { crate::mutate::read_packed_fixed::<KB>(ptr, i) } == needle)
}

/// Deterministic breakdown of dependent node visits during a probe descent.
///
/// Diagnostic instrument (issue #1249); outside the hot path. Not a stable API.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProbeVisits {
    /// Number of edges followed during descent (e.g. from root edge or branch child edges).
    pub edges_followed: usize,
    /// Number of BranchB subarray loads incurred.
    pub branch_b_subarrays: usize,
    /// Number of leaf loads (root leaf, linear leaf, bitmap leaf, or multi-key immediate value array).
    pub leaf_loads: usize,
    /// Whether the key was found in the container.
    pub found: bool,
    /// Value found for the key, if present in a map; identical to `get(key)`.
    pub value: Option<u64>,
}

impl ProbeVisits {
    /// Total dependent node visits: edges followed + BranchB subarray loads + leaf loads.
    #[inline]
    #[must_use]
    pub const fn total_visits(&self) -> usize {
        self.edges_followed + self.branch_b_subarrays + self.leaf_loads
    }
}

/// Walks a map-flavor subtree mirroring `get`, counting dependent node visits.
///
/// Diagnostic walker (Refs #1249); outside the hot path.
/// If `SKIP_BRANCHB_SUBARRAY` is true (scanner-mutation negative control),
/// the BranchB subarray load is not counted.
#[doc(hidden)]
pub unsafe fn walk_map_probe_visits_impl<const SKIP_BRANCHB_SUBARRAY: bool>(
    edge: &Edge,
    key: u64,
    level: u8,
) -> ProbeVisits {
    let mut edge = edge;
    let mut level = level;
    let mut edges_followed = 0;
    let mut branch_b_subarrays = 0;
    let mut leaf_loads = 0;

    loop {
        debug_assert!((1..=8).contains(&level));
        let tag = edge.tag_byte();

        match tag {
            0x00 => {
                return ProbeVisits {
                    edges_followed,
                    branch_b_subarrays,
                    leaf_loads,
                    found: false,
                    value: None,
                };
            }

            0x01 => {
                edges_followed += 1;
                // SAFETY: pointer-tagged edge → live BranchL3.
                let b = unsafe { &*edge.node_ptr().cast::<BranchL3>() };
                let bl = b.hdr.level;
                if bl < level && !crate::get::decode_matches(edge, key, bl, level) {
                    return ProbeVisits {
                        edges_followed,
                        branch_b_subarrays,
                        leaf_loads,
                        found: false,
                        value: None,
                    };
                }
                let d = crate::types::digit(key, bl);
                let num = b.hdr.num as usize;
                let slot = if b.hdr.digits[0] == d {
                    0
                } else if num > 1 && b.hdr.digits[1] == d {
                    1
                } else if num > 2 && b.hdr.digits[2] == d {
                    2
                } else {
                    return ProbeVisits {
                        edges_followed,
                        branch_b_subarrays,
                        leaf_loads,
                        found: false,
                        value: None,
                    };
                };
                // SAFETY: `slot` is within bounds of `b.edges`.
                edge = unsafe { &*b.edges.as_ptr().add(slot) };
                level = bl - 1;
            }

            0x02 => {
                edges_followed += 1;
                // SAFETY: pointer-tagged edge → live BranchL7.
                let b = unsafe { &*edge.node_ptr().cast::<BranchL7>() };
                let bl = b.hdr.level;
                if bl < level && !crate::get::decode_matches(edge, key, bl, level) {
                    return ProbeVisits {
                        edges_followed,
                        branch_b_subarrays,
                        leaf_loads,
                        found: false,
                        value: None,
                    };
                }
                let d = crate::types::digit(key, bl);
                let Some(slot) = b.hdr.find(d) else {
                    return ProbeVisits {
                        edges_followed,
                        branch_b_subarrays,
                        leaf_loads,
                        found: false,
                        value: None,
                    };
                };
                debug_assert!(slot < b.edges.len());
                // SAFETY: `slot` is within bounds of `b.edges`.
                edge = unsafe { &*b.edges.as_ptr().add(slot) };
                level = bl - 1;
            }

            0x03 => {
                edges_followed += 1;
                // SAFETY: pointer-tagged edge → live BranchB.
                let b = unsafe { &*edge.node_ptr().cast::<BranchB>() };
                let bl = b.level;
                if bl < level && !crate::get::decode_matches(edge, key, bl, level) {
                    return ProbeVisits {
                        edges_followed,
                        branch_b_subarrays,
                        leaf_loads,
                        found: false,
                        value: None,
                    };
                }
                let d = crate::types::digit(key, bl);
                let Some((sub, slot)) = b.bitmap.test_and_subexpanse_rank_with_sub(d) else {
                    return ProbeVisits {
                        edges_followed,
                        branch_b_subarrays,
                        leaf_loads,
                        found: false,
                        value: None,
                    };
                };
                if !SKIP_BRANCHB_SUBARRAY {
                    branch_b_subarrays += 1;
                }
                // SAFETY: `sub < 8` accesses a valid subarray pointer.
                let sub_ptr = unsafe { *b.subarrays.as_ptr().add(sub) };
                // SAFETY: slot is the verified rank inside the live subexpanse subarray.
                edge = unsafe { &*sub_ptr.add(slot) };
                level = bl - 1;
            }

            0x04 => {
                edges_followed += 1;
                let mut b_ptr = edge.node_ptr().cast::<BranchU>();
                loop {
                    let d = crate::types::digit(key, level);
                    // SAFETY: pointer-tagged edge → live BranchU with 256 edges.
                    let next_edge = unsafe { &*(*b_ptr).edges.as_ptr().add(d as usize) };
                    level -= 1;
                    let next_tag = next_edge.tag_byte();
                    if next_tag == 0x04 {
                        edges_followed += 1;
                        b_ptr = next_edge.node_ptr().cast::<BranchU>();
                    } else {
                        edge = next_edge;
                        break;
                    }
                }
            }

            0x0C => {
                if level > 1 && !crate::get::decode_matches(edge, key, 1, level) {
                    return ProbeVisits {
                        edges_followed,
                        branch_b_subarrays,
                        leaf_loads,
                        found: false,
                        value: None,
                    };
                }
                leaf_loads += 1;
                let d = (key & 0xFF) as u8;
                // SAFETY: pointer-tagged edge → live LeafBitmapL.
                let l = unsafe { &*edge.node_ptr().cast::<LeafBitmapL>() };
                let Some((sub, slot)) = l.bitmap.test_and_subexpanse_rank_with_sub(d) else {
                    return ProbeVisits {
                        edges_followed,
                        branch_b_subarrays,
                        leaf_loads,
                        found: false,
                        value: None,
                    };
                };
                // SAFETY: `sub < 8` accesses a valid values subarray pointer.
                let vals = unsafe { *l.values.as_ptr().add(sub) };
                // SAFETY: `slot` is verified rank inside the values subarray.
                let val = unsafe { *vals.add(slot) };
                return ProbeVisits {
                    edges_followed,
                    branch_b_subarrays,
                    leaf_loads,
                    found: true,
                    value: Some(val),
                };
            }

            0x05..=0x0B => {
                let lf = tag - 0x04;
                if level > lf && !crate::get::decode_matches(edge, key, lf, level) {
                    return ProbeVisits {
                        edges_followed,
                        branch_b_subarrays,
                        leaf_loads,
                        found: false,
                        value: None,
                    };
                }
                leaf_loads += 1;
                let pop = edge.pop0(lf) as usize + 1;
                let base = edge.node_ptr();
                // SAFETY: map leaves are one live allocation of `pop` values followed by the packed keys.
                let keys = unsafe { base.add(crate::leaf::map_keys_offset(pop)) };
                // SAFETY: `keys` points to the valid packed leaf keys slice.
                let slot = unsafe { crate::leaf::search(keys, pop, lf, key) };
                // SAFETY: `s < pop` indexes a valid value slot in the allocation.
                let val = slot.map(|s| unsafe { *base.cast::<u64>().add(s) });
                return ProbeVisits {
                    edges_followed,
                    branch_b_subarrays,
                    leaf_loads,
                    found: val.is_some(),
                    value: val,
                };
            }

            0x7F => {
                unreachable!("full-expanse edges are set-flavor only");
            }

            0x10 | 0x20 | 0x30 | 0x40 | 0x50 | 0x60 | 0x70 => {
                let kb = (tag >> 4) as usize;
                let matched = ((key ^ edge.aux_word()) & IMM_MASKS[kb]) == 0;
                let val = if matched { Some(edge.word0()) } else { None };
                return ProbeVisits {
                    edges_followed,
                    branch_b_subarrays,
                    leaf_loads,
                    found: matched,
                    value: val,
                };
            }

            _ => {
                let Some(im) = ImmedType::from_u8(tag) else {
                    debug_assert!(false, "invalid edge tag {:#04x}", tag);
                    return ProbeVisits {
                        edges_followed,
                        branch_b_subarrays,
                        leaf_loads,
                        found: false,
                        value: None,
                    };
                };
                debug_assert_eq!(
                    im.key_bytes(),
                    level,
                    "an immediate's key size is its level"
                );
                let slot = immed_find(im, edge.aux_bytes(), key);
                if let Some(s) = slot {
                    leaf_loads += 1;
                    let vals = edge.node_ptr().cast::<u64>();
                    // SAFETY: `s` is a valid index into the immediate value array.
                    let val = unsafe { *vals.add(s) };
                    return ProbeVisits {
                        edges_followed,
                        branch_b_subarrays,
                        leaf_loads,
                        found: true,
                        value: Some(val),
                    };
                } else {
                    return ProbeVisits {
                        edges_followed,
                        branch_b_subarrays,
                        leaf_loads,
                        found: false,
                        value: None,
                    };
                }
            }
        }
    }
}

/// Walks a set-flavor subtree mirroring `contains` / `test_set`, counting dependent node visits.
#[doc(hidden)]
pub unsafe fn walk_set_probe_visits_impl<const SKIP_BRANCHB_SUBARRAY: bool>(
    edge: &Edge,
    key: u64,
    level: u8,
) -> ProbeVisits {
    let mut edge = edge;
    let mut level = level;
    let mut edges_followed = 0;
    let mut branch_b_subarrays = 0;
    let mut leaf_loads = 0;

    loop {
        debug_assert!((1..=8).contains(&level));
        let tag = edge.tag_byte();

        match tag {
            0x00 => {
                return ProbeVisits {
                    edges_followed,
                    branch_b_subarrays,
                    leaf_loads,
                    found: false,
                    value: None,
                };
            }

            0x01 => {
                edges_followed += 1;
                // SAFETY: pointer-tagged edge → live BranchL3.
                let b = unsafe { &*edge.node_ptr().cast::<BranchL3>() };
                let bl = b.hdr.level;
                if bl < level && !crate::get::decode_matches(edge, key, bl, level) {
                    return ProbeVisits {
                        edges_followed,
                        branch_b_subarrays,
                        leaf_loads,
                        found: false,
                        value: None,
                    };
                }
                let d = crate::types::digit(key, bl);
                let num = b.hdr.num as usize;
                let slot = if b.hdr.digits[0] == d {
                    0
                } else if num > 1 && b.hdr.digits[1] == d {
                    1
                } else if num > 2 && b.hdr.digits[2] == d {
                    2
                } else {
                    return ProbeVisits {
                        edges_followed,
                        branch_b_subarrays,
                        leaf_loads,
                        found: false,
                        value: None,
                    };
                };
                // SAFETY: `slot` is within bounds of `b.edges`.
                edge = unsafe { &*b.edges.as_ptr().add(slot) };
                level = bl - 1;
            }

            0x02 => {
                edges_followed += 1;
                // SAFETY: pointer-tagged edge → live BranchL7.
                let b = unsafe { &*edge.node_ptr().cast::<BranchL7>() };
                let bl = b.hdr.level;
                if bl < level && !crate::get::decode_matches(edge, key, bl, level) {
                    return ProbeVisits {
                        edges_followed,
                        branch_b_subarrays,
                        leaf_loads,
                        found: false,
                        value: None,
                    };
                }
                let d = crate::types::digit(key, bl);
                let Some(slot) = b.hdr.find(d) else {
                    return ProbeVisits {
                        edges_followed,
                        branch_b_subarrays,
                        leaf_loads,
                        found: false,
                        value: None,
                    };
                };
                debug_assert!(slot < b.edges.len());
                // SAFETY: `slot` is within bounds of `b.edges`.
                edge = unsafe { &*b.edges.as_ptr().add(slot) };
                level = bl - 1;
            }

            0x03 => {
                edges_followed += 1;
                // SAFETY: pointer-tagged edge → live BranchB.
                let b = unsafe { &*edge.node_ptr().cast::<BranchB>() };
                let bl = b.level;
                if bl < level && !crate::get::decode_matches(edge, key, bl, level) {
                    return ProbeVisits {
                        edges_followed,
                        branch_b_subarrays,
                        leaf_loads,
                        found: false,
                        value: None,
                    };
                }
                let d = crate::types::digit(key, bl);
                let Some((sub, slot)) = b.bitmap.test_and_subexpanse_rank_with_sub(d) else {
                    return ProbeVisits {
                        edges_followed,
                        branch_b_subarrays,
                        leaf_loads,
                        found: false,
                        value: None,
                    };
                };
                if !SKIP_BRANCHB_SUBARRAY {
                    branch_b_subarrays += 1;
                }
                // SAFETY: `sub < 8` accesses a valid subarray pointer.
                let sub_ptr = unsafe { *b.subarrays.as_ptr().add(sub) };
                // SAFETY: `slot` is the verified rank inside the live subexpanse subarray.
                edge = unsafe { &*sub_ptr.add(slot) };
                level = bl - 1;
            }

            0x04 => {
                edges_followed += 1;
                let mut b_ptr = edge.node_ptr().cast::<BranchU>();
                loop {
                    let d = crate::types::digit(key, level);
                    // SAFETY: pointer-tagged edge → live BranchU with 256 edges.
                    let next_edge = unsafe { &*(*b_ptr).edges.as_ptr().add(d as usize) };
                    level -= 1;
                    let next_tag = next_edge.tag_byte();
                    if next_tag == 0x04 {
                        edges_followed += 1;
                        b_ptr = next_edge.node_ptr().cast::<BranchU>();
                    } else {
                        edge = next_edge;
                        break;
                    }
                }
            }

            0x0C => {
                if level > 1 && !crate::get::decode_matches(edge, key, 1, level) {
                    return ProbeVisits {
                        edges_followed,
                        branch_b_subarrays,
                        leaf_loads,
                        found: false,
                        value: None,
                    };
                }
                leaf_loads += 1;
                let d = (key & 0xFF) as u8;
                // SAFETY: pointer-tagged edge → live LeafBitmap1.
                let l = unsafe { &*edge.node_ptr().cast::<LeafBitmap1>() };
                let found = l.bitmap.test(d);
                return ProbeVisits {
                    edges_followed,
                    branch_b_subarrays,
                    leaf_loads,
                    found,
                    value: None,
                };
            }

            0x05..=0x0B => {
                let lf = tag - 0x04;
                if level > lf && !crate::get::decode_matches(edge, key, lf, level) {
                    return ProbeVisits {
                        edges_followed,
                        branch_b_subarrays,
                        leaf_loads,
                        found: false,
                        value: None,
                    };
                }
                leaf_loads += 1;
                let pop = edge.pop0(lf) as usize + 1;
                let base = edge.node_ptr();
                // SAFETY: `base` points to a live leaf allocation with `pop` elements.
                let found = (unsafe { crate::leaf::search(base, pop, lf, key) }).is_some();
                return ProbeVisits {
                    edges_followed,
                    branch_b_subarrays,
                    leaf_loads,
                    found,
                    value: None,
                };
            }

            0x7F => {
                return ProbeVisits {
                    edges_followed,
                    branch_b_subarrays,
                    leaf_loads,
                    found: true,
                    value: None,
                };
            }

            0x10 | 0x20 | 0x30 | 0x40 | 0x50 | 0x60 | 0x70 => {
                let kb = (tag >> 4) as usize;
                let found = ((key ^ edge.word0()) & IMM_MASKS[kb]) == 0;
                return ProbeVisits {
                    edges_followed,
                    branch_b_subarrays,
                    leaf_loads,
                    found,
                    value: None,
                };
            }

            _ => {
                let Some(im) = ImmedType::from_u8(tag) else {
                    debug_assert!(false, "invalid edge tag {:#04x}", tag);
                    return ProbeVisits {
                        edges_followed,
                        branch_b_subarrays,
                        leaf_loads,
                        found: false,
                        value: None,
                    };
                };
                debug_assert_eq!(
                    im.key_bytes(),
                    level,
                    "an immediate's key size is its level"
                );
                let payload = edge.imm_payload();
                let found = immed_find(im, &payload, key).is_some();
                return ProbeVisits {
                    edges_followed,
                    branch_b_subarrays,
                    leaf_loads,
                    found,
                    value: None,
                };
            }
        }
    }
}

/// Walks a map-flavor subtree mirroring `get`, counting dependent node visits.
///
/// # Safety
///
/// `edge` must refer to a valid, live subtree edge descriptor.
#[doc(hidden)]
pub unsafe fn walk_map_probe_visits(edge: &Edge, key: u64, level: u8) -> ProbeVisits {
    // SAFETY: caller guarantees `edge` points to a valid live subtree.
    unsafe { walk_map_probe_visits_impl::<false>(edge, key, level) }
}

/// Walks a set-flavor subtree mirroring `contains` / `test_set`, counting dependent node visits.
///
/// # Safety
///
/// `edge` must refer to a valid, live subtree edge descriptor.
#[doc(hidden)]
pub unsafe fn walk_set_probe_visits(edge: &Edge, key: u64, level: u8) -> ProbeVisits {
    // SAFETY: caller guarantees `edge` points to a valid live subtree.
    unsafe { walk_set_probe_visits_impl::<false>(edge, key, level) }
}

#[cfg(test)]
mod tests {
    use crate::bytesmap::ExpanseBytesMap;
    use crate::map::ExpanseMap;
    use crate::set::ExpanseSet;
    use crate::strmap::{ExpanseStrMap, NulFreeStr};

    /// The demotion predicates read the derived `*_DOWN` constants
    /// (`digits <= BRANCHB_TO_L7_DOWN`, `digits <= BRANCHU_TO_B_DOWN`) where
    /// they once read `digits < BRANCH_L7_CAP` and `digits < BRANCHB_UP`, and
    /// `LEAFB1_DOWN` is derived from `LEAF1_CAP`. Every child count a node can
    /// hold must take the same branch as under the literals 7 and 21 the old
    /// forms compared against, so the rewrite moves no demotion. The U -> B
    /// floor is pinned to its literal 161: the band between it and the
    /// promotion point is one bitmap subexpanse (32 digits) wide. Under the
    /// diagnostic `ablation-one-digit-band` feature the literals are the
    /// one-digit band's (192, width 1), the band #1221 widened.
    #[test]
    fn demotion_thresholds_match_the_literal_forms() {
        use crate::mutate::BRANCHB_UP;
        use crate::types::{
            BRANCH_L7_CAP, BRANCHB_TO_L7_DOWN, BRANCHU_TO_B_DOWN, LEAF1_CAP, LEAFB1_DOWN,
        };
        #[cfg(not(feature = "ablation-one-digit-band"))]
        const U_FLOOR_LITERAL: usize = 161;
        #[cfg(feature = "ablation-one-digit-band")]
        const U_FLOOR_LITERAL: usize = 192;
        const U_BAND_LITERAL: usize = 193 - U_FLOOR_LITERAL;
        for n in 0..=crate::types::BRANCH_FANOUT {
            assert_eq!(n <= BRANCHB_TO_L7_DOWN, n < BRANCH_L7_CAP, "B -> L7 at {n}");
            assert_eq!(n <= BRANCHB_TO_L7_DOWN, n < 7, "B -> L7 at {n}");
            assert_eq!(
                n <= BRANCHU_TO_B_DOWN,
                n < BRANCHB_UP - (U_BAND_LITERAL - 1),
                "U -> B at {n}"
            );
            assert_eq!(n <= BRANCHU_TO_B_DOWN, n < U_FLOOR_LITERAL, "U -> B at {n}");
            // The one predicate every U -> B decision reads (Refs #1079):
            // the exclusive walks, the optimistic removal and the validator
            // share it, so an error in it would move all three together and
            // no drain test could see it. Pinned against the literal here.
            assert_eq!(
                crate::mutate::branch_u_below_floor(n),
                n < U_FLOOR_LITERAL,
                "branch_u_below_floor at {n}"
            );
            assert_eq!(n < LEAFB1_DOWN, n < 21, "bitmap leaf -> Leaf1 at {n}");
        }
        assert_eq!(
            LEAF1_CAP - LEAFB1_DOWN,
            4,
            "Leaf1 band: enters above 25, leaves below 21"
        );
        assert_eq!(
            BRANCHB_UP - BRANCHU_TO_B_DOWN,
            U_BAND_LITERAL,
            "BranchU band: enters above 192, leaves at {}",
            U_FLOOR_LITERAL - 1
        );
    }

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

    #[test]
    fn strmap_node_bytes_sum_to_mem_used() {
        let mut m = ExpanseStrMap::new();
        let s0 = m.stats();
        assert_eq!(s0.node_bytes.total(), m.mem_used());
        assert_eq!(s0.node_bytes.total(), 0);

        let mut rng = 0x1234_5678_9ABC_DEF0u64;
        let mut next = move || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };

        for i in 0..5_000 {
            let key = match i % 5 {
                0 => format!("k{i}"),
                1 => format!("chunk8_{:02}", i % 100),
                2 => format!("suffix_branch_{:04}_longer_key_tail", i),
                3 => format!("t{:03}:orders:{:010}", i % 100, i),
                _ => {
                    let r = next();
                    format!("rnd_{:016x}_{:08x}", r, i)
                }
            };
            let k = NulFreeStr::new(key.as_bytes()).unwrap();
            m.insert(k, i as u64);

            if i < 30 || i % 500 == 0 {
                let s = m.stats();
                assert_eq!(
                    s.node_bytes.total(),
                    m.mem_used(),
                    "strmap insert i={i}: {:?}",
                    s.node_bytes
                );
                assert_eq!(
                    s.node_bytes.sub_maps.total()
                        + s.node_bytes.node_shells
                        + s.node_bytes.suffixes,
                    m.mem_used(),
                    "strmap parts insert i={i}"
                );
                assert_eq!(s.node_bytes.suffixes, s.node_bytes.suffix_bytes());
                assert_eq!(s.node_bytes.node_shells, s.node_bytes.node_shell_bytes());
            }
        }

        let s_full = m.stats();
        assert_eq!(s_full.node_bytes.total(), m.mem_used());

        for i in (0..5_000).step_by(3) {
            let key = match i % 5 {
                0 => format!("k{i}"),
                1 => format!("chunk8_{:02}", i % 100),
                2 => format!("suffix_branch_{:04}_longer_key_tail", i),
                3 => format!("t{:03}:orders:{:010}", i % 100, i),
                _ => continue,
            };
            if let Some(k) = NulFreeStr::new(key.as_bytes()) {
                m.remove(k);
            }
        }
        let s_after_del = m.stats();
        assert_eq!(s_after_del.node_bytes.total(), m.mem_used());

        m.clear();
        let s_empty = m.stats();
        assert_eq!(s_empty.node_bytes.total(), m.mem_used());
        assert_eq!(s_empty.node_bytes.total(), 0);
    }

    #[test]
    fn bytesmap_node_bytes_sum_to_mem_used() {
        let mut m = ExpanseBytesMap::new();
        let s0 = m.stats();
        assert_eq!(s0.node_bytes.total(), m.mem_used());
        assert_eq!(s0.node_bytes.total(), 0);

        for i in 0..5_000 {
            let key = match i % 4 {
                0 => format!("b_{i}").into_bytes(),
                1 => format!("bytes_key_{:010}", i).into_bytes(),
                2 => format!("deeply/nested/path/to/resource/{:06}", i).into_bytes(),
                _ => (0..32).map(|b| ((i + b) % 256) as u8).collect(),
            };
            m.insert(&key, i as u64);

            if i < 30 || i % 500 == 0 {
                let s = m.stats();
                assert_eq!(
                    s.node_bytes.total(),
                    m.mem_used(),
                    "bytesmap insert i={i}: {:?}",
                    s.node_bytes
                );
                assert_eq!(
                    s.node_bytes.sub_maps.total()
                        + s.node_bytes.node_shells
                        + s.node_bytes.terminal_bytes,
                    m.mem_used(),
                    "bytesmap parts insert i={i}"
                );
                assert_eq!(s.node_bytes.node_shells, s.node_bytes.bucket_shells());
                assert_eq!(s.node_bytes.terminal_bytes, s.node_bytes.terminals());
            }
        }

        let s_full = m.stats();
        assert_eq!(s_full.node_bytes.total(), m.mem_used());

        for i in (0..5_000).step_by(2) {
            let key = match i % 4 {
                0 => format!("b_{i}").into_bytes(),
                1 => format!("bytes_key_{:010}", i).into_bytes(),
                2 => format!("deeply/nested/path/to/resource/{:06}", i).into_bytes(),
                _ => (0..32).map(|b| ((i + b) % 256) as u8).collect(),
            };
            m.remove(&key);
        }
        let s_after_del = m.stats();
        assert_eq!(s_after_del.node_bytes.total(), m.mem_used());

        m.clear();
        let s_empty = m.stats();
        assert_eq!(s_empty.node_bytes.total(), m.mem_used());
        assert_eq!(s_empty.node_bytes.total(), 0);
    }
}
