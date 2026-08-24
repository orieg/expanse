//! 32-Bit Digital Tree Node Geometries & Cache-Line Alignment.
//!
//! Sizes node layouts to exact multiples of 32-byte cache lines (Cortex-M7, ESP32 cache)
//! per `docs/RFC_32BIT_EMBEDDED.md`.

use crate::types32::Edge32;
use core::fmt;

/// Header for 32-bit digital branch nodes (8 bytes).
#[derive(Clone, Copy)]
#[repr(C)]
pub struct BranchHeader32 {
    /// Total population count below this node (pop0 = count - 1).
    pub pop0: u32,
    /// Number of active child edges.
    pub num_edges: u8,
    /// Tree level (2..=4).
    pub level: u8,
    /// Optional prefix decode digits.
    pub decode: [u8; 2],
}

const _: () = assert!(core::mem::size_of::<BranchHeader32>() == 8);
const _: () = assert!(core::mem::align_of::<BranchHeader32>() == 4);

impl BranchHeader32 {
    /// Create a new branch header.
    #[inline(always)]
    pub const fn new(level: u8, num_edges: u8, pop0: u32) -> Self {
        Self {
            pop0,
            num_edges,
            level,
            decode: [0, 0],
        }
    }
}

/// Linear branch with 2 child edges (**32 bytes** total = exactly 1 embedded cache line).
#[derive(Clone, Copy)]
#[repr(C, align(32))]
pub struct BranchL2_32 {
    /// Branch header (8 bytes).
    pub header: BranchHeader32,
    /// Active decode digits (2 bytes).
    pub digits: [u8; 2],
    /// Alignment padding (6 bytes).
    pub _pad: [u8; 6],
    /// Child edges (2 x 8B = 16 bytes).
    pub edges: [Edge32; 2],
}

const _: () = assert!(core::mem::size_of::<BranchL2_32>() == 32);
const _: () = assert!(core::mem::align_of::<BranchL2_32>() == 32);

impl BranchL2_32 {
    /// Create a new 2-edge branch node.
    #[inline(always)]
    pub fn new(level: u8, d0: u8, e0: Edge32, d1: u8, e1: Edge32, pop0: u32) -> Self {
        let (digits, edges) = if d0 < d1 {
            ([d0, d1], [e0, e1])
        } else {
            ([d1, d0], [e1, e0])
        };
        Self {
            header: BranchHeader32::new(level, 2, pop0),
            digits,
            _pad: [0; 6],
            edges,
        }
    }

    /// Search for digit in branch, returning edge index if found.
    #[inline(always)]
    pub fn find(&self, digit: u8) -> Option<usize> {
        if self.digits[0] == digit {
            Some(0)
        } else if self.digits[1] == digit {
            Some(1)
        } else {
            None
        }
    }
}

/// Linear branch with 6 child edges (**64 bytes** total = exactly 2 embedded cache lines).
#[derive(Clone, Copy)]
#[repr(C, align(32))]
pub struct BranchL6_32 {
    /// Branch header (8 bytes).
    pub header: BranchHeader32,
    /// Active decode digits (6 bytes).
    pub digits: [u8; 6],
    /// Alignment padding (2 bytes).
    pub _pad: [u8; 2],
    /// Child edges (6 x 8B = 48 bytes).
    pub edges: [Edge32; 6],
}

const _: () = assert!(core::mem::size_of::<BranchL6_32>() == 64);
const _: () = assert!(core::mem::align_of::<BranchL6_32>() == 32);

impl BranchL6_32 {
    /// Create an empty 6-edge branch node.
    #[inline(always)]
    pub fn new(level: u8) -> Self {
        Self {
            header: BranchHeader32::new(level, 0, 0),
            digits: [0; 6],
            _pad: [0; 2],
            edges: [Edge32::null(); 6],
        }
    }

    /// Linear search for digit within active edges.
    #[inline(always)]
    pub fn find(&self, digit: u8) -> Option<usize> {
        let count = self.header.num_edges as usize;
        let mut i = 0;
        while i < count {
            if self.digits[i] == digit {
                return Some(i);
            }
            i += 1;
        }
        None
    }
}

/// Uncompressed branch: a flat page of 256 child edges direct-indexed by
/// digit (**2080 bytes** = 65x 32-byte cache lines). A 50% structural
/// reduction versus the 64-bit `BranchU` (256 x 16B edges + 64B header =
/// 4160 bytes), used only above the linear-branch fanout threshold.
#[derive(Clone, Copy)]
#[repr(C, align(32))]
pub struct BranchU32 {
    /// Total keys below this branch (subtree population).
    pub count: u32,
    /// Number of populated child edges (0..=256).
    pub num_children: u16,
    /// Tree level (2..=4).
    pub level: u8,
    /// Alignment padding to a 32-byte header.
    pub _pad: [u8; 25],
    /// One edge per possible digit; null tag marks an empty subexpanse.
    pub edges: [Edge32; 256],
}

const _: () = assert!(core::mem::size_of::<BranchU32>() == 2080);
const _: () = assert!(core::mem::align_of::<BranchU32>() == 32);

impl BranchU32 {
    /// Create an empty uncompressed branch at `level`.
    #[inline(always)]
    #[must_use]
    pub fn new(level: u8) -> Self {
        Self {
            count: 0,
            num_children: 0,
            level,
            _pad: [0; 25],
            edges: [Edge32::null(); 256],
        }
    }
}

/// 256-Bit Bitmap Leaf for Level 1 (Judy1Set32 leaf).
#[derive(Clone, Copy)]
#[repr(C, align(32))]
pub struct LeafBitmap1_32 {
    /// 256-bit bitmask (32 bytes = 1 cache line).
    pub bitmap: [u64; 4],
    /// Population count in this leaf.
    pub pop0: u16,
    /// Level (always 1).
    pub level: u8,
    /// Padding to 36 bytes.
    pub _pad: u8,
}

impl LeafBitmap1_32 {
    /// Create a new empty bitmap leaf.
    #[inline(always)]
    pub const fn new() -> Self {
        Self {
            bitmap: [0; 4],
            pop0: 0,
            level: 1,
            _pad: 0,
        }
    }

    /// Test if bit is set.
    #[inline(always)]
    pub fn test(&self, bit: u8) -> bool {
        let word_idx = (bit / 64) as usize;
        let bit_idx = bit % 64;
        (self.bitmap[word_idx] & (1 << bit_idx)) != 0
    }

    /// Set a bit in the bitmap, returning true if bit was newly set.
    #[inline(always)]
    pub fn set(&mut self, bit: u8) -> bool {
        let word_idx = (bit / 64) as usize;
        let bit_idx = bit % 64;
        let mask = 1 << bit_idx;
        let was_set = (self.bitmap[word_idx] & mask) != 0;
        if !was_set {
            self.bitmap[word_idx] |= mask;
            self.pop0 += 1;
        }
        !was_set
    }

    /// Clear a bit in the bitmap, returning true if bit was removed.
    #[inline(always)]
    pub fn unset(&mut self, bit: u8) -> bool {
        let word_idx = (bit / 64) as usize;
        let bit_idx = bit % 64;
        let mask = 1 << bit_idx;
        let was_set = (self.bitmap[word_idx] & mask) != 0;
        if was_set {
            self.bitmap[word_idx] &= !mask;
            self.pop0 -= 1;
        }
        was_set
    }
}

impl Default for LeafBitmap1_32 {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for LeafBitmap1_32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LeafBitmap1_32")
            .field("pop", &(self.pop0 + 1))
            .field(
                "bitmap",
                &format_args!(
                    "[0x{:016X}, 0x{:016X}, 0x{:016X}, 0x{:016X}]",
                    self.bitmap[0], self.bitmap[1], self.bitmap[2], self.bitmap[3]
                ),
            )
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node32_sizes_and_alignments() {
        assert_eq!(core::mem::size_of::<BranchHeader32>(), 8);
        assert_eq!(core::mem::size_of::<BranchL2_32>(), 32);
        assert_eq!(core::mem::size_of::<BranchL6_32>(), 64);
        assert_eq!(core::mem::align_of::<BranchL2_32>(), 32);
        assert_eq!(core::mem::align_of::<BranchL6_32>(), 32);
    }

    #[test]
    fn test_branch_l2_32_find() {
        let e0 = Edge32::new_immed_map_u8(1, 100);
        let e1 = Edge32::new_immed_map_u8(2, 200);
        let branch = BranchL2_32::new(2, 0x10, e0, 0x20, e1, 2);

        assert_eq!(branch.find(0x10), Some(0));
        assert_eq!(branch.find(0x20), Some(1));
        assert_eq!(branch.find(0x30), None);
    }

    #[test]
    fn test_leaf_bitmap1_32_set_unset() {
        let mut leaf = LeafBitmap1_32::new();
        assert!(!leaf.test(42));
        assert!(leaf.set(42));
        assert!(leaf.test(42));
        assert!(!leaf.set(42)); // already set

        assert!(leaf.set(200));
        assert_eq!(leaf.pop0, 2);

        assert!(leaf.unset(42));
        assert!(!leaf.test(42));
        assert_eq!(leaf.pop0, 1);
    }
}
