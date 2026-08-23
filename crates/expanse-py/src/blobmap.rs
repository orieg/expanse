//! PyO3 wrapper for ExpanseBlobMap (large-value map with inline packing and arena backing).

use crate::buffer::extract_bytes_key;
use expanse_trie::blobmap::ExpanseBlobMap as InnerBlobMap;
use pyo3::exceptions::{PyIOError, PyKeyError, PyRuntimeError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;

/// A high-performance map from 64-bit integer keys to arbitrary-length byte payloads
/// backed by inline polymorphic 64-bit value slots and chunked slab arenas.
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct ExpanseBlobMap {
    pub(crate) inner: InnerBlobMap,
}

#[pymethods]
impl ExpanseBlobMap {
    /// Creates an empty blob map, optionally with custom arena chunk size in bytes.
    #[new]
    #[pyo3(signature = (chunk_size=None))]
    pub fn new(chunk_size: Option<usize>) -> Self {
        let inner = match chunk_size {
            Some(sz) => InnerBlobMap::with_chunk_size(sz),
            None => InnerBlobMap::new(),
        };
        Self { inner }
    }

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

    /// Returns True if key exists in the map.
    pub fn contains_key(&self, key: u64) -> bool {
        self.inner.contains_key(key)
    }

    /// Inserts a key-blob pair with optional 32-bit hot metadata.
    #[pyo3(signature = (key, data, hot_meta=0))]
    pub fn insert(&mut self, key: u64, data: &Bound<'_, PyAny>, hot_meta: u32) -> PyResult<()> {
        let bytes = extract_bytes_key(data)?;
        self.inner
            .insert(key, &bytes, hot_meta)
            .map_err(|e| PyRuntimeError::new_err(format!("Blob allocation error: {e}")))
    }

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

    /// Removes a key from the map; returns True if key was present.
    pub fn remove(&mut self, key: u64) -> bool {
        self.inner.remove(key)
    }

    /// Clears all entries and resets the slab arena.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Returns total heap memory used by index and slab arena.
    pub fn mem_used(&self) -> usize {
        self.inner.mem_used()
    }

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

        self.inner.scan_filtered(
            start_key..=end_key,
            |key, meta| {
                if let Some(pred) = predicate {
                    match pred.call1((key, meta)) {
                        Ok(res) => res.is_truthy().unwrap_or(true),
                        Err(_) => true,
                    }
                } else {
                    true
                }
            },
            |key, view, meta| {
                let py_bytes = PyBytes::new(py, view.as_bytes());
                if let Some(cb) = callback {
                    match cb.call1((key, py_bytes.clone(), meta)) {
                        Ok(res) => res.is_truthy().unwrap_or(true),
                        Err(_) => false,
                    }
                } else {
                    results.push((key, py_bytes, meta));
                    true
                }
            },
        );

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
        let inner = InnerBlobMap::mmap_file(path)
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
