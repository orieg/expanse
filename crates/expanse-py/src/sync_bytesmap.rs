//! PyO3 wrapper for `SyncExpanseBytesMap`: the shared (multi-writer, GIL-free) unordered bytes
//! map, the Python surface of the C `expanse_sync_bytesmap_*` handle (#1538).

use crate::buffer::extract_bytes_key;
use crate::reader_cache::detached_reader_cache;
use expanse_trie::sync::{DetachedBytesReader, SyncExpanseBytesMap as InnerSyncBytesMap};
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use std::sync::Arc;

// abi-parity: expanse_sync_bytesmap_reader_new, expanse_sync_bytesmap_reader_free
detached_reader_cache!(fn with_bytes_reader, BYTES_READERS, InnerSyncBytesMap, DetachedBytesReader);

// abi-parity: expanse_sync_bytesmap_free
/// A thread-safe, concurrent unordered map from arbitrary byte strings to 64-bit unsigned
/// integers (`SyncExpanseBytesMap`, the JudyHS analogue), with optimistic concurrency control.
///
/// Every operation releases the GIL. Reads (`get`, `contains`) run on the optimistic path
/// through a reader this thread registers once per map; writers from any thread run
/// concurrently with each other and with readers. Keys are `bytes`, `bytearray` or `str`
/// (UTF-8 encoded) and may contain any byte, including NUL. The map is unordered and has no
/// ordered reads; use `SyncExpanseOrderedBytesMap` for those. Its hasher is the standard
/// per-process random one.
#[pyclass(from_py_object, module = "expanse_trie._expanse")]
#[derive(Clone)]
pub struct SyncExpanseBytesMap {
    pub(crate) inner: Arc<InnerSyncBytesMap>,
}

#[pymethods]
impl SyncExpanseBytesMap {
    // abi-parity: expanse_sync_bytesmap_new
    /// Creates an empty shared bytes map.
    #[new]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(InnerSyncBytesMap::new()),
        }
    }

    // abi-parity: expanse_sync_bytesmap_len
    /// Number of entries (GIL-free read).
    pub fn __len__(&self, py: Python<'_>) -> usize {
        py.detach(|| self.inner.len() as usize)
    }

    /// True when no entries are present.
    pub fn is_empty(&self, py: Python<'_>) -> bool {
        py.detach(|| self.inner.is_empty())
    }

    /// Property returning True if empty.
    #[getter]
    pub fn empty(&self, py: Python<'_>) -> bool {
        py.detach(|| self.inner.is_empty())
    }

    /// Truth value testing for Python.
    pub fn __bool__(&self, py: Python<'_>) -> bool {
        py.detach(|| !self.inner.is_empty())
    }

    // abi-parity: expanse_sync_bytesmap_mem_used
    /// Heap bytes used by the map's nodes and leaves, read with writers excluded
    /// (releases the GIL; makes concurrent writers wait for it).
    pub fn mem_used(&self, py: Python<'_>) -> usize {
        py.detach(|| self.inner.mem_used())
    }

    // abi-parity: expanse_sync_bytesmap_mem_held
    /// Heap bytes the map holds from the global allocator: its tree's own share plus the blocks
    /// its epoch collector keeps for reuse or is waiting to reclaim. Read with writers excluded
    /// (releases the GIL).
    pub fn mem_held(&self, py: Python<'_>) -> usize {
        py.detach(|| self.inner.mem_held())
    }

    // abi-parity: expanse_sync_bytesmap_shrink_to_fit
    /// Returns the freed blocks the collector keeps for reuse to the global allocator and
    /// returns the bytes released. Runs beside readers and writers (releases the GIL).
    pub fn shrink_to_fit(&self, py: Python<'_>) -> usize {
        py.detach(|| self.inner.shrink_to_fit())
    }

    /// Membership test `key in map`.
    pub fn __contains__(&self, py: Python<'_>, key: &Bound<'_, PyAny>) -> PyResult<bool> {
        self.contains(py, key)
    }

    // abi-parity: expanse_sync_bytesmap_contains, expanse_sync_bytesmap_reader_contains
    /// Returns True if `key` is present (optimistic read, releases the GIL).
    pub fn contains(&self, py: Python<'_>, key: &Bound<'_, PyAny>) -> PyResult<bool> {
        let k = extract_bytes_key(key)?;
        Ok(py.detach(|| with_bytes_reader(&self.inner, |r| r.contains(&self.inner, &k))))
    }

    /// Returns True if `key` is present.
    pub fn contains_key(&self, py: Python<'_>, key: &Bound<'_, PyAny>) -> PyResult<bool> {
        self.contains(py, key)
    }

    /// Retrieves `val = map[key]`; raises `KeyError` if key is missing.
    pub fn __getitem__(&self, py: Python<'_>, key: &Bound<'_, PyAny>) -> PyResult<u64> {
        self.get(py, key, None)?
            .ok_or_else(|| PyKeyError::new_err("Key not found in SyncExpanseBytesMap"))
    }

    /// Sets `map[key] = val`.
    pub fn __setitem__(&self, py: Python<'_>, key: &Bound<'_, PyAny>, val: u64) -> PyResult<()> {
        self.insert(py, key, val).map(|_| ())
    }

    /// Deletes `del map[key]`; raises `KeyError` if key is missing.
    pub fn __delitem__(&self, py: Python<'_>, key: &Bound<'_, PyAny>) -> PyResult<()> {
        if self.remove(py, key)?.is_some() {
            Ok(())
        } else {
            Err(PyKeyError::new_err("Key not found in SyncExpanseBytesMap"))
        }
    }

    // abi-parity: expanse_sync_bytesmap_get, expanse_sync_bytesmap_reader_get
    /// Look up `key` (optimistic read, releases the GIL), returning `default` (or None) if absent.
    #[pyo3(signature = (key, default=None))]
    pub fn get(
        &self,
        py: Python<'_>,
        key: &Bound<'_, PyAny>,
        default: Option<u64>,
    ) -> PyResult<Option<u64>> {
        let k = extract_bytes_key(key)?;
        Ok(py
            .detach(|| with_bytes_reader(&self.inner, |r| r.get(&self.inner, &k)))
            .or(default))
    }

    // abi-parity: expanse_sync_bytesmap_insert
    /// Inserts `key -> val` releasing the GIL; returns the previous value, if any.
    pub fn insert(
        &self,
        py: Python<'_>,
        key: &Bound<'_, PyAny>,
        val: u64,
    ) -> PyResult<Option<u64>> {
        let k = extract_bytes_key(key)?;
        Ok(py.detach(|| self.inner.insert(&k, val)))
    }

    // abi-parity: expanse_sync_bytesmap_remove
    /// Removes `key` releasing the GIL; returns its previous value, or `None` if absent.
    pub fn remove(&self, py: Python<'_>, key: &Bound<'_, PyAny>) -> PyResult<Option<u64>> {
        let k = extract_bytes_key(key)?;
        Ok(py.detach(|| self.inner.remove(&k)))
    }

    /// Removes `key` and returns its value. If `key` is absent, returns `default`
    /// if given, otherwise raises `KeyError`.
    #[pyo3(signature = (key, default=None))]
    pub fn pop(
        &self,
        py: Python<'_>,
        key: &Bound<'_, PyAny>,
        default: Option<u64>,
    ) -> PyResult<u64> {
        match self.remove(py, key)? {
            Some(v) => Ok(v),
            None => {
                default.ok_or_else(|| PyKeyError::new_err("Key not found in SyncExpanseBytesMap"))
            }
        }
    }

    /// Removes all entries (releases the GIL).
    pub fn clear(&self, py: Python<'_>) {
        py.detach(|| {
            self.inner.clear();
        });
    }

    // abi-parity: expanse_sync_bytesmap_compare_exchange
    /// Atomic compare-and-swap on one key, releasing the GIL: stores `new` under `key` only if
    /// the key's current value is `expected`, as one linearizable step.
    ///
    /// `None` means "absent" on both sides, so `(None, v)` is insert-if-absent and `(e, None)`
    /// is remove-if-equals. Returns `(stored, observed)`: `stored` is `True` when the store
    /// happened, and `observed` is the value the compare saw (equal to `expected` when
    /// `stored`), which a caller retries against.
    pub fn compare_exchange(
        &self,
        py: Python<'_>,
        key: &Bound<'_, PyAny>,
        expected: Option<u64>,
        new: Option<u64>,
    ) -> PyResult<(bool, Option<u64>)> {
        let k = extract_bytes_key(key)?;
        Ok(
            match py.detach(|| self.inner.compare_exchange(&k, expected, new)) {
                Ok(seen) => (true, seen),
                Err(seen) => (false, seen),
            },
        )
    }

    // abi-parity: expanse_sync_bytesmap_get_or_insert
    /// Stores `val` under `key` only if `key` is absent, releasing the GIL, as one linearizable
    /// step. Returns the value already present (left untouched), or `None` when `val` was
    /// stored. Of any number of callers racing on one absent key, exactly one gets `None`.
    pub fn get_or_insert(
        &self,
        py: Python<'_>,
        key: &Bound<'_, PyAny>,
        val: u64,
    ) -> PyResult<Option<u64>> {
        let k = extract_bytes_key(key)?;
        Ok(py.detach(|| self.inner.get_or_insert(&k, val)))
    }

    /// Read-modify-write on one key: replaces the value with `fn(current)`, where `current` is
    /// the value or `None` if absent, and `fn` returns the new value or `None` to remove the
    /// key. Returns the value that was replaced (`None` if the key was absent).
    ///
    /// A retry loop over `compare_exchange` that holds the GIL only while `fn` runs, so `fn`
    /// may run several times for one call and must be free of side effects. An exception
    /// raised by `fn` propagates and leaves the key unchanged.
    pub fn update_with(
        &self,
        py: Python<'_>,
        key: &Bound<'_, PyAny>,
        r#fn: &Bound<'_, PyAny>,
    ) -> PyResult<Option<u64>> {
        let k = extract_bytes_key(key)?;
        let mut expected = py.detach(|| with_bytes_reader(&self.inner, |r| r.get(&self.inner, &k)));
        loop {
            let new: Option<u64> = r#fn.call1((expected,))?.extract()?;
            match py.detach(|| self.inner.compare_exchange(&k, expected, new)) {
                Ok(prev) => return Ok(prev),
                Err(seen) => expected = seen,
            }
        }
    }

    /// String representation for Python.
    pub fn __repr__(&self) -> String {
        format!("SyncExpanseBytesMap(len={})", self.inner.len())
    }
}

impl Default for SyncExpanseBytesMap {
    fn default() -> Self {
        Self::new()
    }
}
