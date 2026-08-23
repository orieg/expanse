//! Node.js / Bun / Deno N-API binding for ExpanseBytesMap (arbitrary byte map, compat: JudyHS).

use crate::common::{BytesInput, BytesMapEntry, KeyInput, bytes_input_to_slice, key_to_u64};
use expanse_trie::bytesmap::ExpanseBytesMap as InnerBytesMap;
use napi::bindgen_prelude::*;
use napi_derive::napi;

/// A sparse, dynamic map from arbitrary byte keys (including NUL bytes) to 64-bit unsigned integers (compat: JudyHS).
#[napi]
pub struct ExpanseBytesMap {
    pub(crate) inner: InnerBytesMap,
}

#[napi]
impl ExpanseBytesMap {
    /// Creates an empty byte map.
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            inner: InnerBytesMap::new(),
        }
    }

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

    /// Membership test `has(key)`. Returns `true` if `key` exists in the map.
    #[napi]
    pub fn has(&self, key: BytesInput) -> bool {
        let bytes = bytes_input_to_slice(&key);
        self.inner.contains_key(bytes)
    }

    /// Sets `map[key] = value`. Returns previous value as BigInt if present, or `null`.
    #[napi]
    pub fn set(&mut self, key: BytesInput, value: KeyInput) -> Result<Option<BigInt>> {
        let bytes = bytes_input_to_slice(&key);
        let v = key_to_u64(value)?;
        Ok(self.inner.insert(bytes, v).map(BigInt::from))
    }

    /// Gets the value for `key`, or `null` if absent.
    #[napi]
    pub fn get(&self, key: BytesInput) -> Option<BigInt> {
        let bytes = bytes_input_to_slice(&key);
        self.inner.get(bytes).map(BigInt::from)
    }

    /// Deletes `key` from the map. Returns `true` if it was present, `false` otherwise.
    #[napi]
    pub fn delete(&mut self, key: BytesInput) -> bool {
        let bytes = bytes_input_to_slice(&key);
        self.inner.remove(bytes).is_some()
    }

    /// Removes all entries and releases memory.
    #[napi]
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Heap bytes used by the hash trie and buckets.
    #[napi]
    pub fn mem_used(&self) -> BigInt {
        BigInt::from(self.inner.mem_used() as u64)
    }

    /// Returns an array of all keys as Node Buffers.
    #[napi]
    pub fn keys(&self) -> Vec<Buffer> {
        let mut keys = Vec::with_capacity(self.inner.len() as usize);
        self.inner.for_each(|k, _| {
            keys.push(Buffer::from(k));
        });
        keys
    }

    /// Returns an array of all values.
    #[napi]
    pub fn values(&self) -> Vec<BigInt> {
        let mut values = Vec::with_capacity(self.inner.len() as usize);
        self.inner.for_each(|_, v| {
            values.push(BigInt::from(v));
        });
        values
    }

    /// Returns an array of all `{ key, value }` entries.
    #[napi]
    pub fn entries(&self) -> Vec<BytesMapEntry> {
        let mut items = Vec::with_capacity(self.inner.len() as usize);
        self.inner.for_each(|k, v| {
            items.push(BytesMapEntry {
                key: Buffer::from(k),
                value: BigInt::from(v),
            });
        });
        items
    }
}

impl Default for ExpanseBytesMap {
    fn default() -> Self {
        Self::new()
    }
}
