//! PyO3 wrapper for `SyncExpanseBlobMap`: the shared (multi-writer, GIL-free) blob map, the
//! Python surface of the C `expanse_sync_blob_map_*` handle (#1538).

use crate::blobmap::status_name;
use crate::buffer::extract_bytes_key;
use crate::reader_cache::detached_reader_cache;
use expanse_trie::blobmap::CompareExchangeError;
use expanse_trie::sync::{DetachedBlobReader, SyncExpanseBlobMap as InnerSyncBlobMap};
use pyo3::exceptions::{PyKeyError, PyRuntimeError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::borrow::Cow;
use std::sync::Arc;

// abi-parity: expanse_sync_blob_map_reader_new, expanse_sync_blob_map_reader_free
detached_reader_cache!(fn with_blob_reader, BLOB_READERS, InnerSyncBlobMap, DetachedBlobReader);

/// A `(payload, hot_meta)` value as the engine carries it.
type BlobValue = (Vec<u8>, u32);

/// A `(bytes, hot_meta)` value returned to Python.
type PyBlobValue<'py> = (Bound<'py, PyBytes>, u32);

/// A `(data, hot_meta)` value argument from Python.
type PyBlobArg<'py> = (Bound<'py, PyAny>, u32);

/// Converts an engine value into a Python `(bytes, int)` pair.
fn value_to_py<'py>(py: Python<'py>, v: Option<BlobValue>) -> Option<PyBlobValue<'py>> {
    v.map(|(data, meta)| (PyBytes::new(py, &data), meta))
}

/// Extracts an optional `(payload, hot_meta)` argument.
fn extract_value<'a>(
    v: &'a Option<(Bound<'_, PyAny>, u32)>,
) -> PyResult<Option<(Cow<'a, [u8]>, u32)>> {
    v.as_ref()
        .map(|(data, meta)| Ok((extract_bytes_key(data)?, *meta)))
        .transpose()
}

// abi-parity: expanse_sync_blob_map_free
/// A thread-safe, concurrent map from 64-bit integer keys to arbitrary-length byte payloads
/// with a 32-bit `hot_meta` (`SyncExpanseBlobMap`), with optimistic concurrency control.
///
/// Every operation releases the GIL. Point reads (`get`, `get_meta`, `contains`) run on the
/// optimistic path through a reader this thread registers once per map and return the payload
/// as an owned `bytes`. Writers from any thread run concurrently with each other and with
/// readers.
///
/// Metadata: only the low 24 bits of `hot_meta` are stored beside an arena payload, and a
/// payload of 7 bytes or fewer, or one the map compresses into its value slot, is stored with
/// no metadata and reads back `hot_meta == 0` (issue #1548). A `hot_meta` wider than 24 bits on
/// an arena payload is refused (`"meta_overflow"`).
///
/// Statuses (as `expanse_blob_status_t`): `"ok"`, `"mismatch"`, `"meta_overflow"`,
/// `"allocation_failed"`, `"cap_refused"` (the capacity cap refused a chunk and nothing was
/// compacted), `"arena_full"` (the call compacted and the record still does not fit) and
/// `"no_meta_field"` (the comparison held but the value has no metadata field).
#[pyclass(from_py_object, module = "expanse_trie._expanse")]
#[derive(Clone)]
pub struct SyncExpanseBlobMap {
    pub(crate) inner: Arc<InnerSyncBlobMap>,
}

#[pymethods]
impl SyncExpanseBlobMap {
    // abi-parity: expanse_sync_blob_map_new, expanse_sync_blob_map_new_with_capacity
    /// Creates an empty shared blob map, optionally with a custom arena chunk size in bytes and
    /// an arena capacity cap `max_capacity` in bytes of allocated chunks (default 1 GiB;
    /// clamped to `[chunk_size, 64 GiB]`).
    #[new]
    #[pyo3(signature = (chunk_size=None, max_capacity=None))]
    pub fn new(chunk_size: Option<usize>, max_capacity: Option<usize>) -> Self {
        let inner = match (chunk_size, max_capacity) {
            (Some(sz), None) => InnerSyncBlobMap::with_chunk_size(sz),
            (None, None) => InnerSyncBlobMap::new(),
            (sz, Some(cap)) => InnerSyncBlobMap::with_chunk_size_and_max_capacity(
                sz.unwrap_or(expanse_trie::blobmap::DEFAULT_CHUNK_SIZE),
                cap,
            ),
        };
        Self {
            inner: Arc::new(inner),
        }
    }

    // abi-parity: expanse_sync_blob_map_len
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

    // abi-parity: expanse_sync_blob_map_mem_used
    /// Heap bytes used by the index and the arena, read with writers excluded; waits for a
    /// running compaction (releases the GIL).
    pub fn mem_used(&self, py: Python<'_>) -> usize {
        py.detach(|| self.inner.mem_used())
    }

    // abi-parity: expanse_sync_blob_map_mem_used_estimate
    /// As `mem_used`, but takes no lock and waits for nothing: an estimate that may lag a
    /// concurrent writer (releases the GIL).
    pub fn mem_used_estimate(&self, py: Python<'_>) -> usize {
        py.detach(|| self.inner.mem_used_estimate())
    }

    // abi-parity: expanse_sync_blob_map_mem_held
    /// Heap bytes the map holds from the global allocator, including blocks its epoch collector
    /// keeps for reuse. Read with writers excluded (releases the GIL).
    pub fn mem_held(&self, py: Python<'_>) -> usize {
        py.detach(|| self.inner.mem_held())
    }

    // abi-parity: expanse_sync_blob_map_shrink_to_fit
    /// Returns the collector's freed index blocks to the global allocator and returns the bytes
    /// released. Runs beside readers and writers (releases the GIL).
    pub fn shrink_to_fit(&self, py: Python<'_>) -> usize {
        py.detach(|| self.inner.shrink_to_fit())
    }

    // abi-parity: expanse_sync_blob_map_set_reclaim_at_cap
    /// Turns the reclaim at the capacity cap on (the default) or off. Off, an insert the cap
    /// refuses compacts nothing.
    pub fn set_reclaim_at_cap(&self, on: bool) {
        self.inner.set_reclaim_at_cap(on);
    }

    /// True if an insert the capacity cap refuses may compact the arena.
    pub fn reclaim_at_cap(&self) -> bool {
        self.inner.reclaim_at_cap()
    }

    // abi-parity: expanse_sync_blob_map_compact
    /// Compacts the arena, returning `(live_bytes_before, live_bytes_after,
    /// total_allocated_before, total_allocated_after)`. Every other writer waits for it; readers
    /// keep reading while it copies (releases the GIL). Raises `RuntimeError` on a refusal.
    pub fn compact(&self, py: Python<'_>) -> PyResult<(usize, usize, usize, usize)> {
        let stats = py
            .detach(|| self.inner.compact())
            .map_err(|e| PyRuntimeError::new_err(format!("Compaction error: {e}")))?;
        Ok((
            stats.live_bytes_before,
            stats.live_bytes_after,
            stats.total_allocated_before,
            stats.total_allocated_after,
        ))
    }

    /// Membership test `key in map`.
    pub fn __contains__(&self, py: Python<'_>, key: u64) -> bool {
        self.contains(py, key)
    }

    // abi-parity: expanse_sync_blob_map_contains, expanse_sync_blob_map_reader_contains
    /// Returns True if `key` is present (optimistic read, releases the GIL).
    pub fn contains(&self, py: Python<'_>, key: u64) -> bool {
        py.detach(|| with_blob_reader(&self.inner, |r| r.contains(&self.inner, key)))
    }

    /// Returns True if `key` is present.
    pub fn contains_key(&self, py: Python<'_>, key: u64) -> bool {
        self.contains(py, key)
    }

    // abi-parity: expanse_sync_blob_map_get_into, expanse_sync_blob_map_reader_get_into
    /// Retrieves `(bytes_payload, hot_meta)` for `key`, or None if absent (optimistic read,
    /// releases the GIL). The payload is an owned copy of any length, so there is no buffer to
    /// size.
    pub fn get<'py>(&self, py: Python<'py>, key: u64) -> Option<PyBlobValue<'py>> {
        let v = py.detach(|| with_blob_reader(&self.inner, |r| r.get(&self.inner, key)));
        value_to_py(py, v)
    }

    /// Retrieves only the byte payload for `key`, or None if absent.
    pub fn get_bytes<'py>(&self, py: Python<'py>, key: u64) -> Option<Bound<'py, PyBytes>> {
        self.get(py, key).map(|(data, _)| data)
    }

    // abi-parity: expanse_sync_blob_map_get_meta, expanse_sync_blob_map_reader_get_meta
    /// Retrieves only the `hot_meta` of `key`, or None if absent. Never touches payload memory
    /// (optimistic read, releases the GIL).
    pub fn get_meta(&self, py: Python<'_>, key: u64) -> Option<u32> {
        py.detach(|| with_blob_reader(&self.inner, |r| r.get_meta(&self.inner, key)))
    }

    /// Retrieves `val = map[key]`; raises `KeyError` if key is missing.
    pub fn __getitem__<'py>(&self, py: Python<'py>, key: u64) -> PyResult<Bound<'py, PyBytes>> {
        self.get_bytes(py, key).ok_or_else(|| {
            PyKeyError::new_err(format!("Key {key} not found in SyncExpanseBlobMap"))
        })
    }

    /// Sets `map[key] = data` (with hot_meta = 0).
    pub fn __setitem__(&self, py: Python<'_>, key: u64, data: &Bound<'_, PyAny>) -> PyResult<()> {
        self.insert(py, key, data, 0)
    }

    /// Deletes `del map[key]`; raises `KeyError` if key is missing.
    pub fn __delitem__(&self, py: Python<'_>, key: u64) -> PyResult<()> {
        if self.remove(py, key) {
            Ok(())
        } else {
            Err(PyKeyError::new_err(format!(
                "Key {key} not found in SyncExpanseBlobMap"
            )))
        }
    }

    // abi-parity: expanse_sync_blob_map_insert
    /// Inserts a key-blob pair with optional 32-bit hot metadata, releasing the GIL. Raises
    /// `RuntimeError` when the insert is refused (`insert_status` reports which refusal).
    #[pyo3(signature = (key, data, hot_meta=0))]
    pub fn insert(
        &self,
        py: Python<'_>,
        key: u64,
        data: &Bound<'_, PyAny>,
        hot_meta: u32,
    ) -> PyResult<()> {
        let bytes = extract_bytes_key(data)?;
        py.detach(|| self.inner.insert(key, &bytes, hot_meta))
            .map_err(|e| PyRuntimeError::new_err(format!("Blob allocation error: {e}")))
    }

    /// As `insert`, but returns a status string instead of raising on a refusal: `"ok"`,
    /// `"meta_overflow"`, `"allocation_failed"`, `"cap_refused"` or `"arena_full"`.
    #[pyo3(signature = (key, data, hot_meta=0))]
    pub fn insert_status(
        &self,
        py: Python<'_>,
        key: u64,
        data: &Bound<'_, PyAny>,
        hot_meta: u32,
    ) -> PyResult<&'static str> {
        let bytes = extract_bytes_key(data)?;
        Ok(
            match py.detach(|| self.inner.insert(key, &bytes, hot_meta)) {
                Ok(()) => "ok",
                Err(e) => status_name(e),
            },
        )
    }

    // abi-parity: expanse_sync_blob_map_remove
    /// Removes `key` releasing the GIL; returns True if it was present.
    pub fn remove(&self, py: Python<'_>, key: u64) -> bool {
        py.detach(|| self.inner.remove(key))
    }

    /// Removes all entries and resets the arena (releases the GIL).
    pub fn clear(&self, py: Python<'_>) {
        py.detach(|| self.inner.clear());
    }

    // abi-parity: expanse_sync_blob_map_compare_exchange
    /// Atomic compare-and-swap of a whole `(payload, hot_meta)` value, releasing the GIL.
    ///
    /// `expected` and `new` are `(data, hot_meta)` tuples or `None` for "absent", so
    /// `(None, v)` is insert-if-absent and `(e, None)` is remove-if-equals. Returns
    /// `(status, observed)`: `"ok"` when the store happened (`observed` is `expected`);
    /// `"mismatch"` when the current value is not `expected` (`observed` is the current
    /// `(bytes, hot_meta)` or None); or a refusal status when the current value WAS `expected`
    /// but `new` cannot be stored (`"meta_overflow"`, `"allocation_failed"`, `"cap_refused"`,
    /// `"arena_full"`, `"no_meta_field"`), with nothing changed and `observed` None. A
    /// retry loop retries on `"mismatch"` and stops on anything else.
    ///
    /// A payload of 7 bytes or fewer reads back `hot_meta == 0` whatever was inserted (issue
    /// #1548), so expecting the metadata it was inserted with reports `"mismatch"`.
    pub fn compare_exchange<'py>(
        &self,
        py: Python<'py>,
        key: u64,
        expected: Option<PyBlobArg<'py>>,
        new: Option<PyBlobArg<'py>>,
    ) -> PyResult<(&'static str, Option<PyBlobValue<'py>>)> {
        let exp = extract_value(&expected)?;
        let newv = extract_value(&new)?;
        let res = py.detach(|| {
            self.inner.compare_exchange(
                key,
                exp.as_ref().map(|(d, m)| (&d[..], *m)),
                newv.as_ref().map(|(d, m)| (&d[..], *m)),
            )
        });
        Ok(match res {
            Ok(_) => ("ok", exp.map(|(d, m)| (PyBytes::new(py, &d), m))),
            Err(CompareExchangeError::Mismatch(seen)) => ("mismatch", value_to_py(py, seen)),
            Err(CompareExchangeError::Unstorable(cause)) => (status_name(cause), None),
        })
    }

    // abi-parity: expanse_sync_blob_map_compare_exchange_meta
    /// Atomic compare-and-swap of only the 24-bit `hot_meta`, with no payload copy and no
    /// allocation, releasing the GIL: the form for a retry loop whose state lives in the
    /// metadata (a sequence number, a status tag). `expected` and `new` are an int or `None` for
    /// "absent"; `new=None` removes the key. Returns `(status, observed)`: `"ok"`; `"mismatch"`
    /// with the current `hot_meta` (or None if absent); `"meta_overflow"` for a `new` wider than
    /// 24 bits; or `"no_meta_field"` when the comparison held but the value has no field (the
    /// key is absent, or `new` is non-zero for a payload stored in its slot word).
    pub fn compare_exchange_meta(
        &self,
        py: Python<'_>,
        key: u64,
        expected: Option<u32>,
        new: Option<u32>,
    ) -> (&'static str, Option<u32>) {
        match py.detach(|| self.inner.compare_exchange_meta(key, expected, new)) {
            Ok(_) => ("ok", expected),
            Err(CompareExchangeError::Mismatch(seen)) => ("mismatch", seen),
            Err(CompareExchangeError::Unstorable(cause)) => (status_name(cause), None),
        }
    }

    // abi-parity: expanse_sync_blob_map_get_or_insert
    /// Stores `(data, hot_meta)` under `key` only if `key` is absent, releasing the GIL, as one
    /// linearizable step. Returns `(status, existing)`: `("ok", None)` when stored;
    /// `("mismatch", (bytes, hot_meta))` when the key was present (left untouched); or a
    /// refusal status with `None` when the key was absent but the value cannot be stored. Of
    /// any number of callers racing on one absent key, exactly one gets `"ok"`.
    #[pyo3(signature = (key, data, hot_meta=0))]
    pub fn get_or_insert<'py>(
        &self,
        py: Python<'py>,
        key: u64,
        data: &Bound<'py, PyAny>,
        hot_meta: u32,
    ) -> PyResult<(&'static str, Option<PyBlobValue<'py>>)> {
        let bytes = extract_bytes_key(data)?;
        Ok(
            match py.detach(|| self.inner.get_or_insert(key, &bytes, hot_meta)) {
                Ok(None) => ("ok", None),
                Ok(seen) => ("mismatch", value_to_py(py, seen)),
                Err(cause) => (status_name(cause), None),
            },
        )
    }

    /// Read-modify-write on one key: replaces the value with `fn(current)`, where `current` is
    /// `(bytes, hot_meta)` or `None` if absent, and `fn` returns the new `(data, hot_meta)` or
    /// `None` to remove the key. Returns the value that was replaced (`None` if the key was
    /// absent).
    ///
    /// A retry loop over `compare_exchange` that holds the GIL only while `fn` runs, so `fn`
    /// may run several times for one call and must be free of side effects. An exception
    /// raised by `fn` propagates and leaves the key unchanged; a refusal of the new value
    /// raises `RuntimeError`.
    pub fn update_with<'py>(
        &self,
        py: Python<'py>,
        key: u64,
        r#fn: &Bound<'py, PyAny>,
    ) -> PyResult<Option<PyBlobValue<'py>>> {
        let mut current = py.detach(|| with_blob_reader(&self.inner, |r| r.get(&self.inner, key)));
        loop {
            let arg = value_to_py(py, current.clone());
            let ret = r#fn.call1((arg,))?;
            let new: Option<PyBlobArg<'py>> = ret.extract()?;
            let newv = extract_value(&new)?;
            let res = py.detach(|| {
                self.inner.compare_exchange(
                    key,
                    current.as_ref().map(|(d, m)| (&d[..], *m)),
                    newv.as_ref().map(|(d, m)| (&d[..], *m)),
                )
            });
            match res {
                Ok(prev) => return Ok(value_to_py(py, prev)),
                Err(CompareExchangeError::Mismatch(seen)) => current = seen,
                Err(CompareExchangeError::Unstorable(cause)) => {
                    return Err(PyRuntimeError::new_err(format!(
                        "Blob update refused: {}",
                        status_name(cause)
                    )));
                }
            }
        }
    }

    /// String representation for Python.
    pub fn __repr__(&self) -> String {
        format!("SyncExpanseBlobMap(len={})", self.inner.len())
    }
}

impl Default for SyncExpanseBlobMap {
    fn default() -> Self {
        Self::new(None, None)
    }
}
