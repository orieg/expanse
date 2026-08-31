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
//! Narrow pointers (level skipping): mutation preserves and splits narrow
//! pointers via `split_skip` and `wrap_skip_level` when keys diverge from
//! skipped prefixes, eliminating single-child branch chains.
//!
//! Population bookkeeping: every edge at level ≤ 7 carries its subtree's
//! `pop0`; the level-8 total lives in the owning tree (`ExpanseSet`), the
//! original's JPM role. Immediates carry their count in the tag.
//!
//! The invariant validator (`validate_subtree`) walks a tree and panics
//! on any structural violation; `docs/TESTING.md` requires a negative
//! control proving it can fail, which lives in the `set` module's tests.

use crate::alloc::NodeAlloc;
use crate::leaf;
use crate::node::{BranchB, BranchL3, BranchL7, BranchU, Edge, LeafBitmap1, LeafBitmapL};
use crate::types::{BRANCH_L3_CAP, BRANCH_L7_CAP, EdgeTag, EdgeType, ImmedType, Key, digit};
pub(crate) use crate::types::{LEAF_CAP, LEAF1_CAP, LEAFB1_DOWN};
use core_alloc::vec::Vec;

/// Bitmap branch upgrades to uncompressed at this many populated digits.
pub(crate) const BRANCHB_UP: usize = crate::types::BITMAP_TO_UNCOMPRESSED_THRESHOLD;

/// Reads the `slot`-th packed key (little-endian, `kb` bytes) as a number.
///
/// # Safety
///
/// `keys` must be valid for reads of `(slot + 1) * kb` bytes.
#[inline(always)]
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
#[inline(always)]
pub(crate) unsafe fn read_packed_fixed<const KB: usize>(keys: *const u8, slot: usize) -> u64 {
    match KB {
        // SAFETY: caller guarantees (slot + 1) * 1 readable bytes.
        1 => unsafe { *keys.add(slot) as u64 },
        // SAFETY: caller guarantees (slot + 1) * 2 readable bytes; unaligned read.
        2 => unsafe { u64::from((keys.add(slot * 2) as *const u16).read_unaligned()) },
        // SAFETY: caller guarantees (slot + 1) * 3 readable bytes; unaligned read.
        3 => unsafe {
            let p = keys.add(slot * 3);
            let low16 = u64::from((p as *const u16).read_unaligned());
            let high8 = u64::from(*p.add(2));
            low16 | (high8 << 16)
        },
        // SAFETY: caller guarantees (slot + 1) * 4 readable bytes; unaligned read.
        4 => unsafe { u64::from((keys.add(slot * 4) as *const u32).read_unaligned()) },
        // SAFETY: caller guarantees (slot + 1) * 5 readable bytes; unaligned read.
        5 => unsafe {
            let p = keys.add(slot * 5);
            let low32 = u64::from((p as *const u32).read_unaligned());
            let high8 = u64::from(*p.add(4));
            low32 | (high8 << 32)
        },
        // SAFETY: caller guarantees (slot + 1) * 6 readable bytes; unaligned read.
        6 => unsafe {
            let p = keys.add(slot * 6);
            let low32 = u64::from((p as *const u32).read_unaligned());
            let high16 = u64::from((p.add(4) as *const u16).read_unaligned());
            low32 | (high16 << 32)
        },
        // SAFETY: caller guarantees (slot + 1) * 7 readable bytes; unaligned read.
        7 => unsafe {
            let p = keys.add(slot * 7);
            let low32 = u64::from((p as *const u32).read_unaligned());
            let mid16 = u64::from((p.add(4) as *const u16).read_unaligned());
            let high8 = u64::from(*p.add(6));
            low32 | (mid16 << 32) | (high8 << 48)
        },
        _ => {
            let mut buf = [0u8; 8];
            // SAFETY: in-bounds per this function's contract; `KB <= 7 < 8`, so
            // the destination has room and the copy width is a constant.
            unsafe { core::ptr::copy_nonoverlapping(keys.add(slot * KB), buf.as_mut_ptr(), KB) };
            u64::from_le_bytes(buf)
        }
    }
}

/// Writes `val`'s low `KB` bytes as the `slot`-th packed key at a compile-time width.
///
/// # Safety
///
/// `keys` must be valid for writes of `(slot + 1) * KB` bytes.
#[inline(always)]
pub(crate) unsafe fn write_packed_fixed<const KB: usize>(keys: *mut u8, slot: usize, val: u64) {
    match KB {
        // SAFETY: caller guarantees (slot + 1) * 1 writable bytes.
        1 => unsafe { *keys.add(slot) = val as u8 },
        // SAFETY: caller guarantees (slot + 1) * 2 writable bytes; unaligned write.
        2 => unsafe { (keys.add(slot * 2) as *mut u16).write_unaligned(val as u16) },
        // SAFETY: caller guarantees (slot + 1) * 3 writable bytes; unaligned write.
        3 => unsafe {
            let p = keys.add(slot * 3);
            (p as *mut u16).write_unaligned(val as u16);
            *p.add(2) = (val >> 16) as u8;
        },
        // SAFETY: caller guarantees (slot + 1) * 4 writable bytes; unaligned write.
        4 => unsafe { (keys.add(slot * 4) as *mut u32).write_unaligned(val as u32) },
        // SAFETY: caller guarantees (slot + 1) * 5 writable bytes; unaligned write.
        5 => unsafe {
            let p = keys.add(slot * 5);
            (p as *mut u32).write_unaligned(val as u32);
            *p.add(4) = (val >> 32) as u8;
        },
        // SAFETY: caller guarantees (slot + 1) * 6 writable bytes; unaligned write.
        6 => unsafe {
            let p = keys.add(slot * 6);
            (p as *mut u32).write_unaligned(val as u32);
            (p.add(4) as *mut u16).write_unaligned((val >> 32) as u16);
        },
        // SAFETY: caller guarantees (slot + 1) * 7 writable bytes; unaligned write.
        7 => unsafe {
            let p = keys.add(slot * 7);
            (p as *mut u32).write_unaligned(val as u32);
            (p.add(4) as *mut u16).write_unaligned((val >> 32) as u16);
            *p.add(6) = (val >> 48) as u8;
        },
        _ => {
            let le = val.to_le_bytes();
            // SAFETY: forwarded contract.
            unsafe {
                core::ptr::copy_nonoverlapping(le.as_ptr(), keys.add(slot * KB), KB);
            }
        }
    }
}

#[inline(always)]
pub(crate) unsafe fn leaf_locate_fixed(
    keys_ptr: *const u8,
    pop: usize,
    kb: u8,
    k: u64,
) -> Result<usize, usize> {
    // SAFETY: forwarded contract.
    unsafe {
        match kb {
            1 => leaf::locate_fixed::<1>(keys_ptr, pop, k),
            2 => leaf::locate_fixed::<2>(keys_ptr, pop, k),
            3 => leaf::locate_fixed::<3>(keys_ptr, pop, k),
            4 => leaf::locate_fixed::<4>(keys_ptr, pop, k),
            5 => leaf::locate_fixed::<5>(keys_ptr, pop, k),
            6 => leaf::locate_fixed::<6>(keys_ptr, pop, k),
            _ => leaf::locate_fixed::<7>(keys_ptr, pop, k),
        }
    }
}

/// Writes `val`'s low `kb` bytes as the `slot`-th packed key.
///
/// # Safety
///
/// `keys` must be valid for writes of `(slot + 1) * kb` bytes.
#[inline(always)]
pub(crate) unsafe fn write_packed(keys: *mut u8, slot: usize, kb: usize, val: u64) {
    // SAFETY: forwarded contract; each arm's KB equals `kb`.
    unsafe {
        match kb {
            1 => write_packed_fixed::<1>(keys, slot, val),
            2 => write_packed_fixed::<2>(keys, slot, val),
            3 => write_packed_fixed::<3>(keys, slot, val),
            4 => write_packed_fixed::<4>(keys, slot, val),
            5 => write_packed_fixed::<5>(keys, slot, val),
            6 => write_packed_fixed::<6>(keys, slot, val),
            7 => write_packed_fixed::<7>(keys, slot, val),
            _ => write_packed_fixed::<8>(keys, slot, val),
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

    #[allow(dead_code)]
    pub(crate) fn insert(&mut self, at: usize, v: T) {
        debug_assert!(at <= self.len && self.len < IMMED_BUF_CAP);
        self.buf.copy_within(at..self.len, at + 1);
        self.buf[at] = v;
        self.len += 1;
    }

    #[allow(dead_code)]
    pub(crate) fn remove(&mut self, at: usize) -> T {
        debug_assert!(at < self.len);
        let v = self.buf[at];
        self.buf.copy_within(at + 1..self.len, at);
        self.len -= 1;
        v
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
pub(crate) fn write_immed(edge: &mut Edge, kb: u8, keys: &[u64]) {
    let count = keys.len();
    if count == 1 {
        *edge = Edge::new_immed_single_set(kb, keys[0]);
        return;
    }
    let im = ImmedType::new(kb, count as u8).expect("immediate capacity");
    let mut payload = [0u8; 16];
    let kb_usize = kb as usize;
    for (slot, &k) in keys.iter().enumerate() {
        // SAFETY: slot < count <= max_count(kb); (slot + 1) * kb_usize <= 15.
        unsafe { write_packed(payload.as_mut_ptr(), slot, kb_usize, k) };
    }
    let mut w0 = [0u8; 8];
    w0.copy_from_slice(&payload[..8]);
    let mut aux = [0u8; 7];
    aux.copy_from_slice(&payload[8..15]);
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

/// Fixed-capacity stack buffer for collecting small leaf keys without heap allocation.
pub(crate) struct StackKeys32 {
    pub(crate) buf: [core::mem::MaybeUninit<u64>; 32],
    pub(crate) len: usize,
}

impl StackKeys32 {
    #[inline(always)]
    pub(crate) const fn new() -> Self {
        Self {
            buf: [core::mem::MaybeUninit::uninit(); 32],
            len: 0,
        }
    }

    #[inline(always)]
    pub(crate) fn push(&mut self, k: u64) {
        debug_assert!(self.len < 32);
        self.buf[self.len].write(k);
        self.len += 1;
    }

    #[inline(always)]
    pub(crate) fn as_slice(&self) -> &[u64] {
        // SAFETY: `len` elements have been written via `push`.
        unsafe { core::slice::from_raw_parts(self.buf.as_ptr().cast::<u64>(), self.len) }
    }
}

/// Collects the sorted keys of a linear leaf.
///
/// # Safety
///
/// The edge must reference a live linear leaf of `pop` keys.
pub(crate) unsafe fn leaf_keys(edge: &Edge, kb: u8, pop: usize) -> Vec<u64> {
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
pub(crate) fn build_leaf(a: &NodeAlloc, edge: &mut Edge, kb: u8, keys: &[u64]) {
    let ptr = a.alloc_bytes(leaf::size_set(kb, keys.len()));
    let base = ptr.as_ptr();
    match kb {
        1 => {
            for (slot, &k) in keys.iter().enumerate() {
                // SAFETY: fresh allocation holds `keys.len()` slots.
                unsafe { write_packed_fixed::<1>(base, slot, k) };
            }
        }
        2 => {
            for (slot, &k) in keys.iter().enumerate() {
                // SAFETY: fresh allocation holds `keys.len()` slots.
                unsafe { write_packed_fixed::<2>(base, slot, k) };
            }
        }
        3 => {
            for (slot, &k) in keys.iter().enumerate() {
                // SAFETY: fresh allocation holds `keys.len()` slots.
                unsafe { write_packed_fixed::<3>(base, slot, k) };
            }
        }
        4 => {
            for (slot, &k) in keys.iter().enumerate() {
                // SAFETY: fresh allocation holds `keys.len()` slots.
                unsafe { write_packed_fixed::<4>(base, slot, k) };
            }
        }
        5 => {
            for (slot, &k) in keys.iter().enumerate() {
                // SAFETY: fresh allocation holds `keys.len()` slots.
                unsafe { write_packed_fixed::<5>(base, slot, k) };
            }
        }
        6 => {
            for (slot, &k) in keys.iter().enumerate() {
                // SAFETY: fresh allocation holds `keys.len()` slots.
                unsafe { write_packed_fixed::<6>(base, slot, k) };
            }
        }
        _ => {
            for (slot, &k) in keys.iter().enumerate() {
                // SAFETY: fresh allocation holds `keys.len()` slots.
                unsafe { write_packed_fixed::<7>(base, slot, k) };
            }
        }
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
        (*node.as_ptr()).hdr.presence = 1 << (t & 0x0F);
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
///
/// # Safety
/// `edge` must be a valid, live, aligned raw pointer to an `Edge`.
#[inline(always)]
pub(crate) unsafe fn bump_pop0(edge: *mut Edge, level: u8, delta: i64) {
    if level <= 7 {
        // SAFETY: caller guarantees edge is valid, live, and aligned.
        unsafe {
            let pop0 = (*edge).pop0(level) as i64;
            (*edge).set_pop0(level, (pop0 + delta) as u64);
        }
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

/// Inserts a new digit + null child into a BranchL3 header at its sorted position
/// and returns the slot, without slice iterators or loops.
#[inline(always)]
pub(crate) fn linear_insert_slot_l3(
    digits: &mut [u8; 8],
    edges: &mut [Edge; BRANCH_L3_CAP],
    num: usize,
    d: u8,
) -> usize {
    let pos = if num == 0 || digits[0] > d {
        0
    } else if num == 1 || digits[1] > d {
        1
    } else {
        2
    };
    if num == 2 {
        if pos == 0 {
            digits[2] = digits[1];
            digits[1] = digits[0];
            edges[2] = edges[1];
            edges[1] = edges[0];
        } else if pos == 1 {
            digits[2] = digits[1];
            edges[2] = edges[1];
        }
    } else if num == 1 && pos == 0 {
        digits[1] = digits[0];
        edges[1] = edges[0];
    }
    digits[pos] = d;
    edges[pos] = Edge::NULL;
    pos
}

/// Tracks the descent path of edges from the root to the active leaf
/// for fast multi-level sequential bypass.
#[derive(Clone, Copy)]
pub(crate) struct InsertPath {
    pub prefix: u64,
    pub edges: [*mut Edge; 8],
    pub levels: [u8; 8],
    pub depth: usize,
    pub leaf: *mut LeafBitmap1,
    pub leaf1: *mut u8,
    pub terminal_pop: u16,
    pub pending_pop: usize,
}

impl InsertPath {
    pub const fn empty() -> Self {
        Self {
            prefix: u64::MAX,
            edges: [core::ptr::null_mut(); 8],
            levels: [0; 8],
            depth: 0,
            leaf: core::ptr::null_mut(),
            leaf1: core::ptr::null_mut(),
            terminal_pop: 0,
            pending_pop: 0,
        }
    }

    #[inline(always)]
    pub fn record_ancestor(&mut self, edge: *mut Edge, level: u8) {
        if self.depth > 0 && self.depth < 8 {
            self.edges[self.depth] = edge;
            self.levels[self.depth] = level;
            self.depth += 1;
        }
    }

    #[inline(always)]
    pub unsafe fn flush(&mut self) {
        if self.pending_pop > 0 {
            let delta = self.pending_pop as i64;
            self.pending_pop = 0;
            for i in 1..self.depth {
                // SAFETY: path contains valid live edge pointers during active bypass.
                unsafe {
                    bump_pop0(self.edges[i], self.levels[i], delta);
                }
            }
        }
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        if self.depth != 0 {
            if self.pending_pop > 0 {
                // SAFETY: flushing pending population before clearing path references.
                unsafe {
                    self.flush();
                }
            }
            self.prefix = u64::MAX;
            self.depth = 0;
            self.leaf = core::ptr::null_mut();
            self.leaf1 = core::ptr::null_mut();
            self.terminal_pop = 0;
        }
    }
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

/// Runtime entry point with path tracking for multi-level sequential insert bypass.
///
/// # Safety
///
/// Same contract as [`insert`].
pub(crate) unsafe fn insert_path_dyn(
    a: &NodeAlloc,
    edge: &mut Edge,
    key: Key,
    level: u8,
    path: &mut InsertPath,
) -> bool {
    // SAFETY: forwarded caller contract.
    unsafe {
        if a.occ_enabled() {
            insert_with_path::<true>(a, edge, key, level, path)
        } else {
            insert_with_path::<false>(a, edge, key, level, path)
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
    level: u8,
) -> bool {
    // SAFETY: forwarded contract.
    unsafe { insert_with_path::<OCC>(a, edge, key, level, &mut InsertPath::empty()) }
}

/// Inserts `key` into the subtree at `edge` while recording the descent path
/// for fast sequential bypass.
///
/// # Safety
///
/// Same contract as [`insert`].
pub(crate) unsafe fn insert_with_path<const OCC: bool>(
    a: &NodeAlloc,
    edge: &mut Edge,
    key: Key,
    level: u8,
    path: &mut InsertPath,
) -> bool {
    if OCC {
        // SAFETY: forwarded contract.
        unsafe { insert_with_path_occ::<OCC>(a, edge, key, level, path) }
    } else {
        // SAFETY: forwarded contract.
        unsafe { insert_with_path_flat(a, edge as *mut Edge, key, level, path) }
    }
}

/// Iterative flat descent without function recursion or version checks for single-threaded sets.
#[inline(always)]
unsafe fn insert_with_path_flat(
    a: &NodeAlloc,
    mut edge: *mut Edge,
    key: Key,
    mut level: u8,
    path: &mut InsertPath,
) -> bool {
    let mut ancestors: [(*mut Edge, u8); 8] = [(core::ptr::null_mut(), 0); 8];
    let mut anc_depth = 0;
    loop {
        debug_assert!((1..=8).contains(&level));
        // SAFETY: edge points to a live valid Edge in the trie.
        let tag = unsafe { (*edge).tag_byte() };
        match tag {
            0x00 => {
                if level == 8 {
                    // No 8-byte leaves/immediates: the top starts as a branch.
                    let node = a.alloc_node_zeroed::<BranchL3>();
                    // SAFETY: node is freshly allocated zeroed BranchL3 memory.
                    unsafe {
                        (*node.as_ptr()).hdr.level = level;
                        *edge = Edge::new_node(node.as_ptr().cast(), EdgeType::BranchL3.as_u8());
                    }
                    path.clear();
                    continue;
                }
                path.clear();
                // SAFETY: write single-key immediate; ancestors array is valid.
                unsafe {
                    *edge = Edge::new_immed_single_set(level, key_low(key, level));
                    for &(anc, al) in ancestors.iter().take(anc_depth) {
                        bump_pop0(anc, al, 1);
                        path.record_ancestor(anc, al);
                    }
                }
                return true;
            }

            0x01 => {
                debug_assert!(level >= 2);
                // SAFETY: edge points to a live BranchL3 node; raw pointer derivations avoid creating unique references over parent edges.
                unsafe {
                    let b_ptr = (*edge).node_ptr().cast::<BranchL3>();
                    let bl = (*b_ptr).hdr.level;
                    if bl < level && !crate::get::decode_matches(&*edge, key, bl, level) {
                        let pop = (*edge).pop0(bl) + 1;
                        path.clear();
                        split_skip(a, &mut *edge, key, level, pop);
                        continue;
                    }
                    let d = digit(key, bl);
                    let num = (*b_ptr).hdr.num as usize;
                    let found = if num >= 1 && (*b_ptr).hdr.digits[0] == d {
                        Some(0)
                    } else if num >= 2 && (*b_ptr).hdr.digits[1] == d {
                        Some(1)
                    } else if num >= 3 && (*b_ptr).hdr.digits[2] == d {
                        Some(2)
                    } else {
                        None
                    };
                    if let Some(slot) = found {
                        ancestors[anc_depth] = (edge, bl);
                        anc_depth += 1;
                        edge = &raw mut (*b_ptr).edges[slot];
                        level = bl - 1;
                        continue;
                    }
                    if num == BRANCH_L3_CAP {
                        path.clear();
                        upgrade_l3_to_l7(a, &mut *edge);
                        continue;
                    }
                    let slot = linear_insert_slot_l3(
                        &mut (*b_ptr).hdr.digits,
                        &mut (*b_ptr).edges,
                        num,
                        d,
                    );
                    (*b_ptr).hdr.num += 1;
                    (*b_ptr).hdr.add_presence(d);
                    ancestors[anc_depth] = (edge, bl);
                    anc_depth += 1;
                    edge = &raw mut (*b_ptr).edges[slot];
                    level = bl - 1;
                    continue;
                }
            }

            0x02 => {
                debug_assert!(level >= 2);
                // SAFETY: edge points to a live BranchL7 node; raw pointer derivations avoid creating unique references over parent edges.
                unsafe {
                    let b_ptr = (*edge).node_ptr().cast::<BranchL7>();
                    let bl = (*b_ptr).hdr.level;
                    if bl < level && !crate::get::decode_matches(&*edge, key, bl, level) {
                        let pop = (*edge).pop0(bl) + 1;
                        path.clear();
                        split_skip(a, &mut *edge, key, level, pop);
                        continue;
                    }
                    let d = digit(key, bl);
                    if let Some(slot) = (*b_ptr).hdr.find(d) {
                        ancestors[anc_depth] = (edge, bl);
                        anc_depth += 1;
                        edge = &raw mut (*b_ptr).edges[slot];
                        level = bl - 1;
                        continue;
                    }
                    let num = (*b_ptr).hdr.num as usize;
                    if num == BRANCH_L7_CAP {
                        path.clear();
                        upgrade_l7_to_b(a, &mut *edge);
                        continue;
                    }
                    let slot =
                        linear_insert_slot(&mut (*b_ptr).hdr.digits, &mut (*b_ptr).edges, num, d);
                    (*b_ptr).hdr.num += 1;
                    (*b_ptr).hdr.add_presence(d);
                    ancestors[anc_depth] = (edge, bl);
                    anc_depth += 1;
                    edge = &raw mut (*b_ptr).edges[slot];
                    level = bl - 1;
                    continue;
                }
            }

            0x03 => {
                debug_assert!(level >= 2);
                // SAFETY: edge is a live BranchB edge.
                let bl = unsafe { branch_form_level(&*edge, EdgeType::BranchB, level) };
                // SAFETY: edge is a live BranchB edge.
                if bl < level && !unsafe { crate::get::decode_matches(&*edge, key, bl, level) } {
                    // SAFETY: edge is a live BranchB edge.
                    let pop = unsafe { (*edge).pop0(bl) + 1 };
                    path.clear();
                    // SAFETY: split_skip maintains valid tree invariants.
                    unsafe { split_skip(a, &mut *edge, key, level, pop) };
                    continue;
                }
                let slot_level = level;
                let d = digit(key, bl);
                // SAFETY: edge points to a live BranchB node.
                let b = unsafe { &mut *(*edge).node_ptr().cast::<BranchB>() };
                if let Some(slot) = b.bitmap.test_and_subexpanse_rank(d) {
                    let sub = b.subarrays[(d >> 5) as usize];
                    ancestors[anc_depth] = (edge, bl);
                    anc_depth += 1;
                    // SAFETY: sub points to valid live subarray; slot is in-bounds.
                    edge = unsafe { sub.add(slot) };
                    level = bl - 1;
                    continue;
                }
                if b.bitmap.count() as usize + 1 > BRANCHB_UP {
                    if bl < slot_level {
                        // SAFETY: edge is a live BranchB node.
                        let pop = unsafe { (*edge).pop0(bl) + 1 };
                        path.clear();
                        // SAFETY: wrap_skip_level maintains valid tree invariants.
                        unsafe { wrap_skip_level(a, &mut *edge, bl + 1, slot_level, pop) };
                        level = slot_level;
                        continue;
                    }
                    path.clear();
                    // SAFETY: upgrade_b_to_u upgrades live BranchB to BranchU.
                    unsafe {
                        upgrade_b_to_u(a, &mut *edge);
                    }
                    level = slot_level;
                    continue;
                }
                let sub = (d >> 5) as usize;
                let old_n = b.pop_counts[sub] as usize;
                let rank = b.bitmap.subexpanse_rank(d) as usize;
                if old_n > 0 && leaf::cap_class(old_n + 1) == leaf::cap_class(old_n) {
                    // SAFETY: spare class capacity; shift subarray in-place.
                    unsafe {
                        let arr = b.subarrays[sub];
                        core::ptr::copy(arr.add(rank), arr.add(rank + 1), old_n - rank);
                        arr.add(rank).write(Edge::NULL);
                    }
                } else {
                    let new = a.alloc_bytes(sub_edges_size(old_n + 1)).cast::<Edge>();
                    // SAFETY: allocate fresh subarray, copy old entries, write NULL, free old subarray.
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
                ancestors[anc_depth] = (edge, bl);
                anc_depth += 1;
                // SAFETY: sub is valid subarray with old_n + 1 edges; rank is in-bounds.
                edge = unsafe { b.subarrays[sub].add(rank) };
                level = bl - 1;
                continue;
            }

            0x04 => {
                debug_assert!(level >= 2);
                let d = digit(key, level);
                // SAFETY: edge is a live BranchU node.
                let b = unsafe { &mut *(*edge).node_ptr().cast::<BranchU>() };
                ancestors[anc_depth] = (edge, level);
                anc_depth += 1;
                edge = &raw mut b.edges[d as usize];
                level -= 1;
                continue;
            }

            0x0C => {
                // SAFETY: edge is a live LeafB1 edge.
                if level > 1 && !unsafe { crate::get::decode_matches(&*edge, key, 1, level) } {
                    // SAFETY: edge is a live LeafB1 edge.
                    let pop = unsafe { (*edge).pop0(1) + 1 };
                    path.clear();
                    // SAFETY: split_skip maintains tree invariants.
                    unsafe { split_skip(a, &mut *edge, key, level, pop) };
                    continue;
                }
                a.assert_bracketed();
                // SAFETY: edge points to a live LeafBitmap1 node.
                let node = unsafe { &mut *(*edge).node_ptr().cast::<LeafBitmap1>() };
                if !node.bitmap.set(digit(key, 1)) {
                    return false;
                }
                // SAFETY: edge is a live LeafB1 edge.
                let pop = unsafe { (*edge).pop0(1) as usize + 2 };
                if pop == 256 && level == 1 {
                    // SAFETY: edge holds a live LeafBitmap1 node pointer.
                    let ptr = core::ptr::NonNull::new(unsafe {
                        (*edge).node_ptr().cast::<LeafBitmap1>()
                    });
                    // SAFETY: free the LeafBitmap1 and convert to FullExpanse.
                    unsafe {
                        a.free_node(ptr.expect("leaf ptr"));
                        *edge = Edge::NULL;
                        (*edge).set_tag(EdgeType::FullExpanse.as_u8());
                        (*edge).set_pop0(1, 255);
                    }
                    path.clear();
                } else {
                    // SAFETY: pop - 1 <= 254 fits in 8 bits.
                    unsafe { (*edge).set_pop0(1, pop as u64 - 1) };
                    if level == 1 {
                        path.prefix = key >> 8;
                        // SAFETY: edge is a live LeafB1 edge.
                        path.leaf = unsafe { (*edge).node_ptr().cast::<LeafBitmap1>() };
                        path.leaf1 = core::ptr::null_mut();
                        path.terminal_pop = pop as u16;
                        path.edges[0] = edge;
                        path.levels[0] = 1;
                        path.depth = 1;
                        path.pending_pop = 0;
                    }
                }
                // SAFETY: ancestors contains valid parent edges.
                unsafe {
                    for &(anc, al) in ancestors.iter().take(anc_depth) {
                        bump_pop0(anc, al, 1);
                        path.record_ancestor(anc, al);
                    }
                }
                return true;
            }

            0x05 => {
                // SAFETY: edge is a live Leaf1 linear leaf.
                let pop = unsafe { (*edge).pop0(1) as usize + 1 };
                // SAFETY: edge is a live Leaf1 linear leaf.
                if level > 1 && !unsafe { crate::get::decode_matches(&*edge, key, 1, level) } {
                    // SAFETY: split_skip maintains valid tree invariants.
                    unsafe { split_skip(a, &mut *edge, key, level, pop as u64) };
                    continue;
                }
                let k = key & 0xFF;
                // SAFETY: edge points to live linear leaf allocation.
                let base = unsafe { (*edge).node_ptr() };
                let pos = if pop > 0 {
                    // SAFETY: base has at least pop keys; reading slot pop - 1 is in-bounds.
                    let last = unsafe { *base.add(pop - 1) as u64 };
                    if k > last {
                        pop
                    } else if k == last {
                        return false;
                    } else {
                        // SAFETY: base holds pop sorted 1-byte keys.
                        match unsafe { leaf::locate_fixed::<1>(base, pop, k) } {
                            Ok(_) => return false,
                            Err(p) => p,
                        }
                    }
                } else {
                    0
                };
                let cap = LEAF1_CAP;
                if pop < cap && leaf::cap_class(pop + 1) == leaf::cap_class(pop) {
                    a.assert_bracketed();
                    // SAFETY: base has spare class capacity; set_insert_at_fixed shifts in-place.
                    unsafe {
                        leaf::set_insert_at_fixed::<1>(base, pop, pos, k);
                        (*edge).set_pop0(1, pop as u64);
                    }
                    if level == 1 {
                        path.prefix = key >> 8;
                        path.leaf = core::ptr::null_mut();
                        path.leaf1 = base;
                        path.terminal_pop = (pop + 1) as u16;
                        path.edges[0] = edge;
                        path.levels[0] = 1;
                        path.depth = 1;
                        path.pending_pop = 0;
                    } else {
                        path.clear();
                    }
                    // SAFETY: ancestors contains valid parent edges.
                    unsafe {
                        for &(anc, al) in ancestors.iter().take(anc_depth) {
                            bump_pop0(anc, al, 1);
                            path.record_ancestor(anc, al);
                        }
                    }
                    return true;
                }
                let old_ptr = base;
                let old_size = leaf::size_set(1, pop);
                // SAFETY: edge is a live linear leaf edge.
                let saved_aux = unsafe { *(*edge).aux_bytes() };
                if pop < cap {
                    let new = a.alloc_bytes(leaf::size_set(1, pop + 1));
                    // SAFETY: copy pop keys around pos into new, set new node, free old allocation.
                    unsafe {
                        leaf::set_realloc_insert_fixed::<1>(base, new.as_ptr(), pop, pos, k);
                        *edge = Edge::new_node(new.as_ptr(), (*edge).tag_byte());
                        (*edge).set_aux_bytes(saved_aux);
                        (*edge).set_pop0(1, pop as u64);
                        a.free_bytes(
                            core::ptr::NonNull::new(old_ptr).expect("leaf ptr"),
                            old_size,
                        );
                    }
                    if level == 1 {
                        path.prefix = key >> 8;
                        path.leaf = core::ptr::null_mut();
                        path.leaf1 = new.as_ptr();
                        path.terminal_pop = (pop + 1) as u16;
                        path.edges[0] = edge;
                        path.levels[0] = 1;
                        path.depth = 1;
                        path.pending_pop = 0;
                    } else {
                        path.clear();
                    }
                    // SAFETY: ancestors contains valid parent edges.
                    unsafe {
                        for &(anc, al) in ancestors.iter().take(anc_depth) {
                            bump_pop0(anc, al, 1);
                            path.record_ancestor(anc, al);
                        }
                    }
                    return true;
                }
                path.clear();
                // SAFETY: edge is a live linear leaf edge of pop keys.
                let mut keys = unsafe { leaf_keys(&*edge, 1, pop) };
                keys.insert(pos, k);
                if level > 1 {
                    // SAFETY: edge is a live linear leaf edge.
                    let prefix = unsafe { decode_value(&*edge, 1, level) } << 8;
                    for k in &mut keys {
                        *k |= prefix;
                    }
                }
                let ptr = a.alloc_node_zeroed::<LeafBitmap1>();
                // SAFETY: populate fresh LeafBitmap1 and install into edge.
                unsafe {
                    for &k in &keys {
                        (*ptr.as_ptr()).bitmap.set(k as u8);
                    }
                    *edge = Edge::new_node(ptr.as_ptr().cast(), EdgeType::LeafB1.as_u8());
                    (*edge).set_pop0(1, keys.len() as u64 - 1);
                    restore_decode(&mut *edge, 1, level, &saved_aux);
                }
                if level == 1 {
                    path.prefix = key >> 8;
                    path.leaf = ptr.as_ptr();
                    path.leaf1 = core::ptr::null_mut();
                    path.terminal_pop = keys.len() as u16;
                    path.edges[0] = edge;
                    path.levels[0] = 1;
                    path.depth = 1;
                    path.pending_pop = 0;
                }
                // SAFETY: free old linear leaf and update ancestors.
                unsafe {
                    a.free_bytes(
                        core::ptr::NonNull::new(old_ptr).expect("leaf ptr"),
                        old_size,
                    );
                    for &(anc, al) in ancestors.iter().take(anc_depth) {
                        bump_pop0(anc, al, 1);
                        path.record_ancestor(anc, al);
                    }
                }
                return true;
            }

            0x06..=0x0B => {
                path.clear();
                let kb = tag - 0x04;
                debug_assert!(kb <= level);
                // SAFETY: edge is a live linear leaf edge.
                let pop = unsafe { (*edge).pop0(kb) as usize + 1 };
                // SAFETY: edge is a live linear leaf edge.
                if kb < level && !unsafe { crate::get::decode_matches(&*edge, key, kb, level) } {
                    // SAFETY: split_skip maintains valid tree invariants.
                    unsafe { split_skip(a, &mut *edge, key, level, pop as u64) };
                    continue;
                }
                let k = key_low(key, kb);
                // SAFETY: edge points to live linear leaf allocation.
                let base = unsafe { (*edge).node_ptr() };
                let pos = if pop > 0 {
                    // SAFETY: base has at least pop keys; reading slot pop - 1 is in-bounds.
                    let last = unsafe { read_packed(base, pop - 1, kb as usize) };
                    if k > last {
                        pop
                    } else if k == last {
                        return false;
                    } else {
                        // SAFETY: base holds pop sorted keys of kb bytes.
                        match unsafe { leaf_locate_fixed(base, pop, kb, k) } {
                            Ok(_) => return false,
                            Err(p) => p,
                        }
                    }
                } else {
                    0
                };
                let cap = LEAF_CAP;
                if pop < cap && leaf::cap_class(pop + 1) == leaf::cap_class(pop) {
                    a.assert_bracketed();
                    // SAFETY: base has spare class capacity; set_insert_at shifts in-place.
                    unsafe {
                        leaf::set_insert_at(base, kb, pop, pos, k);
                        (*edge).set_pop0(kb, pop as u64);
                    }
                    path.clear();
                    // SAFETY: ancestors contains valid parent edges.
                    unsafe {
                        for &(anc, al) in ancestors.iter().take(anc_depth) {
                            bump_pop0(anc, al, 1);
                            path.record_ancestor(anc, al);
                        }
                    }
                    return true;
                }
                let old_ptr = base;
                let old_size = leaf::size_set(kb, pop);
                // SAFETY: edge is a live linear leaf edge.
                let saved_aux = unsafe { *(*edge).aux_bytes() };
                if pop < cap {
                    let new = a.alloc_bytes(leaf::size_set(kb, pop + 1));
                    // SAFETY: copy pop keys around pos into new, set new node, free old allocation.
                    unsafe {
                        leaf::set_realloc_insert(base, new.as_ptr(), kb, pop, pos, k);
                        *edge = Edge::new_node(new.as_ptr(), (*edge).tag_byte());
                        (*edge).set_aux_bytes(saved_aux);
                        (*edge).set_pop0(kb, pop as u64);
                        a.free_bytes(
                            core::ptr::NonNull::new(old_ptr).expect("leaf ptr"),
                            old_size,
                        );
                    }
                    path.clear();
                    // SAFETY: ancestors contains valid parent edges.
                    unsafe {
                        for &(anc, al) in ancestors.iter().take(anc_depth) {
                            bump_pop0(anc, al, 1);
                            path.record_ancestor(anc, al);
                        }
                    }
                    return true;
                }
                // SAFETY: edge is a live linear leaf edge of pop keys.
                let mut keys = unsafe { leaf_keys(&*edge, kb, pop) };
                keys.insert(pos, k);
                if kb < level {
                    // SAFETY: edge is a live linear leaf edge.
                    let prefix = unsafe { decode_value(&*edge, kb, level) } << (8 * u32::from(kb));
                    for k in &mut keys {
                        *k |= prefix;
                    }
                }
                if keys.len() <= cap {
                    // SAFETY: build_leaf allocates a fresh linear leaf of keys.len() keys.
                    unsafe {
                        build_leaf(a, &mut *edge, kb, &keys);
                        restore_decode(&mut *edge, kb, level, &saved_aux);
                    }
                } else if kb == 1 {
                    let ptr = a.alloc_node_zeroed::<LeafBitmap1>();
                    // SAFETY: populate fresh LeafBitmap1 and install into edge.
                    unsafe {
                        for &k in &keys {
                            (*ptr.as_ptr()).bitmap.set(k as u8);
                        }
                        *edge = Edge::new_node(ptr.as_ptr().cast(), EdgeType::LeafB1.as_u8());
                        (*edge).set_pop0(1, keys.len() as u64 - 1);
                        restore_decode(&mut *edge, 1, level, &saved_aux);
                    }
                } else {
                    let d = divergence_level(keys[0], keys[keys.len() - 1], level);
                    if d == 1 {
                        let ptr = a.alloc_node_zeroed::<LeafBitmap1>();
                        // SAFETY: populate fresh LeafBitmap1 and install into edge.
                        unsafe {
                            for &k in &keys {
                                (*ptr.as_ptr()).bitmap.set(k as u8);
                            }
                            *edge = Edge::new_node(ptr.as_ptr().cast(), EdgeType::LeafB1.as_u8());
                            (*edge).set_pop0(1, keys.len() as u64 - 1);
                            write_decode(&mut *edge, 1, level, keys[0]);
                        }
                        if level == 1 {
                            path.prefix = key >> 8;
                            path.leaf = ptr.as_ptr();
                            path.leaf1 = core::ptr::null_mut();
                            path.terminal_pop = keys.len() as u16;
                            path.edges[0] = edge;
                            path.levels[0] = 1;
                            path.depth = 1;
                            path.pending_pop = 0;
                        }
                    } else {
                        let bl = d;
                        let ptr = a.alloc_node_zeroed::<BranchL3>();
                        // SAFETY: initialize fresh BranchL3 and re-insert keys.
                        unsafe {
                            (*ptr.as_ptr()).hdr.level = bl;
                            *edge = Edge::new_node(ptr.as_ptr().cast(), EdgeType::BranchL3.as_u8());
                            if bl < level {
                                write_decode(&mut *edge, bl, level, keys[0]);
                            }
                            for &k in &keys {
                                let ins = insert_with_path_flat(a, edge, k, level, path);
                                debug_assert!(ins);
                            }
                            (*edge).set_pop0(bl, keys.len() as u64 - 1);
                        }
                    }
                }
                // SAFETY: free old linear leaf and update ancestors.
                unsafe {
                    a.free_bytes(
                        core::ptr::NonNull::new(old_ptr).expect("leaf ptr"),
                        old_size,
                    );
                    for &(anc, al) in ancestors.iter().take(anc_depth) {
                        bump_pop0(anc, al, 1);
                        path.record_ancestor(anc, al);
                    }
                }
                return true;
            }

            0x7F => return false,

            _ => {
                let im = ImmedType::from_u8(tag).expect("valid immediate tag");
                debug_assert_eq!(im.key_bytes(), level);
                path.clear();
                let kb = im.key_bytes();
                let k = key_low(key, kb);
                let n = im.key_count() as usize;
                let kb_usize = kb as usize;
                if n == 1 {
                    let mask = if kb >= 8 {
                        u64::MAX
                    } else {
                        (1u64 << (kb * 8)) - 1
                    };
                    // SAFETY: live 1-key immediate edge; read word0.
                    let existing_k = unsafe { (*edge).word0() } & mask;
                    if existing_k == k {
                        return false;
                    }
                    if ImmedType::max_count(kb) >= 2 {
                        let (k0, k1) = if k < existing_k {
                            (k, existing_k)
                        } else {
                            (existing_k, k)
                        };
                        let mut new_payload = [0u8; 15];
                        // SAFETY: new_payload has 15 bytes; 2 keys fit per max_count >= 2.
                        unsafe {
                            write_packed(new_payload.as_mut_ptr(), 0, kb_usize, k0);
                            write_packed(new_payload.as_mut_ptr(), 1, kb_usize, k1);
                        }
                        let mut w0 = [0u8; 8];
                        w0.copy_from_slice(&new_payload[..8]);
                        let mut aux = [0u8; 7];
                        aux.copy_from_slice(&new_payload[8..]);
                        let new_im = ImmedType::new(kb, 2).expect("immediate capacity");
                        // SAFETY: edge is live; ancestors array is valid.
                        unsafe {
                            (*edge).set_imm_bytes(w0);
                            (*edge).set_aux_bytes(aux);
                            (*edge).set_tag(new_im.as_u8());
                            for &(anc, al) in ancestors.iter().take(anc_depth) {
                                bump_pop0(anc, al, 1);
                                path.record_ancestor(anc, al);
                            }
                        }
                        return true;
                    }
                }
                // SAFETY: live immediate edge.
                let payload = unsafe { (*edge).imm_payload() };
                // SAFETY: payload has 15 bytes; n keys of kb bytes.
                let pos = match unsafe { leaf::locate(payload.as_ptr(), n, kb, k) } {
                    Ok(_) => return false,
                    Err(p) => p,
                };
                if n < ImmedType::max_count(kb) as usize {
                    let mut new_payload = payload;
                    if pos < n {
                        new_payload.copy_within(pos * kb_usize..n * kb_usize, (pos + 1) * kb_usize);
                    }
                    // SAFETY: new_payload has 15 bytes; pos <= n; (n + 1) * kb_usize <= 15.
                    unsafe {
                        write_packed(new_payload.as_mut_ptr(), pos, kb_usize, k);
                    }
                    let mut w0 = [0u8; 8];
                    w0.copy_from_slice(&new_payload[..8]);
                    let mut aux = [0u8; 7];
                    aux.copy_from_slice(&new_payload[8..15]);
                    let new_im = ImmedType::new(kb, (n + 1) as u8).expect("immediate capacity");
                    // SAFETY: edge is a live valid edge; ancestors array is valid.
                    unsafe {
                        (*edge).set_imm_bytes(w0);
                        (*edge).set_aux_bytes(aux);
                        (*edge).set_tag(new_im.as_u8());
                        for &(anc, al) in ancestors.iter().take(anc_depth) {
                            bump_pop0(anc, al, 1);
                            path.record_ancestor(anc, al);
                        }
                    }
                    return true;
                }
                // Overflow immediate capacity -> build linear leaf.
                let ptr = a.alloc_bytes(leaf::size_set(kb, n + 1));
                let base = ptr.as_ptr();
                // SAFETY: ptr is freshly allocated with size for n + 1 keys; payload has n keys.
                unsafe {
                    if pos > 0 {
                        core::ptr::copy_nonoverlapping(payload.as_ptr(), base, pos * kb_usize);
                    }
                    write_packed(base, pos, kb_usize, k);
                    if pos < n {
                        core::ptr::copy_nonoverlapping(
                            payload.as_ptr().add(pos * kb_usize),
                            base.add((pos + 1) * kb_usize),
                            (n - pos) * kb_usize,
                        );
                    }
                    *edge = Edge::new_node(base, EdgeType::Leaf1 as u8 + (kb - 1));
                    (*edge).set_pop0(kb, n as u64);
                    for &(anc, al) in ancestors.iter().take(anc_depth) {
                        bump_pop0(anc, al, 1);
                        path.record_ancestor(anc, al);
                    }
                }
                return true;
            }
        }
    }
}

/// Fallback version-bracketed recursive descent for OCC-enabled concurrent sets.
unsafe fn insert_with_path_occ<const OCC: bool>(
    a: &NodeAlloc,
    edge: &mut Edge,
    key: Key,
    mut level: u8,
    path: &mut InsertPath,
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
                    path.clear();
                    continue;
                }
                path.clear();
                write_immed(edge, level, &[key_low(key, level)]);
                return true;
            }

            EdgeTag::Immed(im) => {
                debug_assert_eq!(im.key_bytes(), level);
                path.clear();
                let kb = im.key_bytes();
                let k = key_low(key, kb);
                let n = im.key_count() as usize;
                let kb_usize = kb as usize;
                let payload = edge.imm_payload();
                if n == 1 {
                    // SAFETY: payload has 15 bytes; slot 0 is in-bounds.
                    let existing_k = unsafe { read_packed(payload.as_ptr(), 0, kb_usize) };
                    if existing_k == k {
                        return false;
                    }
                    if ImmedType::max_count(kb) >= 2 {
                        let mut new_payload = [0u8; 16];
                        let (k0, k1) = if k < existing_k {
                            (k, existing_k)
                        } else {
                            (existing_k, k)
                        };
                        // SAFETY: new_payload has 16 bytes; 2 keys of kb <= 7 bytes fit per max_count >= 2.
                        unsafe {
                            write_packed(new_payload.as_mut_ptr(), 0, kb_usize, k0);
                            write_packed(new_payload.as_mut_ptr(), 1, kb_usize, k1);
                        }
                        let mut w0 = [0u8; 8];
                        w0.copy_from_slice(&new_payload[..8]);
                        let mut aux = [0u8; 7];
                        aux.copy_from_slice(&new_payload[8..15]);
                        let new_im = ImmedType::new(kb, 2).expect("immediate capacity");
                        edge.set_imm_bytes(w0);
                        edge.set_aux_bytes(aux);
                        edge.set_tag(new_im.as_u8());
                        return true;
                    }
                }
                // SAFETY: payload has 16 bytes; n keys of kb bytes.
                let pos = match unsafe { leaf::locate(payload.as_ptr(), n, kb, k) } {
                    Ok(_) => return false,
                    Err(p) => p,
                };
                if n < ImmedType::max_count(kb) as usize {
                    let mut new_payload = payload;
                    if pos < n {
                        new_payload.copy_within(pos * kb_usize..n * kb_usize, (pos + 1) * kb_usize);
                    }
                    // SAFETY: new_payload has 16 bytes; pos <= n; (n + 1) * kb_usize <= 15.
                    unsafe {
                        write_packed(new_payload.as_mut_ptr(), pos, kb_usize, k);
                    }
                    let mut w0 = [0u8; 8];
                    w0.copy_from_slice(&new_payload[..8]);
                    let mut aux = [0u8; 7];
                    aux.copy_from_slice(&new_payload[8..15]);
                    let new_im = ImmedType::new(kb, (n + 1) as u8).expect("immediate capacity");
                    edge.set_imm_bytes(w0);
                    edge.set_aux_bytes(aux);
                    edge.set_tag(new_im.as_u8());
                    return true;
                }
                // Overflow immediate capacity -> build linear leaf.
                let mut keys = StackKeys32::new();
                for i in 0..pos {
                    // SAFETY: i < pos <= n <= 7; payload has 15 bytes.
                    let ki = unsafe { read_packed(payload.as_ptr(), i, kb_usize) };
                    keys.push(ki);
                }
                keys.push(k);
                for i in pos..n {
                    // SAFETY: i < n <= 7; payload has 15 bytes.
                    let ki = unsafe { read_packed(payload.as_ptr(), i, kb_usize) };
                    keys.push(ki);
                }
                build_leaf(a, edge, kb, keys.as_slice());
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
                path.clear();
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
                let pos = if pop > 0 {
                    // SAFETY: pop > 0 guarantees slot pop - 1 is in-bounds.
                    let last = unsafe { read_packed(base, pop - 1, kb as usize) };
                    if k > last {
                        pop
                    } else if k == last {
                        return false;
                    } else {
                        // SAFETY: live leaf of `pop` keys per contract.
                        match unsafe { leaf::locate(base, pop, kb, k) } {
                            Ok(_) => return false,
                            Err(p) => p,
                        }
                    }
                } else {
                    0
                };
                let cap = if kb == 1 { LEAF1_CAP } else { LEAF_CAP };
                if pop < cap && leaf::cap_class(pop + 1) == leaf::cap_class(pop) {
                    // Fast path: spare class capacity — shift in place, no
                    // reallocation, no key materialization.
                    // SAFETY: class capacity spare per the check.
                    unsafe { leaf::set_insert_at(base, kb, pop, pos, k) };
                    edge.set_pop0(kb, pop as u64);
                    path.clear();
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
                    path.clear();
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
                        path.clear();
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
                            let ins = unsafe { insert_with_path::<OCC>(a, edge, k, level, path) };
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
                    path.clear();
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
                    path.clear();
                } else {
                    edge.set_pop0(1, pop as u64 - 1);
                    path.clear();
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
                    path.clear();
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
                        let num = b.hdr.num as usize;
                        let found = if num >= 1 && b.hdr.digits[0] == d {
                            Some(0)
                        } else if num >= 2 && b.hdr.digits[1] == d {
                            Some(1)
                        } else if num >= 3 && b.hdr.digits[2] == d {
                            Some(2)
                        } else {
                            None
                        };
                        (found, num)
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
                            let r =
                                insert_with_path::<OCC>(a, &mut b.edges[slot], key, bl - 1, path);
                            crate::occ::version_end_if::<OCC>(a, &mut b.hdr.version);
                            r
                        } else {
                            let b = &mut *edge.node_ptr().cast::<BranchL7>();
                            crate::occ::version_begin_if::<OCC>(a, &mut b.hdr.version);
                            let r =
                                insert_with_path::<OCC>(a, &mut b.edges[slot], key, bl - 1, path);
                            crate::occ::version_end_if::<OCC>(a, &mut b.hdr.version);
                            r
                        }
                    };
                    if inserted {
                        // SAFETY: edge is a valid live edge.
                        unsafe { bump_pop0(edge, bl, 1) };
                        path.record_ancestor(edge as *mut Edge, level);
                    }
                    return inserted;
                }
                let cap = if is_l3 { BRANCH_L3_CAP } else { BRANCH_L7_CAP };
                if num == cap {
                    path.clear();
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
                        let slot = linear_insert_slot_l3(&mut b.hdr.digits, &mut b.edges, num, d);
                        b.hdr.num += 1;
                        b.hdr.add_presence(d);
                        let r = insert_with_path::<OCC>(a, &mut b.edges[slot], key, bl - 1, path);
                        crate::occ::version_end_if::<OCC>(a, &mut b.hdr.version);
                        r
                    } else {
                        let b = &mut *edge.node_ptr().cast::<BranchL7>();
                        crate::occ::version_begin_if::<OCC>(a, &mut b.hdr.version);
                        let slot = linear_insert_slot(&mut b.hdr.digits, &mut b.edges, num, d);
                        b.hdr.num += 1;
                        b.hdr.add_presence(d);
                        let r = insert_with_path::<OCC>(a, &mut b.edges[slot], key, bl - 1, path);
                        crate::occ::version_end_if::<OCC>(a, &mut b.hdr.version);
                        r
                    }
                };
                debug_assert!(inserted);
                // SAFETY: edge is a valid live edge.
                unsafe { bump_pop0(edge, bl, 1) };
                path.record_ancestor(edge as *mut Edge, level);
                return true;
            }

            EdgeTag::Structural(EdgeType::BranchB) => {
                debug_assert!(level >= 2);
                // SAFETY: live branch per contract.
                let bl = unsafe { branch_form_level(edge, EdgeType::BranchB, level) };
                if bl < level && !crate::get::decode_matches(edge, key, bl, level) {
                    let pop = edge.pop0(bl) + 1;
                    path.clear();
                    split_skip(a, edge, key, level, pop);
                    continue;
                }
                let slot_level = level;
                let d = digit(key, bl);
                // SAFETY: live BranchB per contract.
                let b = unsafe { &mut *edge.node_ptr().cast::<BranchB>() };
                if let Some(slot) = b.bitmap.test_and_subexpanse_rank(d) {
                    let sub = b.subarrays[(d >> 5) as usize];
                    // SAFETY: bitmap/subarray consistency invariant. The
                    // node's version brackets the descent (see the L3 arm).
                    crate::occ::version_begin_if::<OCC>(a, &mut b.version);
                    // SAFETY: bitmap/subarray consistency invariant.
                    let inserted = unsafe {
                        insert_with_path::<OCC>(a, &mut *sub.add(slot), key, bl - 1, path)
                    };
                    crate::occ::version_end_if::<OCC>(a, &mut b.version);
                    if inserted {
                        // SAFETY: edge is a valid live edge.
                        unsafe { bump_pop0(edge, bl, 1) };
                        path.record_ancestor(edge as *mut Edge, level);
                    }
                    return inserted;
                }
                if b.bitmap.count() as usize + 1 > BRANCHB_UP {
                    if bl < slot_level {
                        // BranchU has no header level, so it cannot skip:
                        // materialize the level just above the form and retry.
                        let pop = edge.pop0(bl) + 1;
                        path.clear();
                        wrap_skip_level(a, edge, bl + 1, slot_level, pop);
                        level = slot_level;
                        continue;
                    }
                    path.clear();
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
                let inserted = unsafe {
                    insert_with_path::<OCC>(a, &mut *b.subarrays[sub].add(rank), key, bl - 1, path)
                };
                crate::occ::version_end_if::<OCC>(a, &mut b.version);
                debug_assert!(inserted);
                // SAFETY: edge is a valid live edge.
                unsafe { bump_pop0(edge, bl, 1) };
                path.record_ancestor(edge as *mut Edge, level);
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
                let inserted = unsafe {
                    insert_with_path::<OCC>(a, &mut b.edges[d as usize], key, level - 1, path)
                };
                crate::occ::version_end_if::<OCC>(a, &mut b.version);
                if inserted {
                    // SAFETY: edge is a valid live edge.
                    unsafe { bump_pop0(edge, level, 1) };
                    path.record_ancestor(edge as *mut Edge, level);
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
            let n = im.key_count() as usize;
            let payload = edge.imm_payload();
            if n == 1 {
                // SAFETY: single-key immediate payload has 1 key of kb bytes.
                let existing_k = unsafe { read_packed(payload.as_ptr(), 0, kb as usize) };
                if existing_k == k {
                    *edge = Edge::NULL;
                    return true;
                }
                return false;
            }
            // SAFETY: payload holds n packed keys of kb bytes.
            let pos = match unsafe { leaf::locate(payload.as_ptr(), n, kb, k) } {
                Ok(p) => p,
                Err(_) => return false,
            };
            let mut new_payload = payload;
            let kb_usize = kb as usize;
            new_payload.copy_within((pos + 1) * kb_usize..n * kb_usize, pos * kb_usize);
            new_payload[((n - 1) * kb_usize)..].fill(0);
            let new_im = ImmedType::new(kb, (n - 1) as u8).expect("immediate capacity");
            let mut w0 = [0u8; 8];
            w0.copy_from_slice(&new_payload[..8]);
            let mut aux = [0u8; 7];
            aux.copy_from_slice(&new_payload[8..15]);
            edge.set_imm_bytes(w0);
            edge.set_aux_bytes(aux);
            edge.set_tag(new_im.as_u8());
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
            let pos = match unsafe { leaf::locate(base, pop, kb, k) } {
                Ok(pos) => pos,
                Err(_) => return false,
            };
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
            // Slow path (conversion to immediate or null).
            let old_ptr = base;
            let old_size = leaf::size_set(kb, pop);
            if pop == 1 {
                // SAFETY: old leaf allocation no longer referenced.
                unsafe {
                    a.free_bytes(
                        core::ptr::NonNull::new(old_ptr).expect("leaf ptr"),
                        old_size,
                    );
                }
                *edge = Edge::NULL;
                return true;
            }
            let rem_pop = pop - 1;
            let dv = if kb < level {
                decode_value(edge, kb, level)
            } else {
                0
            };
            let mut entries = [0u64; 16];
            let mut idx = 0;
            for slot in 0..pop {
                if slot == pos {
                    continue;
                }
                // SAFETY: live leaf of `pop` keys per contract.
                let low_k = unsafe { read_packed(base, slot, kb as usize) };
                let k = (dv << (8 * u32::from(kb))) | low_k;
                entries[idx] = k;
                idx += 1;
            }
            write_immed(edge, level, &entries[..rem_pop]);
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
            // Hysteresis: convert back to a linear leaf when pop drops below the floor.
            if pop < LEAFB1_DOWN {
                let mut keys = StackKeys32::new();
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
                if keys.len < ImmedType::max_count(level) as usize {
                    // Absorb any decode bytes into full slot-level keys.
                    let mut full = StackKeys32::new();
                    for &low in keys.as_slice() {
                        full.push((dv << 8) | low);
                    }
                    write_immed(edge, level, full.as_slice());
                } else {
                    build_leaf(a, edge, 1, keys.as_slice());
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
            let (removed, child_null) = unsafe {
                if is_l3 {
                    let b = &mut *edge.node_ptr().cast::<BranchL3>();
                    let Some(slot) = b.hdr.find(d) else {
                        return false;
                    };
                    crate::occ::version_begin_if::<OCC>(a, &mut b.hdr.version);
                    let r = remove::<OCC>(a, &mut b.edges[slot], key, bl - 1);
                    let child_null = r && b.edges[slot].is_null();
                    if child_null {
                        linear_remove_slot(
                            &mut b.hdr.digits,
                            &mut b.edges,
                            b.hdr.num as usize,
                            slot,
                        );
                        b.hdr.num -= 1;
                        b.hdr.refresh_presence();
                    }
                    crate::occ::version_end_if::<OCC>(a, &mut b.hdr.version);
                    (r, child_null)
                } else {
                    let b = &mut *edge.node_ptr().cast::<BranchL7>();
                    let Some(slot) = b.hdr.find(d) else {
                        return false;
                    };
                    crate::occ::version_begin_if::<OCC>(a, &mut b.hdr.version);
                    let r = remove::<OCC>(a, &mut b.edges[slot], key, bl - 1);
                    let child_null = r && b.edges[slot].is_null();
                    if child_null {
                        linear_remove_slot(
                            &mut b.hdr.digits,
                            &mut b.edges,
                            b.hdr.num as usize,
                            slot,
                        );
                        b.hdr.num -= 1;
                        b.hdr.refresh_presence();
                    }
                    crate::occ::version_end_if::<OCC>(a, &mut b.hdr.version);
                    (r, child_null)
                }
            };
            if !removed {
                return false;
            }
            if child_null {
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
            } else {
                // SAFETY: edge is a valid live edge.
                unsafe { bump_pop0(edge, bl, -1) };
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
            let Some(rank) = b.bitmap.test_and_subexpanse_rank(d) else {
                return false;
            };
            let sub = (d >> 5) as usize;
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
            if child_null {
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
                // SAFETY: edge is a valid live edge.
                unsafe { bump_pop0(edge, bl, -1) };
                if digits < BRANCH_L7_CAP {
                    // Hysteresis: B → L7 one index below the L7 capacity.
                    // SAFETY: rebuild keeps the subtree owned.
                    unsafe { downgrade_b_to_l7(a, edge) };
                }
            } else {
                // SAFETY: edge is a valid live edge.
                unsafe { bump_pop0(edge, bl, -1) };
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
            let child_is_null = b.edges[d as usize].is_null();
            crate::occ::version_end_if::<OCC>(a, &mut b.version);
            if !removed {
                return false;
            }
            if child_is_null {
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
                // SAFETY: edge is a valid live edge.
                unsafe { bump_pop0(edge, level, -1) };
                if digits < BRANCHB_UP {
                    // Hysteresis: U → B one index below the U threshold.
                    // SAFETY: rebuild keeps the subtree owned.
                    unsafe { downgrade_u_to_b(a, edge, level) };
                }
            } else {
                // SAFETY: edge is a valid live edge.
                unsafe { bump_pop0(edge, level, -1) };
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
            (*new.as_ptr()).hdr.add_presence(dig);
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
                // Multi-key map immediates own a class-sized value array in word 0.
                if MAP && im.key_count() > 1 {
                    a.free_bytes(
                        core::ptr::NonNull::new(edge.node_ptr()).unwrap(),
                        crate::mutate_map::map_immed_val_size(im.key_count() as usize),
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

#[cfg(test)]
mod tests {
    use super::*;
    use core::ptr::NonNull;
    use std::sync::Arc;

    struct TestAllocGuard<'a> {
        alloc: &'a NodeAlloc,
        ptr: NonNull<u8>,
        size: usize,
    }

    impl Drop for TestAllocGuard<'_> {
        fn drop(&mut self) {
            // SAFETY: ptr was allocated with size on alloc and is freed once on drop/unwind.
            unsafe { self.alloc.free_bytes(self.ptr, self.size) };
        }
    }

    /// Negative control (§2.3, #479): exercises `a.assert_bracketed()` at
    /// `insert_with_path_flat` (case 0x05, Leaf1 in-place shift).
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "node interior mutated outside any version bracket")]
    fn negative_control_flat_leaf1_inplace_panics_unbracketed() {
        let alloc = NodeAlloc::new();
        alloc.defer_to(Arc::new(crate::occ::Collector::new()));

        let size = leaf::size_set(1, 3);
        let ptr = alloc.alloc_bytes(size);
        let _guard = TestAllocGuard {
            alloc: &alloc,
            ptr,
            size,
        };

        // SAFETY: freshly allocated leaf has capacity for 3 entries.
        unsafe {
            let p = ptr.as_ptr();
            *p.add(0) = 0x02;
            *p.add(1) = 0x05;
            *p.add(2) = 0x09;
        }
        let mut edge = Edge::new_node(ptr.as_ptr(), EdgeType::Leaf1.as_u8());
        edge.set_pop0(1, 2); // pop0 = 2 => pop = 3

        let mut path = InsertPath::empty();
        // SAFETY: edge is a valid Leaf1 node.
        unsafe { insert_with_path_flat(&alloc, &mut edge, 0x07, 1, &mut path) };
    }

    /// Positive companion: covered Leaf1 in-place mutation succeeds quietly.
    #[test]
    #[cfg(debug_assertions)]
    fn positive_control_flat_leaf1_inplace_quiet_when_covered() {
        let alloc = NodeAlloc::new();
        alloc.defer_to(Arc::new(crate::occ::Collector::new()));

        let size = leaf::size_set(1, 3);
        let ptr = alloc.alloc_bytes(size);
        let _guard = TestAllocGuard {
            alloc: &alloc,
            ptr,
            size,
        };

        // SAFETY: freshly allocated leaf has capacity for 3 entries.
        unsafe {
            let p = ptr.as_ptr();
            *p.add(0) = 0x02;
            *p.add(1) = 0x05;
            *p.add(2) = 0x09;
        }
        let mut edge = Edge::new_node(ptr.as_ptr(), EdgeType::Leaf1.as_u8());
        edge.set_pop0(1, 2); // pop0 = 2 => pop = 3

        let mut path = InsertPath::empty();
        alloc.bracket_enter();
        // SAFETY: edge is a valid Leaf1 node.
        let inserted = unsafe { insert_with_path_flat(&alloc, &mut edge, 0x07, 1, &mut path) };
        alloc.bracket_leave();
        assert!(inserted);
    }

    /// Negative control (§2.3, #479): exercises `a.assert_bracketed()` at
    /// `insert_with_path_flat` (case 0x06..=0x0B, Leaf2..Leaf7 in-place shift).
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "node interior mutated outside any version bracket")]
    fn negative_control_flat_leaf2_inplace_panics_unbracketed() {
        let alloc = NodeAlloc::new();
        alloc.defer_to(Arc::new(crate::occ::Collector::new()));

        let size = leaf::size_set(2, 3);
        let ptr = alloc.alloc_bytes(size);
        let _guard = TestAllocGuard {
            alloc: &alloc,
            ptr,
            size,
        };

        // SAFETY: freshly allocated leaf has capacity for 3 2-byte entries.
        unsafe {
            let p = ptr.as_ptr();
            write_packed(p, 0, 2, 0x0102);
            write_packed(p, 1, 2, 0x0105);
            write_packed(p, 2, 2, 0x0109);
        }
        let mut edge = Edge::new_node(ptr.as_ptr(), EdgeType::Leaf2.as_u8());
        edge.set_pop0(2, 2); // pop0 = 2 => pop = 3

        let mut path = InsertPath::empty();
        // SAFETY: edge is a valid Leaf2 node.
        unsafe { insert_with_path_flat(&alloc, &mut edge, 0x0107, 2, &mut path) };
    }

    /// Positive companion: covered Leaf2 in-place mutation succeeds quietly.
    #[test]
    #[cfg(debug_assertions)]
    fn positive_control_flat_leaf2_inplace_quiet_when_covered() {
        let alloc = NodeAlloc::new();
        alloc.defer_to(Arc::new(crate::occ::Collector::new()));

        let size = leaf::size_set(2, 3);
        let ptr = alloc.alloc_bytes(size);
        let _guard = TestAllocGuard {
            alloc: &alloc,
            ptr,
            size,
        };

        // SAFETY: freshly allocated leaf has capacity for 3 2-byte entries.
        unsafe {
            let p = ptr.as_ptr();
            write_packed(p, 0, 2, 0x0102);
            write_packed(p, 1, 2, 0x0105);
            write_packed(p, 2, 2, 0x0109);
        }
        let mut edge = Edge::new_node(ptr.as_ptr(), EdgeType::Leaf2.as_u8());
        edge.set_pop0(2, 2); // pop0 = 2 => pop = 3

        let mut path = InsertPath::empty();
        alloc.bracket_enter();
        // SAFETY: edge is a valid Leaf2 node.
        let inserted = unsafe { insert_with_path_flat(&alloc, &mut edge, 0x0107, 2, &mut path) };
        alloc.bracket_leave();
        assert!(inserted);
    }
}
