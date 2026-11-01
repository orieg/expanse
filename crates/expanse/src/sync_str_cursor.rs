//! Validated batch cursor on the shared string map's readers (#1143).
//!
//! A [`SyncStrMapCursor`] scans a [`SyncExpanseStrMap`] in ascending
//! byte-lexicographic order, optionally bounded to `[lo, hi)` or to the keys
//! under a prefix, by copying up to [`STR_BATCH_CAP`] entries and
//! [`STR_KEY_ARENA`] key bytes at a time into a cursor-owned buffer under one
//! optimistic read. It is the string twin of the map readers' batch cursor
//! ([`crate::sync_cursor::SyncMapCursor`], #1142), with the fixes that cursor
//! needed (#1382, #1385) from the start: a bounded scan stops at its end bound,
//! [`SyncStrMapCursor::advance_to`] never moves backwards, and a seek past the
//! end bound exhausts the cursor.
//!
//! # Protocol
//!
//! One batch is one attempt of `ExpanseStrMap::scan_validated`
//! (`docs/benchmarks/concurrency/METHODOLOGY.md` §33.10 and §33.11): it
//! descends the probe's exact continuations, drains `StrNode` sub-maps,
//! entering each child it meets and climbing back to the parent's next chunk
//! when a level runs out, copies each key and value into the batch, and
//! validates every version it sampled, and the tree word, before the batch is
//! used. The exact descent and the first drained level retain at most
//! `8d + 6` versions at path depth `d`
//! (`scripts/str_cursor_bounds.py::str_cursor_drain_attempt_read_set_bound`),
//! and every later drain runs only when the read set has room for its worst
//! case; a drain refused while the batch is empty ends the attempt with a
//! validated restart by key.
//! A failed validation retries; after `MAX_RETRIES` failures, or at once when
//! a full read set still validates (a path too deep for it, §33.7.4), the batch
//! is filled under the writer lock by the single-threaded [`StrCursor`].
//!
//! The epoch is pinned only while a batch is copied. Between batches and
//! between calls the cursor holds no pin and no node pointer: it resumes from
//! the root by key, the last key returned followed by `0x01`.
//!
//! # What a scan sees
//!
//! Keys come back strictly ascending and at most once. A key present for the
//! whole scan is returned; a key inserted or removed while the scan runs may or
//! may not be. It is not a snapshot (#1103).
//!
//! [`StrCursor`]: crate::strmap::StrCursor

use crate::bits::shared_word;
use crate::leaf;
use crate::mutate::key_low;
use crate::node::{BranchB, BranchU, Edge, LeafBitmapL};
use crate::occ::Reader;
use crate::strmap::NulFreeStr;
use crate::sync::SyncExpanseStrMap;
use crate::sync::{Retry, RootSnapshot};
use crate::sync_nav::{
    Holder, ReadSet, branch_b_child, immed_value, linear_branch, linear_branch_version, skip_cmp,
};
use crate::types::{EdgeTag, EdgeType, digit};
use core_alloc::boxed::Box;
use core_alloc::vec::Vec;

/// Most entries one batch holds: the u64 cursor's batch, one level-1 bitmap
/// leaf (`crate::sync_cursor::BATCH_CAP`).
pub const STR_BATCH_CAP: usize = crate::sync_cursor::BATCH_CAP;

/// Key bytes one batch holds (§33.1). A key longer than this is returned
/// alone, from a buffer sized to it.
pub const STR_KEY_ARENA: usize = 4096;

/// Offset of an entry whose key lives in the spill buffer.
const SPILL: u32 = u32::MAX;

/// One batch: the copied keys and values, and the drain's scratch pairs.
pub(crate) struct StrBatch {
    keys: [u8; STR_KEY_ARENA],
    used: usize,
    /// `(value, key offset, key length)` per entry.
    entries: [(u64, u32, u32); STR_BATCH_CAP],
    /// Entries written.
    pub(crate) count: usize,
    /// The `(chunk, value)` pairs one sub-map drain collects.
    pub(crate) pairs: [(u64, u64); STR_BATCH_CAP],
    /// The levels the attempt holds, root first (§33.11.2).
    pub(crate) levels: Vec<crate::strmap::ScanLevel>,
    /// The key of a batch of one whose key does not fit the arena.
    spill: Vec<u8>,
}

impl StrBatch {
    fn new() -> Box<Self> {
        Box::new(Self {
            keys: [0; STR_KEY_ARENA],
            used: 0,
            entries: [(0, 0, 0); STR_BATCH_CAP],
            count: 0,
            pairs: [(0, 0); STR_BATCH_CAP],
            levels: Vec::new(),
            spill: Vec::new(),
        })
    }

    /// Empties the batch.
    #[inline]
    pub(crate) fn clear(&mut self) {
        self.count = 0;
        self.used = 0;
    }

    /// Appends the entry whose key is `a ++ b ++ c`. Returns `false`, writing
    /// nothing, when the batch is full or the key does not fit the arena; an
    /// empty batch takes any key, into the spill buffer if it must.
    #[inline]
    pub(crate) fn push(&mut self, a: &[u8], b: &[u8], c: &[u8], value: u64) -> bool {
        if self.count == STR_BATCH_CAP {
            return false;
        }
        let len = a.len() + b.len() + c.len();
        let at = self.used;
        if len <= STR_KEY_ARENA - at {
            let dst = &mut self.keys[at..at + len];
            dst[..a.len()].copy_from_slice(a);
            dst[a.len()..a.len() + b.len()].copy_from_slice(b);
            dst[a.len() + b.len()..].copy_from_slice(c);
            // Both fit in `u32`: `at + len <= STR_KEY_ARENA`.
            self.entries[self.count] = (value, at as u32, len as u32);
            self.used = at + len;
        } else if self.count == 0 {
            self.spill.clear();
            self.spill.extend_from_slice(a);
            self.spill.extend_from_slice(b);
            self.spill.extend_from_slice(c);
            self.entries[0] = (value, SPILL, 0);
        } else {
            return false;
        }
        self.count += 1;
        true
    }

    /// Entry `i`'s key and value.
    #[inline]
    fn entry(&self, i: usize) -> (&[u8], u64) {
        let (v, off, len) = self.entries[i];
        if off == SPILL {
            (&self.spill, v)
        } else {
            (&self.keys[off as usize..(off + len) as usize], v)
        }
    }
}

/// The entries one sub-map drain collected for the string batch cursor
/// (#1143), and why it stopped.
pub(crate) struct SubDrain<'b> {
    /// `(chunk, value)` pairs in ascending chunk order; `count` are written.
    pub(crate) buf: &'b mut [(u64, u64)],
    /// Pairs written.
    pub(crate) count: usize,
    /// Chunks above this are past the scan's end bound.
    pub(crate) end_chunk: u64,
    /// A terminal met a chunk above `end_chunk`. Every later chunk is above
    /// it too, so the branch loops stop walking siblings (#1382).
    pub(crate) past_end: bool,
    /// The pair last pushed ends the drain: the caller descends into it or
    /// stops at it, so nothing after it is needed.
    pub(crate) stopped: bool,
    /// The drain returned after a branch child yielded (one sibling
    /// descent, §33.10.2), so entries of the sub-map may follow its last
    /// pair.
    pub(crate) cut: bool,
}

impl SubDrain<'_> {
    /// True when no more pairs are taken.
    #[inline(always)]
    fn full(&self) -> bool {
        self.stopped || self.count == self.buf.len()
    }

    /// Pushes `(k, v)` unless it is past the end bound or the buffer is full,
    /// and returns whether the drain goes on.
    #[inline(always)]
    fn take(&mut self, k: u64, v: u64, stop: &impl Fn(u64, u64) -> bool) -> bool {
        if k > self.end_chunk {
            self.past_end = true;
            return false;
        }
        if self.count == self.buf.len() {
            return false;
        }
        self.buf[self.count] = (k, v);
        self.count += 1;
        if stop(k, v) {
            self.stopped = true;
            return false;
        }
        true
    }

    /// After a child of a branch was drained from `before` pairs: whether
    /// the branch stops walking its siblings.
    #[inline(always)]
    fn ends_branch(&mut self, before: usize, child: &Edge) -> bool {
        if self.count > before {
            if self.full() || self.past_end {
                return true;
            }
            // A child that yielded ends the walk when it is a branch, which
            // keeps the read set to one sibling descent (§33.10.2).
            self.cut = is_branch_edge(child);
            return self.cut;
        }
        #[cfg(test)]
        if test_hooks::control(test_hooks::Control::WalkPastEnd) {
            return false;
        }
        self.past_end
    }
}

/// True if `edge` names a branch rather than a terminal.
#[inline(always)]
fn is_branch_edge(edge: &Edge) -> bool {
    matches!(
        edge.tag(),
        Some(EdgeTag::Structural(
            EdgeType::BranchL3 | EdgeType::BranchL7 | EdgeType::BranchB | EdgeType::BranchU
        ))
    )
}

/// Drains the entries `>= from` of a `StrNode` sub-map into `d`, for the
/// string batch cursor (#1143): the u64 batch drain (`sync_cursor.rs`,
/// `drain_batch_in`) on a map-form sub-map, with the sub-map's cover word as
/// the holder of its top edge and root leaf, and the retained read set of
/// `sync_nav::next_validated`. It stops at the first pair `stop` names, at a chunk
/// above `d.end_chunk`, when the buffer is full, and as soon as a branch
/// child yielded pairs; `scripts/str_cursor_bounds.py` bounds what it
/// retains (`submap_drain_read_set_branches`, 13). The caller validates the
/// set before it dereferences a pointer value, and again at the end.
///
/// # Safety
///
/// As `sync_nav::seek_submap`: `root` was copied after `csnap` was sampled
/// from `cover` and validated against it afterwards, `cover` is the cover
/// word of an EBR-live `StrNode`, and the caller holds the pin for the whole
/// call.
#[inline]
pub(crate) unsafe fn drain_submap<const CAP: usize>(
    root: RootSnapshot,
    from: u64,
    cover: *const u32,
    csnap: u32,
    rs: &mut ReadSet<CAP>,
    d: &mut SubDrain<'_>,
    stop: &impl Fn(u64, u64) -> bool,
) -> Result<(), Retry> {
    match root {
        RootSnapshot::Empty => Ok(()),
        RootSnapshot::Leaf { ptr, pop } => {
            // SAFETY: the root state was validated against the cover
            // (contract); the loads are covered by the caller's validation.
            unsafe { drain_root_leaf(ptr, pop, from, d, stop) };
            Ok(())
        }
        RootSnapshot::Tree { top } => {
            // SAFETY: the top edge copy is validated against the cover.
            unsafe { drain_in(&top, from, 8, 0, Holder::Node(cover, csnap), rs, d, stop) }
        }
    }
}

/// [`drain_submap`] on a root leaf of `pop` sorted keys and their values.
///
/// # Safety
///
/// `(ptr, pop)` was validated against the cover and is EBR-live.
unsafe fn drain_root_leaf(
    ptr: *const u8,
    pop: usize,
    from: u64,
    d: &mut SubDrain<'_>,
    stop: &impl Fn(u64, u64) -> bool,
) {
    let keys = ptr.cast::<u64>();
    // SAFETY: the value area begins at the class-based offset and holds `pop`
    // values.
    let vals = unsafe { ptr.add(crate::map::leaf_values_offset(pop)).cast::<u64>() };
    let (mut lo, mut hi) = (0usize, pop);
    while lo < hi {
        let mid = (lo + hi) / 2;
        // SAFETY: `mid < pop`, in bounds of the live root leaf.
        let k = unsafe { shared_word::load::<true>(keys.add(mid)) };
        if k < from {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    for i in lo..pop {
        // SAFETY: `i < pop`, in bounds of the live root leaf.
        let (k, v) = unsafe {
            (
                shared_word::load::<true>(keys.add(i)),
                shared_word::load::<true>(vals.add(i)),
            )
        };
        if !d.take(k, v, stop) {
            break;
        }
    }
}

/// [`drain_submap`] below `edge`, a copy validated against `holder`; keys
/// are `prefix | suffix-level key`.
///
/// # Safety
///
/// `edge` is a copy validated against `holder`, and the caller holds the pin.
#[allow(clippy::too_many_arguments)]
unsafe fn drain_in<const CAP: usize>(
    edge: &Edge,
    suffix: u64,
    level: u8,
    prefix: u64,
    holder: Holder<'_>,
    rs: &mut ReadSet<CAP>,
    d: &mut SubDrain<'_>,
    stop: &impl Fn(u64, u64) -> bool,
) -> Result<(), Retry> {
    use core::cmp::Ordering::{Equal, Greater, Less};
    let Some(tag) = edge.tag() else {
        return Err(Retry);
    };
    match tag {
        EdgeTag::Structural(EdgeType::Null) => Ok(()),

        EdgeTag::Immed(im) => {
            if im.key_bytes() != level {
                return Err(Retry);
            }
            let keys = crate::mutate::immed_map_keys(edge, im);
            for (slot, &k) in keys.iter().enumerate() {
                if k < suffix {
                    continue;
                }
                // SAFETY: validated immediate edge copy; `slot` is below its
                // key count.
                let v = unsafe { immed_value(edge, im, slot) };
                if !d.take(prefix | k, v, stop) {
                    break;
                }
            }
            Ok(())
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
                Greater => return Ok(()),
            };
            let pop = edge.pop0(kb) as usize + 1;
            let base = edge.node_ptr();
            let keys = base.wrapping_add(leaf::map_keys_offset(pop));
            // SAFETY: a validated edge copy names a live leaf of `pop` keys of
            // `kb` bytes; the result is covered by the caller's validation.
            let slot = unsafe { leaf::shared_keys::lower_bound(keys, pop, kb as usize, low) };
            let leaf_prefix = prefix | (dv << shift);
            for i in slot..pop {
                // SAFETY: `slot <= i < pop`; map leaves hold `pop` values at
                // the base.
                let (k, v) = unsafe {
                    (
                        leaf::shared_keys::read(keys, i, kb as usize, pop),
                        shared_word::load::<true>(base.cast::<u64>().add(i)),
                    )
                };
                if !d.take(leaf_prefix | k, v, stop) {
                    break;
                }
            }
            Ok(())
        }

        EdgeTag::Structural(EdgeType::LeafB1) => {
            let (ord, dv, shift) = skip_cmp(edge, 1, level, suffix);
            let mut from = match ord {
                Less => 0,
                Equal => key_low(suffix, 1) as u8,
                Greater => return Ok(()),
            };
            let leaf_prefix = prefix | (dv << shift);
            let node = edge.node_ptr().cast::<LeafBitmapL>();
            loop {
                // SAFETY: a validated edge copy names a live map-form bitmap
                // leaf; its bitmap words are loaded atomically and covered
                // by the holder.
                let found = unsafe {
                    crate::bits::shared_bitmap::next_set::<true>(&raw const (*node).bitmap, from)
                };
                let Some(b) = found else {
                    break;
                };
                // SAFETY: as above; the value array is indexed only after
                // `holder.ok()` below.
                let (rank, vals) = unsafe {
                    (
                        crate::bits::shared_bitmap::subexpanse_rank::<true>(
                            &raw const (*node).bitmap,
                            b,
                        ) as usize,
                        shared_word::load_ptr::<true, u64>(
                            (&raw const (*node).values)
                                .cast::<*mut u64>()
                                .add((b >> 5) as usize),
                        ),
                    )
                };
                if vals.is_null() {
                    return Err(Retry);
                }
                // The bitmap and the value array are written separately: a
                // rank from a new bitmap against an old array reads out of
                // bounds, so the pair is validated before the index.
                if !holder.ok() {
                    return Err(Retry);
                }
                // SAFETY: a consistent bitmap and value array hold `rank + 1`
                // values.
                let v = unsafe { shared_word::load::<true>(vals.add(rank)) };
                if !d.take(leaf_prefix | u64::from(b), v, stop) || b == 255 {
                    break;
                }
                from = b + 1;
            }
            Ok(())
        }

        // A map never holds a full expanse.
        EdgeTag::Structural(EdgeType::FullExpanse) => Err(Retry),

        EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
            let is_l3 = matches!(t, EdgeType::BranchL3);
            // SAFETY: validated edge copy → live branch; field projection only.
            let vp = unsafe { linear_branch_version(edge, is_l3) };
            // SAFETY: `vp` is the live branch's version field.
            let nsnap = unsafe { rs.sample(vp)? };
            // The header is loaded after the sample, as in `next_in`.
            // SAFETY: validated edge copy → live branch; loads validated below.
            let (bl, num, digits, edges) = unsafe { linear_branch(edge, is_l3) };
            if !(2..=level).contains(&bl) || num > if is_l3 { 3 } else { 7 } {
                return Err(Retry);
            }
            let (ord, dv, shift) = skip_cmp(edge, bl, level, suffix);
            let suffix = match ord {
                Less => 0,
                Equal => key_low(suffix, bl),
                Greater => return Ok(()),
            };
            let branch_prefix = prefix | (dv << shift);
            let dg = digit(suffix, bl);
            let start = digits[..num].partition_point(|&bd| bd < dg);
            let here = Holder::Node(vp, nsnap);
            for (slot, &bd) in digits.iter().enumerate().take(num).skip(start) {
                let rem = if bd == dg { key_low(suffix, bl - 1) } else { 0 };
                // SAFETY: `slot < num <= capacity`; validated before it is used.
                let child = unsafe { Edge::load_at::<true>(edges.add(slot)) };
                if !here.ok() {
                    return Err(Retry);
                }
                let before = d.count;
                let child_prefix = branch_prefix | (u64::from(bd) << ((bl - 1) * 8));
                // SAFETY: child copy validated against this branch.
                unsafe { drain_in(&child, rem, bl - 1, child_prefix, here, rs, d, stop)? };
                if d.ends_branch(before, &child) {
                    return Ok(());
                }
            }
            Ok(())
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
                Greater => return Ok(()),
            };
            let branch_prefix = prefix | (dv << shift);
            let dg = digit(suffix, bl);
            let here = Holder::Node(vp, nsnap);
            // SAFETY: live node; bitmap loads are validated before use.
            let mut cur = unsafe {
                crate::bits::shared_bitmap::next_set::<true>(&raw const (*node).bitmap, dg)
            };
            while let Some(bd) = cur {
                let rem = if bd == dg { key_low(suffix, bl - 1) } else { 0 };
                // SAFETY: `bd` is set in the bitmap; `here` is checked inside.
                let child = unsafe { branch_b_child(node, bd, here)? };
                let before = d.count;
                let child_prefix = branch_prefix | (u64::from(bd) << ((bl - 1) * 8));
                // SAFETY: child copy validated against this branch.
                unsafe { drain_in(&child, rem, bl - 1, child_prefix, here, rs, d, stop)? };
                if d.ends_branch(before, &child) {
                    return Ok(());
                }
                cur = if bd == 255 {
                    None
                } else {
                    // SAFETY: live bitmap node.
                    unsafe {
                        crate::bits::shared_bitmap::next_set::<true>(
                            &raw const (*node).bitmap,
                            bd + 1,
                        )
                    }
                };
            }
            Ok(())
        }

        EdgeTag::Structural(EdgeType::BranchU) => {
            let node = edge.node_ptr().cast::<BranchU>();
            // SAFETY: validated edge copy → live BranchU; field projection.
            let vp = unsafe { &raw const (*node).version };
            // SAFETY: live version field.
            let nsnap = unsafe { rs.sample(vp)? };
            let here = Holder::Node(vp, nsnap);
            let dg = digit(suffix, level);
            for bd in dg..=255u8 {
                // Every child edge is loaded after the sample, as in `next_in`.
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
                let rem = if bd == dg {
                    key_low(suffix, level - 1)
                } else {
                    0
                };
                let before = d.count;
                let child_prefix = prefix | (u64::from(bd) << ((level - 1) * 8));
                // SAFETY: child copy validated against this branch.
                unsafe { drain_in(&child, rem, level - 1, child_prefix, here, rs, d, stop)? };
                if d.ends_branch(before, &child) {
                    return Ok(());
                }
            }
            Ok(())
        }
    }
}

/// A forward batch cursor over a [`SyncExpanseStrMap`], from
/// [`StrReader::cursor`](crate::sync::StrReader::cursor) and its range and
/// prefix forms.
///
/// The returned key borrows the cursor's buffer and is valid until the next
/// call; copy it if it must outlive that. Unlike the u64 cursor this is not an
/// [`Iterator`]: an item borrows the cursor, which `Iterator` cannot express,
/// and owning each key would allocate per entry.
///
/// The cursor borrows a [`Reader`], whose single epoch slot is not reentrant,
/// so a cursor is neither `Send` nor `Sync`.
pub struct SyncStrMapCursor<'m, 'r> {
    map: &'m SyncExpanseStrMap,
    reader: &'r Reader,
    lo: Vec<u8>,
    hi: Option<Vec<u8>>,
    batch: Box<StrBatch>,
    pos: usize,
    /// Nothing within the bound follows the current batch.
    last: bool,
    exhausted: bool,
    /// The walk's buffers, kept across batches.
    search: Vec<u8>,
    prefix: Vec<u8>,
    restart: Vec<u8>,
}

impl<'m, 'r> SyncStrMapCursor<'m, 'r> {
    /// A cursor over every entry of `map`.
    ///
    /// # Panics
    ///
    /// Panics if `reader` was registered on a different collector than
    /// `map`'s, before any node is read (#1194).
    #[must_use]
    pub fn new(map: &'m SyncExpanseStrMap, reader: &'r Reader) -> Self {
        Self::build(map, reader, Vec::new(), None)
    }

    /// A cursor over the entries with keys in `[lo, hi)`: `lo` inclusive,
    /// `hi` exclusive. Empty when `lo >= hi`.
    ///
    /// # Panics
    ///
    /// As [`Self::new`].
    #[must_use]
    pub fn range(
        map: &'m SyncExpanseStrMap,
        reader: &'r Reader,
        lo: &NulFreeStr,
        hi: &NulFreeStr,
    ) -> Self {
        Self::build(
            map,
            reader,
            lo.as_bytes().to_vec(),
            Some(hi.as_bytes().to_vec()),
        )
    }

    /// A cursor over the entries whose keys start with `prefix`: the range
    /// from `prefix` to the first key past every extension of it. The empty
    /// prefix scans the whole map.
    ///
    /// # Panics
    ///
    /// As [`Self::new`].
    #[must_use]
    pub fn prefix(map: &'m SyncExpanseStrMap, reader: &'r Reader, prefix: &NulFreeStr) -> Self {
        let lo = prefix.as_bytes().to_vec();
        Self::build(map, reader, lo.clone(), prefix_end(lo))
    }

    fn build(
        map: &'m SyncExpanseStrMap,
        reader: &'r Reader,
        lo: Vec<u8>,
        hi: Option<Vec<u8>>,
    ) -> Self {
        // The map's collector is fixed at construction and the cursor holds
        // both borrows, so this one check covers every batch it pins for.
        let reader = map.registered(reader);
        let exhausted = hi.as_ref().is_some_and(|h| lo >= *h);
        Self {
            map,
            reader,
            lo,
            hi,
            batch: StrBatch::new(),
            pos: 0,
            last: false,
            exhausted,
            search: Vec::new(),
            prefix: Vec::new(),
            restart: Vec::new(),
        }
    }

    /// The current entry, without advancing; `None` once exhausted.
    #[must_use]
    pub fn current(&mut self) -> Option<(&[u8], u64)> {
        if self.pos >= self.batch.count {
            if self.exhausted || (self.last && self.batch.count > 0) {
                self.exhausted = true;
                return None;
            }
            self.set_floor();
            if !self.refill() {
                return None;
            }
        }
        Some(self.batch.entry(self.pos))
    }

    /// Returns the current entry and advances past it; `None` once
    /// exhausted. The key borrows the cursor until the next call.
    #[allow(clippy::should_implement_trait)] // lending: the key borrows `self`
    pub fn next(&mut self) -> Option<(&[u8], u64)> {
        self.current()?;
        self.pos += 1;
        Some(self.batch.entry(self.pos - 1))
    }

    /// Advances to and returns the entry with the smallest key `>= target`
    /// that is `>=` the cursor's position; `None` once exhausted.
    ///
    /// The cursor never moves backwards: a `target` below the position, or
    /// below the range's start, returns the current entry. A `target` at or
    /// above the range's end exhausts the cursor.
    pub fn advance_to(&mut self, target: &NulFreeStr) -> Option<(&[u8], u64)> {
        let t = target.as_bytes();
        if self.hi.as_ref().is_some_and(|h| t >= h.as_slice()) {
            self.pos = self.batch.count;
            self.exhausted = true;
            return None;
        }
        if self.pos < self.batch.count {
            if t <= self.batch.entry(self.pos).0 {
                return Some(self.batch.entry(self.pos));
            }
            let (mut lo, mut hi) = (self.pos, self.batch.count);
            while lo < hi {
                let mid = (lo + hi) / 2;
                if self.batch.entry(mid).0 < t {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            }
            if lo < self.batch.count {
                self.pos = lo;
                return Some(self.batch.entry(lo));
            }
            // `t` is past every buffered key, so it is past the position.
            if self.last {
                self.pos = self.batch.count;
                self.exhausted = true;
                return None;
            }
            self.search.clear();
            self.search.extend_from_slice(t);
        } else {
            if self.exhausted || (self.last && self.batch.count > 0) {
                self.exhausted = true;
                return None;
            }
            // The position is the key after the last one returned, or the
            // range's start. A refill below it would rewind.
            self.set_floor();
            if t > self.search.as_slice() {
                self.search.clear();
                self.search.extend_from_slice(t);
            }
        }
        if self.refill() {
            Some(self.batch.entry(0))
        } else {
            None
        }
    }

    /// The upper bound, exclusive; `None` when the scan is unbounded above.
    #[must_use]
    pub fn end_bound(&self) -> Option<&[u8]> {
        self.hi.as_deref()
    }

    /// Writes the position into `search`: the key after the last one in the
    /// consumed batch (that key followed by `0x01`, its immediate successor
    /// among NUL-free strings), or the range's start before the first batch.
    fn set_floor(&mut self) {
        self.search.clear();
        if self.batch.count > 0 {
            let (k, _) = self.batch.entry(self.batch.count - 1);
            self.search.extend_from_slice(k);
            self.search.push(1);
        } else {
            self.search.extend_from_slice(&self.lo);
        }
    }

    /// Refills the batch from `search`. Returns `true` when it holds an
    /// entry; exhausts the cursor otherwise.
    fn refill(&mut self) -> bool {
        self.last = self.map.fill_scan_batch(
            self.reader,
            &mut self.search,
            self.hi.as_deref(),
            &mut self.batch,
            &mut self.prefix,
            &mut self.restart,
        );
        self.pos = 0;
        if self.batch.count == 0 {
            self.exhausted = true;
            return false;
        }
        true
    }
}

/// The first key past every extension of `prefix`: its trailing `0xFF` bytes
/// dropped and its last byte incremented; `None` when there is none (an empty
/// or all-`0xFF` prefix), which leaves the scan unbounded above.
fn prefix_end(mut p: Vec<u8>) -> Option<Vec<u8>> {
    while p.last() == Some(&0xFF) {
        p.pop();
    }
    let b = p.last_mut()?;
    *b += 1;
    Some(p)
}

/// The outcome classes and switches the cursor's tests drive (#1143), on the
/// test thread only. Absent from every other build.
#[cfg(test)]
pub(crate) mod test_hooks {
    use core_alloc::boxed::Box;
    use std::cell::{Cell, RefCell};

    /// A point in the walk a test can arm a write at.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub(crate) enum Site {
        /// A level's sub-map root state was copied, before the cover check.
        LevelEntry,
        /// A level's drain returned, before the holder check.
        Drained,
        /// Before the final validation of an attempt.
        Final,
    }

    /// A negative-control switch: each disables one check.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub(crate) enum Control {
        /// The cover check after a level's root state is copied.
        SkipCoverCheck,
        /// The holder check before a drained pointer is dereferenced.
        SkipHolderCheck,
        /// The final validation, of a batch and of a restart alike.
        SkipFinalValidation,
        /// The drain's branch loops keep walking siblings past the end bound.
        WalkPastEnd,
        /// Drains after an attempt's first drained level run without
        /// checking the read set's room (§33.11.2 item 4).
        SkipAdmission,
        /// Every drain after an attempt's first drained level is refused,
        /// as when the read set has no room left.
        RefuseAdmission,
    }

    /// Per-thread event counts, indexed by discriminant.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub(crate) enum Count {
        /// Optimistic attempts started.
        Attempts,
        /// Attempts that ended with a validated restart.
        Restarts,
        /// Climbs from a level that ran out to its parent, within an attempt.
        Climbs,
        /// Batches filled under the writer lock.
        Fallbacks,
        /// Fallbacks taken at once on a full read set that validated.
        Overflows,
        /// Holder checks that failed and retried.
        HolderRetries,
        /// Dereferences through a holder that no longer validated.
        UnvalidatedDerefs,
        /// Versions retained, summed over every attempt.
        Samples,
    }

    const COUNTS: usize = 8;

    /// The write armed at a site.
    type Armed = Option<(Site, Box<dyn FnOnce()>)>;

    thread_local! {
        static CONTROLS: Cell<u8> = const { Cell::new(0) };
        static COUNTERS: Cell<[u64; COUNTS]> = const { Cell::new([0; COUNTS]) };
        static ARMED: RefCell<Armed> = const { RefCell::new(None) };
    }

    pub(crate) fn set_control(c: Control, on: bool) {
        CONTROLS.with(|s| {
            let bit = 1u8 << c as u8;
            s.set(if on { s.get() | bit } else { s.get() & !bit });
        });
    }

    pub(crate) fn control(c: Control) -> bool {
        CONTROLS.with(|s| s.get() & (1u8 << c as u8) != 0)
    }

    pub(crate) fn count(c: Count) {
        add(c, 1);
    }

    pub(crate) fn add(c: Count, n: u64) {
        COUNTERS.with(|s| {
            let mut a = s.get();
            a[c as usize] += n;
            s.set(a);
        });
    }

    /// This thread's counts since the last call.
    pub(crate) fn take_counts() -> [u64; COUNTS] {
        COUNTERS.with(|s| s.replace([0; COUNTS]))
    }

    /// Runs `write` once, the first time the walk reaches `site`.
    pub(crate) fn arm_site(site: Site, write: Box<dyn FnOnce()>) {
        ARMED.with(|a| *a.borrow_mut() = Some((site, write)));
    }

    pub(crate) fn at(site: Site) {
        let f = ARMED.with(|a| {
            let mut a = a.borrow_mut();
            match a.as_ref() {
                Some((s, _)) if *s == site => a.take().map(|(_, f)| f),
                _ => None,
            }
        });
        if let Some(f) = f {
            f();
        }
    }
}
