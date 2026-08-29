//! Polymorphic 64-bit value slots and columnar predicate filter kernels.
//!
//! Expanse stores a 64-bit word per key in its map leaves. The [`ValueSlot`]
//! abstraction allows packing small payloads (up to 7 bytes) directly inline
//! with zero heap allocation, or packing 24-bit hot metadata (TTL, flags,
//! timestamps) alongside a 32-bit arena locator into a single 64-bit word.
//!
//! # Bit Layouts
//!
//! 1. **Inline Mode** (`<= 7 bytes`):
//!    `[payload_byte_6 .. payload_byte_0 | tag (0x00..=0x07)]`
//!    Bits `[7:0]` encode the payload length (0 to 7 bytes).
//!
//! 2. **Arena Mode**:
//!    `[hot_meta (24 bits) | locator (32 bits) | tag (0x10)]`
//!    - Bits `[63:40]`: 24-bit hot metadata (filterable without cold DRAM fetches).
//!    - Bits `[39:8]`: 32-bit arena locator (address in 16-byte units, up to 64 GiB).
//!    - Bits `[7:0]`: `0x10` ([`SlotTag::ArenaMeta`]).
//!    - The sole arena encoding for [`crate::blobmap::ExpanseBlobMap`]
//!      (`CompactInSlot` layout, #282/#285): a uniform encoding that keeps
//!      24-bit hot metadata alongside the locator across the whole arena, so a
//!      metadata predicate can be evaluated in-slot without a cold-DRAM payload
//!      fetch. The locator resolves through the arena geometry (see
//!      [`crate::blobmap::BlobArena`]).
//!
//! 3. **Raw Scalar / Unmanaged Word**:
//!    Uninterpreted 64-bit machine word.

/// Discriminant tag indicating how a [`ValueSlot`] is formatted.
#[non_exhaustive]
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

    /// Backed by BlobArena with **both** 24-bit hot metadata **and** a 32-bit
    /// arena locator addressing 16-byte-aligned payload units: `[hot_meta (24
    /// bits) | locator (32 bits) | tag (0x10)]`.
    ///
    /// The sole arena encoding for [`crate::blobmap::ExpanseBlobMap`]
    /// (`CompactInSlot` layout, #282/#285): metadata is kept alongside the
    /// locator across the whole arena, so a metadata predicate is evaluable
    /// in-slot without a cold-DRAM payload fetch. The 32-bit locator addresses
    /// `2^32` 16-byte units = **64 GiB** of arena; its chunk/offset resolution
    /// is fixed by the arena geometry, not the slot. Metadata is capped at 24
    /// bits (16,777,216 states). See [`ValueSlot::new_arena_meta`].
    ArenaMeta = 0x10,
    /// Off-heap / External memory reference.
    ///
    /// **Reserved / not yet implemented.** No code path produces or consumes it.
    External = 0x12,

    /// 8-byte integer with upper zero byte compressed into 56 bits.
    CompressedZeroTrim8 = 0x20,
    /// 8-character 6-bit packed alphanumeric string.
    CompressedAlnum8 = 0x22,
    /// 9-character 6-bit packed alphanumeric string.
    CompressedAlnum9 = 0x23,
    /// 8-digit 4-bit packed numeric decimal string.
    CompressedNibble8 = 0x28,
    /// 9-digit 4-bit packed numeric decimal string.
    CompressedNibble9 = 0x29,
    /// 10-digit 4-bit packed numeric decimal string.
    CompressedNibble10 = 0x2A,
    /// 11-digit 4-bit packed numeric decimal string.
    CompressedNibble11 = 0x2B,
    /// 12-digit 4-bit packed numeric decimal string.
    CompressedNibble12 = 0x2C,
    /// 13-digit 4-bit packed numeric decimal string.
    CompressedNibble13 = 0x2D,
    /// 14-digit 4-bit packed numeric decimal string.
    CompressedNibble14 = 0x2E,

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
            0x10 => Self::ArenaMeta,
            0x12 => Self::External,
            0x20 => Self::CompressedZeroTrim8,
            0x22 => Self::CompressedAlnum8,
            0x23 => Self::CompressedAlnum9,
            0x28 => Self::CompressedNibble8,
            0x29 => Self::CompressedNibble9,
            0x2A => Self::CompressedNibble10,
            0x2B => Self::CompressedNibble11,
            0x2C => Self::CompressedNibble12,
            0x2D => Self::CompressedNibble13,
            0x2E => Self::CompressedNibble14,
            0xFE => Self::Tombstone,
            _ => Self::RawWord,
        }
    }

    /// Returns `true` if the tag represents an uncompressed raw inline byte payload (0..=7 bytes).
    #[inline(always)]
    #[must_use]
    pub const fn is_raw_inline(self) -> bool {
        (self as u8) <= 0x07
    }

    /// Returns `true` if the tag represents a compressed inline byte payload.
    #[inline(always)]
    #[must_use]
    pub const fn is_compressed_inline(self) -> bool {
        matches!(
            self,
            Self::CompressedZeroTrim8
                | Self::CompressedAlnum8
                | Self::CompressedAlnum9
                | Self::CompressedNibble8
                | Self::CompressedNibble9
                | Self::CompressedNibble10
                | Self::CompressedNibble11
                | Self::CompressedNibble12
                | Self::CompressedNibble13
                | Self::CompressedNibble14
        )
    }

    /// Returns `true` if the tag represents an inline byte payload (raw 0..=7 bytes or compressed).
    #[inline(always)]
    #[must_use]
    pub const fn is_inline(self) -> bool {
        self.is_raw_inline() || self.is_compressed_inline()
    }

    /// Returns the length of the inline payload, if raw uncompressed inline.
    #[inline(always)]
    #[must_use]
    pub const fn inline_len(self) -> Option<usize> {
        if self.is_raw_inline() {
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
    /// Mask for the 24-bit `ArenaMeta` hot metadata field (pre-shift, `0x00FF_FFFF`).
    pub const ARENA_META_MASK: u64 = 0x00FF_FFFF;
    /// Maximum `ArenaMeta` hot-metadata value (24-bit, `16_777_215`).
    pub const ARENA_META_MAX: u32 = 0x00FF_FFFF;

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

    /// Creates an `ArenaMeta` value slot carrying **both** 24-bit hot metadata
    /// and a 32-bit arena locator: `[hot_meta (24) | locator (32) | tag (8)]`.
    ///
    /// The `locator` is an arena address in 16-byte units (its chunk/offset
    /// split is defined by the arena geometry, not the slot), spanning up to
    /// `2^32 × 16 B = 64 GiB`. Returns `None` if `hot_meta` exceeds the 24-bit
    /// field ([`Self::ARENA_META_MAX`]).
    #[inline(always)]
    #[must_use]
    pub fn new_arena_meta(hot_meta: u32, locator: u32) -> Option<Self> {
        if hot_meta > Self::ARENA_META_MAX {
            return None;
        }
        let raw = (SlotTag::ArenaMeta as u64) | ((locator as u64) << 8) | ((hot_meta as u64) << 40);
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

    /// Extracts the 24-bit hot metadata of an `ArenaMeta` slot (`bits [63:40]`).
    #[inline(always)]
    #[must_use]
    pub const fn arena_meta_meta(self) -> u32 {
        ((self.0 >> 40) & Self::ARENA_META_MASK) as u32
    }

    /// Extracts the 32-bit arena locator of an `ArenaMeta` slot (`bits [39:8]`),
    /// an address in 16-byte units resolved by the arena geometry.
    #[inline(always)]
    #[must_use]
    pub const fn arena_meta_locator(self) -> u32 {
        (self.0 >> 8) as u32
    }

    /// Returns a new `ArenaMeta` slot with updated 24-bit hot metadata, or `None`
    /// if `meta` exceeds [`Self::ARENA_META_MAX`].
    #[inline(always)]
    #[must_use]
    pub const fn with_arena_meta_meta(self, meta: u32) -> Option<Self> {
        if meta > Self::ARENA_META_MAX {
            return None;
        }
        let raw = (self.0 & !(Self::ARENA_META_MASK << 40)) | ((meta as u64) << 40);
        Some(Self(raw))
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
///
/// Metadata is read from the `ArenaMeta` field (`bits [63:40]`, 24-bit);
/// non-`ArenaMeta` slots (inline / raw) carry no metadata and read as `0`.
#[inline]
#[must_use]
pub fn filter_slots_range(slots: &[u64], min_meta: u32, max_meta: u32) -> u32 {
    let count = slots.len().min(32);
    let mut mask = 0u32;
    for (i, &slot) in slots[..count].iter().enumerate() {
        let meta = ((slot >> 40) & ValueSlot::ARENA_META_MASK) as u32;
        if meta >= min_meta && meta <= max_meta {
            mask |= 1 << i;
        }
    }
    mask
}

/// Filters a batch of raw 64-bit value slots (up to 32 slots) with a custom
/// metadata predicate closure, returning a bitmask of matching slot indices.
///
/// Metadata is read from the `ArenaMeta` field (`bits [63:40]`, 24-bit).
#[inline]
pub fn filter_slots_predicate<P: FnMut(u32) -> bool>(slots: &[u64], mut predicate: P) -> u32 {
    let count = slots.len().min(32);
    let mut mask = 0u32;
    for (i, &slot) in slots[..count].iter().enumerate() {
        let meta = ((slot >> 40) & ValueSlot::ARENA_META_MASK) as u32;
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
    fn arena_meta_roundtrip_and_field_independence() {
        let meta = 0x00AB_CDEF & ValueSlot::ARENA_META_MAX; // 24-bit
        let locator = 0xDEAD_BEEFu32; // full 32-bit
        let slot = ValueSlot::new_arena_meta(meta, locator).expect("valid 24-bit meta");
        assert_eq!(slot.tag(), SlotTag::ArenaMeta);
        assert_eq!(slot.arena_meta_meta(), meta);
        assert_eq!(slot.arena_meta_locator(), locator);

        // Meta and locator occupy disjoint fields: changing one must not
        // disturb the other (regression guard against bit-field overlap).
        let updated = slot.with_arena_meta_meta(0x0012_3456).expect("valid meta");
        assert_eq!(updated.arena_meta_meta(), 0x0012_3456);
        assert_eq!(updated.arena_meta_locator(), locator);
        assert_eq!(updated.tag(), SlotTag::ArenaMeta);

        // Full-range boundary values in every field.
        let maxed = ValueSlot::new_arena_meta(ValueSlot::ARENA_META_MAX, u32::MAX).unwrap();
        assert_eq!(maxed.arena_meta_meta(), ValueSlot::ARENA_META_MAX);
        assert_eq!(maxed.arena_meta_locator(), u32::MAX);
        assert_eq!(maxed.tag(), SlotTag::ArenaMeta);

        let zeroed = ValueSlot::new_arena_meta(0, 0).unwrap();
        assert_eq!(zeroed.arena_meta_meta(), 0);
        assert_eq!(zeroed.arena_meta_locator(), 0);
        assert_eq!(zeroed.tag(), SlotTag::ArenaMeta);

        // 24-bit metadata envelope: anything above the field must be rejected,
        // never silently truncated.
        assert!(ValueSlot::new_arena_meta(ValueSlot::ARENA_META_MAX + 1, locator).is_none());
        assert!(ValueSlot::new_arena_meta(u32::MAX, 0).is_none());
        assert!(slot.with_arena_meta_meta(0x0100_0000).is_none());
    }

    #[test]
    fn arena_meta_tag_decodes() {
        assert_eq!(SlotTag::from_u8(0x10), SlotTag::ArenaMeta);
        assert!(!SlotTag::ArenaMeta.is_inline());
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
            let slot = ValueSlot::new_arena_meta(meta, 100).unwrap();
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

    #[test]
    fn compressed_tags_decode_and_classify() {
        let compressed_tags = [
            (0x20, SlotTag::CompressedZeroTrim8),
            (0x22, SlotTag::CompressedAlnum8),
            (0x23, SlotTag::CompressedAlnum9),
            (0x28, SlotTag::CompressedNibble8),
            (0x29, SlotTag::CompressedNibble9),
            (0x2A, SlotTag::CompressedNibble10),
            (0x2B, SlotTag::CompressedNibble11),
            (0x2C, SlotTag::CompressedNibble12),
            (0x2D, SlotTag::CompressedNibble13),
            (0x2E, SlotTag::CompressedNibble14),
        ];

        for (byte, expected_tag) in compressed_tags {
            let tag = SlotTag::from_u8(byte);
            assert_eq!(tag, expected_tag);
            assert!(tag.is_compressed_inline());
            assert!(tag.is_inline());
            assert!(!tag.is_raw_inline());
            assert_eq!(tag.inline_len(), None);
        }
    }
}
