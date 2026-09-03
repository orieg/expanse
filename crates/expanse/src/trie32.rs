//! Real 32-bit digital-trie engine shared by [`ExpanseSet32`] and
//! [`ExpanseMap32`], per `docs/design/32-bit-embedded.md`.
//!
//! This is a genuine 256-ary digital trie (not a `BTree` wrapper): 32-bit
//! keys are decoded one byte ("digit") per level over four levels
//! (`L4 -> L1`), and each subexpanse is stored in the most compact node
//! flavour its population allows — keys packed *immediately* inside the
//! 8-byte [`Edge32`], packed linear leaves sized by [`cap_class`], a
//! 256-bit bitmap leaf for dense level-1 sets, and linear (`BranchL2_32`,
//! `BranchL6_32`) / uncompressed ([`BranchU32`]) branches that grow and
//! shrink with population.
//!
//! ## Portability note: handles, not raw pointers
//!
//! The RFC describes `Edge32`'s word 0 as a raw child pointer. That is a
//! 32-bit-target notion: a real heap pointer does not fit in a `u32` on the
//! 64-bit hosts where this crate's tests, Miri, and differential fuzzing
//! run. So the engine keeps nodes in a per-tree [`Arena`] and stores a
//! 32-bit **handle** (arena index) in `Edge32::w0` instead. This is
//! pointer-width-independent (identical on `i686`/RV32 and on the 64-bit
//! host), needs no `unsafe`, and — because each node is its own real heap
//! allocation sized to the RFC's on-target byte layout — makes
//! [`Arena::bytes_in_use`] an exact, honest memory figure that
//! `ExpanseSet32::mem_used`/`ExpanseMap32::mem_used` report directly.
//!
//! ## Search kernels are scalar
//!
//! Leaf search is a scalar binary/linear scan over packed little-endian
//! keys, bounded by the population. There is no fixed-width SIMD load, so
//! the CVE-class "SIMD load wider than the allocation" gate that guards the
//! 64-bit leaf kernels (see the 64-bit `leaf::simd_gates_within_cap_class`
//! and PR #225) cannot arise here; the analogous invariant — a leaf
//! allocation is never narrower than the keys it holds — is asserted by
//! `cap_class_never_underallocates`.

use core_alloc::{boxed::Box, vec::Vec};

use core::ops::FnMut;
use core::option::Option::{self, None, Some};

use crate::node32::{
    BranchB32, BranchHeader32, BranchL2_32, BranchL6_32, BranchU32, LeafBitmap1_32, LeafBitmapL_32,
};
use crate::types32::{
    BRANCH_B_DOWN_32, BRANCH_B_TO_UNCOMPRESSED_THRESHOLD_32, BRANCH_L6_CAP_32, BRANCH_L6_DOWN_32,
    BRANCH_U_DOWN_32, Edge32, MAP_BITMAP_ENTER_32, MAP_BITMAP_LEAVE_32, MAP_LEAF_MAX_32,
    SET_BITMAP_ENTER_32, SET_BITMAP_LEAVE_32, SET_LEAF_MAX_32,
};

// ---------------------------------------------------------------------------
// Sizing
// ---------------------------------------------------------------------------

/// Slot-capacity class: linear-leaf allocations are sized in multiples of
/// four slots (populations of one or two stay exact), so the allocation
/// size stays derivable from the population alone. Mirrors the 64-bit
/// `leaf::cap_class`.
#[inline]
#[must_use]
pub(crate) const fn cap_class(pop: usize) -> usize {
    if pop <= 2 { pop } else { (pop + 3) & !3 }
}

/// Allocation size of a set-flavour linear leaf (`key_bytes` per key).
#[inline]
#[must_use]
pub(crate) const fn size_set32(key_bytes: u8, pop: usize) -> usize {
    key_bytes as usize * cap_class(pop)
}

/// Allocation size of a map-flavour linear leaf: `pop` 4-byte values then
/// `pop` packed keys, both areas class-sized.
#[inline]
#[must_use]
pub(crate) const fn size_map32(key_bytes: u8, pop: usize) -> usize {
    4 * cap_class(pop) + key_bytes as usize * cap_class(pop)
}

// ---------------------------------------------------------------------------
// Conversion thresholds (anchored to types32.rs constants per #509)
// ---------------------------------------------------------------------------

/// Immediate (in-edge) key capacity for a subtree whose keys have
/// `key_bytes` undecoded bytes: `floor(7 / key_bytes)`.
#[inline]
const fn set_immed_cap(kb: u8) -> usize {
    7 / kb as usize
}

/// A level-`>=2` set linear leaf converts to a branch above this
/// population.
const SET_LEAF_MAX: usize = SET_LEAF_MAX_32;
/// A level-1 set linear leaf converts to a bitmap leaf above this
/// population (bitmap leaf is 64 B, so it only wins once the linear leaf
/// would exceed it).
const SET_BITMAP_ENTER: usize = SET_BITMAP_ENTER_32;
/// A level-1 set bitmap leaf demotes back to a linear leaf at or below this
/// population (hysteresis gap vs. [`SET_BITMAP_ENTER`]).
const SET_BITMAP_LEAVE: usize = SET_BITMAP_LEAVE_32;
/// A level-`>=2` map linear leaf converts to a branch above this
/// population.
const MAP_LEAF_MAX: usize = MAP_LEAF_MAX_32;
/// A level-1 map linear leaf converts to a bitmap leaf above this
/// population.
const MAP_BITMAP_ENTER: usize = MAP_BITMAP_ENTER_32;
/// A level-1 map bitmap leaf demotes back to a linear leaf at or below this
/// population (hysteresis gap vs. [`MAP_BITMAP_ENTER`]).
const MAP_BITMAP_LEAVE: usize = MAP_BITMAP_LEAVE_32;

/// Maximum child capacity of `BranchL6_32`.
const BRANCH_L6_CAP: usize = BRANCH_L6_CAP_32;
/// Demote `BranchL6_32` to `BranchL2_32` when child count <= this (band of 1).
const BRANCH_L6_DOWN: usize = BRANCH_L6_DOWN_32;
/// `BranchB32` promotes to `BranchU32` when child count > this.
const BRANCH_B_TO_UNCOMPRESSED: usize = BRANCH_B_TO_UNCOMPRESSED_THRESHOLD_32;
/// Demote `BranchB32` to `BranchL6_32` when child count <= this (band of 1).
const BRANCH_B_DOWN: usize = BRANCH_B_DOWN_32;
/// Demote `BranchU32` to `BranchB32` when child count <= this (band of 2).
const BRANCH_U_DOWN: usize = BRANCH_U_DOWN_32;

// ---------------------------------------------------------------------------
// Edge tag scheme (raw tag byte, private to the engine)
// ---------------------------------------------------------------------------

const T_NULL: u8 = 0;
const T_L2: u8 = 1;
const T_L6: u8 = 2;
const T_U: u8 = 3;
const T_BITMAP: u8 = 4;
// set linear leaf: 5..=8 (key_bytes 1..=4)
// map linear leaf: 9..=12 (key_bytes 1..=4)
const T_SET_LEAF_BASE: u8 = 4;
const T_MAP_LEAF_BASE: u8 = 8;
const T_B: u8 = 13;
const T_MAP_BITMAP: u8 = 14;
// set immediate: 0x40 | ((kb-1) << 3) | (count-1)
const T_SET_IMMED_BASE: u8 = 0x40;
// map immediate (single entry): 0x60 | (kb-1), kb in 1..=3
const T_MAP_IMMED_BASE: u8 = 0x60;

#[inline]
const fn t_set_leaf(kb: u8) -> u8 {
    T_SET_LEAF_BASE + kb
}
#[inline]
const fn t_map_leaf(kb: u8) -> u8 {
    T_MAP_LEAF_BASE + kb
}
#[inline]
const fn t_set_immed(kb: u8, count: u8) -> u8 {
    T_SET_IMMED_BASE | ((kb - 1) << 3) | (count - 1)
}
#[inline]
const fn t_map_immed(kb: u8) -> u8 {
    T_MAP_IMMED_BASE | (kb - 1)
}

/// Decoded edge kind for dispatch.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Null,
    BranchL2,
    BranchL6,
    BranchB,
    BranchU,
    Bitmap,
    MapBitmap,
    SetLeaf(u8),
    MapLeaf(u8),
    SetImmed { kb: u8, count: u8 },
    MapImmed { kb: u8 },
}

#[inline]
fn kind_of(tag: u8) -> Kind {
    match tag {
        T_NULL => Kind::Null,
        T_L2 => Kind::BranchL2,
        T_L6 => Kind::BranchL6,
        T_B => Kind::BranchB,
        T_U => Kind::BranchU,
        T_BITMAP => Kind::Bitmap,
        T_MAP_BITMAP => Kind::MapBitmap,
        5..=8 => Kind::SetLeaf(tag - T_SET_LEAF_BASE),
        9..=12 => Kind::MapLeaf(tag - T_MAP_LEAF_BASE),
        0x40..=0x5F => Kind::SetImmed {
            kb: ((tag >> 3) & 0x03) + 1,
            count: (tag & 0x07) + 1,
        },
        0x60..=0x62 => Kind::MapImmed {
            kb: (tag & 0x03) + 1,
        },
        _ => Kind::Null,
    }
}

#[inline]
fn kind(e: &Edge32) -> Kind {
    kind_of(e.raw_tag())
}

// ---------------------------------------------------------------------------
// Remainder / digit helpers
// ---------------------------------------------------------------------------

/// Largest remainder value representable in `kb` bytes.
#[inline]
fn rem_mask(kb: u8) -> u32 {
    if kb >= 4 {
        u32::MAX
    } else {
        (1u32 << (kb as u32 * 8)) - 1
    }
}

/// Top decode digit of a `kb`-byte remainder (the byte split on at this
/// level).
#[inline]
fn digit_at(rem: u32, kb: u8) -> u8 {
    (rem >> ((kb - 1) as u32 * 8)) as u8
}

/// The lower `kb-1` bytes of `rem` — the remainder handed to the child.
#[inline]
fn child_rem(rem: u32, kb: u8) -> u32 {
    rem & rem_mask(kb - 1)
}

/// Reassemble a `kb`-byte remainder from a top `digit` and the child's
/// `kb-1`-byte remainder.
#[inline]
fn combine(digit: u8, child: u32, kb: u8) -> u32 {
    ((digit as u32) << ((kb - 1) as u32 * 8)) | child
}

#[inline]
fn read_rem(buf: &[u8], i: usize, kb: usize) -> u32 {
    let mut b = [0u8; 4];
    b[..kb].copy_from_slice(&buf[i * kb..i * kb + kb]);
    u32::from_le_bytes(b)
}

#[inline]
fn write_rem(buf: &mut [u8], i: usize, kb: usize, rem: u32) {
    let rb = rem.to_le_bytes();
    buf[i * kb..i * kb + kb].copy_from_slice(&rb[..kb]);
}

// ---------------------------------------------------------------------------
// Arena
// ---------------------------------------------------------------------------

pub(crate) struct BranchB32Data {
    pub(crate) header: BranchB32,
    pub(crate) count: u32,
    pub(crate) num_children: u16,
    pub(crate) subarrays: [Option<Box<[Edge32]>>; 8],
}

pub(crate) struct LeafBitmapL32Data {
    pub(crate) header: LeafBitmapL_32,
    pub(crate) subarrays: [Option<Box<[u32]>>; 8],
}

/// One arena-owned node. Each variant is an independent heap allocation
/// sized to the RFC's on-target byte layout, so [`Arena::bytes_in_use`]
/// is an exact memory figure.
pub enum NodeBox {
    L2(Box<BranchL2_32>),
    L6(Box<BranchL6_32>),
    B(Box<BranchB32Data>),
    U(Box<BranchU32>),
    Bitmap(Box<LeafBitmap1_32>),
    MapBitmap(Box<LeafBitmapL32Data>),
    Leaf(Box<[u8]>),
}

impl NodeBox {
    #[inline]
    fn heap_bytes(&self) -> usize {
        match self {
            NodeBox::L2(_) => core::mem::size_of::<BranchL2_32>(),
            NodeBox::L6(_) => core::mem::size_of::<BranchL6_32>(),
            NodeBox::B(b) => {
                core::mem::size_of::<BranchB32>()
                    + b.subarrays
                        .iter()
                        .filter_map(|s| s.as_ref())
                        .map(|s| s.len() * core::mem::size_of::<Edge32>())
                        .sum::<usize>()
            }
            NodeBox::U(_) => core::mem::size_of::<BranchU32>(),
            NodeBox::Bitmap(_) => core::mem::size_of::<LeafBitmap1_32>(),
            NodeBox::MapBitmap(b) => {
                core::mem::size_of::<LeafBitmapL_32>()
                    + b.subarrays
                        .iter()
                        .filter_map(|s| s.as_ref())
                        .map(|s| s.len() * core::mem::size_of::<u32>())
                        .sum::<usize>()
            }
            NodeBox::Leaf(b) => b.len(),
        }
    }
}

/// Per-tree node arena. Hands out 32-bit handles (indices) and keeps
/// byte-exact accounting of live node allocations. Mirrors the role of the
/// 64-bit `NodeAlloc`, adapted to be pointer-width-independent.
pub struct Arena {
    slots: Vec<Option<NodeBox>>,
    free: Vec<u32>,
    bytes: usize,
    /// Fixed-capacity mode: `slots` was pre-filled at construction and must
    /// never grow (its `Vec` header is then immutable, which the concurrent
    /// wrapper's racy readers rely on). Opt-in via [`Arena::with_capacity`];
    /// the default arena grows with population per the §2.1 expanse
    /// invariant.
    fixed: bool,
    /// Deferred-reclamation mode: freed allocations are parked in
    /// [`Arena::pending`] instead of being returned to the allocator, so an
    /// optimistic reader mid-walk can never dereference freed memory. The
    /// writer drains the list at a quiescent point.
    deferred: bool,
    /// Retired allocations awaiting a quiescent point (`deferred` only).
    pending: Vec<Retired>,
    /// Bytes parked in `pending` (observability; not part of `bytes`).
    pending_bytes: usize,
    /// Arena allocations since the last watermark reset (validates
    /// [`MUTATION_HEADROOM`] in the sync32 tests).
    #[cfg(test)]
    mut_allocs: usize,
    /// Retirements (frees + subarray replacements) since the last reset.
    #[cfg(test)]
    mut_retires: usize,
}

/// A retired heap allocation parked until reclamation is safe.
///
/// Nodes are the common case; the two subarray variants exist because
/// `BranchB32Data`/`LeafBitmapL32Data` replace their inner boxed subarrays
/// in place during mutation, and those old boxes must outlive concurrent
/// readers exactly like whole nodes do.
pub enum Retired {
    /// A whole arena node.
    Node(NodeBox),
    /// A `BranchB32Data` edge subarray replaced during mutation.
    Edges(Box<[Edge32]>),
    /// A `LeafBitmapL32Data` value subarray replaced during mutation.
    Vals(Box<[u32]>),
}

impl Retired {
    #[inline]
    fn heap_bytes(&self) -> usize {
        match self {
            Retired::Node(n) => n.heap_bytes(),
            Retired::Edges(b) => b.len() * core::mem::size_of::<Edge32>(),
            Retired::Vals(b) => b.len() * core::mem::size_of::<u32>(),
        }
    }
}

/// Worst-case arena allocations one `map_insert`/`set_insert`/`*_remove`
/// can perform, used as the pre-mutation headroom a fixed arena must hold.
///
/// Static ceiling: a split can cascade through all four levels when every
/// key in an overflowing leaf shares its leading bytes — each level
/// materialises at most `max(SET_LEAF_MAX, MAP_LEAF_MAX) + 1` children
/// plus one branch, plus grow-ladder replacements, so 4 levels x 32 bounds
/// it. The `mutation_watermark` assertion in `sync32::Writer32::write`
/// checks the bound on every mutation of the churn tests (observed worst
/// case there: 49 allocations in one insert); a violation of the arena
/// capacity itself trips the fail-loud assert in [`Arena::alloc`].
pub(crate) const MUTATION_HEADROOM: usize = 128;

impl Arena {
    #[inline]
    pub(crate) const fn new() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            bytes: 0,
            fixed: false,
            deferred: false,
            pending: Vec::new(),
            pending_bytes: 0,
            #[cfg(test)]
            mut_allocs: 0,
            #[cfg(test)]
            mut_retires: 0,
        }
    }

    /// A fixed-capacity, deferred-reclamation arena for the concurrent
    /// 32-bit wrapper (`sync32`). Pre-fills all `cap` slots so the backing
    /// `Vec` never reallocates — its (ptr, len) header is immutable after
    /// construction, which the racy reader walk depends on.
    pub(crate) fn with_capacity(cap: usize, pending_cap: usize) -> Self {
        let mut slots = Vec::with_capacity(cap);
        slots.resize_with(cap, || None);
        // Low handles handed out first: pop from the back.
        let free: Vec<u32> = (0..cap as u32).rev().collect();
        Self {
            slots,
            free,
            bytes: 0,
            fixed: true,
            deferred: true,
            pending: Vec::with_capacity(pending_cap),
            pending_bytes: 0,
            #[cfg(test)]
            mut_allocs: 0,
            #[cfg(test)]
            mut_retires: 0,
        }
    }

    /// Free node slots available for the next mutation (`fixed` mode).
    #[inline]
    pub(crate) fn free_slots(&self) -> usize {
        self.free.len()
    }

    /// Spare capacity in the pending list (`deferred` mode).
    #[inline]
    pub(crate) fn pending_spare(&self) -> usize {
        self.pending.capacity() - self.pending.len()
    }

    /// Number of retired allocations awaiting reclamation.
    #[inline]
    pub(crate) fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Bytes parked in the pending list.
    #[inline]
    pub(crate) fn pending_bytes(&self) -> usize {
        self.pending_bytes
    }

    /// Drops every retired allocation. The caller must have established
    /// quiescence: no optimistic reader may still be inside a walk that
    /// began before these allocations were unlinked.
    #[inline]
    pub(crate) fn drain_pending(&mut self) {
        self.pending.clear();
        self.pending_bytes = 0;
    }

    /// Frees the oldest `n` parked allocations — the prefix whose grace
    /// period has elapsed under the writer's reader snapshot (`sync32`,
    /// #594) — and keeps the rest parked.
    #[inline]
    pub(crate) fn drain_pending_prefix(&mut self, n: usize) {
        let n = n.min(self.pending.len());
        for r in self.pending.drain(..n) {
            self.pending_bytes -= r.heap_bytes();
        }
    }

    /// Parks a replaced `BranchB32Data` edge subarray (drops it immediately
    /// outside deferred mode).
    #[inline]
    fn retire_edges(&mut self, old: Option<Box<[Edge32]>>) {
        if let Some(b) = old
            && self.deferred
        {
            self.push_pending(Retired::Edges(b));
        }
    }

    /// Parks a replaced `LeafBitmapL32Data` value subarray.
    #[inline]
    fn retire_vals(&mut self, old: Option<Box<[u32]>>) {
        if let Some(b) = old
            && self.deferred
        {
            self.push_pending(Retired::Vals(b));
        }
    }

    /// Zeroes the per-mutation watermark counters (test builds only).
    #[cfg(test)]
    pub(crate) fn reset_mutation_watermark(&mut self) {
        self.mut_allocs = 0;
        self.mut_retires = 0;
    }

    /// `(allocations, retirements)` since the last reset (test builds only).
    #[cfg(test)]
    pub(crate) fn mutation_watermark(&self) -> (usize, usize) {
        (self.mut_allocs, self.mut_retires)
    }

    #[inline]
    fn push_pending(&mut self, r: Retired) {
        // Fail loud rather than reallocate: a reallocation here would move
        // nothing readers hold, but it would breach the bounded-memory
        // contract the pre-mutation headroom check exists to keep.
        assert!(
            self.pending.len() < self.pending.capacity(),
            "pending list exhausted — mutation ran without headroom"
        );
        #[cfg(test)]
        {
            self.mut_retires += 1;
        }
        self.pending_bytes += r.heap_bytes();
        self.pending.push(r);
    }

    /// Bytes currently held by live nodes (the honest `mem_used` figure).
    #[inline]
    pub(crate) fn bytes_in_use(&self) -> usize {
        self.bytes
    }

    /// Number of live node allocations (leak diagnostics in tests).
    #[cfg(test)]
    #[inline]
    pub(crate) fn live_allocs(&self) -> usize {
        self.slots.len() - self.free.len()
    }

    #[inline]
    fn alloc(&mut self, node: NodeBox) -> u32 {
        #[cfg(test)]
        {
            self.mut_allocs += 1;
        }
        self.bytes += node.heap_bytes();
        if let Some(h) = self.free.pop() {
            self.slots[h as usize] = Some(node);
            h
        } else {
            // A fixed arena must never grow: reallocation would move the
            // slots the concurrent wrapper's readers walk. The wrapper's
            // pre-mutation headroom check makes this unreachable; if it
            // fires, that check was bypassed (fail loud, AGENTS.md §8.1).
            assert!(!self.fixed, "fixed arena exhausted mid-mutation");
            let h = self.slots.len() as u32;
            self.slots.push(Some(node));
            h
        }
    }

    #[inline]
    fn free(&mut self, h: u32) {
        let node = self.slots[h as usize]
            .take()
            .expect("free of an empty arena slot (double free)");
        self.bytes -= node.heap_bytes();
        if self.deferred {
            // The handle can be reused immediately (a stale handle resolves
            // to a live-but-different node, which validation catches); only
            // the *memory* must outlive concurrent readers.
            self.push_pending(Retired::Node(node));
        }
        self.free.push(h);
    }

    #[inline]
    fn get(&self, h: u32) -> &NodeBox {
        self.slots[h as usize].as_ref().expect("live arena slot")
    }

    #[inline]
    fn get_mut(&mut self, h: u32) -> &mut NodeBox {
        self.slots[h as usize].as_mut().expect("live arena slot")
    }

    fn l2(&self, h: u32) -> &BranchL2_32 {
        match self.get(h) {
            NodeBox::L2(b) => b,
            _ => unreachable!("expected BranchL2_32"),
        }
    }
    fn l2_mut(&mut self, h: u32) -> &mut BranchL2_32 {
        match self.get_mut(h) {
            NodeBox::L2(b) => b,
            _ => unreachable!("expected BranchL2_32"),
        }
    }
    fn l6(&self, h: u32) -> &BranchL6_32 {
        match self.get(h) {
            NodeBox::L6(b) => b,
            _ => unreachable!("expected BranchL6_32"),
        }
    }
    fn l6_mut(&mut self, h: u32) -> &mut BranchL6_32 {
        match self.get_mut(h) {
            NodeBox::L6(b) => b,
            _ => unreachable!("expected BranchL6_32"),
        }
    }
    fn b(&self, h: u32) -> &BranchB32Data {
        match self.get(h) {
            NodeBox::B(b) => b,
            _ => unreachable!("expected BranchB32Data"),
        }
    }
    fn b_mut(&mut self, h: u32) -> &mut BranchB32Data {
        match self.get_mut(h) {
            NodeBox::B(b) => b,
            _ => unreachable!("expected BranchB32Data"),
        }
    }
    fn u(&self, h: u32) -> &BranchU32 {
        match self.get(h) {
            NodeBox::U(b) => b,
            _ => unreachable!("expected BranchU32"),
        }
    }
    fn u_mut(&mut self, h: u32) -> &mut BranchU32 {
        match self.get_mut(h) {
            NodeBox::U(b) => b,
            _ => unreachable!("expected BranchU32"),
        }
    }
    fn bitmap(&self, h: u32) -> &LeafBitmap1_32 {
        match self.get(h) {
            NodeBox::Bitmap(b) => b,
            _ => unreachable!("expected LeafBitmap1_32"),
        }
    }
    fn bitmap_mut(&mut self, h: u32) -> &mut LeafBitmap1_32 {
        match self.get_mut(h) {
            NodeBox::Bitmap(b) => b,
            _ => unreachable!("expected LeafBitmap1_32"),
        }
    }
    fn map_bitmap(&self, h: u32) -> &LeafBitmapL32Data {
        match self.get(h) {
            NodeBox::MapBitmap(b) => b,
            _ => unreachable!("expected LeafBitmapL32Data"),
        }
    }
    fn map_bitmap_mut(&mut self, h: u32) -> &mut LeafBitmapL32Data {
        match self.get_mut(h) {
            NodeBox::MapBitmap(b) => b,
            _ => unreachable!("expected LeafBitmapL32Data"),
        }
    }
    fn leaf(&self, h: u32) -> &[u8] {
        match self.get(h) {
            NodeBox::Leaf(b) => b,
            _ => unreachable!("expected leaf bytes"),
        }
    }
    fn leaf_mut(&mut self, h: u32) -> &mut [u8] {
        match self.get_mut(h) {
            NodeBox::Leaf(b) => b,
            _ => unreachable!("expected leaf bytes"),
        }
    }
}

impl Default for Arena {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Edge construction helpers
// ---------------------------------------------------------------------------

#[inline]
fn edge_handle(e: &Edge32) -> u32 {
    e.w0_raw()
}

#[inline]
fn edge_pop(e: &Edge32) -> usize {
    let a = e.aux_raw();
    (a[0] as usize) | ((a[1] as usize) << 8)
}

#[inline]
fn node_edge(handle: u32, tag: u8) -> Edge32 {
    Edge32::from_parts(handle, [0, 0, 0], tag)
}

#[inline]
fn leaf_edge(handle: u32, pop: usize, tag: u8) -> Edge32 {
    Edge32::from_parts(handle, [pop as u8, (pop >> 8) as u8, 0], tag)
}

// Immediate (in-edge) payload spans word0 (4 bytes) + aux (3 bytes) = 7 B.

#[inline]
fn set_immed_edge(kb: u8, keys: &[u32]) -> Edge32 {
    debug_assert!(keys.len() <= set_immed_cap(kb));
    let mut p = [0u8; 7];
    let kbu = kb as usize;
    for (i, &k) in keys.iter().enumerate() {
        let rb = k.to_le_bytes();
        p[i * kbu..i * kbu + kbu].copy_from_slice(&rb[..kbu]);
    }
    Edge32::from_parts(
        u32::from_le_bytes([p[0], p[1], p[2], p[3]]),
        [p[4], p[5], p[6]],
        t_set_immed(kb, keys.len() as u8),
    )
}

#[inline]
fn set_immed_keys(e: &Edge32, kb: u8, count: u8) -> ([u32; 7], usize) {
    let w = e.w0_raw().to_le_bytes();
    let a = e.aux_raw();
    let p = [w[0], w[1], w[2], w[3], a[0], a[1], a[2]];
    let kbu = kb as usize;
    let mut out = [0u32; 7];
    for (i, slot) in out.iter_mut().enumerate().take(count as usize) {
        let mut b = [0u8; 4];
        b[..kbu].copy_from_slice(&p[i * kbu..i * kbu + kbu]);
        *slot = u32::from_le_bytes(b);
    }
    (out, count as usize)
}

#[inline]
fn map_immed_edge(kb: u8, rem: u32, val: u32) -> Edge32 {
    let rb = rem.to_le_bytes();
    let mut aux = [0u8; 3];
    aux[..kb as usize].copy_from_slice(&rb[..kb as usize]);
    Edge32::from_parts(val, aux, t_map_immed(kb))
}

#[inline]
fn map_immed_rem(e: &Edge32, kb: u8) -> u32 {
    let a = e.aux_raw();
    let mut b = [0u8; 4];
    b[..kb as usize].copy_from_slice(&a[..kb as usize]);
    u32::from_le_bytes(b)
}

#[inline]
fn map_immed_val(e: &Edge32) -> u32 {
    e.w0_raw()
}

// ---------------------------------------------------------------------------
// Linear leaf construction / reading (set flavour)
// ---------------------------------------------------------------------------

fn make_set_leaf(a: &mut Arena, kb: u8, keys: &[u32]) -> Edge32 {
    let pop = keys.len();
    let cap = cap_class(pop);
    let mut buf = alloc_zeroed_bytes(size_set32(kb, pop).max(kb as usize * cap));
    for (i, &k) in keys.iter().enumerate() {
        write_rem(&mut buf, i, kb as usize, k);
    }
    let h = a.alloc(NodeBox::Leaf(buf));
    leaf_edge(h, pop, t_set_leaf(kb))
}

fn read_set_leaf(a: &Arena, e: &Edge32, kb: u8) -> Vec<u32> {
    let pop = edge_pop(e);
    let buf = a.leaf(edge_handle(e));
    let mut v = Vec::with_capacity(pop);
    for i in 0..pop {
        v.push(read_rem(buf, i, kb as usize));
    }
    v
}

/// Insert `rem` at sorted position `pos` of the set leaf behind `e`,
/// mutating the buffer in place while the population stays inside the
/// current capacity class and copying region-to-region into a
/// next-class node otherwise. Replaces the scratch-`Vec` rebuild that
/// dominated the 32-bit write-path instruction profile (#577).
fn set_leaf_insert_at(a: &mut Arena, e: &mut Edge32, kb: u8, pos: usize, rem: u32) {
    let pop = edge_pop(e);
    let new_pop = pop + 1;
    let kbz = kb as usize;
    let h = edge_handle(e);
    if cap_class(new_pop) == cap_class(pop) {
        let buf = a.leaf_mut(h);
        buf.copy_within(pos * kbz..pop * kbz, (pos + 1) * kbz);
        write_rem(buf, pos, kbz, rem);
        *e = leaf_edge(h, new_pop, t_set_leaf(kb));
    } else {
        let mut nb = alloc_zeroed_bytes(size_set32(kb, new_pop));
        let old = a.leaf(h);
        nb[..pos * kbz].copy_from_slice(&old[..pos * kbz]);
        nb[(pos + 1) * kbz..new_pop * kbz].copy_from_slice(&old[pos * kbz..pop * kbz]);
        write_rem(&mut nb, pos, kbz, rem);
        let nh = a.alloc(NodeBox::Leaf(nb));
        a.free(h);
        *e = leaf_edge(nh, new_pop, t_set_leaf(kb));
    }
}

/// Remove the element at `pos` of the set leaf behind `e`; see
/// [`set_leaf_remove_span`].
fn set_leaf_remove_at(a: &mut Arena, e: &mut Edge32, kb: u8, pos: usize) {
    set_leaf_remove_span(a, e, kb, pos, pos + 1);
}

/// Remove the contiguous run `i0..i1` of the set leaf behind `e`, in
/// place within the capacity class and by region copy into the exact
/// smaller class otherwise, keeping the allocation size derivable from
/// the population alone. The caller has already ruled out demotion to
/// an immediate or to null. One structural fix-up per touched leaf is
/// what makes a batched range removal cheaper than per-key removes
/// (#578).
fn set_leaf_remove_span(a: &mut Arena, e: &mut Edge32, kb: u8, i0: usize, i1: usize) {
    let pop = edge_pop(e);
    let new_pop = pop - (i1 - i0);
    let kbz = kb as usize;
    let h = edge_handle(e);
    if cap_class(new_pop) == cap_class(pop) {
        let buf = a.leaf_mut(h);
        buf.copy_within(i1 * kbz..pop * kbz, i0 * kbz);
        *e = leaf_edge(h, new_pop, t_set_leaf(kb));
    } else {
        let mut nb = alloc_zeroed_bytes(size_set32(kb, new_pop));
        let old = a.leaf(h);
        nb[..i0 * kbz].copy_from_slice(&old[..i0 * kbz]);
        nb[i0 * kbz..new_pop * kbz].copy_from_slice(&old[i1 * kbz..pop * kbz]);
        let nh = a.alloc(NodeBox::Leaf(nb));
        a.free(h);
        *e = leaf_edge(nh, new_pop, t_set_leaf(kb));
    }
}

// ---------------------------------------------------------------------------
// Linear leaf construction / reading (map flavour)
// ---------------------------------------------------------------------------

fn make_map_leaf(a: &mut Arena, kb: u8, entries: &[(u32, u32)]) -> Edge32 {
    let pop = entries.len();
    let cap = cap_class(pop);
    let mut buf = alloc_zeroed_bytes(size_map32(kb, pop));
    let keys_off = 4 * cap;
    for (i, &(k, v)) in entries.iter().enumerate() {
        buf[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        write_rem(&mut buf[keys_off..], i, kb as usize, k);
    }
    let h = a.alloc(NodeBox::Leaf(buf));
    leaf_edge(h, pop, t_map_leaf(kb))
}

fn read_map_leaf(a: &Arena, e: &Edge32, kb: u8) -> Vec<(u32, u32)> {
    let pop = edge_pop(e);
    let cap = cap_class(pop);
    let keys_off = 4 * cap;
    let buf = a.leaf(edge_handle(e));
    let mut v = Vec::with_capacity(pop);
    for i in 0..pop {
        let mut vb = [0u8; 4];
        vb.copy_from_slice(&buf[i * 4..i * 4 + 4]);
        let val = u32::from_le_bytes(vb);
        let key = read_rem(&buf[keys_off..], i, kb as usize);
        v.push((key, val));
    }
    v
}

/// Map-flavour sibling of [`set_leaf_insert_at`]: values then keys, both
/// capacity-class-sized regions. `keys_off` is unchanged inside a class
/// and recomputed for the next class on growth.
fn map_leaf_insert_at(a: &mut Arena, e: &mut Edge32, kb: u8, pos: usize, rem: u32, val: u32) {
    let pop = edge_pop(e);
    let new_pop = pop + 1;
    let kbz = kb as usize;
    let h = edge_handle(e);
    if cap_class(new_pop) == cap_class(pop) {
        let keys_off = 4 * cap_class(pop);
        let buf = a.leaf_mut(h);
        buf.copy_within(pos * 4..pop * 4, (pos + 1) * 4);
        buf[pos * 4..pos * 4 + 4].copy_from_slice(&val.to_le_bytes());
        let keys = &mut buf[keys_off..];
        keys.copy_within(pos * kbz..pop * kbz, (pos + 1) * kbz);
        write_rem(keys, pos, kbz, rem);
        *e = leaf_edge(h, new_pop, t_map_leaf(kb));
    } else {
        let old_off = 4 * cap_class(pop);
        let new_off = 4 * cap_class(new_pop);
        let mut nb = alloc_zeroed_bytes(size_map32(kb, new_pop));
        let old = a.leaf(h);
        nb[..pos * 4].copy_from_slice(&old[..pos * 4]);
        nb[pos * 4..pos * 4 + 4].copy_from_slice(&val.to_le_bytes());
        nb[(pos + 1) * 4..new_pop * 4].copy_from_slice(&old[pos * 4..pop * 4]);
        nb[new_off..new_off + pos * kbz].copy_from_slice(&old[old_off..old_off + pos * kbz]);
        nb[new_off + (pos + 1) * kbz..new_off + new_pop * kbz]
            .copy_from_slice(&old[old_off + pos * kbz..old_off + pop * kbz]);
        write_rem(&mut nb[new_off..], pos, kbz, rem);
        let nh = a.alloc(NodeBox::Leaf(nb));
        a.free(h);
        *e = leaf_edge(nh, new_pop, t_map_leaf(kb));
    }
}

/// Map-flavour sibling of [`set_leaf_remove_at`]; see
/// [`map_leaf_remove_span`].
fn map_leaf_remove_at(a: &mut Arena, e: &mut Edge32, kb: u8, pos: usize) {
    map_leaf_remove_span(a, e, kb, pos, pos + 1);
}

/// Map-flavour sibling of [`set_leaf_remove_span`]: values then keys,
/// both capacity-class-sized regions. The caller has already ruled out
/// demotion to an immediate or to null.
fn map_leaf_remove_span(a: &mut Arena, e: &mut Edge32, kb: u8, i0: usize, i1: usize) {
    let pop = edge_pop(e);
    let new_pop = pop - (i1 - i0);
    let kbz = kb as usize;
    let h = edge_handle(e);
    if cap_class(new_pop) == cap_class(pop) {
        let keys_off = 4 * cap_class(pop);
        let buf = a.leaf_mut(h);
        buf.copy_within(i1 * 4..pop * 4, i0 * 4);
        let keys = &mut buf[keys_off..];
        keys.copy_within(i1 * kbz..pop * kbz, i0 * kbz);
        *e = leaf_edge(h, new_pop, t_map_leaf(kb));
    } else {
        let old_off = 4 * cap_class(pop);
        let new_off = 4 * cap_class(new_pop);
        let mut nb = alloc_zeroed_bytes(size_map32(kb, new_pop));
        let old = a.leaf(h);
        nb[..i0 * 4].copy_from_slice(&old[..i0 * 4]);
        nb[i0 * 4..new_pop * 4].copy_from_slice(&old[i1 * 4..pop * 4]);
        nb[new_off..new_off + i0 * kbz].copy_from_slice(&old[old_off..old_off + i0 * kbz]);
        nb[new_off + i0 * kbz..new_off + new_pop * kbz]
            .copy_from_slice(&old[old_off + i1 * kbz..old_off + pop * kbz]);
        let nh = a.alloc(NodeBox::Leaf(nb));
        a.free(h);
        *e = leaf_edge(nh, new_pop, t_map_leaf(kb));
    }
}

#[inline]
fn alloc_zeroed_bytes(n: usize) -> Box<[u8]> {
    // `core_alloc::vec![0u8; n]` hits the `from_elem` zero specialization
    // (`alloc_zeroed`), unlike resize-from-empty which pays a separate
    // memset after the allocation — measurably so on the write path (#577).
    core_alloc::vec![0u8; n].into_boxed_slice()
}

// ---------------------------------------------------------------------------
// Bitmap leaf helpers (level 1, key_bytes == 1; remainder is a byte)
// ---------------------------------------------------------------------------

fn bitmap_from_keys(a: &mut Arena, keys: &[u32]) -> Edge32 {
    let mut leaf = LeafBitmap1_32::new();
    for &k in keys {
        leaf.set(k as u8);
    }
    let h = a.alloc(NodeBox::Bitmap(Box::new(leaf)));
    node_edge(h, T_BITMAP)
}

fn bitmap_keys(a: &Arena, e: &Edge32) -> Vec<u32> {
    let leaf = a.bitmap(edge_handle(e));
    let mut v = Vec::new();
    for w in 0..4usize {
        let mut word = leaf.bitmap[w];
        while word != 0 {
            let bit = word.trailing_zeros() as usize;
            v.push((w * 64 + bit) as u32);
            word &= word - 1;
        }
    }
    v
}

#[inline]
fn bitmap_count(a: &Arena, e: &Edge32) -> usize {
    let leaf = a.bitmap(edge_handle(e));
    leaf.pop0 as usize
}

fn bitmap_first_ge_raw(bitmap: &[u64; 4], from: u16) -> Option<u8> {
    if from > 255 {
        return None;
    }
    let from = from as usize;
    let mut w = from / 64;
    let mut word = bitmap[w] & (u64::MAX << (from % 64));
    loop {
        if word != 0 {
            return Some((w * 64 + word.trailing_zeros() as usize) as u8);
        }
        w += 1;
        if w >= 4 {
            return None;
        }
        word = bitmap[w];
    }
}

fn bitmap_last_le_raw(bitmap: &[u64; 4], to: i32) -> Option<u8> {
    if to < 0 {
        return None;
    }
    let to = to.min(255) as usize;
    let mut w = to / 64;
    let shift = 63 - (to % 64);
    let mut word = (bitmap[w] << shift) >> shift;
    loop {
        if word != 0 {
            return Some((w * 64 + (63 - word.leading_zeros() as usize)) as u8);
        }
        if w == 0 {
            return None;
        }
        w -= 1;
        word = bitmap[w];
    }
}

fn bitmap_count_range_raw(bitmap: &[u64; 4], lo: u32, hi: u32) -> usize {
    let lo = lo.min(255) as usize;
    let hi = hi.min(255) as usize;
    if lo > hi {
        return 0;
    }
    let mut n = 0;
    for (w, &word) in bitmap.iter().enumerate() {
        let base = w * 64;
        if base > hi || base + 63 < lo {
            continue;
        }
        let mut mask = u64::MAX;
        if lo > base {
            mask &= u64::MAX << (lo - base);
        }
        if hi < base + 63 {
            mask &= u64::MAX >> (63 - (hi - base));
        }
        n += (word & mask).count_ones() as usize;
    }
    n
}

fn bitmap_first_ge(leaf: &LeafBitmap1_32, from: u16) -> Option<u8> {
    bitmap_first_ge_raw(&leaf.bitmap, from)
}

fn bitmap_last_le(leaf: &LeafBitmap1_32, to: i32) -> Option<u8> {
    bitmap_last_le_raw(&leaf.bitmap, to)
}

fn bitmap_count_range(leaf: &LeafBitmap1_32, lo: u32, hi: u32) -> usize {
    bitmap_count_range_raw(&leaf.bitmap, lo, hi)
}

fn make_map_bitmap(a: &mut Arena, entries: &[(u32, u32)]) -> Edge32 {
    let mut data = LeafBitmapL32Data {
        header: LeafBitmapL_32::new(),
        subarrays: [None, None, None, None, None, None, None, None],
    };
    data.header.pop0 = (entries.len() - 1) as u16;
    for &(key, _) in entries {
        let digit = key as u8;
        let w = (digit >> 6) as usize;
        let bit64 = digit & 63;
        data.header.bitmap[w] |= 1u64 << bit64;
    }
    for sub in 0..8usize {
        let lo = (sub * 32) as u32;
        let hi = lo + 31;
        let mut sub_vals = Vec::new();
        for &(key, val) in entries {
            if key >= lo && key <= hi {
                sub_vals.push(val);
            }
        }
        if !sub_vals.is_empty() {
            // Class-sized like every other subarray, so the first insert
            // into this subexpanse can grow in place (#615).
            sub_vals.resize(cap_class(sub_vals.len()), 0u32);
            data.subarrays[sub] = Some(sub_vals.into_boxed_slice());
        }
    }
    let h = a.alloc(NodeBox::MapBitmap(Box::new(data)));
    node_edge(h, T_MAP_BITMAP)
}

#[inline(always)]
fn bitmap_sub_rank(word: u64, digit: u8) -> usize {
    let bit64 = digit & 63;
    let sub_base = (digit & 32) as u32;
    let bit_mask = 1u64 << bit64;
    (word & ((bit_mask - 1) & (!0u64 << sub_base))).count_ones() as usize
}

/// Population of the 32-digit subexpanse `digit` falls in, read out of the
/// bitmap word that covers it.
///
/// This is the *population* of a bitmap node's rank-ordered subarray. It is
/// deliberately not `subarrays[sub].len()`: since #615 a subarray is
/// allocated at its [`cap_class`] and its trailing spare slots hold filler,
/// so `len()` is the allocation length and only the bitmap (for a value
/// leaf) or `pop_counts` (for a branch) knows how many entries are live.
#[inline(always)]
fn bitmap_sub_pop(word: u64, digit: u8) -> usize {
    let sub_base = (digit & 32) as u32;
    ((word >> sub_base) & 0xFFFF_FFFF).count_ones() as usize
}

/// Inserts `val` at rank `rank` into a bitmap node's rank-ordered subarray
/// that currently holds `pop` live entries, mirroring the 64-bit engine's
/// cap-classed leaf growth (`mutate_map::map_insert_with_path`, #577).
///
/// The subarray is allocated at `cap_class(pop)` slots with the live
/// entries in `[0, pop)` and `filler` in the tail, so a growth that stays
/// inside the class shifts in place and allocates nothing. Only a growth
/// that crosses a class boundary allocates; the replaced box is returned
/// for the caller to retire (§2.3: it must outlive stalled OCC readers).
///
/// The spare slots are always initialised — nothing may read them, since
/// every access is by a rank that is `< pop` by construction, but an
/// optimistic reader racing a shift must never observe uninitialised
/// memory.
fn subarray_insert<T: Copy>(
    slot: &mut Option<Box<[T]>>,
    pop: usize,
    rank: usize,
    val: T,
    filler: T,
) -> Option<Box<[T]>> {
    debug_assert!(rank <= pop);
    let new_cap = cap_class(pop + 1);
    if let Some(existing) = slot.as_mut() {
        debug_assert_eq!(existing.len(), cap_class(pop));
        if existing.len() == new_cap {
            // Spare class capacity: shift the tail right and store in
            // place. No allocation, no retirement.
            existing.copy_within(rank..pop, rank + 1);
            existing[rank] = val;
            return None;
        }
    }
    let mut new_sub: Vec<T> = Vec::with_capacity(new_cap);
    match slot.as_ref() {
        Some(existing) => {
            new_sub.extend_from_slice(&existing[..rank]);
            new_sub.push(val);
            new_sub.extend_from_slice(&existing[rank..pop]);
        }
        None => new_sub.push(val),
    }
    new_sub.resize(new_cap, filler);
    debug_assert_eq!(new_sub.len(), new_cap);
    slot.replace(new_sub.into_boxed_slice())
}

/// Removes rank `rank` from a bitmap node's rank-ordered subarray holding
/// `pop` live entries. The inverse of [`subarray_insert`]: a shrink that
/// stays inside the capacity class compacts in place (the vacated tail slot
/// is reset to `filler`), and only a class crossing reallocates. Returns
/// the replaced box for the caller to retire.
fn subarray_remove<T: Copy>(
    slot: &mut Option<Box<[T]>>,
    pop: usize,
    rank: usize,
    filler: T,
) -> Option<Box<[T]>> {
    debug_assert!(pop > 0 && rank < pop);
    let new_pop = pop - 1;
    if new_pop == 0 {
        return slot.take();
    }
    let new_cap = cap_class(new_pop);
    let existing = slot.as_mut().expect("live subarray");
    debug_assert_eq!(existing.len(), cap_class(pop));
    if existing.len() == new_cap {
        existing.copy_within(rank + 1..pop, rank);
        existing[new_pop] = filler;
        return None;
    }
    let mut new_sub: Vec<T> = Vec::with_capacity(new_cap);
    new_sub.extend_from_slice(&existing[..rank]);
    new_sub.extend_from_slice(&existing[rank + 1..pop]);
    new_sub.resize(new_cap, filler);
    slot.replace(new_sub.into_boxed_slice())
}

/// Byte delta a subarray of `T` undergoes when its population moves from
/// `old_pop` to `new_pop`, for the arena's exact `bytes_in_use` accounting.
#[inline]
fn subarray_bytes_delta<T>(old_pop: usize, new_pop: usize) -> isize {
    let t = core::mem::size_of::<T>() as isize;
    (cap_class(new_pop) as isize - cap_class(old_pop) as isize) * t
}

fn read_map_bitmap(a: &Arena, e: &Edge32) -> Vec<(u32, u32)> {
    let data = a.map_bitmap(edge_handle(e));
    let mut entries = Vec::with_capacity((data.header.pop0 + 1) as usize);
    for w in 0..4usize {
        let mut word = data.header.bitmap[w];
        while word != 0 {
            let bit = word.trailing_zeros() as usize;
            let digit = (w * 64 + bit) as u8;
            let rank = bitmap_sub_rank(data.header.bitmap[w], digit);
            let sub = (digit >> 5) as usize;
            let val = data.subarrays[sub].as_ref().unwrap()[rank];
            entries.push((digit as u32, val));
            word &= word - 1;
        }
    }
    entries
}

// ---------------------------------------------------------------------------
// Branch helpers (shared by set and map — branches only route by digit)
// ---------------------------------------------------------------------------

fn branch_level(a: &Arena, e: &Edge32) -> u8 {
    match kind(e) {
        Kind::BranchL2 => a.l2(edge_handle(e)).header.level,
        Kind::BranchL6 => a.l6(edge_handle(e)).header.level,
        Kind::BranchB => a.b(edge_handle(e)).header.level,
        Kind::BranchU => a.u(edge_handle(e)).level,
        _ => unreachable!(),
    }
}

fn branch_total_keys(a: &Arena, e: &Edge32) -> u32 {
    match kind(e) {
        Kind::BranchL2 => a.l2(edge_handle(e)).header.pop0,
        Kind::BranchL6 => a.l6(edge_handle(e)).header.pop0,
        Kind::BranchB => a.b(edge_handle(e)).count,
        Kind::BranchU => a.u(edge_handle(e)).count,
        _ => unreachable!(),
    }
}

/// One-visit successor to the `branch_set_child` + `branch_add_keys`
/// pair on the descent's way back up: a single arena fetch and tag
/// dispatch stores the child edge (when not null) and applies the
/// subtree key-count delta. Under the AAPCS zero-spill invariant
/// (AGENTS.md §2.1.7), `child` is passed as a bare `Edge32` using
/// `Edge32::null()` as the sentinel for unchanged child edges, and
/// `delta` is passed as a 32-bit `i32` word to avoid register spills.
#[inline(always)]
fn branch_commit(a: &mut Arena, e: &Edge32, digit: u8, child: Edge32, delta: i32) {
    match kind(e) {
        Kind::BranchL2 => {
            let b = a.l2_mut(edge_handle(e));
            b.header.pop0 = (b.header.pop0 as i32 + delta) as u32;
            if !child.is_null() {
                let n = b.header.num_edges as usize;
                if n > 0 && b.digits[0] == digit {
                    b.edges[0] = child;
                } else if n > 1 && b.digits[1] == digit {
                    b.edges[1] = child;
                } else {
                    unreachable!("branch_commit: digit not present");
                }
            }
        }
        Kind::BranchL6 => {
            let b = a.l6_mut(edge_handle(e));
            b.header.pop0 = (b.header.pop0 as i32 + delta) as u32;
            if !child.is_null() {
                let n = b.header.num_edges as usize;
                for i in 0..n {
                    if b.digits[i] == digit {
                        b.edges[i] = child;
                        return;
                    }
                }
                unreachable!("branch_commit: digit not present");
            }
        }
        Kind::BranchB => {
            let b = a.b_mut(edge_handle(e));
            b.count = (b.count as i32 + delta) as u32;
            if !child.is_null() {
                let w = (digit >> 6) as usize;
                let word = b.header.bitmap[w];
                let rank = bitmap_sub_rank(word, digit);
                let sub = (digit >> 5) as usize;
                b.subarrays[sub].as_mut().expect("live subarray")[rank] = child;
            }
        }
        Kind::BranchU => {
            let b = a.u_mut(edge_handle(e));
            b.count = (b.count as i32 + delta) as u32;
            if !child.is_null() {
                b.edges[digit as usize] = child;
            }
        }
        _ => unreachable!(),
    }
}

/// Child edge at `digit`, if present.
#[inline(always)]
fn branch_child(a: &Arena, e: &Edge32, digit: u8) -> Option<Edge32> {
    match kind(e) {
        Kind::BranchL2 => {
            let b = a.l2(edge_handle(e));
            let n = b.header.num_edges as usize;
            if n > 0 && b.digits[0] == digit {
                Some(b.edges[0])
            } else if n > 1 && b.digits[1] == digit {
                Some(b.edges[1])
            } else {
                None
            }
        }
        Kind::BranchL6 => {
            let b = a.l6(edge_handle(e));
            let n = b.header.num_edges as usize;
            for i in 0..n {
                if b.digits[i] == digit {
                    return Some(b.edges[i]);
                }
            }
            None
        }
        Kind::BranchB => {
            let b = a.b(edge_handle(e));
            let w = (digit >> 6) as usize;
            let word = b.header.bitmap[w];
            let bit_mask = 1u64 << (digit & 63);
            if (word & bit_mask) == 0 {
                return None;
            }
            let rank = bitmap_sub_rank(word, digit);
            let sub = (digit >> 5) as usize;
            b.subarrays[sub].as_ref().map(|s| s[rank])
        }
        Kind::BranchU => {
            let b = a.u(edge_handle(e));
            let c = b.edges[digit as usize];
            if c.is_null() { None } else { Some(c) }
        }
        _ => unreachable!(),
    }
}

#[inline]
fn branch_first_child(a: &Arena, e: &Edge32) -> Option<(u8, Edge32)> {
    match kind(e) {
        Kind::BranchL2 => {
            let b = a.l2(edge_handle(e));
            (b.header.num_edges > 0).then(|| (b.digits[0], b.edges[0]))
        }
        Kind::BranchL6 => {
            let b = a.l6(edge_handle(e));
            (b.header.num_edges > 0).then(|| (b.digits[0], b.edges[0]))
        }
        Kind::BranchB => {
            let b = a.b(edge_handle(e));
            for w in 0..4usize {
                let word = b.header.bitmap[w];
                if word != 0 {
                    let bit = word.trailing_zeros() as usize;
                    let digit = (w * 64 + bit) as u8;
                    let rank = bitmap_sub_rank(word, digit);
                    let sub = (digit >> 5) as usize;
                    let child = b.subarrays[sub].as_ref().unwrap()[rank];
                    return Some((digit, child));
                }
            }
            None
        }
        Kind::BranchU => {
            let b = a.u(edge_handle(e));
            for d in 0..256usize {
                let c = b.edges[d];
                if !c.is_null() {
                    return Some((d as u8, c));
                }
            }
            None
        }
        _ => unreachable!(),
    }
}

#[inline]
fn branch_last_child(a: &Arena, e: &Edge32) -> Option<(u8, Edge32)> {
    match kind(e) {
        Kind::BranchL2 => {
            let b = a.l2(edge_handle(e));
            let n = b.header.num_edges as usize;
            (n > 0).then(|| (b.digits[n - 1], b.edges[n - 1]))
        }
        Kind::BranchL6 => {
            let b = a.l6(edge_handle(e));
            let n = b.header.num_edges as usize;
            (n > 0).then(|| (b.digits[n - 1], b.edges[n - 1]))
        }
        Kind::BranchB => {
            let b = a.b(edge_handle(e));
            for w in (0..4usize).rev() {
                let word = b.header.bitmap[w];
                if word != 0 {
                    let bit = 63 - word.leading_zeros() as usize;
                    let digit = (w * 64 + bit) as u8;
                    let rank = bitmap_sub_rank(word, digit);
                    let sub = (digit >> 5) as usize;
                    let child = b.subarrays[sub].as_ref().unwrap()[rank];
                    return Some((digit, child));
                }
            }
            None
        }
        Kind::BranchU => {
            let b = a.u(edge_handle(e));
            for d in (0..256usize).rev() {
                let c = b.edges[d];
                if !c.is_null() {
                    return Some((d as u8, c));
                }
            }
            None
        }
        _ => unreachable!(),
    }
}

#[inline]
fn branch_for_each_child<F>(a: &Arena, e: &Edge32, mut f: F)
where
    F: FnMut(u8, Edge32) -> bool,
{
    match kind(e) {
        Kind::BranchL2 => {
            let b = a.l2(edge_handle(e));
            let n = b.header.num_edges as usize;
            for i in 0..n {
                if !f(b.digits[i], b.edges[i]) {
                    break;
                }
            }
        }
        Kind::BranchL6 => {
            let b = a.l6(edge_handle(e));
            let n = b.header.num_edges as usize;
            for i in 0..n {
                if !f(b.digits[i], b.edges[i]) {
                    break;
                }
            }
        }
        Kind::BranchB => {
            let b = a.b(edge_handle(e));
            for w in 0..4usize {
                let mut word = b.header.bitmap[w];
                while word != 0 {
                    let bit = word.trailing_zeros() as usize;
                    let digit = (w * 64 + bit) as u8;
                    let rank = bitmap_sub_rank(b.header.bitmap[w], digit);
                    let sub = (digit >> 5) as usize;
                    let child = b.subarrays[sub].as_ref().unwrap()[rank];
                    if !f(digit, child) {
                        return;
                    }
                    word &= word - 1;
                }
            }
        }
        Kind::BranchU => {
            let b = a.u(edge_handle(e));
            for d in 0..256usize {
                let c = b.edges[d];
                if !c.is_null() && !f(d as u8, c) {
                    return;
                }
            }
        }
        _ => unreachable!(),
    }
}

#[inline]
fn branch_for_each_child_rev<F>(a: &Arena, e: &Edge32, mut f: F)
where
    F: FnMut(u8, Edge32) -> bool,
{
    match kind(e) {
        Kind::BranchL2 => {
            let b = a.l2(edge_handle(e));
            let n = b.header.num_edges as usize;
            for i in (0..n).rev() {
                if !f(b.digits[i], b.edges[i]) {
                    break;
                }
            }
        }
        Kind::BranchL6 => {
            let b = a.l6(edge_handle(e));
            let n = b.header.num_edges as usize;
            for i in (0..n).rev() {
                if !f(b.digits[i], b.edges[i]) {
                    break;
                }
            }
        }
        Kind::BranchB => {
            let b = a.b(edge_handle(e));
            for w in (0..4usize).rev() {
                let mut word = b.header.bitmap[w];
                while word != 0 {
                    let bit = 63 - word.leading_zeros() as usize;
                    let digit = (w * 64 + bit) as u8;
                    let rank = bitmap_sub_rank(b.header.bitmap[w], digit);
                    let sub = (digit >> 5) as usize;
                    let child = b.subarrays[sub].as_ref().unwrap()[rank];
                    if !f(digit, child) {
                        return;
                    }
                    word &= !(1u64 << bit);
                }
            }
        }
        Kind::BranchU => {
            let b = a.u(edge_handle(e));
            for d in (0..256usize).rev() {
                let c = b.edges[d];
                if !c.is_null() && !f(d as u8, c) {
                    return;
                }
            }
        }
        _ => unreachable!(),
    }
}

/// Collect all `(digit, child)` pairs of a branch in ascending digit order.
fn branch_pairs(a: &Arena, e: &Edge32) -> Vec<(u8, Edge32)> {
    let mut pairs = Vec::new();
    branch_for_each_child(a, e, |d, c| {
        pairs.push((d, c));
        true
    });
    pairs
}

fn make_l2(a: &mut Arena, level: u8, pairs: &[(u8, Edge32)], total: u32) -> Edge32 {
    debug_assert!(pairs.len() <= 2);
    let mut b = BranchL2_32 {
        header: BranchHeader32::new(level, pairs.len() as u8, total),
        digits: [0; 2],
        _pad: [0; 6],
        edges: [Edge32::null(); 2],
    };
    for (i, &(d, c)) in pairs.iter().enumerate() {
        b.digits[i] = d;
        b.edges[i] = c;
    }
    let h = a.alloc(NodeBox::L2(Box::new(b)));
    node_edge(h, T_L2)
}

fn make_l6(a: &mut Arena, level: u8, pairs: &[(u8, Edge32)], total: u32) -> Edge32 {
    debug_assert!(pairs.len() <= 6);
    let mut b = BranchL6_32::new(level);
    b.header.num_edges = pairs.len() as u8;
    b.header.pop0 = total;
    for (i, &(d, c)) in pairs.iter().enumerate() {
        b.digits[i] = d;
        b.edges[i] = c;
    }
    let h = a.alloc(NodeBox::L6(Box::new(b)));
    node_edge(h, T_L6)
}

fn make_b(a: &mut Arena, level: u8, pairs: &[(u8, Edge32)], total: u32) -> Edge32 {
    let mut data = BranchB32Data {
        header: BranchB32::new(level),
        count: total,
        num_children: pairs.len() as u16,
        subarrays: [None, None, None, None, None, None, None, None],
    };
    for &(digit, _) in pairs {
        let w = (digit >> 6) as usize;
        let bit64 = digit & 63;
        data.header.bitmap[w] |= 1u64 << bit64;
        let sub = (digit >> 5) as usize;
        data.header.pop_counts[sub] += 1;
    }
    for sub in 0..8usize {
        let pop = data.header.pop_counts[sub] as usize;
        if pop > 0 {
            let mut sub_edges = Vec::with_capacity(cap_class(pop));
            let lo = (sub * 32) as u8;
            let hi = lo + 31;
            for &(digit, edge) in pairs {
                if digit >= lo && digit <= hi {
                    sub_edges.push(edge);
                }
            }
            // Class-sized like every other subarray (#615).
            sub_edges.resize(cap_class(pop), Edge32::null());
            data.subarrays[sub] = Some(sub_edges.into_boxed_slice());
        }
    }
    let h = a.alloc(NodeBox::B(Box::new(data)));
    node_edge(h, T_B)
}

fn make_u(a: &mut Arena, level: u8, pairs: &[(u8, Edge32)], total: u32) -> Edge32 {
    let mut b = BranchU32::new(level);
    b.count = total;
    b.num_children = pairs.len() as u16;
    for &(d, c) in pairs {
        b.edges[d as usize] = c;
    }
    let h = a.alloc(NodeBox::U(Box::new(b)));
    node_edge(h, T_U)
}

/// Insert a `(digit, child)` for a digit **not currently present**,
/// growing the branch flavour if needed. `child` already carries its own
/// key(s); `keys_added` is the number of keys `child` contributes.
fn branch_insert_new(a: &mut Arena, e: &mut Edge32, digit: u8, child: Edge32, keys_added: u32) {
    let level = branch_level(a, e);
    match kind(e) {
        Kind::BranchL2 => {
            let n = a.l2(edge_handle(e)).header.num_edges as usize;
            if n < 2 {
                let b = a.l2_mut(edge_handle(e));
                // insert sorted
                let mut pos = 0;
                while pos < n && b.digits[pos] < digit {
                    pos += 1;
                }
                let mut i = n;
                while i > pos {
                    b.digits[i] = b.digits[i - 1];
                    b.edges[i] = b.edges[i - 1];
                    i -= 1;
                }
                b.digits[pos] = digit;
                b.edges[pos] = child;
                b.header.num_edges = (n + 1) as u8;
                b.header.pop0 += keys_added;
            } else {
                let total = a.l2(edge_handle(e)).header.pop0 + keys_added;
                let mut pairs = branch_pairs(a, e);
                insert_pair_sorted(&mut pairs, digit, child);
                let old = edge_handle(e);
                *e = make_l6(a, level, &pairs, total);
                a.free(old);
            }
        }
        Kind::BranchL6 => {
            let n = a.l6(edge_handle(e)).header.num_edges as usize;
            if n < BRANCH_L6_CAP {
                let b = a.l6_mut(edge_handle(e));
                let mut pos = 0;
                while pos < n && b.digits[pos] < digit {
                    pos += 1;
                }
                let mut i = n;
                while i > pos {
                    b.digits[i] = b.digits[i - 1];
                    b.edges[i] = b.edges[i - 1];
                    i -= 1;
                }
                b.digits[pos] = digit;
                b.edges[pos] = child;
                b.header.num_edges = (n + 1) as u8;
                b.header.pop0 += keys_added;
            } else {
                // 7th child promotes BranchL6_32 -> BranchB32 (Band 1: 64B vs 96B, 1.5x)
                let total = a.l6(edge_handle(e)).header.pop0 + keys_added;
                let mut pairs = branch_pairs(a, e);
                insert_pair_sorted(&mut pairs, digit, child);
                let old = edge_handle(e);
                *e = make_b(a, level, &pairs, total);
                a.free(old);
            }
        }
        Kind::BranchB => {
            let (promoted, retired_sub, bytes_delta) = {
                let b = a.b_mut(edge_handle(e));
                let n = b.num_children as usize;
                if n < BRANCH_B_TO_UNCOMPRESSED {
                    let w = (digit >> 6) as usize;
                    let bit64 = digit & 63;
                    let bit_mask = 1u64 << bit64;
                    debug_assert!((b.header.bitmap[w] & bit_mask) == 0);
                    let rank = bitmap_sub_rank(b.header.bitmap[w], digit);
                    b.header.bitmap[w] |= bit_mask;
                    let sub = (digit >> 5) as usize;

                    // Population before the insert: `pop_counts`, never
                    // `subarrays[sub].len()` — that is the cap-classed
                    // allocation length (#615).
                    let pop = b.header.pop_counts[sub] as usize;
                    let retired =
                        subarray_insert(&mut b.subarrays[sub], pop, rank, child, Edge32::null());
                    b.header.pop_counts[sub] += 1;
                    b.num_children += 1;
                    b.count += keys_added;
                    (None, retired, subarray_bytes_delta::<Edge32>(pop, pop + 1))
                } else {
                    (Some(b.count + keys_added), None, 0)
                }
            };
            // Replaced subarrays must outlive concurrent readers exactly
            // like freed nodes (a no-op outside deferred mode).
            a.retire_edges(retired_sub);
            if let Some(total) = promoted {
                let mut pairs = branch_pairs(a, e);
                insert_pair_sorted(&mut pairs, digit, child);
                let old = edge_handle(e);
                *e = make_u(a, level, &pairs, total);
                a.free(old);
            } else {
                a.bytes = a.bytes.wrapping_add_signed(bytes_delta);
            }
        }
        Kind::BranchU => {
            let b = a.u_mut(edge_handle(e));
            debug_assert!(b.edges[digit as usize].is_null());
            b.edges[digit as usize] = child;
            b.num_children += 1;
            b.count += keys_added;
        }
        _ => unreachable!(),
    }
}

fn insert_pair_sorted(pairs: &mut Vec<(u8, Edge32)>, digit: u8, child: Edge32) {
    let pos = pairs.partition_point(|&(d, _)| d < digit);
    pairs.insert(pos, (digit, child));
}

/// Remove `(digit, child)` from a branch. Returns `true` if the branch
/// was modified. The caller is responsible for freeing the child node
/// if applicable.
fn branch_remove_digit(a: &mut Arena, e: &mut Edge32, digit: u8, delta: i32) {
    let level = branch_level(a, e);
    match kind(e) {
        Kind::BranchL2 => {
            let b = a.l2_mut(edge_handle(e));
            b.header.pop0 = (b.header.pop0 as i32 + delta) as u32;
            let n = b.header.num_edges as usize;
            let mut pos = None;
            for i in 0..n {
                if b.digits[i] == digit {
                    pos = Some(i);
                    break;
                }
            }
            let pos = pos.expect("branch_remove_digit: digit not present");
            for i in pos..n - 1 {
                b.digits[i] = b.digits[i + 1];
                b.edges[i] = b.edges[i + 1];
            }
            b.digits[n - 1] = 0;
            b.edges[n - 1] = Edge32::null();
            b.header.num_edges = (n - 1) as u8;
            if n - 1 == 0 {
                let old = edge_handle(e);
                *e = Edge32::null();
                a.free(old);
            }
        }
        Kind::BranchL6 => {
            let b = a.l6_mut(edge_handle(e));
            b.header.pop0 = (b.header.pop0 as i32 + delta) as u32;
            let n = b.header.num_edges as usize;
            let mut pos = None;
            for i in 0..n {
                if b.digits[i] == digit {
                    pos = Some(i);
                    break;
                }
            }
            let pos = pos.expect("branch_remove_digit: digit not present");
            for i in pos..n - 1 {
                b.digits[i] = b.digits[i + 1];
                b.edges[i] = b.edges[i + 1];
            }
            b.digits[n - 1] = 0;
            b.edges[n - 1] = Edge32::null();
            b.header.num_edges = (n - 1) as u8;
            let new_n = n - 1;
            // Hysteresis band of 1 (demotes at <= 1).
            if new_n <= BRANCH_L6_DOWN {
                let total = a.l6(edge_handle(e)).header.pop0;
                let pairs = branch_pairs(a, e);
                let old = edge_handle(e);
                *e = if pairs.is_empty() {
                    Edge32::null()
                } else {
                    make_l2(a, level, &pairs, total)
                };
                a.free(old);
            }
        }
        Kind::BranchB => {
            let (new_n, total, retired_sub, bytes_delta) = {
                let b = a.b_mut(edge_handle(e));
                b.count = (b.count as i32 + delta) as u32;
                let w = (digit >> 6) as usize;
                let bit64 = digit & 63;
                let bit_mask = 1u64 << bit64;
                debug_assert!((b.header.bitmap[w] & bit_mask) != 0);
                let rank = bitmap_sub_rank(b.header.bitmap[w], digit);
                let sub = (digit >> 5) as usize;
                b.header.bitmap[w] &= !bit_mask;
                // Population before the removal: `pop_counts`, never
                // `subarrays[sub].len()` (#615).
                let pop = b.header.pop_counts[sub] as usize;
                b.header.pop_counts[sub] -= 1;
                b.num_children -= 1;

                let retired = subarray_remove(&mut b.subarrays[sub], pop, rank, Edge32::null());
                (
                    b.num_children as usize,
                    b.count,
                    retired,
                    subarray_bytes_delta::<Edge32>(pop, pop - 1),
                )
            };
            a.retire_edges(retired_sub);
            a.bytes = a.bytes.wrapping_add_signed(bytes_delta);

            // Demote BranchB32 -> BranchL6_32 when new_n <= 5 (Band 1, 64B vs 96B).
            if new_n <= BRANCH_B_DOWN {
                let pairs = branch_pairs(a, e);
                let old = edge_handle(e);
                *e = if pairs.is_empty() {
                    Edge32::null()
                } else {
                    make_l6(a, level, &pairs, total)
                };
                a.free(old);
            }
        }
        Kind::BranchU => {
            let b = a.u_mut(edge_handle(e));
            b.count = (b.count as i32 + delta) as u32;
            debug_assert!(!b.edges[digit as usize].is_null());
            b.edges[digit as usize] = Edge32::null();
            b.num_children -= 1;
            let new_n = b.num_children as usize;
            // Demote BranchU32 -> BranchB32 when new_n <= 190 (Band 2: 96B vs 2080B, 21.7x).
            if new_n <= BRANCH_U_DOWN {
                let total = a.u(edge_handle(e)).count;
                let pairs = branch_pairs(a, e);
                let old = edge_handle(e);
                *e = if pairs.is_empty() {
                    Edge32::null()
                } else {
                    make_b(a, level, &pairs, total)
                };
                a.free(old);
            }
        }
        _ => unreachable!(),
    }
}

// ---------------------------------------------------------------------------
// SET operations
// ---------------------------------------------------------------------------

/// Total keys in the subtree at `e`.
pub(crate) fn subtree_count(a: &Arena, e: &Edge32) -> usize {
    match kind(e) {
        Kind::Null => 0,
        Kind::SetImmed { count, .. } => count as usize,
        Kind::MapImmed { .. } => 1,
        Kind::SetLeaf(_) | Kind::MapLeaf(_) => edge_pop(e),
        Kind::Bitmap => bitmap_count(a, e),
        Kind::MapBitmap => (a.map_bitmap(edge_handle(e)).header.pop0 + 1) as usize,
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchB | Kind::BranchU => {
            branch_total_keys(a, e) as usize
        }
    }
}

pub(crate) fn set_contains(a: &Arena, e: &Edge32, mut kb: u8, mut rem: u32) -> bool {
    let mut edge = *e;
    loop {
        match kind(&edge) {
            Kind::Null => return false,
            Kind::SetImmed { kb: _, count } => {
                let (keys, n) = set_immed_keys(&edge, kb, count);
                return keys[..n].binary_search(&rem).is_ok();
            }
            Kind::SetLeaf(_) => {
                let pop = edge_pop(&edge);
                let buf = a.leaf(edge_handle(&edge));
                return leaf_lower_bound(buf, pop, kb, rem)
                    .map(|pos| pos < pop && read_rem(buf, pos, kb as usize) == rem)
                    .unwrap_or(false);
            }
            Kind::Bitmap => return a.bitmap(edge_handle(&edge)).test(rem as u8),
            Kind::BranchL2 | Kind::BranchL6 | Kind::BranchB | Kind::BranchU => {
                let d = digit_at(rem, kb);
                match branch_child(a, &edge, d) {
                    Some(c) => {
                        edge = c;
                        rem = child_rem(rem, kb);
                        kb -= 1;
                    }
                    None => return false,
                }
            }
            _ => return false,
        }
    }
}

/// First index whose packed key is `>= needle`; `None` means all keys are
/// less than `needle`.
fn leaf_lower_bound(buf: &[u8], pop: usize, kb: u8, needle: u32) -> Option<usize> {
    let (mut lo, mut hi) = (0usize, pop);
    while lo < hi {
        let mid = (lo + hi) / 2;
        if read_rem(buf, mid, kb as usize) < needle {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    Some(lo)
}

/// Insert position for `needle` in a linear leaf's key area, short-circuiting
/// the ascending-append case.
///
/// Monotonic key arrival is the common telemetry shape, and it always lands
/// past the last key. One compare answers it; the binary search below needs
/// `log2(pop)` dependent loads to reach the same place. The 64-bit engine has
/// had this since ALGORITHMS.md §3.2 item 2 -- the 32-bit leaves never did, so
/// every ascending insert paid the full descent of the search.
///
/// A key equal to the last one falls through to the search, so the caller sees
/// the duplicate at its real index exactly as before.
#[inline(always)]
fn leaf_insert_pos(buf: &[u8], pop: usize, kb: u8, needle: u32) -> usize {
    if pop > 0 && needle > read_rem(buf, pop - 1, kb as usize) {
        return pop;
    }
    leaf_lower_bound(buf, pop, kb, needle).unwrap()
}

pub(crate) fn set_insert(a: &mut Arena, e: &mut Edge32, kb: u8, rem: u32) -> bool {
    match kind(e) {
        Kind::Null => {
            *e = set_immed_edge(kb, &[rem]);
            true
        }
        Kind::SetImmed { kb: _, count } => {
            let (keys, n) = set_immed_keys(e, kb, count);
            match keys[..n].binary_search(&rem) {
                Ok(_) => false,
                Err(pos) => {
                    let mut v = [0u32; 8];
                    v[..pos].copy_from_slice(&keys[..pos]);
                    v[pos] = rem;
                    v[pos + 1..=n].copy_from_slice(&keys[pos..n]);
                    if n < set_immed_cap(kb) {
                        *e = set_immed_edge(kb, &v[..=n]);
                    } else {
                        // Promote immediate -> linear leaf.
                        *e = make_set_leaf(a, kb, &v[..=n]);
                    }
                    true
                }
            }
        }
        Kind::SetLeaf(_) => {
            let pop = edge_pop(e);
            let buf = a.leaf(edge_handle(e));
            let pos = leaf_insert_pos(buf, pop, kb, rem);
            if pos < pop && read_rem(buf, pos, kb as usize) == rem {
                return false;
            }
            let new_pop = pop + 1;
            if kb == 1 && new_pop > SET_BITMAP_ENTER {
                let mut keys = read_set_leaf(a, e, kb);
                keys.insert(pos, rem);
                let old = edge_handle(e);
                *e = bitmap_from_keys(a, &keys);
                a.free(old);
            } else if kb >= 2 && new_pop > SET_LEAF_MAX {
                // Convert to a branch: rebuild by descent.
                let keys = read_set_leaf(a, e, kb);
                let old = edge_handle(e);
                *e = make_l2(a, kb, &[], 0);
                for &k in &keys {
                    set_insert(a, e, kb, k);
                }
                set_insert(a, e, kb, rem);
                a.free(old);
            } else {
                set_leaf_insert_at(a, e, kb, pos, rem);
            }
            true
        }
        Kind::Bitmap => a.bitmap_mut(edge_handle(e)).set(rem as u8),
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchB | Kind::BranchU => {
            let d = digit_at(rem, kb);
            let cr = child_rem(rem, kb);
            match branch_child(a, e, d) {
                Some(mut child) => {
                    let old_child = child;
                    let inserted = set_insert(a, &mut child, kb - 1, cr);
                    if inserted {
                        branch_commit(
                            a,
                            e,
                            d,
                            if child != old_child {
                                child
                            } else {
                                Edge32::null()
                            },
                            1,
                        );
                    }
                    inserted
                }
                None => {
                    let mut child = Edge32::null();
                    set_insert(a, &mut child, kb - 1, cr);
                    branch_insert_new(a, e, d, child, 1);
                    true
                }
            }
        }
        Kind::MapLeaf(_) | Kind::MapImmed { .. } | Kind::MapBitmap => {
            unreachable!("set op on a map edge")
        }
    }
}

pub(crate) fn set_remove(a: &mut Arena, e: &mut Edge32, kb: u8, rem: u32) -> bool {
    match kind(e) {
        Kind::Null => false,
        Kind::SetImmed { kb: _, count } => {
            let (keys, n) = set_immed_keys(e, kb, count);
            match keys[..n].binary_search(&rem) {
                Err(_) => false,
                Ok(pos) => {
                    let mut v = [0u32; 7];
                    v[..pos].copy_from_slice(&keys[..pos]);
                    v[pos..n - 1].copy_from_slice(&keys[pos + 1..n]);
                    *e = if n == 1 {
                        Edge32::null()
                    } else {
                        set_immed_edge(kb, &v[..n - 1])
                    };
                    true
                }
            }
        }
        Kind::SetLeaf(_) => {
            let pop = edge_pop(e);
            let buf = a.leaf(edge_handle(e));
            let pos = leaf_lower_bound(buf, pop, kb, rem).unwrap();
            if pos >= pop || read_rem(buf, pos, kb as usize) != rem {
                return false;
            }
            let new_pop = pop - 1;
            if new_pop == 0 {
                let old = edge_handle(e);
                *e = Edge32::null();
                a.free(old);
            } else if new_pop <= set_immed_cap(kb) {
                // Demote to an immediate: collect the survivors on the stack.
                let kbz = kb as usize;
                let mut v = [0u32; 7];
                let mut n = 0;
                for i in 0..pop {
                    if i != pos {
                        v[n] = read_rem(buf, i, kbz);
                        n += 1;
                    }
                }
                let old = edge_handle(e);
                *e = set_immed_edge(kb, &v[..n]);
                a.free(old);
            } else {
                set_leaf_remove_at(a, e, kb, pos);
            }
            true
        }
        Kind::Bitmap => {
            let removed = a.bitmap_mut(edge_handle(e)).unset(rem as u8);
            if removed {
                let pop = a.bitmap(edge_handle(e)).pop0 as usize;
                if pop == 0 {
                    let old = edge_handle(e);
                    a.free(old);
                    *e = Edge32::null();
                } else if pop <= SET_BITMAP_LEAVE {
                    let keys = bitmap_keys(a, e);
                    let old = edge_handle(e);
                    *e = if keys.len() <= set_immed_cap(1) {
                        set_immed_edge(1, &keys)
                    } else {
                        make_set_leaf(a, 1, &keys)
                    };
                    a.free(old);
                }
            }
            removed
        }
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchB | Kind::BranchU => {
            let d = digit_at(rem, kb);
            let cr = child_rem(rem, kb);
            match branch_child(a, e, d) {
                None => false,
                Some(mut child) => {
                    let old_child = child;
                    let removed = set_remove(a, &mut child, kb - 1, cr);
                    if removed {
                        if child.is_null() {
                            branch_remove_digit(a, e, d, -1);
                        } else {
                            branch_commit(
                                a,
                                e,
                                d,
                                if child != old_child {
                                    child
                                } else {
                                    Edge32::null()
                                },
                                -1,
                            );
                        }
                    }
                    removed
                }
            }
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// MAP operations
// ---------------------------------------------------------------------------

pub(crate) fn map_get(a: &Arena, e: &Edge32, mut kb: u8, mut rem: u32) -> Option<u32> {
    let mut edge = *e;
    loop {
        match kind(&edge) {
            Kind::Null => return None,
            Kind::MapImmed { kb: _ } => {
                return if map_immed_rem(&edge, kb) == rem {
                    Some(map_immed_val(&edge))
                } else {
                    None
                };
            }
            Kind::MapLeaf(_) => {
                let pop = edge_pop(&edge);
                let cap = cap_class(pop);
                let buf = a.leaf(edge_handle(&edge));
                let keys_off = 4 * cap;
                let pos = leaf_lower_bound(&buf[keys_off..], pop, kb, rem)?;
                return if pos < pop && read_rem(&buf[keys_off..], pos, kb as usize) == rem {
                    let mut vb = [0u8; 4];
                    vb.copy_from_slice(&buf[pos * 4..pos * 4 + 4]);
                    Some(u32::from_le_bytes(vb))
                } else {
                    None
                };
            }
            Kind::MapBitmap => {
                let b = a.map_bitmap(edge_handle(&edge));
                let digit = rem as u8;
                let w = (digit >> 6) as usize;
                let word = b.header.bitmap[w];
                let bit_mask = 1u64 << (digit & 63);
                if (word & bit_mask) == 0 {
                    return None;
                }
                let rank = bitmap_sub_rank(word, digit);
                let sub = (digit >> 5) as usize;
                return b.subarrays[sub].as_ref().map(|s| s[rank]);
            }
            Kind::BranchL2 | Kind::BranchL6 | Kind::BranchB | Kind::BranchU => {
                let d = digit_at(rem, kb);
                let c = branch_child(a, &edge, d)?;
                edge = c;
                rem = child_rem(rem, kb);
                kb -= 1;
            }
            _ => return None,
        }
    }
}

/// Cached descent to the level-1 bitmap leaf the last insert terminated in.
///
/// Consecutive keys of a monotonic stream share their top three digits, so
/// every insert into the same expanse re-walks an identical path: four tag
/// dispatches, three `branch_child` arena resolutions on the way down and
/// three `branch_commit` resolutions on the way up. The finger answers the
/// repeat directly -- one resolution for the leaf, one per ancestor for its
/// subtree count.
///
/// It is only armed for a `MapBitmap`/`Bitmap` terminal at `kb == 1`, which is
/// the one form an insert cannot restructure: the bit is set in place, the
/// handle and tag do not change, and no ancestor gains a digit. Every other
/// terminal, and every removal, disarms it.
///
/// Ancestors are held as `Edge32` **by value** rather than as references: the
/// arena's `slots` vector reallocates on growth, so a borrow into a node does
/// not survive the next allocation, while a handle does.
#[derive(Clone, Copy)]
pub(crate) struct Finger32 {
    /// `key >> 8` of the expanse this path descends to.
    prefix: u32,
    /// Arena handle of the terminal level-1 bitmap leaf.
    leaf: u32,
    /// Branch edges from the terminal upward, `depth` of them valid.
    ancestors: [Edge32; 3],
    depth: u8,
    /// False until a descent arms it, and again the moment anything could
    /// have moved the path.
    valid: bool,
}

impl Finger32 {
    #[inline]
    pub(crate) const fn new() -> Self {
        Self {
            prefix: 0,
            leaf: 0,
            ancestors: [Edge32::null(); 3],
            depth: 0,
            valid: false,
        }
    }

    /// Records which expanse the armed path belongs to. Set by the container
    /// after a descent, since the recursion only ever sees a remainder.
    #[inline]
    pub(crate) fn set_prefix(&mut self, prefix: u32) {
        self.prefix = prefix;
    }

    /// Disarms the path. Cheap enough to call unconditionally from any
    /// operation that is not an insert.
    #[inline]
    pub(crate) fn clear(&mut self) {
        self.valid = false;
    }

    /// Records the terminal a descent reached. Called by the leaf arm.
    #[inline]
    fn arm(&mut self, leaf: u32) {
        self.leaf = leaf;
        self.depth = 0;
        self.valid = true;
    }

    /// Records one branch on the way back up, nearest-first.
    #[inline]
    fn record(&mut self, e: &Edge32) {
        if self.valid && (self.depth as usize) < self.ancestors.len() {
            self.ancestors[self.depth as usize] = *e;
            self.depth += 1;
        }
    }
}

/// Inserts through a cached path, or reports that the path does not apply.
///
/// Returns `None` when the finger is not armed for this key, in which case the
/// caller must take the full descent. A `Some` return is the ordinary
/// `map_insert` result: the previous value, or `None` for a new key.
///
/// The work skipped is four tag dispatches, three digit decodes, three
/// `branch_child` arena resolutions and three `branch_commit` ones. What
/// remains is the leaf store plus one count bump per ancestor.
#[inline(always)]
pub(crate) fn map_insert_via_finger(
    a: &mut Arena,
    key: u32,
    val: u32,
    f: &Finger32,
) -> Option<Option<u32>> {
    if !f.valid || f.prefix != key >> 8 {
        return None;
    }
    let digit = key as u8;
    let w = (digit >> 6) as usize;
    let bit_mask = 1u64 << (digit & 63);
    let sub = (digit >> 5) as usize;

    let (old, bytes_delta, retired) = {
        let b = a.map_bitmap_mut(f.leaf);
        let word = b.header.bitmap[w];
        let rank = bitmap_sub_rank(word, digit);
        if (word & bit_mask) != 0 {
            // Overwrite: a single in-place store, and no count anywhere moves.
            let prev = b.subarrays[sub].as_ref().unwrap()[rank];
            b.subarrays[sub].as_mut().unwrap()[rank] = val;
            return Some(Some(prev));
        }
        let pop = bitmap_sub_pop(word, digit);
        b.header.bitmap[w] |= bit_mask;
        b.header.pop0 += 1;
        let retired = subarray_insert(&mut b.subarrays[sub], pop, rank, val, 0u32);
        (None, subarray_bytes_delta::<u32>(pop, pop + 1), retired)
    };
    a.retire_vals(retired);
    a.bytes = a.bytes.wrapping_add_signed(bytes_delta);

    // Subtree counts, the only thing the skipped descent still owed. The
    // digit is unused for a count-only commit.
    for i in 0..f.depth as usize {
        branch_commit(a, &f.ancestors[i], 0, Edge32::null(), 1);
    }
    Some(old)
}

/// Insert without a finger, for callers that hold no cached path (bulk
/// rebuilds, promotions, the blob map's index).
pub(crate) fn map_insert(a: &mut Arena, e: &mut Edge32, kb: u8, rem: u32, val: u32) -> Option<u32> {
    let mut scratch = Finger32::new();
    map_insert_f(a, e, kb, rem, val, &mut scratch)
}

/// Insert, recording the descent in `f` when it terminates somewhere the
/// finger can be reused. Any other outcome disarms it.
pub(crate) fn map_insert_f(
    a: &mut Arena,
    e: &mut Edge32,
    kb: u8,
    rem: u32,
    val: u32,
    f: &mut Finger32,
) -> Option<u32> {
    match kind(e) {
        Kind::Null => {
            f.clear();
            *e = if kb <= 3 {
                map_immed_edge(kb, rem, val)
            } else {
                make_map_leaf(a, kb, &[(rem, val)])
            };
            None
        }
        Kind::MapImmed { kb: _ } => {
            f.clear();
            let r0 = map_immed_rem(e, kb);
            if r0 == rem {
                let old = map_immed_val(e);
                *e = map_immed_edge(kb, rem, val);
                Some(old)
            } else {
                let v0 = map_immed_val(e);
                // `r0 != rem` here, so the pair is always two entries.
                let entries = if r0 < rem {
                    [(r0, v0), (rem, val)]
                } else {
                    [(rem, val), (r0, v0)]
                };
                *e = make_map_leaf(a, kb, &entries);
                None
            }
        }
        Kind::MapLeaf(_) => {
            f.clear();
            let pop = edge_pop(e);
            let cap = cap_class(pop);
            let keys_off = 4 * cap;
            let buf = a.leaf(edge_handle(e));
            let pos = leaf_insert_pos(&buf[keys_off..], pop, kb, rem);
            if pos < pop && read_rem(&buf[keys_off..], pos, kb as usize) == rem {
                // Value overwrite: a single in-place word store; the edge
                // (handle, pop) is unchanged.
                let buf = a.leaf_mut(edge_handle(e));
                let mut vb = [0u8; 4];
                vb.copy_from_slice(&buf[pos * 4..pos * 4 + 4]);
                buf[pos * 4..pos * 4 + 4].copy_from_slice(&val.to_le_bytes());
                return Some(u32::from_le_bytes(vb));
            }
            let new_pop = pop + 1;
            if kb == 1 && new_pop > MAP_BITMAP_ENTER {
                let mut entries = read_map_leaf(a, e, kb);
                entries.insert(pos, (rem, val));
                let oldh = edge_handle(e);
                *e = make_map_bitmap(a, &entries);
                a.free(oldh);
            } else if kb >= 2 && new_pop > MAP_LEAF_MAX {
                let entries = read_map_leaf(a, e, kb);
                let oldh = edge_handle(e);
                *e = make_l2(a, kb, &[], 0);
                for &(k, v) in &entries {
                    map_insert(a, e, kb, k, v);
                }
                map_insert(a, e, kb, rem, val);
                a.free(oldh);
            } else {
                map_leaf_insert_at(a, e, kb, pos, rem, val);
            }
            None
        }
        Kind::MapBitmap => {
            // The one terminal an insert cannot restructure: the bit is set in
            // place and the handle and tag are unchanged, so the path stays
            // valid for every other key of this expanse.
            f.arm(edge_handle(e));
            let (old, bytes_delta, retired_sub) = {
                let b = a.map_bitmap_mut(edge_handle(e));
                let digit = rem as u8;
                let w = (digit >> 6) as usize;
                let word = b.header.bitmap[w];
                let bit_mask = 1u64 << (digit & 63);
                let rank = bitmap_sub_rank(word, digit);
                let sub = (digit >> 5) as usize;
                if (word & bit_mask) != 0 {
                    let old = b.subarrays[sub].as_ref().unwrap()[rank];
                    b.subarrays[sub].as_mut().unwrap()[rank] = val;
                    (Some(old), 0, None)
                } else {
                    // Population before the insert, read off the *pre-set*
                    // bitmap word: `subarrays[sub].len()` is the cap-classed
                    // allocation length, not the population (#615).
                    let pop = bitmap_sub_pop(word, digit);
                    b.header.bitmap[w] |= bit_mask;
                    b.header.pop0 += 1;
                    let retired = subarray_insert(&mut b.subarrays[sub], pop, rank, val, 0u32);
                    (None, subarray_bytes_delta::<u32>(pop, pop + 1), retired)
                }
            };
            a.retire_vals(retired_sub);
            a.bytes = a.bytes.wrapping_add_signed(bytes_delta);
            old
        }
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchB | Kind::BranchU => {
            let d = digit_at(rem, kb);
            let cr = child_rem(rem, kb);
            match branch_child(a, e, d) {
                Some(mut child) => {
                    let old_child = child;
                    let old = map_insert_f(a, &mut child, kb - 1, cr, val, f);
                    let changed = child != old_child;
                    if changed || old.is_none() {
                        branch_commit(
                            a,
                            e,
                            d,
                            if changed { child } else { Edge32::null() },
                            i32::from(old.is_none()),
                        );
                    }
                    if changed {
                        // The child edge moved, so a cached path through it
                        // would resolve to the wrong node.
                        f.clear();
                    }
                    f.record(e);
                    old
                }
                None => {
                    let mut child = Edge32::null();
                    map_insert_f(a, &mut child, kb - 1, cr, val, f);
                    // A new digit can upgrade this branch, replacing the node
                    // the finger would cache.
                    f.clear();
                    branch_insert_new(a, e, d, child, 1);
                    None
                }
            }
        }
        _ => None,
    }
}

pub(crate) fn map_remove(a: &mut Arena, e: &mut Edge32, kb: u8, rem: u32) -> Option<u32> {
    match kind(e) {
        Kind::Null => None,
        Kind::MapImmed { kb: _ } => {
            if map_immed_rem(e, kb) == rem {
                let old = map_immed_val(e);
                *e = Edge32::null();
                Some(old)
            } else {
                None
            }
        }
        Kind::MapLeaf(_) => {
            let pop = edge_pop(e);
            let cap = cap_class(pop);
            let keys_off = 4 * cap;
            let buf = a.leaf(edge_handle(e));
            let pos = leaf_lower_bound(&buf[keys_off..], pop, kb, rem).unwrap();
            if pos >= pop || read_rem(&buf[keys_off..], pos, kb as usize) != rem {
                return None;
            }
            let mut vb = [0u8; 4];
            vb.copy_from_slice(&buf[pos * 4..pos * 4 + 4]);
            let old = u32::from_le_bytes(vb);
            let new_pop = pop - 1;
            if new_pop == 0 {
                let oldh = edge_handle(e);
                *e = Edge32::null();
                a.free(oldh);
            } else if new_pop == 1 && kb <= 3 {
                // Demote to an immediate: the survivor is the other entry.
                let i = if pos == 0 { 1 } else { 0 };
                let k = read_rem(&buf[keys_off..], i, kb as usize);
                let mut sb = [0u8; 4];
                sb.copy_from_slice(&buf[i * 4..i * 4 + 4]);
                let oldh = edge_handle(e);
                *e = map_immed_edge(kb, k, u32::from_le_bytes(sb));
                a.free(oldh);
            } else {
                map_leaf_remove_at(a, e, kb, pos);
            }
            Some(old)
        }
        Kind::MapBitmap => {
            let (old_val, count_after, retired_sub, bytes_delta) = {
                let b = a.map_bitmap_mut(edge_handle(e));
                let digit = rem as u8;
                let w = (digit >> 6) as usize;
                let word = b.header.bitmap[w];
                let bit_mask = 1u64 << (digit & 63);
                if (word & bit_mask) == 0 {
                    return None;
                }
                let rank = bitmap_sub_rank(word, digit);
                let sub = (digit >> 5) as usize;
                // Subexpanse population before the removal, off the
                // *pre-clear* word (#615).
                let sub_pop = bitmap_sub_pop(word, digit);
                b.header.bitmap[w] &= !bit_mask;
                let old_val = b.subarrays[sub].as_ref().expect("live subarray")[rank];
                let retired = subarray_remove(&mut b.subarrays[sub], sub_pop, rank, 0u32);
                let count_after = b.header.pop0.saturating_sub(1) as usize;
                b.header.pop0 = count_after as u16;
                (
                    old_val,
                    count_after,
                    retired,
                    subarray_bytes_delta::<u32>(sub_pop, sub_pop - 1),
                )
            };
            a.retire_vals(retired_sub);
            a.bytes = a.bytes.wrapping_add_signed(bytes_delta);
            if count_after == 0 {
                let old = edge_handle(e);
                a.free(old);
                *e = Edge32::null();
            } else if count_after <= MAP_BITMAP_LEAVE {
                let entries = read_map_bitmap(a, e);
                let old = edge_handle(e);
                *e = if entries.len() == 1 {
                    map_immed_edge(1, entries[0].0, entries[0].1)
                } else {
                    make_map_leaf(a, 1, &entries)
                };
                a.free(old);
            }
            Some(old_val)
        }
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchB | Kind::BranchU => {
            let d = digit_at(rem, kb);
            let cr = child_rem(rem, kb);
            match branch_child(a, e, d) {
                None => None,
                Some(mut child) => {
                    let old_child = child;
                    let old = map_remove(a, &mut child, kb - 1, cr);
                    if old.is_some() {
                        if child.is_null() {
                            branch_remove_digit(a, e, d, -1);
                        } else {
                            branch_commit(
                                a,
                                e,
                                d,
                                if child != old_child {
                                    child
                                } else {
                                    Edge32::null()
                                },
                                -1,
                            );
                        }
                    }
                    old
                }
            }
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Navigation (shared shape; returns remainder assembled from this level
// down — at the root, `kb == 4`, so it returns the full key)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Validated optimistic reads (sync32)
// ---------------------------------------------------------------------------

/// Sentinel: a racy read observed a concurrent mutation (torn data, a
/// version change, or an impossible structure) and the caller must retry
/// or report `Busy`.
pub(crate) struct Torn;

impl Arena {
    /// Non-panicking slot resolution for the optimistic read path. A racy
    /// walk can fabricate garbage handles, so out-of-bounds handles and
    /// empty slots report [`Torn`] instead of panicking.
    #[inline]
    fn try_node(&self, h: u32) -> Result<&NodeBox, Torn> {
        self.slots
            .get(h as usize)
            .and_then(|s| s.as_ref())
            .ok_or(Torn)
    }
}

/// `Ok(v)` if the caller's version word is still unchanged, else [`Torn`].
#[inline]
fn seal<F: Fn() -> bool, T>(still_valid: &F, v: T) -> Result<T, Torn> {
    if still_valid() { Ok(v) } else { Err(Torn) }
}

/// Resolves the child edge for `digit` under the optimistic-read
/// discipline: every racily loaded pointer is validated **before** its
/// first dereference (the "validate before following" rule of optimistic
/// lock coupling — Leis, Scheibner, Kemper & Neumann, DaMoN 2016), every
/// content-derived index is bounds-checked, and an "absent" answer is
/// validated before it is trusted (torn content may misreport absence).
fn branch_child_validated<F: Fn() -> bool>(
    a: &Arena,
    e: &Edge32,
    digit: u8,
    still_valid: &F,
) -> Result<Option<Edge32>, Torn> {
    let node = a.try_node(edge_handle(e))?;
    match (kind(e), node) {
        (Kind::BranchL2, NodeBox::L2(bx)) => {
            let b: &BranchL2_32 = bx;
            if !still_valid() {
                return Err(Torn);
            }
            let n = (b.header.num_edges as usize).min(2);
            for i in 0..n {
                if b.digits[i] == digit {
                    return seal(still_valid, Some(b.edges[i]));
                }
            }
            seal(still_valid, None)
        }
        (Kind::BranchL6, NodeBox::L6(bx)) => {
            let b: &BranchL6_32 = bx;
            if !still_valid() {
                return Err(Torn);
            }
            let n = (b.header.num_edges as usize).min(BRANCH_L6_CAP);
            for i in 0..n {
                if b.digits[i] == digit {
                    return seal(still_valid, Some(b.edges[i]));
                }
            }
            seal(still_valid, None)
        }
        (Kind::BranchB, NodeBox::B(bx)) => {
            let b: &BranchB32Data = bx;
            if !still_valid() {
                return Err(Torn);
            }
            let w = (digit >> 6) as usize;
            let word = b.header.bitmap[w];
            let bit = 1u64 << (digit & 63);
            if (word & bit) == 0 {
                return seal(still_valid, None);
            }
            let rank = bitmap_sub_rank(word, digit);
            let sub = (digit >> 5) as usize;
            let Some(sb) = b.subarrays[sub].as_ref() else {
                return Err(Torn);
            };
            let edges: &[Edge32] = sb;
            if !still_valid() {
                return Err(Torn);
            }
            let Some(&c) = edges.get(rank) else {
                return Err(Torn);
            };
            seal(still_valid, Some(c))
        }
        (Kind::BranchU, NodeBox::U(bx)) => {
            let b: &BranchU32 = bx;
            if !still_valid() {
                return Err(Torn);
            }
            let c = b.edges[digit as usize];
            if c.is_null() {
                seal(still_valid, None)
            } else {
                seal(still_valid, Some(c))
            }
        }
        _ => Err(Torn),
    }
}

/// Validated optimistic map lookup over a tree a single seqlock-bracketed
/// writer may be mutating concurrently.
///
/// Contract: the caller read `root` racily, then validated its version
/// word, and `still_valid` returns `true` iff that word is still
/// unchanged. Any data wholly derived from already-validated bytes is
/// consistent as of the sample (the result linearizes there); any later
/// racy read is re-validated before being used to form an address, and
/// sealed before being returned as data. Depth is explicitly bounded — 4
/// branch hops resolve any 32-bit key, so deeper structures are torn by
/// definition.
pub(crate) fn map_get_validated<F: Fn() -> bool>(
    a: &Arena,
    root: Edge32,
    key: u32,
    still_valid: &F,
) -> Result<Option<u32>, Torn> {
    let mut edge = root;
    let mut kb: u8 = 4;
    let mut rem = key;
    for _ in 0..4 {
        match kind(&edge) {
            Kind::Null => return Ok(None),
            Kind::MapImmed { .. } => {
                return Ok(if map_immed_rem(&edge, kb) == rem {
                    Some(map_immed_val(&edge))
                } else {
                    None
                });
            }
            Kind::MapLeaf(_) => {
                let pop = edge_pop(&edge);
                let cap = cap_class(pop);
                let node = a.try_node(edge_handle(&edge))?;
                let NodeBox::Leaf(bx) = node else {
                    return Err(Torn);
                };
                let buf: &[u8] = bx;
                if !still_valid() {
                    return Err(Torn);
                }
                let kbz = kb as usize;
                let keys_off = 4usize.checked_mul(cap).ok_or(Torn)?;
                let keys_len = pop.checked_mul(kbz).ok_or(Torn)?;
                let keys_end = keys_off.checked_add(keys_len).ok_or(Torn)?;
                if keys_end > buf.len() || pop > cap {
                    return Err(Torn);
                }
                let keys = &buf[keys_off..];
                let Some(pos) = leaf_lower_bound(keys, pop, kb, rem) else {
                    return seal(still_valid, None);
                };
                if pos >= pop || read_rem(keys, pos, kbz) != rem {
                    return seal(still_valid, None);
                }
                let mut vb = [0u8; 4];
                vb.copy_from_slice(&buf[pos * 4..pos * 4 + 4]);
                return seal(still_valid, Some(u32::from_le_bytes(vb)));
            }
            Kind::MapBitmap => {
                let node = a.try_node(edge_handle(&edge))?;
                let NodeBox::MapBitmap(bx) = node else {
                    return Err(Torn);
                };
                let b: &LeafBitmapL32Data = bx;
                if !still_valid() {
                    return Err(Torn);
                }
                let digit = rem as u8;
                let w = (digit >> 6) as usize;
                let word = b.header.bitmap[w];
                let bit = 1u64 << (digit & 63);
                if (word & bit) == 0 {
                    return seal(still_valid, None);
                }
                let rank = bitmap_sub_rank(word, digit);
                let sub = (digit >> 5) as usize;
                let Some(sb) = b.subarrays[sub].as_ref() else {
                    return Err(Torn);
                };
                let vals: &[u32] = sb;
                if !still_valid() {
                    return Err(Torn);
                }
                let Some(&v) = vals.get(rank) else {
                    return Err(Torn);
                };
                return seal(still_valid, Some(v));
            }
            Kind::BranchL2 | Kind::BranchL6 | Kind::BranchB | Kind::BranchU => {
                match branch_child_validated(a, &edge, digit_at(rem, kb), still_valid)? {
                    Some(c) => {
                        edge = c;
                        rem = child_rem(rem, kb);
                        kb -= 1;
                        if kb == 0 {
                            return Err(Torn);
                        }
                    }
                    None => return Ok(None),
                }
            }
            // Set-flavoured kinds inside a map tree only arise from torn
            // reads; report them as such rather than as a miss.
            _ => return Err(Torn),
        }
    }
    Err(Torn)
}

/// Validated optimistic set membership test. Same contract and discipline
/// as [`map_get_validated`].
pub(crate) fn set_contains_validated<F: Fn() -> bool>(
    a: &Arena,
    root: Edge32,
    key: u32,
    still_valid: &F,
) -> Result<bool, Torn> {
    let mut edge = root;
    let mut kb: u8 = 4;
    let mut rem = key;
    for _ in 0..4 {
        match kind(&edge) {
            Kind::Null => return Ok(false),
            Kind::SetImmed { kb: _, count } => {
                // A torn tag can claim more keys than the 7 payload bytes
                // hold; clamp before decoding.
                let n = (count as usize).min(set_immed_cap(kb)) as u8;
                let (keys, n) = set_immed_keys(&edge, kb, n);
                return Ok(keys[..n].binary_search(&rem).is_ok());
            }
            Kind::SetLeaf(_) => {
                let pop = edge_pop(&edge);
                let node = a.try_node(edge_handle(&edge))?;
                let NodeBox::Leaf(bx) = node else {
                    return Err(Torn);
                };
                let buf: &[u8] = bx;
                if !still_valid() {
                    return Err(Torn);
                }
                let kbz = kb as usize;
                if pop.checked_mul(kbz).ok_or(Torn)? > buf.len() {
                    return Err(Torn);
                }
                let hit = leaf_lower_bound(buf, pop, kb, rem)
                    .map(|pos| pos < pop && read_rem(buf, pos, kbz) == rem)
                    .unwrap_or(false);
                return seal(still_valid, hit);
            }
            Kind::Bitmap => {
                let node = a.try_node(edge_handle(&edge))?;
                let NodeBox::Bitmap(bx) = node else {
                    return Err(Torn);
                };
                let b: &LeafBitmap1_32 = bx;
                if !still_valid() {
                    return Err(Torn);
                }
                return seal(still_valid, b.test(rem as u8));
            }
            Kind::BranchL2 | Kind::BranchL6 | Kind::BranchB | Kind::BranchU => {
                match branch_child_validated(a, &edge, digit_at(rem, kb), still_valid)? {
                    Some(c) => {
                        edge = c;
                        rem = child_rem(rem, kb);
                        kb -= 1;
                        if kb == 0 {
                            return Err(Torn);
                        }
                    }
                    None => return Ok(false),
                }
            }
            _ => return Err(Torn),
        }
    }
    Err(Torn)
}

pub(crate) fn first(a: &Arena, e: &Edge32, kb: u8) -> Option<u32> {
    match kind(e) {
        Kind::Null => None,
        Kind::SetImmed { kb: _, count } => {
            let (keys, n) = set_immed_keys(e, kb, count);
            keys[..n].iter().copied().min()
        }
        Kind::MapImmed { kb: _ } => Some(map_immed_rem(e, kb)),
        Kind::SetLeaf(_) => {
            let buf = a.leaf(edge_handle(e));
            (edge_pop(e) > 0).then(|| read_rem(buf, 0, kb as usize))
        }
        Kind::MapLeaf(_) => {
            let pop = edge_pop(e);
            if pop == 0 {
                return None;
            }
            let cap = cap_class(pop);
            let buf = a.leaf(edge_handle(e));
            Some(read_rem(&buf[4 * cap..], 0, kb as usize))
        }
        Kind::Bitmap => bitmap_first_ge(a.bitmap(edge_handle(e)), 0).map(u32::from),
        Kind::MapBitmap => {
            bitmap_first_ge_raw(&a.map_bitmap(edge_handle(e)).header.bitmap, 0).map(u32::from)
        }
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchB | Kind::BranchU => {
            let (d, c) = branch_first_child(a, e)?;
            first(a, &c, kb - 1).map(|cr| combine(d, cr, kb))
        }
    }
}

/// Smallest `(key, value)` under `e` in a single descent — [`first`] plus
/// the value it sits next to, so `ExpanseMap32::first` no longer pays a
/// second full `get` walk for the value (#578).
pub(crate) fn first_entry(a: &Arena, e: &Edge32, kb: u8) -> Option<(u32, u32)> {
    match kind(e) {
        Kind::Null => None,
        Kind::MapImmed { kb: _ } => Some((map_immed_rem(e, kb), map_immed_val(e))),
        Kind::MapLeaf(_) => {
            let pop = edge_pop(e);
            if pop == 0 {
                return None;
            }
            let cap = cap_class(pop);
            let buf = a.leaf(edge_handle(e));
            Some((
                read_rem(&buf[4 * cap..], 0, kb as usize),
                map_leaf_value(buf, 0),
            ))
        }
        Kind::MapBitmap => {
            let b = a.map_bitmap(edge_handle(e));
            let digit = bitmap_first_ge_raw(&b.header.bitmap, 0)?;
            map_bitmap_entry(b, digit)
        }
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchB | Kind::BranchU => {
            let (d, c) = branch_first_child(a, e)?;
            first_entry(a, &c, kb - 1).map(|(cr, v)| (combine(d, cr, kb), v))
        }
        Kind::SetImmed { .. } | Kind::SetLeaf(_) | Kind::Bitmap => {
            unreachable!("map op on a set edge")
        }
    }
}

/// Value beside key index `i` in a map linear leaf buffer.
#[inline]
fn map_leaf_value(buf: &[u8], i: usize) -> u32 {
    let mut vb = [0u8; 4];
    vb.copy_from_slice(&buf[i * 4..i * 4 + 4]);
    u32::from_le_bytes(vb)
}

/// The `(remainder, value)` a map bitmap leaf holds at `digit`.
#[inline]
fn map_bitmap_entry(b: &LeafBitmapL32Data, digit: u8) -> Option<(u32, u32)> {
    let w = (digit >> 6) as usize;
    let rank = bitmap_sub_rank(b.header.bitmap[w], digit);
    let sub = (digit >> 5) as usize;
    Some((u32::from(digit), b.subarrays[sub].as_ref()?[rank]))
}

/// Index of the first key strictly greater than `after` in a linear leaf's
/// key area, or `None` when no key can exceed it — `after` already at the
/// level's largest representable remainder, or every key `<= after`.
#[inline]
fn leaf_index_after(keys: &[u8], pop: usize, kb: u8, after: u32) -> Option<usize> {
    if after >= rem_mask(kb) {
        return None;
    }
    let i = leaf_lower_bound(keys, pop, kb, after + 1)?;
    (i < pop).then_some(i)
}

/// Smallest `(key, value)` under `e` with key strictly greater than `after`,
/// in a single descent — [`next`] plus the value it sits next to, so a
/// forward walk stops paying a second full `get` descent per step (#614).
pub(crate) fn next_entry(a: &Arena, e: &Edge32, kb: u8, after: u32) -> Option<(u32, u32)> {
    match kind(e) {
        Kind::Null => None,
        Kind::MapImmed { kb: _ } => {
            let r = map_immed_rem(e, kb);
            (r > after).then(|| (r, map_immed_val(e)))
        }
        Kind::MapLeaf(_) => {
            let pop = edge_pop(e);
            let cap = cap_class(pop);
            let buf = a.leaf(edge_handle(e));
            let keys = &buf[4 * cap..];
            let i = leaf_index_after(keys, pop, kb, after)?;
            Some((read_rem(keys, i, kb as usize), map_leaf_value(buf, i)))
        }
        Kind::MapBitmap => {
            if after >= 255 {
                return None;
            }
            let b = a.map_bitmap(edge_handle(e));
            let digit = bitmap_first_ge_raw(&b.header.bitmap, after as u16 + 1)?;
            map_bitmap_entry(b, digit)
        }
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchB | Kind::BranchU => {
            let da = digit_at(after, kb);
            let ca = child_rem(after, kb);
            let mut result = None;
            branch_for_each_child(a, e, |d, c| {
                if d < da {
                    return true;
                }
                let hit = if d == da {
                    next_entry(a, &c, kb - 1, ca)
                } else {
                    first_entry(a, &c, kb - 1)
                };
                if let Some((cr, v)) = hit {
                    result = Some((combine(d, cr, kb), v));
                    return false;
                }
                true
            });
            result
        }
        Kind::SetImmed { .. } | Kind::SetLeaf(_) | Kind::Bitmap => {
            unreachable!("map op on a set edge")
        }
    }
}

/// Largest `(key, value)` under `e` in a single descent — [`last`] plus its
/// value, the descending twin of [`first_entry`] (#614).
pub(crate) fn last_entry(a: &Arena, e: &Edge32, kb: u8) -> Option<(u32, u32)> {
    match kind(e) {
        Kind::Null => None,
        Kind::MapImmed { kb: _ } => Some((map_immed_rem(e, kb), map_immed_val(e))),
        Kind::MapLeaf(_) => {
            let pop = edge_pop(e);
            if pop == 0 {
                return None;
            }
            let cap = cap_class(pop);
            let buf = a.leaf(edge_handle(e));
            Some((
                read_rem(&buf[4 * cap..], pop - 1, kb as usize),
                map_leaf_value(buf, pop - 1),
            ))
        }
        Kind::MapBitmap => {
            let b = a.map_bitmap(edge_handle(e));
            let digit = bitmap_last_le_raw(&b.header.bitmap, 255)?;
            map_bitmap_entry(b, digit)
        }
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchB | Kind::BranchU => {
            let (d, c) = branch_last_child(a, e)?;
            last_entry(a, &c, kb - 1).map(|(cr, v)| (combine(d, cr, kb), v))
        }
        Kind::SetImmed { .. } | Kind::SetLeaf(_) | Kind::Bitmap => {
            unreachable!("map op on a set edge")
        }
    }
}

/// Largest `(key, value)` under `e` with key strictly less than `before`,
/// in a single descent — the descending twin of [`next_entry`] (#614).
pub(crate) fn prev_entry(a: &Arena, e: &Edge32, kb: u8, before: u32) -> Option<(u32, u32)> {
    match kind(e) {
        Kind::Null => None,
        Kind::MapImmed { kb: _ } => {
            let r = map_immed_rem(e, kb);
            (r < before).then(|| (r, map_immed_val(e)))
        }
        Kind::MapLeaf(_) => {
            let pop = edge_pop(e);
            let cap = cap_class(pop);
            let buf = a.leaf(edge_handle(e));
            let keys = &buf[4 * cap..];
            let i = leaf_lower_bound(keys, pop, kb, before)?;
            if i == 0 {
                return None;
            }
            Some((
                read_rem(keys, i - 1, kb as usize),
                map_leaf_value(buf, i - 1),
            ))
        }
        Kind::MapBitmap => {
            if before == 0 {
                return None;
            }
            let b = a.map_bitmap(edge_handle(e));
            let digit = bitmap_last_le_raw(&b.header.bitmap, before as i32 - 1)?;
            map_bitmap_entry(b, digit)
        }
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchB | Kind::BranchU => {
            let db = digit_at(before, kb);
            let cbr = child_rem(before, kb);
            let mut result = None;
            branch_for_each_child_rev(a, e, |d, c| {
                if d > db {
                    return true;
                }
                let hit = if d == db {
                    prev_entry(a, &c, kb - 1, cbr)
                } else {
                    last_entry(a, &c, kb - 1)
                };
                if let Some((cr, v)) = hit {
                    result = Some((combine(d, cr, kb), v));
                    return false;
                }
                true
            });
            result
        }
        Kind::SetImmed { .. } | Kind::SetLeaf(_) | Kind::Bitmap => {
            unreachable!("map op on a set edge")
        }
    }
}

/// Remove every map entry whose remainder lies in `lo..=hi` under `e`,
/// calling `f(key, value)` for each in ascending key order; returns the
/// count. `prefix` carries the already-decoded high bytes so `f` sees
/// full keys. One descent to the range plus one structural fix-up per
/// touched node — the batched alternative to a `first`/`remove` descent
/// pair per entry that the TTL-eviction loops paid (#578).
pub(crate) fn map_remove_range<F: FnMut(u32, u32)>(
    a: &mut Arena,
    e: &mut Edge32,
    kb: u8,
    prefix: u32,
    lo: u32,
    hi: u32,
    f: &mut F,
) -> usize {
    debug_assert!(lo <= hi);
    match kind(e) {
        Kind::Null => 0,
        Kind::MapImmed { kb: _ } => {
            let r = map_immed_rem(e, kb);
            if r < lo || r > hi {
                return 0;
            }
            f(prefix | r, map_immed_val(e));
            *e = Edge32::null();
            1
        }
        Kind::MapLeaf(_) => {
            let pop = edge_pop(e);
            let keys_off = 4 * cap_class(pop);
            let kbz = kb as usize;
            let buf = a.leaf(edge_handle(e));
            let keys = &buf[keys_off..];
            let i0 = leaf_lower_bound(keys, pop, kb, lo).unwrap();
            let i1 = if hi == rem_mask(kb) {
                pop
            } else {
                leaf_lower_bound(keys, pop, kb, hi + 1).unwrap()
            };
            if i1 <= i0 {
                return 0;
            }
            for i in i0..i1 {
                let mut vb = [0u8; 4];
                vb.copy_from_slice(&buf[i * 4..i * 4 + 4]);
                f(prefix | read_rem(keys, i, kbz), u32::from_le_bytes(vb));
            }
            let removed = i1 - i0;
            let new_pop = pop - removed;
            let h = edge_handle(e);
            if new_pop == 0 {
                *e = Edge32::null();
                a.free(h);
            } else if new_pop == 1 && kb <= 3 {
                // The hole is contiguous, so the survivor is at one end.
                let i = if i0 == 0 { pop - 1 } else { 0 };
                let k = read_rem(keys, i, kbz);
                let mut sb = [0u8; 4];
                sb.copy_from_slice(&buf[i * 4..i * 4 + 4]);
                *e = map_immed_edge(kb, k, u32::from_le_bytes(sb));
                a.free(h);
            } else {
                map_leaf_remove_span(a, e, kb, i0, i1);
            }
            removed
        }
        Kind::MapBitmap => {
            let (removed, count_after, retired, bytes_delta) = {
                let b = a.map_bitmap_mut(edge_handle(e));
                let mut removed = 0usize;
                let mut bytes_delta = 0isize;
                let mut retired: [Option<Box<[u32]>>; 8] = core::array::from_fn(|_| None);
                for (sub, slot) in retired.iter_mut().enumerate() {
                    let sub_lo = (sub * 32) as u32;
                    let sub_hi = sub_lo + 31;
                    if sub_hi < lo || sub_lo > hi {
                        continue;
                    }
                    let w = sub / 2;
                    let base = (sub & 1) as u32 * 32;
                    let word = b.header.bitmap[w];
                    let from = lo.max(sub_lo) - sub_lo;
                    let to = hi.min(sub_hi) - sub_lo;
                    let span =
                        ((1u64 << (to - from + 1)) - 1) << (u64::from(from) + u64::from(base));
                    let hit = word & span;
                    if hit == 0 {
                        continue;
                    }
                    let Some(existing) = b.subarrays[sub].take() else {
                        continue;
                    };
                    let sub_pop = ((word >> base) & 0xFFFF_FFFF).count_ones() as usize;
                    let mut keep: Vec<u32> = Vec::with_capacity(sub_pop);
                    let mut bits = (word >> base) & 0xFFFF_FFFF;
                    let mut rank = 0usize;
                    while bits != 0 {
                        let i = bits.trailing_zeros();
                        let v = existing[rank];
                        if (hit >> (u64::from(base) + u64::from(i))) & 1 == 1 {
                            f(prefix | (sub_lo + i), v);
                            removed += 1;
                        } else {
                            keep.push(v);
                        }
                        rank += 1;
                        bits &= bits - 1;
                    }
                    b.header.bitmap[w] &= !hit;
                    bytes_delta += subarray_bytes_delta::<u32>(sub_pop, keep.len());
                    b.subarrays[sub] = if keep.is_empty() {
                        None
                    } else {
                        // Class-sized like every other subarray (#615).
                        keep.resize(cap_class(keep.len()), 0u32);
                        Some(keep.into_boxed_slice())
                    };
                    *slot = Some(existing);
                }
                // `pop0` is `count - 1` (see `LeafBitmapL32Data`); keep the
                // count itself in hand so a full clear cannot underflow it.
                let count_after = b.header.pop0 as usize + 1 - removed;
                if count_after > 0 {
                    b.header.pop0 = (count_after - 1) as u16;
                }
                (removed, count_after, retired, bytes_delta)
            };
            for r in retired {
                a.retire_vals(r);
            }
            a.bytes = a.bytes.wrapping_add_signed(bytes_delta);
            if removed == 0 {
                return 0;
            }
            if count_after == 0 {
                let old = edge_handle(e);
                a.free(old);
                *e = Edge32::null();
            } else if count_after <= MAP_BITMAP_LEAVE {
                let entries = read_map_bitmap(a, e);
                let old = edge_handle(e);
                *e = if entries.len() == 1 {
                    map_immed_edge(1, entries[0].0, entries[0].1)
                } else {
                    make_map_leaf(a, 1, &entries)
                };
                a.free(old);
            }
            removed
        }
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchB | Kind::BranchU => {
            let dlo = digit_at(lo, kb);
            let dhi = digit_at(hi, kb);
            let full = rem_mask(kb - 1);
            let mut total = 0usize;
            for d in dlo..=dhi {
                let Some(mut child) = branch_child(a, e, d) else {
                    continue;
                };
                let clo = if d == dlo { child_rem(lo, kb) } else { 0 };
                let chi = if d == dhi { child_rem(hi, kb) } else { full };
                let cprefix = prefix | (u32::from(d) << ((kb - 1) as u32 * 8));
                let before = child;
                let n = map_remove_range(a, &mut child, kb - 1, cprefix, clo, chi, f);
                if n == 0 {
                    continue;
                }
                total += n;
                if child.is_null() {
                    branch_remove_digit(a, e, d, -(n as i32));
                    if e.is_null() {
                        break;
                    }
                } else {
                    branch_commit(
                        a,
                        e,
                        d,
                        if child != before {
                            child
                        } else {
                            Edge32::null()
                        },
                        -(n as i32),
                    );
                }
            }
            total
        }
        Kind::SetImmed { .. } | Kind::SetLeaf(_) | Kind::Bitmap => {
            unreachable!("map op on a set edge")
        }
    }
}

/// Set-flavour sibling of [`map_remove_range`]: `f(key)` per removed key.
pub(crate) fn set_remove_range<F: FnMut(u32)>(
    a: &mut Arena,
    e: &mut Edge32,
    kb: u8,
    prefix: u32,
    lo: u32,
    hi: u32,
    f: &mut F,
) -> usize {
    debug_assert!(lo <= hi);
    match kind(e) {
        Kind::Null => 0,
        Kind::SetImmed { kb: _, count } => {
            let (keys, n) = set_immed_keys(e, kb, count);
            let mut v = [0u32; 7];
            let mut m = 0usize;
            let mut removed = 0usize;
            for &k in &keys[..n] {
                if k >= lo && k <= hi {
                    f(prefix | k);
                    removed += 1;
                } else {
                    v[m] = k;
                    m += 1;
                }
            }
            if removed == 0 {
                return 0;
            }
            *e = if m == 0 {
                Edge32::null()
            } else {
                set_immed_edge(kb, &v[..m])
            };
            removed
        }
        Kind::SetLeaf(_) => {
            let pop = edge_pop(e);
            let kbz = kb as usize;
            let buf = a.leaf(edge_handle(e));
            let i0 = leaf_lower_bound(buf, pop, kb, lo).unwrap();
            let i1 = if hi == rem_mask(kb) {
                pop
            } else {
                leaf_lower_bound(buf, pop, kb, hi + 1).unwrap()
            };
            if i1 <= i0 {
                return 0;
            }
            for i in i0..i1 {
                f(prefix | read_rem(buf, i, kbz));
            }
            let removed = i1 - i0;
            let new_pop = pop - removed;
            let h = edge_handle(e);
            if new_pop == 0 {
                *e = Edge32::null();
                a.free(h);
            } else if new_pop <= set_immed_cap(kb) {
                let mut v = [0u32; 7];
                let mut m = 0usize;
                for i in (0..i0).chain(i1..pop) {
                    v[m] = read_rem(buf, i, kbz);
                    m += 1;
                }
                *e = set_immed_edge(kb, &v[..m]);
                a.free(h);
            } else {
                set_leaf_remove_span(a, e, kb, i0, i1);
            }
            removed
        }
        Kind::Bitmap => {
            // Collect the hits first: the walk borrows the leaf immutably
            // and the clears need it mutably.
            let mut digits = [0u8; 256];
            let mut n = 0usize;
            {
                let leaf = a.bitmap(edge_handle(e));
                let mut from = lo as u16;
                while let Some(d) = bitmap_first_ge(leaf, from) {
                    if u32::from(d) > hi {
                        break;
                    }
                    digits[n] = d;
                    n += 1;
                    from = u16::from(d) + 1;
                }
            }
            if n == 0 {
                return 0;
            }
            let leaf = a.bitmap_mut(edge_handle(e));
            for &d in &digits[..n] {
                leaf.unset(d);
                f(prefix | u32::from(d));
            }
            let pop = leaf.pop0 as usize;
            if pop == 0 {
                let old = edge_handle(e);
                a.free(old);
                *e = Edge32::null();
            } else if pop <= SET_BITMAP_LEAVE {
                let keys = bitmap_keys(a, e);
                let old = edge_handle(e);
                *e = if keys.len() <= set_immed_cap(1) {
                    set_immed_edge(1, &keys)
                } else {
                    make_set_leaf(a, 1, &keys)
                };
                a.free(old);
            }
            n
        }
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchB | Kind::BranchU => {
            let dlo = digit_at(lo, kb);
            let dhi = digit_at(hi, kb);
            let full = rem_mask(kb - 1);
            let mut total = 0usize;
            for d in dlo..=dhi {
                let Some(mut child) = branch_child(a, e, d) else {
                    continue;
                };
                let clo = if d == dlo { child_rem(lo, kb) } else { 0 };
                let chi = if d == dhi { child_rem(hi, kb) } else { full };
                let cprefix = prefix | (u32::from(d) << ((kb - 1) as u32 * 8));
                let before = child;
                let n = set_remove_range(a, &mut child, kb - 1, cprefix, clo, chi, f);
                if n == 0 {
                    continue;
                }
                total += n;
                if child.is_null() {
                    branch_remove_digit(a, e, d, -(n as i32));
                    if e.is_null() {
                        break;
                    }
                } else {
                    branch_commit(
                        a,
                        e,
                        d,
                        if child != before {
                            child
                        } else {
                            Edge32::null()
                        },
                        -(n as i32),
                    );
                }
            }
            total
        }
        Kind::MapLeaf(_) | Kind::MapImmed { .. } | Kind::MapBitmap => {
            unreachable!("set op on a map edge")
        }
    }
}

pub(crate) fn last(a: &Arena, e: &Edge32, kb: u8) -> Option<u32> {
    match kind(e) {
        Kind::Null => None,
        Kind::SetImmed { kb: _, count } => {
            let (keys, n) = set_immed_keys(e, kb, count);
            keys[..n].iter().copied().max()
        }
        Kind::MapImmed { kb: _ } => Some(map_immed_rem(e, kb)),
        Kind::SetLeaf(_) => {
            let pop = edge_pop(e);
            if pop == 0 {
                return None;
            }
            let buf = a.leaf(edge_handle(e));
            Some(read_rem(buf, pop - 1, kb as usize))
        }
        Kind::MapLeaf(_) => {
            let pop = edge_pop(e);
            if pop == 0 {
                return None;
            }
            let cap = cap_class(pop);
            let buf = a.leaf(edge_handle(e));
            Some(read_rem(&buf[4 * cap..], pop - 1, kb as usize))
        }
        Kind::Bitmap => bitmap_last_le(a.bitmap(edge_handle(e)), 255).map(u32::from),
        Kind::MapBitmap => {
            bitmap_last_le_raw(&a.map_bitmap(edge_handle(e)).header.bitmap, 255).map(u32::from)
        }
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchB | Kind::BranchU => {
            let (d, c) = branch_last_child(a, e)?;
            last(a, &c, kb - 1).map(|cr| combine(d, cr, kb))
        }
    }
}

/// Smallest remainder strictly greater than `after`.
pub(crate) fn next(a: &Arena, e: &Edge32, kb: u8, after: u32) -> Option<u32> {
    match kind(e) {
        Kind::Null => None,
        Kind::SetImmed { kb: _, count } => {
            let (keys, n) = set_immed_keys(e, kb, count);
            keys[..n].iter().copied().filter(|&k| k > after).min()
        }
        Kind::MapImmed { kb: _ } => {
            let r = map_immed_rem(e, kb);
            (r > after).then_some(r)
        }
        Kind::SetLeaf(_) => {
            let pop = edge_pop(e);
            let buf = a.leaf(edge_handle(e));
            let i = leaf_index_after(buf, pop, kb, after)?;
            Some(read_rem(buf, i, kb as usize))
        }
        Kind::MapLeaf(_) => {
            let pop = edge_pop(e);
            let cap = cap_class(pop);
            let buf = a.leaf(edge_handle(e));
            let keys = &buf[4 * cap..];
            let i = leaf_index_after(keys, pop, kb, after)?;
            Some(read_rem(keys, i, kb as usize))
        }
        Kind::Bitmap => {
            if after >= 255 {
                None
            } else {
                bitmap_first_ge(a.bitmap(edge_handle(e)), after as u16 + 1).map(u32::from)
            }
        }
        Kind::MapBitmap => {
            if after >= 255 {
                None
            } else {
                bitmap_first_ge_raw(
                    &a.map_bitmap(edge_handle(e)).header.bitmap,
                    after as u16 + 1,
                )
                .map(u32::from)
            }
        }
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchB | Kind::BranchU => {
            let da = digit_at(after, kb);
            let ca = child_rem(after, kb);
            let mut result = None;
            branch_for_each_child(a, e, |d, c| {
                if d < da {
                    return true;
                }
                if d == da {
                    if let Some(cr) = next(a, &c, kb - 1, ca) {
                        result = Some(combine(d, cr, kb));
                        return false;
                    }
                } else {
                    if let Some(cr) = first(a, &c, kb - 1) {
                        result = Some(combine(d, cr, kb));
                        return false;
                    }
                }
                true
            });
            result
        }
    }
}

/// Largest remainder strictly less than `before`.
pub(crate) fn prev(a: &Arena, e: &Edge32, kb: u8, before: u32) -> Option<u32> {
    match kind(e) {
        Kind::Null => None,
        Kind::SetImmed { kb: _, count } => {
            let (keys, n) = set_immed_keys(e, kb, count);
            keys[..n].iter().copied().filter(|&k| k < before).max()
        }
        Kind::MapImmed { kb: _ } => {
            let r = map_immed_rem(e, kb);
            (r < before).then_some(r)
        }
        Kind::SetLeaf(_) => {
            let pop = edge_pop(e);
            let buf = a.leaf(edge_handle(e));
            let i = leaf_lower_bound(buf, pop, kb, before)?;
            (i > 0).then(|| read_rem(buf, i - 1, kb as usize))
        }
        Kind::MapLeaf(_) => {
            let pop = edge_pop(e);
            let cap = cap_class(pop);
            let buf = a.leaf(edge_handle(e));
            let keys = &buf[4 * cap..];
            let i = leaf_lower_bound(keys, pop, kb, before)?;
            (i > 0).then(|| read_rem(keys, i - 1, kb as usize))
        }
        Kind::Bitmap => {
            if before == 0 {
                None
            } else {
                bitmap_last_le(a.bitmap(edge_handle(e)), before as i32 - 1).map(u32::from)
            }
        }
        Kind::MapBitmap => {
            if before == 0 {
                None
            } else {
                bitmap_last_le_raw(
                    &a.map_bitmap(edge_handle(e)).header.bitmap,
                    before as i32 - 1,
                )
                .map(u32::from)
            }
        }
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchB | Kind::BranchU => {
            let db = digit_at(before, kb);
            let cb = child_rem(before, kb);
            let mut result = None;
            branch_for_each_child_rev(a, e, |d, c| {
                if d > db {
                    return true;
                }
                if d == db {
                    if let Some(cr) = prev(a, &c, kb - 1, cb) {
                        result = Some(combine(d, cr, kb));
                        return false;
                    }
                } else {
                    if let Some(cr) = last(a, &c, kb - 1) {
                        result = Some(combine(d, cr, kb));
                        return false;
                    }
                }
                true
            });
            result
        }
    }
}

/// Count of remainders in the inclusive range `[lo, hi]` within the subtree.
pub(crate) fn count_range(a: &Arena, e: &Edge32, kb: u8, lo: u32, hi: u32) -> usize {
    if lo > hi {
        return 0;
    }
    match kind(e) {
        Kind::Null => 0,
        Kind::SetImmed { kb: _, count } => {
            let (keys, n) = set_immed_keys(e, kb, count);
            keys[..n].iter().filter(|&&k| k >= lo && k <= hi).count()
        }
        Kind::MapImmed { kb: _ } => {
            let r = map_immed_rem(e, kb);
            usize::from(r >= lo && r <= hi)
        }
        Kind::SetLeaf(_) => {
            let pop = edge_pop(e);
            let buf = a.leaf(edge_handle(e));
            (0..pop)
                .map(|i| read_rem(buf, i, kb as usize))
                .filter(|&k| k >= lo && k <= hi)
                .count()
        }
        Kind::MapLeaf(_) => {
            let pop = edge_pop(e);
            let cap = cap_class(pop);
            let buf = a.leaf(edge_handle(e));
            (0..pop)
                .map(|i| read_rem(&buf[4 * cap..], i, kb as usize))
                .filter(|&k| k >= lo && k <= hi)
                .count()
        }
        Kind::Bitmap => bitmap_count_range(a.bitmap(edge_handle(e)), lo, hi),
        Kind::MapBitmap => {
            bitmap_count_range_raw(&a.map_bitmap(edge_handle(e)).header.bitmap, lo, hi)
        }
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchB | Kind::BranchU => {
            let d_lo = digit_at(lo, kb);
            let d_hi = digit_at(hi, kb);
            let r_lo = child_rem(lo, kb);
            let r_hi = child_rem(hi, kb);
            let full = rem_mask(kb - 1);
            let mut total = 0;
            branch_for_each_child(a, e, |d, c| {
                if d < d_lo {
                    return true;
                }
                if d > d_hi {
                    return false;
                }
                total += if d == d_lo && d == d_hi {
                    count_range(a, &c, kb - 1, r_lo, r_hi)
                } else if d == d_lo {
                    count_range(a, &c, kb - 1, r_lo, full)
                } else if d == d_hi {
                    count_range(a, &c, kb - 1, 0, r_hi)
                } else {
                    subtree_count(a, &c)
                };
                true
            });
            total
        }
    }
}

/// Walk every map entry whose remainder lies in `lo..=hi` under `e` in
/// ascending key order, calling `f(key, value)` with the already-decoded
/// high bytes folded in. `f` returns `false` to stop the walk; this returns
/// `false` when it did.
///
/// One descent to the range, then contiguous streaming through the leaves it
/// spans — the read-only twin of [`map_remove_range`], and what a C consumer
/// walking a range needs instead of a root re-descent per key (#614).
pub(crate) fn map_for_each_range(
    a: &Arena,
    e: &Edge32,
    kb: u8,
    lo: u32,
    hi: u32,
    f: &mut dyn FnMut(u32, u32) -> bool,
) -> bool {
    if lo > hi {
        return true;
    }
    match kind(e) {
        Kind::Null => true,
        Kind::MapImmed { kb: _ } => {
            let r = map_immed_rem(e, kb);
            if r >= lo && r <= hi {
                return f(r, map_immed_val(e));
            }
            true
        }
        Kind::MapLeaf(_) => {
            let pop = edge_pop(e);
            let keys_off = 4 * cap_class(pop);
            let buf = a.leaf(edge_handle(e));
            let keys = &buf[keys_off..];
            // Binary-search both ends: the span, not the whole leaf.
            let i0 = leaf_lower_bound(keys, pop, kb, lo).unwrap();
            let i1 = if hi >= rem_mask(kb) {
                pop
            } else {
                leaf_lower_bound(keys, pop, kb, hi + 1).unwrap()
            };
            for i in i0..i1 {
                if !f(read_rem(keys, i, kb as usize), map_leaf_value(buf, i)) {
                    return false;
                }
            }
            true
        }
        Kind::MapBitmap => {
            if lo > 255 {
                return true;
            }
            let b = a.map_bitmap(edge_handle(e));
            let lo_u8 = lo as u8;
            let hi_u8 = hi.min(255) as u8;
            let (w_lo, w_hi) = ((lo_u8 >> 6) as usize, (hi_u8 >> 6) as usize);
            for w in w_lo..=w_hi {
                // Mask the partial words at each end so the loop visits only
                // digits inside [lo, hi].
                let mut word = b.header.bitmap[w];
                if w == w_lo {
                    word &= !0u64 << (lo_u8 & 63);
                }
                if w == w_hi && (hi_u8 & 63) != 63 {
                    word &= (1u64 << ((hi_u8 & 63) + 1)) - 1;
                }
                while word != 0 {
                    let digit = (w * 64 + word.trailing_zeros() as usize) as u8;
                    let Some((cr, v)) = map_bitmap_entry(b, digit) else {
                        return true;
                    };
                    if !f(cr, v) {
                        return false;
                    }
                    word &= word - 1;
                }
            }
            true
        }
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchB | Kind::BranchU => {
            let d_lo = digit_at(lo, kb);
            let d_hi = digit_at(hi, kb);
            let r_lo = child_rem(lo, kb);
            let r_hi = child_rem(hi, kb);
            let full = rem_mask(kb - 1);
            let mut go = true;
            branch_for_each_child(a, e, |d, c| {
                if d < d_lo {
                    return true;
                }
                if d > d_hi {
                    return false;
                }
                let (clo, chi) = if d == d_lo && d == d_hi {
                    (r_lo, r_hi)
                } else if d == d_lo {
                    (r_lo, full)
                } else if d == d_hi {
                    (0, r_hi)
                } else {
                    (0, full)
                };
                let mut g = |cr: u32, v: u32| f(combine(d, cr, kb), v);
                go = map_for_each_range(a, &c, kb - 1, clo, chi, &mut g);
                go
            });
            go
        }
        _ => true,
    }
}

// ---------------------------------------------------------------------------
// Stack-based in-order iteration (#614)
// ---------------------------------------------------------------------------

/// The child of `e` with the smallest digit `>= from`, if any — the
/// generalisation of [`branch_first_child`] the stack iterator advances on.
fn branch_child_at_or_after(a: &Arena, e: &Edge32, from: u8) -> Option<(u8, Edge32)> {
    match kind(e) {
        Kind::BranchL2 => {
            let b = a.l2(edge_handle(e));
            let n = b.header.num_edges as usize;
            (0..n)
                .find(|&i| b.digits[i] >= from)
                .map(|i| (b.digits[i], b.edges[i]))
        }
        Kind::BranchL6 => {
            let b = a.l6(edge_handle(e));
            let n = b.header.num_edges as usize;
            (0..n)
                .find(|&i| b.digits[i] >= from)
                .map(|i| (b.digits[i], b.edges[i]))
        }
        Kind::BranchB => {
            let b = a.b(edge_handle(e));
            let digit = bitmap_first_ge_raw(&b.header.bitmap, u16::from(from))?;
            let w = (digit >> 6) as usize;
            let rank = bitmap_sub_rank(b.header.bitmap[w], digit);
            let sub = (digit >> 5) as usize;
            Some((digit, b.subarrays[sub].as_ref()?[rank]))
        }
        Kind::BranchU => {
            let b = a.u(edge_handle(e));
            (from as usize..256).find_map(|d| {
                let c = b.edges[d];
                (!c.is_null()).then_some((d as u8, c))
            })
        }
        _ => unreachable!("branch op on a leaf edge"),
    }
}

/// One branch level on an iterator's descent path: the branch itself, the
/// key bytes and decoded prefix at that level, and the next digit to try
/// when the subtree below it runs out (`256` once exhausted).
#[derive(Clone, Copy)]
struct Frame32 {
    edge: Edge32,
    kb: u8,
    prefix: u32,
    next_digit: u16,
}

/// The leaf an iterator is streaming, and how far through it is. Every
/// variant yields ascending keys and is positioned past everything below
/// the iterator's start bound when it is built.
#[derive(Clone, Copy)]
enum LeafCur32 {
    Done,
    /// Immediate keys live in the edge itself: at most seven of them, copied
    /// out and sorted once so the walk never re-reads the edge.
    Immed {
        keys: [u32; 7],
        n: u8,
        idx: u8,
        val: u32,
        prefix: u32,
    },
    /// A linear leaf, streamed by index through the arena buffer.
    Linear {
        handle: u32,
        pop: u16,
        idx: u16,
        kb: u8,
        prefix: u32,
        is_map: bool,
    },
    /// A set bitmap leaf: the four mask words, low bits already cleared to
    /// the start bound.
    Bitmap {
        words: [u64; 4],
        w: u8,
        prefix: u32,
    },
    /// A map bitmap leaf: the same walk, with the value read through the
    /// leaf's rank-indexed subarrays.
    MapBitmap {
        handle: u32,
        words: [u64; 4],
        w: u8,
        prefix: u32,
    },
}

impl LeafCur32 {
    /// Positions a cursor on the first entry of `e` whose remainder is
    /// `>= from`, or `None` when the leaf holds nothing at or after it.
    fn new(a: &Arena, e: &Edge32, kb: u8, prefix: u32, from: u32) -> Option<Self> {
        match kind(e) {
            Kind::Null => None,
            Kind::SetImmed { kb: _, count } => {
                let (mut keys, n) = set_immed_keys(e, kb, count);
                keys[..n].sort_unstable();
                let idx = keys[..n].partition_point(|&k| k < from);
                (idx < n).then_some(LeafCur32::Immed {
                    keys,
                    n: n as u8,
                    idx: idx as u8,
                    val: 0,
                    prefix,
                })
            }
            Kind::MapImmed { kb: _ } => {
                let r = map_immed_rem(e, kb);
                (r >= from).then(|| LeafCur32::Immed {
                    keys: [r, 0, 0, 0, 0, 0, 0],
                    n: 1,
                    idx: 0,
                    val: map_immed_val(e),
                    prefix,
                })
            }
            Kind::SetLeaf(_) | Kind::MapLeaf(_) => {
                let is_map = matches!(kind(e), Kind::MapLeaf(_));
                let pop = edge_pop(e);
                let handle = edge_handle(e);
                let buf = a.leaf(handle);
                let keys = if is_map {
                    &buf[4 * cap_class(pop)..]
                } else {
                    buf
                };
                let idx = leaf_lower_bound(keys, pop, kb, from)?;
                (idx < pop).then_some(LeafCur32::Linear {
                    handle,
                    pop: pop as u16,
                    idx: idx as u16,
                    kb,
                    prefix,
                    is_map,
                })
            }
            Kind::Bitmap | Kind::MapBitmap => {
                if from > 255 {
                    return None;
                }
                let is_map = matches!(kind(e), Kind::MapBitmap);
                let handle = edge_handle(e);
                let mut words = if is_map {
                    a.map_bitmap(handle).header.bitmap
                } else {
                    a.bitmap(handle).bitmap
                };
                // Clear everything below the start bound so the walk can run
                // on trailing_zeros alone from here on.
                let from = from as usize;
                for (w, word) in words.iter_mut().enumerate() {
                    if w < from / 64 {
                        *word = 0;
                    } else if w == from / 64 {
                        *word &= !0u64 << (from % 64);
                    }
                }
                if words.iter().all(|&w| w == 0) {
                    return None;
                }
                Some(if is_map {
                    LeafCur32::MapBitmap {
                        handle,
                        words,
                        w: 0,
                        prefix,
                    }
                } else {
                    LeafCur32::Bitmap {
                        words,
                        w: 0,
                        prefix,
                    }
                })
            }
            _ => None,
        }
    }

    /// The next `(key, value)` in this leaf; `None` once it is spent. Set
    /// leaves report a zero value.
    fn next(&mut self, a: &Arena) -> Option<(u32, u32)> {
        match self {
            LeafCur32::Done => None,
            LeafCur32::Immed {
                keys,
                n,
                idx,
                val,
                prefix,
            } => {
                if *idx >= *n {
                    return None;
                }
                let k = keys[*idx as usize];
                *idx += 1;
                Some((*prefix | k, *val))
            }
            LeafCur32::Linear {
                handle,
                pop,
                idx,
                kb,
                prefix,
                is_map,
            } => {
                if *idx >= *pop {
                    return None;
                }
                let i = *idx as usize;
                *idx += 1;
                let buf = a.leaf(*handle);
                if *is_map {
                    let keys = &buf[4 * cap_class(*pop as usize)..];
                    Some((
                        *prefix | read_rem(keys, i, *kb as usize),
                        map_leaf_value(buf, i),
                    ))
                } else {
                    Some((*prefix | read_rem(buf, i, *kb as usize), 0))
                }
            }
            LeafCur32::Bitmap { words, w, prefix } => loop {
                if *w >= 4 {
                    return None;
                }
                let word = &mut words[*w as usize];
                if *word == 0 {
                    *w += 1;
                    continue;
                }
                let bit = word.trailing_zeros() as usize;
                *word &= *word - 1;
                return Some((*prefix | (*w as u32 * 64 + bit as u32), 0));
            },
            LeafCur32::MapBitmap {
                handle,
                words,
                w,
                prefix,
            } => loop {
                if *w >= 4 {
                    return None;
                }
                let word = &mut words[*w as usize];
                if *word == 0 {
                    *w += 1;
                    continue;
                }
                let bit = word.trailing_zeros() as usize;
                *word &= *word - 1;
                let digit = (*w as usize * 64 + bit) as u8;
                let b = a.map_bitmap(*handle);
                let (cr, v) = map_bitmap_entry(b, digit)?;
                return Some((*prefix | cr, v));
            },
        }
    }
}

/// A stack-based in-order iterator over a 32-bit trie subtree.
///
/// Holds the descent path — at most three branch frames, since a 32-bit key
/// is four digits — so each step is a leaf index bump, or a pop and one
/// child lookup at a branch that is already in hand. The `first`/`next`
/// primitives it replaces re-descend from the root for every key (#614).
///
/// Frames hold `Edge32` by value and leaves by arena handle, so the walk
/// borrows the arena immutably and needs no raw pointers.
pub(crate) struct RawIter32<'a> {
    a: &'a Arena,
    stack: [Frame32; 4],
    depth: u8,
    leaf: LeafCur32,
}

impl<'a> RawIter32<'a> {
    /// An iterator over `root` positioned at the first key `>= from`.
    pub(crate) fn new(a: &'a Arena, root: Edge32, kb: u8, from: u32) -> Self {
        let mut it = RawIter32 {
            a,
            stack: [Frame32 {
                edge: Edge32::null(),
                kb: 0,
                prefix: 0,
                next_digit: 256,
            }; 4],
            depth: 0,
            leaf: LeafCur32::Done,
        };
        if !it.descend(root, kb, 0, from) {
            it.unwind();
        }
        it
    }

    /// Walks down from `e`, pushing a frame per branch, until a leaf holding
    /// something `>= from` is positioned. Returns `false` when this subtree
    /// has nothing at or after `from`; frames pushed on the way stay on the
    /// stack for [`Self::unwind`] to resume from.
    fn descend(&mut self, mut e: Edge32, mut kb: u8, mut prefix: u32, mut from: u32) -> bool {
        loop {
            match kind(&e) {
                Kind::Null => return false,
                Kind::BranchL2 | Kind::BranchL6 | Kind::BranchB | Kind::BranchU => {
                    let d_from = digit_at(from, kb);
                    let Some((d, c)) = branch_child_at_or_after(self.a, &e, d_from) else {
                        return false;
                    };
                    self.stack[self.depth as usize] = Frame32 {
                        edge: e,
                        kb,
                        prefix,
                        next_digit: u16::from(d) + 1,
                    };
                    self.depth += 1;
                    let child_from = if d > d_from { 0 } else { child_rem(from, kb) };
                    prefix |= u32::from(d) << ((kb - 1) as u32 * 8);
                    e = c;
                    kb -= 1;
                    from = child_from;
                }
                _ => {
                    if let Some(cur) = LeafCur32::new(self.a, &e, kb, prefix, from) {
                        self.leaf = cur;
                        return true;
                    }
                    return false;
                }
            }
        }
    }

    /// Backtracks after a leaf or subtree is spent: takes the next digit at
    /// the deepest unfinished branch and descends into it, or empties the
    /// stack when every branch is finished.
    fn unwind(&mut self) {
        while self.depth > 0 {
            let f = self.stack[self.depth as usize - 1];
            if f.next_digit <= 255
                && let Some((d, c)) = branch_child_at_or_after(self.a, &f.edge, f.next_digit as u8)
            {
                self.stack[self.depth as usize - 1].next_digit = u16::from(d) + 1;
                let prefix = f.prefix | (u32::from(d) << ((f.kb - 1) as u32 * 8));
                let found = self.descend(c, f.kb - 1, prefix, 0);
                // A live child edge always covers at least one entry, so
                // an unbounded descent into one cannot come back empty.
                // The retry below is the defensive path if it ever does.
                debug_assert!(found, "empty subtree under a live child edge");
                if found {
                    return;
                }
                continue;
            }
            self.depth -= 1;
        }
        self.leaf = LeafCur32::Done;
    }

    /// The next `(key, value)` in ascending key order. Set walks report a
    /// zero value.
    pub(crate) fn next(&mut self) -> Option<(u32, u32)> {
        loop {
            if let Some(kv) = self.leaf.next(self.a) {
                return Some(kv);
            }
            if self.depth == 0 {
                return None;
            }
            self.unwind();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_class_matches_64bit_schedule() {
        assert_eq!(cap_class(0), 0);
        assert_eq!(cap_class(1), 1);
        assert_eq!(cap_class(2), 2);
        assert_eq!(cap_class(3), 4);
        assert_eq!(cap_class(4), 4);
        assert_eq!(cap_class(5), 8);
        assert_eq!(cap_class(25), 28);
    }

    #[test]
    fn cap_class_never_underallocates() {
        // The 32-bit analogue of `leaf::simd_gates_within_cap_class`: leaf
        // search is scalar and bounded by `pop`, and a leaf allocation must
        // always be at least as wide as the keys it holds. If this ever
        // failed, a scan reading `pop` keys of `kb` bytes could read past
        // the allocation.
        for pop in 0..=300usize {
            for kb in 1..=4u8 {
                let alloc = size_set32(kb, pop);
                assert!(
                    alloc >= pop * kb as usize,
                    "set leaf under-allocated: kb={kb} pop={pop} alloc={alloc}"
                );
                let map_alloc = size_map32(kb, pop);
                assert!(
                    map_alloc >= pop * (4 + kb as usize),
                    "map leaf under-allocated: kb={kb} pop={pop} alloc={map_alloc}"
                );
            }
        }
    }

    /// Every live bitmap subarray is allocated at exactly `cap_class` of
    /// its **population**, and that population is read off the bitmap (for
    /// a value leaf) or `pop_counts` (for a branch) — never off `len()`,
    /// which since #615 is the allocation length and over-reports by up to
    /// three slots.
    ///
    /// This walks the whole arena after a mix of inserts and removes, so it
    /// covers growth, in-place shifts, class crossings and shrinks.
    fn assert_subarray_invariants(a: &Arena) {
        for slot in a.slots.iter().flatten() {
            match slot {
                NodeBox::B(b) => {
                    for sub in 0..8usize {
                        let pop = b.header.pop_counts[sub] as usize;
                        // `pop_counts` is the source of truth and must agree
                        // with the bitmap half that covers this subexpanse.
                        let word = b.header.bitmap[sub / 2];
                        let base = ((sub & 1) * 32) as u32;
                        assert_eq!(
                            pop,
                            ((word >> base) & 0xFFFF_FFFF).count_ones() as usize,
                            "BranchB32 pop_counts[{sub}] disagrees with the bitmap"
                        );
                        match &b.subarrays[sub] {
                            Some(arr) => {
                                assert!(pop > 0, "empty subexpanse {sub} holds an allocation");
                                assert_eq!(
                                    arr.len(),
                                    cap_class(pop),
                                    "BranchB32 subarray {sub}: pop={pop}"
                                );
                                // Live entries are `[0, pop)`; nothing is
                                // indexed beyond, so the spare tail is free
                                // to hold filler — but it must be filler,
                                // not a stale duplicate child.
                                for e in &arr[pop..] {
                                    assert!(e.is_null(), "spare edge slot is not null");
                                }
                            }
                            None => assert_eq!(pop, 0, "populated subexpanse {sub} has no array"),
                        }
                    }
                }
                NodeBox::MapBitmap(b) => {
                    for sub in 0..8usize {
                        let word = b.header.bitmap[sub / 2];
                        let base = ((sub & 1) * 32) as u32;
                        let pop = ((word >> base) & 0xFFFF_FFFF).count_ones() as usize;
                        match &b.subarrays[sub] {
                            Some(arr) => {
                                assert!(pop > 0, "empty subexpanse {sub} holds an allocation");
                                assert_eq!(
                                    arr.len(),
                                    cap_class(pop),
                                    "LeafBitmapL_32 subarray {sub}: pop={pop}"
                                );
                            }
                            None => assert_eq!(pop, 0, "populated subexpanse {sub} has no array"),
                        }
                    }
                }
                _ => {}
            }
        }
    }

    #[test]
    fn bitmap_subarrays_are_cap_classed() {
        use crate::map32::ExpanseMap32;
        use crate::set32::ExpanseSet32;

        // Sequential: drives every kb=1 subexpanse through MAP_BITMAP and
        // through every capacity class up to 32.
        let mut map = ExpanseMap32::new();
        for i in 0..4_000u32 {
            map.insert(1_700_000_000 + i, i);
            if i % 512 == 0 {
                assert_subarray_invariants(map.arena());
            }
        }
        assert_subarray_invariants(map.arena());

        // Sparse: shallow, wide branches — the BranchB32 child-subarray path.
        let mut set = ExpanseSet32::new();
        for i in 0..3_000u32 {
            set.insert(i.wrapping_mul(2_654_435_761));
        }
        assert_subarray_invariants(set.arena());

        // Removals in a scattered order, back down through every class
        // boundary and out the other side.
        for i in (0..4_000u32).step_by(3) {
            map.remove(1_700_000_000 + i);
        }
        assert_subarray_invariants(map.arena());
        for i in (0..3_000u32).rev().step_by(2) {
            set.remove(i.wrapping_mul(2_654_435_761));
        }
        assert_subarray_invariants(set.arena());

        // Re-insert: growth back into arrays that were shrunk in place.
        for i in (0..4_000u32).step_by(3) {
            map.insert(1_700_000_000 + i, i);
        }
        assert_subarray_invariants(map.arena());
        assert_eq!(map.len(), 4_000);
    }

    /// The population a subexpanse reports must be the bitmap's, not the
    /// subarray's length — the two diverge exactly when a growth reused
    /// spare class capacity.
    #[test]
    fn subexpanse_pop_is_read_from_the_bitmap_not_the_length() {
        use crate::map32::ExpanseMap32;

        let mut map = ExpanseMap32::new();
        // 33 keys sharing a kb=1 expanse: past MAP_BITMAP_ENTER (a bitmap
        // leaf), and spanning both 32-digit subexpanses of the low word.
        for d in 0..70u32 {
            map.insert(0x0011_2200 | d, d);
        }
        let mut diverged = false;
        for slot in map.arena().slots.iter().flatten() {
            if let NodeBox::MapBitmap(b) = slot {
                for sub in 0..8usize {
                    let word = b.header.bitmap[sub / 2];
                    let base = ((sub & 1) * 32) as u32;
                    let pop = ((word >> base) & 0xFFFF_FFFF).count_ones() as usize;
                    if let Some(arr) = &b.subarrays[sub] {
                        assert_eq!(arr.len(), cap_class(pop));
                        diverged |= arr.len() != pop;
                    }
                }
            }
        }
        assert!(
            diverged,
            "no subarray carried spare capacity — the fixture no longer \
             exercises the len() != popcount case this test exists for"
        );
        // Every key still reads back, so nothing indexed into the spare tail.
        for d in 0..70u32 {
            assert_eq!(map.get(0x0011_2200 | d), Some(d), "digit {d}");
        }
        assert_eq!(map.len(), 70);
    }

    /// The ascending-append short circuit must agree with the binary search
    /// it skips, for every position and both the equal-to-last and
    /// past-the-end cases -- a wrong answer here silently corrupts key order.
    #[test]
    fn leaf_insert_pos_agrees_with_binary_search() {
        for kb in 1..=4u8 {
            for pop in 0..=24usize {
                // Keys 10, 20, 30, ... so every gap, hit and end is reachable.
                let mut buf = vec![0u8; (pop + 1) * kb as usize];
                for i in 0..pop {
                    write_rem(&mut buf, i, kb as usize, (i as u32 + 1) * 10);
                }
                for needle in 0..=(pop as u32 + 2) * 10 {
                    if needle > rem_mask(kb) {
                        continue;
                    }
                    let fast = leaf_insert_pos(&buf, pop, kb, needle);
                    let slow = leaf_lower_bound(&buf, pop, kb, needle).unwrap();
                    assert_eq!(
                        fast, slow,
                        "kb={kb} pop={pop} needle={needle}: fast {fast} vs search {slow}"
                    );
                }
            }
        }
    }

    /// ...and it must actually fire on the shape it exists for. A change that
    /// silently stopped taking the fast path would keep every test above green.
    #[test]
    fn leaf_insert_pos_short_circuits_ascending_keys() {
        let kb = 2u8;
        let pop = 12usize;
        let mut buf = vec![0u8; (pop + 1) * kb as usize];
        for i in 0..pop {
            write_rem(&mut buf, i, kb as usize, (i as u32 + 1) * 10);
        }
        // Past the last key: answered without entering the search.
        assert_eq!(leaf_insert_pos(&buf, pop, kb, 130), pop);
        // Equal to the last key: falls through, and finds it where it is.
        assert_eq!(leaf_insert_pos(&buf, pop, kb, 120), pop - 1);
        // Below it: falls through to the search.
        assert_eq!(leaf_insert_pos(&buf, pop, kb, 55), 5);
        // Empty leaf has no last key to compare against.
        assert_eq!(leaf_insert_pos(&buf, 0, kb, 7), 0);
    }

    #[test]
    fn tag_scheme_round_trips() {
        assert!(matches!(kind_of(T_NULL), Kind::Null));
        assert!(matches!(kind_of(T_L2), Kind::BranchL2));
        assert!(matches!(kind_of(T_L6), Kind::BranchL6));
        assert!(matches!(kind_of(T_B), Kind::BranchB));
        assert!(matches!(kind_of(T_U), Kind::BranchU));
        assert!(matches!(kind_of(T_BITMAP), Kind::Bitmap));
        assert!(matches!(kind_of(T_MAP_BITMAP), Kind::MapBitmap));
        for kb in 1..=4u8 {
            assert!(matches!(kind_of(t_set_leaf(kb)), Kind::SetLeaf(k) if k == kb));
            assert!(matches!(kind_of(t_map_leaf(kb)), Kind::MapLeaf(k) if k == kb));
        }
        for kb in 1..=4u8 {
            for count in 1..=set_immed_cap(kb) as u8 {
                let t = t_set_immed(kb, count);
                assert!(
                    matches!(kind_of(t), Kind::SetImmed { kb: k, count: c } if k == kb && c == count),
                    "set immed tag {t:#x} kb={kb} count={count}"
                );
            }
        }
        for kb in 1..=3u8 {
            assert!(matches!(kind_of(t_map_immed(kb)), Kind::MapImmed { kb: k } if k == kb));
        }
    }

    #[test]
    fn drain_frees_every_node() {
        // Grow well past every conversion boundary, then drain: the arena
        // must report zero live allocations and zero bytes.
        let mut a = Arena::new();
        let mut root = Edge32::null();
        #[cfg(not(miri))]
        const N: u32 = 2000;
        #[cfg(miri)]
        const N: u32 = 300;
        let keys: Vec<u32> = (0..N).map(|i| i.wrapping_mul(2_654_435_761)).collect();
        for &k in &keys {
            set_insert(&mut a, &mut root, 4, k);
        }
        assert!(a.bytes_in_use() > 0);
        assert!(a.live_allocs() > 0);
        for &k in &keys {
            assert!(set_remove(&mut a, &mut root, 4, k));
        }
        assert!(root.is_null());
        assert_eq!(a.live_allocs(), 0, "node leak after drain");
        assert_eq!(a.bytes_in_use(), 0, "byte leak after drain");
    }

    #[test]
    fn immediate_payload_round_trips_all_widths() {
        for kb in 1..=4u8 {
            let cap = set_immed_cap(kb);
            let keys: Vec<u32> = (0..cap as u32)
                .map(|i| ((i + 1) * 0x0101_0101) & rem_mask(kb))
                .collect();
            let mut sorted = keys.clone();
            sorted.sort_unstable();
            let e = set_immed_edge(kb, &sorted);
            let (out, n) = set_immed_keys(&e, kb, sorted.len() as u8);
            assert_eq!(&out[..n], &sorted[..]);
        }
    }

    /// Drives one leaf across every capacity-class boundary in both
    /// directions, pinning the #577 in-place/copy split: within a class
    /// the node handle (and thus the allocation) must be reused; across
    /// a class boundary the leaf must move to an exactly-sized node so
    /// the allocation size stays derivable from the population alone.
    #[test]
    fn leaf_mutation_reuses_allocation_within_cap_class() {
        for kb in 2..=4u8 {
            let mut a = Arena::new();
            let mut e = Edge32::null();
            // Interleaved keys so later inserts land at interior positions.
            let key = |i: u32| (i * 7 + 3) & rem_mask(kb);
            let mut n = 0u32;
            while !matches!(kind(&e), Kind::SetLeaf(_)) {
                set_insert(&mut a, &mut e, kb, key(n));
                n += 1;
            }
            while (n as usize) < SET_LEAF_MAX {
                let pop = edge_pop(&e);
                let h = edge_handle(&e);
                let bytes = a.bytes_in_use();
                set_insert(&mut a, &mut e, kb, key(n));
                n += 1;
                assert_eq!(edge_pop(&e), pop + 1);
                assert_eq!(a.bytes_in_use(), size_set32(kb, pop + 1));
                if cap_class(pop + 1) == cap_class(pop) {
                    assert_eq!(edge_handle(&e), h, "in-place insert moved the node");
                    assert_eq!(a.bytes_in_use(), bytes, "in-place insert changed bytes");
                } else {
                    assert_ne!(edge_handle(&e), h, "class growth must reallocate");
                }
            }
            // Walk back down: same contract for removes, until demotion.
            while matches!(kind(&e), Kind::SetLeaf(_)) {
                let pop = edge_pop(&e);
                let h = edge_handle(&e);
                n -= 1;
                assert!(set_remove(&mut a, &mut e, kb, key(n)));
                if matches!(kind(&e), Kind::SetLeaf(_)) {
                    assert_eq!(edge_pop(&e), pop - 1);
                    assert_eq!(a.bytes_in_use(), size_set32(kb, pop - 1));
                    if cap_class(pop - 1) == cap_class(pop) {
                        assert_eq!(edge_handle(&e), h, "in-place remove moved the node");
                    } else {
                        assert_ne!(edge_handle(&e), h, "class shrink must reallocate");
                    }
                }
            }
            // Drain the rest and verify nothing leaked.
            while n > 0 {
                n -= 1;
                assert!(set_remove(&mut a, &mut e, kb, key(n)));
            }
            assert!(e.is_null());
            assert_eq!(a.bytes_in_use(), 0, "byte leak after drain");
        }
    }

    /// The map value-overwrite path must be a pure in-place store: same
    /// node handle, same byte count, old value returned.
    #[test]
    fn map_leaf_overwrite_is_in_place() {
        let kb = 3u8;
        let mut a = Arena::new();
        let mut e = Edge32::null();
        for i in 0..8u32 {
            assert_eq!(map_insert(&mut a, &mut e, kb, i * 5, i), None);
        }
        assert!(matches!(kind(&e), Kind::MapLeaf(_)));
        let h = edge_handle(&e);
        let bytes = a.bytes_in_use();
        assert_eq!(map_insert(&mut a, &mut e, kb, 15, 999), Some(3));
        assert_eq!(edge_handle(&e), h, "overwrite moved the node");
        assert_eq!(a.bytes_in_use(), bytes, "overwrite changed bytes");
        assert_eq!(map_get(&a, &e, kb, 15), Some(999));
    }

    /// Removing a digit from a BranchL2 must accurately update header.pop0
    /// and match subtree_count.
    #[test]
    fn branch_l2_digit_removal_tracks_subtree_count() {
        let mut a = Arena::new();
        // Construct two leaf children
        let mut c1 = Edge32::null();
        map_insert(&mut a, &mut c1, 3, 0x01_0001, 1);
        map_insert(&mut a, &mut c1, 3, 0x01_0002, 2);
        let mut c2 = Edge32::null();
        map_insert(&mut a, &mut c2, 3, 0x02_0001, 3);
        map_insert(&mut a, &mut c2, 3, 0x02_0002, 4);

        let pairs = [(0x10u8, c1), (0x20u8, c2)];
        let mut e = make_l2(&mut a, 4, &pairs, 4);
        assert!(matches!(kind(&e), Kind::BranchL2));
        assert_eq!(branch_total_keys(&a, &e), 4);
        assert_eq!(subtree_count(&a, &e), 4);

        // Remove 1 key from digit 0x10 (in-place commit)
        assert_eq!(map_remove(&mut a, &mut e, 4, 0x1001_0001), Some(1));
        assert_eq!(branch_total_keys(&a, &e), 3);
        assert_eq!(subtree_count(&a, &e), 3);

        // Remove the other key from digit 0x10 (digit removal)
        assert_eq!(map_remove(&mut a, &mut e, 4, 0x1001_0002), Some(2));
        assert!(matches!(kind(&e), Kind::BranchL2));
        assert_eq!(branch_total_keys(&a, &e), 2);
        assert_eq!(subtree_count(&a, &e), 2);
    }
}
