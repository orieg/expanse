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

/// A terminal form an ordered walk answered from or passed over, and the
/// number of branch versions a search retained (#1194;
/// `docs/benchmarks/concurrency/METHODOLOGY.md` §36.4, G36.3). It describes
/// the read, which a census of the tree cannot. Exists in test builds, where
/// the walk logs it per thread, and with `occ-stats`, where it is counted;
/// the default build has neither.
#[cfg(any(test, feature = "occ-stats"))]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Visit {
    /// The root leaf.
    RootLeaf,
    /// An immediate of `count` keys of `key_bytes` bytes, of the map form
    /// when `map` is set.
    Immed {
        /// Bytes per key: the level the immediate is read at.
        key_bytes: u8,
        /// Keys held.
        count: u8,
        /// The map form, whose capacity is `7 / key_bytes`.
        map: bool,
    },
    /// A linear leaf of `key_bytes`-byte keys in a slot of `level`; a narrow
    /// pointer when `key_bytes < level`.
    Leaf {
        /// Bytes per key.
        key_bytes: u8,
        /// The slot's level.
        level: u8,
    },
    /// A bitmap leaf in a slot of `level`; a narrow pointer above level 1.
    BitmapLeaf {
        /// The slot's level.
        level: u8,
    },
    /// A full expanse.
    FullExpanse,
    /// A search that reached its final validation with `len` branch
    /// versions retained.
    ReadSet {
        /// Versions retained.
        len: usize,
    },
}

/// Records `v`: logged on this thread in test builds, counted with
/// `occ-stats`.
#[cfg(any(test, feature = "occ-stats"))]
fn visit(v: Visit) {
    #[cfg(feature = "occ-stats")]
    {
        use crate::occ_stats::{Stat, bump, raise};
        match v {
            Visit::RootLeaf => bump(Stat::OrderedVisitRootLeaf),
            Visit::Immed {
                key_bytes,
                count,
                map,
            } => {
                bump(Stat::OrderedVisitImmed);
                let cap = if map {
                    crate::mutate::map_immed_max(key_bytes)
                } else {
                    crate::types::ImmedType::max_count(key_bytes) as usize
                };
                if count as usize == cap {
                    bump(Stat::OrderedVisitImmedFull);
                }
            }
            Visit::Leaf { key_bytes, level } => {
                bump(match key_bytes {
                    1 => Stat::OrderedVisitLeaf1,
                    2 => Stat::OrderedVisitLeaf2,
                    3 => Stat::OrderedVisitLeaf3,
                    4 => Stat::OrderedVisitLeaf4,
                    5 => Stat::OrderedVisitLeaf5,
                    6 => Stat::OrderedVisitLeaf6,
                    _ => Stat::OrderedVisitLeaf7,
                });
                if key_bytes < level {
                    bump(Stat::OrderedVisitLeafNarrow);
                }
            }
            Visit::BitmapLeaf { level } => {
                bump(Stat::OrderedVisitBitmapLeaf);
                if level > 1 {
                    bump(Stat::OrderedVisitBitmapLeafNarrow);
                }
            }
            Visit::FullExpanse => bump(Stat::OrderedVisitFullExpanse),
            Visit::ReadSet { len } => raise(Stat::OrderedReadSetHwm, len as u64),
        }
    }
    #[cfg(test)]
    crate::sync::test_hooks::record_visit(v);
}

/// [`visit`] at a site of the walk. Expands to nothing in the default build,
/// so the walk compiles as it did without it.
macro_rules! visit {
    ($v:expr) => {
        #[cfg(any(test, feature = "occ-stats"))]
        visit($v);
    };
}

/// Every version a search sampled, kept until the final validation.
///
/// Generic over its capacity by monomorphisation: the u64 searches use
/// [`READ_SET_CAP`], and the string map's cross-level walk uses its own
/// (`docs/benchmarks/concurrency/METHODOLOGY.md` §33.8.1). Each instantiation
/// compiles on its own, so the string capacity does not reach the u64 code.
pub(crate) struct ReadSet<const CAP: usize> {
    entries: [MaybeUninit<(*const u32, u32)>; CAP],
    len: usize,
}

/// The cell behind a retained version field.
///
/// # Safety
///
/// `vp` is the version field of an EBR-live node.
#[cfg(not(loom))]
#[inline(always)]
unsafe fn rs_cell<'a>(vp: *const u32) -> &'a crate::occ::VersionCell {
    // SAFETY: forwarded contract.
    unsafe { version_cell(vp) }
}

/// Under loom the read set holds pointers to the model's own cells, which
/// the loom tests construct and pass in as `*const u32` (`occ::version_cell`
/// has no honest cast under loom; see its docs).
///
/// # Safety
///
/// `vp` was cast from a live `&VersionCell`.
#[cfg(loom)]
#[inline(always)]
unsafe fn rs_cell<'a>(vp: *const u32) -> &'a crate::occ::VersionCell {
    // SAFETY: forwarded contract.
    unsafe { &*vp.cast::<crate::occ::VersionCell>() }
}

impl<const CAP: usize> ReadSet<CAP> {
    #[inline(always)]
    pub(crate) const fn new() -> Self {
        Self {
            entries: [const { MaybeUninit::uninit() }; CAP],
            len: 0,
        }
    }

    /// Samples the version at `vp` and retains it.
    ///
    /// # Safety
    ///
    /// `vp` is the version field of an EBR-live branch node.
    #[inline(always)]
    pub(crate) unsafe fn sample(&mut self, vp: *const u32) -> Result<u32, Retry> {
        // SAFETY: live version field, per this function's contract.
        let snap = node_sample(unsafe { rs_cell(vp) }).ok_or(Retry)?;
        if self.len == CAP {
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
    pub(crate) unsafe fn validate_all(&self) -> bool {
        self.entries[..self.len].iter().all(|e| {
            // SAFETY: entries below `len` were written by `sample`.
            let (vp, snap) = unsafe { e.assume_init() };
            // SAFETY: sampled from an EBR-live node under the same pin.
            node_validate(unsafe { rs_cell(vp) }, snap)
        })
    }

    /// Validates the versions retained from index `start` on: the ones one
    /// seek sampled, which include the holder of the entry it returned.
    ///
    /// # Safety
    ///
    /// As [`Self::validate_all`].
    #[inline(always)]
    pub(crate) unsafe fn validate_from(&self, start: usize) -> bool {
        self.entries[start.min(self.len)..self.len].iter().all(|e| {
            // SAFETY: entries below `len` were written by `sample`.
            let (vp, snap) = unsafe { e.assume_init() };
            // SAFETY: sampled from an EBR-live node under the same pin.
            node_validate(unsafe { rs_cell(vp) }, snap)
        })
    }

    /// Versions retained.
    #[inline(always)]
    pub(crate) const fn len(&self) -> usize {
        self.len
    }

    /// True when another sample would overflow: the depth-overflow test of
    /// `docs/benchmarks/concurrency/METHODOLOGY.md` §33.7.4.
    #[inline(always)]
    pub(crate) const fn is_full(&self) -> bool {
        self.len == CAP
    }

    /// Forgets the versions retained from `mark` on. Only the negative
    /// controls call it: a correct search never forgets a version.
    #[cfg(test)]
    pub(crate) fn truncate(&mut self, mark: usize) {
        self.len = self.len.min(mark);
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

/// The version field of a linear branch, by field projection: nothing is
/// loaded, so the version can be sampled before any field it covers is read.
///
/// # Safety
///
/// `edge` references an EBR-live `BranchL3` (`is_l3`) or `BranchL7`.
#[inline(always)]
unsafe fn linear_branch_version(edge: &Edge, is_l3: bool) -> *const u32 {
    let node = edge.node_ptr();
    // SAFETY: EBR-live node per contract; field projections only.
    unsafe {
        if is_l3 {
            &raw const (*node.cast::<BranchL3>()).hdr.version
        } else {
            &raw const (*node.cast::<BranchL7>()).hdr.version
        }
    }
}

/// The level, digit count, digits and edge base of a linear branch, loaded
/// from memory a writer may be changing. Called only after the branch's
/// version was sampled ([`linear_branch_version`]): a header loaded before
/// the sample can predate an in-place insert the sample already reflects,
/// and its digits then index the shifted edges.
///
/// # Safety
///
/// `edge` references an EBR-live `BranchL3` (`is_l3`) or `BranchL7`.
#[inline(always)]
unsafe fn linear_branch(edge: &Edge, is_l3: bool) -> (u8, usize, [u8; 8], *const Edge) {
    let node = edge.node_ptr();
    // SAFETY: EBR-live node per contract; field projections, and the
    // header's meta and digit words loaded atomically (#1086).
    unsafe {
        if is_l3 {
            let b = node.cast::<BranchL3>();
            let h = crate::node::BranchHeader::load_at(&raw const (*b).hdr);
            (
                h.level,
                h.num as usize,
                h.digits,
                (&raw const (*b).edges).cast::<Edge>(),
            )
        } else {
            let b = node.cast::<BranchL7>();
            let h = crate::node::BranchHeader::load_at(&raw const (*b).hdr);
            (
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

/// True when a full-expanse `edge` read in a slot of `level` holds every
/// key of that level: its population field is the level's full count.
///
/// The walk's full-expanse arm answers from the suffix alone and compares no
/// skipped prefix, which is right only then. On a shared set the form is
/// made at level 1 without a skip (`mutate.rs`, the two `FullExpanse`
/// conversions of a 256-key `LeafBitmap1`; `algebra_build.rs`, reached on a
/// shared set only through `compact`'s sorted build), and
/// `SyncExpanseSet::new` is the only constructor, so no shared set holds
/// another kind today. A full expanse behind a narrow pointer, which the
/// algebra builders make on a plain set, would be read wrongly
/// (`docs/benchmarks/concurrency/METHODOLOGY.md` §36.3, C6).
///
/// The set arm therefore checks it in every build and answers `Err(Retry)`
/// when it does not hold: the read then ends in the locked fallback, which
/// answers by the single-threaded walk, instead of returning a key that was
/// never present. Debug builds also panic there, so the case is loud where
/// tests run. The check reads the population field of the walk's own copy
/// of the edge; it loads nothing shared. The map form returns before it.
#[inline(always)]
fn full_expanse_fills(edge: &Edge, level: u8) -> bool {
    (1..=7).contains(&level) && edge.pop0(level) == pow256(level) - 1
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
    let mut rs = ReadSet::<READ_SET_CAP>::new();
    let found = match root {
        RootSnapshot::Empty => None,
        // SAFETY: root state validated just above, under the caller's pin.
        RootSnapshot::Leaf { ptr, pop } => unsafe { root_leaf::<MAP>(ptr, pop, key, true) },
        RootSnapshot::Tree { top } => {
            // SAFETY: the top edge copy is validated against the tree version.
            unsafe { next_in::<MAP, READ_SET_CAP>(&top, key, 8, Holder::Tree(ver, snap), &mut rs)? }
        }
    };
    visit!(Visit::ReadSet { len: rs.len });
    #[cfg(test)]
    crate::sync::test_hooks::at(crate::sync::test_hooks::Site::OrderedFinal);
    // The answer is returned only after this check (`docs/ARCHITECTURE.md`
    // §4.1, S5).
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
    let mut rs = ReadSet::<READ_SET_CAP>::new();
    let found = match root {
        RootSnapshot::Empty => None,
        // SAFETY: root state validated just above, under the caller's pin.
        RootSnapshot::Leaf { ptr, pop } => unsafe { root_leaf::<MAP>(ptr, pop, key, false) },
        RootSnapshot::Tree { top } => {
            // SAFETY: the top edge copy is validated against the tree version.
            unsafe { prev_in::<MAP, READ_SET_CAP>(&top, key, 8, Holder::Tree(ver, snap), &mut rs)? }
        }
    };
    visit!(Visit::ReadSet { len: rs.len });
    #[cfg(test)]
    crate::sync::test_hooks::at(crate::sync::test_hooks::Site::OrderedFinal);
    // The answer is returned only after this check (`docs/ARCHITECTURE.md`
    // §4.1, S5).
    // SAFETY: same pin as every sample.
    if !unsafe { rs.validate_all() } || !ver.validate(snap) {
        return Err(Retry);
    }
    Ok(found)
}

/// One seek of the string map's cross-level walk (#1143): the smallest entry
/// `>= key` (`forward`) or the largest `<= key` of a `StrNode` sub-map, with
/// the word it maps the chunk to.
///
/// The sub-map's root state has no version of its own; the node's cover word
/// stands where the tree version stands for a map (`strmap.rs`, `StrNode`),
/// so it is the holder of the top edge and of a root leaf. Every branch the
/// seek samples is retained in `rs`, as in [`next_validated`]; the caller
/// owns the final validation.
///
/// # Safety
///
/// `root` was copied after `csnap` was sampled from `cover` and validated
/// against it afterwards, `cover` is the cover word of an EBR-live `StrNode`,
/// and the caller holds the pin for the whole call.
#[inline]
pub(crate) unsafe fn seek_submap<const CAP: usize>(
    root: RootSnapshot,
    key: u64,
    forward: bool,
    cover: *const u32,
    csnap: u32,
    rs: &mut ReadSet<CAP>,
) -> Result<Option<(u64, u64)>, Retry> {
    match root {
        RootSnapshot::Empty => Ok(None),
        // SAFETY: the root state was validated against the cover (contract).
        RootSnapshot::Leaf { ptr, pop } => Ok(unsafe { root_leaf::<true>(ptr, pop, key, forward) }),
        RootSnapshot::Tree { top } => {
            let holder = Holder::Node(cover, csnap);
            // SAFETY: the top edge copy is validated against the cover.
            unsafe {
                if forward {
                    next_in::<true, CAP>(&top, key, 8, holder, rs)
                } else {
                    prev_in::<true, CAP>(&top, key, 8, holder, rs)
                }
            }
        }
    }
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
    visit!(Visit::RootLeaf);
    let keys = ptr.cast::<u64>();
    let (mut lo, mut hi) = (0usize, pop);
    while lo < hi {
        let mid = (lo + hi) / 2;
        // SAFETY: `mid < pop`, in bounds of the live root leaf.
        let k = unsafe { shared_word::load::<true>(keys.add(mid)) };
        // A writer armed here changes the leaf between two key loads; the
        // entry point's final check of the tree version is what sees it.
        #[cfg(test)]
        crate::sync::test_hooks::at(crate::sync::test_hooks::Site::OrderedRootLeafProbe);
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
unsafe fn next_in<const MAP: bool, const CAP: usize>(
    edge: &Edge,
    suffix: u64,
    level: u8,
    holder: Holder<'_>,
    rs: &mut ReadSet<CAP>,
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
            visit!(Visit::Immed {
                key_bytes: level,
                count: im.key_count(),
                map: MAP,
            });
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
            visit!(Visit::Leaf {
                key_bytes: kb,
                level,
            });
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
            visit!(Visit::BitmapLeaf { level });
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
            visit!(Visit::FullExpanse);
            if !full_expanse_fills(edge, level) {
                debug_assert!(false, "a full expanse must hold every key of that level");
                return Err(Retry);
            }
            Ok(Some((suffix, 0)))
        }

        EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
            let is_l3 = matches!(t, EdgeType::BranchL3);
            // SAFETY: validated edge copy → live branch; field projection only.
            let vp = unsafe { linear_branch_version(edge, is_l3) };
            // The negative control loads the header before the sample, as the walk
            // once did.
            #[cfg(test)]
            let early = crate::sync::test_hooks::loads_header_before_sample()
                // SAFETY: as for the load below.
                .then(|| unsafe { linear_branch(edge, is_l3) });
            #[cfg(test)]
            crate::sync::test_hooks::before_branch_sample(edge.node_ptr().cast_const());
            // SAFETY: `vp` is the live branch's version field.
            let nsnap = unsafe { rs.sample(vp)? };
            // The header is loaded after the sample, as every seqlock read loads
            // the data its version covers: an in-place insert shifts the edges and
            // rewrites the header under this version, so a header that predates the
            // sample can pair a digit with its neighbour's edge, and no later check
            // would see it (`docs/ARCHITECTURE.md` §4.1).
            // SAFETY: validated edge copy → live branch; loads validated below.
            let (bl, num, digits, edges) = unsafe { linear_branch(edge, is_l3) };
            #[cfg(test)]
            let (bl, num, digits, edges) = early.unwrap_or((bl, num, digits, edges));
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
                if let Some((r, v)) = unsafe { next_in::<MAP, CAP>(&child, rem, bl - 1, here, rs)? }
                {
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
            // The negative control takes the first digit before the sample.
            #[cfg(test)]
            let early = crate::sync::test_hooks::takes_digit_before_sample()
                // SAFETY: as for the loads below.
                .then(|| unsafe { early_branch_b_digit(node, edge, level, suffix, true) });
            #[cfg(test)]
            crate::sync::test_hooks::before_branch_sample(edge.node_ptr().cast_const());
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
            // The digit search follows the sample, as every load the version
            // covers does: a digit taken before it can outlive the removal of
            // its child, and its rank then indexes a neighbour's edge.
            // SAFETY: live node; bitmap loads are validated before use.
            let mut cur = unsafe {
                crate::bits::shared_bitmap::next_set::<true>(&raw const (*node).bitmap, d)
            };
            #[cfg(test)]
            if let Some(e) = early {
                cur = e;
            }
            while let Some(bd) = cur {
                let rem = if bd == d { key_low(suffix, bl - 1) } else { 0 };
                // SAFETY: as above.
                let child = unsafe { branch_b_child(node, bd, here)? };
                let mark = rs.len;
                // SAFETY: child copy validated against this branch.
                if let Some((r, v)) = unsafe { next_in::<MAP, CAP>(&child, rem, bl - 1, here, rs)? }
                {
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
            // The negative control loads the probe digit's child edge before
            // the sample.
            #[cfg(test)]
            let early = crate::sync::test_hooks::loads_child_before_sample().then(|| {
                // SAFETY: as for the load in the loop below.
                unsafe {
                    Edge::load_at::<true>(
                        (&raw const (*node).edges)
                            .cast::<Edge>()
                            .add(digit(suffix, level) as usize),
                    )
                }
            });
            #[cfg(test)]
            crate::sync::test_hooks::before_branch_sample(edge.node_ptr().cast_const());
            // SAFETY: live version field.
            let nsnap = unsafe { rs.sample(vp)? };
            let here = Holder::Node(vp, nsnap);
            let d = digit(suffix, level);
            for bd in d..=255u8 {
                // Every child edge is loaded after the sample: an edge copied
                // before it can predate an insert below it that the sample
                // already reflects, and the search would then pass over a key
                // and answer from a sibling.
                // SAFETY: direct index into the live 256-slot node.
                let child = unsafe {
                    Edge::load_at::<true>(
                        (&raw const (*node).edges).cast::<Edge>().add(bd as usize),
                    )
                };
                #[cfg(test)]
                let child = match early {
                    Some(e) if bd == d => e,
                    _ => child,
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
                if let Some((r, v)) =
                    // SAFETY: child copy validated against this branch.
                    unsafe { next_in::<MAP, CAP>(&child, rem, level - 1, here, rs)? }
                {
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
unsafe fn prev_in<const MAP: bool, const CAP: usize>(
    edge: &Edge,
    suffix: u64,
    level: u8,
    holder: Holder<'_>,
    rs: &mut ReadSet<CAP>,
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
            visit!(Visit::Immed {
                key_bytes: level,
                count: im.key_count(),
                map: MAP,
            });
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
            visit!(Visit::Leaf {
                key_bytes: kb,
                level,
            });
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
            visit!(Visit::BitmapLeaf { level });
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
            visit!(Visit::FullExpanse);
            if !full_expanse_fills(edge, level) {
                debug_assert!(false, "a full expanse must hold every key of that level");
                return Err(Retry);
            }
            Ok(Some((suffix.min(pow256(level) - 1), 0)))
        }

        EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
            let is_l3 = matches!(t, EdgeType::BranchL3);
            // SAFETY: validated edge copy → live branch; field projection only.
            let vp = unsafe { linear_branch_version(edge, is_l3) };
            // The negative control loads the header before the sample, as the walk
            // once did.
            #[cfg(test)]
            let early = crate::sync::test_hooks::loads_header_before_sample()
                // SAFETY: as for the load below.
                .then(|| unsafe { linear_branch(edge, is_l3) });
            #[cfg(test)]
            crate::sync::test_hooks::before_branch_sample(edge.node_ptr().cast_const());
            // SAFETY: `vp` is the live branch's version field.
            let nsnap = unsafe { rs.sample(vp)? };
            // The header is loaded after the sample, as every seqlock read loads
            // the data its version covers: an in-place insert shifts the edges and
            // rewrites the header under this version, so a header that predates the
            // sample can pair a digit with its neighbour's edge, and no later check
            // would see it (`docs/ARCHITECTURE.md` §4.1).
            // SAFETY: validated edge copy → live branch; loads validated below.
            let (bl, num, digits, edges) = unsafe { linear_branch(edge, is_l3) };
            #[cfg(test)]
            let (bl, num, digits, edges) = early.unwrap_or((bl, num, digits, edges));
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
                if let Some((r, v)) = unsafe { prev_in::<MAP, CAP>(&child, rem, bl - 1, here, rs)? }
                {
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
            // The negative control takes the first digit before the sample.
            #[cfg(test)]
            let early = crate::sync::test_hooks::takes_digit_before_sample()
                // SAFETY: as for the loads below.
                .then(|| unsafe { early_branch_b_digit(node, edge, level, suffix, false) });
            #[cfg(test)]
            crate::sync::test_hooks::before_branch_sample(edge.node_ptr().cast_const());
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
            // The digit search follows the sample, as in `next_in`.
            // SAFETY: live node; bitmap loads are validated before use.
            let mut cur = unsafe {
                crate::bits::shared_bitmap::prev_set::<true>(&raw const (*node).bitmap, d)
            };
            #[cfg(test)]
            if let Some(e) = early {
                cur = e;
            }
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
                if let Some((r, v)) = unsafe { prev_in::<MAP, CAP>(&child, rem, bl - 1, here, rs)? }
                {
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
            // The negative control loads the probe digit's child edge before
            // the sample.
            #[cfg(test)]
            let early = crate::sync::test_hooks::loads_child_before_sample().then(|| {
                // SAFETY: as for the load in the loop below.
                unsafe {
                    Edge::load_at::<true>(
                        (&raw const (*node).edges)
                            .cast::<Edge>()
                            .add(digit(suffix, level) as usize),
                    )
                }
            });
            #[cfg(test)]
            crate::sync::test_hooks::before_branch_sample(edge.node_ptr().cast_const());
            // SAFETY: live version field.
            let nsnap = unsafe { rs.sample(vp)? };
            let here = Holder::Node(vp, nsnap);
            let d = digit(suffix, level);
            for bd in (0..=d).rev() {
                // Every child edge is loaded after the sample: an edge copied
                // before it can predate an insert below it that the sample
                // already reflects, and the search would then pass over a key
                // and answer from a sibling.
                // SAFETY: direct index into the live 256-slot node.
                let child = unsafe {
                    Edge::load_at::<true>(
                        (&raw const (*node).edges).cast::<Edge>().add(bd as usize),
                    )
                };
                #[cfg(test)]
                let child = match early {
                    Some(e) if bd == d => e,
                    _ => child,
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
                if let Some((r, v)) =
                    // SAFETY: child copy validated against this branch.
                    unsafe { prev_in::<MAP, CAP>(&child, rem, level - 1, here, rs)? }
                {
                    return Ok(Some((compose(bd, r, level), v)));
                }
                backtrack(rs, mark);
            }
            Ok(None)
        }
    }
}

/// The first digit a `BranchB` search would take, loaded as the walk loads
/// it. Only the negative control of the bitmap-branch sample order calls it,
/// before the sample (`sync::test_hooks::set_digit_before_sample`); `None`
/// when the search would end without taking a digit.
///
/// # Safety
///
/// `node` is the EBR-live `BranchB` that `edge` names.
#[cfg(test)]
unsafe fn early_branch_b_digit(
    node: *const BranchB,
    edge: &Edge,
    level: u8,
    suffix: u64,
    forward: bool,
) -> Option<u8> {
    use core::cmp::Ordering::{Equal, Greater, Less};
    // SAFETY: live node per contract.
    let bl = unsafe { (*node).level };
    assert!((2..=level).contains(&bl), "a quiescent BranchB");
    let (ord, _, _) = skip_cmp(edge, bl, level, suffix);
    let suffix = match (ord, forward) {
        (Equal, _) => key_low(suffix, bl),
        (Less, true) => 0,
        (Greater, false) => pow256(bl) - 1,
        (Greater, true) | (Less, false) => return None,
    };
    let d = digit(suffix, bl);
    // SAFETY: live node per contract.
    unsafe {
        if forward {
            crate::bits::shared_bitmap::next_set::<true>(&raw const (*node).bitmap, d)
        } else {
            crate::bits::shared_bitmap::prev_set::<true>(&raw const (*node).bitmap, d)
        }
    }
}

/// A child subtree answered nothing; the search moves to its sibling.
///
/// Test builds park here (G12.1), and the negative control forgets the
/// child's snapshots, as a single moving cover would.
#[inline(always)]
fn backtrack<const CAP: usize>(rs: &mut ReadSet<CAP>, mark: usize) {
    #[cfg(test)]
    {
        crate::sync::test_hooks::before_ordered_backtrack();
        crate::sync::test_hooks::at(crate::sync::test_hooks::Site::OrderedBacktrack);
        if crate::sync::test_hooks::drops_child_snapshots() {
            rs.truncate(mark);
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
        // A writer armed here changes the leaf after the walk copied its
        // edge and before it loads a bitmap word; nothing below checks the
        // holder, so the final validation is what sees it.
        #[cfg(test)]
        crate::sync::test_hooks::at(crate::sync::test_hooks::Site::OrderedSetBitmapLeaf);
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

/// The fixture of the branch-sample tests (#900): the tree that the ordered
/// walk, the batch cursor and the set form are driven over when a writer
/// shifts a linear branch in place before the walk samples its version.
///
/// Keys `0x10_01..=0x10_21` and `0x30_01..=0x30_21` (odd low bytes, 17 under
/// each digit) give one level-2 `BranchL3`, B, with digits `0x10` and `0x30`
/// over two `Leaf1` children, under a chain of single-digit branches. B has
/// room for a third digit, so a key under a new level-2 digit is inserted in
/// place: B's edges shift and only B's version moves.
#[cfg(test)]
pub(crate) mod branch_sample_fixture {
    use crate::node::{BranchL3, Edge};
    use crate::sync::RootSnapshot;
    use crate::types::{EdgeTag, EdgeType};

    /// The fixture's keys.
    pub(crate) fn keys() -> Vec<u64> {
        [0x10u64, 0x30]
            .iter()
            .flat_map(|&d| (0..17u64).map(move |i| (d << 8) | (1 + 2 * i)))
            .collect()
    }

    /// B and the version fields of the branches above it, from a root
    /// snapshot of a quiescent tree. Panics unless B is a level-2 `BranchL3`
    /// holding exactly `digits`, under a chain of single-digit `BranchL3`s,
    /// and, when `leaves` is set, unless its two children are 17-key
    /// `Leaf1`s (AGENTS.md §5 negative-control discrimination).
    pub(crate) fn branch_b(
        root: RootSnapshot,
        digits: &[u8],
        leaves: bool,
    ) -> (*const BranchL3, Vec<*const u32>) {
        let RootSnapshot::Tree { top } = root else {
            panic!("root must be a tree")
        };
        let mut above = Vec::new();
        // SAFETY: live nodes of a tree no writer is changing; every field is
        // copied out before any write.
        unsafe {
            let mut edge = top;
            loop {
                assert_eq!(
                    edge.tag(),
                    Some(EdgeTag::Structural(EdgeType::BranchL3)),
                    "every branch on the path is a BranchL3"
                );
                let b = edge.node_ptr().cast::<BranchL3>().cast_const();
                let (level, num, hd) = ((*b).hdr.level, (*b).hdr.num as usize, (*b).hdr.digits);
                let edges: [Edge; 3] = (*b).edges;
                if level == 2 {
                    assert_eq!(&hd[..num], digits, "B's digits");
                    if leaves {
                        for e in &edges[..num] {
                            assert_eq!(e.tag(), Some(EdgeTag::Structural(EdgeType::Leaf1)));
                            assert_eq!(e.pop0(1) + 1, 17);
                        }
                    }
                    return (b, above);
                }
                assert_eq!(num, 1, "a single-digit chain above B");
                above.push(&raw const (*b).hdr.version);
                edge = edges[0];
            }
        }
    }

    /// The version word at `vp`.
    pub(crate) fn version(vp: *const u32) -> u32 {
        // SAFETY: the fixture's nodes are not replaced by the in-place insert
        // the tests make, and each tree outlives every read of its versions.
        let cell = unsafe { crate::occ::version_cell(vp) };
        cell.load(core::sync::atomic::Ordering::Acquire)
    }
}

/// The string map's cross-level retention (#1143) on the real [`ReadSet`]:
/// `ReadSet::sample`, `validate_from` and `validate_all` over node cover
/// words, as `StrNode` covers stand in the walk. A successor search descends
/// a child level C, finds nothing there, unwinds and takes the minimum of
/// the next sibling level S. The writer inserts `K_PRIME` into C under C's
/// bracket, then a new minimum `M` into S under S's. The successor is `M0`
/// before the first insert and `K_PRIME` from then on, so `M` never is.
#[cfg(all(test, loom))]
mod loom_tests {
    use super::ReadSet;
    use crate::occ::{VersionCell, version_begin, version_end};
    use loom::sync::Arc;
    use loom::sync::atomic::{AtomicU64, Ordering};

    const M0: u64 = 0x30;
    const M: u64 = 0x20;
    const K_PRIME: u64 = 0x10;

    fn cover(v: &Arc<VersionCell>) -> *const u32 {
        Arc::as_ptr(v).cast::<u32>()
    }

    /// One search; `drop_level` forgets C's versions at the unwind, as the
    /// `DropLevelOnUnwind` control does in `ExpanseStrMap::ordered_validated`.
    fn search(
        c_v: &Arc<VersionCell>,
        c_key: &AtomicU64,
        s_v: &Arc<VersionCell>,
        s_min: &AtomicU64,
        drop_level: bool,
    ) -> Option<u64> {
        let mut rs = ReadSet::<4>::new();
        let level = rs.len();
        // SAFETY: the cell outlives the search; under loom the read set
        // casts the pointer back to it.
        unsafe { rs.sample(cover(c_v)) }.ok()?;
        let x = c_key.load(Ordering::Relaxed);
        if x != 0 {
            // SAFETY: as above.
            return unsafe { rs.validate_all() }.then_some(x);
        }
        // C answered nothing: unwind to the sibling level.
        if drop_level {
            rs.truncate(level);
        }
        let from = rs.len();
        // SAFETY: as above.
        unsafe { rs.sample(cover(s_v)) }.ok()?;
        let y = s_min.load(Ordering::Relaxed);
        // The holder check before a dereference, then the final validation.
        // SAFETY: as above.
        if !unsafe { rs.validate_from(from) } || !unsafe { rs.validate_all() } {
            return None;
        }
        Some(y)
    }

    fn model(drop_level: bool) {
        loom::model(move || {
            let (c_v, s_v) = (Arc::new(VersionCell::new(0)), Arc::new(VersionCell::new(0)));
            let (c_key, s_min) = (Arc::new(AtomicU64::new(0)), Arc::new(AtomicU64::new(M0)));
            let writer = {
                let (c_v, s_v, c_key, s_min) = (
                    Arc::clone(&c_v),
                    Arc::clone(&s_v),
                    Arc::clone(&c_key),
                    Arc::clone(&s_min),
                );
                loom::thread::spawn(move || {
                    version_begin(&c_v);
                    c_key.store(K_PRIME, Ordering::Relaxed);
                    version_end(&c_v);
                    version_begin(&s_v);
                    s_min.store(M, Ordering::Relaxed);
                    version_end(&s_v);
                })
            };
            if let Some(a) = search(&c_v, &c_key, &s_v, &s_min, drop_level) {
                assert!(
                    a == M0 || a == K_PRIME,
                    "string ordered read returned {a:#x}, never the successor"
                );
            }
            writer.join().unwrap();
        });
    }

    #[test]
    fn loom_str_ordered_read_retained_read_set() {
        model(false);
    }

    /// The negative control: forgetting C's versions at the unwind, as a
    /// single moving cover would, lets the search return `M`.
    #[test]
    #[should_panic(expected = "never the successor")]
    fn loom_str_ordered_read_hand_over_hand_is_not_enough() {
        model(true);
    }

    /// The linear-branch arm of `next_in` (#900) on the real [`ReadSet`]: a
    /// branch B with digits `0x10` and `0x30`, whose header word packs the
    /// digit count in its top byte and the digits from its low byte. Each
    /// child edge is modelled by the digit it was linked under. The writer
    /// inserts digit `0x20` in place under B's bracket, as
    /// `linear_insert_at_shared` does: the edges shift right, then the header
    /// is rewritten. The successor search for digit `0x21` must pair `0x30`
    /// with the edge linked under `0x30`.
    fn branch_search(
        b_v: &Arc<VersionCell>,
        hdr: &AtomicU64,
        edges: &[AtomicU64; 3],
        header_first: bool,
    ) -> Option<(u8, u64)> {
        let load_header = || {
            let h = hdr.load(Ordering::Relaxed);
            let num = (h >> 56) as usize;
            let digits: [u8; 8] = h.to_le_bytes();
            (num, digits)
        };
        let mut rs = ReadSet::<2>::new();
        let early = header_first.then(load_header);
        // SAFETY: the cell outlives the search; under loom the read set
        // casts the pointer back to it.
        unsafe { rs.sample(cover(b_v)) }.ok()?;
        let (num, digits) = early.unwrap_or_else(load_header);
        if num > 3 {
            return None;
        }
        let start = digits[..num].partition_point(|&d| d < 0x21);
        let slot = start;
        if slot == num {
            return None;
        }
        let child = edges[slot].load(Ordering::Relaxed);
        // The holder check after the edge load, then the final validation.
        // SAFETY: as above.
        if !unsafe { rs.validate_from(0) } || !unsafe { rs.validate_all() } {
            return None;
        }
        Some((digits[slot], child))
    }

    fn branch_model(header_first: bool) {
        loom::model(move || {
            let b_v = Arc::new(VersionCell::new(0));
            let pack = |num: u64, digits: &[u8]| {
                let mut b = [0u8; 8];
                b[..digits.len()].copy_from_slice(digits);
                u64::from_le_bytes(b) | (num << 56)
            };
            let hdr = Arc::new(AtomicU64::new(pack(2, &[0x10, 0x30])));
            let edges = Arc::new([
                AtomicU64::new(0x10),
                AtomicU64::new(0x30),
                AtomicU64::new(0),
            ]);
            let writer = {
                let (b_v, hdr, edges) = (Arc::clone(&b_v), Arc::clone(&hdr), Arc::clone(&edges));
                loom::thread::spawn(move || {
                    version_begin(&b_v);
                    edges[2].store(0x30, Ordering::Relaxed);
                    edges[1].store(0x20, Ordering::Relaxed);
                    hdr.store(pack(3, &[0x10, 0x20, 0x30]), Ordering::Relaxed);
                    version_end(&b_v);
                })
            };
            if let Some((d, child)) = branch_search(&b_v, &hdr, &edges, header_first) {
                assert!(
                    u64::from(d) == child,
                    "digit {d:#x} paired with the edge linked under {child:#x}"
                );
            }
            writer.join().unwrap();
        });
    }

    #[test]
    fn loom_ordered_read_samples_a_linear_branch_before_its_header() {
        branch_model(false);
    }

    /// The negative control: loading the header before the sample, the
    /// order the walk shipped with, lets an old digit pair with a shifted
    /// edge.
    #[test]
    #[should_panic(expected = "paired with the edge linked under")]
    fn loom_ordered_read_header_before_sample_pairs_a_shifted_edge() {
        branch_model(true);
    }

    // ---- The set form's terminals (#1194; METHODOLOGY.md §36.4, G36.5) ----
    //
    // What these two models cover: the validation rule, on the real
    // `ReadSet::sample` and `validate_all`, for a terminal read in more than
    // one load and for a terminal whose holder slot is rewritten. What they
    // do not cover: `bitmap_leaf::<false>`, `next_in` and `prev_in`
    // themselves. The search is written by hand over atomics, so a model
    // stays green if that code is wrong; the direct-call tests in `sync`
    // are what run it.

    /// Two words of a set bitmap leaf.
    type Words = [AtomicU64; 2];

    /// The first set bit at or after `from`, read as the walk reads a set
    /// bitmap leaf: word 0, then word 1 only when word 0 has nothing.
    fn first_bit(w: &Words, from: u32) -> Option<u64> {
        let w0 = w[0].load(Ordering::Relaxed) & (u64::MAX << from);
        if w0 != 0 {
            return Some(u64::from(w0.trailing_zeros()));
        }
        let w1 = w[1].load(Ordering::Relaxed);
        (w1 != 0).then(|| 64 + u64::from(w1.trailing_zeros()))
    }

    /// G36.5 (a), the search: a holder branch `h_v` and a set bitmap leaf of
    /// two words, covered by the holder alone. `validate_final` off is the
    /// negative control.
    fn bitmap_terminal_search(
        h_v: &Arc<VersionCell>,
        w: &Words,
        validate_final: bool,
    ) -> Option<u64> {
        let mut rs = ReadSet::<1>::new();
        // SAFETY: the cell outlives the search; under loom the read set
        // casts the pointer back to it.
        unsafe { rs.sample(cover(h_v)) }.ok()?;
        let found = first_bit(w, 0)?;
        // SAFETY: as above.
        if validate_final && !unsafe { rs.validate_all() } {
            return None;
        }
        Some(found)
    }

    /// The successor is bit 64 + 5 until the writer sets bit 3 of word 0,
    /// and bit 3 from then on. The writer then sets bit 2 of word 1, in the
    /// same bracket on the holder: a search that read word 0 before the
    /// bracket and word 1 after it finds bit 64 + 2, which never was the
    /// successor.
    fn bitmap_terminal_model(validate_final: bool) {
        loom::model(move || {
            let h_v = Arc::new(VersionCell::new(0));
            let w: Arc<Words> = Arc::new([AtomicU64::new(0), AtomicU64::new(1 << 5)]);
            let writer = {
                let (h_v, w) = (Arc::clone(&h_v), Arc::clone(&w));
                loom::thread::spawn(move || {
                    version_begin(&h_v);
                    w[0].store(1 << 3, Ordering::Relaxed);
                    w[1].store((1 << 5) | (1 << 2), Ordering::Relaxed);
                    version_end(&h_v);
                })
            };
            if let Some(a) = bitmap_terminal_search(&h_v, &w, validate_final) {
                assert!(
                    a == 64 + 5 || a == 3,
                    "set bitmap search returned bit {a}, never the successor"
                );
            }
            writer.join().unwrap();
        });
    }

    #[test]
    fn loom_set_ordered_bitmap_terminal_is_covered_by_the_final_validation() {
        bitmap_terminal_model(true);
    }

    /// The negative control of (a): without the final validation the search
    /// returns the bit that never was the successor.
    #[test]
    #[should_panic(expected = "never the successor")]
    fn loom_set_ordered_bitmap_terminal_without_the_final_validation_tears() {
        bitmap_terminal_model(false);
    }

    const SLOT_BITMAP: u64 = 1;
    const SLOT_FULL: u64 = 2;
    /// The probe of (b): the top digit of word 0.
    const FROM: u32 = 63;

    /// G36.5 (b), the search: the holder's slot names a bitmap leaf or a
    /// full expanse. `validate_last` off is the negative control, which
    /// validates the holder after the slot load and before the terminal's
    /// loads, and not after them.
    fn replaced_terminal_search(
        h_v: &Arc<VersionCell>,
        slot: &AtomicU64,
        w: &Words,
        validate_last: bool,
    ) -> Option<u64> {
        let mut rs = ReadSet::<1>::new();
        // SAFETY: the cell outlives the search; under loom the read set
        // casts the pointer back to it.
        unsafe { rs.sample(cover(h_v)) }.ok()?;
        let edge = slot.load(Ordering::Relaxed);
        // SAFETY: as above.
        if !validate_last && !unsafe { rs.validate_all() } {
            return None;
        }
        let found = if edge == SLOT_FULL {
            u64::from(FROM)
        } else {
            first_bit(w, FROM)?
        };
        // SAFETY: as above.
        if validate_last && !unsafe { rs.validate_all() } {
            return None;
        }
        Some(found)
    }

    /// The leaf starts one key short of full: every digit but 63. The
    /// successor of 63 is 64. The writer sets bit 63 and rewrites the slot
    /// to a full expanse (successor 63), then, in a second bracket, removes
    /// digit 64 and rewrites the slot to a bitmap leaf again (successor
    /// still 63). A search that read word 0 before the first bracket and
    /// word 1 after the second finds 65, which never was the successor.
    ///
    /// **A rule-level check on an address-stable cell, not evidence about
    /// the engine.** The model keeps the leaf's words at one address across
    /// the two rewrites. On a shared set the second rewrite builds a new
    /// node, and the node a pinned search holds is not reused while its pin
    /// lasts, so the schedule on which the control below fails cannot occur
    /// there. What protects the engine on this path is the pin and the rule
    /// that a pinned node is not reused; neither has a test here. The
    /// positive model checks the validation rule against an adversary
    /// stronger than the engine; its control shows the model can fail, and
    /// is not counted as a negative control of the engine
    /// (`docs/benchmarks/concurrency/METHODOLOGY.md` §36.13, item 7).
    fn replaced_terminal_model(validate_last: bool) {
        loom::model(move || {
            let h_v = Arc::new(VersionCell::new(0));
            let slot = Arc::new(AtomicU64::new(SLOT_BITMAP));
            let w: Arc<Words> = Arc::new([AtomicU64::new(!(1 << FROM)), AtomicU64::new(u64::MAX)]);
            let writer = {
                let (h_v, slot, w) = (Arc::clone(&h_v), Arc::clone(&slot), Arc::clone(&w));
                loom::thread::spawn(move || {
                    version_begin(&h_v);
                    w[0].store(u64::MAX, Ordering::Relaxed);
                    slot.store(SLOT_FULL, Ordering::Relaxed);
                    version_end(&h_v);
                    version_begin(&h_v);
                    w[1].store(u64::MAX & !1, Ordering::Relaxed);
                    slot.store(SLOT_BITMAP, Ordering::Relaxed);
                    version_end(&h_v);
                })
            };
            if let Some(a) = replaced_terminal_search(&h_v, &slot, &w, validate_last) {
                assert!(
                    a == 64 || a == u64::from(FROM),
                    "search over a replaced terminal returned {a}, never the successor"
                );
            }
            writer.join().unwrap();
        });
    }

    #[test]
    fn loom_set_ordered_replaced_terminal_is_covered_by_the_final_validation() {
        replaced_terminal_model(true);
    }

    /// The model of (b) with its holder validated only before the
    /// terminal's loads: the search returns 65. It shows that the model
    /// can fail, on a schedule the engine cannot produce (see
    /// `replaced_terminal_model`), so it is not a negative control of the
    /// engine.
    #[test]
    #[should_panic(expected = "never the successor")]
    fn loom_set_ordered_replaced_terminal_validated_only_before_its_loads_tears() {
        replaced_terminal_model(false);
    }
}
