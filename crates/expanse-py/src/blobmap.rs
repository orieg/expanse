//! PyO3 wrapper for ExpanseBlobMap (large-value map with inline packing and arena backing).

use crate::buffer::extract_bytes_key;
use expanse_trie::blobmap::{ArenaError, ExpanseBlobMap as InnerBlobMap};
use pyo3::exceptions::{PyIOError, PyKeyError, PyRuntimeError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};

/// The status name `insert_status` returns for an engine refusal, matching
/// `expanse_blob_status_t` in `include/expanse.h`.
fn status_name(e: ArenaError) -> &'static str {
    match e {
        ArenaError::MetaOverflow => "meta_overflow",
        ArenaError::AllocationFailed => "allocation_failed",
        ArenaError::OffsetOverflow => "cap_refused",
        ArenaError::ArenaFull => "arena_full",
        _ => "error",
    }
}

// abi-parity: expanse_blob_map_free
/// A high-performance map from 64-bit integer keys to arbitrary-length byte payloads
/// backed by inline polymorphic 64-bit value slots and chunked slab arenas.
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct ExpanseBlobMap {
    pub(crate) inner: InnerBlobMap,
}

#[pymethods]
impl ExpanseBlobMap {
    // abi-parity: expanse_blob_map_new, expanse_blob_map_new_with_capacity
    /// Creates an empty blob map, optionally with custom arena chunk size in bytes
    /// and an arena capacity cap `max_capacity` in bytes of allocated chunks
    /// (default 1 GiB; clamped to `[chunk_size, 64 GiB]`).
    #[new]
    #[pyo3(signature = (chunk_size=None, max_capacity=None))]
    pub fn new(chunk_size: Option<usize>, max_capacity: Option<usize>) -> Self {
        let inner = match (chunk_size, max_capacity) {
            (Some(sz), None) => InnerBlobMap::with_chunk_size(sz),
            (None, None) => InnerBlobMap::new(),
            (sz, Some(cap)) => InnerBlobMap::with_chunk_size_and_max_capacity(
                sz.unwrap_or(expanse_trie::blobmap::DEFAULT_CHUNK_SIZE),
                cap,
            ),
        };
        Self { inner }
    }

    // abi-parity: expanse_blob_map_set_reclaim_at_cap
    /// Turns the reclaim at the capacity cap on (the default) or off. Off, an
    /// insert the cap refuses compacts nothing.
    pub fn set_reclaim_at_cap(&mut self, on: bool) {
        self.inner.set_reclaim_at_cap(on);
    }

    /// True if an insert the capacity cap refuses may compact the arena.
    pub fn reclaim_at_cap(&self) -> bool {
        self.inner.reclaim_at_cap()
    }

    // abi-parity: expanse_blob_map_arena_stats
    /// Arena accounting as a dict: `live_bytes` (payloads plus 8-byte headers),
    /// `allocated_bytes` (chunk bytes, what the cap counts), `max_capacity`,
    /// `chunk_size` and `reclaim_at_cap`.
    pub fn arena_stats<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let arena = self.inner.arena();
        let d = PyDict::new(py);
        d.set_item("live_bytes", arena.live_bytes())?;
        d.set_item("allocated_bytes", arena.mem_used())?;
        d.set_item("max_capacity", arena.max_capacity())?;
        d.set_item("chunk_size", arena.chunk_size())?;
        d.set_item("reclaim_at_cap", self.inner.reclaim_at_cap())?;
        Ok(d)
    }

    // abi-parity: expanse_blob_map_insert_ex
    /// Inserts like `insert`, but returns a status instead of raising on a
    /// refusal: `"ok"`, `"meta_overflow"`, `"allocation_failed"`,
    /// `"cap_refused"` (the capacity cap refused a chunk and nothing was
    /// compacted, so `compact()` may free dead bytes) or `"arena_full"` (this
    /// insert compacted and the record still does not fit).
    #[pyo3(signature = (key, data, hot_meta=0))]
    pub fn insert_status(
        &mut self,
        key: u64,
        data: &Bound<'_, PyAny>,
        hot_meta: u32,
    ) -> PyResult<&'static str> {
        let bytes = extract_bytes_key(data)?;
        Ok(match self.inner.insert(key, &bytes, hot_meta) {
            Ok(()) => "ok",
            Err(e) => status_name(e),
        })
    }

    // abi-parity: expanse_blob_map_len
    /// Number of entries stored in the map.
    pub fn __len__(&self) -> usize {
        self.inner.len() as usize
    }

    /// Number of entries stored in the map.
    pub fn len(&self) -> usize {
        self.inner.len() as usize
    }

    /// True when no entries are in the map.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Property returning True if empty.
    #[getter]
    pub fn empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Truth value testing for Python.
    pub fn __bool__(&self) -> bool {
        !self.inner.is_empty()
    }

    /// Membership test `key in map`.
    pub fn __contains__(&self, key: u64) -> bool {
        self.inner.contains_key(key)
    }

    // abi-parity: expanse_blob_map_contains
    /// Returns True if key exists in the map.
    pub fn contains(&self, key: u64) -> bool {
        self.inner.contains_key(key)
    }

    // abi-parity: expanse_blob_map_contains_key
    /// Returns True if key exists in the map.
    pub fn contains_key(&self, key: u64) -> bool {
        self.inner.contains_key(key)
    }

    // abi-parity: expanse_blob_map_insert
    /// Inserts a key-blob pair with optional 32-bit hot metadata.
    #[pyo3(signature = (key, data, hot_meta=0))]
    pub fn insert(&mut self, key: u64, data: &Bound<'_, PyAny>, hot_meta: u32) -> PyResult<()> {
        let bytes = extract_bytes_key(data)?;
        self.inner
            .insert(key, &bytes, hot_meta)
            .map_err(|e| PyRuntimeError::new_err(format!("Blob allocation error: {e}")))
    }

    // abi-parity: expanse_blob_map_get, expanse_blob_map_get_into
    /// Retrieves `(bytes_payload, hot_meta)` for a key, or None if absent.
    pub fn get<'py>(&self, py: Python<'py>, key: u64) -> Option<(Bound<'py, PyBytes>, u32)> {
        let (view, meta) = self.inner.get(key)?;
        let py_bytes = PyBytes::new(py, view.as_bytes());
        Some((py_bytes, meta))
    }

    /// Retrieves only the byte payload for a key, or None if absent.
    pub fn get_bytes<'py>(&self, py: Python<'py>, key: u64) -> Option<Bound<'py, PyBytes>> {
        let (view, _) = self.inner.get(key)?;
        Some(PyBytes::new(py, view.as_bytes()))
    }

    /// Retrieves `val = map[key]`; raises `KeyError` if key is missing.
    pub fn __getitem__<'py>(&self, py: Python<'py>, key: u64) -> PyResult<Bound<'py, PyBytes>> {
        self.get_bytes(py, key)
            .ok_or_else(|| PyKeyError::new_err(format!("Key {key} not found in ExpanseBlobMap")))
    }

    /// Sets `map[key] = data` (with hot_meta = 0).
    pub fn __setitem__(&mut self, key: u64, data: &Bound<'_, PyAny>) -> PyResult<()> {
        self.insert(key, data, 0)
    }

    /// Deletes `del map[key]`; raises `KeyError` if key is missing.
    pub fn __delitem__(&mut self, key: u64) -> PyResult<()> {
        if self.inner.remove(key) {
            Ok(())
        } else {
            Err(PyKeyError::new_err(format!(
                "Key {key} not found in ExpanseBlobMap"
            )))
        }
    }

    // abi-parity: expanse_blob_map_remove
    /// Removes a key from the map; returns True if key was present.
    pub fn remove(&mut self, key: u64) -> bool {
        self.inner.remove(key)
    }

    // abi-parity: expanse_blob_map_clear
    /// Clears all entries and resets the slab arena.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    // abi-parity: expanse_blob_map_mem_used
    /// Returns total heap memory used by index and slab arena.
    pub fn mem_used(&self) -> usize {
        self.inner.mem_used()
    }

    // abi-parity: expanse_blob_map_compact
    /// Runs in-place garbage collection and compaction, returning
    /// `(live_bytes_before, live_bytes_after, total_allocated_before, total_allocated_after)`.
    pub fn compact(&mut self) -> PyResult<(usize, usize, usize, usize)> {
        let stats = self
            .inner
            .compact()
            .map_err(|e| PyRuntimeError::new_err(format!("Compaction error: {e}")))?;
        Ok((
            stats.live_bytes_before,
            stats.live_bytes_after,
            stats.total_allocated_before,
            stats.total_allocated_after,
        ))
    }

    // abi-parity: expanse_blob_map_scan_filtered
    /// Executes a range scan over keys in `[start_key, end_key]` with optional predicate filtering
    /// on 32-bit hot metadata.
    #[pyo3(signature = (start_key, end_key, predicate=None, callback=None))]
    pub fn scan_filtered<'py>(
        &self,
        py: Python<'py>,
        start_key: u64,
        end_key: u64,
        predicate: Option<&Bound<'py, PyAny>>,
        callback: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Vec<(u64, Bound<'py, PyBytes>, u32)>> {
        let mut results = Vec::new();
        // A Python predicate/callback can raise. The core `scan_filtered` closures
        // return `bool`, so we cannot propagate a PyErr through them directly: instead
        // we stash the first error here, stop the scan, and re-raise afterwards. The
        // previous code silently swallowed these errors (treating a raised predicate as
        // "keep" and a raised callback as "stop"), producing quietly wrong results.
        let err_cell: std::cell::RefCell<Option<PyErr>> = std::cell::RefCell::new(None);

        self.inner.scan_filtered(
            start_key..=end_key,
            |key, meta| {
                if err_cell.borrow().is_some() {
                    return false; // already aborting: skip remaining keys (no Python calls)
                }
                if let Some(pred) = predicate {
                    match pred.call1((key, meta)).and_then(|res| res.is_truthy()) {
                        Ok(keep) => keep,
                        Err(e) => {
                            *err_cell.borrow_mut() = Some(e);
                            // Return true so the callback runs next and breaks the scan.
                            true
                        }
                    }
                } else {
                    true
                }
            },
            |key, view, meta| {
                if err_cell.borrow().is_some() {
                    return false; // abort the scan as soon as an error is pending
                }
                let py_bytes = PyBytes::new(py, view.as_bytes());
                if let Some(cb) = callback {
                    match cb
                        .call1((key, py_bytes.clone(), meta))
                        .and_then(|res| res.is_truthy())
                    {
                        Ok(keep_going) => keep_going,
                        Err(e) => {
                            *err_cell.borrow_mut() = Some(e);
                            false
                        }
                    }
                } else {
                    results.push((key, py_bytes, meta));
                    true
                }
            },
        );

        if let Some(e) = err_cell.into_inner() {
            return Err(e);
        }
        Ok(results)
    }

    /// Saves the map to a relocatable binary image file.
    pub fn save_to_file(&self, path: &str) -> PyResult<usize> {
        self.inner
            .save_to_file(path)
            .map_err(|e| PyIOError::new_err(format!("Failed to save file: {e}")))
    }

    /// Loads a map from a relocatable binary image file.
    #[staticmethod]
    pub fn load_from_file(path: &str) -> PyResult<Self> {
        let inner = InnerBlobMap::load_from_file(path)
            .map_err(|e| PyIOError::new_err(format!("Failed to load file: {e}")))?;
        Ok(Self { inner })
    }

    /// String representation for Python.
    pub fn __repr__(&self) -> String {
        format!(
            "ExpanseBlobMap(len={}, mem_used={})",
            self.inner.len(),
            self.inner.mem_used()
        )
    }
}
