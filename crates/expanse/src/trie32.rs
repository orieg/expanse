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

#[cfg(not(feature = "std"))]
extern crate alloc as alloc_crate;
#[cfg(not(feature = "std"))]
use alloc_crate::{boxed::Box, vec::Vec};

use crate::node32::{BranchHeader32, BranchL2_32, BranchL6_32, BranchU32, LeafBitmap1_32};
use crate::types32::Edge32;

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
// Conversion thresholds (memory/perf tuning only; never affect correctness)
// ---------------------------------------------------------------------------

/// Immediate (in-edge) key capacity for a subtree whose keys have
/// `key_bytes` undecoded bytes: `floor(7 / key_bytes)`.
#[inline]
const fn set_immed_cap(kb: u8) -> usize {
    7 / kb as usize
}

/// A level-`>=2` set linear leaf converts to a branch above this
/// population.
const SET_LEAF_MAX: usize = 24;
/// A level-1 set linear leaf converts to a bitmap leaf above this
/// population (bitmap leaf is 64 B, so it only wins once the linear leaf
/// would exceed it).
const SET_BITMAP_ENTER: usize = 64;
/// A level-1 set bitmap leaf demotes back to a linear leaf at or below this
/// population (hysteresis gap vs. [`SET_BITMAP_ENTER`]).
const SET_BITMAP_LEAVE: usize = 48;
/// A level-`>=2` map linear leaf converts to a branch above this
/// population.
const MAP_LEAF_MAX: usize = 16;

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
    BranchU,
    Bitmap,
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
        T_U => Kind::BranchU,
        T_BITMAP => Kind::Bitmap,
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

/// One arena-owned node. Each variant is an independent heap allocation
/// sized to the RFC's on-target byte layout, so [`Arena::bytes_in_use`]
/// is an exact memory figure.
enum NodeBox {
    L2(Box<BranchL2_32>),
    L6(Box<BranchL6_32>),
    U(Box<BranchU32>),
    Bitmap(Box<LeafBitmap1_32>),
    Leaf(Box<[u8]>),
}

impl NodeBox {
    #[inline]
    fn heap_bytes(&self) -> usize {
        match self {
            NodeBox::L2(_) => core::mem::size_of::<BranchL2_32>(),
            NodeBox::L6(_) => core::mem::size_of::<BranchL6_32>(),
            NodeBox::U(_) => core::mem::size_of::<BranchU32>(),
            NodeBox::Bitmap(_) => core::mem::size_of::<LeafBitmap1_32>(),
            NodeBox::Leaf(b) => b.len(),
        }
    }
}

/// Per-tree node arena. Hands out 32-bit handles (indices) and keeps
/// byte-exact accounting of live node allocations. Mirrors the role of the
/// 64-bit `NodeAlloc`, adapted to be pointer-width-independent.
pub(crate) struct Arena {
    slots: Vec<Option<NodeBox>>,
    free: Vec<u32>,
    bytes: usize,
}

impl Arena {
    #[inline]
    pub(crate) const fn new() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            bytes: 0,
        }
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
        self.bytes += node.heap_bytes();
        if let Some(h) = self.free.pop() {
            self.slots[h as usize] = Some(node);
            h
        } else {
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
    fn leaf(&self, h: u32) -> &[u8] {
        match self.get(h) {
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

#[inline]
fn alloc_zeroed_bytes(n: usize) -> Box<[u8]> {
    // `vec![0u8; n]` needs the `vec!` macro, which is awkward to name under
    // `no_std`; this resize-from-empty is the portable equivalent.
    #[allow(clippy::slow_vector_initialization)]
    let mut v = Vec::new();
    v.resize(n, 0u8);
    v.into_boxed_slice()
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

fn bitmap_first_ge(leaf: &LeafBitmap1_32, from: u16) -> Option<u8> {
    if from > 255 {
        return None;
    }
    let from = from as usize;
    let mut w = from / 64;
    let mut word = leaf.bitmap[w] & (u64::MAX << (from % 64));
    loop {
        if word != 0 {
            return Some((w * 64 + word.trailing_zeros() as usize) as u8);
        }
        w += 1;
        if w >= 4 {
            return None;
        }
        word = leaf.bitmap[w];
    }
}

fn bitmap_last_le(leaf: &LeafBitmap1_32, to: i32) -> Option<u8> {
    if to < 0 {
        return None;
    }
    let to = to.min(255) as usize;
    let mut w = to / 64;
    let shift = 63 - (to % 64);
    let mut word = (leaf.bitmap[w] << shift) >> shift;
    loop {
        if word != 0 {
            return Some((w * 64 + (63 - word.leading_zeros() as usize)) as u8);
        }
        if w == 0 {
            return None;
        }
        w -= 1;
        word = leaf.bitmap[w];
    }
}

fn bitmap_count_range(leaf: &LeafBitmap1_32, lo: u32, hi: u32) -> usize {
    let lo = lo.min(255) as usize;
    let hi = hi.min(255) as usize;
    if lo > hi {
        return 0;
    }
    let mut n = 0;
    for w in 0..4usize {
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
        n += (leaf.bitmap[w] & mask).count_ones() as usize;
    }
    n
}

// ---------------------------------------------------------------------------
// Branch helpers (shared by set and map — branches only route by digit)
// ---------------------------------------------------------------------------

fn branch_level(a: &Arena, e: &Edge32) -> u8 {
    match kind(e) {
        Kind::BranchL2 => a.l2(edge_handle(e)).header.level,
        Kind::BranchL6 => a.l6(edge_handle(e)).header.level,
        Kind::BranchU => a.u(edge_handle(e)).level,
        _ => unreachable!(),
    }
}

fn branch_total_keys(a: &Arena, e: &Edge32) -> u32 {
    match kind(e) {
        Kind::BranchL2 => a.l2(edge_handle(e)).header.pop0,
        Kind::BranchL6 => a.l6(edge_handle(e)).header.pop0,
        Kind::BranchU => a.u(edge_handle(e)).count,
        _ => unreachable!(),
    }
}

fn branch_add_keys(a: &mut Arena, e: &Edge32, delta: i64) {
    match kind(e) {
        Kind::BranchL2 => {
            let b = a.l2_mut(edge_handle(e));
            b.header.pop0 = (b.header.pop0 as i64 + delta) as u32;
        }
        Kind::BranchL6 => {
            let b = a.l6_mut(edge_handle(e));
            b.header.pop0 = (b.header.pop0 as i64 + delta) as u32;
        }
        Kind::BranchU => {
            let b = a.u_mut(edge_handle(e));
            b.count = (b.count as i64 + delta) as u32;
        }
        _ => unreachable!(),
    }
}

/// Child edge at `digit`, if present.
fn branch_child(a: &Arena, e: &Edge32, digit: u8) -> Option<Edge32> {
    match kind(e) {
        Kind::BranchL2 => {
            let b = a.l2(edge_handle(e));
            let n = b.header.num_edges as usize;
            (0..n).find(|&i| b.digits[i] == digit).map(|i| b.edges[i])
        }
        Kind::BranchL6 => {
            let b = a.l6(edge_handle(e));
            let n = b.header.num_edges as usize;
            (0..n).find(|&i| b.digits[i] == digit).map(|i| b.edges[i])
        }
        Kind::BranchU => {
            let b = a.u(edge_handle(e));
            let c = b.edges[digit as usize];
            if c.is_null() { None } else { Some(c) }
        }
        _ => unreachable!(),
    }
}

/// Overwrite the (already-present) child edge at `digit`.
fn branch_set_child(a: &mut Arena, e: &Edge32, digit: u8, child: Edge32) {
    match kind(e) {
        Kind::BranchL2 => {
            let b = a.l2_mut(edge_handle(e));
            let n = b.header.num_edges as usize;
            for i in 0..n {
                if b.digits[i] == digit {
                    b.edges[i] = child;
                    return;
                }
            }
            unreachable!("branch_set_child: digit not present");
        }
        Kind::BranchL6 => {
            let b = a.l6_mut(edge_handle(e));
            let n = b.header.num_edges as usize;
            for i in 0..n {
                if b.digits[i] == digit {
                    b.edges[i] = child;
                    return;
                }
            }
            unreachable!("branch_set_child: digit not present");
        }
        Kind::BranchU => {
            a.u_mut(edge_handle(e)).edges[digit as usize] = child;
        }
        _ => unreachable!(),
    }
}

/// Collect all `(digit, child)` pairs of a branch in ascending digit order.
fn branch_pairs(a: &Arena, e: &Edge32) -> Vec<(u8, Edge32)> {
    match kind(e) {
        Kind::BranchL2 => {
            let b = a.l2(edge_handle(e));
            let n = b.header.num_edges as usize;
            (0..n).map(|i| (b.digits[i], b.edges[i])).collect()
        }
        Kind::BranchL6 => {
            let b = a.l6(edge_handle(e));
            let n = b.header.num_edges as usize;
            (0..n).map(|i| (b.digits[i], b.edges[i])).collect()
        }
        Kind::BranchU => {
            let b = a.u(edge_handle(e));
            (0..256usize)
                .filter(|&d| !b.edges[d].is_null())
                .map(|d| (d as u8, b.edges[d]))
                .collect()
        }
        _ => unreachable!(),
    }
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
            if n < 6 {
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
                let total = a.l6(edge_handle(e)).header.pop0 + keys_added;
                let mut pairs = branch_pairs(a, e);
                insert_pair_sorted(&mut pairs, digit, child);
                let old = edge_handle(e);
                *e = make_u(a, level, &pairs, total);
                a.free(old);
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

/// Remove the structurally-empty child at `digit` (its subtree is now
/// null). The total-key count has already been decremented by the caller.
/// Frees or demotes the branch as its child count shrinks; sets `*e` to
/// null if the branch becomes empty.
fn branch_remove_digit(a: &mut Arena, e: &mut Edge32, digit: u8) {
    let level = branch_level(a, e);
    match kind(e) {
        Kind::BranchL2 => {
            let b = a.l2_mut(edge_handle(e));
            let n = b.header.num_edges as usize;
            let mut pos = 0;
            while pos < n && b.digits[pos] != digit {
                pos += 1;
            }
            debug_assert!(pos < n);
            for i in pos..n - 1 {
                b.digits[i] = b.digits[i + 1];
                b.edges[i] = b.edges[i + 1];
            }
            b.digits[n - 1] = 0;
            b.edges[n - 1] = Edge32::null();
            b.header.num_edges = (n - 1) as u8;
            if n - 1 == 0 {
                let old = edge_handle(e);
                a.free(old);
                *e = Edge32::null();
            }
        }
        Kind::BranchL6 => {
            let b = a.l6_mut(edge_handle(e));
            let n = b.header.num_edges as usize;
            let mut pos = 0;
            while pos < n && b.digits[pos] != digit {
                pos += 1;
            }
            debug_assert!(pos < n);
            for i in pos..n - 1 {
                b.digits[i] = b.digits[i + 1];
                b.edges[i] = b.edges[i + 1];
            }
            b.digits[n - 1] = 0;
            b.edges[n - 1] = Edge32::null();
            b.header.num_edges = (n - 1) as u8;
            let new_n = n - 1;
            if new_n <= 2 {
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
        Kind::BranchU => {
            let b = a.u_mut(edge_handle(e));
            debug_assert!(!b.edges[digit as usize].is_null());
            b.edges[digit as usize] = Edge32::null();
            b.num_children -= 1;
            let new_n = b.num_children as usize;
            if new_n <= 6 {
                let total = a.u(edge_handle(e)).count;
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
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchU => branch_total_keys(a, e) as usize,
    }
}

pub(crate) fn set_contains(a: &Arena, e: &Edge32, kb: u8, rem: u32) -> bool {
    match kind(e) {
        Kind::Null => false,
        Kind::SetImmed { kb: _, count } => {
            let (keys, n) = set_immed_keys(e, kb, count);
            keys[..n].binary_search(&rem).is_ok()
        }
        Kind::SetLeaf(_) => {
            let pop = edge_pop(e);
            let buf = a.leaf(edge_handle(e));
            leaf_lower_bound(buf, pop, kb, rem)
                .map(|pos| pos < pop && read_rem(buf, pos, kb as usize) == rem)
                .unwrap_or(false)
        }
        Kind::Bitmap => a.bitmap(edge_handle(e)).test(rem as u8),
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchU => {
            let d = digit_at(rem, kb);
            match branch_child(a, e, d) {
                Some(c) => set_contains(a, &c, kb - 1, child_rem(rem, kb)),
                None => false,
            }
        }
        _ => false,
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
                    if n < set_immed_cap(kb) {
                        let mut v: Vec<u32> = keys[..n].to_vec();
                        v.insert(pos, rem);
                        *e = set_immed_edge(kb, &v);
                    } else {
                        // Promote immediate -> linear leaf.
                        let mut v: Vec<u32> = keys[..n].to_vec();
                        v.insert(pos, rem);
                        *e = make_set_leaf(a, kb, &v);
                    }
                    true
                }
            }
        }
        Kind::SetLeaf(_) => {
            let pop = edge_pop(e);
            let buf = a.leaf(edge_handle(e));
            let pos = leaf_lower_bound(buf, pop, kb, rem).unwrap();
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
                let mut keys = read_set_leaf(a, e, kb);
                keys.insert(pos, rem);
                let old = edge_handle(e);
                *e = make_set_leaf(a, kb, &keys);
                a.free(old);
            }
            true
        }
        Kind::Bitmap => a.bitmap_mut(edge_handle(e)).set(rem as u8),
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchU => {
            let d = digit_at(rem, kb);
            let cr = child_rem(rem, kb);
            match branch_child(a, e, d) {
                Some(mut child) => {
                    let inserted = set_insert(a, &mut child, kb - 1, cr);
                    if inserted {
                        branch_set_child(a, e, d, child);
                        branch_add_keys(a, e, 1);
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
        Kind::MapLeaf(_) | Kind::MapImmed { .. } => unreachable!("set op on a map edge"),
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
                    let mut v: Vec<u32> = keys[..n].to_vec();
                    v.remove(pos);
                    *e = if v.is_empty() {
                        Edge32::null()
                    } else {
                        set_immed_edge(kb, &v)
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
            let mut keys = read_set_leaf(a, e, kb);
            keys.remove(pos);
            let old = edge_handle(e);
            *e = if keys.is_empty() {
                Edge32::null()
            } else if keys.len() <= set_immed_cap(kb) {
                set_immed_edge(kb, &keys)
            } else {
                make_set_leaf(a, kb, &keys)
            };
            a.free(old);
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
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchU => {
            let d = digit_at(rem, kb);
            let cr = child_rem(rem, kb);
            match branch_child(a, e, d) {
                None => false,
                Some(mut child) => {
                    let removed = set_remove(a, &mut child, kb - 1, cr);
                    if removed {
                        branch_add_keys(a, e, -1);
                        if child.is_null() {
                            branch_remove_digit(a, e, d);
                        } else {
                            branch_set_child(a, e, d, child);
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

pub(crate) fn map_get(a: &Arena, e: &Edge32, kb: u8, rem: u32) -> Option<u32> {
    match kind(e) {
        Kind::Null => None,
        Kind::MapImmed { kb: _ } => {
            if map_immed_rem(e, kb) == rem {
                Some(map_immed_val(e))
            } else {
                None
            }
        }
        Kind::MapLeaf(_) => {
            let pop = edge_pop(e);
            let cap = cap_class(pop);
            let buf = a.leaf(edge_handle(e));
            let keys_off = 4 * cap;
            let pos = leaf_lower_bound(&buf[keys_off..], pop, kb, rem)?;
            if pos < pop && read_rem(&buf[keys_off..], pos, kb as usize) == rem {
                let mut vb = [0u8; 4];
                vb.copy_from_slice(&buf[pos * 4..pos * 4 + 4]);
                Some(u32::from_le_bytes(vb))
            } else {
                None
            }
        }
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchU => {
            let d = digit_at(rem, kb);
            let c = branch_child(a, e, d)?;
            map_get(a, &c, kb - 1, child_rem(rem, kb))
        }
        _ => None,
    }
}

pub(crate) fn map_insert(a: &mut Arena, e: &mut Edge32, kb: u8, rem: u32, val: u32) -> Option<u32> {
    match kind(e) {
        Kind::Null => {
            *e = if kb <= 3 {
                map_immed_edge(kb, rem, val)
            } else {
                make_map_leaf(a, kb, &[(rem, val)])
            };
            None
        }
        Kind::MapImmed { kb: _ } => {
            let r0 = map_immed_rem(e, kb);
            if r0 == rem {
                let old = map_immed_val(e);
                *e = map_immed_edge(kb, rem, val);
                Some(old)
            } else {
                let v0 = map_immed_val(e);
                let mut entries = if r0 < rem {
                    Vec::from([(r0, v0), (rem, val)])
                } else {
                    Vec::from([(rem, val), (r0, v0)])
                };
                entries.dedup_by_key(|p| p.0);
                *e = make_map_leaf(a, kb, &entries);
                None
            }
        }
        Kind::MapLeaf(_) => {
            let pop = edge_pop(e);
            let cap = cap_class(pop);
            let keys_off = 4 * cap;
            let buf = a.leaf(edge_handle(e));
            let pos = leaf_lower_bound(&buf[keys_off..], pop, kb, rem).unwrap();
            if pos < pop && read_rem(&buf[keys_off..], pos, kb as usize) == rem {
                let mut entries = read_map_leaf(a, e, kb);
                let old = entries[pos].1;
                entries[pos].1 = val;
                let oldh = edge_handle(e);
                *e = make_map_leaf(a, kb, &entries);
                a.free(oldh);
                return Some(old);
            }
            let new_pop = pop + 1;
            if kb >= 2 && new_pop > MAP_LEAF_MAX {
                let entries = read_map_leaf(a, e, kb);
                let oldh = edge_handle(e);
                *e = make_l2(a, kb, &[], 0);
                for &(k, v) in &entries {
                    map_insert(a, e, kb, k, v);
                }
                map_insert(a, e, kb, rem, val);
                a.free(oldh);
            } else {
                let mut entries = read_map_leaf(a, e, kb);
                entries.insert(pos, (rem, val));
                let oldh = edge_handle(e);
                *e = make_map_leaf(a, kb, &entries);
                a.free(oldh);
            }
            None
        }
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchU => {
            let d = digit_at(rem, kb);
            let cr = child_rem(rem, kb);
            match branch_child(a, e, d) {
                Some(mut child) => {
                    let old = map_insert(a, &mut child, kb - 1, cr, val);
                    branch_set_child(a, e, d, child);
                    if old.is_none() {
                        branch_add_keys(a, e, 1);
                    }
                    old
                }
                None => {
                    let mut child = Edge32::null();
                    map_insert(a, &mut child, kb - 1, cr, val);
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
            let mut entries = read_map_leaf(a, e, kb);
            let old = entries.remove(pos).1;
            let oldh = edge_handle(e);
            *e = if entries.is_empty() {
                Edge32::null()
            } else if entries.len() == 1 && kb <= 3 {
                map_immed_edge(kb, entries[0].0, entries[0].1)
            } else {
                make_map_leaf(a, kb, &entries)
            };
            a.free(oldh);
            Some(old)
        }
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchU => {
            let d = digit_at(rem, kb);
            let cr = child_rem(rem, kb);
            match branch_child(a, e, d) {
                None => None,
                Some(mut child) => {
                    let old = map_remove(a, &mut child, kb - 1, cr);
                    if old.is_some() {
                        branch_add_keys(a, e, -1);
                        if child.is_null() {
                            branch_remove_digit(a, e, d);
                        } else {
                            branch_set_child(a, e, d, child);
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
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchU => {
            let pairs = branch_pairs(a, e);
            let (d, c) = pairs.first()?;
            first(a, c, kb - 1).map(|cr| combine(*d, cr, kb))
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
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchU => {
            let pairs = branch_pairs(a, e);
            let (d, c) = pairs.last()?;
            last(a, c, kb - 1).map(|cr| combine(*d, cr, kb))
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
            (0..pop)
                .map(|i| read_rem(buf, i, kb as usize))
                .find(|&k| k > after)
        }
        Kind::MapLeaf(_) => {
            let pop = edge_pop(e);
            let cap = cap_class(pop);
            let buf = a.leaf(edge_handle(e));
            (0..pop)
                .map(|i| read_rem(&buf[4 * cap..], i, kb as usize))
                .find(|&k| k > after)
        }
        Kind::Bitmap => {
            if after >= 255 {
                None
            } else {
                bitmap_first_ge(a.bitmap(edge_handle(e)), after as u16 + 1).map(u32::from)
            }
        }
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchU => {
            let da = digit_at(after, kb);
            let ca = child_rem(after, kb);
            let pairs = branch_pairs(a, e);
            for &(d, c) in &pairs {
                if d < da {
                    continue;
                }
                if d == da {
                    if let Some(cr) = next(a, &c, kb - 1, ca) {
                        return Some(combine(d, cr, kb));
                    }
                } else {
                    if let Some(cr) = first(a, &c, kb - 1) {
                        return Some(combine(d, cr, kb));
                    }
                }
            }
            None
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
            (0..pop)
                .rev()
                .map(|i| read_rem(buf, i, kb as usize))
                .find(|&k| k < before)
        }
        Kind::MapLeaf(_) => {
            let pop = edge_pop(e);
            let cap = cap_class(pop);
            let buf = a.leaf(edge_handle(e));
            (0..pop)
                .rev()
                .map(|i| read_rem(&buf[4 * cap..], i, kb as usize))
                .find(|&k| k < before)
        }
        Kind::Bitmap => {
            if before == 0 {
                None
            } else {
                bitmap_last_le(a.bitmap(edge_handle(e)), before as i32 - 1).map(u32::from)
            }
        }
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchU => {
            let db = digit_at(before, kb);
            let cb = child_rem(before, kb);
            let pairs = branch_pairs(a, e);
            for &(d, c) in pairs.iter().rev() {
                if d > db {
                    continue;
                }
                if d == db {
                    if let Some(cr) = prev(a, &c, kb - 1, cb) {
                        return Some(combine(d, cr, kb));
                    }
                } else {
                    if let Some(cr) = last(a, &c, kb - 1) {
                        return Some(combine(d, cr, kb));
                    }
                }
            }
            None
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
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchU => {
            let d_lo = digit_at(lo, kb);
            let d_hi = digit_at(hi, kb);
            let r_lo = child_rem(lo, kb);
            let r_hi = child_rem(hi, kb);
            let full = rem_mask(kb - 1);
            let mut total = 0;
            for (d, c) in branch_pairs(a, e) {
                if d < d_lo || d > d_hi {
                    continue;
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
            }
            total
        }
    }
}

/// Visit every `(remainder, value)` of a map subtree whose remainder lies
/// in `[lo, hi]`, in ascending order.
pub(crate) fn map_for_each_range(
    a: &Arena,
    e: &Edge32,
    kb: u8,
    lo: u32,
    hi: u32,
    f: &mut dyn FnMut(u32, u32),
) {
    if lo > hi {
        return;
    }
    match kind(e) {
        Kind::Null => {}
        Kind::MapImmed { kb: _ } => {
            let r = map_immed_rem(e, kb);
            if r >= lo && r <= hi {
                f(r, map_immed_val(e));
            }
        }
        Kind::MapLeaf(_) => {
            let pop = edge_pop(e);
            let cap = cap_class(pop);
            let buf = a.leaf(edge_handle(e));
            let keys_off = 4 * cap;
            for i in 0..pop {
                let k = read_rem(&buf[keys_off..], i, kb as usize);
                if k >= lo && k <= hi {
                    let mut vb = [0u8; 4];
                    vb.copy_from_slice(&buf[i * 4..i * 4 + 4]);
                    f(k, u32::from_le_bytes(vb));
                }
            }
        }
        Kind::BranchL2 | Kind::BranchL6 | Kind::BranchU => {
            let d_lo = digit_at(lo, kb);
            let d_hi = digit_at(hi, kb);
            let r_lo = child_rem(lo, kb);
            let r_hi = child_rem(hi, kb);
            let full = rem_mask(kb - 1);
            for (d, c) in branch_pairs(a, e) {
                if d < d_lo || d > d_hi {
                    continue;
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
                // Re-thread the caller's closure with the reconstructed key.
                let mut g = |cr: u32, v: u32| f(combine(d, cr, kb), v);
                map_for_each_range(a, &c, kb - 1, clo, chi, &mut g);
            }
        }
        _ => {}
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

    #[test]
    fn tag_scheme_round_trips() {
        assert!(matches!(kind_of(T_NULL), Kind::Null));
        assert!(matches!(kind_of(T_L2), Kind::BranchL2));
        assert!(matches!(kind_of(T_U), Kind::BranchU));
        assert!(matches!(kind_of(T_BITMAP), Kind::Bitmap));
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
}
