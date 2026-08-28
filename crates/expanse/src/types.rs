//! Phase 1 foundation: key/value word types, node geometry constants, the
//! edge type-tag encoding, and digit extraction.
//!
//! An expanse trie decodes a 64-bit key one byte ("digit") at a time across up
//! to eight levels. Level 8 consumes the most significant byte; level 1 the
//! least significant. Every edge in the trie is a 16-byte tagged descriptor
//! (the original literature's "Judy Pointer" / JP; that name is reserved for
//! the `expanse-capi` compat layer) whose last byte is a type tag describing
//! what the edge refers to (branch
//! flavor, leaf flavor, or keys stored immediately inside the edge itself).
//! This module defines that tag encoding; the node layouts themselves land
//! in a later phase.

/// A key: one native machine word.
pub type Key = u64;

/// A value: one native machine word (the map flavor maps `Key -> Value`).
pub type Value = u64;

/// Node-packing granule (64 B) all node geometries are designed around.
///
/// This is a **node-packing constant**, not a live hardware cache-line
/// query: the original Judy IV assumed 16-word (128-byte) cache lines, and
/// nodes here are sized to exactly one (64 B) or two (128 B) of these
/// granules. It happens to match the 64-byte line of mainstream x86-64 and
/// most Cortex-A / Neoverse parts, but the ARM cache-line size is
/// IMPLEMENTATION DEFINED — read from `CTR_EL0`, and 128 B on Apple Silicon
/// (M1–M4). So over-aligning to this constant stays correct everywhere while
/// the "one node = one line" performance premise does not hold on 128-byte-
/// line parts. See docs/HARDWARE.md §2.4.
///
/// It is **not** false-sharing padding, and no `target_vendor`-gated 128-byte
/// widening is warranted: nothing in the concurrent path (`sync`/`occ`) pads a
/// field to this width. That path serializes writers on a mutex and validates
/// lock-free readers under a seqlock + EBR protocol (each reader's epoch slot
/// is its own `Arc<AtomicUsize>` allocation, never a slot in a shared array),
/// so nodes are read-mostly with a single intermittent writer — not the
/// multi-writer "every thread stores to its own adjacent field" pattern a
/// cache-line pad exists to isolate. Two 64-byte nodes co-residing on one
/// 128-byte Apple line is therefore a locality effect, never false sharing or
/// a correctness question, and over-aligning stays sound because 64 divides
/// 128.
pub const CACHE_LINE: usize = 64;

/// Alignment for **raw byte** allocations — packed linear leaves, edge
/// subarrays, value subarrays. These are addressed by computed offset and
/// are never cast to a `#[repr(C, align(64))]` node type, so they need
/// only `u64` alignment.
///
/// 16 rather than 8 because that is glibc's `MALLOC_ALIGNMENT` on 64-bit:
/// at or below it `alloc_zeroed` reaches `calloc`, which can hand back
/// pre-zeroed pages; above it the allocator takes `aligned_alloc` plus an
/// explicit memset. Per-function profiling measured `_int_malloc` at 11.2%
/// and `_mid_memalign` at 4.7% of `map_insert/random` when every
/// allocation asked for 64.
///
/// The six aligned node types keep [`CACHE_LINE`] via `alloc_node`.
pub const RAW_ALIGN: usize = 16;

/// Bytes in a full key, and the maximum decode depth of the tree.
pub const MAX_LEVEL: u8 = 8;

/// Fanout of one decode step: one byte selects among 256 subexpanses.
pub const BRANCH_FANOUT: usize = 256;

/// Capacity of the one-cache-line linear branch (16-byte header + 3 edges).
///
/// Note: a 4-edge linear branch cannot fit one 64-byte line once any header
/// exists (8 + 4 x 16 = 72 > 64); capacity 3 with a 16-byte header (which
/// also hosts the Phase 7 OCC version counter) is exact.
pub const BRANCH_L3_CAP: usize = 3;

/// Capacity of the two-cache-line linear branch (16-byte header + 7 edges).
pub const BRANCH_L7_CAP: usize = 7;

/// Populated-subexpanse count at which a bitmap branch converts to an
/// uncompressed (flat 256-slot) branch.
pub const BITMAP_TO_UNCOMPRESSED_THRESHOLD: usize = 192;

/// Bytes available inside a 16-byte edge for immediately stored keys
/// (payload word + decode/pop bytes, excluding the 1-byte type tag).
pub const IMMED_PAYLOAD_BYTES: usize = 15;

/// Structural edge type tags: branches and pointed-to leaves.
///
/// Immediate tags (keys packed inside the edge) use a separate nibble-packed
/// encoding — see [`ImmedType`] and [`EdgeTag`], which unify both spaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EdgeType {
    /// Empty subexpanse: no keys present under this edge.
    Null = 0x00,
    /// Linear branch, one cache line, up to [`BRANCH_L3_CAP`] child edges.
    BranchL3 = 0x01,
    /// Linear branch, two cache lines, up to [`BRANCH_L7_CAP`] child edges.
    BranchL7 = 0x02,
    /// Bitmap branch: 256-bit membership bitmap over packed child-edge arrays.
    BranchB = 0x03,
    /// Uncompressed branch: flat array of 256 child edges.
    BranchU = 0x04,
    /// Linear leaf holding packed 1-byte key remainders.
    Leaf1 = 0x05,
    /// Linear leaf holding packed 2-byte key remainders.
    Leaf2 = 0x06,
    /// Linear leaf holding packed 3-byte key remainders.
    Leaf3 = 0x07,
    /// Linear leaf holding packed 4-byte key remainders.
    Leaf4 = 0x08,
    /// Linear leaf holding packed 5-byte key remainders.
    Leaf5 = 0x09,
    /// Linear leaf holding packed 6-byte key remainders.
    Leaf6 = 0x0A,
    /// Linear leaf holding packed 7-byte key remainders.
    Leaf7 = 0x0B,
    /// Bitmap leaf at level 1: 256-bit mask over the final key byte.
    LeafB1 = 0x0C,
    /// Fully populated subexpanse (set flavor): all keys present, no node
    /// needed.
    FullExpanse = 0x7F,
    /// Linear branch (L3) at level 2.
    BranchL3L2 = 0x82,
    /// Linear branch (L3) at level 3.
    BranchL3L3 = 0x83,
    /// Linear branch (L3) at level 4.
    BranchL3L4 = 0x84,
    /// Linear branch (L3) at level 5.
    BranchL3L5 = 0x85,
    /// Linear branch (L3) at level 6.
    BranchL3L6 = 0x86,
    /// Linear branch (L3) at level 7.
    BranchL3L7 = 0x87,
    /// Linear branch (L3) at level 8.
    BranchL3L8 = 0x88,
}

impl EdgeType {
    /// Decodes a structural tag from its raw byte, if it is one.
    #[inline(always)]
    #[must_use]
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            0x00 => Some(Self::Null),
            0x01 => Some(Self::BranchL3),
            0x02 => Some(Self::BranchL7),
            0x03 => Some(Self::BranchB),
            0x04 => Some(Self::BranchU),
            0x05 => Some(Self::Leaf1),
            0x06 => Some(Self::Leaf2),
            0x07 => Some(Self::Leaf3),
            0x08 => Some(Self::Leaf4),
            0x09 => Some(Self::Leaf5),
            0x0A => Some(Self::Leaf6),
            0x0B => Some(Self::Leaf7),
            0x0C => Some(Self::LeafB1),
            0x7F => Some(Self::FullExpanse),
            0x82 => Some(Self::BranchL3L2),
            0x83 => Some(Self::BranchL3L3),
            0x84 => Some(Self::BranchL3L4),
            0x85 => Some(Self::BranchL3L5),
            0x86 => Some(Self::BranchL3L6),
            0x87 => Some(Self::BranchL3L7),
            0x88 => Some(Self::BranchL3L8),
            _ => None,
        }
    }

    /// The raw tag byte.
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// True for the four branch flavors.
    #[inline]
    #[must_use]
    pub const fn is_branch(self) -> bool {
        matches!(
            self,
            Self::BranchL3
                | Self::BranchL7
                | Self::BranchB
                | Self::BranchU
                | Self::BranchL3L2
                | Self::BranchL3L3
                | Self::BranchL3L4
                | Self::BranchL3L5
                | Self::BranchL3L6
                | Self::BranchL3L7
                | Self::BranchL3L8
        )
    }

    /// True for BranchL3 (unspecialized or per-level).
    #[inline]
    #[must_use]
    pub const fn is_branch_l3(self) -> bool {
        matches!(
            self,
            Self::BranchL3
                | Self::BranchL3L2
                | Self::BranchL3L3
                | Self::BranchL3L4
                | Self::BranchL3L5
                | Self::BranchL3L6
                | Self::BranchL3L7
                | Self::BranchL3L8
        )
    }

    /// Tag byte for a BranchL3 at `level` (2..=8).
    #[inline]
    #[must_use]
    pub const fn branch_l3_tag(level: u8) -> u8 {
        if level >= 2 && level <= 8 {
            0x80 | level
        } else {
            0x01
        }
    }

    /// True for the linear and bitmap leaf flavors.
    #[inline]
    #[must_use]
    pub const fn is_leaf(self) -> bool {
        matches!(
            self,
            Self::Leaf1
                | Self::Leaf2
                | Self::Leaf3
                | Self::Leaf4
                | Self::Leaf5
                | Self::Leaf6
                | Self::Leaf7
                | Self::LeafB1
        )
    }

    /// Undecoded key bytes remaining in each key of a linear leaf
    /// (`None` for non-linear-leaf tags).
    #[inline]
    #[must_use]
    pub const fn leaf_key_bytes(self) -> Option<u8> {
        match self {
            Self::Leaf1 => Some(1),
            Self::Leaf2 => Some(2),
            Self::Leaf3 => Some(3),
            Self::Leaf4 => Some(4),
            Self::Leaf5 => Some(5),
            Self::Leaf6 => Some(6),
            Self::Leaf7 => Some(7),
            _ => None,
        }
    }
}

/// An immediate edge tag: keys stored directly inside the 16-byte edge.
///
/// Packed encoding: `(key_bytes << 4) | (key_count - 1)`, giving the raw
/// range `0x10..=0x71`. A combination is valid when `1 <= key_bytes <= 7`
/// and `key_bytes * key_count <= IMMED_PAYLOAD_BYTES`, so the encoding never
/// collides with the structural tags above (`0x00..=0x0C`, `0x7F`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImmedType {
    key_bytes: u8,
    key_count: u8,
}

impl ImmedType {
    /// Builds an immediate tag, if the combination fits in an edge.
    #[inline]
    #[must_use]
    pub const fn new(key_bytes: u8, key_count: u8) -> Option<Self> {
        if key_bytes >= 1
            && key_bytes <= 7
            && key_count >= 1
            && (key_bytes as usize) * (key_count as usize) <= IMMED_PAYLOAD_BYTES
        {
            Some(Self {
                key_bytes,
                key_count,
            })
        } else {
            None
        }
    }

    /// Decodes an immediate tag from its raw byte, if it is one.
    #[inline(always)]
    #[must_use]
    pub const fn from_u8(raw: u8) -> Option<Self> {
        Self::new(raw >> 4, (raw & 0x0F) + 1)
    }

    /// The raw tag byte.
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        (self.key_bytes << 4) | (self.key_count - 1)
    }

    /// Undecoded bytes per key stored in this edge (1..=7).
    #[inline]
    #[must_use]
    pub const fn key_bytes(self) -> u8 {
        self.key_bytes
    }

    /// Number of keys stored in this edge (1..=15).
    #[inline]
    #[must_use]
    pub const fn key_count(self) -> u8 {
        self.key_count
    }

    /// Largest key count that fits for a given key size.
    #[inline]
    #[must_use]
    pub const fn max_count(key_bytes: u8) -> u8 {
        (IMMED_PAYLOAD_BYTES / key_bytes as usize) as u8
    }
}

/// The full edge tag space: structural tags plus immediate tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeTag {
    /// A branch, leaf, null, or full-expanse tag.
    Structural(EdgeType),
    /// Keys stored immediately inside the edge.
    Immed(ImmedType),
}

impl EdgeTag {
    /// Decodes any valid tag byte.
    #[inline(always)]
    #[must_use]
    pub const fn from_u8(raw: u8) -> Option<Self> {
        if let Some(t) = EdgeType::from_u8(raw) {
            Some(Self::Structural(t))
        } else if let Some(i) = ImmedType::from_u8(raw) {
            Some(Self::Immed(i))
        } else {
            None
        }
    }

    /// The raw tag byte.
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Structural(t) => t.as_u8(),
            Self::Immed(i) => i.as_u8(),
        }
    }
}

/// Extracts the decode digit for `key` at `level` (1..=[`MAX_LEVEL`]).
///
/// Level 8 is the most significant byte, level 1 the least significant:
/// a root-to-leaf descent consumes digits at levels 8, 7, ... down to 1.
#[inline(always)]
#[must_use]
pub const fn digit(key: Key, level: u8) -> u8 {
    debug_assert!(level >= 1 && level <= MAX_LEVEL);
    (key >> ((level - 1) * 8)) as u8
}

// Layout invariants the rest of the implementation relies on.
const _: () = assert!(size_of::<usize>() == 8, "64-bit targets only");
const _: () = assert!(size_of::<Key>() == 8);
const _: () = assert!(CACHE_LINE == 4 * 16, "4 edges per cache line");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structural_tags_round_trip() {
        let all = [
            EdgeType::Null,
            EdgeType::BranchL3,
            EdgeType::BranchL7,
            EdgeType::BranchB,
            EdgeType::BranchU,
            EdgeType::Leaf1,
            EdgeType::Leaf2,
            EdgeType::Leaf3,
            EdgeType::Leaf4,
            EdgeType::Leaf5,
            EdgeType::Leaf6,
            EdgeType::Leaf7,
            EdgeType::LeafB1,
            EdgeType::FullExpanse,
            EdgeType::BranchL3L2,
            EdgeType::BranchL3L3,
            EdgeType::BranchL3L4,
            EdgeType::BranchL3L5,
            EdgeType::BranchL3L6,
            EdgeType::BranchL3L7,
            EdgeType::BranchL3L8,
        ];
        for t in all {
            assert_eq!(EdgeType::from_u8(t.as_u8()), Some(t));
        }
    }

    #[test]
    fn tag_spaces_are_disjoint_and_total() {
        // Every byte decodes as structural, immediate, or invalid — never both.
        let mut valid = 0;
        for raw in 0..=u8::MAX {
            let s = EdgeType::from_u8(raw);
            let i = ImmedType::from_u8(raw);
            assert!(
                s.is_none() || i.is_none(),
                "tag {raw:#04x} decodes as both structural and immediate"
            );
            if s.is_some() || i.is_some() {
                valid += 1;
                let tag = EdgeTag::from_u8(raw).unwrap();
                assert_eq!(tag.as_u8(), raw);
            } else {
                assert_eq!(EdgeTag::from_u8(raw), None);
            }
        }
        // 21 structural tags + all (key_bytes, count) combos with
        // key_bytes * count <= 15: 15 + 7 + 5 + 3 + 3 + 2 + 2 = 37.
        assert_eq!(valid, 21 + 37);
    }

    #[test]
    fn immed_capacity_bounds() {
        assert_eq!(ImmedType::max_count(1), 15);
        assert_eq!(ImmedType::max_count(2), 7);
        assert_eq!(ImmedType::max_count(3), 5);
        assert_eq!(ImmedType::max_count(4), 3);
        assert_eq!(ImmedType::max_count(5), 3);
        assert_eq!(ImmedType::max_count(6), 2);
        assert_eq!(ImmedType::max_count(7), 2);
        assert!(ImmedType::new(1, 15).is_some());
        assert!(ImmedType::new(1, 16).is_none());
        assert!(ImmedType::new(7, 2).is_some());
        assert!(ImmedType::new(7, 3).is_none());
        assert!(ImmedType::new(0, 1).is_none());
        assert!(ImmedType::new(8, 1).is_none());
        assert!(ImmedType::new(1, 0).is_none());
    }

    #[test]
    fn immed_tags_never_collide_with_structural() {
        for kb in 1..=7u8 {
            for count in 1..=ImmedType::max_count(kb) {
                let raw = ImmedType::new(kb, count).unwrap().as_u8();
                assert!(EdgeType::from_u8(raw).is_none(), "collision at {raw:#04x}");
            }
        }
    }

    #[test]
    fn classification_helpers() {
        assert!(EdgeType::BranchL3.is_branch());
        assert!(EdgeType::BranchU.is_branch());
        assert!(!EdgeType::Leaf1.is_branch());
        assert!(EdgeType::Leaf7.is_leaf());
        assert!(EdgeType::LeafB1.is_leaf());
        assert!(!EdgeType::Null.is_leaf());
        assert!(!EdgeType::FullExpanse.is_branch());
        assert!(!EdgeType::FullExpanse.is_leaf());
        assert_eq!(EdgeType::Leaf3.leaf_key_bytes(), Some(3));
        assert_eq!(EdgeType::LeafB1.leaf_key_bytes(), None);
        assert_eq!(EdgeType::BranchB.leaf_key_bytes(), None);
    }

    #[test]
    fn digit_extraction() {
        let key: Key = 0x8877_6655_4433_2211;
        assert_eq!(digit(key, 1), 0x11);
        assert_eq!(digit(key, 2), 0x22);
        assert_eq!(digit(key, 7), 0x77);
        assert_eq!(digit(key, 8), 0x88);
        assert_eq!(digit(0, 8), 0x00);
        assert_eq!(digit(Key::MAX, 4), 0xFF);
    }
}
