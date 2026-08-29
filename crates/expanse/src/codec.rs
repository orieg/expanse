//! Zero-allocation lightweight in-register value compression codecs (#392).
//!
//! Provides SWAR and bit-packing codecs for multi-byte payloads that compress
//! into the 56 available payload bits of a 64-bit [`ValueSlot`].
//!
//! Codecs supported:
//! - **ZeroTrim8**: 8-byte integers with upper zero byte (`u64 < 2^56`).
//! - **Nibble4**: 8..=14 ASCII decimal digits (`'0'..='9'`) packed 4 bits per char.
//! - **Alnum6**: 8 or 9 ASCII alphanumeric characters (`[0-9A-Za-z_-]`) packed 6 bits per char.

use crate::slot::{SlotTag, ValueSlot};

/// Dictionary for 6-bit alphanumeric encoding (64 symbols).
pub const ALNUM6_LUT: [u8; 64] = [
    b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', // 0..=9
    b'A', b'B', b'C', b'D', b'E', b'F', b'G', b'H', b'I', b'J', // 10..=19
    b'K', b'L', b'M', b'N', b'O', b'P', b'Q', b'R', b'S', b'T', // 20..=29
    b'U', b'V', b'W', b'X', b'Y', b'Z', // 30..=35
    b'a', b'b', b'c', b'd', b'e', b'f', b'g', b'h', b'i', b'j', // 36..=45
    b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r', b's', b't', // 46..=55
    b'u', b'v', b'w', b'x', b'y', b'z', // 56..=61
    b'-', b'_', // 62..=63
];

/// Encodes an ASCII character into its 6-bit Alnum6 code.
#[inline(always)]
pub fn alnum6_encode_char(b: u8) -> Option<u64> {
    match b {
        b'0'..=b'9' => Some((b - b'0') as u64),
        b'A'..=b'Z' => Some((b - b'A' + 10) as u64),
        b'a'..=b'z' => Some((b - b'a' + 36) as u64),
        b'-' => Some(62),
        b'_' => Some(63),
        _ => None,
    }
}

/// Attempts to compress an 8-byte integer with upper zero byte (`u64 < 2^56`).
#[inline(always)]
pub fn compress_zero_trim_8(data: &[u8]) -> Option<ValueSlot> {
    if data.len() != 8 {
        return None;
    }
    let val = u64::from_le_bytes(data.try_into().ok()?);
    if val < (1 << 56) {
        let raw = (val << 8) | (SlotTag::CompressedZeroTrim8 as u64);
        Some(ValueSlot::from_raw(raw))
    } else {
        None
    }
}

/// Decompresses a ZeroTrim8 value slot into `out`.
#[inline(always)]
pub fn decompress_zero_trim_8(slot: ValueSlot, out: &mut [u8; 16]) -> usize {
    let val = slot.to_raw() >> 8;
    out[..8].copy_from_slice(&val.to_le_bytes());
    8
}

/// Attempts to compress 8..=14 ASCII decimal digits into a Nibble4 slot.
#[inline(always)]
pub fn compress_nibble_4(data: &[u8]) -> Option<ValueSlot> {
    let len = data.len();
    if !(8..=14).contains(&len) {
        return None;
    }
    let mut packed = 0u64;
    for (i, &b) in data.iter().enumerate() {
        if !b.is_ascii_digit() {
            return None;
        }
        let nibble = (b - b'0') as u64;
        packed |= nibble << (4 * i);
    }
    let tag_u8 = (SlotTag::CompressedNibble8 as u8) + (len - 8) as u8;
    let raw = (packed << 8) | (tag_u8 as u64);
    Some(ValueSlot::from_raw(raw))
}

/// Decompresses a Nibble4 value slot of the given `len` into `out`.
#[inline(always)]
pub fn decompress_nibble_4(slot: ValueSlot, len: usize, out: &mut [u8; 16]) -> usize {
    let packed = slot.to_raw() >> 8;
    for (i, target) in out.iter_mut().enumerate().take(len) {
        let nibble = ((packed >> (4 * i)) & 0x0F) as u8;
        *target = b'0' + nibble;
    }
    len
}

/// Attempts to compress an 8 or 9 character alphanumeric string into an Alnum6 slot.
#[inline(always)]
pub fn compress_alnum_6(data: &[u8]) -> Option<ValueSlot> {
    let len = data.len();
    if len != 8 && len != 9 {
        return None;
    }
    let mut packed = 0u64;
    for (i, &b) in data.iter().enumerate() {
        let code = alnum6_encode_char(b)?;
        packed |= code << (6 * i);
    }
    let tag = if len == 8 {
        SlotTag::CompressedAlnum8
    } else {
        SlotTag::CompressedAlnum9
    };
    let raw = (packed << 8) | (tag as u64);
    Some(ValueSlot::from_raw(raw))
}

/// Decompresses an Alnum6 value slot of the given `len` into `out`.
#[inline(always)]
pub fn decompress_alnum_6(slot: ValueSlot, len: usize, out: &mut [u8; 16]) -> usize {
    let packed = slot.to_raw() >> 8;
    for (i, target) in out.iter_mut().enumerate().take(len) {
        let code = ((packed >> (6 * i)) & 0x3F) as usize;
        *target = ALNUM6_LUT[code];
    }
    len
}

/// Master inlining compressor: attempts to compress a multi-byte payload into a single `ValueSlot`.
#[inline(always)]
pub fn try_compress_inline(data: &[u8]) -> Option<ValueSlot> {
    if let Some(slot) = compress_zero_trim_8(data) {
        return Some(slot);
    }
    if let Some(slot) = compress_nibble_4(data) {
        return Some(slot);
    }
    if let Some(slot) = compress_alnum_6(data) {
        return Some(slot);
    }
    None
}

/// Master inlining decompressor: decodes a compressed `ValueSlot` into `out`, returning decoded byte length.
#[inline(always)]
pub fn decompress_inline(slot: ValueSlot, out: &mut [u8; 16]) -> Option<usize> {
    let tag = slot.tag();
    match tag {
        SlotTag::CompressedZeroTrim8 => Some(decompress_zero_trim_8(slot, out)),
        SlotTag::CompressedNibble8 => Some(decompress_nibble_4(slot, 8, out)),
        SlotTag::CompressedNibble9 => Some(decompress_nibble_4(slot, 9, out)),
        SlotTag::CompressedNibble10 => Some(decompress_nibble_4(slot, 10, out)),
        SlotTag::CompressedNibble11 => Some(decompress_nibble_4(slot, 11, out)),
        SlotTag::CompressedNibble12 => Some(decompress_nibble_4(slot, 12, out)),
        SlotTag::CompressedNibble13 => Some(decompress_nibble_4(slot, 13, out)),
        SlotTag::CompressedNibble14 => Some(decompress_nibble_4(slot, 14, out)),
        SlotTag::CompressedAlnum8 => Some(decompress_alnum_6(slot, 8, out)),
        SlotTag::CompressedAlnum9 => Some(decompress_alnum_6(slot, 9, out)),
        _ => None,
    }
}
