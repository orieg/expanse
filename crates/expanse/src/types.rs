//! Phase 1 foundation: key/value word types, node geometry constants, the
//! Judy Pointer (JP) type-tag encoding, and digit extraction.
//!
//! A Judy tree decodes a 64-bit key one byte ("digit") at a time across up
//! to eight levels. Level 8 consumes the most significant byte; level 1 the
//! least significant. Every edge in the tree is a 16-byte Judy Pointer whose
//! last byte is a type tag describing what the pointer refers to (branch
//! flavor, leaf flavor, or keys stored immediately inside the JP itself).
//! This module defines that tag encoding; the node layouts themselves land
//! in a later phase.

/// A key: one native machine word.
pub type Key = u64;

/// A value: one native machine word (JudyL maps `Key -> Value`).
pub type Value = u64;

/// Cache line size all node geometries are designed around.
///
/// The original Judy IV assumed 16-word (128-byte) cache lines; every modern
/// x86-64 and AArch64 core uses 64-byte lines, so nodes here are sized to
/// exactly one (64 B) or two (128 B) lines.
pub const CACHE_LINE: usize = 64;

/// Bytes in a full key, and the maximum decode depth of the tree.
pub const MAX_LEVEL: u8 = 8;

/// Fanout of one decode step: one byte selects among 256 subexpanses.
pub const BRANCH_FANOUT: usize = 256;

/// Capacity of the one-cache-line linear branch (4 child JPs + header).
pub const BRANCH_L4_CAP: usize = 4;

/// Capacity of the two-cache-line linear branch (7 child JPs + header).
pub const BRANCH_L8_CAP: usize = 7;

/// Populated-subexpanse count at which a bitmap branch converts to an
/// uncompressed (flat 256-slot) branch.
pub const BITMAP_TO_UNCOMPRESSED_THRESHOLD: usize = 192;

/// Bytes available inside a 16-byte JP for immediately stored keys
/// (payload word + decode/pop bytes, excluding the 1-byte type tag).
pub const IMMED_PAYLOAD_BYTES: usize = 15;

/// Structural JP type tags: branches and pointed-to leaves.
///
/// Immediate tags (keys packed inside the JP) use a separate nibble-packed
/// encoding — see [`ImmedType`] and [`JpTag`], which unify both spaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum JpType {
    /// Empty subexpanse: no keys present under this JP.
    Null = 0x00,
    /// Linear branch, one cache line, up to [`BRANCH_L4_CAP`] child JPs.
    BranchL4 = 0x01,
    /// Linear branch, two cache lines, up to [`BRANCH_L8_CAP`] child JPs.
    BranchL8 = 0x02,
    /// Bitmap branch: 256-bit membership bitmap over packed child JP arrays.
    BranchB = 0x03,
    /// Uncompressed branch: flat array of 256 child JPs.
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
    /// Fully populated subexpanse (Judy1): all keys present, no node needed.
    FullExpanse = 0x7F,
}

impl JpType {
    /// Decodes a structural tag from its raw byte, if it is one.
    #[must_use]
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            0x00 => Some(Self::Null),
            0x01 => Some(Self::BranchL4),
            0x02 => Some(Self::BranchL8),
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
            _ => None,
        }
    }

    /// The raw tag byte.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// True for the four branch flavors.
    #[must_use]
    pub const fn is_branch(self) -> bool {
        matches!(
            self,
            Self::BranchL4 | Self::BranchL8 | Self::BranchB | Self::BranchU
        )
    }

    /// True for the linear and bitmap leaf flavors.
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

/// An immediate JP tag: keys stored directly inside the 16-byte JP.
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
    /// Builds an immediate tag, if the combination fits in a JP.
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
    #[must_use]
    pub const fn from_u8(raw: u8) -> Option<Self> {
        Self::new(raw >> 4, (raw & 0x0F) + 1)
    }

    /// The raw tag byte.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        (self.key_bytes << 4) | (self.key_count - 1)
    }

    /// Undecoded bytes per key stored in this JP (1..=7).
    #[must_use]
    pub const fn key_bytes(self) -> u8 {
        self.key_bytes
    }

    /// Number of keys stored in this JP (1..=15).
    #[must_use]
    pub const fn key_count(self) -> u8 {
        self.key_count
    }

    /// Largest key count that fits for a given key size.
    #[must_use]
    pub const fn max_count(key_bytes: u8) -> u8 {
        (IMMED_PAYLOAD_BYTES / key_bytes as usize) as u8
    }
}

/// The full JP tag space: structural tags plus immediate tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JpTag {
    /// A branch, leaf, null, or full-expanse tag.
    Structural(JpType),
    /// Keys stored immediately inside the JP.
    Immed(ImmedType),
}

impl JpTag {
    /// Decodes any valid tag byte.
    #[must_use]
    pub const fn from_u8(raw: u8) -> Option<Self> {
        if let Some(t) = JpType::from_u8(raw) {
            Some(Self::Structural(t))
        } else if let Some(i) = ImmedType::from_u8(raw) {
            Some(Self::Immed(i))
        } else {
            None
        }
    }

    /// The raw tag byte.
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
#[must_use]
pub const fn digit(key: Key, level: u8) -> u8 {
    debug_assert!(level >= 1 && level <= MAX_LEVEL);
    (key >> ((level - 1) * 8)) as u8
}

// Layout invariants the rest of the implementation relies on.
const _: () = assert!(size_of::<usize>() == 8, "64-bit targets only");
const _: () = assert!(size_of::<Key>() == 8);
const _: () = assert!(CACHE_LINE == 4 * 16, "4 JPs per cache line");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structural_tags_round_trip() {
        let all = [
            JpType::Null,
            JpType::BranchL4,
            JpType::BranchL8,
            JpType::BranchB,
            JpType::BranchU,
            JpType::Leaf1,
            JpType::Leaf2,
            JpType::Leaf3,
            JpType::Leaf4,
            JpType::Leaf5,
            JpType::Leaf6,
            JpType::Leaf7,
            JpType::LeafB1,
            JpType::FullExpanse,
        ];
        for t in all {
            assert_eq!(JpType::from_u8(t.as_u8()), Some(t));
        }
    }

    #[test]
    fn tag_spaces_are_disjoint_and_total() {
        // Every byte decodes as structural, immediate, or invalid — never both.
        let mut valid = 0;
        for raw in 0..=u8::MAX {
            let s = JpType::from_u8(raw);
            let i = ImmedType::from_u8(raw);
            assert!(
                s.is_none() || i.is_none(),
                "tag {raw:#04x} decodes as both structural and immediate"
            );
            if s.is_some() || i.is_some() {
                valid += 1;
                let tag = JpTag::from_u8(raw).unwrap();
                assert_eq!(tag.as_u8(), raw);
            } else {
                assert_eq!(JpTag::from_u8(raw), None);
            }
        }
        // 14 structural tags + all (key_bytes, count) combos with
        // key_bytes * count <= 15: 15 + 7 + 5 + 3 + 3 + 2 + 2 = 37.
        assert_eq!(valid, 14 + 37);
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
                assert!(JpType::from_u8(raw).is_none(), "collision at {raw:#04x}");
            }
        }
    }

    #[test]
    fn classification_helpers() {
        assert!(JpType::BranchL4.is_branch());
        assert!(JpType::BranchU.is_branch());
        assert!(!JpType::Leaf1.is_branch());
        assert!(JpType::Leaf7.is_leaf());
        assert!(JpType::LeafB1.is_leaf());
        assert!(!JpType::Null.is_leaf());
        assert!(!JpType::FullExpanse.is_branch());
        assert!(!JpType::FullExpanse.is_leaf());
        assert_eq!(JpType::Leaf3.leaf_key_bytes(), Some(3));
        assert_eq!(JpType::LeafB1.leaf_key_bytes(), None);
        assert_eq!(JpType::BranchB.leaf_key_bytes(), None);
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
