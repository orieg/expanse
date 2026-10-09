//! Node.js / Bun / Deno N-API binding for ExpanseOrderedBytesMap (ordered arbitrary-byte map, C ABI: `expanse_ordered_bytesmap_*`).

use crate::common::{BytesInput, BytesMapEntry, KeyInput, bytes_input_to_slice, key_to_u64};
use expanse_trie::ordered_bytesmap::ExpanseOrderedBytesMap as InnerOrderedBytesMap;
use napi::bindgen_prelude::*;
use napi_derive::napi;

// abi-parity: expanse_ordered_bytesmap_free
/// An ordered map from arbitrary byte keys (any length, including empty, `0x00` and `0xFF`) to 64-bit unsigned integers.
/// Keys are ordered as unsigned bytes, lexicographically; a key sorts before any longer key it prefixes.
#[napi]
pub struct ExpanseOrderedBytesMap {
    pub(crate) inner: InnerOrderedBytesMap,
}

#[napi]
impl ExpanseOrderedBytesMap {
    // abi-parity: expanse_ordered_bytesmap_new
    /// Creates an empty ordered byte map.
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            inner: InnerOrderedBytesMap::new(),
        }
    }

    // abi-parity: expanse_ordered_bytesmap_len
    /// Number of entries stored in the map.
    #[napi]
    pub fn size(&self) -> BigInt {
        BigInt::from(self.inner.len())
    }

    /// Returns `true` if the map contains no entries.
    #[napi]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    // abi-parity: expanse_ordered_bytesmap_contains
    /// Membership test `has(key)`. Returns `true` if `key` exists in the map.
    #[napi]
    pub fn has(&self, key: BytesInput) -> bool {
        let bytes = bytes_input_to_slice(&key);
        self.inner.contains_key(bytes)
    }

    // abi-parity: expanse_ordered_bytesmap_insert
    /// Sets `map[key] = value`. Returns previous value as BigInt if present, or `null`.
    #[napi]
    pub fn set(&mut self, key: BytesInput, value: KeyInput) -> Result<Option<BigInt>> {
        let bytes = bytes_input_to_slice(&key);
        let v = key_to_u64(value)?;
        Ok(self.inner.insert(bytes, v).map(BigInt::from))
    }

    // abi-parity: expanse_ordered_bytesmap_get, expanse_ordered_bytesmap_slot
    /// Gets the value for `key`, or `null` if absent.
    #[napi]
    pub fn get(&self, key: BytesInput) -> Option<BigInt> {
        let bytes = bytes_input_to_slice(&key);
        self.inner.get(bytes).map(BigInt::from)
    }

    // abi-parity: expanse_ordered_bytesmap_ins_slot
    /// Returns the value for `key`, inserting it with value `0` first if absent. An existing value is kept.
    #[napi]
    pub fn insert_slot(&mut self, key: BytesInput) -> BigInt {
        let bytes = bytes_input_to_slice(&key);
        if let Some(v) = self.inner.get(bytes) {
            return BigInt::from(v);
        }
        self.inner.insert(bytes, 0);
        BigInt::from(0u64)
    }

    // abi-parity: expanse_ordered_bytesmap_remove
    /// Deletes `key` from the map. Returns `true` if it was present, `false` otherwise.
    #[napi]
    pub fn delete(&mut self, key: BytesInput) -> bool {
        let bytes = bytes_input_to_slice(&key);
        self.inner.remove(bytes).is_some()
    }

    // abi-parity: expanse_ordered_bytesmap_clear
    /// Removes all entries and releases memory.
    #[napi]
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    // abi-parity: expanse_ordered_bytesmap_mem_used
    /// Heap bytes used by the trie nodes and value slots.
    #[napi]
    pub fn mem_used(&self) -> BigInt {
        BigInt::from(self.inner.mem_used() as u64)
    }

    // abi-parity: expanse_ordered_bytesmap_mem_held
    /// Heap bytes held by the map, including spare capacity.
    #[napi]
    pub fn mem_held(&self) -> BigInt {
        BigInt::from(self.inner.mem_held() as u64)
    }

    // abi-parity: expanse_ordered_bytesmap_shrink_to_fit
    /// Releases spare capacity and returns the number of bytes released.
    #[napi]
    pub fn shrink_to_fit(&mut self) -> BigInt {
        BigInt::from(self.inner.shrink_to_fit() as u64)
    }

    // abi-parity: expanse_ordered_bytesmap_first
    /// Smallest key and its value, or `null` if the map is empty. The key is returned as a Buffer, never truncated.
    #[napi]
    pub fn first(&self) -> Option<BytesMapEntry> {
        self.inner.first_entry().map(entry_from)
    }

    // abi-parity: expanse_ordered_bytesmap_last
    /// Largest key and its value, or `null` if the map is empty.
    #[napi]
    pub fn last(&self) -> Option<BytesMapEntry> {
        self.inner.last_entry().map(entry_from)
    }

    // abi-parity: expanse_ordered_bytesmap_next_at_or_after
    /// Smallest entry whose key is `>= key`, or `null` if none.
    #[napi]
    pub fn ceiling(&self, key: BytesInput) -> Option<BytesMapEntry> {
        let bytes = bytes_input_to_slice(&key);
        self.inner.next_at_or_after_entry(bytes).map(entry_from)
    }

    // abi-parity: expanse_ordered_bytesmap_next_after
    /// Smallest entry whose key is `> key`, or `null` if none.
    #[napi]
    pub fn higher(&self, key: BytesInput) -> Option<BytesMapEntry> {
        let bytes = bytes_input_to_slice(&key);
        self.inner.next_after_entry(bytes).map(entry_from)
    }

    // abi-parity: expanse_ordered_bytesmap_prev_at_or_before
    /// Largest entry whose key is `<= key`, or `null` if none.
    #[napi]
    pub fn floor(&self, key: BytesInput) -> Option<BytesMapEntry> {
        let bytes = bytes_input_to_slice(&key);
        self.inner.prev_at_or_before_entry(bytes).map(entry_from)
    }

    // abi-parity: expanse_ordered_bytesmap_prev_before
    /// Largest entry whose key is `< key`, or `null` if none.
    #[napi]
    pub fn lower(&self, key: BytesInput) -> Option<BytesMapEntry> {
        let bytes = bytes_input_to_slice(&key);
        self.inner.prev_before_entry(bytes).map(entry_from)
    }
}

impl Default for ExpanseOrderedBytesMap {
    fn default() -> Self {
        Self::new()
    }
}

fn entry_from((key, value): (Vec<u8>, u64)) -> BytesMapEntry {
    BytesMapEntry {
        key: Buffer::from(key),
        value: BigInt::from(value),
    }
}
