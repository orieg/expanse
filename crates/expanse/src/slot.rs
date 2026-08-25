//! Polymorphic 64-bit value slots and columnar predicate filter kernels.
//!
//! Expanse stores a 64-bit word per key in its map leaves. The [`ValueSlot`]
//! abstraction allows packing small payloads (up to 7 bytes) directly inline
//! with zero heap allocation, or packing 32-bit hot metadata (TTL, flags,
//! timestamps) alongside a 24-bit arena locator into a single 64-bit word.
//!
//! # Bit Layouts
//!
//! 1. **Inline Mode** (`<= 7 bytes`):
//!    `[payload_byte_6 .. payload_byte_0 | tag (0x00..=0x07)]`
//!    Bits `[7:0]` encode the payload length (0 to 7 bytes).
//!
//! 2. **Arena Mode (Short)**:
//!    `[hot_meta (32 bits) | arena_offset (24 bits) | tag (0x10)]`
//!    - Bits `[63:32]`: 32-bit hot metadata (filterable without cold DRAM fetches).
//!    - Bits `[31:8]`: 24-bit arena byte offset (up to 16 MiB).
//!    - Bits `[7:0]`: `0x10` ([`SlotTag::ArenaShort`]).
//!
//! 3. **Arena Mode (Long)**:
//!    `[chunk_id (16 bits) | chunk_offset (40 bits) | tag (0x11)]`
//!    - Bits `[63:48]`: 16-bit chunk ID (arena chunk index).
//!    - Bits `[47:8]`: 40-bit intra-chunk byte offset.
//!    - Bits `[7:0]`: `0x11` ([`SlotTag::ArenaLong`]).
//!    - Produced by [`crate::blobmap::BlobArena`] once a blob's global byte
//!      offset would exceed the 24-bit `ArenaShort` range (16 MiB); it lifts
//!      the arena ceiling to `65536 * chunk_size` (bounded by a shipped safety
//!      cap, see [`crate::blobmap::MAX_ARENA_CAPACITY`]). Unlike `ArenaShort`,
//!      this encoding has no room for the 32-bit hot-metadata word, so
//!      `ArenaLong`-backed values carry no filterable hot metadata (reported
//!      as `0`).
//!
//! 4. **Raw Scalar / Unmanaged Word**:
//!    Uninterpreted 64-bit machine word.

/// Discriminant tag indicating how a [`ValueSlot`] is formatted.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum SlotTag {
    /// Inline payload of 0 bytes (empty value).
    Inline0 = 0x00,
    /// Inline payload of 1 byte (in bits `[15:8]`).
    Inline1 = 0x01,
    /// Inline payload of 2 bytes (in bits `[23:8]`).
    Inline2 = 0x02,
    /// Inline payload of 3 bytes (in bits `[31:8]`).
    Inline3 = 0x03,
    /// Inline payload of 4 bytes (in bits `[39:8]`).
    Inline4 = 0x04,
    /// Inline payload of 5 bytes (in bits `[47:8]`).
    Inline5 = 0x05,
    /// Inline payload of 6 bytes (in bits `[55:8]`).
    Inline6 = 0x06,
    /// Inline payload of 7 bytes (in bits `[63:8]`).
    Inline7 = 0x07,

    /// Backed by BlobArena: 32-bit hot metadata + 24-bit arena locator
    /// (global byte offset). Addresses the first 16 MiB of arena.
    ArenaShort = 0x10,
    /// Backed by BlobArena beyond the 16 MiB `ArenaShort` range: 16-bit chunk
    /// ID + 40-bit intra-chunk offset.
    ///
    /// The blob map produces this encoding (via [`ValueSlot::new_arena_long`],
    /// read back via [`ValueSlot::arena_long_loc`]) once a blob's global offset
    /// crosses the 24-bit `ArenaShort` ceiling, lifting the live arena beyond
    /// 16 MiB. It has no hot-metadata field — all 56 non-tag bits address the
    /// payload — so `ArenaLong`-backed values report hot metadata as `0`.
    ArenaLong = 0x11,
    /// Off-heap / External memory reference.
    ///
    /// **Reserved / not yet implemented.** No code path produces or consumes it.
    External = 0x12,

    /// Soft-deleted tombstone marker.
    Tombstone = 0xFE,
    /// Raw uninterpreted 64-bit word (unmanaged).
    RawWord = 0xFF,
}

impl SlotTag {
    /// Decodes a tag from its raw discriminant byte.
    #[inline(always)]
    #[must_use]
    pub const fn from_u8(tag: u8) -> Self {
        match tag {
            0x00 => Self::Inline0,
            0x01 => Self::Inline1,
            0x02 => Self::Inline2,
            0x03 => Self::Inline3,
            0x04 => Self::Inline4,
            0x05 => Self::Inline5,
            0x06 => Self::Inline6,
            0x07 => Self::Inline7,
            0x10 => Self::ArenaShort,
            0x11 => Self::ArenaLong,
            0x12 => Self::External,
            0xFE => Self::Tombstone,
            _ => Self::RawWord,
        }
    }

    /// Returns `true` if the tag represents an inline byte payload (0..=7 bytes).
    #[inline(always)]
    #[must_use]
    pub const fn is_inline(self) -> bool {
        (self as u8) <= 0x07
    }

    /// Returns the length of the inline payload, if inline.
    #[inline(always)]
    #[must_use]
    pub const fn inline_len(self) -> Option<usize> {
        if self.is_inline() {
            Some(self as u8 as usize)
        } else {
            None
        }
    }
}

/// A 64-bit polymorphic value slot packed directly into an Expanse leaf node.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ValueSlot(pub u64);

impl ValueSlot {
    /// Mask for the 8-bit discriminant tag (`bits [7:0]`).
    pub const TAG_MASK: u64 = 0xFF;
    /// Mask for 24-bit arena byte offset (`0x00FF_FFFF`).
    pub const ARENA_OFFSET_MASK: u64 = 0x00FF_FFFF;
    /// Mask for 40-bit chunk byte offset (`0x00FF_FFFF_FFFF`).
    pub const ARENA_LONG_OFFSET_MASK: u64 = 0x00FF_FFFF_FFFF;
    /// Mask for 32-bit hot metadata word (`bits [63:32]`).
    pub const META_MASK: u64 = 0xFFFF_FFFF_0000_0000;

    /// Creates an inline value slot from a byte slice (length `<= 7`).
    ///
    /// Returns `None` if `bytes.len() > 7`.
    #[inline(always)]
    #[must_use]
    pub fn new_inline(bytes: &[u8]) -> Option<Self> {
        let len = bytes.len();
        if len > 7 {
            return None;
        }
        let mut raw = len as u64; // Tag is Inline0..Inline7
        for (i, &b) in bytes.iter().enumerate() {
            raw |= (b as u64) << (8 * (i + 1));
        }
        Some(Self(raw))
    }

    /// Creates an arena-backed value slot with 32-bit hot metadata and a 24-bit offset.
    ///
    /// Returns `None` if `arena_offset > 0x00FF_FFFF`.
    #[inline(always)]
    #[must_use]
    pub fn new_arena_short(hot_meta: u32, arena_offset: u32) -> Option<Self> {
        if arena_offset > (Self::ARENA_OFFSET_MASK as u32) {
            return None;
        }
        let raw =
            (SlotTag::ArenaShort as u64) | ((arena_offset as u64) << 8) | ((hot_meta as u64) << 32);
        Some(Self(raw))
    }

    /// Creates a large/multi-chunk arena value slot with a 16-bit chunk ID and 40-bit offset.
    ///
    /// Returns `None` if `chunk_offset > 0x00FF_FFFF_FFFF`.
    #[inline(always)]
    #[must_use]
    pub fn new_arena_long(chunk_id: u16, chunk_offset: u64) -> Option<Self> {
        if chunk_offset > Self::ARENA_LONG_OFFSET_MASK {
            return None;
        }
        let raw = (SlotTag::ArenaLong as u64)
            | ((chunk_offset & Self::ARENA_LONG_OFFSET_MASK) << 8)
            | ((chunk_id as u64) << 48);
        Some(Self(raw))
    }

    /// Returns the slot tag discriminant.
    #[inline(always)]
    #[must_use]
    pub const fn tag(self) -> SlotTag {
        SlotTag::from_u8((self.0 & Self::TAG_MASK) as u8)
    }

    /// Extracts inline payload into a fixed 7-byte buffer and returns `(buffer, length)`.
    #[inline(always)]
    #[must_use]
    pub fn inline_payload(self) -> ([u8; 7], usize) {
        let len = (self.0 & Self::TAG_MASK) as usize;
        let effective_len = len.min(7);
        let mut buf = [0u8; 7];
        let val = self.0 >> 8;
        for (i, byte) in buf.iter_mut().enumerate().take(effective_len) {
            *byte = ((val >> (8 * i)) & 0xFF) as u8;
        }
        (buf, effective_len)
    }

    /// Extracts the 32-bit hot metadata word (`bits [63:32]`).
    #[inline(always)]
    #[must_use]
    pub const fn hot_meta(self) -> u32 {
        (self.0 >> 32) as u32
    }

    /// Returns a new slot with updated 32-bit hot metadata.
    #[inline(always)]
    #[must_use]
    pub const fn with_hot_meta(self, meta: u32) -> Self {
        let raw = (self.0 & 0x0000_0000_FFFF_FFFF) | ((meta as u64) << 32);
        Self(raw)
    }

    /// Extracts the 24-bit arena byte offset (`bits [31:8]`).
    #[inline(always)]
    #[must_use]
    pub const fn arena_offset(self) -> u32 {
        ((self.0 >> 8) & Self::ARENA_OFFSET_MASK) as u32
    }

    /// Returns a new slot with updated 24-bit arena offset.
    ///
    /// Returns `None` if `offset > 0x00FF_FFFF`.
    #[inline(always)]
    #[must_use]
    pub const fn with_arena_offset(self, offset: u32) -> Option<Self> {
        if offset > (Self::ARENA_OFFSET_MASK as u32) {
            return None;
        }
        let raw = (self.0 & !(Self::ARENA_OFFSET_MASK << 8)) | ((offset as u64) << 8);
        Some(Self(raw))
    }

    /// Extracts the 16-bit chunk ID and 40-bit chunk byte offset for an `ArenaLong` slot.
    #[inline(always)]
    #[must_use]
    pub const fn arena_long_loc(self) -> (u16, u64) {
        let chunk_id = (self.0 >> 48) as u16;
        let chunk_offset = (self.0 >> 8) & Self::ARENA_LONG_OFFSET_MASK;
        (chunk_id, chunk_offset)
    }

    /// Converts the slot to its raw 64-bit representation.
    #[inline(always)]
    #[must_use]
    pub const fn to_raw(self) -> u64 {
        self.0
    }

    /// Constructs a slot from an uninterpreted 64-bit word.
    #[inline(always)]
    #[must_use]
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }
}

impl From<u64> for ValueSlot {
    #[inline(always)]
    fn from(val: u64) -> Self {
        Self(val)
    }
}

impl From<ValueSlot> for u64 {
    #[inline(always)]
    fn from(slot: ValueSlot) -> Self {
        slot.0
    }
}

// ---------------------------------------------------------------------------
// Phase B: Columnar Predicate Filter Kernels
// ---------------------------------------------------------------------------

/// Filters a batch of raw 64-bit value slots (up to 32 slots) against a closed
/// range `[min_meta, max_meta]`, returning a bitmask where bit `i` is set iff
/// `slots[i]` has `min_meta <= hot_meta <= max_meta`.
#[inline]
#[must_use]
pub fn filter_slots_range(slots: &[u64], min_meta: u32, max_meta: u32) -> u32 {
    let count = slots.len().min(32);
    let mut mask = 0u32;
    for (i, &slot) in slots[..count].iter().enumerate() {
        let meta = (slot >> 32) as u32;
        if meta >= min_meta && meta <= max_meta {
            mask |= 1 << i;
        }
    }
    mask
}

/// Filters a batch of raw 64-bit value slots (up to 32 slots) with a custom
/// metadata predicate closure, returning a bitmask of matching slot indices.
#[inline]
pub fn filter_slots_predicate<P: FnMut(u32) -> bool>(slots: &[u64], mut predicate: P) -> u32 {
    let count = slots.len().min(32);
    let mut mask = 0u32;
    for (i, &slot) in slots[..count].iter().enumerate() {
        let meta = (slot >> 32) as u32;
        if predicate(meta) {
            mask |= 1 << i;
        }
    }
    mask
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_payload_roundtrip_all_sizes() {
        // Test 0..=7 byte slices
        for len in 0..=7 {
            let data: Vec<u8> = (0..len).map(|i| (i + 1) * 0x11).collect();
            let slot = ValueSlot::new_inline(&data).expect("len <= 7 must succeed");
            assert_eq!(slot.tag().inline_len(), Some(len as usize));
            assert!(slot.tag().is_inline());
            let (buf, extracted_len) = slot.inline_payload();
            assert_eq!(extracted_len, len as usize);
            assert_eq!(&buf[..extracted_len], &data[..]);
        }

        // 8 bytes must return None
        let data8 = [1, 2, 3, 4, 5, 6, 7, 8];
        assert!(ValueSlot::new_inline(&data8).is_none());
    }

    #[test]
    fn arena_short_roundtrip_and_mutations() {
        let meta = 0xDEAD_BEEF;
        let offset = 0x00A1_B2C3;
        let slot = ValueSlot::new_arena_short(meta, offset).expect("valid short offset");
        assert_eq!(slot.tag(), SlotTag::ArenaShort);
        assert_eq!(slot.hot_meta(), meta);
        assert_eq!(slot.arena_offset(), offset);

        // Update hot meta
        let updated_meta = slot.with_hot_meta(0x1234_5678);
        assert_eq!(updated_meta.hot_meta(), 0x1234_5678);
        assert_eq!(updated_meta.arena_offset(), offset);

        // Update offset
        let updated_offset = slot.with_arena_offset(0x0011_2233).expect("valid offset");
        assert_eq!(updated_offset.hot_meta(), meta);
        assert_eq!(updated_offset.arena_offset(), 0x0011_2233);

        // Offset overflow
        assert!(ValueSlot::new_arena_short(meta, 0x0100_0000).is_none());
        assert!(slot.with_arena_offset(0x0100_0000).is_none());
    }

    #[test]
    fn arena_long_roundtrip() {
        let chunk_id = 0x42AB;
        let chunk_offset = 0x00AB_CDEF_1234;
        let slot = ValueSlot::new_arena_long(chunk_id, chunk_offset).expect("valid long offset");
        assert_eq!(slot.tag(), SlotTag::ArenaLong);
        let (cid, coff) = slot.arena_long_loc();
        assert_eq!(cid, chunk_id);
        assert_eq!(coff, chunk_offset);

        // Offset overflow (> 40 bits)
        assert!(ValueSlot::new_arena_long(chunk_id, 0x0100_0000_0000).is_none());
    }

    #[test]
    fn raw_and_tombstone_tags() {
        let raw_val = 0x1234_5678_9ABC_DEF0;
        let slot = ValueSlot::from_raw(raw_val);
        assert_eq!(slot.to_raw(), raw_val);

        let tombstone = ValueSlot::from_raw(0x0000_0000_0000_00FE);
        assert_eq!(tombstone.tag(), SlotTag::Tombstone);
    }

    #[test]
    fn filter_kernels_match_expected() {
        let mut slots = Vec::new();
        for i in 0..16 {
            let meta = i * 10;
            let slot = ValueSlot::new_arena_short(meta, 100).unwrap();
            slots.push(slot.to_raw());
        }

        // Range filter: meta in 30..=80 -> indices 3, 4, 5, 6, 7, 8
        let mask = filter_slots_range(&slots, 30, 80);
        let mut matching_indices = Vec::new();
        for i in 0..16 {
            if (mask & (1 << i)) != 0 {
                matching_indices.push(i);
            }
        }
        assert_eq!(matching_indices, vec![3, 4, 5, 6, 7, 8]);

        // Custom predicate: even metadata
        let pred_mask = filter_slots_predicate(&slots, |meta| (meta / 10) % 2 == 0);
        let mut pred_indices = Vec::new();
        for i in 0..16 {
            if (pred_mask & (1 << i)) != 0 {
                pred_indices.push(i);
            }
        }
        assert_eq!(pred_indices, vec![0, 2, 4, 6, 8, 10, 12, 14]);
    }
}
