//! Node.js / Bun / Deno N-API binding for ExpanseBlobMap (large-value map with inline packing and arena backing).

use crate::common::{
    BlobMetaResult, BytesInput, CompactionStatsResult, KeyInput, bytes_input_to_slice, key_to_u64,
};
use expanse_trie::blobmap::ExpanseBlobMap as InnerBlobMap;
use expanse_trie::slot::ValueSlot;
use napi::bindgen_prelude::*;
use napi_derive::napi;

/// A high-performance map from 64-bit integer keys to arbitrary-length byte payloads
/// backed by inline polymorphic 64-bit value slots and chunked slab arenas.
#[napi]
pub struct ExpanseBlobMap {
    pub(crate) inner: InnerBlobMap,
}

#[napi]
impl ExpanseBlobMap {
    /// Creates an empty blob map, optionally with custom arena chunk size in bytes.
    #[napi(constructor)]
    pub fn new(chunk_size: Option<u32>) -> Self {
        let inner = match chunk_size {
            Some(sz) => InnerBlobMap::with_chunk_size(sz as usize),
            None => InnerBlobMap::new(),
        };
        Self { inner }
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
    pub fn has(&self, key: KeyInput) -> Result<bool> {
        let k = key_to_u64(key)?;
        Ok(self.inner.contains_key(k))
    }

    /// Inserts a key-blob pair with optional 32-bit hot metadata.
    #[napi]
    pub fn set(&mut self, key: KeyInput, payload: BytesInput, hot_meta: Option<u32>) -> Result<()> {
        let k = key_to_u64(key)?;
        let bytes = bytes_input_to_slice(&payload);
        let meta = hot_meta.unwrap_or(0);
        self.inner
            .insert(k, bytes, meta)
            .map_err(|e| Error::new(Status::GenericFailure, format!("Blob insertion error: {e}")))
    }

    /// Retrieves only the byte payload for `key`, or `null` if absent.
    #[napi]
    pub fn get(&self, key: KeyInput) -> Result<Option<Buffer>> {
        let k = key_to_u64(key)?;
        Ok(self
            .inner
            .get(k)
            .map(|(view, _)| Buffer::from(view.as_bytes())))
    }

    /// Retrieves `{ payload: Buffer, hotMeta: number, isInline: boolean }` for `key`, or `null` if absent.
    #[napi(js_name = "getWithMeta")]
    pub fn get_with_meta(&self, key: KeyInput) -> Result<Option<BlobMetaResult>> {
        let k = key_to_u64(key)?;
        Ok(self.inner.get(k).map(|(view, meta)| BlobMetaResult {
            payload: Buffer::from(view.as_bytes()),
            hot_meta: meta,
            is_inline: view.is_inline(),
        }))
    }

    /// Deletes `key` from the map. Returns `true` if it was present, `false` otherwise.
    #[napi]
    pub fn delete(&mut self, key: KeyInput) -> Result<bool> {
        let k = key_to_u64(key)?;
        Ok(self.inner.remove(k))
    }

    /// Clears all entries and resets the slab arena.
    #[napi]
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Returns total heap memory used by index and slab arena.
    #[napi]
    pub fn mem_used(&self) -> BigInt {
        BigInt::from(self.inner.mem_used() as u64)
    }

    /// Evaluates `predicate(key, hotMeta)` and deletes matching keys. Returns count of pruned entries.
    #[napi]
    pub fn prune(&mut self, predicate: Function<(BigInt, u32), bool>) -> Result<u32> {
        let mut to_remove = Vec::new();
        for (key, raw_slot) in self.inner.index().iter() {
            let slot = ValueSlot::from_raw(raw_slot);
            let meta = if slot.tag() == expanse_trie::slot::SlotTag::ArenaShort {
                slot.hot_meta()
            } else {
                0
            };
            let should_prune = predicate.call((BigInt::from(key), meta))?;
            if should_prune {
                to_remove.push(key);
            }
        }
        let count = to_remove.len() as u32;
        for k in to_remove {
            self.inner.remove(k);
        }
        Ok(count)
    }

    /// Runs in-place garbage collection and compaction, returning memory statistics.
    #[napi]
    pub fn compact(&mut self) -> Result<CompactionStatsResult> {
        let stats = self
            .inner
            .compact()
            .map_err(|e| Error::new(Status::GenericFailure, format!("Compaction error: {e}")))?;
        Ok(CompactionStatsResult {
            live_bytes_before: BigInt::from(stats.live_bytes_before as u64),
            live_bytes_after: BigInt::from(stats.live_bytes_after as u64),
            total_allocated_before: BigInt::from(stats.total_allocated_before as u64),
            total_allocated_after: BigInt::from(stats.total_allocated_after as u64),
        })
    }

    /// Saves the map to a relocatable binary image file. Returns the number of bytes
    /// written as a BigInt (an image can exceed 4 GiB, so the count does not fit in u32).
    #[napi(js_name = "saveImage")]
    pub fn save_image(&self, path: String) -> Result<BigInt> {
        let written = self.inner.save_to_file(&path).map_err(|e| {
            Error::new(Status::GenericFailure, format!("Failed to save image: {e}"))
        })?;
        Ok(BigInt::from(written as u64))
    }

    /// Loads a map from a relocatable binary image file.
    ///
    /// The whole file is read into memory and the index is rebuilt entry-by-entry;
    /// the image is NOT memory-mapped. (There is therefore no lazy-fault SIGBUS hazard
    /// from a file being truncated while mapped — the trade-off is that the full image
    /// is resident after load. A previous `mmap` argument was accepted but never had
    /// any effect, so it has been removed rather than left as a misleading no-op.)
    #[napi(factory, js_name = "openImage")]
    pub fn open_image(path: String) -> Result<ExpanseBlobMap> {
        let inner = InnerBlobMap::load_from_file(&path).map_err(|e| {
            Error::new(Status::GenericFailure, format!("Failed to open image: {e}"))
        })?;
        Ok(Self { inner })
    }
}

impl Default for ExpanseBlobMap {
    fn default() -> Self {
        Self::new(None)
    }
}
