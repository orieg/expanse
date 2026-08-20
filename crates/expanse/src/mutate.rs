//! Phase 6: the set-flavor mutation engine.
//!
//! Insert and remove follow the **least-compressed-form ladder**: a
//! subexpanse only decompresses when its current form overflows, and only
//! recompresses with a **1-index hysteresis band** so an insert/delete
//! oscillation at a boundary never thrashes allocations:
//!
//! ```text
//! grow:   Null → Immed(1 key) → Immed(n) → LinearLeaf → { level 1: BitmapLeaf → FullExpanse
//!                                                        { level ≥2: BranchL3 → L7 → B → U (cascade)
//! shrink: each conversion runs one index later than its grow twin
//! ```
//!
//! v1 restrictions (matching the lookup engine): mutation never creates
//! narrow pointers — a sparse chain is a chain of one-child linear
//! branches, and every child sits exactly one level below its parent. The
//! original's level-skipping compression returns together with the
//! per-level tag redesign it requires.
//!
//! Population bookkeeping: every edge at level ≤ 7 carries its subtree's
//! `pop0`; the level-8 total lives in the owning tree (`ExpanseSet`), the
//! original's JPM role. Immediates carry their count in the tag.
//!
//! The invariant validator ([`validate_subtree`]) walks a tree and panics
//! on any structural violation; `docs/TESTING.md` requires a negative
//! control proving it can fail, which lives in the `set` module's tests.

use crate::alloc::NodeAlloc;
use crate::leaf;
use crate::node::{BranchB, BranchL3, BranchL7, BranchU, Edge, LeafBitmap1, LeafBitmapL};
use crate::types::{BRANCH_L3_CAP, BRANCH_L7_CAP, EdgeTag, EdgeType, ImmedType, Key, digit};

/// Linear-leaf population cap at level 1; overflow converts to a bitmap
/// leaf (the published design converts at populations above ~25).
pub(crate) const LEAF1_CAP: usize = 25;
/// Linear-leaf population cap at levels 2..=7; overflow cascades into a
/// branch. 32 seven-byte keys = 224 B, a handful of cache lines.
pub(crate) const LEAF_CAP: usize = 32;
/// Bitmap branch upgrades to uncompressed at this many populated digits.
pub(crate) const BRANCHB_UP: usize = crate::types::BITMAP_TO_UNCOMPRESSED_THRESHOLD;

/// Reads the `slot`-th packed key (little-endian, `kb` bytes) as a number.
///
/// # Safety
///
/// `keys` must be valid for reads of `(slot + 1) * kb` bytes.
pub(crate) unsafe fn read_packed(keys: *const u8, slot: usize, kb: usize) -> u64 {
    // Width-monomorphized like `leaf::search_fixed`: at a runtime width
    // the copy does not inline, and this sits in the innermost insert
    // loop via `leaf::lower_bound` (issue #1 item 3).
    // SAFETY: forwarded contract; each arm's KB equals `kb`.
    unsafe {
        match kb {
            1 => read_packed_fixed::<1>(keys, slot),
            2 => read_packed_fixed::<2>(keys, slot),
            3 => read_packed_fixed::<3>(keys, slot),
            4 => read_packed_fixed::<4>(keys, slot),
            5 => read_packed_fixed::<5>(keys, slot),
            6 => read_packed_fixed::<6>(keys, slot),
            7 => read_packed_fixed::<7>(keys, slot),
            _ => {
                debug_assert!(false, "packed key width out of range: {kb}");
                0
            }
        }
    }
}

/// Reads the `slot`-th packed key at a compile-time width.
///
/// # Safety
///
/// `keys` must be valid for reads of `(slot + 1) * KB` bytes.
#[inline]
pub(crate) unsafe fn read_packed_fixed<const KB: usize>(keys: *const u8, slot: usize) -> u64 {
    let mut buf = [0u8; 8];
    // SAFETY: in-bounds per this function's contract; `KB <= 7 < 8`, so
    // the destination has room and the copy width is a constant.
    unsafe { core::ptr::copy_nonoverlapping(keys.add(slot * KB), buf.as_mut_ptr(), KB) };
    u64::from_le_bytes(buf)
}

/// Writes `val`'s low `kb` bytes as the `slot`-th packed key.
///
/// # Safety
///
/// `keys` must be valid for writes of `(slot + 1) * kb` bytes.
pub(crate) unsafe fn write_packed(keys: *mut u8, slot: usize, kb: usize, val: u64) {
    let le = val.to_le_bytes();
    // SAFETY: forwarded contract; the copy width is a constant in each
    // arm, so it inlines rather than calling out (issue #1 item 3).
    unsafe {
        match kb {
            1 => core::ptr::copy_nonoverlapping(le.as_ptr(), keys.add(slot), 1),
            2 => core::ptr::copy_nonoverlapping(le.as_ptr(), keys.add(slot * 2), 2),
            3 => core::ptr::copy_nonoverlapping(le.as_ptr(), keys.add(slot * 3), 3),
            4 => core::ptr::copy_nonoverlapping(le.as_ptr(), keys.add(slot * 4), 4),
            5 => core::ptr::copy_nonoverlapping(le.as_ptr(), keys.add(slot * 5), 5),
            6 => core::ptr::copy_nonoverlapping(le.as_ptr(), keys.add(slot * 6), 6),
            7 => core::ptr::copy_nonoverlapping(le.as_ptr(), keys.add(slot * 7), 7),
            _ => debug_assert!(false, "packed key width out of range: {kb}"),
        }
    }
}

/// Masks a key down to its low `kb` bytes.
pub(crate) const fn key_low(key: Key, kb: u8) -> u64 {
    if kb >= 8 {
        key
    } else {
        key & ((1u64 << (8 * kb as u32)) - 1)
    }
}

/// A stack-resident buffer for one immediate edge's payload.
///
/// Immediates hold at most [`IMMED_PAYLOAD_BYTES`] single-byte keys (15),
/// plus room for one more while an insert decides whether the form still
/// fits — so 16 slots cover every case and nothing needs the heap. This
/// used to be a `Vec`, i.e. a malloc/free on **every** insert into an
/// immediate: the most common terminal form for sparse and random keys,
/// and the widest part of the insert gap vs stock (issue #1 item 2).
#[derive(Debug)]
pub(crate) struct ImmedBuf<T: Copy + Default> {
    buf: [T; IMMED_BUF_CAP],
    len: usize,
}

/// Slot capacity of [`ImmedBuf`]: the 15-key immediate maximum plus one
/// transient slot for a key being inserted.
const IMMED_BUF_CAP: usize = crate::types::IMMED_PAYLOAD_BYTES + 1;

impl<T: Copy + Default> ImmedBuf<T> {
    pub(crate) fn new() -> Self {
        Self {
            buf: [T::default(); IMMED_BUF_CAP],
            len: 0,
        }
    }

    pub(crate) fn push(&mut self, v: T) {
        debug_assert!(self.len < IMMED_BUF_CAP, "immediate buffer overflow");
        self.buf[self.len] = v;
        self.len += 1;
    }

    pub(crate) fn insert(&mut self, at: usize, v: T) {
        debug_assert!(at <= self.len && self.len < IMMED_BUF_CAP);
        self.buf.copy_within(at..self.len, at + 1);
        self.buf[at] = v;
        self.len += 1;
    }

    pub(crate) fn remove(&mut self, at: usize) -> T {
        debug_assert!(at < self.len);
        let v = self.buf[at];
        self.buf.copy_within(at + 1..self.len, at);
        self.len -= 1;
        v
    }

    pub(crate) fn as_slice(&self) -> &[T] {
        &self.buf[..self.len]
    }
}

/// Read access to the populated prefix: indexing, `binary_search`,
/// `windows`, iteration and the rest of the slice API come from here,
/// and `&ImmedBuf<T>` coerces to `&[T]` at call sites that want one.
impl<T: Copy + Default> core::ops::Deref for ImmedBuf<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        &self.buf[..self.len]
    }
}

/// Collects the sorted keys of a set-flavor immediate edge.
pub(crate) fn immed_keys(edge: &Edge, im: ImmedType) -> ImmedBuf<u64> {
    let payload = edge.imm_payload();
    let kb = im.key_bytes() as usize;
    let mut out = ImmedBuf::new();
    for slot in 0..im.key_count() as usize {
        let mut buf = [0u8; 8];
        buf[..kb].copy_from_slice(&payload[slot * kb..(slot + 1) * kb]);
        out.push(u64::from_le_bytes(buf));
    }
    out
}

/// Rebuilds a set-flavor immediate edge from sorted keys.
fn write_immed(edge: &mut Edge, kb: u8, keys: &[u64]) {
    let im = ImmedType::new(kb, keys.len() as u8).expect("immediate capacity");
    let mut payload = [0u8; 15];
    for (slot, &k) in keys.iter().enumerate() {
        payload[slot * kb as usize..(slot + 1) * kb as usize]
            .copy_from_slice(&k.to_le_bytes()[..kb as usize]);
    }
    let mut w0 = [0u8; 8];
    w0.copy_from_slice(&payload[..8]);
    let mut aux = [0u8; 7];
    aux.copy_from_slice(&payload[8..]);
    edge.set_imm_bytes(w0);
    edge.set_aux_bytes(aux);
    edge.set_tag(im.as_u8());
}

/// Map immediates keep their keys in the 7 aux bytes (word 0 holds the
/// value or the value-array pointer), so capacity is `7 / key_bytes`.
pub(crate) const fn map_immed_max(kb: u8) -> usize {
    7 / kb as usize
}

/// Collects the sorted keys of a map-flavor immediate edge (aux bytes).
pub(crate) fn immed_map_keys(edge: &Edge, im: ImmedType) -> ImmedBuf<u64> {
    let aux = edge.aux_bytes();
    let kb = im.key_bytes() as usize;
    let mut out = ImmedBuf::new();
    for slot in 0..im.key_count() as usize {
        let mut buf = [0u8; 8];
        buf[..kb].copy_from_slice(&aux[slot * kb..(slot + 1) * kb]);
        out.push(u64::from_le_bytes(buf));
    }
    out
}

/// Collects the sorted keys of a linear leaf.
///
/// # Safety
///
/// The edge must reference a live linear leaf of `pop` keys.
unsafe fn leaf_keys(edge: &Edge, kb: u8, pop: usize) -> Vec<u64> {
    let base = edge.node_ptr();
    // Headroom for the callers' mid-buffer insert, filled via `extend`
    // so TrustedLen elides the per-element capacity checks — see
    // `read_map_leaf`.
    let mut out = Vec::with_capacity(pop + 1);
    // SAFETY: leaf holds pop packed keys per contract.
    out.extend((0..pop).map(|slot| unsafe { read_packed(base, slot, kb as usize) }));
    out
}

/// Allocates a linear leaf from sorted keys and points `edge` at it.
fn build_leaf(a: &NodeAlloc, edge: &mut Edge, kb: u8, keys: &[u64]) {
    let ptr = a.alloc_bytes(leaf::size_set(kb, keys.len()));
    for (slot, &k) in keys.iter().enumerate() {
        // SAFETY: in-bounds writes of the fresh allocation.
        unsafe { write_packed(ptr.as_ptr(), slot, kb as usize, k) };
    }
    let tag = match kb {
        1 => EdgeType::Leaf1,
        2 => EdgeType::Leaf2,
        3 => EdgeType::Leaf3,
        4 => EdgeType::Leaf4,
        5 => EdgeType::Leaf5,
        6 => EdgeType::Leaf6,
        _ => EdgeType::Leaf7,
    };
    *edge = Edge::new_node(ptr.as_ptr(), tag.as_u8());
    edge.set_pop0(kb, keys.len() as u64 - 1);
}

/// A branch node's own level, read from its header. Behind a narrow
/// pointer it sits below the edge's slot level; `BranchU` never skips
/// (it has no header), so it is always at its slot level.
///
/// # Safety
///
/// `edge` must reference a live branch node of type `t`.
pub(crate) unsafe fn branch_form_level(edge: &Edge, t: EdgeType, slot_level: u8) -> u8 {
    // SAFETY: live node of the tagged type per contract.
    unsafe {
        match t {
            EdgeType::BranchL3 => (*edge.node_ptr().cast::<BranchL3>()).hdr.level,
            EdgeType::BranchL7 => (*edge.node_ptr().cast::<BranchL7>()).hdr.level,
            EdgeType::BranchB => (*edge.node_ptr().cast::<BranchB>()).level,
            _ => slot_level,
        }
    }
}

/// Class-sized byte length of a bitmap-branch edge subarray.
pub(crate) const fn sub_edges_size(n: usize) -> usize {
    leaf::cap_class(n) * size_of::<Edge>()
}

/// Class-sized byte length of a bitmap-map-leaf value subarray.
pub(crate) const fn sub_vals_size(n: usize) -> usize {
    leaf::cap_class(n) * 8
}

/// Highest level (1-based) at which two `level`-byte suffixes differ —
/// the level where a key set diverges (its natural branch point).
pub(crate) fn divergence_level(a: u64, b: u64, level: u8) -> u8 {
    let x = key_low(a ^ b, level);
    debug_assert!(x != 0);
    (63 - x.leading_zeros() as u8) / 8 + 1
}

/// Reads a skipping edge's decode digits (levels `kb+1..=level`) as one
/// integer, LSB = the digit at level `kb+1`.
pub(crate) fn decode_value(edge: &Edge, kb: u8, level: u8) -> u64 {
    let d = edge.decode_bytes(kb);
    let mut v = 0u64;
    let mut i = (level - kb) as usize;
    while i > 0 {
        i -= 1;
        v = (v << 8) | u64::from(d[i]);
    }
    v
}

/// Writes the decode digits for a child at `kb` under a slot at `level`,
/// taken from `key`'s digits at levels `kb+1..=level`.
pub(crate) fn write_decode(edge: &mut Edge, kb: u8, level: u8, key: u64) {
    let mut dec = [0u8; 7];
    for (i, slot) in dec.iter_mut().enumerate().take((level - kb) as usize) {
        *slot = digit(key, kb + 1 + i as u8);
    }
    edge.set_decode_bytes(kb, &dec[..(level - kb) as usize]);
}

/// Restores a narrow pointer's decode bytes after a form rebuild that
/// reset the aux field (leaf reallocation, bitmap-leaf conversion).
pub(crate) fn restore_decode(edge: &mut Edge, kb: u8, level: u8, saved_aux: &[u8; 7]) {
    if kb < level {
        edge.set_decode_bytes(kb, &saved_aux[kb as usize..level as usize]);
    }
}

/// Materializes one skipped level of a narrow pointer as a one-slot
/// `BranchL3` at `at`, consuming the decode digit there as the branch
/// digit. The branch inherits the decode bytes above `at` (it is itself
/// narrow when `at < slot_level`); the child keeps those below. Used
/// when a full skipping `BranchB` must become a `BranchU` (which has no
/// header and cannot skip): wrapping at `bl + 1` un-skips it in place.
pub(crate) fn wrap_skip_level(
    a: &NodeAlloc,
    edge: &mut Edge,
    at: u8,
    slot_level: u8,
    subtree_pop: u64,
) {
    debug_assert!((2..=slot_level).contains(&at));
    let aux = *edge.aux_bytes();
    let t = aux[at as usize - 1];
    let mut child = *edge;
    for i in (at as usize - 1)..(slot_level as usize).min(7) {
        child.clear_aux_byte(i);
    }
    let node = a.alloc_node_zeroed::<BranchL3>();
    // SAFETY: node is freshly allocated zeroed BranchL3 memory.
    unsafe {
        (*node.as_ptr()).hdr.level = at;
        (*node.as_ptr()).hdr.num = 1;
        (*node.as_ptr()).hdr.digits[0] = t;
        (*node.as_ptr()).edges[0] = child;
    }
    *edge = Edge::new_node(node.as_ptr().cast(), EdgeType::BranchL3.as_u8());
    if at < slot_level {
        edge.set_decode_bytes(at, &aux[at as usize..slot_level as usize]);
    }
    edge.set_pop0(at, subtree_pop - 1);
}

/// Splits a narrow pointer where an insert diverges from its skipped
/// prefix: the branch materializes at the **highest** decode level whose
/// digit disagrees with `key`, not one level at a time — the retry then
/// lands in this branch and opens a sibling slot, with no single-child
/// chain along the divergence path.
pub(crate) fn split_skip(a: &NodeAlloc, edge: &mut Edge, key: Key, level: u8, subtree_pop: u64) {
    let aux = *edge.aux_bytes();
    let mut at = level;
    while digit(key, at) == aux[at as usize - 1] {
        at -= 1;
    }
    wrap_skip_level(a, edge, at, level, subtree_pop);
}

/// Bumps the subtree `pop0` of an edge at `level` by `delta` (level 8 has
/// no pop0 field; the tree owner tracks the total).
pub(crate) fn bump_pop0(edge: &mut Edge, level: u8, delta: i64) {
    if level <= 7 {
        let pop0 = edge.pop0(level) as i64;
        edge.set_pop0(level, (pop0 + delta) as u64);
    }
}

/// Inserts a new digit + null child into a linear branch header at its
/// sorted position and returns the slot.
pub(crate) fn linear_insert_slot(
    digits: &mut [u8; 8],
    edges: &mut [Edge],
    num: usize,
    d: u8,
) -> usize {
    let pos = digits[..num].iter().position(|&x| x > d).unwrap_or(num);
    for i in (pos..num).rev() {
        digits[i + 1] = digits[i];
        edges[i + 1] = edges[i];
    }
    digits[pos] = d;
    edges[pos] = Edge::NULL;
    pos
}

/// Runtime entry point: resolves the OCC flag **once per operation** and
/// dispatches into the monomorphized engine. Public callers use this;
/// the engine itself recurses with `OCC` already fixed, so the fences
/// and their checks vanish entirely on single-threaded trees.
///
/// # Safety
///
/// Same contract as [`insert`].
pub(crate) unsafe fn insert_dyn(a: &NodeAlloc, edge: &mut Edge, key: Key, level: u8) -> bool {
    // SAFETY: forwarded caller contract.
    unsafe {
        if a.occ_enabled() {
            insert::<true>(a, edge, key, level)
        } else {
            insert::<false>(a, edge, key, level)
        }
    }
}

/// Runtime entry point for removal; see [`insert_dyn`].
///
/// # Safety
///
/// Same contract as [`remove`].
pub(crate) unsafe fn remove_dyn(a: &NodeAlloc, edge: &mut Edge, key: Key, level: u8) -> bool {
    // SAFETY: forwarded caller contract.
    unsafe {
        if a.occ_enabled() {
            remove::<true>(a, edge, key, level)
        } else {
            remove::<false>(a, edge, key, level)
        }
    }
}

/// Inserts `key` into the subtree at `edge` (covering `level` undecoded
/// bytes); returns `true` if the key was newly inserted.
///
/// # Safety
///
/// The subtree must be well-formed (mutation-engine invariants: no narrow
/// pointers, children one level below parents) and owned by `a`.
pub(crate) unsafe fn insert<const OCC: bool>(
    a: &NodeAlloc,
    edge: &mut Edge,
    key: Key,
    mut level: u8,
) -> bool {
    loop {
        debug_assert!((1..=8).contains(&level));
        let tag = edge.tag().expect("valid edge tag");
        match tag {
            EdgeTag::Structural(EdgeType::Null) => {
                if level == 8 {
                    // No 8-byte leaves/immediates: the top starts as a branch.
                    let node = a.alloc_node_zeroed::<BranchL3>();
                    // SAFETY: node is freshly allocated zeroed BranchL3 memory.
                    unsafe {
                        (*node.as_ptr()).hdr.level = level;
                    }
                    *edge = Edge::new_node(node.as_ptr().cast(), EdgeType::BranchL3.as_u8());
                    continue;
                }
                write_immed(edge, level, &[key_low(key, level)]);
                return true;
            }

            EdgeTag::Immed(im) => {
                debug_assert_eq!(im.key_bytes(), level);
                let kb = im.key_bytes();
                let k = key_low(key, kb);
                let mut keys = immed_keys(edge, im);
                let Err(pos) = keys.binary_search(&k) else {
                    return false;
                };
                keys.insert(pos, k);
                if keys.len() <= ImmedType::max_count(kb) as usize {
                    write_immed(edge, kb, &keys);
                } else {
                    build_leaf(a, edge, kb, &keys);
                }
                return true;
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
                if kb < level && !crate::get::decode_matches(edge, key, kb, level) {
                    // The key diverges inside the skipped prefix: expose one
                    // decode digit as a branch level and retry (a chain forms
                    // only along the divergence path).
                    split_skip(a, edge, key, level, pop as u64);
                    continue;
                }
                let k = key_low(key, kb);
                // Phase 7 coverage invariant (see alloc::assert_bracketed):
                // linear leaves carry no version; the parent's bracket does.
                a.assert_bracketed();
                let base = edge.node_ptr();
                // SAFETY: live leaf of `pop` keys per contract.
                let pos = unsafe { leaf::lower_bound(base, pop, kb, k) };
                // SAFETY: pos < pop is in bounds.
                if pos < pop && unsafe { read_packed(base, pos, kb as usize) } == k {
                    return false;
                }
                let cap = if kb == 1 { LEAF1_CAP } else { LEAF_CAP };
                if pop < cap && leaf::cap_class(pop + 1) == leaf::cap_class(pop) {
                    // Fast path: spare class capacity — shift in place, no
                    // reallocation, no key materialization.
                    // SAFETY: class capacity spare per the check.
                    unsafe { leaf::set_insert_at(base, kb, pop, pos, k) };
                    edge.set_pop0(kb, pop as u64);
                    return true;
                }
                let old_ptr = base;
                let old_size = leaf::size_set(kb, pop);
                let saved_aux = *edge.aux_bytes();
                if pop < cap {
                    // Class-crossing grow that stays this leaf: direct copy
                    // with a gap — the set twin of the map-flavor fix, which
                    // measured -13.27% on churn and up to -25% on inserts.
                    // Form and kb are unchanged, so aux (decode + pop0) is
                    // preserved wholesale and no key widening is needed.
                    let new = a.alloc_bytes(leaf::size_set(kb, pop + 1));
                    // SAFETY: live source leaf of `pop` keys; fresh
                    // destination sized for `pop + 1`; `pos <= pop`.
                    unsafe { leaf::set_realloc_insert(base, new.as_ptr(), kb, pop, pos, k) };
                    *edge = Edge::new_node(new.as_ptr(), edge.tag_byte());
                    edge.set_aux_bytes(saved_aux);
                    edge.set_pop0(kb, pop as u64);
                    // SAFETY: unlinked above; freed with its allocation size.
                    unsafe {
                        a.free_bytes(
                            core::ptr::NonNull::new(old_ptr).expect("leaf ptr"),
                            old_size,
                        );
                    }
                    return true;
                }
                // Slow path (form conversion). A skipping leaf's keys are
                // widened to full `level`-byte suffixes with its decode
                // prefix, so the conversions below can place the replacement
                // form at the true divergence level.
                // SAFETY: live leaf of `pop` keys per contract.
                let mut keys = unsafe { leaf_keys(edge, kb, pop) };
                keys.insert(pos, k);
                if kb < level {
                    let prefix = decode_value(edge, kb, level) << (8 * u32::from(kb));
                    for k in &mut keys {
                        *k |= prefix;
                    }
                }
                if keys.len() <= cap {
                    build_leaf(a, edge, kb, &keys);
                    restore_decode(edge, kb, level, &saved_aux);
                } else if kb == 1 {
                    // Level-1 overflow: linear leaf → bitmap leaf (any narrow
                    // pointer carries over).
                    let ptr = a.alloc_node_zeroed::<LeafBitmap1>();
                    // SAFETY: ptr is freshly allocated zeroed LeafBitmap1 memory.
                    unsafe {
                        for &k in &keys {
                            (*ptr.as_ptr()).bitmap.set(k as u8);
                        }
                    }
                    *edge = Edge::new_node(ptr.as_ptr().cast(), EdgeType::LeafB1.as_u8());
                    edge.set_pop0(1, keys.len() as u64 - 1);
                    restore_decode(edge, 1, level, &saved_aux);
                } else {
                    let d = divergence_level(keys[0], keys[keys.len() - 1], level);
                    if d == 1 {
                        // Narrow-pointer synthesis: every key shares its
                        // digits at levels 2..=kb, so the whole set fits one
                        // bitmap leaf whose decode bytes hold the shared
                        // prefix — no single-child branch chain.
                        let ptr = a.alloc_node_zeroed::<LeafBitmap1>();
                        // SAFETY: ptr is freshly allocated zeroed LeafBitmap1 memory.
                        unsafe {
                            for &k in &keys {
                                (*ptr.as_ptr()).bitmap.set(k as u8);
                            }
                        }
                        *edge = Edge::new_node(ptr.as_ptr().cast(), EdgeType::LeafB1.as_u8());
                        edge.set_pop0(1, keys.len() as u64 - 1);
                        write_decode(edge, 1, level, keys[0]);
                    } else {
                        // Cascade: an empty branch at the divergence level (a
                        // narrow pointer when that sits below the slot; the
                        // root edge has no room for decode bytes, so level-8
                        // slots always branch in place), then re-insert.
                        let bl = if level <= 7 { d } else { level };
                        let node = a.alloc_node_zeroed::<BranchL3>();
                        // SAFETY: node is freshly allocated zeroed BranchL3 memory.
                        unsafe {
                            (*node.as_ptr()).hdr.level = bl;
                        }
                        *edge = Edge::new_node(node.as_ptr().cast(), EdgeType::BranchL3.as_u8());
                        if bl < level {
                            write_decode(edge, bl, level, keys[0]);
                        }
                        for &k in &keys {
                            // SAFETY: freshly built branch subtree owned by `a`.
                            let ins = unsafe { insert::<OCC>(a, edge, k, level) };
                            debug_assert!(ins);
                        }
                        // pop0 encodes pop-1 and cannot express the transient
                        // empty branch, so the per-insert bumps land one high;
                        // pin the true population once the cascade settles.
                        edge.set_pop0(bl, keys.len() as u64 - 1);
                    }
                }
                // SAFETY: the old leaf allocation is no longer referenced.
                unsafe {
                    a.free_bytes(
                        core::ptr::NonNull::new(old_ptr).expect("leaf ptr"),
                        old_size,
                    );
                }
                return true;
            }

            EdgeTag::Structural(EdgeType::LeafB1) => {
                if level > 1 && !crate::get::decode_matches(edge, key, 1, level) {
                    // Diverges inside the skipped prefix: branch out one
                    // level and retry.
                    let pop = edge.pop0(1) + 1;
                    split_skip(a, edge, key, level, pop);
                    continue;
                }
                // Phase 7: see mutate_map — the parent branch's bracket
                // covers this leaf's payload for concurrent readers.
                a.assert_bracketed();
                // SAFETY: live LeafBitmap1 per contract.
                let node = unsafe { &mut *edge.node_ptr().cast::<LeafBitmap1>() };
                if !node.bitmap.set(digit(key, 1)) {
                    return false;
                }
                let pop = edge.pop0(1) as usize + 2; // old pop + the new key
                if pop == 256 && level == 1 {
                    // Full-expanse edges cover their entire current expanse,
                    // so the conversion only applies to non-skipping leaves.
                    let ptr = core::ptr::NonNull::new(edge.node_ptr().cast::<LeafBitmap1>());
                    // SAFETY: node no longer referenced after the tag swap.
                    unsafe { a.free_node(ptr.expect("leaf ptr")) };
                    *edge = Edge::NULL;
                    edge.set_tag(EdgeType::FullExpanse.as_u8());
                    edge.set_pop0(1, 255);
                } else {
                    edge.set_pop0(1, pop as u64 - 1);
                }
                return true;
            }

            EdgeTag::Structural(EdgeType::FullExpanse) => return false,

            EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
                debug_assert!(level >= 2);
                // SAFETY: live branch per contract.
                let bl = unsafe { branch_form_level(edge, t, level) };
                if bl < level && !crate::get::decode_matches(edge, key, bl, level) {
                    // Diverges inside the skipped prefix: branch out one
                    // level and retry.
                    let pop = edge.pop0(bl) + 1;
                    split_skip(a, edge, key, level, pop);
                    continue;
                }
                let d = digit(key, bl);
                let is_l3 = matches!(t, EdgeType::BranchL3);
                // SAFETY: live branch node per contract; header/edges layouts
                // are identical prefixes apart from the edge-array length.
                let (hdr_find, num) = unsafe {
                    if is_l3 {
                        let b = &*edge.node_ptr().cast::<BranchL3>();
                        (b.hdr.find(d), b.hdr.num as usize)
                    } else {
                        let b = &*edge.node_ptr().cast::<BranchL7>();
                        (b.hdr.find(d), b.hdr.num as usize)
                    }
                };
                if let Some(slot) = hdr_find {
                    // SAFETY: slot < num ≤ capacity; child subtree well-formed.
                    // The node's version brackets the descent: the child slot
                    // and everything beneath it may be rewritten in place.
                    let inserted = unsafe {
                        if is_l3 {
                            let b = &mut *edge.node_ptr().cast::<BranchL3>();
                            crate::occ::version_begin_if::<OCC>(a, &mut b.hdr.version);
                            let r = insert::<OCC>(a, &mut b.edges[slot], key, bl - 1);
                            crate::occ::version_end_if::<OCC>(a, &mut b.hdr.version);
                            r
                        } else {
                            let b = &mut *edge.node_ptr().cast::<BranchL7>();
                            crate::occ::version_begin_if::<OCC>(a, &mut b.hdr.version);
                            let r = insert::<OCC>(a, &mut b.edges[slot], key, bl - 1);
                            crate::occ::version_end_if::<OCC>(a, &mut b.hdr.version);
                            r
                        }
                    };
                    if inserted {
                        bump_pop0(edge, bl, 1);
                    }
                    return inserted;
                }
                let cap = if is_l3 { BRANCH_L3_CAP } else { BRANCH_L7_CAP };
                if num == cap {
                    // SAFETY: upgrade rebuilds the node; subtree stays owned.
                    unsafe {
                        if is_l3 {
                            upgrade_l3_to_l7(a, edge);
                        } else {
                            upgrade_l7_to_b(a, edge);
                        }
                    }
                    continue;
                }
                // SAFETY: live branch; slot arithmetic bounded by capacity.
                let inserted = unsafe {
                    if is_l3 {
                        let b = &mut *edge.node_ptr().cast::<BranchL3>();
                        crate::occ::version_begin_if::<OCC>(a, &mut b.hdr.version);
                        let slot = linear_insert_slot(&mut b.hdr.digits, &mut b.edges, num, d);
                        b.hdr.num += 1;
                        let r = insert::<OCC>(a, &mut b.edges[slot], key, bl - 1);
                        crate::occ::version_end_if::<OCC>(a, &mut b.hdr.version);
                        r
                    } else {
                        let b = &mut *edge.node_ptr().cast::<BranchL7>();
                        crate::occ::version_begin_if::<OCC>(a, &mut b.hdr.version);
                        let slot = linear_insert_slot(&mut b.hdr.digits, &mut b.edges, num, d);
                        b.hdr.num += 1;
                        let r = insert::<OCC>(a, &mut b.edges[slot], key, bl - 1);
                        crate::occ::version_end_if::<OCC>(a, &mut b.hdr.version);
                        r
                    }
                };
                debug_assert!(inserted);
                bump_pop0(edge, bl, 1);
                return true;
            }

            EdgeTag::Structural(EdgeType::BranchB) => {
                debug_assert!(level >= 2);
                // SAFETY: live branch per contract.
                let bl = unsafe { branch_form_level(edge, EdgeType::BranchB, level) };
                if bl < level && !crate::get::decode_matches(edge, key, bl, level) {
                    let pop = edge.pop0(bl) + 1;
                    split_skip(a, edge, key, level, pop);
                    continue;
                }
                let slot_level = level;
                let d = digit(key, bl);
                // SAFETY: live BranchB per contract.
                let b = unsafe { &mut *edge.node_ptr().cast::<BranchB>() };
                if b.bitmap.test(d) {
                    let slot = b.bitmap.subexpanse_rank(d) as usize;
                    let sub = b.subarrays[(d >> 5) as usize];
                    // SAFETY: bitmap/subarray consistency invariant. The
                    // node's version brackets the descent (see the L3 arm).
                    crate::occ::version_begin_if::<OCC>(a, &mut b.version);
                    // SAFETY: bitmap/subarray consistency invariant.
                    let inserted = unsafe { insert::<OCC>(a, &mut *sub.add(slot), key, bl - 1) };
                    crate::occ::version_end_if::<OCC>(a, &mut b.version);
                    if inserted {
                        bump_pop0(edge, bl, 1);
                    }
                    return inserted;
                }
                if b.bitmap.count() as usize + 1 > BRANCHB_UP {
                    if bl < slot_level {
                        // BranchU has no header level, so it cannot skip:
                        // materialize the level just above the form and retry.
                        let pop = edge.pop0(bl) + 1;
                        wrap_skip_level(a, edge, bl + 1, slot_level, pop);
                        level = slot_level;
                        continue;
                    }
                    // SAFETY: upgrade rebuilds the node; subtree stays owned
                    // (the guard above ensures it sits at its slot level).
                    unsafe {
                        upgrade_b_to_u(a, edge);
                    }
                    level = slot_level;
                    continue;
                }
                // Grow this subexpanse's packed array by one slot at the rank
                // (bracketed: the shift/realloc, bitmap set, and descent all
                // mutate state a reader may be traversing).
                let sub = (d >> 5) as usize;
                let old_n = b.pop_counts[sub] as usize;
                let rank = b.bitmap.subexpanse_rank(d) as usize;
                crate::occ::version_begin_if::<OCC>(a, &mut b.version);
                if old_n > 0 && leaf::cap_class(old_n + 1) == leaf::cap_class(old_n) {
                    // Fast path: spare class capacity — shift in place.
                    // SAFETY: the subarray holds cap_class(old_n) slots.
                    unsafe {
                        let arr = b.subarrays[sub];
                        core::ptr::copy(arr.add(rank), arr.add(rank + 1), old_n - rank);
                        arr.add(rank).write(Edge::NULL);
                    }
                } else {
                    let new = a.alloc_bytes(sub_edges_size(old_n + 1)).cast::<Edge>();
                    // SAFETY: copying old_n live edges around the inserted
                    // slot into cap_class(old_n + 1) slots; the empty case
                    // touches no old pointer.
                    unsafe {
                        if old_n > 0 {
                            let old = b.subarrays[sub];
                            new.as_ptr().copy_from_nonoverlapping(old, rank);
                            new.as_ptr()
                                .add(rank + 1)
                                .copy_from_nonoverlapping(old.add(rank), old_n - rank);
                            a.free_bytes(
                                core::ptr::NonNull::new(old.cast()).expect("subarray"),
                                sub_edges_size(old_n),
                            );
                        }
                        new.as_ptr().add(rank).write(Edge::NULL);
                    }
                    b.subarrays[sub] = new.as_ptr();
                }
                b.pop_counts[sub] = (old_n + 1) as u16;
                b.bitmap.set(d);
                // SAFETY: fresh null child slot within the subarray.
                let inserted =
                    unsafe { insert::<OCC>(a, &mut *b.subarrays[sub].add(rank), key, bl - 1) };
                crate::occ::version_end_if::<OCC>(a, &mut b.version);
                debug_assert!(inserted);
                bump_pop0(edge, bl, 1);
                return true;
            }

            EdgeTag::Structural(EdgeType::BranchU) => {
                debug_assert!(level >= 2);
                let d = digit(key, level);
                // SAFETY: live BranchU per contract (never skipping).
                let b = unsafe { &mut *edge.node_ptr().cast::<BranchU>() };
                // SAFETY: child subtree well-formed (or null) per contract.
                crate::occ::version_begin_if::<OCC>(a, &mut b.version);
                // SAFETY: child subtree well-formed (or null) per contract.
                let inserted =
                    unsafe { insert::<OCC>(a, &mut b.edges[d as usize], key, level - 1) };
                crate::occ::version_end_if::<OCC>(a, &mut b.version);
                if inserted {
                    bump_pop0(edge, level, 1);
                }
                return inserted;
            }
        }
    }
}

/// L3 → L7: copy the three children into a bigger node.
///
/// # Safety
///
/// `edge` must reference a live, full `BranchL3` owned by `a`.
pub(crate) unsafe fn upgrade_l3_to_l7(a: &NodeAlloc, edge: &mut Edge) {
    // SAFETY: live BranchL3 per contract.
    let old = unsafe { &*edge.node_ptr().cast::<BranchL3>() };
    let new = a.alloc_node_zeroed::<BranchL7>();
    // SAFETY: new is freshly allocated zeroed BranchL7 memory; old is live BranchL3.
    unsafe {
        core::ptr::addr_of_mut!((*new.as_ptr()).hdr).write(old.hdr);
        core::ptr::copy_nonoverlapping(
            old.edges.as_ptr(),
            (*new.as_ptr()).edges.as_mut_ptr(),
            BRANCH_L3_CAP,
        );
    }
    let aux = *edge.aux_bytes();
    // SAFETY: old node no longer referenced.
    unsafe { a.free_node(core::ptr::NonNull::new(edge.node_ptr().cast::<BranchL3>()).unwrap()) };
    *edge = Edge::new_node(new.as_ptr().cast(), EdgeType::BranchL7.as_u8());
    edge.set_aux_bytes(aux);
}

/// L7 → BranchB: distribute the seven children into bitmap subarrays.
///
/// # Safety
///
/// `edge` must reference a live, full `BranchL7` owned by `a`.
pub(crate) unsafe fn upgrade_l7_to_b(a: &NodeAlloc, edge: &mut Edge) {
    // SAFETY: live BranchL7 per contract.
    let old = unsafe { &*edge.node_ptr().cast::<BranchL7>() };
    let new = a.alloc_node_zeroed::<BranchB>();
    // SAFETY: new is freshly allocated zeroed BranchB memory; old is live BranchL7.
    unsafe {
        (*new.as_ptr()).level = old.hdr.level;
        // Count digits per subexpanse first, then pack per-subexpanse arrays.
        for i in 0..old.hdr.num as usize {
            (*new.as_ptr()).pop_counts[(old.hdr.digits[i] >> 5) as usize] += 1;
        }
        for sub in 0..8 {
            let n = (*new.as_ptr()).pop_counts[sub] as usize;
            if n > 0 {
                (*new.as_ptr()).subarrays[sub] =
                    a.alloc_bytes(sub_edges_size(n)).cast::<Edge>().as_ptr();
            }
        }
        let mut filled = [0usize; 8];
        for i in 0..old.hdr.num as usize {
            let d = old.hdr.digits[i];
            let sub = (d >> 5) as usize;
            (*new.as_ptr()).bitmap.set(d);
            // SAFETY: filled[sub] < pop_counts[sub] slots just allocated.
            (*new.as_ptr()).subarrays[sub]
                .add(filled[sub])
                .write(old.edges[i]);
            filled[sub] += 1;
        }
    }
    let aux = *edge.aux_bytes();
    // SAFETY: old node no longer referenced.
    unsafe { a.free_node(core::ptr::NonNull::new(edge.node_ptr().cast::<BranchL7>()).unwrap()) };
    *edge = Edge::new_node(new.as_ptr().cast(), EdgeType::BranchB.as_u8());
    edge.set_aux_bytes(aux);
}

/// BranchB → BranchU: flatten the subarrays into 256 direct slots.
///
/// # Safety
///
/// `edge` must reference a live **non-skipping** `BranchB` owned by `a`
/// (`BranchU` has no header level; callers wrap a skipping node first).
pub(crate) unsafe fn upgrade_b_to_u(a: &NodeAlloc, edge: &mut Edge) {
    let new = a.alloc_node_zeroed::<BranchU>();
    // SAFETY: live BranchB; subarray reads bounded by pop_counts.
    unsafe {
        let old = &*edge.node_ptr().cast::<BranchB>();
        let u = &mut *new.as_ptr();
        let mut d = old.bitmap.next_set(0);
        while let Some(dig) = d {
            let sub = (dig >> 5) as usize;
            let rank = old.bitmap.subexpanse_rank(dig) as usize;
            u.edges[dig as usize] = *old.subarrays[sub].add(rank);
            d = if dig == 255 {
                None
            } else {
                old.bitmap.next_set(dig + 1)
            };
        }
        for sub in 0..8 {
            let n = old.pop_counts[sub] as usize;
            if n > 0 {
                a.free_bytes(
                    core::ptr::NonNull::new(old.subarrays[sub].cast()).unwrap(),
                    sub_edges_size(n),
                );
            }
        }
    }
    let aux = *edge.aux_bytes();
    // SAFETY: old node no longer referenced.
    unsafe { a.free_node(core::ptr::NonNull::new(edge.node_ptr().cast::<BranchB>()).unwrap()) };
    *edge = Edge::new_node(new.as_ptr().cast(), EdgeType::BranchU.as_u8());
    edge.set_aux_bytes(aux);
}

/// Removes `key` from the subtree at `edge`; returns `true` if removed.
///
/// # Safety
///
/// Same contract as [`insert`].
pub(crate) unsafe fn remove<const OCC: bool>(
    a: &NodeAlloc,
    edge: &mut Edge,
    key: Key,
    level: u8,
) -> bool {
    debug_assert!((1..=8).contains(&level));
    let tag = edge.tag().expect("valid edge tag");
    match tag {
        EdgeTag::Structural(EdgeType::Null) => false,

        EdgeTag::Immed(im) => {
            let kb = im.key_bytes();
            debug_assert_eq!(kb, level);
            let k = key_low(key, kb);
            let mut keys = immed_keys(edge, im);
            let Ok(pos) = keys.binary_search(&k) else {
                return false;
            };
            keys.remove(pos);
            if keys.is_empty() {
                *edge = Edge::NULL;
            } else {
                write_immed(edge, kb, &keys);
            }
            true
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
            if kb < level && !crate::get::decode_matches(edge, key, kb, level) {
                return false;
            }
            let pop = edge.pop0(kb) as usize + 1;
            let k = key_low(key, kb);
            // Phase 7 coverage invariant (see alloc::assert_bracketed):
            // linear leaves carry no version; the parent's bracket does.
            a.assert_bracketed();
            let base = edge.node_ptr();
            // SAFETY: live leaf of `pop` keys per contract.
            let pos = unsafe { leaf::lower_bound(base, pop, kb, k) };
            // SAFETY: pos < pop is in bounds.
            if pos == pop || unsafe { read_packed(base, pos, kb as usize) } != k {
                return false;
            }
            if pop > ImmedType::max_count(level) as usize
                && leaf::cap_class(pop - 1) == leaf::cap_class(pop)
            {
                // Fast path: stays a leaf in the same class — shift left
                // in place.
                // SAFETY: pos < pop; same-class allocation.
                unsafe { leaf::set_remove_at(base, kb, pop, pos) };
                edge.set_pop0(kb, pop as u64 - 2);
                return true;
            }
            if pop >= 2 && pop > ImmedType::max_count(level) as usize {
                // Class-crossing shrink that stays this leaf (the
                // hysteresis band keeps it one below the immediate
                // boundary): direct copy with the slot elided — the set
                // twin of the map-flavor fix.
                let new = a.alloc_bytes(leaf::size_set(kb, pop - 1));
                // SAFETY: live source leaf of `pop >= 2` keys; fresh
                // destination sized for `pop - 1`; `pos < pop`.
                unsafe { leaf::set_realloc_remove(base, new.as_ptr(), kb, pop, pos) };
                let saved_aux = *edge.aux_bytes();
                *edge = Edge::new_node(new.as_ptr(), edge.tag_byte());
                edge.set_aux_bytes(saved_aux);
                edge.set_pop0(kb, pop as u64 - 2);
                // SAFETY: unlinked above; freed with its allocation size.
                unsafe {
                    a.free_bytes(
                        core::ptr::NonNull::new(base).expect("leaf ptr"),
                        leaf::size_set(kb, pop),
                    );
                }
                return true;
            }
            // Slow path (conversion to immediate).
            // SAFETY: live leaf of `pop` keys per contract.
            let mut keys = unsafe { leaf_keys(edge, kb, pop) };
            keys.remove(pos);
            let old_ptr = base;
            let old_size = leaf::size_set(kb, pop);
            let saved_aux = *edge.aux_bytes();
            // Hysteresis: convert to an immediate one index below the
            // immediate→leaf boundary. A skipping leaf converts to an
            // immediate at the *slot* level, absorbing its decode bytes
            // back into full key remainders.
            if keys.len() < ImmedType::max_count(level) as usize {
                let dv = if kb < level {
                    decode_value(edge, kb, level)
                } else {
                    0
                };
                let full: Vec<u64> = keys
                    .iter()
                    .map(|&low| (dv << (8 * u32::from(kb))) | low)
                    .collect();
                write_immed(edge, level, &full);
            } else {
                build_leaf(a, edge, kb, &keys);
                restore_decode(edge, kb, level, &saved_aux);
            }
            // SAFETY: old leaf allocation no longer referenced.
            unsafe {
                a.free_bytes(
                    core::ptr::NonNull::new(old_ptr).expect("leaf ptr"),
                    old_size,
                );
            }
            true
        }

        EdgeTag::Structural(EdgeType::LeafB1) => {
            if level > 1 && !crate::get::decode_matches(edge, key, 1, level) {
                return false;
            }
            // Phase 7: see mutate_map — the parent branch's bracket
            // covers this leaf's payload for concurrent readers.
            a.assert_bracketed();
            // SAFETY: live LeafBitmap1 per contract.
            let node = unsafe { &mut *edge.node_ptr().cast::<LeafBitmap1>() };
            if !node.bitmap.clear(digit(key, 1)) {
                return false;
            }
            let pop = edge.pop0(1) as usize; // old pop - 1
            // Hysteresis: convert back to a linear leaf one below the cap.
            if pop < LEAF1_CAP {
                let mut keys = Vec::with_capacity(pop);
                let mut d = node.bitmap.next_set(0);
                while let Some(dig) = d {
                    keys.push(u64::from(dig));
                    d = if dig == 255 {
                        None
                    } else {
                        node.bitmap.next_set(dig + 1)
                    };
                }
                let saved_aux = *edge.aux_bytes();
                let dv = if level > 1 {
                    decode_value(edge, 1, level)
                } else {
                    0
                };
                // SAFETY: node no longer referenced after rebuild.
                unsafe {
                    a.free_node(
                        core::ptr::NonNull::new(edge.node_ptr().cast::<LeafBitmap1>()).unwrap(),
                    );
                }
                if keys.len() < ImmedType::max_count(level) as usize {
                    // Absorb any decode bytes into full slot-level keys.
                    let full: Vec<u64> = keys.iter().map(|&low| (dv << 8) | low).collect();
                    write_immed(edge, level, &full);
                } else {
                    build_leaf(a, edge, 1, &keys);
                    restore_decode(edge, 1, level, &saved_aux);
                }
            } else {
                edge.set_pop0(1, pop as u64 - 1);
            }
            true
        }

        EdgeTag::Structural(EdgeType::FullExpanse) => {
            // Materialize one decompression step, then retry the removal.
            if level == 1 {
                let node = a.alloc_node_zeroed::<LeafBitmap1>();
                // SAFETY: node is freshly allocated zeroed LeafBitmap1 memory.
                unsafe {
                    for d in 0..=255u8 {
                        (*node.as_ptr()).bitmap.set(d);
                    }
                }
                *edge = Edge::new_node(node.as_ptr().cast(), EdgeType::LeafB1.as_u8());
                edge.set_pop0(1, 255);
            } else {
                let ptr = a.alloc_node_zeroed::<BranchU>();
                // SAFETY: ptr is freshly allocated zeroed BranchU memory.
                unsafe {
                    for child in &mut (*ptr.as_ptr()).edges {
                        child.set_tag(EdgeType::FullExpanse.as_u8());
                        child.set_pop0(level - 1, pow256(level - 1) - 1);
                    }
                }
                *edge = Edge::new_node(ptr.as_ptr().cast(), EdgeType::BranchU.as_u8());
                edge.set_pop0(level, pow256(level) - 1);
            }
            // SAFETY: freshly materialized well-formed subtree.
            unsafe { remove::<OCC>(a, edge, key, level) }
        }

        EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
            // SAFETY: live branch per contract.
            let bl = unsafe { branch_form_level(edge, t, level) };
            if bl < level && !crate::get::decode_matches(edge, key, bl, level) {
                return false;
            }
            let d = digit(key, bl);
            let is_l3 = matches!(t, EdgeType::BranchL3);
            // SAFETY: live branch per contract.
            let removed = unsafe {
                if is_l3 {
                    let b = &mut *edge.node_ptr().cast::<BranchL3>();
                    let Some(slot) = b.hdr.find(d) else {
                        return false;
                    };
                    crate::occ::version_begin_if::<OCC>(a, &mut b.hdr.version);
                    let r = remove::<OCC>(a, &mut b.edges[slot], key, bl - 1);
                    if r && b.edges[slot].is_null() {
                        linear_remove_slot(
                            &mut b.hdr.digits,
                            &mut b.edges,
                            b.hdr.num as usize,
                            slot,
                        );
                        b.hdr.num -= 1;
                    }
                    crate::occ::version_end_if::<OCC>(a, &mut b.hdr.version);
                    r
                } else {
                    let b = &mut *edge.node_ptr().cast::<BranchL7>();
                    let Some(slot) = b.hdr.find(d) else {
                        return false;
                    };
                    crate::occ::version_begin_if::<OCC>(a, &mut b.hdr.version);
                    let r = remove::<OCC>(a, &mut b.edges[slot], key, bl - 1);
                    if r && b.edges[slot].is_null() {
                        linear_remove_slot(
                            &mut b.hdr.digits,
                            &mut b.edges,
                            b.hdr.num as usize,
                            slot,
                        );
                        b.hdr.num -= 1;
                    }
                    crate::occ::version_end_if::<OCC>(a, &mut b.hdr.version);
                    r
                }
            };
            if !removed {
                return false;
            }
            // SAFETY: node rebuilds below keep the subtree owned.
            unsafe {
                let num = if is_l3 {
                    (*edge.node_ptr().cast::<BranchL3>()).hdr.num as usize
                } else {
                    (*edge.node_ptr().cast::<BranchL7>()).hdr.num as usize
                };
                if num == 0 {
                    // Last key of the subtree: pop0 cannot express an
                    // empty branch, so free it instead of bumping.
                    free_branch_node(a, edge, is_l3);
                    *edge = Edge::NULL;
                    return true;
                }
                bump_pop0(edge, bl, -1);
                if !is_l3 && num < BRANCH_L3_CAP {
                    // Hysteresis: L7 → L3 one index below the L3 capacity.
                    downgrade_l7_to_l3(a, edge);
                }
            }
            true
        }

        EdgeTag::Structural(EdgeType::BranchB) => {
            // SAFETY: live branch per contract.
            let bl = unsafe { branch_form_level(edge, EdgeType::BranchB, level) };
            if bl < level && !crate::get::decode_matches(edge, key, bl, level) {
                return false;
            }
            let d = digit(key, bl);
            // SAFETY: live BranchB per contract.
            let b = unsafe { &mut *edge.node_ptr().cast::<BranchB>() };
            if !b.bitmap.test(d) {
                return false;
            }
            let sub = (d >> 5) as usize;
            let rank = b.bitmap.subexpanse_rank(d) as usize;
            // SAFETY: bitmap/subarray consistency invariant. Bracketed
            // through the subarray shrink below (a reader may be inside).
            crate::occ::version_begin_if::<OCC>(a, &mut b.version);
            // SAFETY: bitmap/subarray consistency invariant.
            let removed =
                unsafe { remove::<OCC>(a, &mut *b.subarrays[sub].add(rank), key, bl - 1) };
            if !removed {
                crate::occ::version_end_if::<OCC>(a, &mut b.version);
                return false;
            }
            // SAFETY: child slot checked/live per invariant.
            let child_null = unsafe { (*b.subarrays[sub].add(rank)).is_null() };
            if child_null {
                let old_n = b.pop_counts[sub] as usize;
                // SAFETY: shrink of the packed subarray — in place when
                // the class holds, reallocating across class boundaries.
                unsafe {
                    let old = b.subarrays[sub];
                    if old_n == 1 {
                        b.subarrays[sub] = core::ptr::null_mut();
                        a.free_bytes(
                            core::ptr::NonNull::new(old.cast()).unwrap(),
                            sub_edges_size(old_n),
                        );
                    } else if leaf::cap_class(old_n - 1) == leaf::cap_class(old_n) {
                        core::ptr::copy(old.add(rank + 1), old.add(rank), old_n - 1 - rank);
                    } else {
                        let new = a.alloc_bytes(sub_edges_size(old_n - 1)).cast::<Edge>();
                        new.as_ptr().copy_from_nonoverlapping(old, rank);
                        new.as_ptr()
                            .add(rank)
                            .copy_from_nonoverlapping(old.add(rank + 1), old_n - 1 - rank);
                        b.subarrays[sub] = new.as_ptr();
                        a.free_bytes(
                            core::ptr::NonNull::new(old.cast()).unwrap(),
                            sub_edges_size(old_n),
                        );
                    }
                }
                b.pop_counts[sub] = (old_n - 1) as u16;
                b.bitmap.clear(d);
            }
            crate::occ::version_end_if::<OCC>(a, &mut b.version);
            let digits = b.bitmap.count() as usize;
            if digits == 0 {
                // SAFETY: empty node no longer referenced.
                unsafe {
                    a.free_node(
                        core::ptr::NonNull::new(edge.node_ptr().cast::<BranchB>()).unwrap(),
                    );
                }
                *edge = Edge::NULL;
                return true;
            }
            bump_pop0(edge, bl, -1);
            if digits < BRANCH_L7_CAP {
                // Hysteresis: B → L7 one index below the L7 capacity.
                // SAFETY: rebuild keeps the subtree owned.
                unsafe { downgrade_b_to_l7(a, edge) };
            }
            true
        }

        EdgeTag::Structural(EdgeType::BranchU) => {
            let d = digit(key, level);
            // SAFETY: live BranchU per contract.
            let b = unsafe { &mut *edge.node_ptr().cast::<BranchU>() };
            // SAFETY: child subtree well-formed (or null) per contract.
            crate::occ::version_begin_if::<OCC>(a, &mut b.version);
            // SAFETY: child subtree well-formed (or null) per contract.
            let removed = unsafe { remove::<OCC>(a, &mut b.edges[d as usize], key, level - 1) };
            crate::occ::version_end_if::<OCC>(a, &mut b.version);
            if !removed {
                return false;
            }
            let digits = b.edges.iter().filter(|e| !e.is_null()).count();
            if digits == 0 {
                // SAFETY: empty node no longer referenced.
                unsafe {
                    a.free_node(
                        core::ptr::NonNull::new(edge.node_ptr().cast::<BranchU>()).unwrap(),
                    );
                }
                *edge = Edge::NULL;
                return true;
            }
            bump_pop0(edge, level, -1);
            if digits < BRANCHB_UP {
                // Hysteresis: U → B one index below the U threshold.
                // SAFETY: rebuild keeps the subtree owned.
                unsafe { downgrade_u_to_b(a, edge, level) };
            }
            true
        }
    }
}

pub(crate) fn linear_remove_slot(
    digits: &mut [u8; 8],
    edges: &mut [Edge],
    num: usize,
    slot: usize,
) {
    for i in slot..num - 1 {
        digits[i] = digits[i + 1];
        edges[i] = edges[i + 1];
    }
    digits[num - 1] = 0;
    edges[num - 1] = Edge::NULL;
}

/// # Safety
///
/// `edge` must reference a live linear-branch node of the flavor named by
/// `is_l3`, no longer referenced afterwards.
unsafe fn free_branch_node(a: &NodeAlloc, edge: &mut Edge, is_l3: bool) {
    // SAFETY: forwarded contract.
    unsafe {
        if is_l3 {
            a.free_node(core::ptr::NonNull::new(edge.node_ptr().cast::<BranchL3>()).unwrap());
        } else {
            a.free_node(core::ptr::NonNull::new(edge.node_ptr().cast::<BranchL7>()).unwrap());
        }
    }
}

/// # Safety
///
/// `edge` must reference a live `BranchL7` with ≤ 3 children, owned by `a`.
pub(crate) unsafe fn downgrade_l7_to_l3(a: &NodeAlloc, edge: &mut Edge) {
    // SAFETY: live BranchL7 per contract.
    let old = unsafe { &*edge.node_ptr().cast::<BranchL7>() };
    let new = a.alloc_node_zeroed::<BranchL3>();
    // SAFETY: new is freshly allocated zeroed BranchL3 memory; old is live BranchL7.
    unsafe {
        core::ptr::addr_of_mut!((*new.as_ptr()).hdr).write(old.hdr);
        core::ptr::copy_nonoverlapping(
            old.edges.as_ptr(),
            (*new.as_ptr()).edges.as_mut_ptr(),
            BRANCH_L3_CAP,
        );
    }
    let aux = *edge.aux_bytes();
    // SAFETY: old node no longer referenced.
    unsafe { a.free_node(core::ptr::NonNull::new(edge.node_ptr().cast::<BranchL7>()).unwrap()) };
    *edge = Edge::new_node(new.as_ptr().cast(), EdgeType::BranchL3.as_u8());
    edge.set_aux_bytes(aux);
}

/// # Safety
///
/// `edge` must reference a live `BranchB` with ≤ 7 digits, owned by `a`.
pub(crate) unsafe fn downgrade_b_to_l7(a: &NodeAlloc, edge: &mut Edge) {
    // SAFETY: live BranchB per contract (level read below).
    let b_level = unsafe { (*edge.node_ptr().cast::<BranchB>()).level };
    let new = a.alloc_node_zeroed::<BranchL7>();
    // SAFETY: live BranchB; reads bounded by bitmap/pop_counts invariant.
    unsafe {
        let old = &*edge.node_ptr().cast::<BranchB>();
        (*new.as_ptr()).hdr.level = b_level;
        let mut d = old.bitmap.next_set(0);
        while let Some(dig) = d {
            let sub = (dig >> 5) as usize;
            let rank = old.bitmap.subexpanse_rank(dig) as usize;
            let num = (*new.as_ptr()).hdr.num as usize;
            (*new.as_ptr()).hdr.digits[num] = dig;
            (*new.as_ptr()).edges[num] = *old.subarrays[sub].add(rank);
            (*new.as_ptr()).hdr.num += 1;
            d = if dig == 255 {
                None
            } else {
                old.bitmap.next_set(dig + 1)
            };
        }
        for sub in 0..8 {
            let n = old.pop_counts[sub] as usize;
            if n > 0 {
                a.free_bytes(
                    core::ptr::NonNull::new(old.subarrays[sub].cast()).unwrap(),
                    sub_edges_size(n),
                );
            }
        }
    }
    let aux = *edge.aux_bytes();
    // SAFETY: old node no longer referenced.
    unsafe { a.free_node(core::ptr::NonNull::new(edge.node_ptr().cast::<BranchB>()).unwrap()) };
    *edge = Edge::new_node(new.as_ptr().cast(), EdgeType::BranchL7.as_u8());
    edge.set_aux_bytes(aux);
}

/// # Safety
///
/// `edge` must reference a live `BranchU` with ≤ `BRANCHB_UP - 1` non-null
/// children, owned by `a`.
pub(crate) unsafe fn downgrade_u_to_b(a: &NodeAlloc, edge: &mut Edge, level: u8) {
    let new = a.alloc_node_zeroed::<BranchB>();
    // SAFETY: live BranchU per contract.
    unsafe {
        let old = &*edge.node_ptr().cast::<BranchU>();
        (*new.as_ptr()).level = level;
        for (d, child) in old.edges.iter().enumerate() {
            if !child.is_null() {
                (*new.as_ptr()).pop_counts[d >> 5] += 1;
                (*new.as_ptr()).bitmap.set(d as u8);
            }
        }
        for sub in 0..8 {
            let n = (*new.as_ptr()).pop_counts[sub] as usize;
            if n > 0 {
                (*new.as_ptr()).subarrays[sub] =
                    a.alloc_bytes(sub_edges_size(n)).cast::<Edge>().as_ptr();
            }
        }
        let mut filled = [0usize; 8];
        for (d, child) in old.edges.iter().enumerate() {
            if !child.is_null() {
                (*new.as_ptr()).subarrays[d >> 5]
                    .add(filled[d >> 5])
                    .write(*child);
                filled[d >> 5] += 1;
            }
        }
    }
    let aux = *edge.aux_bytes();
    // SAFETY: old node no longer referenced.
    unsafe { a.free_node(core::ptr::NonNull::new(edge.node_ptr().cast::<BranchU>()).unwrap()) };
    *edge = Edge::new_node(new.as_ptr().cast(), EdgeType::BranchB.as_u8());
    edge.set_aux_bytes(aux);
}

pub(crate) const fn pow256(level: u8) -> u64 {
    1u64 << (8 * level as u32)
}

/// Frees an entire subtree, returning `edge` to null.
///
/// # Safety
///
/// Same contract as [`insert`]; nothing may reference the subtree after.
pub(crate) unsafe fn free_subtree<const MAP: bool>(a: &NodeAlloc, edge: &mut Edge) {
    let Some(tag) = edge.tag() else { return };
    // SAFETY: live nodes per this function's contract, freed exactly once,
    // children freed before their parent node.
    unsafe {
        match tag {
            EdgeTag::Structural(EdgeType::Null | EdgeType::FullExpanse) => {
                debug_assert!(!(MAP && matches!(tag, EdgeTag::Structural(EdgeType::FullExpanse))));
            }
            EdgeTag::Immed(im) => {
                // Multi-key map immediates own a value array in word 0.
                if MAP && im.key_count() > 1 {
                    a.free_bytes(
                        core::ptr::NonNull::new(edge.node_ptr()).unwrap(),
                        im.key_count() as usize * 8,
                    );
                }
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
                let size = if MAP {
                    leaf::size_map(kb, pop)
                } else {
                    leaf::size_set(kb, pop)
                };
                a.free_bytes(core::ptr::NonNull::new(edge.node_ptr()).unwrap(), size);
            }
            EdgeTag::Structural(EdgeType::LeafB1) => {
                if MAP {
                    let node = &*edge.node_ptr().cast::<LeafBitmapL>();
                    for sub in 0..8 {
                        let n = node.bitmap.subexpanse_count(sub) as usize;
                        if n > 0 {
                            a.free_bytes(
                                core::ptr::NonNull::new(node.values[sub].cast()).unwrap(),
                                sub_vals_size(n),
                            );
                        }
                    }
                    a.free_node(
                        core::ptr::NonNull::new(edge.node_ptr().cast::<LeafBitmapL>()).unwrap(),
                    );
                } else {
                    a.free_node(
                        core::ptr::NonNull::new(edge.node_ptr().cast::<LeafBitmap1>()).unwrap(),
                    );
                }
            }
            EdgeTag::Structural(EdgeType::BranchL3) => {
                let b = &mut *edge.node_ptr().cast::<BranchL3>();
                for i in 0..b.hdr.num as usize {
                    free_subtree::<MAP>(a, &mut b.edges[i]);
                }
                a.free_node(core::ptr::NonNull::new(edge.node_ptr().cast::<BranchL3>()).unwrap());
            }
            EdgeTag::Structural(EdgeType::BranchL7) => {
                let b = &mut *edge.node_ptr().cast::<BranchL7>();
                for i in 0..b.hdr.num as usize {
                    free_subtree::<MAP>(a, &mut b.edges[i]);
                }
                a.free_node(core::ptr::NonNull::new(edge.node_ptr().cast::<BranchL7>()).unwrap());
            }
            EdgeTag::Structural(EdgeType::BranchB) => {
                let b = &mut *edge.node_ptr().cast::<BranchB>();
                for sub in 0..8 {
                    let n = b.pop_counts[sub] as usize;
                    for i in 0..n {
                        free_subtree::<MAP>(a, &mut *b.subarrays[sub].add(i));
                    }
                    if n > 0 {
                        a.free_bytes(
                            core::ptr::NonNull::new(b.subarrays[sub].cast()).unwrap(),
                            sub_edges_size(n),
                        );
                    }
                }
                a.free_node(core::ptr::NonNull::new(edge.node_ptr().cast::<BranchB>()).unwrap());
            }
            EdgeTag::Structural(EdgeType::BranchU) => {
                let b = &mut *edge.node_ptr().cast::<BranchU>();
                for child in &mut b.edges {
                    free_subtree::<MAP>(a, child);
                }
                a.free_node(core::ptr::NonNull::new(edge.node_ptr().cast::<BranchU>()).unwrap());
            }
        }
    }
    *edge = Edge::NULL;
}

/// Walks a subtree, panicking on any structural-invariant violation, and
/// returns the subtree population. `docs/TESTING.md` "Structural invariant
/// validator" is the checklist this implements.
///
/// # Safety
///
/// Same contract as [`insert`].
pub(crate) unsafe fn validate_subtree<const MAP: bool>(edge: &Edge, level: u8) -> u64 {
    assert!((1..=8).contains(&level), "level out of range");
    let tag = edge.tag().expect("invalid edge tag byte");
    // Branch form level: below the slot level only behind a narrow
    // pointer (never at level 8 — the root edge has no room for both
    // pop0 and decode bytes, and BranchU has no header to skip with).
    let bl = match tag {
        EdgeTag::Structural(
            t @ (EdgeType::BranchL3 | EdgeType::BranchL7 | EdgeType::BranchB | EdgeType::BranchU),
        ) => {
            // SAFETY: live branch node of type `t` per contract.
            let bl = unsafe { branch_form_level(edge, t, level) };
            assert!(bl >= 2, "branch below level 2");
            assert!(bl <= level, "branch form level above its slot level");
            assert!(level < 8 || bl == level, "level-8 slots cannot skip");
            assert!(
                bl == level || !matches!(t, EdgeType::BranchU),
                "uncompressed branches never skip"
            );
            bl
        }
        _ => level,
    };
    let pop = match tag {
        EdgeTag::Structural(EdgeType::Null) => 0,
        EdgeTag::Immed(im) => {
            assert_eq!(im.key_bytes(), level, "immediate key size must equal level");
            let keys = if MAP {
                assert!(
                    im.key_count() as usize <= map_immed_max(im.key_bytes()),
                    "map immediate above aux capacity"
                );
                immed_map_keys(edge, im)
            } else {
                immed_keys(edge, im)
            };
            assert!(
                keys.windows(2).all(|w| w[0] < w[1]),
                "immediate keys unsorted"
            );
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
            let kb = t.leaf_key_bytes().expect("leaf tag");
            assert!(kb <= level, "leaf key size above its slot level");
            let pop = edge.pop0(kb) + 1;
            let cap = if kb == 1 { LEAF1_CAP } else { LEAF_CAP };
            assert!(pop as usize <= cap, "linear leaf above capacity");
            // Conversion to an immediate happens at the *slot* level (a
            // skipping leaf absorbs its decode bytes back into full
            // remainders), so the hysteresis floor is level-based.
            let floor = if MAP {
                map_immed_max(level)
            } else {
                ImmedType::max_count(level) as usize
            };
            assert!(
                pop as usize >= floor,
                "leaf below immediate hysteresis floor"
            );
            let keys = if MAP {
                let base = edge.node_ptr();
                // SAFETY: map leaf = values then packed keys, per layout.
                (0..pop as usize)
                    .map(|slot| unsafe {
                        read_packed(
                            base.add(leaf::map_keys_offset(pop as usize)),
                            slot,
                            kb as usize,
                        )
                    })
                    .collect::<Vec<_>>()
            } else {
                // SAFETY: live leaf per contract.
                unsafe { leaf_keys(edge, kb, pop as usize) }
            };
            assert!(keys.windows(2).all(|w| w[0] < w[1]), "leaf keys unsorted");
            pop
        }
        EdgeTag::Structural(EdgeType::LeafB1) => {
            // A bitmap leaf's own level is 1; a slot level above 1 means a
            // narrow pointer whose decode bytes name the skipped digits.
            // SAFETY: live bitmap leaf of the flavor's node type per
            // contract; the map flavor adds value subarrays checked here.
            let count = if MAP {
                // SAFETY: live LeafBitmapL per contract.
                let node = unsafe { &*edge.node_ptr().cast::<LeafBitmapL>() };
                for sub in 0..8 {
                    let n = node.bitmap.subexpanse_count(sub) as usize;
                    assert_eq!(
                        node.values[sub].is_null(),
                        n == 0,
                        "value subarray/bitmap disagreement"
                    );
                }
                u64::from(node.bitmap.count())
            } else {
                // SAFETY: live LeafBitmap1 per contract.
                let node = unsafe { &*edge.node_ptr().cast::<LeafBitmap1>() };
                u64::from(node.bitmap.count())
            };
            assert_eq!(
                edge.pop0(1) + 1,
                count,
                "bitmap-leaf pop0 disagrees with bitmap"
            );
            assert!(
                count as usize > LEAF1_CAP - 1,
                "bitmap leaf below hysteresis floor"
            );
            count
        }
        EdgeTag::Structural(EdgeType::FullExpanse) => {
            assert!(!MAP, "full-expanse edges are set-flavor only");
            pow256(level)
        }
        EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
            let is_l3 = matches!(t, EdgeType::BranchL3);
            // SAFETY: live branch per contract.
            let (num, digits, edges): (usize, [u8; 8], Vec<Edge>) = unsafe {
                if is_l3 {
                    let b = &*edge.node_ptr().cast::<BranchL3>();
                    (b.hdr.num as usize, b.hdr.digits, b.edges.to_vec())
                } else {
                    let b = &*edge.node_ptr().cast::<BranchL7>();
                    (b.hdr.num as usize, b.hdr.digits, b.edges.to_vec())
                }
            };
            let cap = if is_l3 { BRANCH_L3_CAP } else { BRANCH_L7_CAP };
            assert!(num >= 1 && num <= cap, "linear branch count out of range");
            assert!(
                digits[..num].windows(2).all(|w| w[0] < w[1]),
                "branch digits unsorted"
            );
            let mut pop = 0;
            for child in edges.iter().take(num) {
                assert!(!child.is_null(), "linear branch holds a null child");
                // SAFETY: child subtree per contract.
                pop += unsafe { validate_subtree::<MAP>(child, bl - 1) };
            }
            pop
        }
        EdgeTag::Structural(EdgeType::BranchB) => {
            // SAFETY: live BranchB per contract.
            let b = unsafe { &*edge.node_ptr().cast::<BranchB>() };
            let digits = b.bitmap.count() as usize;
            assert!(
                digits >= BRANCH_L7_CAP,
                "bitmap branch below hysteresis floor"
            );
            assert!(
                digits <= BRANCHB_UP,
                "bitmap branch above uncompressed threshold"
            );
            let mut pop = 0;
            for sub in 0..8usize {
                let expected = (0..32u8)
                    .filter(|i| b.bitmap.test((sub * 32) as u8 + i))
                    .count();
                assert_eq!(
                    b.pop_counts[sub] as usize, expected,
                    "bitmap-branch rank cache disagrees with bitmap"
                );
                if expected == 0 {
                    assert!(b.subarrays[sub].is_null(), "empty subexpanse with subarray");
                }
                for i in 0..expected {
                    // SAFETY: subarray holds `expected` live edges.
                    let child = unsafe { &*b.subarrays[sub].add(i) };
                    assert!(!child.is_null(), "bitmap branch holds a null child");
                    // SAFETY: child subtree per contract.
                    pop += unsafe { validate_subtree::<MAP>(child, bl - 1) };
                }
            }
            pop
        }
        EdgeTag::Structural(EdgeType::BranchU) => {
            // SAFETY: live BranchU per contract.
            let b = unsafe { &*edge.node_ptr().cast::<BranchU>() };
            let digits = b.edges.iter().filter(|e| !e.is_null()).count();
            assert!(
                digits >= BRANCHB_UP,
                "uncompressed branch below hysteresis floor"
            );
            let mut pop = 0;
            for child in &b.edges {
                if !child.is_null() {
                    // SAFETY: child subtree per contract.
                    pop += unsafe { validate_subtree::<MAP>(child, bl - 1) };
                }
            }
            pop
        }
    };
    // The pop0 field is authoritative on branch edges (computed from the
    // children here); immediates carry their count in the tag, leaves and
    // bitmap leaves were checked in their own arms.
    if bl <= 7
        && matches!(
            tag,
            EdgeTag::Structural(
                EdgeType::BranchL3 | EdgeType::BranchL7 | EdgeType::BranchB | EdgeType::BranchU
            )
        )
    {
        assert_eq!(edge.pop0(bl) + 1, pop, "branch pop0 disagrees with subtree");
    }
    pop
}
