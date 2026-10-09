//! PyO3 wrapper for ExpanseOrderedBytesMap (ordered map over arbitrary byte strings, C ABI `expanse_ordered_bytesmap_*`).
//!
//! Keys compare as unsigned bytes, lexicographically; a key sorts before any longer key it prefixes.
//! Navigation returns each key as an owned `bytes` object, so no caller-sized buffer exists that
//! could truncate a key.

use crate::buffer::extract_bytes_key;
use expanse_trie::ordered_bytesmap::ExpanseOrderedBytesMap as InnerOrderedBytesMap;
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;

/// Converts an engine `(key, value)` entry into a Python `(bytes, int)` pair.
fn entry_to_py(py: Python<'_>, entry: Option<(Vec<u8>, u64)>) -> Option<(Py<PyAny>, u64)> {
    entry.map(|(k, v)| (PyBytes::new(py, &k).into_any().unbind(), v))
}

// abi-parity: expanse_ordered_bytesmap_free
/// An ordered map from arbitrary byte-string keys to 64-bit unsigned integers.
///
/// Keys compare as unsigned bytes in lexicographic order, so a key sorts before any longer key it
/// prefixes. Keys may be empty and may contain any byte, including `0x00` and `0xFF`. Unlike
/// `ExpanseBytesMap` (unordered), this map supports ordered navigation.
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct ExpanseOrderedBytesMap {
    pub(crate) inner: InnerOrderedBytesMap,
}

#[pymethods]
impl ExpanseOrderedBytesMap {
    // abi-parity: expanse_ordered_bytesmap_new
    /// Creates an empty ordered byte map.
    #[new]
    pub fn new() -> Self {
        Self {
            inner: InnerOrderedBytesMap::new(),
        }
    }

    // abi-parity: expanse_ordered_bytesmap_len
    /// Number of entries stored in the map.
    pub fn __len__(&self) -> usize {
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
    pub fn __contains__(&self, key: &Bound<'_, PyAny>) -> PyResult<bool> {
        let k = extract_bytes_key(key)?;
        Ok(self.inner.contains_key(&k))
    }

    // abi-parity: expanse_ordered_bytesmap_contains
    /// Returns True if key exists in the map.
    pub fn contains(&self, key: &Bound<'_, PyAny>) -> PyResult<bool> {
        self.__contains__(key)
    }

    /// Retrieves `val = map[key]`; raises `KeyError` if key is missing.
    pub fn __getitem__(&self, key: &Bound<'_, PyAny>) -> PyResult<u64> {
        let k = extract_bytes_key(key)?;
        self.inner
            .get(&k)
            .ok_or_else(|| PyKeyError::new_err("Key not found in ExpanseOrderedBytesMap"))
    }

    /// Sets `map[key] = val`.
    pub fn __setitem__(&mut self, key: &Bound<'_, PyAny>, val: u64) -> PyResult<()> {
        let k = extract_bytes_key(key)?;
        self.inner.insert(&k, val);
        Ok(())
    }

    /// Deletes `del map[key]`; raises `KeyError` if key is missing.
    pub fn __delitem__(&mut self, key: &Bound<'_, PyAny>) -> PyResult<()> {
        let k = extract_bytes_key(key)?;
        if self.inner.remove(&k).is_some() {
            Ok(())
        } else {
            Err(PyKeyError::new_err(
                "Key not found in ExpanseOrderedBytesMap",
            ))
        }
    }

    // abi-parity: expanse_ordered_bytesmap_get
    /// Look up `key`, returning `default` (or None) if absent.
    #[pyo3(signature = (key, default=None))]
    pub fn get(&self, key: &Bound<'_, PyAny>, default: Option<u64>) -> PyResult<Option<u64>> {
        let k = extract_bytes_key(key)?;
        Ok(self.inner.get(&k).or(default))
    }

    // abi-parity: expanse_ordered_bytesmap_insert
    /// Inserts `key -> val`; returns the previous value, if any.
    pub fn insert(&mut self, key: &Bound<'_, PyAny>, val: u64) -> PyResult<Option<u64>> {
        let k = extract_bytes_key(key)?;
        Ok(self.inner.insert(&k, val))
    }

    // abi-parity: expanse_ordered_bytesmap_remove
    /// Removes `key`; returns its value or None if missing.
    pub fn remove(&mut self, key: &Bound<'_, PyAny>) -> PyResult<Option<u64>> {
        let k = extract_bytes_key(key)?;
        Ok(self.inner.remove(&k))
    }

    // abi-parity: expanse_ordered_bytesmap_slot
    /// Value stored for `key`, or None when absent.
    ///
    /// Python has no writable value pointer, so this reads the slot's value; store through `map[key] = val`.
    pub fn slot(&self, key: &Bound<'_, PyAny>) -> PyResult<Option<u64>> {
        let k = extract_bytes_key(key)?;
        Ok(self.inner.get(&k))
    }

    // abi-parity: expanse_ordered_bytesmap_ins_slot
    /// Inserts `key` with value 0 if absent (an existing value is kept) and returns its value.
    ///
    /// Python has no writable value pointer; store through `map[key] = val`.
    pub fn ins_slot(&mut self, key: &Bound<'_, PyAny>) -> PyResult<u64> {
        let k = extract_bytes_key(key)?;
        match self.inner.get(&k) {
            Some(v) => Ok(v),
            None => {
                self.inner.insert(&k, 0);
                Ok(0)
            }
        }
    }

    // abi-parity: expanse_ordered_bytesmap_clear
    /// Removes all entries and releases memory.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    // abi-parity: expanse_ordered_bytesmap_mem_used
    /// Heap bytes used by the live structure.
    pub fn mem_used(&self) -> usize {
        self.inner.mem_used()
    }

    // abi-parity: expanse_ordered_bytesmap_mem_held
    /// Heap bytes held by the map: live structure plus retained free capacity.
    pub fn mem_held(&self) -> usize {
        self.inner.mem_held()
    }

    // abi-parity: expanse_ordered_bytesmap_shrink_to_fit
    /// Releases retained free capacity; returns the bytes released.
    pub fn shrink_to_fit(&mut self) -> usize {
        self.inner.shrink_to_fit()
    }

    // abi-parity: expanse_ordered_bytesmap_first
    /// Smallest entry `(key, value)` in byte-lexicographic order, or None when empty.
    pub fn first(&self, py: Python<'_>) -> Option<(Py<PyAny>, u64)> {
        entry_to_py(py, self.inner.first_entry())
    }

    // abi-parity: expanse_ordered_bytesmap_last
    /// Largest entry `(key, value)` in byte-lexicographic order, or None when empty.
    pub fn last(&self, py: Python<'_>) -> Option<(Py<PyAny>, u64)> {
        entry_to_py(py, self.inner.last_entry())
    }

    // abi-parity: expanse_ordered_bytesmap_next_at_or_after
    /// Smallest entry with key `>= key`, or None.
    pub fn next_at_or_after(
        &self,
        py: Python<'_>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Option<(Py<PyAny>, u64)>> {
        let k = extract_bytes_key(key)?;
        Ok(entry_to_py(py, self.inner.next_at_or_after_entry(&k)))
    }

    // abi-parity: expanse_ordered_bytesmap_next_after
    /// Smallest entry with key `> key`, or None.
    pub fn next_after(
        &self,
        py: Python<'_>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Option<(Py<PyAny>, u64)>> {
        let k = extract_bytes_key(key)?;
        Ok(entry_to_py(py, self.inner.next_after_entry(&k)))
    }

    // abi-parity: expanse_ordered_bytesmap_prev_at_or_before
    /// Largest entry with key `<= key`, or None.
    pub fn prev_at_or_before(
        &self,
        py: Python<'_>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Option<(Py<PyAny>, u64)>> {
        let k = extract_bytes_key(key)?;
        Ok(entry_to_py(py, self.inner.prev_at_or_before_entry(&k)))
    }

    // abi-parity: expanse_ordered_bytesmap_prev_before
    /// Largest entry with key `< key`, or None.
    pub fn prev_before(
        &self,
        py: Python<'_>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Option<(Py<PyAny>, u64)>> {
        let k = extract_bytes_key(key)?;
        Ok(entry_to_py(py, self.inner.prev_before_entry(&k)))
    }

    /// Key iterator for `for key in map:`, in ascending byte order.
    pub fn __iter__(&self) -> ExpanseOrderedBytesMapKeyIter {
        ExpanseOrderedBytesMapKeyIter {
            keys: self.inner.iter().map(|(k, _)| k).collect(),
            index: 0,
        }
    }

    /// Returns an iterator of all byte keys in ascending order.
    pub fn keys(&self) -> ExpanseOrderedBytesMapKeyIter {
        self.__iter__()
    }

    /// Returns an iterator of all values in ascending key order.
    pub fn values(&self) -> ExpanseOrderedBytesMapValueIter {
        ExpanseOrderedBytesMapValueIter {
            values: self.inner.iter().map(|(_, v)| v).collect(),
            index: 0,
        }
    }

    /// Returns an iterator of `(key, value)` pairs in ascending key order.
    pub fn items(&self) -> ExpanseOrderedBytesMapItemIter {
        ExpanseOrderedBytesMapItemIter {
            items: self.inner.iter().collect(),
            index: 0,
        }
    }

    /// String representation.
    pub fn __repr__(&self) -> String {
        format!("ExpanseOrderedBytesMap(len={})", self.inner.len())
    }
}

impl Default for ExpanseOrderedBytesMap {
    fn default() -> Self {
        Self::new()
    }
}

/// Key iterator for [`ExpanseOrderedBytesMap`].
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct ExpanseOrderedBytesMapKeyIter {
    pub(crate) keys: Vec<Vec<u8>>,
    pub(crate) index: usize,
}

#[pymethods]
impl ExpanseOrderedBytesMapKeyIter {
    /// Returns the iterator instance.
    pub fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Returns next byte key.
    pub fn __next__<'py>(&mut self, py: Python<'py>) -> Option<Bound<'py, PyBytes>> {
        if self.index < self.keys.len() {
            let item = &self.keys[self.index];
            self.index += 1;
            Some(PyBytes::new(py, item))
        } else {
            None
        }
    }

    /// Returns remaining key count.
    pub fn __len__(&self) -> usize {
        self.keys.len().saturating_sub(self.index)
    }
}

/// Value iterator for [`ExpanseOrderedBytesMap`].
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct ExpanseOrderedBytesMapValueIter {
    pub(crate) values: Vec<u64>,
    pub(crate) index: usize,
}

#[pymethods]
impl ExpanseOrderedBytesMapValueIter {
    /// Returns the iterator instance.
    pub fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Returns next value.
    pub fn __next__(&mut self) -> Option<u64> {
        if self.index < self.values.len() {
            let item = self.values[self.index];
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Returns remaining value count.
    pub fn __len__(&self) -> usize {
        self.values.len().saturating_sub(self.index)
    }
}

/// Item iterator for [`ExpanseOrderedBytesMap`].
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct ExpanseOrderedBytesMapItemIter {
    pub(crate) items: Vec<(Vec<u8>, u64)>,
    pub(crate) index: usize,
}

#[pymethods]
impl ExpanseOrderedBytesMapItemIter {
    /// Returns the iterator instance.
    pub fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Returns next `(key, value)` pair.
    pub fn __next__<'py>(&mut self, py: Python<'py>) -> Option<(Bound<'py, PyBytes>, u64)> {
        if self.index < self.items.len() {
            let (k, v) = &self.items[self.index];
            self.index += 1;
            Some((PyBytes::new(py, k), *v))
        } else {
            None
        }
    }

    /// Returns remaining item count.
    pub fn __len__(&self) -> usize {
        self.items.len().saturating_sub(self.index)
    }
}
