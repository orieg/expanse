//! Validated batch cursor on concurrent map readers (#1142).
//!
//! A [`SyncMapCursor`] traverses a [`SyncExpanseMap`] in ascending order
//! by copying up to [`BATCH_CAP`] entries `(key, u64)` at a time into an
//! internal stack-resident buffer under an optimistic read bracket.
//!
//! # Soundness and Concurrency Invariants
//!
//! In Expanse's digital trie:
//! - Terminal leaves (`Leaf1`..`Leaf7`, `LeafB1`, immediates) carry no version
//!   word of their own (`docs/ARCHITECTURE.md` §4.1; `sync_nav.rs:26-29`).
//!   Their payloads are covered by their direct parent branch's version word
//!   (`Holder::Node(vp, snap)`), which brackets every in-place store
//!   (`mutate_map.rs:1339, 1485, 1500, 1684`).
//! - Draining a terminal leaf under its covering parent validates strictly 1
//!   branch node (`terminal_drain_read_set_branches() = 1`), paying 2 version
//!   loads and 1 acquire fence (`scripts/batch_cursor_bounds.py`).
//! - Sibling-step resume: within a batch (while pinned), the cursor steps to
//!   the next sibling under the direct parent's retained version word. Under
//!   lazy census rollup, child mutations do not move parent versions; however,
//!   filling an empty child slot in a branch rewrites the branch's digit and
//!   edge arrays inside that branch's own bracket (`mutate_map.rs:1868, 1904, 2000, 2140`).
//!   Therefore, the parent branch's version word covers the absence of any
//!   skipped empty sibling.
//! - Any entered non-empty child branch is retained in the read set
//!   (`sync_nav.rs:407, 442, 490`), bounded by $\le 13$ branches
//!   (`scripts/olc_bounds.py`), within [`READ_SET_CAP`] = 16, and re-validated
//!   at the end.
//! - Across-batch resume: across batches, the cursor unpins and re-descends from
//!   the root by key (`next_at_or_after`). Node pointers are never retained
//!   across an unpin, ensuring no use-after-free under epoch-based reclamation.
//! - User iterator yields by value `(u64, u64)` and never holds an epoch pin
//!   across user code or callbacks.

use crate::bits::shared_word;
use crate::leaf;
use crate::map::ExpanseMap;
use crate::mutate::key_low;
use crate::node::{BranchB, BranchL3, BranchL7, BranchU, Edge, LeafBitmapL};
use crate::occ::{Reader, SeqVersion, node_sample, node_validate, version_cell};
use crate::sync::{Retry, RootSnapshot, SyncExpanseMap};
use crate::types::{EdgeTag, EdgeType, digit};
use core::mem::MaybeUninit;

/// Maximum entries copied into a cursor batch.
///
/// In Expanse's digital trie, the widest terminal is a level-1 bitmap leaf
/// (`LeafB1`), which represents an 8-bit byte expanse and holds at most
/// 2^8 = 256 entries (`scripts/batch_cursor_bounds.py`). At 16 bytes per entry,
/// a 4 KiB buffer accommodates any terminal in the trie with 0 mid-leaf splits.
pub const BATCH_CAP: usize = 256;

/// Retained branch versions capacity during descent/sibling walk.
const READ_SET_CAP: usize = 16;

/// Every branch version sampled during batch descent, kept until final validation.
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

    /// Validates every retained version after the batch's last load.
    ///
    /// # Safety
    ///
    /// The caller still holds the pin under which every entry was sampled.
    #[inline(always)]
    unsafe fn validate_all(&self) -> bool {
        #[cfg(test)]
        if test_hooks::skips_branch_validation() || test_hooks::skips_final_validation() {
            return true;
        }
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
        #[cfg(test)]
        if test_hooks::skips_parent_validation() {
            return true;
        }
        match self {
            Holder::Tree(v, s) => v.validate(s),
            // SAFETY: the holding node is EBR-live for the reader's pin.
            Holder::Node(p, s) => node_validate(unsafe { version_cell(p) }, s),
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

/// The child edge for set digit `bd` of a `BranchB`, validated against the
/// branch before its subarray is indexed and again after the load.
///
/// # Safety
///
/// `node` is an EBR-live `BranchB` whose version `here` names.
#[inline(always)]
unsafe fn branch_b_child(node: *const BranchB, bd: u8, here: Holder<'_>) -> Result<Edge, Retry> {
    // SAFETY: `node` is an EBR-live BranchB and bitmap loads are atomic.
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
    if !here.ok() {
        return Err(Retry);
    }
    // SAFETY: `rank` is in bounds of the subarray validated by `here.ok()`.
    let child = unsafe { Edge::load_at::<true>(sub.add(rank)) };
    if !here.ok() {
        return Err(Retry);
    }
    Ok(child)
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
        // SAFETY: multi-key immediate value array has length equal to key count and slot is within it.
        unsafe { crate::bits::shared_word::load::<true>(edge.node_ptr().cast::<u64>().add(slot)) }
    }
}

/// True if `edge` names a branch rather than a terminal leaf.
#[inline(always)]
fn is_branch_edge(edge: &Edge) -> bool {
    matches!(
        edge.tag(),
        Some(EdgeTag::Structural(
            EdgeType::BranchL3 | EdgeType::BranchL7 | EdgeType::BranchB | EdgeType::BranchU
        ))
    )
}

/// A child subtree answered nothing; the search moves to its sibling.
#[inline(always)]
fn backtrack(rs: &mut ReadSet, mark: usize) {
    #[cfg(test)]
    {
        if test_hooks::skips_branch_validation() {
            rs.len = mark;
        }
    }
    #[cfg(not(test))]
    let _ = (rs, mark);
}

/// A forward batch cursor over a [`SyncExpanseMap`] yielding `(u64, u64)` entries.
///
/// Cursors pin the epoch only while copying a batch of entries into a cursor-owned
/// 4 KiB buffer. Across batches and across user iterator calls, no epoch pin is held
/// and no raw node pointers are retained.
pub struct SyncMapCursor<'m, 'r> {
    map: &'m SyncExpanseMap,
    reader: &'r Reader,
    start_key: u64,
    end_key: u64,
    last_emitted: Option<u64>,
    buf: [(u64, u64); BATCH_CAP],
    pos: usize,
    len: usize,
    exhausted: bool,
}

impl<'m, 'r> SyncMapCursor<'m, 'r> {
    /// Creates a forward batch cursor scanning all entries in `map`.
    #[must_use]
    pub fn new(map: &'m SyncExpanseMap, reader: &'r Reader) -> Self {
        Self::range(map, reader, 0, u64::MAX)
    }

    /// Creates a forward batch cursor scanning entries with keys in `start..=end`.
    ///
    /// The range is inclusive of both bounds (`start..=end`), matching future
    /// single-threaded `MapCursor RangeBounds` conventions.
    #[must_use]
    pub fn range(map: &'m SyncExpanseMap, reader: &'r Reader, start: u64, end: u64) -> Self {
        let exhausted = start > end;
        Self {
            map,
            reader,
            start_key: start,
            end_key: end,
            last_emitted: None,
            buf: [(0, 0); BATCH_CAP],
            pos: 0,
            len: 0,
            exhausted,
        }
    }

    /// Creates a forward batch cursor scanning entries matching `bounds`.
    #[must_use]
    pub fn range_bounds<R: core::ops::RangeBounds<u64>>(
        map: &'m SyncExpanseMap,
        reader: &'r Reader,
        bounds: R,
    ) -> Self {
        let start = match bounds.start_bound() {
            core::ops::Bound::Included(&s) => s,
            core::ops::Bound::Excluded(&s) => match s.checked_add(1) {
                Some(next) => next,
                None => {
                    return Self {
                        map,
                        reader,
                        start_key: u64::MAX,
                        end_key: 0,
                        last_emitted: None,
                        buf: [(0, 0); BATCH_CAP],
                        pos: 0,
                        len: 0,
                        exhausted: true,
                    };
                }
            },
            core::ops::Bound::Unbounded => 0,
        };
        let end = match bounds.end_bound() {
            core::ops::Bound::Included(&e) => e,
            core::ops::Bound::Excluded(&e) => match e.checked_sub(1) {
                Some(prev) => prev,
                None => {
                    return Self {
                        map,
                        reader,
                        start_key: 1,
                        end_key: 0,
                        last_emitted: None,
                        buf: [(0, 0); BATCH_CAP],
                        pos: 0,
                        len: 0,
                        exhausted: true,
                    };
                }
            },
            core::ops::Bound::Unbounded => u64::MAX,
        };
        Self::range(map, reader, start, end)
    }

    /// Peeks the current entry at the cursor's position without advancing.
    #[must_use]
    pub fn current(&mut self) -> Option<(u64, u64)> {
        if self.pos < self.len {
            Some(self.buf[self.pos])
        } else if self.exhausted {
            None
        } else {
            let next_k = match self.last_emitted {
                None => self.start_key,
                Some(k) => k.checked_add(1)?,
            };
            if self.refill(next_k) {
                Some(self.buf[self.pos])
            } else {
                None
            }
        }
    }

    /// Advances to and returns the entry with the smallest key `>= target`
    /// that is `>=` the cursor's current position; `None` once exhausted.
    pub fn advance_to(&mut self, target: u64) -> Option<(u64, u64)> {
        if target > self.end_key {
            self.exhausted = true;
            return None;
        }
        // First check current entry (if buffer has one):
        if self.pos < self.len {
            let cur_k = self.buf[self.pos].0;
            if target <= cur_k {
                return Some(self.buf[self.pos]);
            }
            let rem = &self.buf[self.pos..self.len];
            let offset = rem.partition_point(|&(k, _)| k < target);
            if self.pos + offset < self.len {
                self.pos += offset;
                return Some(self.buf[self.pos]);
            }
        } else if self.exhausted {
            return None;
        }

        // Target is past current buffer: refill starting at target.
        if self.refill(target) {
            Some(self.buf[0])
        } else {
            None
        }
    }

    /// Refills the batch starting from `search_key`. Returns `true` if at least
    /// one entry was loaded into `self.buf`.
    fn refill(&mut self, search_key: u64) -> bool {
        if search_key > self.end_key {
            self.exhausted = true;
            return false;
        }

        let map = self.map;
        let reader = self.reader;
        let end_key = self.end_key;
        let buf_ptr = self.buf.as_mut_ptr();

        let count = map.optimistic_read(
            reader,
            // SAFETY: `buf_ptr` points to `self.buf` held exclusively by `&mut self`
            // and `try_fill_buf` is called under the epoch pin held by `optimistic_read`.
            |root, ver, snap| unsafe {
                Self::try_fill_buf(
                    &mut *buf_ptr.cast::<[(u64, u64); BATCH_CAP]>(),
                    end_key,
                    root,
                    ver,
                    snap,
                    search_key,
                )
            },
            // SAFETY: `buf_ptr` points to `self.buf` held exclusively by `&mut self`
            // and writer mutex is held during this fallback.
            |tree| unsafe {
                Self::fill_buf_locked(
                    &mut *buf_ptr.cast::<[(u64, u64); BATCH_CAP]>(),
                    end_key,
                    tree,
                    search_key,
                )
            },
        );

        self.pos = 0;
        self.len = count;
        if count == 0 {
            self.exhausted = true;
            false
        } else {
            true
        }
    }

    /// Fills the batch buffer under the writer mutex fallback.
    fn fill_buf_locked(
        buf: &mut [(u64, u64); BATCH_CAP],
        end_key: u64,
        tree: &ExpanseMap,
        search_key: u64,
    ) -> usize {
        let mut count = 0;
        let mut cur = tree.next_at_or_after(search_key);
        while let Some((k, v)) = cur {
            if k > end_key || count == BATCH_CAP {
                break;
            }
            buf[count] = (k, v);
            count += 1;
            cur = k
                .checked_add(1)
                .and_then(|next| tree.next_at_or_after(next));
        }
        count
    }
}

/// Batch buffer drain accumulator.
struct BatchDrain<'b> {
    buf: &'b mut [(u64, u64); BATCH_CAP],
    end_key: u64,
    count: usize,
}

impl BatchDrain<'_> {
    #[inline(always)]
    fn push(&mut self, k: u64, v: u64) {
        self.buf[self.count] = (k, v);
        self.count += 1;
    }
}

impl SyncMapCursor<'_, '_> {
    /// Optimistic batch drain attempt.
    ///
    /// # Safety
    ///
    /// The caller holds an epoch pin for the whole call, and `snap` was
    /// sampled from `ver` before `root` was copied.
    unsafe fn try_fill_buf(
        buf: &mut [(u64, u64); BATCH_CAP],
        end_key: u64,
        root: RootSnapshot,
        ver: &SeqVersion,
        snap: u64,
        search_key: u64,
    ) -> Result<usize, Retry> {
        if !ver.validate(snap) {
            return Err(Retry);
        }
        match root {
            RootSnapshot::Empty => Ok(0),
            RootSnapshot::Leaf { ptr, pop } => {
                let mut drain = BatchDrain {
                    buf,
                    end_key,
                    count: 0,
                };
                // SAFETY: `ptr` points to a validated, EBR-live root leaf of `pop` keys.
                unsafe {
                    Self::drain_root_leaf(&mut drain, ptr, pop, search_key);
                }
                #[cfg(test)]
                test_hooks::at(test_hooks::Site::CursorFinal);
                if !ver.validate(snap) {
                    return Err(Retry);
                }
                Ok(drain.count)
            }
            RootSnapshot::Tree { top } => {
                let mut rs = ReadSet::new();
                let mut drain = BatchDrain {
                    buf,
                    end_key,
                    count: 0,
                };
                // SAFETY: `top` is validated against root version and caller holds the epoch pin.
                unsafe {
                    Self::drain_batch_in(
                        &mut drain,
                        &top,
                        search_key,
                        8,
                        0,
                        Holder::Tree(ver, snap),
                        &mut rs,
                    )?;
                }
                #[cfg(test)]
                test_hooks::at(test_hooks::Site::CursorFinal);
                // SAFETY: caller holds the epoch pin under which all read-set entries were sampled.
                let all_valid = unsafe { rs.validate_all() };
                let tree_valid = ver.validate(snap);
                #[cfg(test)]
                let skip_final = test_hooks::skips_final_validation();
                #[cfg(not(test))]
                let skip_final = false;

                if !skip_final && (!all_valid || !tree_valid) {
                    return Err(Retry);
                }
                Ok(drain.count)
            }
        }
    }

    /// Drains entries from a flat root leaf into `drain`.
    unsafe fn drain_root_leaf(
        drain: &mut BatchDrain<'_>,
        ptr: *const u8,
        pop: usize,
        search_key: u64,
    ) {
        let keys = ptr.cast::<u64>();
        // SAFETY: value area begins at class offset and holds `pop` u64 values.
        let vals = unsafe { ptr.add(crate::map::leaf_values_offset(pop)).cast::<u64>() };
        let mut lo = 0;
        let mut hi = pop;
        while lo < hi {
            let mid = (lo + hi) / 2;
            // SAFETY: `mid < pop`, in bounds of live root leaf.
            let k = unsafe { shared_word::load::<true>(keys.add(mid)) };
            if k < search_key {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for i in lo..pop {
            // SAFETY: `i < pop`, in bounds of live root leaf keys.
            let k = unsafe { shared_word::load::<true>(keys.add(i)) };
            if k > drain.end_key || drain.count == BATCH_CAP {
                break;
            }
            // SAFETY: `i < pop`, in bounds of live root leaf values.
            let v = unsafe { shared_word::load::<true>(vals.add(i)) };
            drain.push(k, v);
        }
    }

    /// Recursive descent and sibling collection of terminal payloads.
    unsafe fn drain_batch_in(
        drain: &mut BatchDrain<'_>,
        edge: &Edge,
        suffix: u64,
        level: u8,
        prefix: u64,
        holder: Holder<'_>,
        rs: &mut ReadSet,
    ) -> Result<(), Retry> {
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
                for slot in 0..keys.len() {
                    let k = keys[slot];
                    if k < suffix {
                        continue;
                    }
                    let full_k = prefix | k;
                    if full_k > drain.end_key || drain.count == BATCH_CAP {
                        break;
                    }
                    // SAFETY: validated immediate edge copy and slot is within key count.
                    let v = unsafe { immed_value(edge, im, slot) };
                    drain.push(full_k, v);
                }
                #[cfg(test)]
                test_hooks::at(test_hooks::Site::TerminalDrained);
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
                    core::cmp::Ordering::Less => 0,
                    core::cmp::Ordering::Equal => key_low(suffix, kb),
                    core::cmp::Ordering::Greater => return Ok(()),
                };
                let pop = edge.pop0(kb) as usize + 1;
                let base = edge.node_ptr();
                let keys = base.wrapping_add(leaf::map_keys_offset(pop));
                // SAFETY: a validated edge copy names a live leaf of `pop` keys of `kb` bytes.
                let slot = unsafe { leaf::shared_keys::lower_bound(keys, pop, kb as usize, low) };
                if slot == pop {
                    return Ok(());
                }
                let leaf_prefix = prefix | (dv << shift);
                for i in slot..pop {
                    // SAFETY: `slot <= i < pop`.
                    let k = unsafe { leaf::shared_keys::read(keys, i, kb as usize, pop) };
                    let full_k = leaf_prefix | k;
                    if full_k > drain.end_key || drain.count == BATCH_CAP {
                        break;
                    }
                    // SAFETY: map leaves hold `pop` values at the base.
                    let v = unsafe {
                        crate::bits::shared_word::load::<true>(base.cast::<u64>().add(i))
                    };
                    drain.push(full_k, v);
                }
                #[cfg(test)]
                test_hooks::at(test_hooks::Site::TerminalDrained);
                Ok(())
            }

            EdgeTag::Structural(EdgeType::LeafB1) => {
                let (ord, dv, shift) = skip_cmp(edge, 1, level, suffix);
                let from = match ord {
                    core::cmp::Ordering::Less => 0,
                    core::cmp::Ordering::Equal => key_low(suffix, 1) as u8,
                    core::cmp::Ordering::Greater => return Ok(()),
                };
                let node = edge.node_ptr().cast::<LeafBitmapL>();
                let leaf_prefix = prefix | (dv << shift);
                // SAFETY: live bitmap leaf; bitmap loads are validated before use.
                let mut cur = unsafe {
                    crate::bits::shared_bitmap::next_set::<true>(&raw const (*node).bitmap, from)
                };
                while let Some(d) = cur {
                    let full_k = leaf_prefix | u64::from(d);
                    if full_k > drain.end_key || drain.count == BATCH_CAP {
                        break;
                    }
                    // SAFETY: live node; rank calculation within bitmap.
                    let rank = unsafe {
                        crate::bits::shared_bitmap::subexpanse_rank::<true>(
                            &raw const (*node).bitmap,
                            d,
                        ) as usize
                    };
                    // SAFETY: live node; subarray pointers are atomically loaded.
                    let vals = unsafe {
                        crate::bits::shared_word::load_ptr::<true, u64>(
                            (&raw const (*node).values)
                                .cast::<*mut u64>()
                                .add((d >> 5) as usize),
                        )
                    };
                    if vals.is_null() {
                        return Err(Retry);
                    }
                    if !holder.ok() {
                        return Err(Retry);
                    }
                    // SAFETY: `rank` is in bounds of the subarray validated by `holder.ok()`.
                    let v = unsafe { crate::bits::shared_word::load::<true>(vals.add(rank)) };
                    drain.push(full_k, v);
                    cur = if d == 255 {
                        None
                    } else {
                        // SAFETY: live bitmap leaf.
                        unsafe {
                            crate::bits::shared_bitmap::next_set::<true>(
                                &raw const (*node).bitmap,
                                d + 1,
                            )
                        }
                    };
                }
                #[cfg(test)]
                test_hooks::at(test_hooks::Site::TerminalDrained);
                Ok(())
            }

            EdgeTag::Structural(EdgeType::FullExpanse) => Err(Retry),

            EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
                let is_l3 = matches!(t, EdgeType::BranchL3);
                // SAFETY: validated edge copy names live branch; loads validated below.
                let (vp, bl, num, digits, edges) = unsafe { linear_branch(edge, is_l3) };
                // SAFETY: `vp` is the live branch's version field.
                let nsnap = unsafe { rs.sample(vp)? };
                #[cfg(test)]
                if is_l3 && num == 3 {
                    test_hooks::at(test_hooks::Site::ParentSampled);
                }
                if !(2..=level).contains(&bl) || num > if is_l3 { 3 } else { 7 } {
                    return Err(Retry);
                }
                let (ord, dv, shift) = skip_cmp(edge, bl, level, suffix);
                let branch_prefix = prefix | (dv << shift);
                let suffix = match ord {
                    core::cmp::Ordering::Less => 0,
                    core::cmp::Ordering::Equal => key_low(suffix, bl),
                    core::cmp::Ordering::Greater => return Ok(()),
                };
                let d = digit(suffix, bl);
                let start = digits[..num].partition_point(|&bd| bd < d);
                let here = Holder::Node(vp, nsnap);
                for (slot, &bd) in digits.iter().enumerate().take(num).skip(start) {
                    #[cfg(test)]
                    if slot > 0 && bd > digits[slot - 1] + 1 {
                        test_hooks::at(test_hooks::Site::SkippedSibling);
                    }
                    let rem = if bd == d { key_low(suffix, bl - 1) } else { 0 };
                    // SAFETY: `slot < num <= capacity`; validated before use.
                    let child = unsafe { Edge::load_at::<true>(edges.add(slot)) };
                    if !here.ok() {
                        return Err(Retry);
                    }
                    let child_prefix = branch_prefix | (u64::from(bd) << ((bl - 1) * 8));
                    let prev_count = drain.count;
                    if prev_count > 0 {
                        #[cfg(test)]
                        test_hooks::at(test_hooks::Site::BeforeNextLeaf);
                    }
                    let mark = rs.len;
                    // SAFETY: child copy validated against this branch.
                    unsafe {
                        Self::drain_batch_in(
                            drain,
                            &child,
                            rem,
                            bl - 1,
                            child_prefix,
                            here,
                            rs,
                        )?;
                    }
                    if drain.count > prev_count {
                        if drain.count >= BATCH_CAP || drain.buf[drain.count - 1].0 >= drain.end_key {
                            return Ok(());
                        }
                        if is_branch_edge(&child) {
                            return Ok(());
                        }
                    } else {
                        #[cfg(test)]
                        test_hooks::at(test_hooks::Site::SkippedSibling);
                        backtrack(rs, mark);
                    }
                }
                Ok(())
            }

            EdgeTag::Structural(EdgeType::BranchB) => {
                let node = edge.node_ptr().cast::<BranchB>();
                // SAFETY: validated edge copy names live BranchB; field projection.
                let vp = unsafe { &raw const (*node).version };
                // SAFETY: live version field.
                let nsnap = unsafe { rs.sample(vp)? };
                #[cfg(test)]
                test_hooks::at(test_hooks::Site::ParentSampled);
                // SAFETY: live node; torn level rejected below.
                let bl = unsafe { (*node).level };
                if !(2..=level).contains(&bl) {
                    return Err(Retry);
                }
                let (ord, dv, shift) = skip_cmp(edge, bl, level, suffix);
                let branch_prefix = prefix | (dv << shift);
                let suffix = match ord {
                    core::cmp::Ordering::Less => 0,
                    core::cmp::Ordering::Equal => key_low(suffix, bl),
                    core::cmp::Ordering::Greater => return Ok(()),
                };
                let d = digit(suffix, bl);
                let here = Holder::Node(vp, nsnap);
                // SAFETY: live node; bitmap loads are validated before use.
                let mut cur = unsafe {
                    crate::bits::shared_bitmap::next_set::<true>(&raw const (*node).bitmap, d)
                };
                while let Some(bd) = cur {
                    let rem = if bd == d { key_low(suffix, bl - 1) } else { 0 };
                    // SAFETY: `bd` was present in bitmap and `here` is checked inside.
                    let child = unsafe { branch_b_child(node, bd, here)? };
                    let child_prefix = branch_prefix | (u64::from(bd) << ((bl - 1) * 8));
                    let prev_count = drain.count;
                    if prev_count > 0 {
                        #[cfg(test)]
                        test_hooks::at(test_hooks::Site::BeforeNextLeaf);
                    }
                    let mark = rs.len;
                    // SAFETY: child copy validated against this branch.
                    unsafe {
                        Self::drain_batch_in(
                            drain,
                            &child,
                            rem,
                            bl - 1,
                            child_prefix,
                            here,
                            rs,
                        )?;
                    }
                    if drain.count > prev_count {
                        if drain.count >= BATCH_CAP || drain.buf[drain.count - 1].0 >= drain.end_key {
                            return Ok(());
                        }
                        if is_branch_edge(&child) {
                            return Ok(());
                        }
                    } else {
                        #[cfg(test)]
                        test_hooks::at(test_hooks::Site::SkippedSibling);
                        backtrack(rs, mark);
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
                // SAFETY: validated edge copy names live BranchU; field projection.
                let vp = unsafe { &raw const (*node).version };
                // SAFETY: live version field.
                let nsnap = unsafe { rs.sample(vp)? };
                #[cfg(test)]
                test_hooks::at(test_hooks::Site::ParentSampled);
                let here = Holder::Node(vp, nsnap);
                let d = digit(suffix, level);
                for bd in d..=255u8 {
                    // SAFETY: direct index into live 256-slot node.
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
                    let child_prefix = prefix | (u64::from(bd) << ((level - 1) * 8));
                    let prev_count = drain.count;
                    if prev_count > 0 {
                        #[cfg(test)]
                        test_hooks::at(test_hooks::Site::BeforeNextLeaf);
                    }
                    let mark = rs.len;
                    // SAFETY: child copy validated against this branch.
                    unsafe {
                        Self::drain_batch_in(
                            drain,
                            &child,
                            rem,
                            level - 1,
                            child_prefix,
                            here,
                            rs,
                        )?;
                    }
                    if drain.count > prev_count {
                        if drain.count >= BATCH_CAP || drain.buf[drain.count - 1].0 >= drain.end_key {
                            return Ok(());
                        }
                        if is_branch_edge(&child) {
                            return Ok(());
                        }
                    } else {
                        #[cfg(test)]
                        test_hooks::at(test_hooks::Site::SkippedSibling);
                        backtrack(rs, mark);
                    }
                }
                Ok(())
            }
        }
    }
}

impl<'m, 'r> Iterator for SyncMapCursor<'m, 'r> {
    type Item = (u64, u64);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let (k, v) = self.current()?;
        self.pos += 1;
        self.last_emitted = Some(k);
        Some((k, v))
    }
}

#[cfg(test)]
pub(crate) mod test_hooks {
    use std::cell::{Cell, RefCell};

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub(crate) enum Site {
        TerminalDrained,
        SkippedSibling,
        BeforeNextLeaf,
        ParentSampled,
        CursorFinal,
    }

    type HookFn = Box<dyn FnMut()>;

    thread_local! {
        static ARMED_SITE: RefCell<Option<(Site, HookFn)>> = const { RefCell::new(None) };
        static NEG_SKIP_PARENT_VALIDATION: Cell<bool> = const { Cell::new(false) };
        static NEG_SKIP_BRANCH_VALIDATION: Cell<bool> = const { Cell::new(false) };
        static NEG_SKIP_FINAL_VALIDATION: Cell<bool> = const { Cell::new(false) };
    }

    pub(crate) fn arm_site(site: Site, f: impl FnMut() + 'static) {
        ARMED_SITE.with(|c| *c.borrow_mut() = Some((site, Box::new(f))));
    }

    pub(crate) fn set_skip_parent_validation(skip: bool) {
        NEG_SKIP_PARENT_VALIDATION.with(|c| c.set(skip));
    }

    pub(crate) fn skips_parent_validation() -> bool {
        NEG_SKIP_PARENT_VALIDATION.with(|c| c.get())
    }

    pub(crate) fn set_skip_branch_validation(skip: bool) {
        NEG_SKIP_BRANCH_VALIDATION.with(|c| c.set(skip));
    }

    pub(crate) fn skips_branch_validation() -> bool {
        NEG_SKIP_BRANCH_VALIDATION.with(|c| c.get())
    }

    pub(crate) fn set_skip_final_validation(skip: bool) {
        NEG_SKIP_FINAL_VALIDATION.with(|c| c.set(skip));
    }

    pub(crate) fn skips_final_validation() -> bool {
        NEG_SKIP_FINAL_VALIDATION.with(|c| c.get())
    }

    #[inline(always)]
    pub(crate) fn at(site: Site) {
        let action = ARMED_SITE.with(|c| {
            let mut opt = c.borrow_mut();
            if let Some((s, _)) = opt.as_ref() {
                if *s == site { opt.take() } else { None }
            } else {
                None
            }
        });
        if let Some((_, mut f)) = action {
            f();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    struct XorShift64(u64);
    impl XorShift64 {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
    }

    // --- Boundary tests (G32.6) -------------------------------------------

    #[test]
    fn test_boundary_empty_map() {
        let map = SyncExpanseMap::new();
        let rd = map.reader();
        let mut cur = rd.cursor();
        assert_eq!(cur.next(), None);
    }

    #[test]
    fn test_boundary_empty_range_lo_greater_hi() {
        let map = SyncExpanseMap::new();
        map.insert(50, 500);
        let rd = map.reader();
        let mut cur = rd.range_cursor(100, 50);
        assert_eq!(cur.next(), None);
    }

    #[test]
    fn test_boundary_single_key_present_lo_equals_hi() {
        let map = SyncExpanseMap::new();
        map.insert(42, 420);
        map.insert(100, 1000);
        let rd = map.reader();
        let mut cur = rd.range_cursor(42, 42);
        assert_eq!(cur.next(), Some((42, 420)));
        assert_eq!(cur.next(), None);
    }

    #[test]
    fn test_boundary_single_key_absent_lo_equals_hi() {
        let map = SyncExpanseMap::new();
        map.insert(42, 420);
        map.insert(100, 1000);
        let rd = map.reader();
        let mut cur = rd.range_cursor(50, 50);
        assert_eq!(cur.next(), None);
    }

    #[test]
    fn test_boundary_bounds_between_keys() {
        let map = SyncExpanseMap::new();
        for k in [10u64, 20, 30, 40, 50] {
            map.insert(k, k * 10);
        }
        let rd = map.reader();
        let mut cur = rd.range_cursor(15, 35);
        assert_eq!(cur.next(), Some((20, 200)));
        assert_eq!(cur.next(), Some((30, 300)));
        assert_eq!(cur.next(), None);
    }

    #[test]
    fn test_boundary_extremal_keys_zero_and_max() {
        let map = SyncExpanseMap::new();
        map.insert(0, 1);
        map.insert(u64::MAX, 2);
        let rd = map.reader();
        let mut cur = rd.cursor();
        assert_eq!(cur.next(), Some((0, 1)));
        assert_eq!(cur.next(), Some((u64::MAX, 2)));
        assert_eq!(cur.next(), None);
    }

    // --- Differential testing against MapCursor and BTreeMap (G32.5) -------

    fn verify_differential_against_btreemap_and_mapcursor(keys: Vec<u64>) {
        let sync_map = SyncExpanseMap::new();
        let mut btree = BTreeMap::new();
        for &k in &keys {
            let v = k.wrapping_mul(3);
            sync_map.insert(k, v);
            btree.insert(k, v);
        }

        let rd = sync_map.reader();
        let cursor = rd.cursor();
        let mut cursor_collected = Vec::new();
        for item in cursor {
            cursor_collected.push(item);
        }

        let btree_collected: Vec<(u64, u64)> = btree.iter().map(|(&k, &v)| (k, v)).collect();
        assert_eq!(cursor_collected, btree_collected, "matches BTreeMap");

        // Differential against single-threaded MapCursor under with_locked
        sync_map.with_locked(|inner_map| {
            let mut map_cursor = inner_map.cursor();
            let mut mc_collected = Vec::new();
            while let Some(item) = map_cursor.next() {
                mc_collected.push(item);
            }
            assert_eq!(cursor_collected, mc_collected, "matches MapCursor");
        });
    }

    #[test]
    fn test_differential_uniform_random() {
        let mut rng = XorShift64(0xDEAD_BEEF_CAFE_BABE);
        let mut keys = Vec::with_capacity(1000);
        for _ in 0..1000 {
            keys.push(rng.next());
        }
        verify_differential_against_btreemap_and_mapcursor(keys);
    }

    #[test]
    fn test_differential_sequential() {
        let keys: Vec<u64> = (1000..3000).collect();
        verify_differential_against_btreemap_and_mapcursor(keys);
    }

    #[test]
    fn test_differential_clustered() {
        let mut keys = Vec::new();
        for cluster in [0x1000u64, 0x50000, 0x1234_5600] {
            for offset in 0..200 {
                keys.push(cluster + offset);
            }
        }
        verify_differential_against_btreemap_and_mapcursor(keys);
    }

    #[test]
    fn test_cursor_advance_to() {
        let map = SyncExpanseMap::new();
        for i in 1..=100u64 {
            map.insert(i * 10, i * 100);
        }
        let rd = map.reader();
        let mut cur = rd.cursor();
        assert_eq!(cur.next(), Some((10, 100)));
        assert_eq!(cur.advance_to(45), Some((50, 500)));
        assert_eq!(cur.next(), Some((50, 500)));
        assert_eq!(cur.advance_to(60), Some((60, 600))); // Monotone no-op
        assert_eq!(cur.next(), Some((60, 600)));
        assert_eq!(cur.advance_to(1000), Some((1000, 10000)));
        assert_eq!(cur.next(), Some((1000, 10000)));
        assert_eq!(cur.advance_to(1001), None);
    }

    #[test]
    fn test_differential_advance_to_interleave() {
        let sync_map = SyncExpanseMap::new();
        let keys: Vec<u64> = (0u64..500).map(|i| i * 37).collect();
        for &k in &keys {
            sync_map.insert(k, k * 10);
        }
        let rd = sync_map.reader();
        let mut cur = rd.cursor();
        sync_map.with_locked(|inner_map| {
            let mut mc = inner_map.cursor();
            for i in 0..250u64 {
                if i % 2 == 0 {
                    assert_eq!(cur.next(), mc.next(), "step {i} next");
                } else {
                    let target = i * 71;
                    assert_eq!(
                        cur.advance_to(target),
                        mc.advance_to(target),
                        "step {i} advance_to({target})"
                    );
                }
            }
        });
    }

    // --- Deterministic park-point tests & negative controls (G32.1) -------

    #[test]
    fn test_park_point_insert_into_copied_terminal_before_validation() {
        let map = Arc::new(SyncExpanseMap::new());
        // Fifty-one keys across 3 digits -> RootSnapshot::Tree with BranchL3 and Leaf1 terminals (> ROOT_LEAF_CAP).
        for d in [0x10u64, 0x20, 0x30] {
            for i in 0..17u64 {
                let k = (d << 8) | (1 + 2 * i);
                map.insert(k, k * 10);
            }
        }
        let rd = map.reader();

        // Positive control: writer inserts into the terminal while reader is parked
        let m = Arc::clone(&map);
        let inserted_key = (0x10 << 8) | 2;
        test_hooks::arm_site(test_hooks::Site::TerminalDrained, move || {
            m.insert(inserted_key, 9999);
        });
        test_hooks::set_skip_parent_validation(false);

        let cur = rd.cursor();
        let mut collected = Vec::new();
        for e in cur {
            collected.push(e);
        }
        assert!(
            collected.iter().any(|&(k, _)| k == inserted_key),
            "validated cursor must see the concurrent insert after retry"
        );

        // Negative control: skipping parent validation accepts stale terminal
        let map2 = Arc::new(SyncExpanseMap::new());
        for d in [0x10u64, 0x20, 0x30] {
            for i in 0..17u64 {
                let k = (d << 8) | (1 + 2 * i);
                map2.insert(k, k * 10);
            }
        }
        let rd2 = map2.reader();
        let m2 = Arc::clone(&map2);
        test_hooks::arm_site(test_hooks::Site::TerminalDrained, move || {
            m2.insert(inserted_key, 9999);
        });
        test_hooks::set_skip_parent_validation(true);
        test_hooks::set_skip_final_validation(true);

        let cur2 = rd2.cursor();
        let mut collected2 = Vec::new();
        for e in cur2 {
            collected2.push(e);
        }
        test_hooks::set_skip_parent_validation(false);
        test_hooks::set_skip_final_validation(false);
        assert!(
            !collected2.iter().any(|&(k, _)| k == inserted_key),
            "negative control: without parent validation, cursor misses concurrent insert"
        );
    }

    #[test]
    fn test_park_point_insert_into_skipped_empty_sibling() {
        let map = Arc::new(SyncExpanseMap::new());
        // Thirty-four keys across digits 0x10 and 0x30 (> ROOT_LEAF_CAP). Digit 0x20 is empty.
        for d in [0x10u64, 0x30] {
            for i in 0..17u64 {
                let k = (d << 8) | (1 + 2 * i);
                map.insert(k, k * 10);
            }
        }
        let rd = map.reader();

        // Positive control: writer inserts into empty sibling 0x20
        let inserted_key = (0x20 << 8) | 1;
        let m = Arc::clone(&map);
        test_hooks::arm_site(test_hooks::Site::SkippedSibling, move || {
            m.insert(inserted_key, 9999);
        });
        test_hooks::set_skip_branch_validation(false);

        let cur = rd.cursor();
        let mut collected = Vec::new();
        for e in cur {
            collected.push(e);
        }
        assert!(
            collected.iter().any(|&(k, _)| k == inserted_key),
            "retained read set detects insert into skipped sibling"
        );

        // Negative control:
        let map2 = Arc::new(SyncExpanseMap::new());
        for d in [0x10u64, 0x30] {
            for i in 0..17u64 {
                let k = (d << 8) | (1 + 2 * i);
                map2.insert(k, k * 10);
            }
        }
        let rd2 = map2.reader();
        let m2 = Arc::clone(&map2);
        test_hooks::arm_site(test_hooks::Site::SkippedSibling, move || {
            m2.insert(inserted_key, 9999);
        });
        test_hooks::set_skip_parent_validation(true);
        test_hooks::set_skip_branch_validation(true);
        test_hooks::set_skip_final_validation(true);

        let cur2 = rd2.cursor();
        let mut collected2 = Vec::new();
        for e in cur2 {
            collected2.push(e);
        }
        test_hooks::set_skip_parent_validation(false);
        test_hooks::set_skip_branch_validation(false);
        test_hooks::set_skip_final_validation(false);
        assert!(
            !collected2.iter().any(|&(k, _)| k == inserted_key),
            "negative control: without branch validation, cursor misses skipped sibling insert"
        );
    }

    #[test]
    fn test_park_point_removal_from_next_leaf() {
        let map = Arc::new(SyncExpanseMap::new());
        for d in [0x10u64, 0x20] {
            for i in 0..17u64 {
                let k = (d << 8) | (1 + 2 * i);
                map.insert(k, k * 10);
            }
        }
        let rd = map.reader();

        let removed_key = (0x20 << 8) | 1;
        let m = Arc::clone(&map);
        test_hooks::arm_site(test_hooks::Site::BeforeNextLeaf, move || {
            m.remove(removed_key);
        });
        test_hooks::set_skip_parent_validation(false);

        let cur = rd.cursor();
        let mut collected = Vec::new();
        for e in cur {
            collected.push(e);
        }
        assert!(
            !collected.iter().any(|&(k, _)| k == removed_key),
            "removal from leaf during traversal is not observed"
        );

        // Negative control: skipping parent validation accepts stale leaf containing removed key
        let map2 = Arc::new(SyncExpanseMap::new());
        for d in [0x10u64, 0x20] {
            for i in 0..17u64 {
                let k = (d << 8) | (1 + 2 * i);
                map2.insert(k, k * 10);
            }
        }
        let rd2 = map2.reader();
        let m2 = Arc::clone(&map2);
        test_hooks::arm_site(test_hooks::Site::BeforeNextLeaf, move || {
            m2.remove(removed_key);
        });
        test_hooks::set_skip_parent_validation(true);
        test_hooks::set_skip_final_validation(true);

        let cur2 = rd2.cursor();
        let mut collected2 = Vec::new();
        for e in cur2 {
            collected2.push(e);
        }
        test_hooks::set_skip_parent_validation(false);
        test_hooks::set_skip_final_validation(false);
        assert!(
            collected2.iter().any(|&(k, _)| k == removed_key),
            "negative control: without parent validation, cursor observes removed key from stale leaf"
        );
    }

    #[test]
    fn test_park_point_split_obsolete_parent() {
        let map = Arc::new(SyncExpanseMap::new());
        // Fifty-one keys across digits 0x10, 0x20, 0x30: BranchL3 full at 3 digits
        for d in [0x10u64, 0x20, 0x30] {
            for i in 0..17u64 {
                let k = (d << 8) | (1 + 2 * i);
                map.insert(k, k * 10);
            }
        }
        let rd = map.reader();

        let inserted_key = (0x25 << 8) | 1;
        let m = Arc::clone(&map);
        test_hooks::arm_site(test_hooks::Site::ParentSampled, move || {
            // Expanding BranchL3 to BranchL7 marks BranchL3 obsolete
            m.insert(inserted_key, 9999);
        });
        test_hooks::set_skip_parent_validation(false);
        test_hooks::set_skip_branch_validation(false);
        test_hooks::set_skip_final_validation(false);

        let cur = rd.cursor();
        let mut collected = Vec::new();
        for e in cur {
            collected.push(e);
        }
        assert_eq!(
            collected.len(),
            52,
            "cursor recovers from parent split/expansion"
        );
        assert!(
            collected.iter().any(|&(k, _)| k == inserted_key),
            "cursor sees entry inserted during parent split"
        );

        // Negative control: skipping validation traverses obsolete parent and misses inserted key
        let map2 = Arc::new(SyncExpanseMap::new());
        for d in [0x10u64, 0x20, 0x30] {
            for i in 0..17u64 {
                let k = (d << 8) | (1 + 2 * i);
                map2.insert(k, k * 10);
            }
        }
        let rd2 = map2.reader();
        let m2 = Arc::clone(&map2);
        test_hooks::arm_site(test_hooks::Site::ParentSampled, move || {
            m2.insert(inserted_key, 9999);
        });
        test_hooks::set_skip_parent_validation(true);
        test_hooks::set_skip_branch_validation(true);
        test_hooks::set_skip_final_validation(true);

        let cur2 = rd2.cursor();
        let mut collected2 = Vec::new();
        for e in cur2 {
            collected2.push(e);
        }
        test_hooks::set_skip_parent_validation(false);
        test_hooks::set_skip_branch_validation(false);
        test_hooks::set_skip_final_validation(false);
        assert_eq!(
            collected2.len(),
            51,
            "negative control: without validation, cursor traverses obsolete parent"
        );
        assert!(
            !collected2.iter().any(|&(k, _)| k == inserted_key),
            "negative control: without validation, cursor misses entry inserted during parent split"
        );
    }

    #[test]
    fn test_park_point_leaf_demotion() {
        let map = Arc::new(SyncExpanseMap::new());
        for d in [0x10u64, 0x20] {
            for i in 0..17u64 {
                let k = (d << 8) | (1 + 2 * i);
                map.insert(k, k * 10);
            }
        }
        let rd = map.reader();

        let m = Arc::clone(&map);
        test_hooks::arm_site(test_hooks::Site::CursorFinal, move || {
            // Remove keys under 0x20
            for i in 0..17u64 {
                let k = (0x20 << 8) | (1 + 2 * i);
                m.remove(k);
            }
        });
        test_hooks::set_skip_final_validation(false);

        let cur = rd.cursor();
        let mut collected = Vec::new();
        for e in cur {
            collected.push(e);
        }
        assert_eq!(
            collected.len(),
            17,
            "cursor handles leaf demotion under retry"
        );

        // Negative control: skipping final validation accepts stale pre-demotion leaf buffer
        let map2 = Arc::new(SyncExpanseMap::new());
        for d in [0x10u64, 0x20] {
            for i in 0..17u64 {
                let k = (d << 8) | (1 + 2 * i);
                map2.insert(k, k * 10);
            }
        }
        let rd2 = map2.reader();
        let m2 = Arc::clone(&map2);
        test_hooks::arm_site(test_hooks::Site::CursorFinal, move || {
            for i in 0..17u64 {
                let k = (0x20 << 8) | (1 + 2 * i);
                m2.remove(k);
            }
        });
        test_hooks::set_skip_final_validation(true);

        let cur2 = rd2.cursor();
        let mut collected2 = Vec::new();
        for e in cur2 {
            collected2.push(e);
        }
        test_hooks::set_skip_final_validation(false);
        assert_eq!(
            collected2.len(),
            34,
            "negative control: without final validation, cursor emits stale pre-demotion keys"
        );
    }
}
