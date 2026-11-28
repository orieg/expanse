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
    // SAFETY: this function's contract.
    unsafe { edge_pop_inline(edge, level) }
}

/// [`edge_pop`] with the two terminal forms a select passes most often
/// decoded from the raw tag byte, inline: a linear leaf (`0x05..=0x0B`, its
/// `pop0` at its own key width) and an immediate (every tag from `0x10` but
/// the set-only `0x7F`, its count in the low nibble). Every other form goes
/// to the out-of-line [`edge_pop`].
///
/// # Safety
///
/// As [`edge_pop`].
#[inline(always)]
unsafe fn edge_pop_fast(edge: &Edge, level: u8) -> u64 {
    let t = edge.tag_byte();
    if (0x05..=0x0B).contains(&t) {
        edge.pop0(t - 0x04) + 1
    } else if t >= 0x10 && t != 0x7F {
        debug_assert!(matches!(edge.tag(), Some(EdgeTag::Immed(_))));
        u64::from(t & 0x0F) + 1
    } else {
        // SAFETY: this function's contract.
        unsafe { edge_pop(edge, level) }
    }
}

/// [`edge_pop`] for the loops that sum a branch's children, inlined into
/// them.
///
/// # Safety
///
/// As [`edge_pop`].
#[inline(always)]
unsafe fn edge_pop_inline(edge: &Edge, level: u8) -> u64 {
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
        crate::mutate::decode_value_word(edge, kb, level)
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

/// The sorted keys of an immediate edge, read in place: a set immediate
/// packs them in its 15 payload bytes (word 0, then the aux bytes), a map
/// immediate in its 7 aux bytes, `key_bytes` each, little-endian.
/// [`ImmedKeys::get`] extracts one with a shift and a mask, so a search
/// reads the edge's two words instead of copying an
/// [`ImmedBuf`](crate::mutate::ImmedBuf) out.
///
/// [`next`] and [`prev`] use it for both flavours. In the set walks,
/// reading in place changes the code their leaf and bitmap arms compile
/// to on x86-64 and costs the `clustered` and `sequential` set arms
/// instructions; the set walks took it only together with the register
/// return ([`Key`]), which saves those arms more. The commit that made
/// the change carries the counts.
struct ImmedKeys {
    payload: u128,
    kb: u32,
    count: usize,
}

impl ImmedKeys {
    #[inline(always)]
    fn of<const MAP: bool>(edge: &Edge, im: crate::types::ImmedType) -> Self {
        let aux = u128::from(u64::from_le(edge.aux_word()));
        let payload = if MAP {
            aux
        } else {
            u128::from(u64::from_le_bytes(edge.imm_bytes())) | (aux << 64)
        };
        debug_assert!(im.key_bytes() < 8);
        Self {
            payload,
            kb: u32::from(im.key_bytes()),
            count: im.key_count() as usize,
        }
    }

    /// Key `slot` (`slot < count`).
    #[inline(always)]
    fn get(&self, slot: usize) -> u64 {
        debug_assert!(slot < self.count);
        let bits = 8 * self.kb;
        ((self.payload >> (bits * slot as u32)) as u64) & ((1u64 << bits) - 1)
    }

    /// The first slot whose key fails `pred` (keys ascending), as
    /// `partition_point`.
    #[inline(always)]
    fn partition_point(&self, pred: impl Fn(u64) -> bool) -> usize {
        let (mut lo, mut hi) = (0, self.count);
        while lo < hi {
            let mid = (lo + hi) / 2;
            if pred(self.get(mid)) {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo
    }

    /// The last slot whose key satisfies `pred` (keys ascending, `pred`
    /// true on a prefix), with its key: the search keeps the key of the
    /// last probe that satisfied `pred`, which is that slot's, so the key
    /// is not extracted a second time.
    #[inline(always)]
    fn last_where(&self, pred: impl Fn(u64) -> bool) -> Option<(usize, u64)> {
        let (mut lo, mut hi, mut key) = (0, self.count, 0);
        while lo < hi {
            let mid = (lo + hi) / 2;
            let k = self.get(mid);
            if pred(k) {
                lo = mid + 1;
                key = k;
            } else {
                hi = mid;
            }
        }
        if lo == 0 { None } else { Some((lo - 1, key)) }
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

/// What a navigation walk hands back for a found map entry: its value
/// ([`Val`], the engine's ordered reads) or a writable pointer to its value
/// slot ([`Slot`], the compat layer's `JudyLFirst`/`Next`/`Last`/`Prev`/
/// `ByCount`, which return the slot of the key they find). One walk serves
/// both; each flavour is its own instantiation, so the value walk reads the
/// value where it always did.
pub(crate) trait Hit {
    /// The per-entry result.
    type Out: Copy;
    /// The result a set-flavour walk returns (sets carry no values).
    const NONE: Self::Out;
    /// The entry at `slot` of a map immediate.
    ///
    /// # Safety
    ///
    /// `ep` points at a live map immediate edge reached through raw
    /// pointers from the tree's node allocations; multi-key immediates
    /// hold a live value array in word 0.
    unsafe fn immed(ep: *const Edge, im: crate::types::ImmedType, slot: usize) -> Self::Out;
    /// The entry whose value word is at `p`.
    ///
    /// # Safety
    ///
    /// `p` points at a live value word, derived from a stored node or
    /// value-array pointer.
    unsafe fn at(p: *mut u64) -> Self::Out;
    /// Whether [`prev_hit`] reads immediates in place and takes a
    /// `BranchB`'s later children one slot lower ([`prev_child`]). The
    /// value walk ([`Val`]) keeps the original immediate arm and `BranchB`
    /// loop: the in-place form, applied to it, measured +0.65% on
    /// `map_prev sequential` and +0.30% on `map_prev clustered`
    /// (Callgrind, x86), whose paths do not reach the changed arms,
    /// through code generation of the shared walk. The set walk ([`Key`])
    /// and the slot walk ([`Slot`], `JudyLPrev` / `JudyLLast`) take it; no
    /// Callgrind arm walks a sequential or clustered population through
    /// the slot walk.
    const PREV_IN_PLACE: bool;
}

/// [`Hit`] returning the value.
pub(crate) enum Val {}

impl Hit for Val {
    type Out = u64;
    const NONE: u64 = 0;
    const PREV_IN_PLACE: bool = false;
    #[inline(always)]
    unsafe fn immed(ep: *const Edge, im: crate::types::ImmedType, slot: usize) -> u64 {
        // SAFETY: this trait method's contract.
        unsafe { immed_value(&*ep, im, slot) }
    }
    #[inline(always)]
    unsafe fn at(p: *mut u64) -> u64 {
        // SAFETY: this trait method's contract.
        unsafe { *p }
    }
}

/// [`Hit`] for the set flavour: a found key carries nothing, so a set
/// walk returns `Option<(u64, ())>`, which fits in two registers, where
/// `Option<(u64, u64)>` is returned through memory at every level of the
/// recursion.
pub(crate) enum Key {}

impl Hit for Key {
    type Out = ();
    const NONE: () = ();
    const PREV_IN_PLACE: bool = true;
    #[inline(always)]
    unsafe fn immed(_ep: *const Edge, _im: crate::types::ImmedType, _slot: usize) {}
    #[inline(always)]
    unsafe fn at(_p: *mut u64) {}
}

/// [`Hit`] returning a writable pointer to the value slot — the pointer
/// `get::locate_slot` would return for the same key, valid until the next
/// structural mutation (the classic JudyL slot contract). Every pointer it
/// returns is derived from raw node pointers, never from a shared
/// reference, so writes through it are sound.
pub(crate) enum Slot {}

impl Hit for Slot {
    type Out = *mut u64;
    const NONE: *mut u64 = core::ptr::null_mut();
    const PREV_IN_PLACE: bool = true;
    #[inline(always)]
    unsafe fn immed(ep: *const Edge, im: crate::types::ImmedType, slot: usize) -> *mut u64 {
        if im.key_count() == 1 {
            // Word 0 (offset 0 of the edge) is the value itself, as in
            // `get::locate_slot`.
            ep.cast_mut().cast::<u64>()
        } else {
            // SAFETY: live value array of key_count values per contract.
            unsafe { (*ep).node_ptr().cast::<u64>().add(slot) }
        }
    }
    #[inline(always)]
    unsafe fn at(p: *mut u64) -> *mut u64 {
        p
    }
}

/// The child-edge array of a linear branch, derived from the stored node
/// pointer so slots below it keep the node allocation's provenance.
///
/// # Safety
///
/// `edge` references a live `BranchL3` (`l3`) or `BranchL7`.
#[inline(always)]
unsafe fn linear_branch_parts(edge: &Edge, l3: bool) -> (usize, [u8; 8], *const Edge) {
    // SAFETY: live branch per contract; the header is read by value and
    // the edge array's address is taken without a reference.
    unsafe {
        if l3 {
            let b = edge.node_ptr().cast::<BranchL3>();
            (
                (*b).hdr.num as usize,
                (*b).hdr.digits,
                (&raw const (*b).edges).cast::<Edge>(),
            )
        } else {
            let b = edge.node_ptr().cast::<BranchL7>();
            (
                (*b).hdr.num as usize,
                (*b).hdr.digits,
                (&raw const (*b).edges).cast::<Edge>(),
            )
        }
    }
}

/// Smallest key `>= suffix` in the subtree, with its value (0 for sets).
///
/// # Safety
///
/// Same tree contract as `get::test_set`, for the matching flavor.
#[inline(always)]
pub(crate) unsafe fn next<const MAP: bool>(
    edge: &Edge,
    suffix: u64,
    level: u8,
) -> Option<(u64, u64)> {
    if MAP {
        // SAFETY: forwarded contract.
        unsafe { next_hit::<true, Val>(edge, suffix, level) }
    } else {
        // SAFETY: forwarded contract.
        unsafe { next_hit::<false, Key>(edge, suffix, level) }.map(|(k, ())| (k, 0))
    }
}

/// [`next`] returning the found entry's value slot (map flavor).
///
/// # Safety
///
/// As [`next`]; `edge` must carry write provenance for the slot to be
/// written through (see [`Slot`]).
#[inline(always)]
pub(crate) unsafe fn next_slot(
    edge: *const Edge,
    suffix: u64,
    level: u8,
) -> Option<(u64, *mut u64)> {
    // SAFETY: forwarded contract.
    unsafe { next_hit::<true, Slot>(edge, suffix, level) }
}

/// The walk behind [`next`] and [`next_slot`].
///
/// # Safety
///
/// As [`next`].
unsafe fn next_hit<const MAP: bool, H: Hit>(
    ep: *const Edge,
    suffix: u64,
    level: u8,
) -> Option<(u64, H::Out)> {
    // SAFETY: live edge per contract.
    let edge = unsafe { &*ep };
    debug_assert!((1..=8).contains(&level));
    debug_assert!(level == 8 || suffix < pow256(level));
    match edge.tag().expect("valid edge tag") {
        EdgeTag::Structural(EdgeType::Null) => None,

        EdgeTag::Immed(im) => {
            let (slot, k) = {
                let keys = ImmedKeys::of::<MAP>(edge, im);
                let slot = keys.partition_point(|k| k < suffix);
                if slot == keys.count {
                    return None;
                }
                (slot, keys.get(slot))
            };
            let v = if MAP {
                // SAFETY: live map immediate per contract.
                unsafe { H::immed(ep, im, slot) }
            } else {
                H::NONE
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
            let base = edge.node_ptr();
            let keys = if MAP {
                // SAFETY: map leaf = values then packed keys.
                unsafe { base.add(leaf::map_keys_offset(pop)) }
            } else {
                base
            };
            // SAFETY: `keys` points to a live leaf of `pop` keys of `kb` bytes.
            let slot = unsafe {
                match kb {
                    1 => leaf::lower_bound_fixed::<1>(keys, pop, low),
                    2 => leaf::lower_bound_fixed::<2>(keys, pop, low),
                    3 => leaf::lower_bound_fixed::<3>(keys, pop, low),
                    4 => leaf::lower_bound_fixed::<4>(keys, pop, low),
                    5 => leaf::lower_bound_fixed::<5>(keys, pop, low),
                    6 => leaf::lower_bound_fixed::<6>(keys, pop, low),
                    7 => leaf::lower_bound_fixed::<7>(keys, pop, low),
                    _ => unreachable!(),
                }
            };
            if slot == pop {
                return None;
            }
            // SAFETY: slot < pop.
            let k = unsafe { crate::mutate::read_packed(keys, slot, kb as usize) };
            let v = if MAP {
                // SAFETY: map leaves hold pop values at the base.
                unsafe { H::at(base.cast::<u64>().add(slot)) }
            } else {
                H::NONE
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
                let v = unsafe { H::at(node.values[sub].add(rank)) };
                Some(((dv << shift) | u64::from(d), v))
            } else {
                // SAFETY: live LeafBitmap1 per contract.
                let node = unsafe { &*edge.node_ptr().cast::<LeafBitmap1>() };
                Some((
                    (dv << shift) | u64::from(node.bitmap.next_set(suffix as u8)?),
                    H::NONE,
                ))
            }
        }

        EdgeTag::Structural(EdgeType::FullExpanse) => {
            debug_assert!(!MAP);
            // Every key in the expanse is present.
            Some((suffix, H::NONE))
        }

        EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
            // SAFETY: live branch per contract.
            let bl = unsafe { crate::mutate::branch_form_level(edge, t, level) };
            let (dv, shift): (u64, u32) = if bl < level {
                (
                    crate::mutate::decode_value_word(edge, bl, level),
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
            let (num, digits, edges_ptr) =
                unsafe { linear_branch_parts(edge, matches!(t, EdgeType::BranchL3)) };
            let start = digits[..num].partition_point(|&bd| bd < d);
            for (slot, &bd) in digits.iter().enumerate().take(num).skip(start) {
                let rem = if bd == d { key_low(suffix, bl - 1) } else { 0 };
                // SAFETY: slot < num live child edges.
                let child = unsafe { edges_ptr.add(slot) };
                // SAFETY: child subtree per contract.
                if let Some((r, v)) = unsafe { next_hit::<MAP, H>(child, rem, bl - 1) } {
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
                    crate::mutate::decode_value_word(edge, bl, level),
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
                let child = unsafe { b.subarrays[sub].add(rank).cast_const() };
                // SAFETY: child subtree per contract.
                if let Some((r, v)) = unsafe { next_hit::<MAP, H>(child, rem, bl - 1) } {
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
            // SAFETY: live BranchU per contract; the edge array's address
            // is taken from the stored node pointer, without a reference.
            let edges =
                unsafe { (&raw const (*edge.node_ptr().cast::<BranchU>()).edges).cast::<Edge>() };
            for bd in d..=255u8 {
                // SAFETY: a BranchU holds 256 live child edges.
                let child = unsafe { edges.add(bd as usize) };
                // SAFETY: as above.
                if unsafe { (*child).is_null() } {
                    continue;
                }
                let rem = if bd == d {
                    key_low(suffix, level - 1)
                } else {
                    0
                };
                // SAFETY: child subtree per contract.
                if let Some((r, v)) = unsafe { next_hit::<MAP, H>(child, rem, level - 1) } {
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
#[inline(always)]
pub(crate) unsafe fn prev<const MAP: bool>(
    edge: &Edge,
    suffix: u64,
    level: u8,
) -> Option<(u64, u64)> {
    if MAP {
        // SAFETY: forwarded contract.
        unsafe { prev_hit::<true, Val>(edge, suffix, level) }
    } else {
        // SAFETY: forwarded contract.
        unsafe { prev_hit::<false, Key>(edge, suffix, level) }.map(|(k, ())| (k, 0))
    }
}

/// [`prev`] returning the found entry's value slot (map flavor).
///
/// # Safety
///
/// As [`next_slot`].
#[inline(always)]
pub(crate) unsafe fn prev_slot(
    edge: *const Edge,
    suffix: u64,
    level: u8,
) -> Option<(u64, *mut u64)> {
    // SAFETY: forwarded contract.
    unsafe { prev_hit::<true, Slot>(edge, suffix, level) }
}

/// Largest key `<= suffix` of an immediate edge, with its entry.
///
/// # Safety
///
/// `ep` points at a live immediate edge of tag `im`, as [`prev_hit`].
#[inline(always)]
unsafe fn immed_prev<const MAP: bool, H: Hit>(
    ep: *const Edge,
    im: crate::types::ImmedType,
    suffix: u64,
) -> Option<(u64, H::Out)> {
    // SAFETY: live edge per contract.
    let edge = unsafe { &*ep };
    let (slot, k) = ImmedKeys::of::<MAP>(edge, im).last_where(|k| k <= suffix)?;
    let v = if MAP {
        // SAFETY: live map immediate per contract.
        unsafe { H::immed(ep, im, slot) }
    } else {
        H::NONE
    };
    Some((k, v))
}

/// The largest key of an immediate edge, with its entry.
///
/// # Safety
///
/// As [`immed_prev`].
#[inline(always)]
unsafe fn immed_last<const MAP: bool, H: Hit>(
    ep: *const Edge,
    im: crate::types::ImmedType,
) -> (u64, H::Out) {
    // SAFETY: live edge per contract.
    let keys = ImmedKeys::of::<MAP>(unsafe { &*ep }, im);
    let slot = keys.count - 1;
    let v = if MAP {
        // SAFETY: live map immediate per contract.
        unsafe { H::immed(ep, im, slot) }
    } else {
        H::NONE
    };
    (keys.get(slot), v)
}

/// [`prev_hit`] on a branch's child at `level`, which holds the key bytes
/// below the branch digit. `bounded` says the child's digit is the
/// suffix's own, so the search is bounded by the suffix's low `level`
/// bytes; otherwise the child's digit is below it and the answer is the
/// child's largest key.
///
/// An immediate child is read in place rather than through another call:
/// below a sparse branch most children are immediates, and a backward step
/// from a present key reads two of them (its own, which holds nothing
/// smaller, then its neighbour's largest key, which needs no search).
///
/// # Safety
///
/// As [`prev_hit`].
#[inline(always)]
unsafe fn prev_child<const MAP: bool, H: Hit>(
    child: *const Edge,
    bounded: bool,
    suffix: u64,
    level: u8,
) -> Option<(u64, H::Out)> {
    let rem = if bounded {
        key_low(suffix, level)
    } else {
        pow256(level) - 1
    };
    // SAFETY: live child edge per contract.
    if let Some(EdgeTag::Immed(im)) = unsafe { (*child).tag() } {
        if bounded {
            // SAFETY: as above.
            unsafe { immed_prev::<MAP, H>(child, im, rem) }
        } else {
            // SAFETY: as above.
            Some(unsafe { immed_last::<MAP, H>(child, im) })
        }
    } else {
        // SAFETY: forwarded contract.
        unsafe { prev_hit::<MAP, H>(child, rem, level) }
    }
}

/// The walk behind [`prev`] and [`prev_slot`].
///
/// # Safety
///
/// As [`next`].
unsafe fn prev_hit<const MAP: bool, H: Hit>(
    ep: *const Edge,
    suffix: u64,
    level: u8,
) -> Option<(u64, H::Out)> {
    // SAFETY: live edge per contract.
    let edge = unsafe { &*ep };
    debug_assert!((1..=8).contains(&level));
    debug_assert!(level == 8 || suffix < pow256(level));
    match edge.tag().expect("valid edge tag") {
        EdgeTag::Structural(EdgeType::Null) => None,

        EdgeTag::Immed(im) => {
            if H::PREV_IN_PLACE {
                // SAFETY: live immediate per contract.
                return unsafe { immed_prev::<MAP, H>(ep, im, suffix) };
            }
            // The value walk keeps this arm, and the `BranchB` loop below,
            // as they were ([`Hit::PREV_IN_PLACE`]).
            let (slot, k) = {
                let keys = ImmedKeys::of::<MAP>(edge, im);
                let slot = keys.partition_point(|k| k <= suffix).checked_sub(1)?;
                (slot, keys.get(slot))
            };
            let v = if MAP {
                // SAFETY: live map immediate per contract.
                unsafe { H::immed(ep, im, slot) }
            } else {
                H::NONE
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
                core::cmp::Ordering::Less => return None,
                core::cmp::Ordering::Equal => key_low(suffix, kb),
                core::cmp::Ordering::Greater => key_low(u64::MAX, kb),
            };
            let base = edge.node_ptr();
            let keys = if MAP {
                // SAFETY: map leaf = values then packed keys.
                unsafe { base.add(leaf::map_keys_offset(pop)) }
            } else {
                base
            };
            let bound = if low == key_low(u64::MAX, kb) {
                pop
            } else {
                // SAFETY: `keys` points to a live leaf of `pop` keys of `kb` bytes.
                unsafe {
                    match kb {
                        1 => leaf::lower_bound_fixed::<1>(keys, pop, low + 1),
                        2 => leaf::lower_bound_fixed::<2>(keys, pop, low + 1),
                        3 => leaf::lower_bound_fixed::<3>(keys, pop, low + 1),
                        4 => leaf::lower_bound_fixed::<4>(keys, pop, low + 1),
                        5 => leaf::lower_bound_fixed::<5>(keys, pop, low + 1),
                        6 => leaf::lower_bound_fixed::<6>(keys, pop, low + 1),
                        7 => leaf::lower_bound_fixed::<7>(keys, pop, low + 1),
                        _ => unreachable!(),
                    }
                }
            };
            let slot = bound.checked_sub(1)?;
            // SAFETY: slot < pop.
            let k = unsafe { crate::mutate::read_packed(keys, slot, kb as usize) };
            let v = if MAP {
                // SAFETY: map leaves hold pop values at the base.
                unsafe { H::at(base.cast::<u64>().add(slot)) }
            } else {
                H::NONE
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
                let v = unsafe { H::at(node.values[sub].add(rank)) };
                Some(((dv << shift) | u64::from(d), v))
            } else {
                // SAFETY: live LeafBitmap1 per contract.
                let node = unsafe { &*edge.node_ptr().cast::<LeafBitmap1>() };
                Some((
                    (dv << shift) | u64::from(node.bitmap.prev_set(from)?),
                    H::NONE,
                ))
            }
        }

        EdgeTag::Structural(EdgeType::FullExpanse) => {
            debug_assert!(!MAP);
            Some((suffix.min(pow256(level) - 1), H::NONE))
        }

        EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
            // SAFETY: live branch per contract.
            let bl = unsafe { crate::mutate::branch_form_level(edge, t, level) };
            let (dv, shift): (u64, u32) = if bl < level {
                (
                    crate::mutate::decode_value_word(edge, bl, level),
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
            let (num, digits, edges_ptr) =
                unsafe { linear_branch_parts(edge, matches!(t, EdgeType::BranchL3)) };
            let bound = digits[..num].partition_point(|&bd| bd <= d);
            for (slot, &bd) in digits[..bound].iter().enumerate().rev() {
                let rem = if bd == d {
                    key_low(suffix, bl - 1)
                } else {
                    pow256(bl - 1) - 1
                };
                // SAFETY: slot < num live child edges.
                let child = unsafe { edges_ptr.add(slot) };
                // SAFETY: child subtree per contract.
                if let Some((r, v)) = unsafe { prev_hit::<MAP, H>(child, rem, bl - 1) } {
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
                    crate::mutate::decode_value_word(edge, bl, level),
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
            if !H::PREV_IN_PLACE {
                // The value walk keeps the loop below unchanged
                // ([`Hit::PREV_IN_PLACE`]).
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
                    let child = unsafe { b.subarrays[sub].add(rank).cast_const() };
                    // SAFETY: child subtree per contract.
                    if let Some((r, v)) = unsafe { prev_hit::<MAP, H>(child, rem, bl - 1) } {
                        return Some(((dv << shift) | compose(bd, r, bl), v));
                    }
                    cur = if bd == 0 {
                        None
                    } else {
                        b.bitmap.prev_set(bd - 1)
                    };
                }
                None
            } else {
                // The first digit tried is the suffix's own or the next set one
                // below it; every later one is below the suffix's digit, so its
                // child's largest key is the answer. A later digit in the
                // subexpanse of the one tried before it is the next set bit
                // below it there, and its edge sits one slot lower: only the
                // first digit tried in a subexpanse needs its rank.
                let mut bd = b.bitmap.prev_set(d)?;
                let mut sub = (bd >> 5) as usize;
                let rank = b.bitmap.subexpanse_rank(bd) as usize;
                // SAFETY: bit set → subarray holds rank + 1 edges.
                let mut child = unsafe { b.subarrays[sub].add(rank).cast_const() };
                // SAFETY: child subtree per contract.
                let found = unsafe { prev_child::<MAP, H>(child, bd == d, suffix, bl - 1) };
                if let Some((r, v)) = found {
                    return Some(((dv << shift) | compose(bd, r, bl), v));
                }
                while bd > 0 {
                    bd = b.bitmap.prev_set(bd - 1)?;
                    if (bd >> 5) as usize == sub {
                        // SAFETY: the digit tried before this one is the next
                        // set bit above it in this subexpanse, at slot >= 1.
                        child = unsafe { child.sub(1) };
                    } else {
                        sub = (bd >> 5) as usize;
                        let rank = b.bitmap.subexpanse_rank(bd) as usize;
                        // SAFETY: bit set → subarray holds rank + 1 edges.
                        child = unsafe { b.subarrays[sub].add(rank).cast_const() };
                    }
                    // SAFETY: child subtree per contract.
                    let found = unsafe { prev_child::<MAP, H>(child, false, suffix, bl - 1) };
                    if let Some((r, v)) = found {
                        return Some(((dv << shift) | compose(bd, r, bl), v));
                    }
                }
                None
            }
        }

        EdgeTag::Structural(EdgeType::BranchU) => {
            let d = digit(suffix, level);
            // SAFETY: live BranchU per contract; the edge array's address
            // is taken from the stored node pointer, without a reference.
            let edges =
                unsafe { (&raw const (*edge.node_ptr().cast::<BranchU>()).edges).cast::<Edge>() };
            for bd in (0..=d).rev() {
                // SAFETY: a BranchU holds 256 live child edges.
                let child = unsafe { edges.add(bd as usize) };
                // SAFETY: as above.
                if unsafe { (*child).is_null() } {
                    continue;
                }
                let rem = if bd == d {
                    key_low(suffix, level - 1)
                } else {
                    pow256(level - 1) - 1
                };
                // SAFETY: child subtree per contract.
                if let Some((r, v)) = unsafe { prev_hit::<MAP, H>(child, rem, level - 1) } {
                    return Some((compose(bd, r, level), v));
                }
            }
            None
        }
    }
}

/// Number of keys strictly below `suffix` in the subtree (rank). At a
/// branch the children on the side of the suffix's digit spanning fewer
/// digits are summed; the other side is the branch population less them.
/// `top_pop` is the subtree's population, read only for a level-8 branch,
/// whose edge carries none.
///
/// # Safety
///
/// Same contract as [`next`], and `top_pop` is the population of a level-8
/// subtree.
pub(crate) unsafe fn count_below<const MAP: bool>(
    edge: &Edge,
    suffix: u64,
    level: u8,
    top_pop: u64,
) -> u64 {
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
                    let base = edge.node_ptr();
                    let keys = if MAP {
                        // SAFETY: map leaf = values then packed keys.
                        unsafe { base.add(leaf::map_keys_offset(pop)) }
                    } else {
                        base
                    };
                    let low = key_low(suffix, kb);
                    // SAFETY: `keys` points to a live leaf of `pop` keys of `kb` bytes.
                    let slot = unsafe {
                        match kb {
                            1 => leaf::lower_bound_fixed::<1>(keys, pop, low),
                            2 => leaf::lower_bound_fixed::<2>(keys, pop, low),
                            3 => leaf::lower_bound_fixed::<3>(keys, pop, low),
                            4 => leaf::lower_bound_fixed::<4>(keys, pop, low),
                            5 => leaf::lower_bound_fixed::<5>(keys, pop, low),
                            6 => leaf::lower_bound_fixed::<6>(keys, pop, low),
                            7 => leaf::lower_bound_fixed::<7>(keys, pop, low),
                            _ => unreachable!(),
                        }
                    };
                    slot as u64
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

        EdgeTag::Structural(
            t @ (EdgeType::BranchL3 | EdgeType::BranchL7 | EdgeType::BranchB | EdgeType::BranchU),
        ) => {
            // SAFETY: live branch per contract.
            let bl = unsafe { crate::mutate::branch_form_level(edge, t, level) };
            // A narrow branch holds one sub-expanse: a suffix outside it
            // counts none of its keys or all of them.
            let suffix = if bl < level {
                let dv = crate::mutate::decode_value_word(edge, bl, level);
                match (suffix >> (8 * u32::from(bl))).cmp(&dv) {
                    core::cmp::Ordering::Less => return 0,
                    core::cmp::Ordering::Equal => key_low(suffix, bl),
                    core::cmp::Ordering::Greater => return edge.pop0(bl) + 1,
                }
            } else {
                suffix
            };
            // SAFETY: live branch of form `t` per contract.
            let node = unsafe { BranchNode::of(edge, t) };
            let d = digit(suffix, bl);
            let cl = bl - 1;
            let child = node.child(d);
            let inner = child.map_or(0, |c| {
                // SAFETY: live child per contract.
                unsafe { count_below::<MAP>(c, key_low(suffix, cl), cl, 0) }
            });
            if d <= 255 - d {
                if d == 0 {
                    inner
                } else {
                    // SAFETY: live children per contract.
                    inner + unsafe { node.sum_pops(0, d - 1, cl) }
                }
            } else {
                let own = if bl == 8 { top_pop } else { edge.pop0(bl) + 1 };
                let above = if d == 255 {
                    0
                } else {
                    // SAFETY: live children per contract.
                    unsafe { node.sum_pops(d + 1, 255, cl) }
                };
                // SAFETY: live child per contract.
                let at_d = child.map_or(0, |c| unsafe { edge_pop(c, cl) });
                own - above - (at_d - inner)
            }
        }
    }
}

/// The largest suffix at `level`.
#[inline(always)]
const fn max_suffix(level: u8) -> u64 {
    if level >= 8 {
        u64::MAX
    } else {
        pow256(level) - 1
    }
}

/// Number of keys `<= suffix` in a child subtree at `level` (1..=7).
///
/// # Safety
///
/// Same contract as [`next`].
#[inline(always)]
unsafe fn count_at_most<const MAP: bool>(edge: &Edge, suffix: u64, level: u8) -> u64 {
    debug_assert!(level <= 7);
    if suffix == max_suffix(level) {
        // SAFETY: live child per contract.
        unsafe { edge_pop(edge, level) }
    } else {
        // SAFETY: live child per contract.
        unsafe { count_below::<MAP>(edge, suffix + 1, level, 0) }
    }
}

/// A branch node's children, by form.
enum BranchNode<'a> {
    /// `num` children, their sorted digits, and their edges.
    Linear(usize, [u8; 8], *const Edge),
    Bitmap(&'a BranchB),
    Uncompressed(&'a BranchU),
}

impl BranchNode<'_> {
    /// The node of a branch edge of form `t`.
    ///
    /// # Safety
    ///
    /// `edge` references a live branch of form `t`, which outlives the view.
    #[inline(always)]
    unsafe fn of(edge: &Edge, t: EdgeType) -> Self {
        // SAFETY: live branch of form `t` per contract.
        unsafe {
            match t {
                EdgeType::BranchL3 => {
                    let b = &*edge.node_ptr().cast::<BranchL3>();
                    Self::Linear(b.hdr.num as usize, b.hdr.digits, b.edges.as_ptr())
                }
                EdgeType::BranchL7 => {
                    let b = &*edge.node_ptr().cast::<BranchL7>();
                    Self::Linear(b.hdr.num as usize, b.hdr.digits, b.edges.as_ptr())
                }
                EdgeType::BranchB => Self::Bitmap(&*edge.node_ptr().cast::<BranchB>()),
                _ => Self::Uncompressed(&*edge.node_ptr().cast::<BranchU>()),
            }
        }
    }

    /// The child at digit `d`, if present.
    #[inline(always)]
    fn child(&self, d: u8) -> Option<&Edge> {
        match self {
            Self::Linear(num, digits, edges) => {
                let slot = digits[..*num].iter().position(|&bd| bd == d)?;
                // SAFETY: slot < num live child edges.
                Some(unsafe { &*edges.add(slot) })
            }
            Self::Bitmap(b) => {
                if !b.bitmap.test(d) {
                    return None;
                }
                let rank = b.bitmap.subexpanse_rank(d) as usize;
                // SAFETY: bit set → subarray holds rank + 1 edges.
                Some(unsafe { &*b.subarrays[(d >> 5) as usize].add(rank) })
            }
            Self::Uncompressed(b) => {
                let c = &b.edges[d as usize];
                (!c.is_null()).then_some(c)
            }
        }
    }

    /// Summed population of the children with digits in `lo..=hi`.
    ///
    /// # Safety
    ///
    /// Every child is a live subtree at `child_level`.
    #[inline(always)]
    unsafe fn sum_pops(&self, lo: u8, hi: u8, child_level: u8) -> u64 {
        let mut sum = 0;
        match self {
            Self::Linear(num, digits, edges) => {
                for (slot, &bd) in digits.iter().enumerate().take(*num) {
                    if bd > hi {
                        break;
                    }
                    if bd >= lo {
                        // SAFETY: slot < num live child edges, live per contract.
                        sum += unsafe { edge_pop_inline(&*edges.add(slot), child_level) };
                    }
                }
            }
            Self::Bitmap(b) => {
                for sub in (lo >> 5)..=(hi >> 5) {
                    // The subexpanse's 32 membership bits, as `test` reads them.
                    let word = (b.bitmap.words[(sub >> 1) as usize] >> (32 * (sub & 1))) as u32;
                    let first = if sub == lo >> 5 { lo & 31 } else { 0 };
                    let last = if sub == hi >> 5 { hi & 31 } else { 31 };
                    let below_first = (1u32 << first) - 1;
                    let through_last = u32::MAX >> (31 - last);
                    let n = (word & through_last & !below_first).count_ones() as usize;
                    if n == 0 {
                        continue;
                    }
                    let start = (word & below_first).count_ones() as usize;
                    let arr = b.subarrays[sub as usize];
                    for i in start..start + n {
                        // SAFETY: the subarray holds one edge per set bit of
                        // its subexpanse, in digit order; `i` is a set bit's.
                        sum += unsafe { edge_pop_inline(&*arr.add(i), child_level) };
                    }
                }
            }
            Self::Uncompressed(b) => {
                for c in &b.edges[lo as usize..=hi as usize] {
                    if !c.is_null() {
                        // SAFETY: live child per contract.
                        sum += unsafe { edge_pop_inline(c, child_level) };
                    }
                }
            }
        }
        sum
    }
}

/// Number of keys in the inclusive suffix range `lo..=hi` (`lo <= hi`) of
/// the subtree: one descent to the branch where the two ends take different
/// digits, the children strictly between them summed from their edges, and
/// one rank descent into each end's child. `top_pop` is the subtree's
/// population, read only for a level-8 branch, whose edge carries none.
///
/// # Safety
///
/// Same contract as [`next`], and `top_pop` is the population of a level-8
/// subtree.
pub(crate) unsafe fn count_range<const MAP: bool>(
    edge: &Edge,
    lo: u64,
    hi: u64,
    level: u8,
    top_pop: u64,
) -> u64 {
    debug_assert!((1..=8).contains(&level));
    debug_assert!(lo <= hi && hi <= max_suffix(level));
    let (t, bl) = match edge.tag().expect("valid edge tag") {
        EdgeTag::Structural(
            t @ (EdgeType::BranchL3 | EdgeType::BranchL7 | EdgeType::BranchB | EdgeType::BranchU),
        ) => {
            // SAFETY: live branch per contract.
            (t, unsafe {
                crate::mutate::branch_form_level(edge, t, level)
            })
        }
        // A terminal answers both ends from its own keys.
        _ => {
            // SAFETY: live subtree per contract.
            let below_lo = unsafe { count_below::<MAP>(edge, lo, level, top_pop) };
            let through_hi = if hi == max_suffix(level) {
                // SAFETY: a terminal carries its own count (a full expanse
                // never sits at level 8, the one level without `pop0`).
                unsafe { edge_pop(edge, level) }
            } else {
                // SAFETY: live subtree per contract.
                unsafe { count_below::<MAP>(edge, hi + 1, level, top_pop) }
            };
            return through_hi - below_lo;
        }
    };
    // A narrow branch holds one sub-expanse: clip the range to it.
    let (lo, hi) = if bl < level {
        let dv = crate::mutate::decode_value_word(edge, bl, level);
        let shift = 8 * u32::from(bl);
        let (lo_top, hi_top) = (lo >> shift, hi >> shift);
        if hi_top < dv || lo_top > dv {
            return 0;
        }
        let lo = if lo_top < dv { 0 } else { key_low(lo, bl) };
        let hi = if hi_top > dv {
            max_suffix(bl)
        } else {
            key_low(hi, bl)
        };
        if lo == 0 && hi == max_suffix(bl) {
            return edge.pop0(bl) + 1;
        }
        (lo, hi)
    } else {
        (lo, hi)
    };
    // SAFETY: live branch of form `t` per contract.
    let node = unsafe { BranchNode::of(edge, t) };
    let (da, db) = (digit(lo, bl), digit(hi, bl));
    let cl = bl - 1;
    let (lo_c, hi_c) = (key_low(lo, cl), key_low(hi, cl));
    if da == db {
        return node.child(da).map_or(0, |c| {
            // SAFETY: live child per contract.
            unsafe { count_range::<MAP>(c, lo_c, hi_c, cl, 0) }
        });
    }
    let (ca, cb) = (node.child(da), node.child(db));
    // SAFETY: live child per contract.
    let pop_a = ca.map_or(0, |c| unsafe { edge_pop(c, cl) });
    let from_lo = ca.map_or(0, |c| {
        // SAFETY: live child per contract.
        pop_a - unsafe { count_below::<MAP>(c, lo_c, cl, 0) }
    });
    let to_hi = cb.map_or(0, |c| {
        // SAFETY: live child per contract.
        unsafe { count_at_most::<MAP>(c, hi_c, cl) }
    });
    // The children strictly between the two ends, read from whichever side
    // spans fewer digits: directly, or as the branch population less the
    // children outside the window and the two end children.
    let inside = db - da - 1;
    let between = if inside <= 254 - inside {
        if inside == 0 {
            0
        } else {
            // SAFETY: live children per contract.
            unsafe { node.sum_pops(da + 1, db - 1, cl) }
        }
    } else {
        let own = if bl == 8 { top_pop } else { edge.pop0(bl) + 1 };
        // SAFETY: live children per contract.
        let outside = unsafe {
            (if da > 0 {
                node.sum_pops(0, da - 1, cl)
            } else {
                0
            }) + if db < 255 {
                node.sum_pops(db + 1, 255, cl)
            } else {
                0
            }
        };
        // SAFETY: live child per contract.
        let pop_b = cb.map_or(0, |c| unsafe { edge_pop(c, cl) });
        own - outside - pop_a - pop_b
    };
    from_lo + between + to_hi
}

/// The key with `n` keys below it (0-based select), with its value.
/// `n` must be below the subtree population. At a branch the children are
/// summed from the end nearer to `n`: from the low end for a rank in the
/// lower half of the branch population, from the high end otherwise.
/// `top_pop` is the subtree's population, read only for a level-8 branch,
/// whose edge carries none.
///
/// # Safety
///
/// Same contract as [`next`], and `top_pop` is the population of a level-8
/// subtree.
#[inline(always)]
pub(crate) unsafe fn by_count<const MAP: bool>(
    edge: &Edge,
    n: u64,
    level: u8,
    top_pop: u64,
) -> (u64, u64) {
    // SAFETY: forwarded contract.
    unsafe { by_count_hit::<MAP, Val>(edge, n, level, top_pop) }
}

/// [`by_count`] returning the selected entry's value slot (map flavor).
///
/// # Safety
///
/// As [`next_slot`]; `n` is below the subtree population.
#[inline(always)]
pub(crate) unsafe fn by_count_slot(
    edge: *const Edge,
    n: u64,
    level: u8,
    top_pop: u64,
) -> (u64, *mut u64) {
    // SAFETY: forwarded contract.
    unsafe { by_count_hit::<true, Slot>(edge, n, level, top_pop) }
}

/// The walk behind [`by_count`] and [`by_count_slot`].
///
/// # Safety
///
/// As [`by_count`].
unsafe fn by_count_hit<const MAP: bool, H: Hit>(
    ep: *const Edge,
    n: u64,
    level: u8,
    top_pop: u64,
) -> (u64, H::Out) {
    // SAFETY: live edge per contract.
    let edge = unsafe { &*ep };
    debug_assert!((1..=8).contains(&level));
    match edge.tag().expect("valid edge tag") {
        EdgeTag::Structural(EdgeType::Null) => unreachable!("select into empty subtree"),

        EdgeTag::Immed(im) => {
            let keys = immed_suffixes::<MAP>(edge, im);
            let v = if MAP {
                // SAFETY: live map immediate per contract.
                unsafe { H::immed(ep, im, n as usize) }
            } else {
                H::NONE
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
                unsafe { H::at(edge.node_ptr().cast::<u64>().add(n as usize)) }
            } else {
                H::NONE
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
                unsafe { H::at(node.values[sub].add(rank)) }
            } else {
                H::NONE
            };
            ((dv << shift) | u64::from(d), v)
        }

        EdgeTag::Structural(EdgeType::FullExpanse) => {
            debug_assert!(!MAP);
            debug_assert!(n < pow256(level));
            (n, H::NONE)
        }

        EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
            // SAFETY: live branch per contract.
            let bl = unsafe { crate::mutate::branch_form_level(edge, t, level) };
            let (dv, shift): (u64, u32) = if bl < level {
                (
                    crate::mutate::decode_value_word(edge, bl, level),
                    8 * u32::from(bl),
                )
            } else {
                (0, 0)
            };
            // SAFETY: live branch per contract.
            let (num, digits, edges_ptr) =
                unsafe { linear_branch_parts(edge, matches!(t, EdgeType::BranchL3)) };
            let own = if bl == 8 { top_pop } else { edge.pop0(bl) + 1 };
            if n < own - n {
                let mut n = n;
                for (slot, &bd) in digits.iter().enumerate().take(num) {
                    // SAFETY: slot < num live child edges.
                    let child = unsafe { edges_ptr.add(slot) };
                    // SAFETY: live child per contract.
                    let pop = unsafe { edge_pop_fast(&*child, bl - 1) };
                    if n < pop {
                        // SAFETY: child subtree per contract.
                        let (r, v) = unsafe { by_count_hit::<MAP, H>(child, n, bl - 1, 0) };
                        return ((dv << shift) | compose(bd, r, bl), v);
                    }
                    n -= pop;
                }
            } else {
                // `m` keys lie above the selected one.
                let mut m = own - 1 - n;
                for (slot, &bd) in digits.iter().enumerate().take(num).rev() {
                    // SAFETY: slot < num live child edges.
                    let child = unsafe { edges_ptr.add(slot) };
                    // SAFETY: live child per contract.
                    let pop = unsafe { edge_pop_fast(&*child, bl - 1) };
                    if m < pop {
                        // SAFETY: child subtree per contract; pop - 1 - m < pop.
                        let (r, v) =
                            unsafe { by_count_hit::<MAP, H>(child, pop - 1 - m, bl - 1, 0) };
                        return ((dv << shift) | compose(bd, r, bl), v);
                    }
                    m -= pop;
                }
            }
            unreachable!("select beyond branch population")
        }

        EdgeTag::Structural(EdgeType::BranchB) => {
            // SAFETY: live branch per contract.
            let bl = unsafe { crate::mutate::branch_form_level(edge, EdgeType::BranchB, level) };
            let (dv, shift): (u64, u32) = if bl < level {
                (
                    crate::mutate::decode_value_word(edge, bl, level),
                    8 * u32::from(bl),
                )
            } else {
                (0, 0)
            };
            // SAFETY: live BranchB per contract.
            let b = unsafe { &*edge.node_ptr().cast::<BranchB>() };
            let own = if bl == 8 { top_pop } else { edge.pop0(bl) + 1 };
            if n < own - n {
                let mut n = n;
                let mut cur = b.bitmap.next_set(0);
                while let Some(bd) = cur {
                    let sub = (bd >> 5) as usize;
                    let rank = b.bitmap.subexpanse_rank(bd) as usize;
                    // SAFETY: bit set → subarray holds rank + 1 edges.
                    let child = unsafe { b.subarrays[sub].add(rank).cast_const() };
                    // SAFETY: live child per contract.
                    let pop = unsafe { edge_pop_fast(&*child, bl - 1) };
                    if n < pop {
                        // SAFETY: child subtree per contract.
                        let (r, v) = unsafe { by_count_hit::<MAP, H>(child, n, bl - 1, 0) };
                        return ((dv << shift) | compose(bd, r, bl), v);
                    }
                    n -= pop;
                    cur = if bd == 255 {
                        None
                    } else {
                        b.bitmap.next_set(bd + 1)
                    };
                }
            } else {
                // `m` keys lie above the selected one.
                let mut m = own - 1 - n;
                let mut cur = b.bitmap.prev_set(255);
                while let Some(bd) = cur {
                    let sub = (bd >> 5) as usize;
                    let rank = b.bitmap.subexpanse_rank(bd) as usize;
                    // SAFETY: bit set → subarray holds rank + 1 edges.
                    let child = unsafe { b.subarrays[sub].add(rank).cast_const() };
                    // SAFETY: live child per contract.
                    let pop = unsafe { edge_pop_fast(&*child, bl - 1) };
                    if m < pop {
                        // SAFETY: child subtree per contract; pop - 1 - m < pop.
                        let (r, v) =
                            unsafe { by_count_hit::<MAP, H>(child, pop - 1 - m, bl - 1, 0) };
                        return ((dv << shift) | compose(bd, r, bl), v);
                    }
                    m -= pop;
                    cur = if bd == 0 {
                        None
                    } else {
                        b.bitmap.prev_set(bd - 1)
                    };
                }
            }
            unreachable!("select beyond branch population")
        }

        EdgeTag::Structural(EdgeType::BranchU) => {
            // SAFETY: live BranchU per contract; the edge array's address
            // is taken from the stored node pointer, without a reference.
            let edges =
                unsafe { (&raw const (*edge.node_ptr().cast::<BranchU>()).edges).cast::<Edge>() };
            let own = if level == 8 {
                top_pop
            } else {
                edge.pop0(level) + 1
            };
            if n < own - n {
                let mut n = n;
                for bd in 0..=255u8 {
                    // SAFETY: a BranchU holds 256 live child edges.
                    let child = unsafe { edges.add(bd as usize) };
                    // SAFETY: as above.
                    if unsafe { (*child).is_null() } {
                        continue;
                    }
                    // SAFETY: live child per contract.
                    let pop = unsafe { edge_pop_fast(&*child, level - 1) };
                    if n < pop {
                        // SAFETY: child subtree per contract.
                        let (r, v) = unsafe { by_count_hit::<MAP, H>(child, n, level - 1, 0) };
                        return (compose(bd, r, level), v);
                    }
                    n -= pop;
                }
            } else {
                // `m` keys lie above the selected one.
                let mut m = own - 1 - n;
                for bd in (0..=255u8).rev() {
                    // SAFETY: a BranchU holds 256 live child edges.
                    let child = unsafe { edges.add(bd as usize) };
                    // SAFETY: as above.
                    if unsafe { (*child).is_null() } {
                        continue;
                    }
                    // SAFETY: live child per contract.
                    let pop = unsafe { edge_pop_fast(&*child, level - 1) };
                    if m < pop {
                        // SAFETY: child subtree per contract; pop - 1 - m < pop.
                        let (r, v) =
                            unsafe { by_count_hit::<MAP, H>(child, pop - 1 - m, level - 1, 0) };
                        return (compose(bd, r, level), v);
                    }
                    m -= pop;
                }
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
