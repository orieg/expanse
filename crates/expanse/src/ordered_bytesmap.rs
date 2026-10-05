//! `ExpanseOrderedBytesMap`: an ordered digital trie map from arbitrary
//! byte sequences (`&[u8]`) to `u64` values (Refs #808, `docs/ARCHITECTURE.md` §3.7).
//!
//! # Key Domain & Order Preservation
//!
//! Unlike [`crate::strmap::ExpanseStrMap`], which operates strictly on C-style
//! NUL-free byte strings ([`crate::strmap::NulFreeStr`]), `ExpanseOrderedBytesMap`
//! supports arbitrary binary keys of any length, including keys containing embedded
//! NUL (`0x00`) and `0x01` bytes (such as raw 16-byte binary UUIDs, serialized
//! protobuf payloads, and composite keys).
//!
//! Keys are encoded transparently using an order-preserving, prefix-free escape
//! transformation (first specified in `docs/ARCHITECTURE.md` §3.7.2):
//! - `0x00 -> [0x01, 0x01]`
//! - `0x01 -> [0x01, 0x02]`
//! - `b    -> [b]` for `b in 0x02..=0xFF`
//!
//! "Prefix-free" is a property of the byte code, as §3.7.2 defines it: no
//! byte's encoding is a prefix of another's, so an encoded key decodes one
//! way. It is not a property of whole keys. The encoding of `a` is a prefix
//! of the encoding of `ab`, exactly as `a` is a prefix of `ab`, which is what
//! keeps a key ordered before its extensions.
//!
//! # Mathematical and Structural Properties
//!
//! 1. **Order preservation**: For all byte sequences $A$ and $B$,
//!    $A <_{\text{lex}} B \iff \text{escape}(A) <_{\text{lex}} \text{escape}(B)$.
//! 2. **Clean-key fast path**: Keys containing neither `0x00` nor `0x01` pay only
//!    a single scan `!key.iter().any(|&b| b <= 1)` and proceed with **zero heap
//!    allocation**.
//! 3. **Stack-buffered short keys**: Keys containing `0x00` or `0x01` with length
//!    $\le 32$ bytes are encoded into a stack buffer, also with zero heap allocation.
//! 4. **No ring buffers (AGENTS §2.4)**: Decoded keys are yielded as owned `Vec<u8>`
//!    in owned iterators or decoded directly into caller-provided buffers via
//!    `*_decode_into` methods.

pub use crate::domain::EscapeDecodeError;
use crate::domain::{escape_decode, escape_decode_in_place, escape_decode_into, escape_encode};
use crate::strmap::{ExpanseStrMap, NulFreeStr, StrCursor};
#[cfg(all(target_pointer_width = "64", feature = "std"))]
pub use crate::sync::SyncExpanseOrderedBytesMap;
use core::ptr::NonNull;
use core_alloc::vec::Vec;

/// Encodes `data` into `buf`, returning the number of encoded bytes written.
///
/// Every byte expands to at most 2 bytes ($0x00 \to [1, 1]$, $0x01 \to [1, 2]$),
/// so `buf` must have capacity $\ge 2 \times \text{data.len()}$.
#[inline]
pub(crate) fn encode_into_slice(data: &[u8], buf: &mut [u8]) -> usize {
    let mut w = 0;
    for &b in data {
        match b {
            0 => {
                buf[w] = 1;
                buf[w + 1] = 1;
                w += 2;
            }
            1 => {
                buf[w] = 1;
                buf[w + 1] = 2;
                w += 2;
            }
            _ => {
                buf[w] = b;
                w += 1;
            }
        }
    }
    w
}

/// Executes closure `f` with an encoded [`NulFreeStr`] view of `key`.
///
/// - **Clean key**: if `key` contains no byte $\le 1$, borrows `key` directly with zero allocation.
/// - **Short key ($\le 32$ B)**: encodes into a stack buffer `[u8; 64]` with zero heap allocation.
/// - **Long key**: falls back to [`escape_encode`].
#[inline]
pub(crate) fn with_encoded_key<R>(key: &[u8], f: impl FnOnce(&NulFreeStr) -> R) -> R {
    if !key.iter().any(|&b| b <= 1) {
        debug_assert!(!key.contains(&0));
        // SAFETY:
        // - Invariant: clean path checks `!key.iter().any(|&b| b <= 1)`.
        // - Since no byte is <= 1, no byte in `key` is 0x00 (NUL).
        // - Therefore `key` is strictly NUL-free and valid for `NulFreeStr`.
        let nul_free = unsafe { NulFreeStr::new_unchecked(key) };
        f(nul_free)
    } else if key.len() <= 32 {
        let mut buf = [0u8; 64];
        let len = encode_into_slice(key, &mut buf);
        let slice = &buf[..len];
        debug_assert!(!slice.contains(&0));
        // SAFETY:
        // - Invariant: `encode_into_slice` (lines 44-63) maps 0x00 -> [0x01, 0x01],
        //   0x01 -> [0x01, 0x02], and preserves bytes in 0x02..=0xFF.
        // - Every byte written to `buf` is >= 1, so `slice` contains no 0x00 (NUL).
        // - Capacity is sufficient: len <= 2 * key.len() <= 64 for key.len() <= 32.
        let nul_free = unsafe { NulFreeStr::new_unchecked(slice) };
        f(nul_free)
    } else {
        let enc = escape_encode(key);
        debug_assert!(!enc.contains(&0));
        // SAFETY:
        // - Invariant: `escape_encode` (crates/expanse/src/domain.rs:188-202) maps
        //   0x00 -> [0x01, 0x01], 0x01 -> [0x01, 0x02], and preserves bytes in 0x02..=0xFF.
        // - The resulting encoded `Vec<u8>` is strictly NUL-free by construction.
        let nul_free = unsafe { NulFreeStr::new_unchecked(&enc) };
        f(nul_free)
    }
}

/// An ordered map from arbitrary byte sequences (`&[u8]`) to `u64` values.
///
/// Marked `#[repr(transparent)]` over [`ExpanseStrMap`] so [`SyncExpanseOrderedBytesMap::with_locked`]
/// can soundly cast `&ExpanseStrMap` to `&ExpanseOrderedBytesMap` inside its exclusive lock closure
/// without allocation.
#[derive(Default)]
#[repr(transparent)]
pub struct ExpanseOrderedBytesMap {
    pub(crate) inner: ExpanseStrMap,
}

impl ExpanseOrderedBytesMap {
    /// Borrows an [`ExpanseStrMap`] reference as an [`ExpanseOrderedBytesMap`] reference.
    #[cfg(feature = "std")]
    #[inline]
    #[must_use]
    pub(crate) fn from_ref(inner: &ExpanseStrMap) -> &Self {
        // SAFETY: ExpanseOrderedBytesMap is #[repr(transparent)] over ExpanseStrMap.
        unsafe { &*(inner as *const ExpanseStrMap as *const Self) }
    }
    /// Creates an empty map.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: ExpanseStrMap::new(),
        }
    }

    /// Consumes the wrapper and returns the underlying [`ExpanseStrMap`].
    #[must_use]
    pub fn into_inner(self) -> ExpanseStrMap {
        self.inner
    }

    /// Returns a reference to the underlying [`ExpanseStrMap`].
    #[must_use]
    pub fn as_inner(&self) -> &ExpanseStrMap {
        &self.inner
    }

    /// Number of entries stored.
    #[inline]
    #[must_use]
    pub fn len(&self) -> u64 {
        self.inner.len()
    }

    /// True when no entries are stored.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Heap bytes used by the underlying digital trie.
    #[inline]
    #[must_use]
    pub fn mem_used(&self) -> usize {
        self.inner.mem_used()
    }

    /// Heap bytes held from the system allocator, including slab pages and freelists.
    #[inline]
    #[must_use]
    pub fn mem_held(&self) -> usize {
        self.inner.mem_held()
    }

    /// Returns unused held memory back to the system allocator.
    #[inline]
    pub fn shrink_to_fit(&mut self) -> usize {
        self.inner.shrink_to_fit()
    }

    /// Removes every entry, returning the heap bytes released.
    #[inline]
    pub fn clear(&mut self) -> u64 {
        self.inner.clear()
    }

    /// Inserts `key → val`; returns the previous value if `key` was present.
    pub fn insert(&mut self, key: &[u8], val: u64) -> Option<u64> {
        with_encoded_key(key, |nul_free| self.inner.insert(nul_free, val))
    }

    /// Returns the value stored for `key`, or `None` if absent.
    #[must_use]
    pub fn get(&self, key: &[u8]) -> Option<u64> {
        with_encoded_key(key, |nul_free| self.inner.get(nul_free))
    }

    /// Returns `true` if `key` is present in the map.
    #[inline]
    #[must_use]
    pub fn contains_key(&self, key: &[u8]) -> bool {
        with_encoded_key(key, |nul_free| self.inner.contains_key(nul_free))
    }

    /// Convenience alias for [`Self::contains_key`].
    #[inline]
    #[must_use]
    pub fn contains(&self, key: &[u8]) -> bool {
        self.contains_key(key)
    }

    /// Removes `key`; returns its value if present.
    pub fn remove(&mut self, key: &[u8]) -> Option<u64> {
        with_encoded_key(key, |nul_free| self.inner.remove(nul_free))
    }

    /// Returns a writable pointer to `key`'s value slot, or `None` if absent.
    ///
    /// Valid until the next structural mutation of the map.
    #[must_use]
    pub fn get_slot_ptr(&self, key: &[u8]) -> Option<NonNull<u64>> {
        with_encoded_key(key, |nul_free| self.inner.get_slot_ptr(nul_free))
    }

    /// Returns a writable pointer to `key`'s value slot, or `None` if absent.
    /// Takes `&mut self`.
    #[must_use]
    pub fn get_value_slot(&mut self, key: &[u8]) -> Option<NonNull<u64>> {
        with_encoded_key(key, |nul_free| self.inner.get_value_slot(nul_free))
    }

    /// Inserts `key` with value 0 if absent (existing value preserved) and
    /// returns a writable pointer to its value slot.
    ///
    /// Valid until the next structural mutation of the map.
    pub fn ins_slot(&mut self, key: &[u8]) -> NonNull<u64> {
        with_encoded_key(key, |nul_free| self.inner.ins_slot(nul_free))
    }

    // -----------------------------------------------------------------------
    // Ordered Navigation
    // -----------------------------------------------------------------------

    /// Smallest entry in byte-lexicographical order: `(key, value_slot)`.
    pub fn first(&self) -> Option<(Vec<u8>, NonNull<u64>)> {
        let (mut k, slot) = self.inner.first()?;
        let len = escape_decode_in_place(&mut k).expect("valid escaped key in trie");
        k.truncate(len);
        Some((k, slot))
    }

    /// Largest entry in byte-lexicographical order: `(key, value_slot)`.
    pub fn last(&self) -> Option<(Vec<u8>, NonNull<u64>)> {
        let (mut k, slot) = self.inner.last()?;
        let len = escape_decode_in_place(&mut k).expect("valid escaped key in trie");
        k.truncate(len);
        Some((k, slot))
    }

    /// Smallest entry with key `>= key`: `(key, value_slot)`.
    pub fn next_at_or_after(&self, key: &[u8]) -> Option<(Vec<u8>, NonNull<u64>)> {
        let (mut k, slot) =
            with_encoded_key(key, |nul_free| self.inner.next_at_or_after(nul_free))?;
        let len = escape_decode_in_place(&mut k).expect("valid escaped key in trie");
        k.truncate(len);
        Some((k, slot))
    }

    /// Smallest entry with key `> key`: `(key, value_slot)`.
    pub fn next_after(&self, key: &[u8]) -> Option<(Vec<u8>, NonNull<u64>)> {
        let (mut k, slot) = with_encoded_key(key, |nul_free| self.inner.next_after(nul_free))?;
        let len = escape_decode_in_place(&mut k).expect("valid escaped key in trie");
        k.truncate(len);
        Some((k, slot))
    }

    /// Largest entry with key `<= key`: `(key, value_slot)`.
    pub fn prev_at_or_before(&self, key: &[u8]) -> Option<(Vec<u8>, NonNull<u64>)> {
        let (mut k, slot) =
            with_encoded_key(key, |nul_free| self.inner.prev_at_or_before(nul_free))?;
        let len = escape_decode_in_place(&mut k).expect("valid escaped key in trie");
        k.truncate(len);
        Some((k, slot))
    }

    /// Largest entry with key `< key`: `(key, value_slot)`.
    pub fn prev_before(&self, key: &[u8]) -> Option<(Vec<u8>, NonNull<u64>)> {
        let (mut k, slot) = with_encoded_key(key, |nul_free| self.inner.prev_before(nul_free))?;
        let len = escape_decode_in_place(&mut k).expect("valid escaped key in trie");
        k.truncate(len);
        Some((k, slot))
    }

    /// Smallest entry: `(key, value)`.
    #[inline]
    pub fn first_entry(&self) -> Option<(Vec<u8>, u64)> {
        self.first().map(|(k, slot)| {
            // SAFETY:
            // - The map holds a shared borrow (&self), preventing concurrent mutation.
            // - The slot pointer is a valid, aligned, initialized u64 returned by the inner map.
            let val = unsafe { slot.as_ptr().read() };
            (k, val)
        })
    }

    /// Largest entry: `(key, value)`.
    #[inline]
    pub fn last_entry(&self) -> Option<(Vec<u8>, u64)> {
        self.last().map(|(k, slot)| {
            // SAFETY:
            // - The map holds a shared borrow (&self), preventing concurrent mutation.
            // - The slot pointer is a valid, aligned, initialized u64 returned by the inner map.
            let val = unsafe { slot.as_ptr().read() };
            (k, val)
        })
    }

    /// Smallest entry with key `>= key`: `(key, value)`.
    #[inline]
    pub fn next_at_or_after_entry(&self, key: &[u8]) -> Option<(Vec<u8>, u64)> {
        self.next_at_or_after(key).map(|(k, slot)| {
            // SAFETY:
            // - The map holds a shared borrow (&self), preventing concurrent mutation.
            // - The slot pointer is a valid, aligned, initialized u64 returned by the inner map.
            let val = unsafe { slot.as_ptr().read() };
            (k, val)
        })
    }

    /// Smallest entry with key `> key`: `(key, value)`.
    #[inline]
    pub fn next_after_entry(&self, key: &[u8]) -> Option<(Vec<u8>, u64)> {
        self.next_after(key).map(|(k, slot)| {
            // SAFETY:
            // - The map holds a shared borrow (&self), preventing concurrent mutation.
            // - The slot pointer is a valid, aligned, initialized u64 returned by the inner map.
            let val = unsafe { slot.as_ptr().read() };
            (k, val)
        })
    }

    /// Largest entry with key `<= key`: `(key, value)`.
    #[inline]
    pub fn prev_at_or_before_entry(&self, key: &[u8]) -> Option<(Vec<u8>, u64)> {
        self.prev_at_or_before(key).map(|(k, slot)| {
            // SAFETY:
            // - The map holds a shared borrow (&self), preventing concurrent mutation.
            // - The slot pointer is a valid, aligned, initialized u64 returned by the inner map.
            let val = unsafe { slot.as_ptr().read() };
            (k, val)
        })
    }

    /// Largest entry with key `< key`: `(key, value)`.
    #[inline]
    pub fn prev_before_entry(&self, key: &[u8]) -> Option<(Vec<u8>, u64)> {
        self.prev_before(key).map(|(k, slot)| {
            // SAFETY:
            // - The map holds a shared borrow (&self), preventing concurrent mutation.
            // - The slot pointer is a valid, aligned, initialized u64 returned by the inner map.
            let val = unsafe { slot.as_ptr().read() };
            (k, val)
        })
    }

    // -----------------------------------------------------------------------
    // Caller-buffer `*_decode_into` APIs
    // -----------------------------------------------------------------------

    /// Decodes the smallest entry's key into `buf`.
    ///
    /// Returns:
    /// - `Ok(Some((decoded_len, slot)))` on success.
    /// - `Ok(None)` if the map is empty.
    /// - `Err(EscapeDecodeError::BufferTooSmall)` if `buf` is too small.
    pub fn first_decode_into(
        &self,
        buf: &mut [u8],
    ) -> Result<Option<(usize, NonNull<u64>)>, EscapeDecodeError> {
        let Some((k, slot)) = self.inner.first() else {
            return Ok(None);
        };
        let len = escape_decode_into(&k, buf)?;
        Ok(Some((len, slot)))
    }

    /// Decodes the largest entry's key into `buf`.
    ///
    /// Returns:
    /// - `Ok(Some((decoded_len, slot)))` on success.
    /// - `Ok(None)` if the map is empty.
    /// - `Err(EscapeDecodeError::BufferTooSmall)` if `buf` is too small.
    pub fn last_decode_into(
        &self,
        buf: &mut [u8],
    ) -> Result<Option<(usize, NonNull<u64>)>, EscapeDecodeError> {
        let Some((k, slot)) = self.inner.last() else {
            return Ok(None);
        };
        let len = escape_decode_into(&k, buf)?;
        Ok(Some((len, slot)))
    }

    /// Decodes the key of the smallest entry `>= key` into `buf`.
    pub fn next_at_or_after_decode_into(
        &self,
        key: &[u8],
        buf: &mut [u8],
    ) -> Result<Option<(usize, NonNull<u64>)>, EscapeDecodeError> {
        let Some((k, slot)) =
            with_encoded_key(key, |nul_free| self.inner.next_at_or_after(nul_free))
        else {
            return Ok(None);
        };
        let len = escape_decode_into(&k, buf)?;
        Ok(Some((len, slot)))
    }

    /// Decodes the key of the smallest entry `> key` into `buf`.
    pub fn next_after_decode_into(
        &self,
        key: &[u8],
        buf: &mut [u8],
    ) -> Result<Option<(usize, NonNull<u64>)>, EscapeDecodeError> {
        let Some((k, slot)) = with_encoded_key(key, |nul_free| self.inner.next_after(nul_free))
        else {
            return Ok(None);
        };
        let len = escape_decode_into(&k, buf)?;
        Ok(Some((len, slot)))
    }

    /// Decodes the key of the largest entry `<= key` into `buf`.
    pub fn prev_at_or_before_decode_into(
        &self,
        key: &[u8],
        buf: &mut [u8],
    ) -> Result<Option<(usize, NonNull<u64>)>, EscapeDecodeError> {
        let Some((k, slot)) =
            with_encoded_key(key, |nul_free| self.inner.prev_at_or_before(nul_free))
        else {
            return Ok(None);
        };
        let len = escape_decode_into(&k, buf)?;
        Ok(Some((len, slot)))
    }

    /// Decodes the key of the largest entry `< key` into `buf`.
    pub fn prev_before_decode_into(
        &self,
        key: &[u8],
        buf: &mut [u8],
    ) -> Result<Option<(usize, NonNull<u64>)>, EscapeDecodeError> {
        let Some((k, slot)) = with_encoded_key(key, |nul_free| self.inner.prev_before(nul_free))
        else {
            return Ok(None);
        };
        let len = escape_decode_into(&k, buf)?;
        Ok(Some((len, slot)))
    }

    /// Decodes the smallest entry's key into `buf` and reads its value.
    pub fn first_entry_decode_into(
        &self,
        buf: &mut [u8],
    ) -> Result<Option<(usize, u64)>, EscapeDecodeError> {
        let Some((len, slot)) = self.first_decode_into(buf)? else {
            return Ok(None);
        };
        // SAFETY:
        // - The map holds a shared borrow (&self), preventing concurrent mutation.
        // - The slot pointer is a valid, aligned, initialized u64 returned by the inner map.
        let val = unsafe { slot.as_ptr().read() };
        Ok(Some((len, val)))
    }

    /// Decodes the largest entry's key into `buf` and reads its value.
    pub fn last_entry_decode_into(
        &self,
        buf: &mut [u8],
    ) -> Result<Option<(usize, u64)>, EscapeDecodeError> {
        let Some((len, slot)) = self.last_decode_into(buf)? else {
            return Ok(None);
        };
        // SAFETY:
        // - The map holds a shared borrow (&self), preventing concurrent mutation.
        // - The slot pointer is a valid, aligned, initialized u64 returned by the inner map.
        let val = unsafe { slot.as_ptr().read() };
        Ok(Some((len, val)))
    }

    /// Decodes the key of the smallest entry `>= key` into `buf` and reads its value.
    pub fn next_at_or_after_entry_decode_into(
        &self,
        key: &[u8],
        buf: &mut [u8],
    ) -> Result<Option<(usize, u64)>, EscapeDecodeError> {
        let Some((len, slot)) = self.next_at_or_after_decode_into(key, buf)? else {
            return Ok(None);
        };
        // SAFETY:
        // - The map holds a shared borrow (&self), preventing concurrent mutation.
        // - The slot pointer is a valid, aligned, initialized u64 returned by the inner map.
        let val = unsafe { slot.as_ptr().read() };
        Ok(Some((len, val)))
    }

    /// Decodes the key of the smallest entry `> key` into `buf` and reads its value.
    pub fn next_after_entry_decode_into(
        &self,
        key: &[u8],
        buf: &mut [u8],
    ) -> Result<Option<(usize, u64)>, EscapeDecodeError> {
        let Some((len, slot)) = self.next_after_decode_into(key, buf)? else {
            return Ok(None);
        };
        // SAFETY:
        // - The map holds a shared borrow (&self), preventing concurrent mutation.
        // - The slot pointer is a valid, aligned, initialized u64 returned by the inner map.
        let val = unsafe { slot.as_ptr().read() };
        Ok(Some((len, val)))
    }

    /// Decodes the key of the largest entry `<= key` into `buf` and reads its value.
    pub fn prev_at_or_before_entry_decode_into(
        &self,
        key: &[u8],
        buf: &mut [u8],
    ) -> Result<Option<(usize, u64)>, EscapeDecodeError> {
        let Some((len, slot)) = self.prev_at_or_before_decode_into(key, buf)? else {
            return Ok(None);
        };
        // SAFETY:
        // - The map holds a shared borrow (&self), preventing concurrent mutation.
        // - The slot pointer is a valid, aligned, initialized u64 returned by the inner map.
        let val = unsafe { slot.as_ptr().read() };
        Ok(Some((len, val)))
    }

    /// Decodes the key of the largest entry `< key` into `buf` and reads its value.
    pub fn prev_before_entry_decode_into(
        &self,
        key: &[u8],
        buf: &mut [u8],
    ) -> Result<Option<(usize, u64)>, EscapeDecodeError> {
        let Some((len, slot)) = self.prev_before_decode_into(key, buf)? else {
            return Ok(None);
        };
        // SAFETY:
        // - The map holds a shared borrow (&self), preventing concurrent mutation.
        // - The slot pointer is a valid, aligned, initialized u64 returned by the inner map.
        let val = unsafe { slot.as_ptr().read() };
        Ok(Some((len, val)))
    }

    // -----------------------------------------------------------------------
    // Cursors & Iterators
    // -----------------------------------------------------------------------

    /// An ordered cursor from the smallest entry.
    #[must_use]
    pub fn cursor(&self) -> OrderedBytesCursor<'_> {
        OrderedBytesCursor {
            inner: self.inner.cursor(),
        }
    }

    /// An ordered cursor positioned at the smallest entry `>= key`.
    #[must_use]
    pub fn cursor_at_or_after(&self, key: &[u8]) -> OrderedBytesCursor<'_> {
        with_encoded_key(key, |nul_free| OrderedBytesCursor {
            inner: self.inner.cursor_at_or_after(nul_free),
        })
    }

    /// An iterator over all `(key, value)` entries in byte-lexicographical order.
    #[must_use]
    pub fn iter(&self) -> Iter<'_> {
        Iter {
            cursor: self.cursor(),
            remaining: self.len() as usize,
        }
    }
}

impl Clone for ExpanseOrderedBytesMap {
    fn clone(&self) -> Self {
        let mut copy = Self::new();
        for (k, v) in self.iter() {
            copy.insert(&k, v);
        }
        copy
    }
}

impl core::fmt::Debug for ExpanseOrderedBytesMap {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

// ---------------------------------------------------------------------------
// Cursor
// ---------------------------------------------------------------------------

/// An ordered cursor over [`ExpanseOrderedBytesMap`].
///
/// Provides zero-allocation streaming traversals via [`Self::next_decode_into`]
/// into caller buffers (AGENTS §2.4).
pub struct OrderedBytesCursor<'a> {
    inner: StrCursor<'a>,
}

impl<'a> OrderedBytesCursor<'a> {
    /// Advances to the next entry in byte-lexicographical order, returning the
    /// decoded arbitrary byte key and its `u64` value.
    pub fn next_owned(&mut self) -> Option<(Vec<u8>, u64)> {
        let (escaped, slot) = self.inner.next()?;
        // SAFETY:
        // - The cursor holds a shared borrow of the map, preventing concurrent mutation.
        // - The slot pointer is a valid, aligned, initialized u64 returned by the inner cursor.
        let val = unsafe { slot.as_ptr().read() };
        let decoded = escape_decode(escaped).expect("valid escaped key in trie");
        Some((decoded, val))
    }

    /// Advances to the next entry in byte-lexicographical order, decoding the key
    /// directly into `buf`.
    ///
    /// Returns:
    /// - `Ok(Some((decoded_len, slot)))` on success with writable slot pointer.
    /// - `Ok(None)` when the cursor is exhausted.
    /// - `Err(EscapeDecodeError::BufferTooSmall)` if `buf` is too small to hold the decoded key.
    pub fn next_decode_into(
        &mut self,
        buf: &mut [u8],
    ) -> Result<Option<(usize, NonNull<u64>)>, EscapeDecodeError> {
        let Some((escaped, slot)) = self.inner.next() else {
            return Ok(None);
        };
        let len = escape_decode_into(escaped, buf)?;
        Ok(Some((len, slot)))
    }

    /// Advances to the next entry in byte-lexicographical order, decoding the key
    /// directly into `buf` and reading the value.
    pub fn next_entry_decode_into(
        &mut self,
        buf: &mut [u8],
    ) -> Result<Option<(usize, u64)>, EscapeDecodeError> {
        let Some((escaped, slot)) = self.inner.next() else {
            return Ok(None);
        };
        let len = escape_decode_into(escaped, buf)?;
        // SAFETY:
        // - The cursor holds a shared borrow of the map, preventing concurrent mutation.
        // - The slot pointer is a valid, aligned, initialized u64 returned by the inner cursor.
        let val = unsafe { slot.as_ptr().read() };
        Ok(Some((len, val)))
    }
}

// ---------------------------------------------------------------------------
// Iterators
// ---------------------------------------------------------------------------

/// An owned iterator over entries of an [`ExpanseOrderedBytesMap`].
pub struct Iter<'a> {
    cursor: OrderedBytesCursor<'a>,
    remaining: usize,
}

impl<'a> Iterator for Iter<'a> {
    type Item = (Vec<u8>, u64);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let entry = self.cursor.next_owned()?;
        self.remaining = self.remaining.saturating_sub(1);
        Some(entry)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<'a> ExactSizeIterator for Iter<'a> {}

impl<'a> IntoIterator for &'a ExpanseOrderedBytesMap {
    type Item = (Vec<u8>, u64);
    type IntoIter = Iter<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// An owning iterator consuming an [`ExpanseOrderedBytesMap`].
pub struct IntoIter {
    items: core_alloc::vec::IntoIter<(Vec<u8>, u64)>,
}

impl Iterator for IntoIter {
    type Item = (Vec<u8>, u64);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.items.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.items.size_hint()
    }
}

impl ExactSizeIterator for IntoIter {}

impl DoubleEndedIterator for IntoIter {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.items.next_back()
    }
}

impl IntoIterator for ExpanseOrderedBytesMap {
    type Item = (Vec<u8>, u64);
    type IntoIter = IntoIter;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            items: self.iter().collect::<Vec<_>>().into_iter(),
        }
    }
}

impl FromIterator<(Vec<u8>, u64)> for ExpanseOrderedBytesMap {
    fn from_iter<I: IntoIterator<Item = (Vec<u8>, u64)>>(iter: I) -> Self {
        let mut map = Self::new();
        for (k, v) in iter {
            map.insert(&k, v);
        }
        map
    }
}

impl<'a> FromIterator<(&'a [u8], u64)> for ExpanseOrderedBytesMap {
    fn from_iter<I: IntoIterator<Item = (&'a [u8], u64)>>(iter: I) -> Self {
        let mut map = Self::new();
        for (k, v) in iter {
            map.insert(k, v);
        }
        map
    }
}

impl Extend<(Vec<u8>, u64)> for ExpanseOrderedBytesMap {
    fn extend<I: IntoIterator<Item = (Vec<u8>, u64)>>(&mut self, iter: I) {
        for (k, v) in iter {
            self.insert(&k, v);
        }
    }
}

impl<'a> Extend<(&'a [u8], u64)> for ExpanseOrderedBytesMap {
    fn extend<I: IntoIterator<Item = (&'a [u8], u64)>>(&mut self, iter: I) {
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}
