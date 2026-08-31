//! 32-Bit Polymorphic Blob Map (`ExpanseBlobMap32`).
//!
//! Integrates `ValueSlot32` with embedded slab arenas for variable-length payload storage
//! and columnar hot metadata range filtering on 32-bit targets per `docs/design/32-bit-embedded.md`.
//!
//! ## Implementation characteristics
//!
//! - **Zero-heap inlining**: Payloads `<= 3` bytes are packed directly into `ValueSlot32`
//!   (in bits 31:8, with low byte storing `len`) with 0 heap allocations and 0 arena slots.
//! - **Freelist recycling**: Arena slab indices for `> 3`-byte payloads are recycled via an
//!   internal freelist upon removal and overwrite.
//! - **12-bit addressable slab**: At most `0x0FFF` (4095) live arena entries are addressable;
//!   `insert` returns [`BlobMap32Error::OffsetOverflow`] when all 4096 arena slots are exhausted.
//! - **12-bit hot metadata**: Columnar metadata `> 0x0FFF` returns [`BlobMap32Error::MetaOverflow`].

use core_alloc::vec::Vec;

use core::fmt;
use core::option::Option::{self, None, Some};

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
    /// A `<= 3`-byte payload stored directly inside `ValueSlot32` with zero heap allocation.
    Inline([u8; 3], u8),
    /// Arena payload stored in the arena slab.
    Arena(&'a [u8]),
}

impl<'a> BlobView32<'a> {
    /// Returns the byte slice of this payload view.
    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            BlobView32::Inline(buf, len) => &buf[..*len as usize],
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

    /// Check if the payload is stored inline in the value slot.
    #[inline(always)]
    pub fn is_inline(&self) -> bool {
        matches!(self, BlobView32::Inline(..))
    }

    /// Check if the payload is stored in the arena.
    #[inline(always)]
    pub fn is_arena(&self) -> bool {
        matches!(self, BlobView32::Arena(..))
    }
}

impl<'a> core::ops::Deref for BlobView32<'a> {
    type Target = [u8];
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.as_bytes()
    }
}

impl<'a> AsRef<[u8]> for BlobView32<'a> {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
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
    arena: Vec<Option<Vec<u8>>>,
    /// Freelist for recycling deallocated arena slab indices.
    freelist: Vec<u16>,
}

impl ExpanseBlobMap32 {
    /// Create a new empty 32-bit blob map.
    #[inline(always)]
    #[must_use]
    pub fn new() -> Self {
        Self {
            index: ExpanseMap32::new(),
            arena: Vec::new(),
            freelist: Vec::new(),
        }
    }

    fn alloc_arena_slot(&mut self, data: &[u8]) -> Result<u16, BlobMap32Error> {
        if let Some(free_idx) = self.freelist.pop() {
            self.arena[free_idx as usize] = Some(data.to_vec());
            Ok(free_idx)
        } else {
            let offset = self.arena.len();
            if offset > 0x0FFF {
                return Err(BlobMap32Error::OffsetOverflow);
            }
            self.arena.push(Some(data.to_vec()));
            Ok(offset as u16)
        }
    }

    /// Insert a byte payload with 12-bit hot metadata into the map.
    ///
    /// # Errors
    ///
    /// Returns [`BlobMap32Error::OffsetOverflow`] if the arena has already
    /// grown to the 12-bit slab-offset ceiling (`0x0FFF` entries), or
    /// [`BlobMap32Error::MetaOverflow`] if `hot_meta > 0x0FFF` for an
    /// arena-backed (`> 3`-byte) payload.
    pub fn insert(&mut self, key: Key32, data: &[u8], hot_meta: u16) -> Result<(), BlobMap32Error> {
        if data.len() <= 3 {
            // Zero-heap inlining into ValueSlot32
            let slot = ValueSlot32::new_inline(data).expect("data.len() <= 3");
            if let Some(old_raw) = self.index.insert(key, slot.to_raw()) {
                let old_slot = ValueSlot32::from_raw(old_raw);
                if old_slot.tag() == SlotTag32::Arena {
                    let offset = old_slot.slab_offset() as usize;
                    if offset < self.arena.len() {
                        self.arena[offset] = None;
                        self.freelist.push(offset as u16);
                    }
                }
            }
            Ok(())
        } else {
            if hot_meta > 0x0FFF {
                return Err(BlobMap32Error::MetaOverflow);
            }
            let old_raw = self.index.get(key);
            let offset = if let Some(raw) = old_raw {
                let old_slot = ValueSlot32::from_raw(raw);
                if old_slot.tag() == SlotTag32::Arena {
                    let off = old_slot.slab_offset() as usize;
                    if off < self.arena.len() {
                        self.arena[off] = Some(data.to_vec());
                        off as u16
                    } else {
                        self.alloc_arena_slot(data)?
                    }
                } else {
                    self.alloc_arena_slot(data)?
                }
            } else {
                self.alloc_arena_slot(data)?
            };
            let slot =
                ValueSlot32::new_arena(hot_meta, offset).ok_or(BlobMap32Error::OffsetOverflow)?;
            self.index.insert(key, slot.to_raw());
            Ok(())
        }
    }

    /// Lookup a blob payload and its associated hot metadata.
    pub fn get<'a>(&'a self, key: Key32) -> Option<(BlobView32<'a>, u16)> {
        let raw = self.index.get(key)?;
        let slot = ValueSlot32::from_raw(raw);
        match slot.tag() {
            SlotTag32::Inline0 | SlotTag32::Inline1 | SlotTag32::Inline2 | SlotTag32::Inline3 => {
                let (payload, len) = slot.inline_payload();
                Some((BlobView32::Inline(payload, len as u8), 0))
            }
            SlotTag32::Arena => {
                let offset = slot.slab_offset() as usize;
                let meta = slot.hot_meta();
                if let Some(Some(vec)) = self.arena.get(offset) {
                    Some((BlobView32::Arena(vec.as_slice()), meta))
                } else {
                    None
                }
            }
            SlotTag32::RawWord => None,
        }
    }

    /// Remove a blob entry by key, recycling any arena slot back to the freelist.
    pub fn remove(&mut self, key: Key32) -> bool {
        if let Some(old_raw) = self.index.remove(key) {
            let old_slot = ValueSlot32::from_raw(old_raw);
            if old_slot.tag() == SlotTag32::Arena {
                let offset = old_slot.slab_offset() as usize;
                if offset < self.arena.len() {
                    self.arena[offset] = None;
                    self.freelist.push(offset as u16);
                }
            }
            true
        } else {
            false
        }
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
            |k, raw| {
                let slot = ValueSlot32::from_raw(raw);
                match slot.tag() {
                    SlotTag32::Inline0
                    | SlotTag32::Inline1
                    | SlotTag32::Inline2
                    | SlotTag32::Inline3 => {
                        let (payload, len) = slot.inline_payload();
                        cb(k, BlobView32::Inline(payload, len as u8), 0);
                    }
                    SlotTag32::Arena => {
                        let offset = slot.slab_offset() as usize;
                        let meta = slot.hot_meta();
                        if let Some(Some(vec)) = self.arena.get(offset) {
                            cb(k, BlobView32::Arena(vec.as_slice()), meta);
                        }
                    }
                    SlotTag32::RawWord => {}
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

        // Inline payloads (<= 3 bytes) - 0 arena allocations
        map.insert(1, b"a", 0).unwrap();
        map.insert(2, b"ab", 0).unwrap();
        map.insert(3, b"abc", 0).unwrap();
        assert_eq!(
            map.arena.len(),
            0,
            "inline payloads must not allocate in arena"
        );

        // Arena payloads (> 3 bytes)
        map.insert(100, b"large payload in arena", 0xABC).unwrap();
        assert_eq!(map.arena.len(), 1);

        assert_eq!(map.len(), 4);

        let (v1, _) = map.get(1).unwrap();
        assert_eq!(v1.as_bytes(), b"a");
        assert!(v1.is_inline());

        let (v2, _) = map.get(2).unwrap();
        assert_eq!(v2.as_bytes(), b"ab");
        assert!(v2.is_inline());

        let (v3, _) = map.get(3).unwrap();
        assert_eq!(v3.as_bytes(), b"abc");
        assert!(v3.is_inline());

        let (v100, meta) = map.get(100).unwrap();
        assert_eq!(v100.as_bytes(), b"large payload in arena");
        assert!(v100.is_arena());
        assert_eq!(meta, 0xABC);
    }

    #[test]
    fn test_blobmap32_freelist_recycling() {
        let mut map = ExpanseBlobMap32::new();

        // Insert 3 arena payloads
        map.insert(10, b"payload-1", 10).unwrap();
        map.insert(20, b"payload-2", 20).unwrap();
        map.insert(30, b"payload-3", 30).unwrap();
        assert_eq!(map.arena.len(), 3);
        assert_eq!(map.freelist.len(), 0);

        // Remove key 20
        assert!(map.remove(20));
        assert_eq!(map.freelist.len(), 1);
        assert_eq!(map.get(20), None);

        // Insert key 40 (should reuse slot 1)
        map.insert(40, b"payload-4", 40).unwrap();
        assert_eq!(
            map.arena.len(),
            3,
            "freelist recycling should not expand arena"
        );
        assert_eq!(map.freelist.len(), 0);

        let (v40, meta40) = map.get(40).unwrap();
        assert_eq!(v40.as_bytes(), b"payload-4");
        assert_eq!(meta40, 40);
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
        for i in 0..=0x0FFFu32 {
            map.insert(i, b"arena-payload", 0).unwrap();
        }
        assert_eq!(
            map.insert(0x1000, b"arena-payload", 0),
            Err(BlobMap32Error::OffsetOverflow)
        );

        // But inline payloads can still be inserted without arena slots!
        assert!(map.insert(0x2000, b"abc", 0).is_ok());
    }
}
