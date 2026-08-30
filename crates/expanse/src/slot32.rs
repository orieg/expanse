//! Polymorphic 32-Bit Value Slot (`ValueSlot32`).
//!
//! Provides inline payload packing (<= 3 bytes, 0 heap allocations),
//! columnar hot metadata tagging (12-bit meta + 12-bit arena offset),
//! and transparent raw 32-bit word drop-in C ABI compatibility.

use core::convert::From;
use core::default::Default;
use core::fmt;
use core::option::Option::{self, None, Some};

/// Discriminant tag for 32-bit value slots.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SlotTag32 {
    /// Inline payload with 0 bytes.
    Inline0 = 0x00,
    /// Inline payload with 1 byte.
    Inline1 = 0x01,
    /// Inline payload with 2 bytes.
    Inline2 = 0x02,
    /// Inline payload with 3 bytes.
    Inline3 = 0x03,
    /// Arena mode: 12-bit hot metadata + 12-bit slab offset (up to 4096 entries).
    Arena = 0x10,
    /// Raw uninterpreted 32-bit word (C ABI drop-in).
    RawWord = 0xFF,
}

impl From<u8> for SlotTag32 {
    #[inline(always)]
    fn from(byte: u8) -> Self {
        match byte {
            0x00 => SlotTag32::Inline0,
            0x01 => SlotTag32::Inline1,
            0x02 => SlotTag32::Inline2,
            0x03 => SlotTag32::Inline3,
            0x10 => SlotTag32::Arena,
            _ => SlotTag32::RawWord,
        }
    }
}

/// Transparent newtype over a 32-bit machine word representing a polymorphic slot.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct ValueSlot32(pub u32);

const _: () = {
    assert!(core::mem::size_of::<ValueSlot32>() == 4);
    assert!(core::mem::align_of::<ValueSlot32>() == 4);
};

impl ValueSlot32 {
    /// Discriminant tag mask (lowest 8 bits).
    pub const TAG_MASK: u32 = 0x0000_00FF;

    /// Arena offset mask (bits 8..19, 12 bits).
    pub const ARENA_OFFSET_MASK: u32 = 0x000F_FF00;
    /// Bit-shift for arena offset.
    pub const ARENA_OFFSET_SHIFT: u32 = 8;

    /// Arena hot metadata mask (bits 20..31, 12 bits or packed 16 bits).
    pub const ARENA_META_MASK: u32 = 0xFFF0_0000;
    /// Bit-shift for arena hot metadata.
    pub const ARENA_META_SHIFT: u32 = 20;

    /// Create a slot from a raw 32-bit machine integer.
    #[inline(always)]
    pub const fn from_raw(val: u32) -> Self {
        Self(val)
    }

    /// Extract the raw 32-bit integer.
    #[inline(always)]
    pub const fn to_raw(self) -> u32 {
        self.0
    }

    /// Extract the slot tag discriminant.
    #[inline(always)]
    pub fn tag(self) -> SlotTag32 {
        SlotTag32::from((self.0 & Self::TAG_MASK) as u8)
    }

    /// Pack an inline byte payload (0..=3 bytes) directly into the slot.
    #[inline(always)]
    pub fn new_inline(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > 3 {
            return None;
        }
        let tag = bytes.len() as u8;
        let mut raw = [0u8; 4];
        raw[0] = tag;
        let mut i = 0;
        while i < bytes.len() {
            raw[i + 1] = bytes[i];
            i += 1;
        }
        Some(Self(u32::from_le_bytes(raw)))
    }

    /// Unpack an inline payload from the slot.
    #[inline(always)]
    pub fn inline_payload(self) -> ([u8; 3], usize) {
        let bytes = self.0.to_le_bytes();
        let len = (bytes[0] & 0x03) as usize;
        let mut payload = [0u8; 3];
        let mut i = 0;
        while i < len {
            payload[i] = bytes[i + 1];
            i += 1;
        }
        (payload, len)
    }

    /// Pack an arena reference with 12-bit hot metadata and 12-bit slab offset.
    #[inline(always)]
    pub fn new_arena(hot_meta: u16, slab_offset: u16) -> Option<Self> {
        if slab_offset > 0x0FFF || hot_meta > 0x0FFF {
            return None;
        }
        let val = (SlotTag32::Arena as u32)
            | ((slab_offset as u32) << Self::ARENA_OFFSET_SHIFT)
            | ((hot_meta as u32) << Self::ARENA_META_SHIFT);
        Some(Self(val))
    }

    /// Extract the 12-bit slab offset in arena mode.
    #[inline(always)]
    pub const fn slab_offset(self) -> u16 {
        ((self.0 & Self::ARENA_OFFSET_MASK) >> Self::ARENA_OFFSET_SHIFT) as u16
    }

    /// Extract the 12-bit hot metadata in arena mode.
    #[inline(always)]
    pub const fn hot_meta(self) -> u16 {
        ((self.0 & Self::ARENA_META_MASK) >> Self::ARENA_META_SHIFT) as u16
    }

    /// Update the hot metadata while preserving tag and slab offset.
    #[inline(always)]
    pub fn with_hot_meta(self, hot_meta: u16) -> Self {
        debug_assert!(hot_meta <= 0x0FFF);
        let cleared = self.0 & !Self::ARENA_META_MASK;
        Self(cleared | ((hot_meta as u32) << Self::ARENA_META_SHIFT))
    }
}

impl fmt::Debug for ValueSlot32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.tag() {
            SlotTag32::Inline0 | SlotTag32::Inline1 | SlotTag32::Inline2 | SlotTag32::Inline3 => {
                let (payload, len) = self.inline_payload();
                f.debug_struct("ValueSlot32::Inline")
                    .field("len", &len)
                    .field("payload", &&payload[..len])
                    .finish()
            }
            SlotTag32::Arena => f
                .debug_struct("ValueSlot32::Arena")
                .field("hot_meta", &self.hot_meta())
                .field("slab_offset", &self.slab_offset())
                .finish(),
            SlotTag32::RawWord => f
                .debug_struct("ValueSlot32::Raw")
                .field("raw", &format_args!("0x{:08X}", self.0))
                .finish(),
        }
    }
}

/// Filter a slice of 32-bit value slots matching `min_meta <= hot_meta <= max_meta`.
#[inline]
pub fn filter_slots_range_32(slots: &[u32], min_meta: u16, max_meta: u16) -> u32 {
    let mut mask = 0u32;
    for (idx, &slot_raw) in slots.iter().enumerate().take(32) {
        let slot = ValueSlot32::from_raw(slot_raw);
        if slot.tag() == SlotTag32::Arena {
            let meta = slot.hot_meta();
            if meta >= min_meta && meta <= max_meta {
                mask |= 1 << idx;
            }
        }
    }
    mask
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_slot32_inline_all_lengths() {
        for len in 0..=3 {
            let sample: Vec<u8> = (0..len).map(|i| 0xAA + i as u8).collect();
            let slot = ValueSlot32::new_inline(&sample).expect("valid inline");
            assert_eq!(slot.tag() as u8, len as u8);
            let (payload, extracted_len) = slot.inline_payload();
            assert_eq!(extracted_len, len);
            assert_eq!(&payload[..extracted_len], &sample[..]);
        }
    }

    #[test]
    fn test_value_slot32_arena_roundtrip() {
        let slot = ValueSlot32::new_arena(0xABC, 0x123).expect("valid arena");
        assert_eq!(slot.tag(), SlotTag32::Arena);
        assert_eq!(slot.hot_meta(), 0xABC);
        assert_eq!(slot.slab_offset(), 0x123);

        let updated = slot.with_hot_meta(0x789);
        assert_eq!(updated.hot_meta(), 0x789);
        assert_eq!(updated.slab_offset(), 0x123);
    }

    #[test]
    fn test_filter_slots_range_32() {
        let s1 = ValueSlot32::new_arena(100, 1).unwrap().to_raw();
        let s2 = ValueSlot32::new_arena(250, 2).unwrap().to_raw();
        let s3 = ValueSlot32::new_arena(500, 3).unwrap().to_raw();
        let s4 = ValueSlot32::new_inline(b"abc").unwrap().to_raw();

        let slots = [s1, s2, s3, s4];
        let match_mask = filter_slots_range_32(&slots, 200, 600);
        assert_eq!(match_mask, (1 << 1) | (1 << 2));
    }
}
