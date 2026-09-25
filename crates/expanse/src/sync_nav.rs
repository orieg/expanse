//! Validated ordered navigation over a shared tree (#900).
//!
//! [`next_validated`] and [`prev_validated`] answer the same questions as
//! `nav::next` and `nav::prev` under the optimistic read protocol, without
//! excluding writers. They are separate from `nav.rs` so the single-threaded
//! paths compile exactly as before.
//!
//! `get`'s walk validates hand-over-hand: it moves one cover down the path.
//! That is not enough here. A search that finds nothing at or below its target
//! in a child subtree goes back to the parent and descends a sibling, so its
//! answer depends on several subtrees. A search that dropped the child's
//! snapshot before the sibling descent could return a key that was never the
//! answer at any instant (`loom_ordered_read_hand_over_hand_is_not_enough`).
//!
//! The rule used instead is a *retained read set*
//! (`loom_ordered_read_retained_read_set`):
//!
//! - every branch version the search sampled is kept, and all of them are
//!   validated again, with the tree version, after the last load. An answer,
//!   and every empty subtree the search passed over, then held at one instant:
//!   the one after the last sample and before the first final validation.
//! - memory safety is separate from that, and follows `walk_validated`: the
//!   node that holds a pointer is validated after the pointer is loaded and
//!   before it is dereferenced.
//!
//! Terminal payloads (linear leaves, immediates, bitmap leaves, a `BranchB`
//! subarray) carry no version and are covered by the branch whose slot points
//! at them, or by the tree version for the top edge (`docs/ARCHITECTURE.md`
//! §4.1). A search backtracks at most once below a branch with a live digit on
//! the searched side, so a consistent read holds at most ℓ + 5 branch
//! versions for a backtrack at level ℓ, 13 at most
//! (`scripts/olc_bounds.py::ordered_read_set_branches`). A read that needs more
//! than [`READ_SET_CAP`] can only have read a moving tree, and restarts.

use crate::bits::shared_word;
use crate::leaf;
use crate::mutate::{key_low, pow256};
use crate::node::{BranchB, BranchL3, BranchL7, BranchU, Edge, LeafBitmap1, LeafBitmapL};
use crate::occ::{SeqVersion, node_sample, node_validate, version_cell};
use crate::sync::{Retry, RootSnapshot};
use crate::types::{EdgeTag, EdgeType, Key, digit};
use core::mem::MaybeUninit;

/// Branch versions one search may retain before it restarts. Above the
/// consistent-read bound of 13, so only a read of a moving tree reaches it.
const READ_SET_CAP: usize = 16;

/// Every branch version a search sampled, kept until the final validation.
struct ReadSet {
    entries: [MaybeUninit<(*const u32, u32)>; READ_SET_CAP],
    len: usize,
}

impl ReadSet {
    #[inline(always)]
    const fn new() -> Self {
        Self {
            entries: [const { MaybeUninit::uninit() }; READ_SET_CAP],
            len: 0,
        }
    }

    /// Samples the version at `vp` and retains it.
    ///
    /// # Safety
    ///
    /// `vp` is the version field of an EBR-live branch node.
    #[inline(always)]
    unsafe fn sample(&mut self, vp: *const u32) -> Result<u32, Retry> {
        // SAFETY: live version field, per this function's contract.
        let snap = node_sample(unsafe { version_cell(vp) }).ok_or(Retry)?;
        if self.len == READ_SET_CAP {
            return Err(Retry);
        }
        self.entries[self.len] = MaybeUninit::new((vp, snap));
        self.len += 1;
        Ok(snap)
    }

    /// Validates every retained version after the search's last load.
    ///
    /// # Safety
    ///
    /// The caller still holds the pin under which every entry was sampled.
    #[inline(always)]
    unsafe fn validate_all(&self) -> bool {
        self.entries[..self.len].iter().all(|e| {
            // SAFETY: entries below `len` were written by `sample`.
            let (vp, snap) = unsafe { e.assume_init() };
            // SAFETY: sampled from an EBR-live node under the same pin.
            node_validate(unsafe { version_cell(vp) }, snap)
        })
    }
}

/// The version that covers an edge copy and the payload behind it: the tree
/// version for the top edge, the holding branch's otherwise.
#[derive(Clone, Copy)]
enum Holder<'a> {
    Tree(&'a SeqVersion, u64),
    Node(*const u32, u32),
}

impl Holder<'_> {
    /// True while the holder is unchanged, so pointers loaded under it are
    /// EBR-live and consistent with each other.
    #[inline(always)]
    fn ok(self) -> bool {
        match self {
            Holder::Tree(v, s) => v.validate(s),
            // SAFETY: the holding node is EBR-live for the reader's pin.
            Holder::Node(p, s) => node_validate(unsafe { version_cell(p) }, s),
        }
    }
}

/// Composes a branch digit with a child-level result suffix.
#[inline(always)]
fn compose(d: u8, rem: u64, level: u8) -> u64 {
    (u64::from(d) << ((level - 1) * 8)) | rem
}

/// The version field, level, digit count, digits and edge base of a linear
/// branch, loaded from memory a writer may be changing.
///
/// # Safety
///
/// `edge` references an EBR-live `BranchL3` (`is_l3`) or `BranchL7`.
#[inline(always)]
unsafe fn linear_branch(edge: &Edge, is_l3: bool) -> (*const u32, u8, usize, [u8; 8], *const Edge) {
    let node = edge.node_ptr();
    // SAFETY: EBR-live node per contract; field projections, and the
    // header's meta and digit words loaded atomically (#1086).
    unsafe {
        if is_l3 {
            let b = node.cast::<BranchL3>();
            let h = crate::node::BranchHeader::load_at(&raw const (*b).hdr);
            (
                &raw const (*b).hdr.version,
                h.level,
                h.num as usize,
                h.digits,
                (&raw const (*b).edges).cast::<Edge>(),
            )
        } else {
            let b = node.cast::<BranchL7>();
            let h = crate::node::BranchHeader::load_at(&raw const (*b).hdr);
            (
                &raw const (*b).hdr.version,
                h.level,
                h.num as usize,
                h.digits,
                (&raw const (*b).edges).cast::<Edge>(),
            )
        }
    }
}

/// A skipping node's position against `suffix`: `Less` and `Greater` when the
/// whole node sits above or below it, otherwise the suffix inside the node.
#[inline(always)]
fn skip_cmp(
    edge: &Edge,
    node_level: u8,
    level: u8,
    suffix: u64,
) -> (core::cmp::Ordering, u64, u32) {
    if node_level < level {
        let dv = crate::mutate::decode_value(edge, node_level, level);
        let shift = 8 * u32::from(node_level);
        ((suffix >> shift).cmp(&dv), dv, shift)
    } else {
        (core::cmp::Ordering::Equal, 0, 0)
    }
}

/// Smallest key `>= key` under `root`, with its value (0 for sets).
///
/// # Safety
///
/// As `walk_validated`: `snap` is an even version sampled from `ver`, `root`
/// was copied after that sample, and the caller holds an epoch pin for the
/// whole call.
pub(crate) unsafe fn next_validated<const MAP: bool>(
    root: RootSnapshot,
    key: Key,
    ver: &SeqVersion,
    snap: u64,
) -> Result<Option<(u64, u64)>, Retry> {
    // The root snapshot was copied after `snap` was sampled.
    if !ver.validate(snap) {
        return Err(Retry);
    }
    let mut rs = ReadSet::new();
    let found = match root {
        RootSnapshot::Empty => None,
        // SAFETY: root state validated just above, under the caller's pin.
        RootSnapshot::Leaf { ptr, pop } => unsafe { root_leaf::<MAP>(ptr, pop, key, true) },
        RootSnapshot::Tree { top } => {
            // SAFETY: the top edge copy is validated against the tree version.
            unsafe { next_in::<MAP>(&top, key, 8, Holder::Tree(ver, snap), &mut rs)? }
        }
    };
    // SAFETY: same pin as every sample.
    if !unsafe { rs.validate_all() } || !ver.validate(snap) {
        return Err(Retry);
    }
    Ok(found)
}

/// Largest key `<= key` under `root`, with its value (0 for sets).
///
/// # Safety
///
/// As [`next_validated`].
pub(crate) unsafe fn prev_validated<const MAP: bool>(
    root: RootSnapshot,
    key: Key,
    ver: &SeqVersion,
    snap: u64,
) -> Result<Option<(u64, u64)>, Retry> {
    if !ver.validate(snap) {
        return Err(Retry);
    }
    let mut rs = ReadSet::new();
    let found = match root {
        RootSnapshot::Empty => None,
        // SAFETY: root state validated just above, under the caller's pin.
        RootSnapshot::Leaf { ptr, pop } => unsafe { root_leaf::<MAP>(ptr, pop, key, false) },
        RootSnapshot::Tree { top } => {
            // SAFETY: the top edge copy is validated against the tree version.
            unsafe { prev_in::<MAP>(&top, key, 8, Holder::Tree(ver, snap), &mut rs)? }
        }
    };
    // SAFETY: same pin as every sample.
    if !unsafe { rs.validate_all() } || !ver.validate(snap) {
        return Err(Retry);
    }
    Ok(found)
}

/// The successor (`forward`) or predecessor of `key` in a root leaf of `pop`
/// sorted keys, with the map value behind it. Loads are validated by the
/// caller's final tree-version check.
///
/// # Safety
///
/// `(ptr, pop)` was validated against the tree version and is EBR-live.
unsafe fn root_leaf<const MAP: bool>(
    ptr: *const u8,
    pop: usize,
    key: Key,
    forward: bool,
) -> Option<(u64, u64)> {
    let keys = ptr.cast::<u64>();
    let (mut lo, mut hi) = (0usize, pop);
    while lo < hi {
        let mid = (lo + hi) / 2;
        // SAFETY: `mid < pop`, in bounds of the live root leaf.
        let k = unsafe { shared_word::load::<true>(keys.add(mid)) };
        let right = if forward { k < key } else { k <= key };
        if right {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    let at = if forward {
        (lo < pop).then_some(lo)?
    } else {
        lo.checked_sub(1)?
    };
    // SAFETY: `at < pop`.
    let k = unsafe { shared_word::load::<true>(keys.add(at)) };
    let v = if MAP {
        // SAFETY: the value area begins at the class-based offset and holds
        // `pop` values.
        unsafe {
            shared_word::load::<true>(
                ptr.add(crate::map::leaf_values_offset(pop))
                    .cast::<u64>()
                    .add(at),
            )
        }
    } else {
        0
    };
    Some((k, v))
}

/// `nav::next` under the retained read set.
///
/// # Safety
///
/// `edge` is a copy validated against `holder`, and the caller holds the pin.
unsafe fn next_in<const MAP: bool>(
    edge: &Edge,
    suffix: u64,
    level: u8,
    holder: Holder<'_>,
    rs: &mut ReadSet,
) -> Result<Option<(u64, u64)>, Retry> {
    use core::cmp::Ordering::{Equal, Greater, Less};
    let Some(tag) = edge.tag() else {
        return Err(Retry);
    };
    match tag {
        EdgeTag::Structural(EdgeType::Null) => Ok(None),

        EdgeTag::Immed(im) => {
            if im.key_bytes() != level {
                return Err(Retry);
            }
            let keys = if MAP {
                crate::mutate::immed_map_keys(edge, im)
            } else {
                crate::mutate::immed_keys(edge, im)
            };
            let slot = keys.partition_point(|&k| k < suffix);
            let Some(&k) = keys.get(slot) else {
                return Ok(None);
            };
            let v = if MAP {
                // SAFETY: validated immediate edge copy.
                unsafe { immed_value(edge, im, slot) }
            } else {
                0
            };
            Ok(Some((k, v)))
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
            let kb = t.leaf_key_bytes().ok_or(Retry)?;
            if kb > level {
                return Err(Retry);
            }
            let (ord, dv, shift) = skip_cmp(edge, kb, level, suffix);
            let low = match ord {
                Less => 0,
                Equal => key_low(suffix, kb),
                Greater => return Ok(None),
            };
            let pop = edge.pop0(kb) as usize + 1;
            let base = edge.node_ptr();
            let keys = if MAP {
                base.wrapping_add(leaf::map_keys_offset(pop))
            } else {
                base
            };
            // SAFETY: a validated edge copy names a live leaf of `pop` keys of
            // `kb` bytes; the result is covered by the holder's final validation.
            let slot = unsafe { leaf::shared_keys::lower_bound(keys, pop, kb as usize, low) };
            if slot == pop {
                return Ok(None);
            }
            // SAFETY: `slot < pop`.
            let k = unsafe { leaf::shared_keys::read(keys, slot, kb as usize, pop) };
            let v = if MAP {
                // SAFETY: map leaves hold `pop` values at the base.
                unsafe { crate::bits::shared_word::load::<true>(base.cast::<u64>().add(slot)) }
            } else {
                0
            };
            Ok(Some(((dv << shift) | k, v)))
        }

        EdgeTag::Structural(EdgeType::LeafB1) => {
            let (ord, dv, shift) = skip_cmp(edge, 1, level, suffix);
            let from = match ord {
                Less => 0,
                Equal => key_low(suffix, 1) as u8,
                Greater => return Ok(None),
            };
            // SAFETY: validated edge copy → live bitmap leaf of the tagged form.
            let found = unsafe { bitmap_leaf::<MAP>(edge, holder, from, true)? };
            Ok(found.map(|(d, v)| ((dv << shift) | u64::from(d), v)))
        }

        EdgeTag::Structural(EdgeType::FullExpanse) => {
            if MAP {
                return Err(Retry);
            }
            Ok(Some((suffix, 0)))
        }

        EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
            let is_l3 = matches!(t, EdgeType::BranchL3);
            // SAFETY: validated edge copy → live branch; loads validated below.
            let (vp, bl, num, digits, edges) = unsafe { linear_branch(edge, is_l3) };
            // SAFETY: `vp` is the live branch's version field.
            let nsnap = unsafe { rs.sample(vp)? };
            if !(2..=level).contains(&bl) || num > if is_l3 { 3 } else { 7 } {
                return Err(Retry);
            }
            let (ord, dv, shift) = skip_cmp(edge, bl, level, suffix);
            let suffix = match ord {
                Less => 0,
                Equal => key_low(suffix, bl),
                Greater => return Ok(None),
            };
            let d = digit(suffix, bl);
            let start = digits[..num].partition_point(|&bd| bd < d);
            for (slot, &bd) in digits.iter().enumerate().take(num).skip(start) {
                let rem = if bd == d { key_low(suffix, bl - 1) } else { 0 };
                // SAFETY: `slot < num <= capacity`; validated before it is used.
                let child = unsafe { Edge::load_at::<true>(edges.add(slot)) };
                let here = Holder::Node(vp, nsnap);
                if !here.ok() {
                    return Err(Retry);
                }
                let mark = rs.len;
                // SAFETY: child copy validated against this branch.
                if let Some((r, v)) = unsafe { next_in::<MAP>(&child, rem, bl - 1, here, rs)? } {
                    return Ok(Some(((dv << shift) | compose(bd, r, bl), v)));
                }
                backtrack(rs, mark);
            }
            Ok(None)
        }

        EdgeTag::Structural(EdgeType::BranchB) => {
            let node = edge.node_ptr().cast::<BranchB>();
            // SAFETY: validated edge copy → live BranchB; field projection.
            let vp = unsafe { &raw const (*node).version };
            // SAFETY: live version field.
            let nsnap = unsafe { rs.sample(vp)? };
            // SAFETY: live node; a torn level is rejected below.
            let bl = unsafe { (*node).level };
            if !(2..=level).contains(&bl) {
                return Err(Retry);
            }
            let (ord, dv, shift) = skip_cmp(edge, bl, level, suffix);
            let suffix = match ord {
                Less => 0,
                Equal => key_low(suffix, bl),
                Greater => return Ok(None),
            };
            let d = digit(suffix, bl);
            let here = Holder::Node(vp, nsnap);
            // SAFETY: live node; bitmap loads are validated before use.
            let mut cur = unsafe {
                crate::bits::shared_bitmap::next_set::<true>(&raw const (*node).bitmap, d)
            };
            while let Some(bd) = cur {
                let rem = if bd == d { key_low(suffix, bl - 1) } else { 0 };
                // SAFETY: as above.
                let child = unsafe { branch_b_child(node, bd, here)? };
                let mark = rs.len;
                // SAFETY: child copy validated against this branch.
                if let Some((r, v)) = unsafe { next_in::<MAP>(&child, rem, bl - 1, here, rs)? } {
                    return Ok(Some(((dv << shift) | compose(bd, r, bl), v)));
                }
                backtrack(rs, mark);
                cur = if bd == 255 {
                    None
                } else {
                    // SAFETY: as above.
                    unsafe {
                        crate::bits::shared_bitmap::next_set::<true>(
                            &raw const (*node).bitmap,
                            bd + 1,
                        )
                    }
                };
            }
            Ok(None)
        }

        EdgeTag::Structural(EdgeType::BranchU) => {
            let node = edge.node_ptr().cast::<BranchU>();
            // SAFETY: validated edge copy → live BranchU; field projection.
            let vp = unsafe { &raw const (*node).version };
            // SAFETY: live version field.
            let nsnap = unsafe { rs.sample(vp)? };
            let here = Holder::Node(vp, nsnap);
            let d = digit(suffix, level);
            for bd in d..=255u8 {
                // SAFETY: direct index into the live 256-slot node.
                let child = unsafe {
                    Edge::load_at::<true>(
                        (&raw const (*node).edges).cast::<Edge>().add(bd as usize),
                    )
                };
                if child.is_null() {
                    continue;
                }
                if !here.ok() {
                    return Err(Retry);
                }
                let rem = if bd == d {
                    key_low(suffix, level - 1)
                } else {
                    0
                };
                let mark = rs.len;
                // SAFETY: child copy validated against this branch.
                if let Some((r, v)) = unsafe { next_in::<MAP>(&child, rem, level - 1, here, rs)? } {
                    return Ok(Some((compose(bd, r, level), v)));
                }
                backtrack(rs, mark);
            }
            Ok(None)
        }
    }
}

/// `nav::prev` under the retained read set.
///
/// # Safety
///
/// As [`next_in`].
unsafe fn prev_in<const MAP: bool>(
    edge: &Edge,
    suffix: u64,
    level: u8,
    holder: Holder<'_>,
    rs: &mut ReadSet,
) -> Result<Option<(u64, u64)>, Retry> {
    use core::cmp::Ordering::{Equal, Greater, Less};
    let Some(tag) = edge.tag() else {
        return Err(Retry);
    };
    match tag {
        EdgeTag::Structural(EdgeType::Null) => Ok(None),

        EdgeTag::Immed(im) => {
            if im.key_bytes() != level {
                return Err(Retry);
            }
            let keys = if MAP {
                crate::mutate::immed_map_keys(edge, im)
            } else {
                crate::mutate::immed_keys(edge, im)
            };
            let Some(slot) = keys.partition_point(|&k| k <= suffix).checked_sub(1) else {
                return Ok(None);
            };
            let v = if MAP {
                // SAFETY: validated immediate edge copy.
                unsafe { immed_value(edge, im, slot) }
            } else {
                0
            };
            Ok(Some((keys[slot], v)))
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
            let kb = t.leaf_key_bytes().ok_or(Retry)?;
            if kb > level {
                return Err(Retry);
            }
            let (ord, dv, shift) = skip_cmp(edge, kb, level, suffix);
            let low = match ord {
                Less => return Ok(None),
                Equal => key_low(suffix, kb),
                Greater => key_low(u64::MAX, kb),
            };
            let pop = edge.pop0(kb) as usize + 1;
            let base = edge.node_ptr();
            let keys = if MAP {
                base.wrapping_add(leaf::map_keys_offset(pop))
            } else {
                base
            };
            let bound = if low == key_low(u64::MAX, kb) {
                pop
            } else {
                // SAFETY: as in `next_in`.
                unsafe { leaf::shared_keys::lower_bound(keys, pop, kb as usize, low + 1) }
            };
            let Some(slot) = bound.checked_sub(1) else {
                return Ok(None);
            };
            // SAFETY: `slot < pop`.
            let k = unsafe { leaf::shared_keys::read(keys, slot, kb as usize, pop) };
            let v = if MAP {
                // SAFETY: map leaves hold `pop` values at the base.
                unsafe { crate::bits::shared_word::load::<true>(base.cast::<u64>().add(slot)) }
            } else {
                0
            };
            Ok(Some(((dv << shift) | k, v)))
        }

        EdgeTag::Structural(EdgeType::LeafB1) => {
            let (ord, dv, shift) = skip_cmp(edge, 1, level, suffix);
            let from = match ord {
                Less => return Ok(None),
                Equal => key_low(suffix, 1) as u8,
                Greater => 255,
            };
            // SAFETY: validated edge copy → live bitmap leaf of the tagged form.
            let found = unsafe { bitmap_leaf::<MAP>(edge, holder, from, false)? };
            Ok(found.map(|(d, v)| ((dv << shift) | u64::from(d), v)))
        }

        EdgeTag::Structural(EdgeType::FullExpanse) => {
            if MAP {
                return Err(Retry);
            }
            Ok(Some((suffix.min(pow256(level) - 1), 0)))
        }

        EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
            let is_l3 = matches!(t, EdgeType::BranchL3);
            // SAFETY: validated edge copy → live branch; loads validated below.
            let (vp, bl, num, digits, edges) = unsafe { linear_branch(edge, is_l3) };
            // SAFETY: `vp` is the live branch's version field.
            let nsnap = unsafe { rs.sample(vp)? };
            if !(2..=level).contains(&bl) || num > if is_l3 { 3 } else { 7 } {
                return Err(Retry);
            }
            let (ord, dv, shift) = skip_cmp(edge, bl, level, suffix);
            let suffix = match ord {
                Less => return Ok(None),
                Equal => key_low(suffix, bl),
                Greater => pow256(bl) - 1,
            };
            let d = digit(suffix, bl);
            let bound = digits[..num].partition_point(|&bd| bd <= d);
            for (slot, &bd) in digits.iter().enumerate().take(bound).rev() {
                let rem = if bd == d {
                    key_low(suffix, bl - 1)
                } else {
                    pow256(bl - 1) - 1
                };
                // SAFETY: `slot < num <= capacity`; validated before it is used.
                let child = unsafe { Edge::load_at::<true>(edges.add(slot)) };
                let here = Holder::Node(vp, nsnap);
                if !here.ok() {
                    return Err(Retry);
                }
                let mark = rs.len;
                // SAFETY: child copy validated against this branch.
                if let Some((r, v)) = unsafe { prev_in::<MAP>(&child, rem, bl - 1, here, rs)? } {
                    return Ok(Some(((dv << shift) | compose(bd, r, bl), v)));
                }
                backtrack(rs, mark);
            }
            Ok(None)
        }

        EdgeTag::Structural(EdgeType::BranchB) => {
            let node = edge.node_ptr().cast::<BranchB>();
            // SAFETY: validated edge copy → live BranchB; field projection.
            let vp = unsafe { &raw const (*node).version };
            // SAFETY: live version field.
            let nsnap = unsafe { rs.sample(vp)? };
            // SAFETY: live node; a torn level is rejected below.
            let bl = unsafe { (*node).level };
            if !(2..=level).contains(&bl) {
                return Err(Retry);
            }
            let (ord, dv, shift) = skip_cmp(edge, bl, level, suffix);
            let suffix = match ord {
                Less => return Ok(None),
                Equal => key_low(suffix, bl),
                Greater => pow256(bl) - 1,
            };
            let d = digit(suffix, bl);
            let here = Holder::Node(vp, nsnap);
            // SAFETY: live node; bitmap loads are validated before use.
            let mut cur = unsafe {
                crate::bits::shared_bitmap::prev_set::<true>(&raw const (*node).bitmap, d)
            };
            while let Some(bd) = cur {
                let rem = if bd == d {
                    key_low(suffix, bl - 1)
                } else {
                    pow256(bl - 1) - 1
                };
                // SAFETY: as above.
                let child = unsafe { branch_b_child(node, bd, here)? };
                let mark = rs.len;
                // SAFETY: child copy validated against this branch.
                if let Some((r, v)) = unsafe { prev_in::<MAP>(&child, rem, bl - 1, here, rs)? } {
                    return Ok(Some(((dv << shift) | compose(bd, r, bl), v)));
                }
                backtrack(rs, mark);
                cur = if bd == 0 {
                    None
                } else {
                    // SAFETY: as above.
                    unsafe {
                        crate::bits::shared_bitmap::prev_set::<true>(
                            &raw const (*node).bitmap,
                            bd - 1,
                        )
                    }
                };
            }
            Ok(None)
        }

        EdgeTag::Structural(EdgeType::BranchU) => {
            let node = edge.node_ptr().cast::<BranchU>();
            // SAFETY: validated edge copy → live BranchU; field projection.
            let vp = unsafe { &raw const (*node).version };
            // SAFETY: live version field.
            let nsnap = unsafe { rs.sample(vp)? };
            let here = Holder::Node(vp, nsnap);
            let d = digit(suffix, level);
            for bd in (0..=d).rev() {
                // SAFETY: direct index into the live 256-slot node.
                let child = unsafe {
                    Edge::load_at::<true>(
                        (&raw const (*node).edges).cast::<Edge>().add(bd as usize),
                    )
                };
                if child.is_null() {
                    continue;
                }
                if !here.ok() {
                    return Err(Retry);
                }
                let rem = if bd == d {
                    key_low(suffix, level - 1)
                } else {
                    pow256(level - 1) - 1
                };
                let mark = rs.len;
                // SAFETY: child copy validated against this branch.
                if let Some((r, v)) = unsafe { prev_in::<MAP>(&child, rem, level - 1, here, rs)? } {
                    return Ok(Some((compose(bd, r, level), v)));
                }
                backtrack(rs, mark);
            }
            Ok(None)
        }
    }
}

/// A child subtree answered nothing; the search moves to its sibling.
///
/// Test builds park here (G12.1), and the negative control forgets the
/// child's snapshots, as a single moving cover would.
#[inline(always)]
fn backtrack(rs: &mut ReadSet, mark: usize) {
    #[cfg(test)]
    {
        crate::sync::test_hooks::before_ordered_backtrack();
        if crate::sync::test_hooks::drops_child_snapshots() {
            rs.len = mark;
        }
    }
    #[cfg(not(test))]
    let _ = (rs, mark);
}

/// The child edge for set digit `bd` of a `BranchB`, validated against the
/// branch before its subarray is indexed and again after the load.
///
/// # Safety
///
/// `node` is an EBR-live `BranchB` whose version `here` names.
#[inline(always)]
unsafe fn branch_b_child(node: *const BranchB, bd: u8, here: Holder<'_>) -> Result<Edge, Retry> {
    // SAFETY: live node per contract.
    let (rank, sub) = unsafe {
        (
            crate::bits::shared_bitmap::subexpanse_rank::<true>(&raw const (*node).bitmap, bd)
                as usize,
            crate::bits::shared_word::load_ptr::<true, Edge>(
                (&raw const (*node).subarrays)
                    .cast::<*mut Edge>()
                    .add((bd >> 5) as usize),
            ),
        )
    };
    if sub.is_null() {
        return Err(Retry);
    }
    // The bitmap and the subarray pointer are written separately: a rank from
    // a new bitmap against an old, shorter subarray reads out of bounds, so
    // the pair is validated before the index (as `walk_validated` does).
    if !here.ok() {
        return Err(Retry);
    }
    // SAFETY: consistent bitmap/subarray pair → at least `rank + 1` edges.
    let child = unsafe { Edge::load_at::<true>(sub.add(rank)) };
    if !here.ok() {
        return Err(Retry);
    }
    Ok(child)
}

/// The next (`forward`) or previous set digit from `from` in a bitmap leaf,
/// with its map value.
///
/// # Safety
///
/// `edge` is a validated copy naming a live bitmap leaf of the `MAP` form,
/// covered by `holder`.
#[inline(always)]
unsafe fn bitmap_leaf<const MAP: bool>(
    edge: &Edge,
    holder: Holder<'_>,
    from: u8,
    forward: bool,
) -> Result<Option<(u8, u64)>, Retry> {
    if MAP {
        let node = edge.node_ptr().cast::<LeafBitmapL>();
        // SAFETY: live leaf; loads validated before the value deref.
        let Some(d) = (unsafe {
            if forward {
                crate::bits::shared_bitmap::next_set::<true>(&raw const (*node).bitmap, from)
            } else {
                crate::bits::shared_bitmap::prev_set::<true>(&raw const (*node).bitmap, from)
            }
        }) else {
            return Ok(None);
        };
        // SAFETY: as above.
        let (rank, vals) = unsafe {
            (
                crate::bits::shared_bitmap::subexpanse_rank::<true>(&raw const (*node).bitmap, d)
                    as usize,
                crate::bits::shared_word::load_ptr::<true, u64>(
                    (&raw const (*node).values)
                        .cast::<*mut u64>()
                        .add((d >> 5) as usize),
                ),
            )
        };
        if vals.is_null() {
            return Err(Retry);
        }
        if !holder.ok() {
            return Err(Retry);
        }
        // SAFETY: consistent bitmap/value-array pair → `rank + 1` values.
        let v = unsafe { crate::bits::shared_word::load::<true>(vals.add(rank)) };
        Ok(Some((d, v)))
    } else {
        let node = edge.node_ptr().cast::<LeafBitmap1>();
        // SAFETY: live leaf; covered by the holder's final validation.
        let d = unsafe {
            if forward {
                crate::bits::shared_bitmap::next_set::<true>(&raw const (*node).bitmap, from)
            } else {
                crate::bits::shared_bitmap::prev_set::<true>(&raw const (*node).bitmap, from)
            }
        };
        Ok(d.map(|d| (d, 0)))
    }
}

/// The value of slot `slot` in a map immediate.
///
/// # Safety
///
/// `edge` is a validated map-immediate copy; a multi-key one names a live
/// value array of its key count.
#[inline(always)]
unsafe fn immed_value(edge: &Edge, im: crate::types::ImmedType, slot: usize) -> u64 {
    if im.key_count() == 1 {
        u64::from_le_bytes(edge.imm_bytes())
    } else {
        // SAFETY: live value array per contract; `slot < key_count`.
        unsafe { crate::bits::shared_word::load::<true>(edge.node_ptr().cast::<u64>().add(slot)) }
    }
}
