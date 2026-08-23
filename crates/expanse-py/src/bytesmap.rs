//! PyO3 wrapper for ExpanseBytesMap (unordered map over arbitrary byte strings, compat: JudyHS).

use crate::buffer::extract_bytes_key;
use expanse_trie::bytesmap::ExpanseBytesMap as InnerBytesMap;
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::types::PyDictMethods;

/// A sparse, dynamic, unordered map from arbitrary byte keys (including NUL bytes) to 64-bit unsigned integers (compat: JudyHS).
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct ExpanseBytesMap {
    pub(crate) inner: InnerBytesMap,
}

#[pymethods]
impl ExpanseBytesMap {
    /// Creates an empty byte map.
    #[new]
    pub fn new() -> Self {
        Self {
            inner: InnerBytesMap::new(),
        }
    }

    /// Number of entries stored in the map.
    pub fn __len__(&self) -> usize {
        self.inner.len() as usize
    }

    /// True when no entries are in the map.
    /// True when empty.
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

    /// Returns True if key exists in the map.
    pub fn contains_key(&self, key: &Bound<'_, PyAny>) -> PyResult<bool> {
        self.__contains__(key)
    }

    /// Retrieves `val = map[key]`; raises `KeyError` if key is missing.
    pub fn __getitem__(&self, key: &Bound<'_, PyAny>) -> PyResult<u64> {
        let k = extract_bytes_key(key)?;
        self.inner
            .get(&k)
            .ok_or_else(|| PyKeyError::new_err("Key not found in ExpanseBytesMap"))
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
            Err(PyKeyError::new_err("Key not found in ExpanseBytesMap"))
        }
    }

    /// Look up `key`, returning `default` (or None) if absent.
    #[pyo3(signature = (key, default=None))]
    pub fn get(&self, key: &Bound<'_, PyAny>, default: Option<u64>) -> PyResult<Option<u64>> {
        let k = extract_bytes_key(key)?;
        Ok(self.inner.get(&k).or(default))
    }

    /// Inserts `key -> val`; returns previous value, if any.
    pub fn insert(&mut self, key: &Bound<'_, PyAny>, val: u64) -> PyResult<Option<u64>> {
        let k = extract_bytes_key(key)?;
        Ok(self.inner.insert(&k, val))
    }

    /// Removes `key`; returns its value or None if missing.
    pub fn remove(&mut self, key: &Bound<'_, PyAny>) -> PyResult<Option<u64>> {
        let k = extract_bytes_key(key)?;
        Ok(self.inner.remove(&k))
    }

    /// Removes `key` and returns its value. If `key` is absent, returns `default`
    /// if given, otherwise raises `KeyError`.
    #[pyo3(signature = (key, default=None))]
    pub fn pop(&mut self, key: &Bound<'_, PyAny>, default: Option<u64>) -> PyResult<u64> {
        let k = extract_bytes_key(key)?;
        match self.inner.remove(&k) {
            Some(v) => Ok(v),
            None => default.ok_or_else(|| PyKeyError::new_err("Key not found in ExpanseBytesMap")),
        }
    }

    /// Removes all entries and releases memory.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Heap bytes used by the hash trie and buckets.
    pub fn mem_used(&self) -> usize {
        self.inner.mem_used()
    }

    /// Key iterator for Python `for key in map:`.
    pub fn __iter__(&self) -> ExpanseBytesMapKeyIter {
        let mut keys = Vec::with_capacity(self.inner.len() as usize);
        self.inner.for_each(|k, _| {
            keys.push(k.to_vec());
        });
        ExpanseBytesMapKeyIter { keys, index: 0 }
    }

    /// Returns an iterator of all byte keys.
    pub fn keys(&self) -> ExpanseBytesMapKeyIter {
        self.__iter__()
    }

    /// Returns an iterator of all values.
    pub fn values(&self) -> ExpanseBytesMapValueIter {
        let mut values = Vec::with_capacity(self.inner.len() as usize);
        self.inner.for_each(|_, v| {
            values.push(v);
        });
        ExpanseBytesMapValueIter { values, index: 0 }
    }

    /// Returns an iterator of `(key, value)` pairs.
    pub fn items(&self) -> ExpanseBytesMapItemIter {
        let mut items = Vec::with_capacity(self.inner.len() as usize);
        self.inner.for_each(|k, v| {
            items.push((k.to_vec(), v));
        });
        ExpanseBytesMapItemIter { items, index: 0 }
    }

    /// Updates the map from another mapping or iterable of `(key, value)` pairs.
    pub fn update(&mut self, other: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Ok(dict) = other.cast::<pyo3::types::PyDict>() {
            for (k, v) in dict.iter() {
                let k_bytes = extract_bytes_key(&k)?;
                let v: u64 = v.extract()?;
                self.inner.insert(&k_bytes, v);
            }
            return Ok(());
        }

        for item in other.try_iter()? {
            let item = item?;
            let (k_obj, v_obj): (Bound<'_, PyAny>, u64) = item.extract()?;
            let k_bytes = extract_bytes_key(&k_obj)?;
            self.inner.insert(&k_bytes, v_obj);
        }
        Ok(())
    }

    /// String representation.
    pub fn __repr__(&self) -> String {
        format!("ExpanseBytesMap(len={})", self.inner.len())
    }
}

impl Default for ExpanseBytesMap {
    fn default() -> Self {
        Self::new()
    }
}

/// Key iterator for [`ExpanseBytesMap`].
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct ExpanseBytesMapKeyIter {
    pub(crate) keys: Vec<Vec<u8>>,
    pub(crate) index: usize,
}

#[pymethods]
impl ExpanseBytesMapKeyIter {
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

/// Value iterator for [`ExpanseBytesMap`].
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct ExpanseBytesMapValueIter {
    pub(crate) values: Vec<u64>,
    pub(crate) index: usize,
}

#[pymethods]
impl ExpanseBytesMapValueIter {
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

/// Item iterator for [`ExpanseBytesMap`].
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct ExpanseBytesMapItemIter {
    pub(crate) items: Vec<(Vec<u8>, u64)>,
    pub(crate) index: usize,
}

#[pymethods]
impl ExpanseBytesMapItemIter {
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
