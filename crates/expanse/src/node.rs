//! Phase 3: cache-line-native node layouts.
//!
//! Every node type is sized to exactly one (64 B) or two (128 B) cache
//! lines — or, for the uncompressed branch, a flat 4 KiB page — and aligned
//! to [`CACHE_LINE`], so a single node traversal never straddles an
//! unplanned line boundary. Layout invariants are enforced at compile time
//! with `size_of`/`align_of`/`offset_of` const assertions (the Phase 3
//! gate).
//!
//! The 16-byte [`Edge`] — the original literature's "Judy Pointer" (JP);
//! the compat-layer name is reserved for `expanse-capi` — follows the
//! published Judy IV word layout: word 0 is the child pointer (or immediate key payload), word 1 packs a
//! 7-byte auxiliary field plus the 1-byte type tag. The auxiliary field is
//! **level-split**: for a child at level `L`, its low `L` bytes hold
//! `pop0` (subtree population minus one — a level-`L` subtree holds at most
//! `256^L` keys, so `L` bytes always suffice) and the remaining high bytes
//! hold the narrow-pointer *decode* bytes. This is why no branch header
//! needs a wide population field.
//!
//! Linear-branch geometry note: the naive "8-byte header + 4 edges" 64-byte
//! branch is arithmetically impossible (8 + 4×16 = 72 > 64). The exact
//! one-line form is a 16-byte header + 3 edges ([`BranchL3`]); the two-line
//! form is the same header + 7 edges ([`BranchL7`]). The 16-byte header also
//! reserves the version counter the Phase 7 OCC read protocol needs.
//!
//! Linear *leaves* are variable-length allocations (packed key remainders
//! plus, for maps, a parallel value array); their concrete layout lands
//! with the allocator in Phase 5.

use crate::bits::Bitmap256;
use crate::types::{BRANCH_FANOUT, BRANCH_L3_CAP, BRANCH_L7_CAP, CACHE_LINE, EdgeTag, EdgeType};

/// Word 0 of an edge: a child-node pointer, or 8 of the up-to-15 immediate
/// key-payload bytes.
#[derive(Clone, Copy)]
#[repr(C)]
union Word0 {
    ptr: *mut u8,
    imm: [u8; 8],
}

/// The uniform 16-byte tagged edge descriptor of the trie (known as a
/// "Judy Pointer" / JP in the original literature).
///
/// ```text
/// offset  0: word 0   8 B  child pointer, or immediate key bytes
/// offset  8: aux      7 B  level-split: low L bytes pop0, high bytes decode
/// offset 15: tag      1 B  type tag (see types::EdgeTag)
/// ```
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Edge {
    w0: Word0,
    aux: [u8; 7],
    tag: u8,
}

impl Edge {
    /// The null edge: empty subexpanse.
    pub const NULL: Self = Self {
        w0: Word0 { imm: [0; 8] },
        aux: [0; 7],
        tag: EdgeType::Null as u8,
    };

    /// Builds an edge referring to a child node.
    #[inline]
    #[must_use]
    pub const fn new_node(ptr: *mut u8, tag: u8) -> Self {
        Self {
            w0: Word0 { ptr },
            aux: [0; 7],
            tag,
        }
    }

    /// The raw tag byte.
    #[inline]
    #[must_use]
    pub const fn tag_byte(&self) -> u8 {
        self.tag
    }

    /// The decoded tag, if the byte is a valid encoding.
    #[inline]
    #[must_use]
    pub const fn tag(&self) -> Option<EdgeTag> {
        EdgeTag::from_u8(self.tag)
    }

    /// Overwrites the tag byte.
    #[inline]
    pub const fn set_tag(&mut self, tag: u8) {
        self.tag = tag;
    }

    /// True for the null edge.
    #[inline]
    #[must_use]
    pub const fn is_null(&self) -> bool {
        self.tag == EdgeType::Null as u8
    }

    /// The child-node pointer of a pointer-carrying edge.
    ///
    /// Any 8 bytes are a valid `*mut u8` *value*, so this read is always
    /// defined; but the result carries provenance (and may be dereferenced)
    /// only for edges built with [`Self::new_node`] — which is the only way
    /// pointer-tagged edges are constructed.
    #[inline]
    #[must_use]
    pub fn node_ptr(&self) -> *mut u8 {
        // SAFETY: reading the `ptr` view of the union; every bit pattern is
        // a valid raw-pointer value (validity does not require provenance).
        unsafe { self.w0.ptr }
    }

    /// The 8 word-0 payload bytes of an immediate edge.
    ///
    /// Callers must only use this on immediate-tagged edges (which are built
    /// via [`Self::set_imm_bytes`], so word 0 holds plain bytes, never a
    /// pointer whose bytes would carry provenance).
    #[inline]
    #[must_use]
    pub fn imm_bytes(&self) -> [u8; 8] {
        debug_assert!(matches!(self.tag(), Some(EdgeTag::Immed(_))));
        // SAFETY: reading the `imm` view of the union; immediate edges store
        // plain bytes in word 0.
        unsafe { self.w0.imm }
    }

    /// Stores immediate key-payload bytes into word 0.
    #[inline]
    pub const fn set_imm_bytes(&mut self, bytes: [u8; 8]) {
        self.w0 = Word0 { imm: bytes };
    }

    /// The full 15-byte immediate payload (word 0 followed by the aux
    /// bytes) of a set-flavor immediate edge, which packs its keys across
    /// both regions. Same caller obligations as [`Self::imm_bytes`].
    #[inline]
    #[must_use]
    pub fn imm_payload(&self) -> [u8; 15] {
        let mut out = [0u8; 15];
        let w0 = self.imm_bytes();
        out[..8].copy_from_slice(&w0);
        out[8..].copy_from_slice(&self.aux);
        out
    }

    /// The 7 aux bytes, where map-flavor immediate edges pack their keys
    /// (word 0 holds the value, or the value-array pointer, instead).
    #[inline]
    #[must_use]
    pub const fn aux_bytes(&self) -> &[u8; 7] {
        &self.aux
    }

    /// Writes the aux bytes wholesale (immediate-edge key storage).
    #[inline]
    pub const fn set_aux_bytes(&mut self, bytes: [u8; 7]) {
        self.aux = bytes;
    }

    /// Clears one aux byte (used when a narrow pointer's top decode digit
    /// is consumed by a wrapping branch).
    #[inline]
    pub(crate) const fn clear_aux_byte(&mut self, i: usize) {
        self.aux[i] = 0;
    }

    /// Subtree population minus one, for a child at `level` (1..=7): reads
    /// the low `level` bytes of the aux field (little-endian).
    #[inline]
    #[must_use]
    pub fn pop0(&self, level: u8) -> u64 {
        debug_assert!((1..=7).contains(&level));
        // One masked load. The byte-at-a-time loop this replaces compiled
        // to ~22 instructions and 6 data-dependent branches — a serial
        // dependent-load chain — over a field that is already a
        // contiguous little-endian integer. Insert paid it twice per
        // level, on the way down and on the way back up (issue #1).
        self.aux_word() & (u64::MAX >> (64 - 8 * u32::from(level)))
    }

    /// The `aux` bytes and the tag byte read as one little-endian word:
    /// `aux[0]` is the low byte, `tag` the high byte.
    ///
    /// `Edge` is `repr(C)` and 8-byte aligned (word 0 is a union over a
    /// pointer), with `aux` at offset 8 and `tag` at 15 — both
    /// const-asserted at the bottom of this file — so the pair is exactly
    /// the second 64-bit word of the struct.
    #[inline]
    fn aux_word(&self) -> u64 {
        // SAFETY: the read takes its provenance from the whole `Edge`
        // (not from the `aux` field alone), stays inside the 16-byte
        // object, and is 8-byte aligned by the struct's own alignment.
        unsafe { (&raw const *self).cast::<u64>().add(1).read() }
    }

    /// Writes the `aux`/tag word read by [`Self::aux_word`].
    #[inline]
    fn set_aux_word(&mut self, w: u64) {
        // SAFETY: as in `aux_word`; same object, same offset, same
        // alignment, and `Edge` has no padding to invalidate.
        unsafe { (&raw mut *self).cast::<u64>().add(1).write(w) };
    }

    /// Stores `pop0` for a child at `level` (1..=7) into the low `level`
    /// aux bytes. `pop0` must fit in `level` bytes (a level-`level` subtree
    /// holds at most `256^level` keys).
    #[inline]
    pub fn set_pop0(&mut self, level: u8, pop0: u64) {
        debug_assert!((1..=7).contains(&level));
        debug_assert!(level == 7 || pop0 < 1u64 << (level as u32 * 8));
        // Masked read-modify-write of the same word `pop0` reads, so the
        // decode bytes above `level` and the tag byte are preserved.
        let mask = u64::MAX >> (64 - 8 * u32::from(level));
        let w = self.aux_word();
        self.set_aux_word((w & !mask) | (pop0 & mask));
    }

    /// The narrow-pointer decode bytes for a child at `level`: the high
    /// `7 - level` aux bytes. `decode[0]` is the byte nearest the child's
    /// level; unused when the edge skips no levels.
    #[inline]
    #[must_use]
    pub fn decode_bytes(&self, level: u8) -> &[u8] {
        debug_assert!((1..=7).contains(&level));
        &self.aux[level as usize..]
    }

    /// Writes `bytes` into the decode region for a child at `level`.
    /// `bytes.len()` must be at most `7 - level`.
    #[inline]
    pub fn set_decode_bytes(&mut self, level: u8, bytes: &[u8]) {
        debug_assert!((1..=7).contains(&level));
        self.aux[level as usize..level as usize + bytes.len()].copy_from_slice(bytes);
    }
}

impl Default for Edge {
    fn default() -> Self {
        Self::NULL
    }
}

impl core::fmt::Debug for Edge {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Edge")
            .field("tag", &format_args!("{:#04x}", self.tag))
            .field("aux", &self.aux)
            .finish_non_exhaustive()
    }
}

/// Common 16-byte header of linear branches: OCC version counter, child
/// count, and the sorted digit array searched with `bits::find_byte_8`.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct BranchHeader {
    /// Phase 7 OCC version counter (odd = mutation in progress). Plain for
    /// now; becomes atomic when the concurrent read protocol lands.
    pub version: u32,
    /// Number of populated child edges.
    pub num: u8,
    /// The node's own level. Behind a narrow pointer this sits below the
    /// slot level; the edge's decode bytes name the skipped digits.
    pub level: u8,
    _pad: [u8; 2],
    /// Sorted decode digits of the populated children; slot 7 is unused
    /// padding so the array is directly searchable as one 64-bit word.
    pub digits: [u8; 8],
}

impl BranchHeader {
    /// An empty header (version 0, no children) at `level`.
    #[must_use]
    pub const fn new(level: u8) -> Self {
        Self {
            version: 0,
            num: 0,
            level,
            _pad: [0; 2],
            digits: [0; 8],
        }
    }

    /// Slot of `digit` among the populated children, if present.
    #[inline]
    #[must_use]
    pub const fn find(&self, digit: u8) -> Option<usize> {
        crate::bits::find_byte_8(&self.digits, self.num as usize, digit)
    }
}

/// One-cache-line linear branch: header + up to 3 child edges.
#[derive(Debug)]
#[repr(C, align(64))]
pub struct BranchL3 {
    /// Header (version, count, digits).
    pub hdr: BranchHeader,
    /// Child edges, `hdr.num` of them populated, in digit order.
    pub edges: [Edge; BRANCH_L3_CAP],
}

impl BranchL3 {
    /// An empty branch at `level`.
    #[must_use]
    pub const fn new(level: u8) -> Self {
        Self {
            hdr: BranchHeader::new(level),
            edges: [Edge::NULL; BRANCH_L3_CAP],
        }
    }
}

/// Two-cache-line linear branch: header + up to 7 child edges.
#[derive(Debug)]
#[repr(C, align(64))]
pub struct BranchL7 {
    /// Header (version, count, digits).
    pub hdr: BranchHeader,
    /// Child edges, `hdr.num` of them populated, in digit order.
    pub edges: [Edge; BRANCH_L7_CAP],
}

impl BranchL7 {
    /// An empty branch at `level`.
    #[must_use]
    pub const fn new(level: u8) -> Self {
        Self {
            hdr: BranchHeader::new(level),
            edges: [Edge::NULL; BRANCH_L7_CAP],
        }
    }
}

/// Two-cache-line bitmap branch: 256-bit membership bitmap over eight
/// packed child-edge subarrays (one per 32-digit subexpanse).
///
/// Line 0 holds the bitmap and the first four subarray pointers — a lookup
/// that hits subexpanses 0x00..0x7F touches one line before the child edge.
/// Line 1 holds the remaining pointers, cached per-subexpanse population
/// counts (rank acceleration for `Count`/`ByCount`), and the OCC version.
#[derive(Debug)]
#[repr(C, align(64))]
pub struct BranchB {
    /// Membership bitmap over the 256 possible digits.
    pub bitmap: Bitmap256,
    /// Packed edge subarrays, one per 32-digit subexpanse; null when empty.
    pub subarrays: [*mut Edge; 8],
    /// Cached population count of each subexpanse's subarray.
    pub pop_counts: [u16; 8],
    /// Phase 7 OCC version counter.
    pub version: u32,
    /// The node's own level (see `BranchHeader::level`).
    pub level: u8,
    _pad: [u8; 11],
}

impl BranchB {
    /// An empty branch at `level`.
    #[must_use]
    pub const fn new(level: u8) -> Self {
        Self {
            bitmap: Bitmap256::new(),
            subarrays: [core::ptr::null_mut(); 8],
            pop_counts: [0; 8],
            version: 0,
            level,
            _pad: [0; 11],
        }
    }
}

/// Uncompressed branch: a flat page of 256 child edges, direct-indexed by
/// digit. One header line + 4 KiB of edges; used only above the
/// bitmap-density threshold. The header exists for the Phase 7 per-node
/// OCC version (a `BranchU` never skips, so it carries no level).
#[derive(Debug)]
#[repr(C, align(64))]
pub struct BranchU {
    /// Phase 7 OCC version counter (odd = mutation in progress).
    pub version: u32,
    _pad: [u8; 60],
    /// One edge per possible digit (null tag = empty subexpanse).
    pub edges: [Edge; BRANCH_FANOUT],
}

impl BranchU {
    /// An empty branch.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            version: 0,
            _pad: [0; 60],
            edges: [Edge::NULL; BRANCH_FANOUT],
        }
    }
}

impl Default for BranchU {
    fn default() -> Self {
        Self::new()
    }
}

/// One-cache-line bitmap leaf for key-presence (`ExpanseSet`; compat:
/// Judy1):
/// membership of the final key byte is the bitmap itself.
#[derive(Debug)]
#[repr(C, align(64))]
pub struct LeafBitmap1 {
    /// Membership of each possible final byte.
    pub bitmap: Bitmap256,
    /// Phase 7 OCC version counter.
    pub version: u32,
    _pad: [u8; 28],
}

impl LeafBitmap1 {
    /// An empty leaf.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bitmap: Bitmap256::new(),
            version: 0,
            _pad: [0; 28],
        }
    }
}

impl Default for LeafBitmap1 {
    fn default() -> Self {
        Self::new()
    }
}

/// Two-cache-line bitmap leaf for maps (`ExpanseMap`; compat: JudyL):
/// the bitmap
/// plus eight value-subarray pointers, one per 32-digit subexpanse; a
/// present key's value slot is found by popcount rank within its subarray.
#[derive(Debug)]
#[repr(C, align(64))]
pub struct LeafBitmapL {
    /// Membership of each possible final byte.
    pub bitmap: Bitmap256,
    /// Packed value subarrays, one per 32-digit subexpanse; null when empty.
    pub values: [*mut u64; 8],
    /// Phase 7 OCC version counter.
    pub version: u32,
    _pad: [u8; 28],
}

impl LeafBitmapL {
    /// An empty leaf.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bitmap: Bitmap256::new(),
            values: [core::ptr::null_mut(); 8],
            version: 0,
            _pad: [0; 28],
        }
    }
}

impl Default for LeafBitmapL {
    fn default() -> Self {
        Self::new()
    }
}

// ---- Phase 3 gate: layout invariants proven at compile time ----
const _: () = {
    use core::mem::{align_of, offset_of, size_of};

    assert!(size_of::<Edge>() == 16);
    assert!(align_of::<Edge>() == 8);
    assert!(offset_of!(Edge, aux) == 8);
    assert!(offset_of!(Edge, tag) == 15);
    // `Edge::aux_word` reads `aux` + `tag` as one little-endian word, so
    // `aux[0]` must be the low byte. The crate is 64-bit-only already;
    // this widens that gate to endianness.
    assert!(cfg!(target_endian = "little"));

    assert!(size_of::<BranchHeader>() == 16);
    assert!(offset_of!(BranchHeader, digits) == 8);

    assert!(size_of::<BranchL3>() == CACHE_LINE);
    assert!(align_of::<BranchL3>() == CACHE_LINE);
    assert!(offset_of!(BranchL3, edges) == 16);

    assert!(size_of::<BranchL7>() == 2 * CACHE_LINE);
    assert!(align_of::<BranchL7>() == CACHE_LINE);
    assert!(offset_of!(BranchL7, edges) == 16);

    assert!(size_of::<Bitmap256>() == 32);
    assert!(size_of::<BranchB>() == 2 * CACHE_LINE);
    assert!(align_of::<BranchB>() == CACHE_LINE);
    // Line 0: bitmap + first half of the subarray pointers.
    assert!(offset_of!(BranchB, subarrays) == 32);
    // Line 1: second half + rank cache + version.
    assert!(offset_of!(BranchB, pop_counts) == 96);
    assert!(offset_of!(BranchB, version) == 112);

    assert!(size_of::<BranchU>() == 4096 + CACHE_LINE);
    assert!(align_of::<BranchU>() == CACHE_LINE);

    assert!(size_of::<LeafBitmap1>() == CACHE_LINE);
    assert!(size_of::<LeafBitmapL>() == 2 * CACHE_LINE);
    assert!(offset_of!(LeafBitmapL, values) == 32);
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ImmedType;

    #[test]
    fn null_jp() {
        let jp = Edge::NULL;
        assert!(jp.is_null());
        assert_eq!(jp.tag(), Some(EdgeTag::Structural(EdgeType::Null)));
        assert_eq!(Edge::default().tag_byte(), jp.tag_byte());
    }

    #[test]
    fn node_pointer_roundtrip() {
        let mut backing = BranchL3::new(2);
        let raw = (&raw mut backing).cast::<u8>();
        let jp = Edge::new_node(raw, EdgeType::BranchL3.as_u8());
        assert!(!jp.is_null());
        assert_eq!(jp.node_ptr(), raw);
        assert_eq!(jp.tag(), Some(EdgeTag::Structural(EdgeType::BranchL3)));
    }

    #[test]
    fn imm_bytes_roundtrip() {
        let mut jp = Edge::NULL;
        jp.set_tag(ImmedType::new(2, 3).unwrap().as_u8());
        jp.set_imm_bytes([1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(jp.imm_bytes(), [1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn pop0_roundtrip_all_levels() {
        for level in 1..=7u8 {
            let max = if level == 7 {
                (1u64 << 56) - 1
            } else {
                (1u64 << (level as u32 * 8)) - 1
            };
            for pop0 in [0u64, 1, max / 2, max] {
                let mut jp = Edge::NULL;
                jp.set_pop0(level, pop0);
                assert_eq!(jp.pop0(level), pop0, "level={level} pop0={pop0}");
            }
        }
    }

    #[test]
    fn pop0_and_decode_do_not_overlap() {
        for level in 1..=6u8 {
            let mut jp = Edge::NULL;
            let max_pop0 = (1u64 << (level as u32 * 8)) - 1;
            jp.set_pop0(level, max_pop0);
            let decode: Vec<u8> = (0..(7 - level)).map(|i| 0xA0 | i).collect();
            jp.set_decode_bytes(level, &decode);
            assert_eq!(jp.pop0(level), max_pop0, "pop0 clobbered at level {level}");
            assert_eq!(jp.decode_bytes(level), &decode[..]);
        }
    }

    #[test]
    fn branch_header_find() {
        let mut hdr = BranchHeader::new(2);
        hdr.digits[..4].copy_from_slice(&[0x03, 0x41, 0x9C, 0xFF]);
        hdr.num = 4;
        assert_eq!(hdr.find(0x03), Some(0));
        assert_eq!(hdr.find(0x9C), Some(2));
        assert_eq!(hdr.find(0xFF), Some(3));
        assert_eq!(hdr.find(0x42), None);
        hdr.num = 2;
        assert_eq!(hdr.find(0x9C), None, "count limits the searched digits");
        // Unused slots (zero-filled) must not produce phantom matches.
        let empty = BranchHeader::new(2);
        assert_eq!(empty.find(0x00), None);
    }

    #[test]
    fn empty_nodes() {
        assert!(BranchL3::new(2).edges.iter().all(Edge::is_null));
        assert!(BranchL7::new(2).edges.iter().all(Edge::is_null));
        assert!(BranchU::new().edges.iter().all(Edge::is_null));
        let bb = BranchB::new(2);
        assert!(bb.bitmap.is_empty());
        assert!(bb.subarrays.iter().all(|p| p.is_null()));
        assert!(LeafBitmap1::new().bitmap.is_empty());
        let bl = LeafBitmapL::new();
        assert!(bl.bitmap.is_empty());
        assert!(bl.values.iter().all(|p| p.is_null()));
    }
}
