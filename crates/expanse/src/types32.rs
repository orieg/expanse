//! 32-bit machine types, constants, and compact 8-byte `Edge32` descriptor.
//!
//! Implements the core 32-bit data structures for embedded targets (RV32, ESP32, Cortex-M)
//! per `docs/design/32-bit-embedded.md`.

use core::convert::From;
use core::default::Default;
use core::fmt;
use core::option::Option::{self, None, Some};

/// 32-bit machine key index.
pub type Key32 = u32;

/// 32-bit machine value word.
pub type Value32 = u32;

/// Maximum decode level for 32-bit architecture (Level 4 -> Level 1).
pub const MAX_LEVEL_32: u8 = 4;

/// 32-byte node-packing granule for embedded microcontrollers.
///
/// Matches the 32-byte cache line of cached cores (Cortex-M7, ESP32 MMU
/// cache), but the CI-targeted Cortex-M4 is **cacheless** — there the real
/// justification is AXI burst length / DMA block size / avoiding
/// misaligned-SRAM-access traps, not a cache line. See the design rationale
/// in docs/design/32-bit-embedded.md and docs/HARDWARE.md §4.1.
pub const CACHE_LINE_32: usize = 32;

/// Extract the decode digit for `key` at `level` (1..=4).
///
/// - Level 4: extracts MSB byte `[31:24]` (root descent)
/// - Level 3: extracts byte `[23:16]`
/// - Level 2: extracts byte `[15:8]`
/// - Level 1: extracts LSB byte `[7:0]` (leaf level)
#[inline(always)]
#[must_use]
pub const fn digit32(key: Key32, level: u8) -> u8 {
    debug_assert!(level >= 1 && level <= MAX_LEVEL_32);
    (key >> ((level - 1) * 8)) as u8
}

/// Mask the key below `level`.
///
/// Returns the remainder of `key` below the given level.
#[inline(always)]
#[must_use]
pub const fn mask_below32(key: Key32, level: u8) -> Key32 {
    if level <= 1 {
        0
    } else {
        let shift = (level - 1) * 8;
        key & ((1 << shift) - 1)
    }
}

/// Maximum capacity of a `BranchL2_32` node (2 child edges).
pub const BRANCH_L2_CAP_32: usize = 2;

/// Maximum capacity of a `BranchL6_32` node (6 child edges).
pub const BRANCH_L6_CAP_32: usize = 6;

/// Threshold where a `BranchB32` bitmap branch promotes to a `BranchU32` uncompressed branch (> 192 child edges).
pub const BRANCH_B_TO_UNCOMPRESSED_THRESHOLD_32: usize = 192;

/// Demotion threshold for `BranchU32` back to `BranchB32` (<= 190 child edges, band of 2).
pub const BRANCH_U_DOWN_32: usize = 190;

/// Demotion threshold for `BranchB32` back to `BranchL6_32` (<= 5 child edges, band of 1).
pub const BRANCH_B_DOWN_32: usize = 5;

/// Demotion threshold for `BranchL6_32` back to `BranchL2_32` (<= 1 child edge, band of 1).
pub const BRANCH_L6_DOWN_32: usize = 1;

/// Maximum population for a set linear leaf at level >= 2 before converting to a branch.
pub const SET_LEAF_MAX_32: usize = 24;

/// Population threshold where a level-1 set linear leaf converts to `LeafBitmap1_32`.
pub const SET_BITMAP_ENTER_32: usize = 64;

/// Population threshold where a `LeafBitmap1_32` set leaf demotes to a linear leaf (band of 16).
pub const SET_BITMAP_LEAVE_32: usize = 48;

/// Maximum population for a map linear leaf at level >= 2 before converting to a branch.
pub const MAP_LEAF_MAX_32: usize = 16;

/// Population threshold where a level-1 map linear leaf converts to `LeafBitmapL_32`.
pub const MAP_BITMAP_ENTER_32: usize = 64;

/// Population threshold where a `LeafBitmapL_32` map leaf demotes to a linear leaf (band of 16).
pub const MAP_BITMAP_LEAVE_32: usize = 48;

/// Tag discriminants for `Edge32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Tag32 {
    /// Empty edge.
    Null = 0x00,
    /// Level 1 Bitmap Leaf (256-bit set or 256-word map).
    LeafBitmap1 = 0x01,
    /// Level 1 Linear Leaf.
    LeafLinear1 = 0x02,
    /// Level 2 Linear Leaf.
    LeafLinear2 = 0x03,
    /// Level 3 Linear Leaf.
    LeafLinear3 = 0x04,
    /// Level 2 Branch (2 child edges, 32 bytes total).
    BranchL2 = 0x05,
    /// Level 6 Branch (6 child edges, 64 bytes total).
    BranchL6 = 0x06,
    /// Bitmap Branch (256-bit mask + dynamic child array).
    BranchB = 0x07,
    /// Uncompressed 256-edge branch.
    BranchU = 0x08,
    /// Level 1 Map Bitmap Leaf (256-bit mask + 8 value subarrays).
    LeafBitmapL = 0x09,
    /// In-edge immediate set (1..=7 keys packed in 7 bytes).
    ImmedSet = 0x10,
    /// In-edge immediate map (1 key + 1 value packed in edge).
    ImmedMap = 0x11,
    /// Polymorphic 32-bit ValueSlot inline payload (0..=3 bytes).
    ValueSlotInline = 0x20,
    /// Polymorphic 32-bit ValueSlot arena reference (16b meta + 12b offset).
    ValueSlotArena = 0x21,
    /// Raw uninterpreted 32-bit value.
    ValueSlotRaw = 0x22,
    /// Unknown / custom tag.
    Custom = 0xFF,
}

impl Tag32 {
    /// Convert raw byte to Tag32 in const context.
    #[inline(always)]
    pub const fn from_u8(tag: u8) -> Self {
        match tag {
            0x00 => Tag32::Null,
            0x01 => Tag32::LeafBitmap1,
            0x02 => Tag32::LeafLinear1,
            0x03 => Tag32::LeafLinear2,
            0x04 => Tag32::LeafLinear3,
            0x05 => Tag32::BranchL2,
            0x06 => Tag32::BranchL6,
            0x07 => Tag32::BranchB,
            0x08 => Tag32::BranchU,
            0x09 => Tag32::LeafBitmapL,
            0x10 => Tag32::ImmedSet,
            0x11 => Tag32::ImmedMap,
            0x20 => Tag32::ValueSlotInline,
            0x21 => Tag32::ValueSlotArena,
            0x22 => Tag32::ValueSlotRaw,
            _ => Tag32::Custom,
        }
    }
}

impl From<u8> for Tag32 {
    #[inline(always)]
    fn from(tag: u8) -> Self {
        Self::from_u8(tag)
    }
}

/// Compact 8-byte Edge descriptor for 32-bit targets.
///
/// Halves structural pointer overhead from 16 bytes (64-bit) to 8 bytes (32-bit).
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Edge32 {
    /// Word 0 (4 bytes): Child node pointer OR 32-bit payload.
    w0: u32,
    /// Aux field (3 bytes): Decode bytes / subtree population count / metadata.
    aux: [u8; 3],
    /// Tag discriminant (1 byte).
    tag: u8,
}

const _: () = assert!(core::mem::size_of::<Edge32>() == 8);
const _: () = assert!(core::mem::align_of::<Edge32>() == 4);

impl Edge32 {
    /// Create a null edge.
    #[inline(always)]
    pub const fn null() -> Self {
        Self {
            w0: 0,
            aux: [0; 3],
            tag: Tag32::Null as u8,
        }
    }

    /// Check if this edge is null.
    #[inline(always)]
    pub const fn is_null(&self) -> bool {
        self.tag == Tag32::Null as u8 && self.w0 == 0
    }

    /// Returns the tag discriminant.
    #[inline(always)]
    pub const fn tag(&self) -> Tag32 {
        Tag32::from_u8(self.tag)
    }

    /// Returns the raw tag byte.
    #[inline(always)]
    pub const fn raw_tag(&self) -> u8 {
        self.tag
    }

    /// Create an edge pointing to a child node with a tag and 24-bit aux value.
    #[inline(always)]
    pub fn new_node(ptr: *mut u8, tag: Tag32, aux_val: u32) -> Self {
        debug_assert!((aux_val & 0xFF00_0000) == 0, "aux_val must fit in 24 bits");
        let aux = [
            (aux_val & 0xFF) as u8,
            ((aux_val >> 8) & 0xFF) as u8,
            ((aux_val >> 16) & 0xFF) as u8,
        ];
        Self {
            w0: ptr as usize as u32,
            aux,
            tag: tag as u8,
        }
    }

    /// Returns the child node raw pointer.
    #[inline(always)]
    pub fn node_ptr<T>(&self) -> *mut T {
        self.w0 as usize as *mut T
    }

    /// Returns the 24-bit aux value.
    #[inline(always)]
    pub const fn aux_u24(&self) -> u32 {
        (self.aux[0] as u32) | ((self.aux[1] as u32) << 8) | ((self.aux[2] as u32) << 16)
    }

    /// Returns the aux slice (3 bytes).
    #[inline(always)]
    pub const fn aux_bytes(&self) -> &[u8; 3] {
        &self.aux
    }

    /// Create an immediate set edge packing up to 7 1-byte keys.
    #[inline(always)]
    pub fn new_immed_set_u8(keys: &[u8]) -> Option<Self> {
        if keys.len() > 7 {
            return None;
        }
        let mut raw = [0u8; 7];
        let mut i = 0;
        while i < keys.len() {
            raw[i] = keys[i];
            i += 1;
        }
        let w0 = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
        let aux = [raw[4], raw[5], raw[6]];
        Some(Self {
            w0,
            aux,
            tag: Tag32::ImmedSet as u8,
        })
    }

    /// Extract immediate 1-byte keys from this edge.
    #[inline(always)]
    pub fn immed_set_u8(&self, count: usize) -> [u8; 7] {
        let w0_bytes = self.w0.to_le_bytes();
        let mut res = [0u8; 7];
        let mut i = 0;
        while i < 4 && i < count {
            res[i] = w0_bytes[i];
            i += 1;
        }
        while i < 7 && i < count {
            res[i] = self.aux[i - 4];
            i += 1;
        }
        res
    }

    /// Create an immediate map edge storing a 1-byte key remainder and a 32-bit value.
    #[inline(always)]
    pub fn new_immed_map_u8(key_byte: u8, val: Value32) -> Self {
        Self {
            w0: val,
            aux: [key_byte, 0, 0],
            tag: Tag32::ImmedMap as u8,
        }
    }

    /// Returns the immediate map value.
    #[inline(always)]
    pub const fn immed_map_val(&self) -> Value32 {
        self.w0
    }

    /// Returns the immediate map key remainder byte.
    #[inline(always)]
    pub const fn immed_map_key_byte(&self) -> u8 {
        self.aux[0]
    }

    // -- Low-level part accessors used by the real 32-bit trie engine
    //    (`trie32`). These treat the edge as an opaque 8-byte record:
    //    `w0` is a child *handle* (arena index) or packed immediate
    //    payload, `aux` carries per-kind metadata (leaf population, or
    //    more immediate payload), and the tag byte is written raw by the
    //    engine's own tag scheme rather than the [`Tag32`] enum.

    /// Builds an edge from its raw parts.
    #[inline(always)]
    #[must_use]
    pub(crate) const fn from_parts(w0: u32, aux: [u8; 3], tag: u8) -> Self {
        Self { w0, aux, tag }
    }

    /// Raw word-0 value (child handle or low 4 immediate payload bytes).
    #[inline(always)]
    #[must_use]
    pub(crate) const fn w0_raw(&self) -> u32 {
        self.w0
    }

    /// Raw 3-byte aux field.
    #[inline(always)]
    #[must_use]
    pub(crate) const fn aux_raw(&self) -> [u8; 3] {
        self.aux
    }
}

impl Default for Edge32 {
    #[inline(always)]
    fn default() -> Self {
        Self::null()
    }
}

impl fmt::Debug for Edge32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Edge32")
            .field("tag", &self.tag())
            .field("w0", &format_args!("0x{:08X}", self.w0))
            .field("aux", &format_args!("0x{:06X}", self.aux_u24()))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge32_layout_and_size() {
        assert_eq!(core::mem::size_of::<Edge32>(), 8);
        assert_eq!(core::mem::align_of::<Edge32>(), 4);
    }

    #[test]
    fn test_digit32_extraction() {
        let key: Key32 = 0x12345678;
        assert_eq!(digit32(key, 4), 0x12);
        assert_eq!(digit32(key, 3), 0x34);
        assert_eq!(digit32(key, 2), 0x56);
        assert_eq!(digit32(key, 1), 0x78);
    }

    #[test]
    fn test_mask_below32() {
        let key: Key32 = 0x12345678;
        assert_eq!(mask_below32(key, 4), 0x00345678);
        assert_eq!(mask_below32(key, 3), 0x00005678);
        assert_eq!(mask_below32(key, 2), 0x00000078);
        assert_eq!(mask_below32(key, 1), 0x00000000);
    }

    #[test]
    fn test_edge32_null() {
        let edge = Edge32::null();
        assert!(edge.is_null());
        assert_eq!(edge.tag(), Tag32::Null);
    }

    #[test]
    fn test_edge32_node_ptr_roundtrip() {
        let dummy = 0x12345678_usize as *mut u8;
        let edge = Edge32::new_node(dummy, Tag32::BranchL2, 0xABCDEF);
        assert_eq!(edge.tag(), Tag32::BranchL2);
        assert_eq!(edge.node_ptr::<u8>(), dummy);
        assert_eq!(edge.aux_u24(), 0xABCDEF);
    }

    #[test]
    fn test_edge32_immed_set_u8() {
        let keys = [0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70];
        let edge = Edge32::new_immed_set_u8(&keys).expect("valid keys <= 7");
        assert_eq!(edge.tag(), Tag32::ImmedSet);
        let extracted = edge.immed_set_u8(7);
        assert_eq!(extracted, keys);
    }

    #[test]
    fn test_edge32_immed_map_u8() {
        let edge = Edge32::new_immed_map_u8(0x5A, 0xDEADBEEF);
        assert_eq!(edge.tag(), Tag32::ImmedMap);
        assert_eq!(edge.immed_map_key_byte(), 0x5A);
        assert_eq!(edge.immed_map_val(), 0xDEADBEEF);
    }
}
