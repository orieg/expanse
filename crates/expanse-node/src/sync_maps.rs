//! Node.js / Bun / Deno N-API binding for the shared (concurrent, OCC) string, bytes,
//! ordered-bytes and blob maps: `SyncExpanseStrMap`, `SyncExpanseBytesMap`,
//! `SyncExpanseOrderedBytesMap` and `SyncExpanseBlobMap`.
//!
//! Like `SyncExpanseMap`, each wrapper owns its Rust map directly and registers a
//! throwaway reader per call, so no reader object crosses into JavaScript. The addon's
//! objects belong to one JavaScript thread; the maps themselves are the shared
//! structures the C handles wrap.

use crate::common::{
    BlobMetaResult, BytesInput, BytesMapEntry, CompactionStatsResult, KeyInput, StrMapEntry,
    bytes_input_to_slice, key_to_u64, str_to_nul_free_bytes,
};
use crate::sync::CompareExchangeResult;
use expanse_trie::blobmap::{ArenaError, CompareExchangeError};
use expanse_trie::sync::{
    SyncExpanseBlobMap as InnerBlobMap, SyncExpanseBytesMap as InnerBytesMap,
    SyncExpanseOrderedBytesMap as InnerOrderedBytesMap, SyncExpanseStrMap as InnerStrMap,
};
use napi::bindgen_prelude::*;
use napi_derive::napi;

/// The outcome of a word-valued exchange as the JavaScript result object.
fn exchange_result(r: core::result::Result<Option<u64>, Option<u64>>) -> CompareExchangeResult {
    let (stored, seen) = match r {
        Ok(seen) => (true, seen),
        Err(seen) => (false, seen),
    };
    CompareExchangeResult {
        stored,
        observed: seen.map_or(Either::B(Null), |v| Either::A(BigInt::from(v))),
    }
}

/// An optional `u64` as a BigInt-or-null JavaScript value.
fn opt_u64(v: Option<u64>) -> Option<BigInt> {
    v.map(BigInt::from)
}

// --- String map -----------------------------------------------------------------

fn str_entry((key, value): (Vec<u8>, u64)) -> StrMapEntry {
    StrMapEntry {
        key: String::from_utf8_lossy(&key).into_owned(),
        value: BigInt::from(value),
    }
}

// abi-parity: expanse_sync_strmap_free
/// A thread-safe concurrent map from NUL-free UTF-8 strings to 64-bit unsigned integers,
/// with optimistic concurrency control (OCC) and epoch-based memory reclamation.
///
/// Lookups and the ordered reads run on the optimistic path; iteration order is
/// byte-lexicographical.
#[napi]
pub struct SyncExpanseStrMap {
    inner: InnerStrMap,
}

#[napi]
impl SyncExpanseStrMap {
    // abi-parity: expanse_sync_strmap_new
    /// Creates an empty concurrent string map.
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            inner: InnerStrMap::new(),
        }
    }

    // abi-parity: expanse_sync_strmap_len
    /// Number of entries in the map.
    #[napi]
    pub fn size(&self) -> BigInt {
        BigInt::from(self.inner.len())
    }

    /// Returns `true` if the map contains no entries.
    #[napi]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    // abi-parity: expanse_sync_strmap_mem_used
    /// Heap bytes used by the map's nodes and leaves, read with writers excluded.
    #[napi]
    pub fn mem_used(&self) -> BigInt {
        BigInt::from(self.inner.mem_used() as u64)
    }

    // abi-parity: expanse_sync_strmap_mem_held
    /// Heap bytes the map holds from the global allocator, including the blocks its
    /// epoch collector keeps for reuse or is waiting to reclaim.
    #[napi]
    pub fn mem_held(&self) -> BigInt {
        BigInt::from(self.inner.mem_held() as u64)
    }

    // abi-parity: expanse_sync_strmap_shrink_to_fit
    /// Returns the freed blocks the collector keeps for reuse to the global allocator;
    /// returns the bytes released. Runs beside readers and writers.
    #[napi]
    pub fn shrink_to_fit(&self) -> BigInt {
        BigInt::from(self.inner.shrink_to_fit() as u64)
    }

    // abi-parity: expanse_sync_strmap_reader_contains
    /// Optimistic membership test `has(key)`.
    #[napi]
    pub fn has(&self, key: String) -> Result<bool> {
        let k = str_to_nul_free_bytes(&key)?;
        Ok(self.inner.contains_key(k))
    }

    // abi-parity: expanse_sync_strmap_insert
    /// Sets `map[key] = value`. Returns the previous value, or `null`.
    #[napi]
    pub fn set(&self, key: String, value: KeyInput) -> Result<Option<BigInt>> {
        let k = str_to_nul_free_bytes(&key)?;
        let v = key_to_u64(value)?;
        Ok(opt_u64(self.inner.insert(k, v)))
    }

    // abi-parity: expanse_sync_strmap_get, expanse_sync_strmap_contains
    // abi-parity: expanse_sync_strmap_reader_new, expanse_sync_strmap_reader_free
    // abi-parity: expanse_sync_strmap_reader_get
    /// Optimistic retrieval of the value for `key`, or `null` if absent.
    #[napi]
    pub fn get(&self, key: String) -> Result<Option<BigInt>> {
        let k = str_to_nul_free_bytes(&key)?;
        Ok(opt_u64(self.inner.get(k)))
    }

    // abi-parity: expanse_sync_strmap_remove
    /// Deletes `key`. Returns `true` if it was present.
    #[napi]
    pub fn delete(&self, key: String) -> Result<bool> {
        let k = str_to_nul_free_bytes(&key)?;
        Ok(self.inner.remove(k).is_some())
    }

    /// Removes all entries.
    #[napi]
    pub fn clear(&self) {
        self.inner.clear();
    }

    // abi-parity: expanse_sync_strmap_compare_exchange
    /// Atomic compare-and-swap on one key: stores `newValue` only if the key's current
    /// value is `expected`, as one linearizable step. `null` (or `undefined`) is
    /// "absent" on both sides. `stored` is `true` when the store happened; `observed`
    /// is the value the compare saw.
    #[napi]
    pub fn compare_exchange(
        &self,
        key: String,
        expected: Option<KeyInput>,
        new_value: Option<KeyInput>,
    ) -> Result<CompareExchangeResult> {
        let k = str_to_nul_free_bytes(&key)?;
        let e = expected.map(key_to_u64).transpose()?;
        let n = new_value.map(key_to_u64).transpose()?;
        Ok(exchange_result(self.inner.compare_exchange(k, e, n)))
    }

    // abi-parity: expanse_sync_strmap_get_or_insert
    /// Stores `value` under `key` only if `key` is absent, as one linearizable step.
    /// Returns the value already present, or `null` when `value` was stored.
    #[napi]
    pub fn get_or_insert(&self, key: String, value: KeyInput) -> Result<Option<BigInt>> {
        let k = str_to_nul_free_bytes(&key)?;
        let v = key_to_u64(value)?;
        Ok(opt_u64(self.inner.get_or_insert(k, v)))
    }

    /// Read-modify-write on one key: replaces the value with `fn(current)` (`current`
    /// is `null` if absent; a `null` result removes the key). Returns the value that
    /// was replaced. A retry loop over `compareExchange` on the calling thread: `fn`
    /// may run more than once and must be free of side effects.
    #[napi]
    pub fn update(
        &self,
        key: String,
        r#fn: Function<Option<BigInt>, Option<KeyInput>>,
    ) -> Result<Option<BigInt>> {
        let k = str_to_nul_free_bytes(&key)?;
        let mut expected = self.inner.get(k);
        loop {
            let new = r#fn.call(opt_u64(expected))?.map(key_to_u64).transpose()?;
            match self.inner.compare_exchange(k, expected, new) {
                Ok(prev) => return Ok(opt_u64(prev)),
                Err(seen) => expected = seen,
            }
        }
    }

    // abi-parity: expanse_sync_strmap_reader_first
    /// Smallest entry in byte-lexicographical order, or `null` if empty (optimistic read).
    #[napi]
    pub fn first(&self) -> Option<StrMapEntry> {
        self.inner.reader().first().map(str_entry)
    }

    // abi-parity: expanse_sync_strmap_reader_last
    /// Largest entry in byte-lexicographical order, or `null` if empty (optimistic read).
    #[napi]
    pub fn last(&self) -> Option<StrMapEntry> {
        self.inner.reader().last().map(str_entry)
    }

    // abi-parity: expanse_sync_strmap_reader_next_at_or_after
    // abi-parity: expanse_sync_strmap_reader_next_after
    /// Smallest entry with key strictly `> key` (or `>= key` if `inclusive` is `true`)
    /// (optimistic read).
    #[napi]
    pub fn next(&self, key: String, inclusive: Option<bool>) -> Result<Option<StrMapEntry>> {
        let k = str_to_nul_free_bytes(&key)?;
        let r = self.inner.reader();
        let e = if inclusive.unwrap_or(false) {
            r.next_at_or_after(k)
        } else {
            r.next_after(k)
        };
        Ok(e.map(str_entry))
    }

    // abi-parity: expanse_sync_strmap_reader_prev_at_or_before
    // abi-parity: expanse_sync_strmap_reader_prev_before
    /// Largest entry with key strictly `< key` (or `<= key` if `inclusive` is `true`)
    /// (optimistic read).
    #[napi]
    pub fn prev(&self, key: String, inclusive: Option<bool>) -> Result<Option<StrMapEntry>> {
        let k = str_to_nul_free_bytes(&key)?;
        let r = self.inner.reader();
        let e = if inclusive.unwrap_or(false) {
            r.prev_at_or_before(k)
        } else {
            r.prev_before(k)
        };
        Ok(e.map(str_entry))
    }
}

impl Default for SyncExpanseStrMap {
    fn default() -> Self {
        Self::new()
    }
}

// --- Bytes map ------------------------------------------------------------------

// abi-parity: expanse_sync_bytesmap_free
/// A thread-safe concurrent map from arbitrary byte keys (NUL bytes included) to 64-bit
/// unsigned integers, with optimistic concurrency control (OCC). Unordered.
#[napi]
pub struct SyncExpanseBytesMap {
    inner: InnerBytesMap,
}

#[napi]
impl SyncExpanseBytesMap {
    // abi-parity: expanse_sync_bytesmap_new
    /// Creates an empty concurrent bytes map.
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            inner: InnerBytesMap::new(),
        }
    }

    // abi-parity: expanse_sync_bytesmap_len
    /// Number of entries in the map.
    #[napi]
    pub fn size(&self) -> BigInt {
        BigInt::from(self.inner.len())
    }

    /// Returns `true` if the map contains no entries.
    #[napi]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    // abi-parity: expanse_sync_bytesmap_mem_used
    /// Heap bytes used by the map, read with writers excluded.
    #[napi]
    pub fn mem_used(&self) -> BigInt {
        BigInt::from(self.inner.mem_used() as u64)
    }

    // abi-parity: expanse_sync_bytesmap_mem_held
    /// Heap bytes the map holds from the global allocator, including the blocks its
    /// epoch collector keeps for reuse or is waiting to reclaim.
    #[napi]
    pub fn mem_held(&self) -> BigInt {
        BigInt::from(self.inner.mem_held() as u64)
    }

    // abi-parity: expanse_sync_bytesmap_shrink_to_fit
    /// Returns the freed blocks the collector keeps for reuse to the global allocator;
    /// returns the bytes released.
    #[napi]
    pub fn shrink_to_fit(&self) -> BigInt {
        BigInt::from(self.inner.shrink_to_fit() as u64)
    }

    // abi-parity: expanse_sync_bytesmap_reader_contains
    /// Optimistic membership test `has(key)`.
    #[napi]
    pub fn has(&self, key: BytesInput) -> bool {
        self.inner.contains_key(bytes_input_to_slice(&key))
    }

    // abi-parity: expanse_sync_bytesmap_insert
    /// Sets `map[key] = value`. Returns the previous value, or `null`.
    #[napi]
    pub fn set(&self, key: BytesInput, value: KeyInput) -> Result<Option<BigInt>> {
        let v = key_to_u64(value)?;
        Ok(opt_u64(self.inner.insert(bytes_input_to_slice(&key), v)))
    }

    // abi-parity: expanse_sync_bytesmap_get, expanse_sync_bytesmap_contains
    // abi-parity: expanse_sync_bytesmap_reader_new, expanse_sync_bytesmap_reader_free
    // abi-parity: expanse_sync_bytesmap_reader_get
    /// Optimistic retrieval of the value for `key`, or `null` if absent.
    #[napi]
    pub fn get(&self, key: BytesInput) -> Option<BigInt> {
        opt_u64(self.inner.get(bytes_input_to_slice(&key)))
    }

    // abi-parity: expanse_sync_bytesmap_remove
    /// Deletes `key`. Returns `true` if it was present.
    #[napi]
    pub fn delete(&self, key: BytesInput) -> bool {
        self.inner.remove(bytes_input_to_slice(&key)).is_some()
    }

    /// Removes all entries.
    #[napi]
    pub fn clear(&self) {
        self.inner.clear();
    }

    // abi-parity: expanse_sync_bytesmap_compare_exchange
    /// Atomic compare-and-swap on one key: stores `newValue` only if the key's current
    /// value is `expected`, as one linearizable step. `null` (or `undefined`) is
    /// "absent" on both sides. `stored` is `true` when the store happened; `observed`
    /// is the value the compare saw.
    #[napi]
    pub fn compare_exchange(
        &self,
        key: BytesInput,
        expected: Option<KeyInput>,
        new_value: Option<KeyInput>,
    ) -> Result<CompareExchangeResult> {
        let e = expected.map(key_to_u64).transpose()?;
        let n = new_value.map(key_to_u64).transpose()?;
        Ok(exchange_result(self.inner.compare_exchange(
            bytes_input_to_slice(&key),
            e,
            n,
        )))
    }

    // abi-parity: expanse_sync_bytesmap_get_or_insert
    /// Stores `value` under `key` only if `key` is absent, as one linearizable step.
    /// Returns the value already present, or `null` when `value` was stored.
    #[napi]
    pub fn get_or_insert(&self, key: BytesInput, value: KeyInput) -> Result<Option<BigInt>> {
        let v = key_to_u64(value)?;
        Ok(opt_u64(
            self.inner.get_or_insert(bytes_input_to_slice(&key), v),
        ))
    }

    /// Read-modify-write on one key: replaces the value with `fn(current)` (`current`
    /// is `null` if absent; a `null` result removes the key). Returns the value that
    /// was replaced. A retry loop over `compareExchange` on the calling thread: `fn`
    /// may run more than once and must be free of side effects.
    #[napi]
    pub fn update(
        &self,
        key: BytesInput,
        r#fn: Function<Option<BigInt>, Option<KeyInput>>,
    ) -> Result<Option<BigInt>> {
        let k = bytes_input_to_slice(&key);
        let mut expected = self.inner.get(k);
        loop {
            let new = r#fn.call(opt_u64(expected))?.map(key_to_u64).transpose()?;
            match self.inner.compare_exchange(k, expected, new) {
                Ok(prev) => return Ok(opt_u64(prev)),
                Err(seen) => expected = seen,
            }
        }
    }
}

impl Default for SyncExpanseBytesMap {
    fn default() -> Self {
        Self::new()
    }
}

// --- Ordered-bytes map ----------------------------------------------------------

fn bytes_entry((key, value): (Vec<u8>, u64)) -> BytesMapEntry {
    BytesMapEntry {
        key: Buffer::from(key),
        value: BigInt::from(value),
    }
}

// abi-parity: expanse_sync_ordered_bytesmap_free
/// A thread-safe concurrent map from arbitrary byte keys (NUL bytes included) to 64-bit
/// unsigned integers, ordered by byte-lexicographical key order.
///
/// Point operations (`get`, `has`, `set`, `delete`, the exchanges) run on the
/// optimistic path. The ordered reads (`first`, `last`, `next`, `prev`) run with
/// writers excluded: each blocks concurrent writers for the duration of the call.
#[napi]
pub struct SyncExpanseOrderedBytesMap {
    inner: InnerOrderedBytesMap,
}

#[napi]
impl SyncExpanseOrderedBytesMap {
    // abi-parity: expanse_sync_ordered_bytesmap_new
    /// Creates an empty concurrent ordered-bytes map.
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            inner: InnerOrderedBytesMap::new(),
        }
    }

    // abi-parity: expanse_sync_ordered_bytesmap_len
    /// Number of entries in the map.
    #[napi]
    pub fn size(&self) -> BigInt {
        BigInt::from(self.inner.len())
    }

    /// Returns `true` if the map contains no entries.
    #[napi]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    // abi-parity: expanse_sync_ordered_bytesmap_mem_used
    /// Heap bytes used by the map, read with writers excluded.
    #[napi]
    pub fn mem_used(&self) -> BigInt {
        BigInt::from(self.inner.mem_used() as u64)
    }

    // abi-parity: expanse_sync_ordered_bytesmap_mem_held
    /// Heap bytes the map holds from the global allocator, including the blocks its
    /// epoch collector keeps for reuse or is waiting to reclaim.
    #[napi]
    pub fn mem_held(&self) -> BigInt {
        BigInt::from(self.inner.mem_held() as u64)
    }

    // abi-parity: expanse_sync_ordered_bytesmap_shrink_to_fit
    /// Returns the freed blocks the collector keeps for reuse to the global allocator;
    /// returns the bytes released.
    #[napi]
    pub fn shrink_to_fit(&self) -> BigInt {
        BigInt::from(self.inner.shrink_to_fit() as u64)
    }

    // abi-parity: expanse_sync_ordered_bytesmap_reader_contains
    /// Optimistic membership test `has(key)`.
    #[napi]
    pub fn has(&self, key: BytesInput) -> bool {
        self.inner.contains_key(bytes_input_to_slice(&key))
    }

    // abi-parity: expanse_sync_ordered_bytesmap_insert
    /// Sets `map[key] = value`. Returns the previous value, or `null`.
    #[napi]
    pub fn set(&self, key: BytesInput, value: KeyInput) -> Result<Option<BigInt>> {
        let v = key_to_u64(value)?;
        Ok(opt_u64(self.inner.insert(bytes_input_to_slice(&key), v)))
    }

    // abi-parity: expanse_sync_ordered_bytesmap_get, expanse_sync_ordered_bytesmap_contains
    // abi-parity: expanse_sync_ordered_bytesmap_reader_new
    // abi-parity: expanse_sync_ordered_bytesmap_reader_free
    // abi-parity: expanse_sync_ordered_bytesmap_reader_get
    /// Optimistic retrieval of the value for `key`, or `null` if absent.
    #[napi]
    pub fn get(&self, key: BytesInput) -> Option<BigInt> {
        opt_u64(self.inner.get(bytes_input_to_slice(&key)))
    }

    // abi-parity: expanse_sync_ordered_bytesmap_remove
    /// Deletes `key`. Returns `true` if it was present.
    #[napi]
    pub fn delete(&self, key: BytesInput) -> bool {
        self.inner.remove(bytes_input_to_slice(&key)).is_some()
    }

    /// Removes all entries.
    #[napi]
    pub fn clear(&self) {
        self.inner.clear();
    }

    // abi-parity: expanse_sync_ordered_bytesmap_compare_exchange
    /// Atomic compare-and-swap on one key: stores `newValue` only if the key's current
    /// value is `expected`, as one linearizable step. `null` (or `undefined`) is
    /// "absent" on both sides. `stored` is `true` when the store happened; `observed`
    /// is the value the compare saw.
    #[napi]
    pub fn compare_exchange(
        &self,
        key: BytesInput,
        expected: Option<KeyInput>,
        new_value: Option<KeyInput>,
    ) -> Result<CompareExchangeResult> {
        let e = expected.map(key_to_u64).transpose()?;
        let n = new_value.map(key_to_u64).transpose()?;
        Ok(exchange_result(self.inner.compare_exchange(
            bytes_input_to_slice(&key),
            e,
            n,
        )))
    }

    // abi-parity: expanse_sync_ordered_bytesmap_get_or_insert
    /// Stores `value` under `key` only if `key` is absent, as one linearizable step.
    /// Returns the value already present, or `null` when `value` was stored.
    #[napi]
    pub fn get_or_insert(&self, key: BytesInput, value: KeyInput) -> Result<Option<BigInt>> {
        let v = key_to_u64(value)?;
        // The exchange against absent, which is what
        // `SyncExpanseOrderedBytesMap::get_or_insert` runs.
        let seen = match self
            .inner
            .compare_exchange(bytes_input_to_slice(&key), None, Some(v))
        {
            Ok(_) => None,
            Err(seen) => seen,
        };
        Ok(opt_u64(seen))
    }

    /// Read-modify-write on one key: replaces the value with `fn(current)` (`current`
    /// is `null` if absent; a `null` result removes the key). Returns the value that
    /// was replaced. A retry loop over `compareExchange` on the calling thread: `fn`
    /// may run more than once and must be free of side effects.
    #[napi]
    pub fn update(
        &self,
        key: BytesInput,
        r#fn: Function<Option<BigInt>, Option<KeyInput>>,
    ) -> Result<Option<BigInt>> {
        let k = bytes_input_to_slice(&key);
        let mut expected = self.inner.get(k);
        loop {
            let new = r#fn.call(opt_u64(expected))?.map(key_to_u64).transpose()?;
            match self.inner.compare_exchange(k, expected, new) {
                Ok(prev) => return Ok(opt_u64(prev)),
                Err(seen) => expected = seen,
            }
        }
    }

    // abi-parity: expanse_sync_ordered_bytesmap_first
    /// Smallest entry in byte-lexicographical order, or `null` if empty. Runs with
    /// writers excluded.
    #[napi]
    pub fn first(&self) -> Option<BytesMapEntry> {
        self.inner.first().map(bytes_entry)
    }

    // abi-parity: expanse_sync_ordered_bytesmap_last
    /// Largest entry in byte-lexicographical order, or `null` if empty. Runs with
    /// writers excluded.
    #[napi]
    pub fn last(&self) -> Option<BytesMapEntry> {
        self.inner.last().map(bytes_entry)
    }

    // abi-parity: expanse_sync_ordered_bytesmap_next_at_or_after
    // abi-parity: expanse_sync_ordered_bytesmap_next_after
    /// Smallest entry with key strictly `> key` (or `>= key` if `inclusive` is `true`).
    /// Runs with writers excluded.
    #[napi]
    pub fn next(&self, key: BytesInput, inclusive: Option<bool>) -> Option<BytesMapEntry> {
        let k = bytes_input_to_slice(&key);
        let e = if inclusive.unwrap_or(false) {
            self.inner.next_at_or_after(k)
        } else {
            self.inner.next_after(k)
        };
        e.map(bytes_entry)
    }

    // abi-parity: expanse_sync_ordered_bytesmap_prev_at_or_before
    // abi-parity: expanse_sync_ordered_bytesmap_prev_before
    /// Largest entry with key strictly `< key` (or `<= key` if `inclusive` is `true`).
    /// Runs with writers excluded.
    #[napi]
    pub fn prev(&self, key: BytesInput, inclusive: Option<bool>) -> Option<BytesMapEntry> {
        let k = bytes_input_to_slice(&key);
        let e = if inclusive.unwrap_or(false) {
            self.inner.prev_at_or_before(k)
        } else {
            self.inner.prev_before(k)
        };
        e.map(bytes_entry)
    }
}

impl Default for SyncExpanseOrderedBytesMap {
    fn default() -> Self {
        Self::new()
    }
}

// --- Blob map -------------------------------------------------------------------

#[napi(object)]
/// A blob payload and its 24-bit hot metadata, as the shared blob map's owned reads
/// and exchanges report it.
pub struct BlobValue {
    /// The payload bytes.
    pub payload: Buffer,
    /// The hot metadata word, as stored, at every payload length (#1548).
    pub hot_meta: u32,
}

#[napi(object)]
/// A blob value offered to `compareExchange`: payload plus optional hot metadata
/// (default `0`).
pub struct BlobValueInput {
    /// The payload bytes (`Buffer`, `Uint8Array` or UTF-8 `string`).
    pub payload: BytesInput,
    /// The hot metadata word, at most 2^24 - 1 on an arena payload.
    pub hot_meta: Option<u32>,
}

#[napi(object)]
/// The outcome of `SyncExpanseBlobMap.compareExchange`.
pub struct BlobCompareExchangeResult {
    /// `true` when the new value was stored (or the key removed).
    pub stored: bool,
    /// The value the compare saw, or `null` if the key was absent.
    pub observed: Either<BlobValue, Null>,
}

#[napi(object)]
/// The outcome of `SyncExpanseBlobMap.compareExchangeMeta`.
pub struct BlobMetaExchangeResult {
    /// `true` when the new metadata was stored (or the key removed).
    pub stored: bool,
    /// The metadata the compare saw, or `null` if the key was absent.
    pub observed: Either<u32, Null>,
}

fn blob_value((payload, hot_meta): (Vec<u8>, u32)) -> BlobValue {
    BlobValue {
        payload: Buffer::from(payload),
        hot_meta,
    }
}

fn blob_observed(v: Option<(Vec<u8>, u32)>) -> Either<BlobValue, Null> {
    v.map_or(Either::B(Null), |v| Either::A(blob_value(v)))
}

fn arena_err(what: &str, e: ArenaError) -> Error {
    Error::new(Status::GenericFailure, format!("{what}: {e}"))
}

// abi-parity: expanse_sync_blob_map_free
/// A thread-safe concurrent map from 64-bit integer keys to arbitrary-length byte
/// payloads with 24-bit hot metadata, with optimistic concurrency control (OCC).
///
/// Reads return owned copies (`Buffer`), valid whatever writers do afterwards.
/// A value is stored in its value slot, without metadata, only when written with
/// `hotMeta` `0`; a payload of any length written with metadata keeps it (#1548).
#[napi]
pub struct SyncExpanseBlobMap {
    inner: InnerBlobMap,
}

#[napi]
impl SyncExpanseBlobMap {
    // abi-parity: expanse_sync_blob_map_new, expanse_sync_blob_map_new_with_capacity
    /// Creates an empty concurrent blob map, optionally with an arena chunk size in
    /// bytes and an arena capacity cap `maxCapacity` in bytes of allocated chunks
    /// (default 1 GiB; clamped to `[chunkSize, 64 GiB]`).
    #[napi(constructor)]
    pub fn new(chunk_size: Option<u32>, max_capacity: Option<i64>) -> Result<Self> {
        let inner = match (chunk_size, max_capacity) {
            (Some(sz), None) => InnerBlobMap::with_chunk_size(sz as usize),
            (None, None) => InnerBlobMap::new(),
            (sz, Some(cap)) => {
                let cap = usize::try_from(cap).map_err(|_| {
                    Error::new(Status::InvalidArg, format!("maxCapacity {cap} is negative"))
                })?;
                InnerBlobMap::with_chunk_size_and_max_capacity(
                    sz.map_or(expanse_trie::blobmap::DEFAULT_CHUNK_SIZE, |s| s as usize),
                    cap,
                )
            }
        };
        Ok(Self { inner })
    }

    // abi-parity: expanse_sync_blob_map_len
    /// Number of entries in the map.
    #[napi]
    pub fn size(&self) -> BigInt {
        BigInt::from(self.inner.len())
    }

    /// Returns `true` if the map contains no entries.
    #[napi]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    // abi-parity: expanse_sync_blob_map_mem_used
    /// Heap bytes used by the index and arena, read with writers excluded.
    #[napi]
    pub fn mem_used(&self) -> BigInt {
        BigInt::from(self.inner.mem_used() as u64)
    }

    // abi-parity: expanse_sync_blob_map_mem_used_estimate
    /// A bounded-cost estimate of `memUsed()` that does not exclude writers.
    #[napi]
    pub fn mem_used_estimate(&self) -> BigInt {
        BigInt::from(self.inner.mem_used_estimate() as u64)
    }

    // abi-parity: expanse_sync_blob_map_mem_held
    /// Heap bytes the map holds from the global allocator, including the blocks its
    /// epoch collector keeps for reuse or is waiting to reclaim.
    #[napi]
    pub fn mem_held(&self) -> BigInt {
        BigInt::from(self.inner.mem_held() as u64)
    }

    // abi-parity: expanse_sync_blob_map_shrink_to_fit
    /// Returns the freed blocks the collector keeps for reuse to the global allocator;
    /// returns the bytes released.
    #[napi]
    pub fn shrink_to_fit(&self) -> BigInt {
        BigInt::from(self.inner.shrink_to_fit() as u64)
    }

    // abi-parity: expanse_sync_blob_map_set_reclaim_at_cap
    /// Turns the reclaim at the capacity cap on (the default) or off. Off, an insert
    /// the cap refuses compacts nothing.
    #[napi(js_name = "setReclaimAtCap")]
    pub fn set_reclaim_at_cap(&self, on: bool) {
        self.inner.set_reclaim_at_cap(on);
    }

    /// Whether an insert the capacity cap refuses may compact the arena.
    #[napi(js_name = "reclaimAtCap")]
    pub fn reclaim_at_cap(&self) -> bool {
        self.inner.reclaim_at_cap()
    }

    // abi-parity: expanse_sync_blob_map_compact
    /// Compacts the arena, returning memory statistics. Concurrent writers wait for it;
    /// reads continue.
    #[napi]
    pub fn compact(&self) -> Result<CompactionStatsResult> {
        let stats = self
            .inner
            .compact()
            .map_err(|e| arena_err("Compaction error", e))?;
        Ok(CompactionStatsResult {
            live_bytes_before: BigInt::from(stats.live_bytes_before as u64),
            live_bytes_after: BigInt::from(stats.live_bytes_after as u64),
            total_allocated_before: BigInt::from(stats.total_allocated_before as u64),
            total_allocated_after: BigInt::from(stats.total_allocated_after as u64),
        })
    }

    // abi-parity: expanse_sync_blob_map_reader_contains
    /// Optimistic membership test `has(key)`.
    #[napi]
    pub fn has(&self, key: KeyInput) -> Result<bool> {
        Ok(self.inner.contains_key(key_to_u64(key)?))
    }

    // abi-parity: expanse_sync_blob_map_contains
    /// Same as `has`.
    #[napi]
    pub fn contains(&self, key: KeyInput) -> Result<bool> {
        Ok(self.inner.contains_key(key_to_u64(key)?))
    }

    // abi-parity: expanse_sync_blob_map_insert
    /// Inserts a key-blob pair with optional 24-bit hot metadata. Throws if the arena
    /// refuses it (metadata overflow, capacity cap, allocation failure).
    #[napi]
    pub fn set(&self, key: KeyInput, payload: BytesInput, hot_meta: Option<u32>) -> Result<()> {
        let k = key_to_u64(key)?;
        self.inner
            .insert(k, bytes_input_to_slice(&payload), hot_meta.unwrap_or(0))
            .map_err(|e| arena_err("Blob insertion error", e))
    }

    // abi-parity: expanse_sync_blob_map_get_into, expanse_sync_blob_map_reader_get_into
    // abi-parity: expanse_sync_blob_map_reader_new, expanse_sync_blob_map_reader_free
    /// Retrieves the payload for `key` as an owned `Buffer`, or `null` if absent.
    #[napi]
    pub fn get(&self, key: KeyInput) -> Result<Option<Buffer>> {
        let k = key_to_u64(key)?;
        Ok(self.inner.get(k).map(|(p, _)| Buffer::from(p)))
    }

    /// Retrieves `{ payload, hotMeta, isInline }` for `key`, or `null` if absent.
    #[napi(js_name = "getWithMeta")]
    pub fn get_with_meta(&self, key: KeyInput) -> Result<Option<BlobMetaResult>> {
        let k = key_to_u64(key)?;
        let mut reader = self.inner.reader();
        let guard = reader.pin();
        Ok(guard.get(k).map(|(view, meta)| BlobMetaResult {
            payload: Buffer::from(view.as_bytes()),
            hot_meta: meta,
            is_inline: view.is_inline(),
        }))
    }

    // abi-parity: expanse_sync_blob_map_get_meta, expanse_sync_blob_map_reader_get_meta
    /// Retrieves only the hot metadata of `key`, or `null` if absent, without touching
    /// the payload. A value written with `hotMeta` `0` reads `0`.
    #[napi(js_name = "getMeta")]
    pub fn get_meta(&self, key: KeyInput) -> Result<Option<u32>> {
        Ok(self.inner.get_meta(key_to_u64(key)?))
    }

    // abi-parity: expanse_sync_blob_map_remove
    /// Deletes `key`. Returns `true` if it was present.
    #[napi]
    pub fn delete(&self, key: KeyInput) -> Result<bool> {
        Ok(self.inner.remove(key_to_u64(key)?))
    }

    /// Removes all entries and resets the arena.
    #[napi]
    pub fn clear(&self) {
        self.inner.clear();
    }

    // abi-parity: expanse_sync_blob_map_compare_exchange
    /// Atomic compare-and-swap on one key over `{ payload, hotMeta }` values: stores
    /// `newValue` only if the key's current value equals `expected` (payload and
    /// metadata), as one linearizable step. `null` (or `undefined`) is "absent" on both
    /// sides, so `(null, v)` inserts if absent and `(e, null)` removes if equal.
    ///
    /// `stored` is `true` when the store happened; `observed` is the value the compare
    /// saw. Throws when the values compare equal but `newValue` cannot be stored
    /// (metadata over 24 bits on an arena payload, or the arena refuses it); the map is
    /// then unchanged. Like `getOrInsert`, this path does not run the reclaim rule at
    /// the capacity cap.
    #[napi]
    pub fn compare_exchange(
        &self,
        key: KeyInput,
        expected: Option<BlobValueInput>,
        new_value: Option<BlobValueInput>,
    ) -> Result<BlobCompareExchangeResult> {
        let k = key_to_u64(key)?;
        let e = expected
            .as_ref()
            .map(|v| (bytes_input_to_slice(&v.payload), v.hot_meta.unwrap_or(0)));
        let n = new_value
            .as_ref()
            .map(|v| (bytes_input_to_slice(&v.payload), v.hot_meta.unwrap_or(0)));
        match self.inner.compare_exchange(k, e, n) {
            Ok(seen) => Ok(BlobCompareExchangeResult {
                stored: true,
                observed: blob_observed(seen),
            }),
            Err(CompareExchangeError::Mismatch(seen)) => Ok(BlobCompareExchangeResult {
                stored: false,
                observed: blob_observed(seen),
            }),
            Err(CompareExchangeError::Unstorable(cause)) => {
                Err(arena_err("Blob compareExchange refused", cause))
            }
        }
    }

    // abi-parity: expanse_sync_blob_map_compare_exchange_meta
    /// Atomic compare-and-swap on the 24-bit hot metadata of one key, with no payload
    /// copy: stores `newMeta` only if the key's current metadata is `expected`. `null`
    /// (or `undefined`) is "absent" on both sides; `(e, null)` removes the key if equal.
    /// A payload of 7 bytes or fewer written with metadata keeps it (#1548).
    ///
    /// Throws when the metadata compares equal but `newMeta` cannot be stored (above 24
    /// bits, or non-zero on an absent key or on a payload stored inline with metadata
    /// `0`); the map is then
    /// unchanged.
    #[napi(js_name = "compareExchangeMeta")]
    pub fn compare_exchange_meta(
        &self,
        key: KeyInput,
        expected: Option<u32>,
        new_meta: Option<u32>,
    ) -> Result<BlobMetaExchangeResult> {
        let k = key_to_u64(key)?;
        let observed = |s: Option<u32>| s.map_or(Either::B(Null), Either::A);
        match self.inner.compare_exchange_meta(k, expected, new_meta) {
            Ok(seen) => Ok(BlobMetaExchangeResult {
                stored: true,
                observed: observed(seen),
            }),
            Err(CompareExchangeError::Mismatch(seen)) => Ok(BlobMetaExchangeResult {
                stored: false,
                observed: observed(seen),
            }),
            Err(CompareExchangeError::Unstorable(cause)) => {
                Err(arena_err("Blob compareExchangeMeta refused", cause))
            }
        }
    }

    // abi-parity: expanse_sync_blob_map_get_or_insert
    /// Stores `payload` (with `hotMeta`, default `0`) under `key` only if `key` is
    /// absent, as one linearizable step. Returns the `{ payload, hotMeta }` already
    /// present (left untouched), or `null` when the value was stored. Throws if the
    /// value cannot be stored; like `compareExchange`, it does not run the reclaim rule
    /// at the capacity cap.
    #[napi]
    pub fn get_or_insert(
        &self,
        key: KeyInput,
        payload: BytesInput,
        hot_meta: Option<u32>,
    ) -> Result<Option<BlobValue>> {
        let k = key_to_u64(key)?;
        let seen = self
            .inner
            .get_or_insert(k, bytes_input_to_slice(&payload), hot_meta.unwrap_or(0))
            .map_err(|e| arena_err("Blob getOrInsert refused", e))?;
        Ok(seen.map(blob_value))
    }
}

impl Default for SyncExpanseBlobMap {
    fn default() -> Self {
        Self {
            inner: InnerBlobMap::new(),
        }
    }
}
