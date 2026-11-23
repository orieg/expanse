//! Empty-index search over a set-flavour subtree: the smallest absent key
//! `>= start` (`FWD`) or the largest absent key `<= start`, found by one
//! recursive descent.
//!
//! The search goes down to the terminal holding `start`, finds the end of
//! `start`'s run of present keys there, and returns the key after it. A
//! terminal whose run reaches the end of its expanse sends the search to the
//! next child of its parent branch, where an absent digit is the answer at
//! once; only a child that is itself present is descended into. Each
//! terminal visited is one step of the caller's budget.
//!
//! This is the `Judy1FirstEmpty` / `NextEmpty` / `LastEmpty` / `PrevEmpty`
//! search behind [`ExpanseSet::first_absent_within`]: it reads only the
//! nodes on the path and their siblings, where the ordered cursor it
//! replaces also builds and moves its descent stack.
//!
//! [`ExpanseSet::first_absent_within`]: crate::ExpanseSet::first_absent_within

use crate::mutate::read_packed;
use crate::node::{BranchB, BranchL3, BranchL7, BranchU, Edge, LeafBitmap1};
use crate::types::{EdgeType, digit};

/// Outcome of the search in one subtree, in that subtree's suffix space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Gap {
    /// The absent suffix nearest `start` in the search direction.
    At(u64),
    /// Every suffix from `start` to the end of the subtree's expanse in the
    /// search direction is present.
    Full,
    /// The budget ran out before this suffix's terminal was read; every
    /// suffix from `start` up to it (exclusive, in the search direction) is
    /// present.
    Budget(u64),
}

/// The last suffix of a `level`-byte expanse (`1 <= level <= 8`), which is
/// also the mask of a `level`-byte suffix: one shift, where
/// `mutate::key_low` branches on `level >= 8`.
#[inline(always)]
const fn top_of(level: u8) -> u64 {
    debug_assert!(level >= 1 && level <= 8);
    u64::MAX >> (64 - 8 * level as u32)
}

/// The low `level` bytes of `key` (`1 <= level <= 8`).
#[inline(always)]
const fn key_low(key: u64, level: u8) -> u64 {
    key & top_of(level)
}

/// The first suffix of child `d` at branch level `bl`, offset by `rem`.
#[inline(always)]
const fn compose(d: u8, rem: u64, bl: u8) -> u64 {
    ((d as u64) << ((bl - 1) * 8)) | rem
}

/// Maps a gap found inside a narrow node — whose keys all share the decode
/// digits `dv` above its own `kb` bytes — back to the slot's `level`-byte
/// suffix space. A narrow node that is full through its end leaves the
/// adjacent decode value, which holds no key, as the answer.
#[inline(always)]
fn widen<const FWD: bool>(inner: Gap, dv: u64, kb: u8, level: u8) -> Gap {
    let shift = 8 * u32::from(kb);
    match inner {
        Gap::At(r) => Gap::At((dv << shift) | r),
        Gap::Budget(r) => Gap::Budget((dv << shift) | r),
        Gap::Full => {
            if FWD {
                if dv < top_of(level - kb) {
                    Gap::At((dv + 1) << shift)
                } else {
                    Gap::Full
                }
            } else if dv > 0 {
                Gap::At((dv << shift) - 1)
            } else {
                Gap::Full
            }
        }
    }
}

/// A narrow node's decode digits (its slot's levels `kb + 1 ..= level`) as
/// one integer, as `mutate::decode_value` computes them: the aux bytes hold
/// the digit for level `kb + 1 + i` at byte `kb + i`, so they are the key's
/// bit positions already.
#[inline(always)]
fn decode(edge: &Edge, kb: u8, level: u8) -> u64 {
    debug_assert!(kb < level && level <= 7);
    let dv = key_low(edge.aux_word() >> (8 * u32::from(kb)), level - kb);
    debug_assert_eq!(dv, crate::mutate::decode_value(edge, kb, level));
    dv
}

/// The run of present keys at `a` in a strictly ascending `read`, followed
/// in the search direction: the index of its last key (`FWD`) or its first.
#[inline(always)]
fn run_edge<const FWD: bool>(read: impl Fn(usize) -> u64, a: usize, n: usize) -> usize {
    let base = read(a);
    if FWD {
        // Consecutiveness is a prefix property of a strictly ascending
        // sequence, so it bisects.
        let (mut lo, mut hi) = (a, n);
        while lo + 1 < hi {
            let mid = lo + (hi - lo) / 2;
            if read(mid) - base == (mid - a) as u64 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    } else {
        let (mut lo, mut hi) = (0usize, a);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if base - read(mid) == (a - mid) as u64 {
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }
        hi
    }
}

/// The key after the run ending at `k` in the search direction, inside a
/// `kb`-byte expanse that holds no key past the run.
#[inline(always)]
fn past<const FWD: bool>(k: u64, kb: u8) -> Gap {
    if FWD {
        if k < top_of(kb) {
            Gap::At(k + 1)
        } else {
            Gap::Full
        }
    } else if k > 0 {
        Gap::At(k - 1)
    } else {
        Gap::Full
    }
}

/// The gap search over sorted `n` keys `read(0..n)` of a `kb`-byte expanse,
/// from `low`.
#[inline(always)]
fn sorted_gap<const FWD: bool>(read: impl Fn(usize) -> u64, n: usize, low: u64, kb: u8) -> Gap {
    // The first key >= low: `low` itself, or `low` is absent.
    let (mut at, mut hi) = (0usize, n);
    while at < hi {
        let mid = at + (hi - at) / 2;
        if read(mid) < low {
            at = mid + 1;
        } else {
            hi = mid;
        }
    }
    if at == n || read(at) != low {
        return Gap::At(low);
    }
    let j = run_edge::<FWD>(&read, at, n);
    past::<FWD>(read(j), kb)
}

/// How a branch finds the child at a digit.
#[derive(Clone, Copy)]
enum Children {
    /// A linear branch: `num` sorted digits and their edges.
    Linear {
        num: usize,
        digits: [u8; 8],
        edges: *const Edge,
    },
    /// A bitmap branch.
    Bitmap(*const BranchB),
    /// An uncompressed branch's 256 edges.
    Uncompressed(*const Edge),
}

impl Children {
    /// The child at digit `d`, if present.
    ///
    /// # Safety
    ///
    /// `self` describes a live branch.
    #[inline(always)]
    unsafe fn at(self, d: u8) -> Option<*const Edge> {
        // SAFETY: a live branch per this function's contract: a digit found
        // in the branch's own digit list or bitmap is at its slot or
        // subexpanse rank, and a non-null uncompressed slot is a child.
        unsafe {
            match self {
                Children::Linear { num, digits, edges } => digits[..num]
                    .iter()
                    .position(|&bd| bd == d)
                    .map(|slot| edges.add(slot)),
                Children::Bitmap(b) => {
                    let bm = &(*b).bitmap;
                    bm.test(d).then(|| {
                        (*b).subarrays[(d >> 5) as usize]
                            .add(bm.subexpanse_rank(d) as usize)
                            .cast_const()
                    })
                }
                Children::Uncompressed(edges) => {
                    let c = edges.add(d as usize);
                    (!(*c).is_null()).then_some(c)
                }
            }
        }
    }
}

/// Raw tag bytes of the forms the search dispatches on (`EdgeType`'s
/// discriminants; an immediate's tag packs its key width and count,
/// `ImmedType::as_u8`). Dispatching on the byte skips `Edge::tag`'s
/// validating decode, which a live tree never needs.
const TAG_NULL: u8 = EdgeType::Null as u8;
const TAG_BRANCH_L3: u8 = EdgeType::BranchL3 as u8;
const TAG_BRANCH_L7: u8 = EdgeType::BranchL7 as u8;
const TAG_BRANCH_B: u8 = EdgeType::BranchB as u8;
const TAG_BRANCH_U: u8 = EdgeType::BranchU as u8;
const TAG_LEAF1: u8 = EdgeType::Leaf1 as u8;
const TAG_LEAF7: u8 = EdgeType::Leaf7 as u8;
const TAG_LEAF_B1: u8 = EdgeType::LeafB1 as u8;
const TAG_FULL: u8 = EdgeType::FullExpanse as u8;

/// True for the four branch forms; every other tag is a terminal.
#[inline(always)]
const fn is_branch(raw: u8) -> bool {
    raw >= TAG_BRANCH_L3 && raw <= TAG_BRANCH_U
}

/// The search below a branch at level `bl` from the `bl`-byte `suffix`.
/// A terminal child is read here, without a call; a branch child recurses.
///
/// # Safety
///
/// `children` describes a live set-flavour branch at `bl`.
#[inline(always)]
unsafe fn branch_gap<const FWD: bool>(
    children: Children,
    suffix: u64,
    bl: u8,
    budget: &mut usize,
) -> Gap {
    let mut d = digit(suffix, bl);
    let mut rem = key_low(suffix, bl - 1);
    loop {
        // SAFETY: a live branch per this function's contract.
        let Some(c) = (unsafe { children.at(d) }) else {
            return Gap::At(compose(d, rem, bl));
        };
        // SAFETY: a live child per this function's contract.
        let child = unsafe { &*c };
        let raw = child.tag_byte();
        // SAFETY: a live child edge at `bl - 1`.
        let r = unsafe {
            if is_branch(raw) {
                gap::<FWD>(child, rem, bl - 1, budget)
            } else {
                terminal::<FWD>(child, raw, rem, bl - 1, budget)
            }
        };
        match r {
            Gap::At(r) => return Gap::At(compose(d, r, bl)),
            Gap::Budget(r) => return Gap::Budget(compose(d, r, bl)),
            Gap::Full => {}
        }
        if FWD {
            if d == 255 {
                return Gap::Full;
            }
            d += 1;
            rem = 0;
        } else {
            if d == 0 {
                return Gap::Full;
            }
            d -= 1;
            rem = top_of(bl - 1);
        }
    }
}

/// The gap search in a terminal (an immediate, a leaf, a full expanse or a
/// null edge) `edge` with tag byte `raw`, a slot at `level`, from the
/// `level`-byte `suffix`. The read takes one step of `budget`.
///
/// # Safety
///
/// `edge` is a live set-flavour terminal edge at `level`.
#[inline(always)]
unsafe fn terminal<const FWD: bool>(
    edge: &Edge,
    raw: u8,
    suffix: u64,
    level: u8,
    budget: &mut usize,
) -> Gap {
    debug_assert!(edge.tag().is_some() && !is_branch(raw));
    if *budget == 0 {
        return Gap::Budget(suffix);
    }
    *budget -= 1;
    match raw {
        TAG_NULL => Gap::At(suffix),

        TAG_LEAF1..=TAG_LEAF7 => {
            let kb = raw - TAG_LEAF1 + 1;
            let dv = if kb < level {
                decode(edge, kb, level)
            } else {
                0
            };
            if kb < level && suffix >> (8 * u32::from(kb)) != dv {
                // Every key of this slot shares the decode digits.
                return Gap::At(suffix);
            }
            let pop = edge.pop0(kb) as usize + 1;
            let keys = edge.node_ptr().cast_const();
            // SAFETY: a live linear leaf of `pop` packed `kb`-byte keys, and
            // every index read is below `pop`.
            let read = |i: usize| unsafe { read_packed(keys, i, kb as usize) };
            let inner = sorted_gap::<FWD>(read, pop, key_low(suffix, kb), kb);
            if kb < level {
                widen::<FWD>(inner, dv, kb, level)
            } else {
                inner
            }
        }

        TAG_LEAF_B1 => {
            let dv = if level > 1 { decode(edge, 1, level) } else { 0 };
            if level > 1 && suffix >> 8 != dv {
                return Gap::At(suffix);
            }
            // SAFETY: a live LeafBitmap1 per this function's contract.
            let words = unsafe { &(*edge.node_ptr().cast::<LeafBitmap1>()).bitmap.words };
            let low = (suffix & 0xFF) as u32;
            let inner = if FWD {
                let z = first_clear_from(words, low);
                if z < 256 {
                    Gap::At(u64::from(z))
                } else {
                    Gap::Full
                }
            } else {
                match last_clear_upto(words, low) {
                    Some(z) => Gap::At(u64::from(z)),
                    None => Gap::Full,
                }
            };
            if level > 1 {
                widen::<FWD>(inner, dv, 1, level)
            } else {
                inner
            }
        }

        // Every key of the slot's expanse is present.
        TAG_FULL => Gap::Full,

        // An immediate: key width in the high nibble, count - 1 in the low.
        _ => {
            let kb = raw >> 4;
            let n = (raw & 0x0F) as usize + 1;
            if n == 1 {
                let k = key_low(edge.word0(), kb);
                if k != suffix {
                    return Gap::At(suffix);
                }
                return past::<FWD>(k, level);
            }
            let payload = edge.imm_payload();
            let read = |i: usize| {
                let mut b = [0u8; 8];
                b[..kb as usize].copy_from_slice(&payload[i * kb as usize..(i + 1) * kb as usize]);
                u64::from_le_bytes(b)
            };
            sorted_gap::<FWD>(read, n, suffix, level)
        }
    }
}

/// [`terminal`] as a call, for a root edge that is itself a terminal.
///
/// # Safety
///
/// As [`terminal`].
#[inline(never)]
unsafe fn terminal_root<const FWD: bool>(
    edge: &Edge,
    raw: u8,
    suffix: u64,
    level: u8,
    budget: &mut usize,
) -> Gap {
    // SAFETY: forwarded contract.
    unsafe { terminal::<FWD>(edge, raw, suffix, level, budget) }
}

/// The gap search in the subtree under `edge`, a slot at `level`, from the
/// `level`-byte `suffix`. Each terminal read takes one step of `budget`.
///
/// # Safety
///
/// `edge` is a live set-flavour edge at `level` (the tree contract of
/// `get::test_set`).
pub(crate) unsafe fn gap<const FWD: bool>(
    edge: &Edge,
    suffix: u64,
    level: u8,
    budget: &mut usize,
) -> Gap {
    debug_assert!((1..=8).contains(&level));
    let raw = edge.tag_byte();
    if !is_branch(raw) {
        // SAFETY: a live terminal edge per this function's contract.
        return unsafe { terminal_root::<FWD>(edge, raw, suffix, level, budget) };
    }
    let base = edge.node_ptr();
    // SAFETY: a live branch of the tagged form per this function's
    // contract; headers are read by value and edge arrays addressed without
    // a reference. `BranchU` has no header and never skips; the other forms
    // store their own level, below the slot's behind a narrow pointer.
    let (children, bl) = unsafe {
        match raw {
            TAG_BRANCH_U => {
                let b = base.cast::<BranchU>().cast_const();
                let edges = (&raw const (*b).edges).cast::<Edge>();
                return branch_gap::<FWD>(Children::Uncompressed(edges), suffix, level, budget);
            }
            TAG_BRANCH_B => {
                let b = base.cast::<BranchB>().cast_const();
                (Children::Bitmap(b), (*b).level)
            }
            TAG_BRANCH_L3 => {
                let b = base.cast::<BranchL3>().cast_const();
                let children = Children::Linear {
                    num: (*b).hdr.num as usize,
                    digits: (*b).hdr.digits,
                    edges: (&raw const (*b).edges).cast::<Edge>(),
                };
                (children, (*b).hdr.level)
            }
            _ => {
                debug_assert_eq!(raw, TAG_BRANCH_L7);
                let b = base.cast::<BranchL7>().cast_const();
                let children = Children::Linear {
                    num: (*b).hdr.num as usize,
                    digits: (*b).hdr.digits,
                    edges: (&raw const (*b).edges).cast::<Edge>(),
                };
                (children, (*b).hdr.level)
            }
        }
    };
    let dv = if bl < level {
        decode(edge, bl, level)
    } else {
        0
    };
    if bl < level && suffix >> (8 * u32::from(bl)) != dv {
        return Gap::At(suffix);
    }
    // SAFETY: `children` describes this live branch.
    let inner = unsafe { branch_gap::<FWD>(children, key_low(suffix, bl), bl, budget) };
    if bl < level {
        widen::<FWD>(inner, dv, bl, level)
    } else {
        inner
    }
}

/// First clear bit at or above `start` in a 256-bit set; 256 if none.
#[inline(always)]
fn first_clear_from(words: &[u64; 4], start: u32) -> u32 {
    let mut i = (start / 64) as usize;
    let mut w = !words[i] & (u64::MAX << (start % 64));
    loop {
        if w != 0 {
            return (i as u32) * 64 + w.trailing_zeros();
        }
        i += 1;
        if i == 4 {
            return 256;
        }
        w = !words[i];
    }
}

/// Last clear bit at or below `end` (`end < 256`) in a 256-bit set.
#[inline(always)]
fn last_clear_upto(words: &[u64; 4], end: u32) -> Option<u32> {
    let mut i = (end / 64) as usize;
    let mut w = !words[i] & (u64::MAX >> (63 - end % 64));
    loop {
        if w != 0 {
            return Some((i as u32) * 64 + 63 - w.leading_zeros());
        }
        if i == 0 {
            return None;
        }
        i -= 1;
        w = !words[i];
    }
}

#[cfg(test)]
mod tests {
    use crate::ExpanseSet;
    use std::collections::BTreeSet;
    use std::vec::Vec;

    /// Every budget from 0 up: a result is either the model's answer or a
    /// resume point with every key between the start and it present.
    #[test]
    fn budget_results_are_exact_or_resumable() {
        let mut keys = BTreeSet::new();
        // Adjacent full leaves, a bitmap leaf with one hole, single keys.
        keys.extend(0x10_0000u64..0x10_0000 + 5 * 256);
        keys.extend((0x20_0000u64..0x20_0100).filter(|&k| k != 0x20_00F0));
        keys.extend([7u64, 9, 1 << 40, u64::MAX]);
        keys.extend(u64::MAX - 600..u64::MAX);
        let mut set = ExpanseSet::new();
        for &k in &keys {
            set.insert(k);
        }
        let probes: Vec<u64> = [
            0x10_0000,
            0x10_0080,
            0x10_0000 + 5 * 256 - 1,
            0x20_0000,
            0x20_00F1,
            u64::MAX - 600,
            u64::MAX,
            7,
            8,
        ]
        .into();
        for p in probes {
            let want_up = (p..=u64::MAX).find(|k| !keys.contains(k));
            let want_down = (0..=p).rev().find(|k| !keys.contains(k));
            for steps in 0..12 {
                match set.first_absent_within(p, steps) {
                    Ok(r) => assert_eq!(r, want_up, "up {p:#x} steps {steps}"),
                    Err(r) => {
                        assert!(r >= p, "up {p:#x} steps {steps}: resume {r:#x}");
                        assert!((p..r).all(|k| keys.contains(&k)), "up {p:#x} steps {steps}");
                        assert!(want_up.is_none_or(|w| w >= r), "up {p:#x} steps {steps}");
                    }
                }
                match set.last_absent_within(p, steps) {
                    Ok(r) => assert_eq!(r, want_down, "down {p:#x} steps {steps}"),
                    Err(r) => {
                        assert!(r <= p, "down {p:#x} steps {steps}: resume {r:#x}");
                        assert!(
                            (r..=p).skip(1).all(|k| keys.contains(&k)),
                            "down {p:#x} steps {steps}"
                        );
                        assert!(
                            want_down.is_none_or(|w| w <= r),
                            "down {p:#x} steps {steps}"
                        );
                    }
                }
            }
            assert_eq!(set.first_absent_within(p, 1024), Ok(want_up));
            assert_eq!(set.last_absent_within(p, 1024), Ok(want_down));
        }
    }
}
