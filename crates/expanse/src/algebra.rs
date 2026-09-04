//! Native set-algebra kernels over the trie structure (`docs/ALGORITHMS.md`,
//! "Set-algebra kernels").
//!
//! Measurements of this module live in `docs/benchmarks/set_algebra/`, which
//! routes to every harness that exercises it: the interned-domain arms it owns
//! directly, the Boolean and WAND pillars in `search_inverted_index/`, the
//! grammar-mask arm in `llm_inference/`, and the `Bitmap256::count_and` kernel
//! sweep in `avx512/`.
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
//! use direct-emission bulk construction in `algebra_build.rs` (#348); the
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

#[cfg(not(feature = "std"))]
use core_alloc::{vec, vec::Vec};
#[cfg(feature = "std")]
use std::vec::Vec;

/// True when `tag` names one of the four branch flavors.
#[inline(always)]
pub(crate) fn is_branch(tag: u8) -> bool {
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
pub(crate) unsafe fn subtree_count(edge: &Edge, level: u8) -> u64 {
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
pub(crate) const fn full_count(level: u8) -> u64 {
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
pub(crate) unsafe fn as_level1_bitmap(edge: &Edge, level: u8) -> Option<(u64, Bitmap256)> {
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
pub(crate) unsafe fn child_by_digit(edge: &Edge, level: u8, d: u8) -> Edge {
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

/// Read accessor over a live branch node, caching the form and skipping status
/// to avoid repeated dispatch and tag decoding during digit-by-digit operations.
#[derive(Copy, Clone)]
pub(crate) enum BranchAccessor<'e> {
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
    pub(crate) fn populated_bitmap(&self) -> Bitmap256 {
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
    pub(crate) unsafe fn child(&self, edge: &Edge, d: u8) -> Edge {
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
pub(crate) unsafe fn branch_accessor<'e>(edge: &'e Edge, level: u8) -> BranchAccessor<'e> {
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
        Some(EdgeTag::Structural(EdgeType::BranchL3)) => {
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

/// The `level`-byte key remainders of a terminal (leaf / immediate) edge, or
/// `None` when `edge` is a branch (which has no cheap flat enumeration). Each
/// remainder carries the edge's skipped decode digits, so it can be probed
/// against a sibling subtree with [`get::test_set`] at the same `level`.
///
/// # Safety
/// Extracts the `level`-byte key remainders of a terminal (leaf / immediate)
/// edge into `buf`, returning the count of keys written, or `None` when `edge` is a
/// branch.
///
/// # Safety
///
/// `edge` must reference a live node/leaf of its tagged type. `buf` must have
/// capacity >= 256.
#[inline]
pub(crate) unsafe fn terminal_remainders_buf(
    edge: &Edge,
    level: u8,
    buf: &mut [u64],
) -> Option<usize> {
    match edge.tag() {
        Some(EdgeTag::Immed(im)) => {
            // Immediates store the whole remainder (key_bytes == slot level).
            let keys = mutate::immed_keys(edge, im);
            let n = keys.len();
            debug_assert!(buf.len() >= n);
            buf[..n].copy_from_slice(&keys);
            Some(n)
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
            let mut count = 0;
            let mut from = b.bitmap.next_set(0);
            while let Some(bit) = from {
                if count < buf.len() {
                    buf[count] = base | u64::from(bit);
                    count += 1;
                }
                from = if bit == 255 {
                    None
                } else {
                    b.bitmap.next_set(bit + 1)
                };
            }
            Some(count)
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
            let mut count = 0;
            for k in packed {
                if count < buf.len() {
                    buf[count] = base | k;
                    count += 1;
                }
            }
            Some(count)
        }
        _ => None,
    }
}

/// Population of the subtree rooted at `edge` covering `level` undecoded bytes.
/// At `level <= 7`, delegates to [`subtree_count`], which reads `pop0` in `O(1)`.
/// At `level >= 8` (a container root edge), recursively counts keys across children
/// at `level - 1` without attempting to read out-of-range `pop0(8)`.
///
/// # Safety
///
/// `edge` must reference a live, well-formed node/leaf of its tagged type at `level`.
#[inline]
pub(crate) unsafe fn edge_count(edge: &Edge, level: u8) -> u64 {
    if level <= 7 {
        // SAFETY: caller guarantees edge is live at level <= 7.
        unsafe { subtree_count(edge, level) }
    } else {
        if edge.is_null() {
            return 0;
        }
        if edge.tag_byte() == EdgeType::FullExpanse as u8 {
            return full_count(level);
        }
        let mut kbuf = [0u64; 256];
        if !is_branch(edge.tag_byte()) {
            // SAFETY: caller guarantees edge is live at level.
            if let Some(n) = unsafe { terminal_remainders_buf(edge, level, &mut kbuf) } {
                return n as u64;
            }
        }
        // SAFETY: edge is a live branch at level >= 8 per contract.
        let acc = unsafe { branch_accessor(edge, level) };
        let bm = acc.populated_bitmap();
        let mut total = 0u64;
        let mut from = bm.next_set(0);
        while let Some(d) = from {
            // SAFETY: live branch at level >= 8.
            let c = unsafe { child_by_digit(edge, level, d) };
            if !c.is_null() {
                // SAFETY: child is live at level - 1.
                total += unsafe { edge_count(&c, level - 1) };
            }
            from = if d == 255 { None } else { bm.next_set(d + 1) };
        }
        total
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
    #[cfg(all(target_arch = "x86_64", not(target_feature = "popcnt")))]
    // SAFETY: caller contract forwarded; available() gates popcnt.
    unsafe {
        if crate::bits::popcnt_rt::available() {
            for &k in keys {
                if get::test_set_popcnt(probe, k, level) {
                    acc += 1;
                }
            }
        } else {
            for &k in keys {
                if get::test_set_swar(probe, k, level) {
                    acc += 1;
                }
            }
        }
    }
    #[cfg(any(not(target_arch = "x86_64"), target_feature = "popcnt"))]
    {
        for &k in keys {
            // SAFETY: probe is a live subtree at `level`; k's high bits are zero
            // above `level`, which test_set ignores.
            if unsafe { get::test_set(probe, k, level) } {
                acc += 1;
            }
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
/// Internal trait parameterizing the set-algebra cardinality walks over their
/// instruction-set flavor: portable SWAR vs hardware `popcnt`.
///
/// On baseline `x86-64`, `Bitmap256::count_and`, `Bitmap256::count_or`, and
/// `Bitmap256::subexpanse_rank` lower to ~12-instruction SWAR sequences unless
/// inlined into a `#[target_feature(enable = "popcnt")]` caller. Cloning the
/// walks at entry and recurring within the cloned flavor keeps the entire
/// descent inside hardware `popcnt` without per-node CPUID checks or
/// feature-boundary crossing (#638).
///
/// # Safety
///
/// Implementors must uphold the invariants of their respective instruction
/// flavor: `PopcntFlavor` requires hardware CPUID `popcnt` support; all methods
/// require caller-supplied edge and level validity invariants to hold.
trait AlgebraFlavor: Copy {
    unsafe fn intersection_len(ea: &Edge, eb: &Edge, level: u8) -> u64;
    unsafe fn intersection_len_many(edges: &[Edge], level: u8) -> u64;
    unsafe fn union_len_many(edges: &[Edge], level: u8) -> u64;
}

#[cfg(all(target_arch = "x86_64", not(target_feature = "popcnt")))]
#[derive(Copy, Clone)]
struct SwarFlavor;

#[cfg(all(target_arch = "x86_64", not(target_feature = "popcnt")))]
impl AlgebraFlavor for SwarFlavor {
    #[inline(always)]
    unsafe fn intersection_len(ea: &Edge, eb: &Edge, level: u8) -> u64 {
        // SAFETY: forwarded caller contract.
        unsafe { intersection_len_swar(ea, eb, level) }
    }

    #[inline(always)]
    unsafe fn intersection_len_many(edges: &[Edge], level: u8) -> u64 {
        // SAFETY: forwarded caller contract.
        unsafe { intersection_len_many_swar(edges, level) }
    }

    #[inline(always)]
    unsafe fn union_len_many(edges: &[Edge], level: u8) -> u64 {
        // SAFETY: forwarded caller contract.
        unsafe { union_len_many_swar(edges, level) }
    }
}

#[cfg(all(target_arch = "x86_64", not(target_feature = "popcnt")))]
#[derive(Copy, Clone)]
struct PopcntFlavor;

#[cfg(all(target_arch = "x86_64", not(target_feature = "popcnt")))]
impl AlgebraFlavor for PopcntFlavor {
    #[inline(always)]
    unsafe fn intersection_len(ea: &Edge, eb: &Edge, level: u8) -> u64 {
        // SAFETY: forwarded caller contract; caller verified popcnt CPUID support.
        unsafe { intersection_len_popcnt(ea, eb, level) }
    }

    #[inline(always)]
    unsafe fn intersection_len_many(edges: &[Edge], level: u8) -> u64 {
        // SAFETY: forwarded caller contract; caller verified popcnt CPUID support.
        unsafe { intersection_len_many_popcnt(edges, level) }
    }

    #[inline(always)]
    unsafe fn union_len_many(edges: &[Edge], level: u8) -> u64 {
        // SAFETY: forwarded caller contract; caller verified popcnt CPUID support.
        unsafe { union_len_many_popcnt(edges, level) }
    }
}

#[cfg(any(not(target_arch = "x86_64"), target_feature = "popcnt"))]
#[derive(Copy, Clone)]
struct NativeFlavor;

#[cfg(any(not(target_arch = "x86_64"), target_feature = "popcnt"))]
impl AlgebraFlavor for NativeFlavor {
    #[inline(always)]
    unsafe fn intersection_len(ea: &Edge, eb: &Edge, level: u8) -> u64 {
        // SAFETY: forwarded caller contract.
        unsafe { intersection_len_impl::<NativeFlavor>(ea, eb, level) }
    }

    #[inline(always)]
    unsafe fn intersection_len_many(edges: &[Edge], level: u8) -> u64 {
        // SAFETY: forwarded caller contract.
        unsafe { intersection_len_many_impl::<NativeFlavor>(edges, level) }
    }

    #[inline(always)]
    unsafe fn union_len_many(edges: &[Edge], level: u8) -> u64 {
        // SAFETY: forwarded caller contract.
        unsafe { union_len_many_impl::<NativeFlavor>(edges, level) }
    }
}

#[inline(always)]
unsafe fn intersection_branch_impl<F: AlgebraFlavor>(ea: &Edge, eb: &Edge, level: u8) -> u64 {
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
        return unsafe { F::intersection_len(&ca, eb, level - 1) };
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
                    acc += unsafe { F::intersection_len(&ca, cb, level - 1) };
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
                    acc += unsafe { F::intersection_len(&ca, &cb, level - 1) };
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
                    acc += unsafe { F::intersection_len(&ca, &cb, level - 1) };
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
                    acc += unsafe { F::intersection_len(&ca, &cb, level - 1) };
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
/// are branches (that case is handled by [`intersection_branch_impl`]).
#[inline(always)]
unsafe fn intersection_terminal(ea: &Edge, eb: &Edge, level: u8) -> u64 {
    let mut ka_buf = [0u64; 256];
    let mut kb_buf = [0u64; 256];
    // SAFETY: forwarded contract.
    let ra = unsafe { terminal_remainders_buf(ea, level, &mut ka_buf) };
    // SAFETY: forwarded contract.
    let rb = unsafe { terminal_remainders_buf(eb, level, &mut kb_buf) };
    match (ra, rb) {
        (Some(na), Some(nb)) => {
            let ka = &ka_buf[..na];
            let kb = &kb_buf[..nb];
            // Both terminal: probe the smaller list into the larger subtree.
            if na <= nb {
                // SAFETY: eb is a live subtree at `level`.
                unsafe { probe_count(ka, eb, level) }
            } else {
                // SAFETY: ea is a live subtree at `level`.
                unsafe { probe_count(kb, ea, level) }
            }
        }
        // SAFETY: the branch side is a live subtree at `level`.
        (Some(na), None) => unsafe { probe_count(&ka_buf[..na], eb, level) },
        // SAFETY: the branch side is a live subtree at `level`.
        (None, Some(nb)) => unsafe { probe_count(&kb_buf[..nb], ea, level) },
        (None, None) => 0,
    }
}

/// Inner generic implementation of intersection cardinality of two non-null
/// subtrees covering the same expanse of `level` undecoded bytes.
///
/// # Safety
///
/// Same contract as [`intersection_len`].
#[inline(always)]
unsafe fn intersection_len_impl<F: AlgebraFlavor>(ea: &Edge, eb: &Edge, level: u8) -> u64 {
    debug_assert!(!ea.is_null() && !eb.is_null());
    let ta = ea.tag_byte();
    let tb = eb.tag_byte();

    // A full expanse contains every key of its expanse, so its intersection
    // with the other side is exactly the other side's population.
    if ta == EdgeType::FullExpanse as u8 {
        // SAFETY: eb is a live subtree at `level`.
        return unsafe { edge_count(eb, level) };
    }
    if tb == EdgeType::FullExpanse as u8 {
        // SAFETY: ea is a live subtree at `level`.
        return unsafe { edge_count(ea, level) };
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
        return unsafe { intersection_branch_impl::<F>(ea, eb, level) };
    }

    // SAFETY: at least one terminal; both live at `level`.
    unsafe { intersection_terminal(ea, eb, level) }
}

/// Inner generic implementation of intersection cardinality of k non-null
/// subtrees covering the same expanse of `level` undecoded bytes (#610).
///
/// # Safety
///
/// Same contract as [`intersection_len_many`].
#[inline(always)]
unsafe fn intersection_len_many_impl<F: AlgebraFlavor>(edges: &[Edge], level: u8) -> u64 {
    // SAFETY: caller guarantees all edges in `edges` reference live, well-formed
    // subtrees covering `level` undecoded bytes.
    unsafe {
        if edges.is_empty() {
            return 0;
        }
        if edges.len() == 1 {
            return edge_count(&edges[0], level);
        }
        if edges.len() == 2 {
            // SAFETY: caller guarantees both edges live at `level`.
            return F::intersection_len(&edges[0], &edges[1], level);
        }

        // 1. Any null edge immediately means empty intersection.
        for e in edges {
            if e.is_null() {
                return 0;
            }
        }

        // 2. FullExpanse edges cover the entire expanse, so they impose no constraint.
        let mut non_full_buf: [Edge; 16] = [Edge::NULL; 16];
        let non_full_vec: Vec<Edge>;
        let active_edges: &[Edge] = if edges
            .iter()
            .any(|e| e.tag_byte() == EdgeType::FullExpanse as u8)
        {
            let count_non_full = edges
                .iter()
                .filter(|e| e.tag_byte() != EdgeType::FullExpanse as u8)
                .count();
            if count_non_full == 0 {
                return full_count(level);
            }
            if count_non_full == 1 {
                let single = edges
                    .iter()
                    .find(|e| e.tag_byte() != EdgeType::FullExpanse as u8)
                    .unwrap();
                return edge_count(single, level);
            }
            if count_non_full <= 16 {
                let mut idx = 0;
                for e in edges {
                    if e.tag_byte() != EdgeType::FullExpanse as u8 {
                        non_full_buf[idx] = *e;
                        idx += 1;
                    }
                }
                &non_full_buf[..idx]
            } else {
                non_full_vec = edges
                    .iter()
                    .filter(|e| e.tag_byte() != EdgeType::FullExpanse as u8)
                    .copied()
                    .collect();
                &non_full_vec
            }
        } else {
            edges
        };

        if active_edges.len() == 2 {
            // SAFETY: both active edges live at `level`.
            return F::intersection_len(&active_edges[0], &active_edges[1], level);
        }

        // 3. Check if all active edges reduce to a final-byte bitmap (real level 1):
        let mut all_bms = true;
        let mut bms_buf: [Bitmap256; 16] = [Bitmap256::new(); 16];

        if let Some((dv, bm)) = as_level1_bitmap(&active_edges[0], level) {
            let first_dv = dv;
            bms_buf[0] = bm;
            for (i, e) in active_edges[1..].iter().enumerate() {
                if let Some((dv_i, bm_i)) = as_level1_bitmap(e, level) {
                    if dv_i != first_dv {
                        return 0;
                    }
                    if i + 1 < 16 {
                        bms_buf[i + 1] = bm_i;
                    }
                } else {
                    all_bms = false;
                    break;
                }
            }
        } else {
            all_bms = false;
        }

        if all_bms {
            let mut acc = bms_buf[0];
            for (i, e) in active_edges.iter().enumerate().skip(1) {
                let bm_i = if i < 16 {
                    &bms_buf[i]
                } else {
                    let (_, bm) = as_level1_bitmap(e, level).unwrap();
                    bms_buf[0] = bm;
                    &bms_buf[0]
                };
                acc = acc.and(bm_i);
                if acc.is_empty() {
                    return 0;
                }
            }
            return u64::from(acc.count());
        }

        // 4. Check if at least one active edge is a terminal.
        let mut smallest_term_idx = None;
        let mut smallest_term_count = usize::MAX;
        let mut term_keys_buf = [0u64; 256];
        let mut best_keys_buf = [0u64; 256];

        for (i, e) in active_edges.iter().enumerate() {
            if !is_branch(e.tag_byte())
                && let Some(n) = terminal_remainders_buf(e, level, &mut term_keys_buf)
                && n < smallest_term_count
            {
                smallest_term_count = n;
                smallest_term_idx = Some(i);
                best_keys_buf[..n].copy_from_slice(&term_keys_buf[..n]);
            }
        }

        if let Some(t_idx) = smallest_term_idx {
            let keys = &best_keys_buf[..smallest_term_count];
            let mut acc = 0u64;
            'key_loop: for &k in keys {
                for (i, e) in active_edges.iter().enumerate() {
                    if i == t_idx {
                        continue;
                    }
                    if !get::test_set(e, k, level) {
                        continue 'key_loop;
                    }
                }
                acc += 1;
            }
            return acc;
        }

        // 5. All active edges are branches.
        // SAFETY: active_edges contains only branches live at `level`.
        intersection_branches_many_impl::<F>(active_edges, level)
    }
}

/// Intersection cardinality when all edges are branches covering `level`.
#[inline(always)]
unsafe fn intersection_branches_many_impl<F: AlgebraFlavor>(edges: &[Edge], level: u8) -> u64 {
    // SAFETY: caller guarantees all edges in `edges` reference live branch nodes at `level`.
    unsafe {
        let mut skipping_pd: Option<u8> = None;
        for e in edges {
            let t = match e.tag() {
                Some(EdgeTag::Structural(t)) if t.is_branch() => t,
                _ => return 0,
            };
            let bl = mutate::branch_form_level(e, t, level);
            if bl < level {
                let pd = e.decode_bytes(bl)[(level - bl - 1) as usize];
                if let Some(prev) = skipping_pd {
                    if prev != pd {
                        return 0;
                    }
                } else {
                    skipping_pd = Some(pd);
                }
            }
        }

        if let Some(pd) = skipping_pd {
            let mut child_buf: [Edge; 16] = [Edge::NULL; 16];
            let mut child_vec: Vec<Edge>;
            let children: &[Edge] = if edges.len() <= 16 {
                for (i, e) in edges.iter().enumerate() {
                    let c = child_by_digit(e, level, pd);
                    if c.is_null() {
                        return 0;
                    }
                    child_buf[i] = c;
                }
                &child_buf[..edges.len()]
            } else {
                child_vec = Vec::with_capacity(edges.len());
                for e in edges {
                    let c = child_by_digit(e, level, pd);
                    if c.is_null() {
                        return 0;
                    }
                    child_vec.push(c);
                }
                &child_vec
            };
            // SAFETY: children are live subtrees at level - 1.
            return F::intersection_len_many(children, level - 1);
        }

        // No branch skips this level. Pick branch with fewest active children.
        let mut best_driver_idx = 0;
        let mut min_children = 257usize;

        for (i, e) in edges.iter().enumerate() {
            let t = e.tag_byte();
            let num = if t == EdgeType::BranchL3 as u8 {
                let b = &*e.node_ptr().cast::<BranchL3>();
                b.hdr.num as usize
            } else if t == EdgeType::BranchL7 as u8 {
                let b = &*e.node_ptr().cast::<BranchL7>();
                b.hdr.num as usize
            } else if t == EdgeType::BranchB as u8 {
                let b = &*e.node_ptr().cast::<BranchB>();
                b.bitmap.count() as usize
            } else {
                256
            };
            if num < min_children {
                min_children = num;
                best_driver_idx = i;
            }
        }

        let driver = &edges[best_driver_idx];
        let driver_t = driver.tag_byte();
        let mut acc = 0u64;

        let probe_digit = |d: u8, driver_child: Edge| -> u64 {
            let mut child_buf: [Edge; 16] = [Edge::NULL; 16];
            let mut child_vec: Vec<Edge>;
            let children: &[Edge] = if edges.len() <= 16 {
                for (i, e) in edges.iter().enumerate() {
                    if i == best_driver_idx {
                        child_buf[i] = driver_child;
                    } else {
                        let c = child_by_digit(e, level, d);
                        if c.is_null() {
                            return 0;
                        }
                        child_buf[i] = c;
                    }
                }
                &child_buf[..edges.len()]
            } else {
                child_vec = Vec::with_capacity(edges.len());
                for (i, e) in edges.iter().enumerate() {
                    if i == best_driver_idx {
                        child_vec.push(driver_child);
                    } else {
                        let c = child_by_digit(e, level, d);
                        if c.is_null() {
                            return 0;
                        }
                        child_vec.push(c);
                    }
                }
                &child_vec
            };
            // SAFETY: children are live subtrees at level - 1.
            F::intersection_len_many(children, level - 1)
        };

        if driver_t == EdgeType::BranchL3 as u8 {
            let b = &*driver.node_ptr().cast::<BranchL3>();
            for i in 0..b.hdr.num as usize {
                let d = b.hdr.digits[i];
                let c = b.edges[i];
                acc += probe_digit(d, c);
            }
        } else if driver_t == EdgeType::BranchL7 as u8 {
            let b = &*driver.node_ptr().cast::<BranchL7>();
            for i in 0..b.hdr.num as usize {
                let d = b.hdr.digits[i];
                let c = b.edges[i];
                acc += probe_digit(d, c);
            }
        } else if driver_t == EdgeType::BranchB as u8 {
            let b = &*driver.node_ptr().cast::<BranchB>();
            let mut from = b.bitmap.next_set(0);
            while let Some(d) = from {
                let sub = (d >> 5) as usize;
                let slot = b.bitmap.subexpanse_rank(d) as usize;
                let c = *b.subarrays[sub].add(slot);
                acc += probe_digit(d, c);
                from = if d == 255 {
                    None
                } else {
                    b.bitmap.next_set(d + 1)
                };
            }
        } else if driver_t == EdgeType::BranchU as u8 {
            let b = &*driver.node_ptr().cast::<BranchU>();
            for (d, &c) in b.edges.iter().enumerate() {
                if !c.is_null() {
                    acc += probe_digit(d as u8, c);
                }
            }
        }

        acc
    }
}

/// Inner generic implementation of union cardinality of k subtrees covering
/// the same expanse of `level` undecoded bytes (#610).
///
/// # Safety
///
/// Same contract as [`union_len_many`].
#[inline(always)]
unsafe fn union_len_many_impl<F: AlgebraFlavor>(edges: &[Edge], level: u8) -> u64 {
    // SAFETY: caller guarantees all non-null edges in `edges` reference live
    // subtrees covering `level` undecoded bytes.
    unsafe {
        if edges.is_empty() {
            return 0;
        }
        let mut non_null_buf: [Edge; 16] = [Edge::NULL; 16];
        let non_null_vec: Vec<Edge>;
        let active: &[Edge] = if edges.iter().any(|e| e.is_null()) {
            let count = edges.iter().filter(|e| !e.is_null()).count();
            if count == 0 {
                return 0;
            }
            if count <= 16 {
                let mut idx = 0;
                for e in edges {
                    if !e.is_null() {
                        non_null_buf[idx] = *e;
                        idx += 1;
                    }
                }
                &non_null_buf[..idx]
            } else {
                non_null_vec = edges.iter().filter(|e| !e.is_null()).copied().collect();
                &non_null_vec
            }
        } else {
            edges
        };

        if active.len() == 1 {
            return edge_count(&active[0], level);
        }
        if active.len() == 2 {
            if let (Some((da, ba)), Some((db, bb))) = (
                as_level1_bitmap(&active[0], level),
                as_level1_bitmap(&active[1], level),
            ) {
                return if da == db {
                    u64::from(ba.count_or(&bb))
                } else {
                    u64::from(ba.count() + bb.count())
                };
            }
            let ca = edge_count(&active[0], level);
            let cb = edge_count(&active[1], level);
            // SAFETY: active[0] and active[1] are live subtrees at `level`.
            let ci = F::intersection_len(&active[0], &active[1], level);
            return ca + cb - ci;
        }

        // 1. If any edge is FullExpanse, the union covers the whole expanse.
        if active
            .iter()
            .any(|e| e.tag_byte() == EdgeType::FullExpanse as u8)
        {
            return full_count(level);
        }

        // 2. Check if all active edges reduce to a final-byte bitmap (real level 1):
        let mut all_bms = true;
        let mut bm_pairs: [(u64, Bitmap256); 16] = [(0, Bitmap256::new()); 16];
        let bm_pairs_vec: Vec<(u64, Bitmap256)>;
        for (i, e) in active.iter().enumerate() {
            if let Some((dv, bm)) = as_level1_bitmap(e, level) {
                if i < 16 {
                    bm_pairs[i] = (dv, bm);
                }
            } else {
                all_bms = false;
                break;
            }
        }

        if all_bms {
            let pairs: &[(u64, Bitmap256)] = if active.len() <= 16 {
                &bm_pairs[..active.len()]
            } else {
                bm_pairs_vec = active
                    .iter()
                    .map(|e| as_level1_bitmap(e, level).unwrap())
                    .collect();
                &bm_pairs_vec
            };

            let mut total = 0u64;
            let mut handled = [false; 32];
            let mut handled_vec: Vec<bool>;
            let is_handled: &mut [bool] = if pairs.len() <= 32 {
                &mut handled[..pairs.len()]
            } else {
                handled_vec = vec![false; pairs.len()];
                &mut handled_vec
            };

            for i in 0..pairs.len() {
                if is_handled[i] {
                    continue;
                }
                let (target_dv, mut acc_bm) = pairs[i];
                is_handled[i] = true;
                let mut match_j = None;
                let mut matches_count = 1;
                for j in (i + 1)..pairs.len() {
                    if !is_handled[j] && pairs[j].0 == target_dv {
                        match_j = Some(j);
                        matches_count += 1;
                        acc_bm = acc_bm.or(&pairs[j].1);
                        is_handled[j] = true;
                    }
                }
                if matches_count == 1 {
                    total += u64::from(pairs[i].1.count());
                } else if matches_count == 2 {
                    total += u64::from(pairs[i].1.count_or(&pairs[match_j.unwrap()].1));
                } else {
                    total += u64::from(acc_bm.count());
                }
            }
            return total;
        }

        // 3. If any active edge is a terminal, extract its keys and probe against the rest.
        for (i, e) in active.iter().enumerate() {
            let mut kbuf = [0u64; 256];
            if !is_branch(e.tag_byte())
                && let Some(n) = terminal_remainders_buf(e, level, &mut kbuf)
            {
                let keys = &kbuf[..n];
                let mut remaining: Vec<Edge> = Vec::with_capacity(active.len() - 1);
                for (j, other_e) in active.iter().enumerate() {
                    if j != i {
                        remaining.push(*other_e);
                    }
                }
                let mut solo_keys = 0u64;
                for &k in keys {
                    let mut in_other = false;
                    for rem_e in &remaining {
                        if get::test_set(rem_e, k, level) {
                            in_other = true;
                            break;
                        }
                    }
                    if !in_other {
                        solo_keys += 1;
                    }
                }
                // SAFETY: remaining edges live at `level`.
                return solo_keys + F::union_len_many(&remaining, level);
            }
        }

        // 4. All active edges are branches.
        // SAFETY: active contains only branches live at `level`.
        union_branches_many_impl::<F>(active, level)
    }
}

/// Union cardinality when all edges are branches covering `level`.
#[inline(always)]
unsafe fn union_branches_many_impl<F: AlgebraFlavor>(edges: &[Edge], level: u8) -> u64 {
    // SAFETY: caller guarantees all edges in `edges` reference live branch nodes at `level`.
    unsafe {
        let mut digit_maps: [Bitmap256; 16] = [Bitmap256::new(); 16];
        let mut digit_maps_vec: Vec<Bitmap256>;
        let mut active_digits = Bitmap256::new();

        let maps: &[Bitmap256] = if edges.len() <= 16 {
            for (i, e) in edges.iter().enumerate() {
                let acc = branch_accessor(e, level);
                let bm = acc.populated_bitmap();
                digit_maps[i] = bm;
                active_digits = active_digits.or(&bm);
            }
            &digit_maps[..edges.len()]
        } else {
            digit_maps_vec = Vec::with_capacity(edges.len());
            for e in edges {
                let acc = branch_accessor(e, level);
                let bm = acc.populated_bitmap();
                active_digits = active_digits.or(&bm);
                digit_maps_vec.push(bm);
            }
            &digit_maps_vec
        };

        let mut total = 0u64;
        let mut from = active_digits.next_set(0);

        while let Some(d) = from {
            let mut child_buf: [Edge; 16] = [Edge::NULL; 16];
            let mut child_vec: Vec<Edge>;
            let mut count_present = 0usize;

            if edges.len() <= 16 {
                for (i, e) in edges.iter().enumerate() {
                    if maps[i].test(d) {
                        let c = child_by_digit(e, level, d);
                        if !c.is_null() {
                            child_buf[count_present] = c;
                            count_present += 1;
                        }
                    }
                }
                if count_present == 1 {
                    total += subtree_count(&child_buf[0], level - 1);
                } else if count_present > 1 {
                    // SAFETY: child_buf contains live subtrees at level - 1.
                    total += F::union_len_many(&child_buf[..count_present], level - 1);
                }
            } else {
                child_vec = Vec::new();
                for (i, e) in edges.iter().enumerate() {
                    if maps[i].test(d) {
                        let c = child_by_digit(e, level, d);
                        if !c.is_null() {
                            child_vec.push(c);
                        }
                    }
                }
                if child_vec.len() == 1 {
                    total += subtree_count(&child_vec[0], level - 1);
                } else if child_vec.len() > 1 {
                    // SAFETY: child_vec contains live subtrees at level - 1.
                    total += F::union_len_many(&child_vec, level - 1);
                }
            }

            from = if d == 255 {
                None
            } else {
                active_digits.next_set(d + 1)
            };
        }

        total
    }
}

// ---------------------------------------------------------------------------
// Runtime feature dispatch & entry clones (x86-64 without compile-time popcnt)
// ---------------------------------------------------------------------------

#[cfg(all(target_arch = "x86_64", not(target_feature = "popcnt")))]
#[inline(never)]
unsafe fn intersection_len_swar(ea: &Edge, eb: &Edge, level: u8) -> u64 {
    // SAFETY: forwarded contract.
    unsafe { intersection_len_impl::<SwarFlavor>(ea, eb, level) }
}

#[cfg(all(target_arch = "x86_64", not(target_feature = "popcnt")))]
#[target_feature(enable = "popcnt")]
unsafe fn intersection_len_popcnt(ea: &Edge, eb: &Edge, level: u8) -> u64 {
    // SAFETY: forwarded contract; caller verified popcnt CPUID support.
    unsafe { intersection_len_impl::<PopcntFlavor>(ea, eb, level) }
}

#[cfg(all(target_arch = "x86_64", not(target_feature = "popcnt")))]
#[inline(never)]
unsafe fn intersection_len_many_swar(edges: &[Edge], level: u8) -> u64 {
    // SAFETY: forwarded contract.
    unsafe { intersection_len_many_impl::<SwarFlavor>(edges, level) }
}

#[cfg(all(target_arch = "x86_64", not(target_feature = "popcnt")))]
#[target_feature(enable = "popcnt")]
unsafe fn intersection_len_many_popcnt(edges: &[Edge], level: u8) -> u64 {
    // SAFETY: forwarded contract; caller verified popcnt CPUID support.
    unsafe { intersection_len_many_impl::<PopcntFlavor>(edges, level) }
}

#[cfg(all(target_arch = "x86_64", not(target_feature = "popcnt")))]
#[inline(never)]
unsafe fn union_len_many_swar(edges: &[Edge], level: u8) -> u64 {
    // SAFETY: forwarded contract.
    unsafe { union_len_many_impl::<SwarFlavor>(edges, level) }
}

#[cfg(all(target_arch = "x86_64", not(target_feature = "popcnt")))]
#[target_feature(enable = "popcnt")]
unsafe fn union_len_many_popcnt(edges: &[Edge], level: u8) -> u64 {
    // SAFETY: forwarded contract; caller verified popcnt CPUID support.
    unsafe { union_len_many_impl::<PopcntFlavor>(edges, level) }
}

// ---------------------------------------------------------------------------
// Crate-public cardinality entry points
// ---------------------------------------------------------------------------

/// Intersection cardinality of two non-null subtrees covering the same
/// expanse of `level` undecoded bytes. `eb` should be the smaller side.
///
/// # Safety
///
/// Both edges must reference live, well-formed subtrees at `level`, each
/// pointer-tagged edge referencing a live node of its tagged type.
#[inline(always)]
pub(crate) unsafe fn intersection_len(ea: &Edge, eb: &Edge, level: u8) -> u64 {
    #[cfg(all(target_arch = "x86_64", not(target_feature = "popcnt")))]
    // SAFETY: contracts forwarded; `available()` gates the popcnt clone.
    unsafe {
        if crate::bits::popcnt_rt::available() {
            intersection_len_popcnt(ea, eb, level)
        } else {
            intersection_len_swar(ea, eb, level)
        }
    }
    #[cfg(any(not(target_arch = "x86_64"), target_feature = "popcnt"))]
    {
        // SAFETY: forwarded caller contract.
        unsafe { intersection_len_impl::<NativeFlavor>(ea, eb, level) }
    }
}

/// Intersection cardinality of k non-null subtrees covering the same
/// expanse of `level` undecoded bytes (#610).
///
/// # Safety
///
/// All edges in `edges` must reference live, well-formed subtrees at `level`.
#[inline(always)]
pub(crate) unsafe fn intersection_len_many(edges: &[Edge], level: u8) -> u64 {
    #[cfg(all(target_arch = "x86_64", not(target_feature = "popcnt")))]
    // SAFETY: contracts forwarded; `available()` gates the popcnt clone.
    unsafe {
        if crate::bits::popcnt_rt::available() {
            intersection_len_many_popcnt(edges, level)
        } else {
            intersection_len_many_swar(edges, level)
        }
    }
    #[cfg(any(not(target_arch = "x86_64"), target_feature = "popcnt"))]
    {
        // SAFETY: forwarded caller contract.
        unsafe { intersection_len_many_impl::<NativeFlavor>(edges, level) }
    }
}

/// Union cardinality of k subtrees covering the same expanse of `level`
/// undecoded bytes (#610).
///
/// # Safety
///
/// All non-null edges in `edges` must reference live subtrees at `level`.
#[inline(always)]
pub(crate) unsafe fn union_len_many(edges: &[Edge], level: u8) -> u64 {
    #[cfg(all(target_arch = "x86_64", not(target_feature = "popcnt")))]
    // SAFETY: contracts forwarded; `available()` gates the popcnt clone.
    unsafe {
        if crate::bits::popcnt_rt::available() {
            union_len_many_popcnt(edges, level)
        } else {
            union_len_many_swar(edges, level)
        }
    }
    #[cfg(any(not(target_arch = "x86_64"), target_feature = "popcnt"))]
    {
        // SAFETY: forwarded caller contract.
        unsafe { union_len_many_impl::<NativeFlavor>(edges, level) }
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;
    use crate::set::ExpanseSet;

    #[test]
    fn test_algebra_flavor_sanity() {
        let mut sa = ExpanseSet::new();
        let mut sb = ExpanseSet::new();
        for i in 0..200u64 {
            sa.insert(i);
        }
        for i in 100..250u64 {
            sb.insert(i);
        }

        assert_eq!(sa.intersection_len(&sb), 100);
        assert_eq!(sb.intersection_len(&sa), 100);
        assert_eq!(sa.union_len(&sb), 250);
        assert_eq!(ExpanseSet::intersection_len_many(&[&sa, &sb]), 100);
        assert_eq!(ExpanseSet::union_len_many(&[&sa, &sb]), 250);
    }

    #[test]
    #[cfg(all(target_arch = "x86_64", not(target_feature = "popcnt")))]
    fn test_popcnt_swar_arm_parity() {
        if !crate::bits::popcnt_rt::available() {
            return;
        }

        let test_cases: Vec<(Vec<u64>, Vec<u64>)> = vec![
            // Aligned level-1 bitmaps
            ((0..200).collect(), (100..250).collect()),
            // Disjoint bitmaps
            ((0..100).collect(), (200..300).collect()),
            // Multi-level branches
            (
                (0..5000).step_by(3).collect(),
                (0..5000).step_by(5).collect(),
            ),
            // Wide sparse
            (
                vec![1, 1 << 16, 1 << 32, 1 << 48],
                vec![1, 1 << 16, 1 << 40, 1 << 48],
            ),
        ];

        for (ka, kb) in test_cases {
            let mut sa = ExpanseSet::new();
            for &k in &ka {
                sa.insert(k);
            }
            let mut sb = ExpanseSet::new();
            for &k in &kb {
                sb.insert(k);
            }

            let ea = match sa.root_tree_edge() {
                Some(top) => top,
                None => continue,
            };
            let eb = match sb.root_tree_edge() {
                Some(top) => top,
                None => continue,
            };

            // SAFETY: top edges are live at level 8.
            let swar_and = unsafe { intersection_len_swar(&ea, &eb, 8) };
            // SAFETY: top edges are live at level 8; popcnt CPUID verified above.
            let popcnt_and = unsafe { intersection_len_popcnt(&ea, &eb, 8) };
            assert_eq!(swar_and, popcnt_and, "intersection_len parity mismatch");

            let edges = [ea, eb];
            // SAFETY: edges are live at level 8.
            let swar_and_many = unsafe { intersection_len_many_swar(&edges, 8) };
            // SAFETY: edges are live at level 8; popcnt CPUID verified above.
            let popcnt_and_many = unsafe { intersection_len_many_popcnt(&edges, 8) };
            assert_eq!(
                swar_and_many, popcnt_and_many,
                "intersection_len_many parity mismatch"
            );

            // SAFETY: edges are live at level 8.
            let swar_or_many = unsafe { union_len_many_swar(&edges, 8) };
            // SAFETY: edges are live at level 8; popcnt CPUID verified above.
            let popcnt_or_many = unsafe { union_len_many_popcnt(&edges, 8) };
            assert_eq!(
                swar_or_many, popcnt_or_many,
                "union_len_many parity mismatch"
            );
        }
    }
}
