//! 32-Bit Polymorphic Blob Map (`ExpanseBlobMap32`).
//!
//! Integrates `ValueSlot32` with embedded slab arenas for variable-length payload storage
//! and columnar hot metadata range filtering on 32-bit targets per `docs/RFC_32BIT_EMBEDDED.md`.
//!
//! ## Current-implementation limits
//!
//! - The slab offset packed into `ValueSlot32` is **12 bits**, so at most
//!   `0x0FFF` (4095) arena entries are addressable; `insert` returns
//!   [`BlobMap32Error::OffsetOverflow`] once the arena grows past that instead
//!   of truncating the offset (which previously silently corrupted lookups).
//! - Hot metadata is likewise 12 bits; a value `> 0x0FFF` on an arena payload
//!   returns [`BlobMap32Error::MetaOverflow`].
//! - **Every** payload, including `<= 3`-byte "inline" ones, currently occupies
//!   an arena `Vec<u8>` slot — inline storage is *not* zero-heap in this
//!   implementation; the tag records the length and the slot holds the arena
//!   index. (A future revision may store `<= 3`-byte payloads without a heap
//!   allocation.)
//! - `remove` and overwrite only drop the index entry; the arena entry they
//!   pointed to is **never reclaimed** (the arena grows monotonically until the
//!   map is dropped).

#[cfg(feature = "std")]
use std::vec::Vec;
#[cfg(not(feature = "std"))]
extern crate alloc as alloc_crate;
#[cfg(not(feature = "std"))]
use alloc_crate::vec::Vec;
use core::fmt;

use crate::map32::ExpanseMap32;
use crate::slot32::{SlotTag32, ValueSlot32};
use crate::types32::Key32;

/// Error returned by [`ExpanseBlobMap32::insert`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlobMap32Error {
    /// The arena grew beyond the 12-bit slab-offset field (> `0x0FFF` entries).
    OffsetOverflow,
    /// The hot metadata exceeded the 12-bit field (> `0x0FFF`).
    MetaOverflow,
}

impl fmt::Display for BlobMap32Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OffsetOverflow => write!(f, "arena slab offset overflow (> 0x0FFF)"),
            Self::MetaOverflow => write!(f, "hot metadata overflow (> 0x0FFF)"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for BlobMap32Error {}

/// View into a stored blob payload in `ExpanseBlobMap32`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BlobView32<'a> {
    /// A `<= 3`-byte payload tagged inline. Note: in the current
    /// implementation the bytes still live in the arena (not zero-heap); the
    /// tag only records that the payload is short.
    Inline(&'a [u8]),
    /// Arena payload stored in the arena slab.
    Arena(&'a [u8]),
}

impl<'a> BlobView32<'a> {
    /// Returns the byte slice of this payload view.
    #[inline(always)]
    pub fn as_bytes(&self) -> &'a [u8] {
        match self {
            BlobView32::Inline(slice) => slice,
            BlobView32::Arena(slice) => slice,
        }
    }

    /// Returns the length of the payload in bytes.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.as_bytes().len()
    }

    /// Check if the payload is empty.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<'a> fmt::Debug for BlobView32<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("BlobView32").field(&self.as_bytes()).finish()
    }
}

/// 32-bit large-value associative map with hot metadata pushdown.
pub struct ExpanseBlobMap32 {
    /// 32-bit trie index storing `ValueSlot32` as raw words.
    index: ExpanseMap32,
    /// Storage for arena payloads (> 3 bytes).
    arena: Vec<Vec<u8>>,
}

impl ExpanseBlobMap32 {
    /// Create a new empty 32-bit blob map.
    #[inline(always)]
    #[must_use]
    pub fn new() -> Self {
        Self {
            index: ExpanseMap32::new(),
            arena: Vec::new(),
        }
    }

    /// Insert a byte payload with 12-bit hot metadata into the map.
    ///
    /// # Errors
    ///
    /// Returns [`BlobMap32Error::OffsetOverflow`] if the arena has already
    /// grown to the 12-bit slab-offset ceiling (`0x0FFF` entries), or
    /// [`BlobMap32Error::MetaOverflow`] if `hot_meta > 0x0FFF` for an
    /// arena-backed (`> 3`-byte) payload. Both are validated *before* any
    /// truncation, so an out-of-range value can never silently corrupt a slot
    /// (this replaces a former `.expect()` panic on `offset as u16` overflow).
    pub fn insert(&mut self, key: Key32, data: &[u8], hot_meta: u16) -> Result<(), BlobMap32Error> {
        // Validate the arena index against the 12-bit slab-offset field BEFORE
        // truncating it to u16 — the arena is shared by inline and arena
        // payloads, so any entry past 0x0FFF would overflow the field.
        let offset = self.arena.len();
        if offset > 0x0FFF {
            return Err(BlobMap32Error::OffsetOverflow);
        }
        if data.len() > 3 && hot_meta > 0x0FFF {
            return Err(BlobMap32Error::MetaOverflow);
        }
        let offset = offset as u16;
        self.arena.push(data.to_vec());
        if data.len() <= 3 {
            let slot_val = (data.len() as u32) | ((offset as u32) << 8);
            self.index.insert(key, slot_val);
        } else {
            // Validated above; `ok_or` keeps this panic-free regardless.
            let slot =
                ValueSlot32::new_arena(hot_meta, offset).ok_or(BlobMap32Error::OffsetOverflow)?;
            self.index.insert(key, slot.to_raw());
        }
        Ok(())
    }

    /// Lookup a blob payload and its associated hot metadata.
    pub fn get<'a>(&'a self, key: Key32) -> Option<(BlobView32<'a>, u16)> {
        let raw = self.index.get(key)?;
        let slot = ValueSlot32::from_raw(raw);
        match slot.tag() {
            SlotTag32::Inline0 | SlotTag32::Inline1 | SlotTag32::Inline2 | SlotTag32::Inline3 => {
                let offset = (slot.to_raw() >> 8) as usize;
                if offset < self.arena.len() {
                    Some((BlobView32::Inline(&self.arena[offset]), 0))
                } else {
                    None
                }
            }
            SlotTag32::Arena => {
                let offset = slot.slab_offset() as usize;
                let meta = slot.hot_meta();
                if offset < self.arena.len() {
                    Some((BlobView32::Arena(&self.arena[offset]), meta))
                } else {
                    None
                }
            }
            SlotTag32::RawWord => None,
        }
    }

    /// Remove a blob entry by key.
    ///
    /// Only the index entry is dropped; the arena slot it referenced is **not**
    /// reclaimed (see the module-level note on monotonic arena growth).
    pub fn remove(&mut self, key: Key32) -> bool {
        self.index.remove(key).is_some()
    }

    /// Returns the number of live blob records.
    #[inline(always)]
    #[must_use]
    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// Check if the blob map is empty.
    #[inline(always)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Columnar metadata-filtered scan evaluating `pred(key, hot_meta)` before fetching payloads.
    pub fn scan_filtered<P, F>(&self, start: Key32, end: Key32, mut pred: P, mut cb: F)
    where
        P: FnMut(Key32, u16) -> bool,
        F: FnMut(Key32, BlobView32<'_>, u16),
    {
        self.index.scan_filtered(
            start,
            end,
            |k, raw| {
                let slot = ValueSlot32::from_raw(raw);
                let meta = if slot.tag() == SlotTag32::Arena {
                    slot.hot_meta()
                } else {
                    0
                };
                pred(k, meta)
            },
            |k, _raw| {
                if let Some((view, meta)) = self.get(k) {
                    cb(k, view, meta);
                }
            },
        );
    }
}

impl Default for ExpanseBlobMap32 {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blobmap32_inline_and_arena() {
        let mut map = ExpanseBlobMap32::new();

        // Inline payloads (<= 3 bytes)
        map.insert(1, b"a", 0).unwrap();
        map.insert(2, b"ab", 0).unwrap();
        map.insert(3, b"abc", 0).unwrap();

        // Arena payloads (> 3 bytes)
        map.insert(100, b"large payload in arena", 0xABC).unwrap();

        assert_eq!(map.len(), 4);

        let (v1, _) = map.get(1).unwrap();
        assert_eq!(v1.as_bytes(), b"a");

        let (v2, _) = map.get(2).unwrap();
        assert_eq!(v2.as_bytes(), b"ab");

        let (v3, _) = map.get(3).unwrap();
        assert_eq!(v3.as_bytes(), b"abc");

        let (v100, meta) = map.get(100).unwrap();
        assert_eq!(v100.as_bytes(), b"large payload in arena");
        assert_eq!(meta, 0xABC);
    }

    #[test]
    fn test_blobmap32_scan_filtered() {
        let mut map = ExpanseBlobMap32::new();
        map.insert(10, b"sensor-1", 100).unwrap();
        map.insert(20, b"sensor-2", 300).unwrap();
        map.insert(30, b"sensor-3", 500).unwrap();

        let mut matching_keys = Vec::new();
        map.scan_filtered(
            10,
            30,
            |_k, meta| meta >= 250,
            |k, view, meta| {
                matching_keys.push((k, view.as_bytes().to_vec(), meta));
            },
        );

        assert_eq!(matching_keys.len(), 2);
        assert_eq!(matching_keys[0].0, 20);
        assert_eq!(matching_keys[1].0, 30);
    }

    #[test]
    #[cfg_attr(miri, ignore = "4096-entry fill is slow under Miri")]
    fn insert_validates_before_truncation() {
        let mut map = ExpanseBlobMap32::new();
        // Out-of-range hot metadata on an arena payload is rejected, not truncated.
        assert_eq!(
            map.insert(1, b"arena payload here", 0x1000),
            Err(BlobMap32Error::MetaOverflow)
        );
        assert_eq!(map.len(), 0);

        // Filling past the 12-bit slab-offset ceiling returns OffsetOverflow
        // instead of panicking on an `offset as u16` truncation.
        for i in 0..=0x0FFFu32 {
            map.insert(i, b"x", 0).unwrap();
        }
        assert_eq!(
            map.insert(0x1000, b"x", 0),
            Err(BlobMap32Error::OffsetOverflow)
        );
    }
}
