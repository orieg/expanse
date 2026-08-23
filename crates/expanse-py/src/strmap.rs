//! PyO3 wrapper for ExpanseStrMap (ordered, prefix-compressed string map, compat: JudySL).

use crate::buffer::extract_str_key;
use expanse_trie::strmap::ExpanseStrMap as InnerStrMap;
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use pyo3::types::PyDictMethods;

/// A sorted map from NUL-free byte strings / UTF-8 strings to 64-bit unsigned integers (compat: JudySL).
///
/// Iteration order is byte-lexicographical. Keys are prefix-compressed across 8-byte word boundaries.
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct ExpanseStrMap {
    pub(crate) inner: InnerStrMap,
}

#[pymethods]
impl ExpanseStrMap {
    /// Creates an empty string map.
    #[new]
    pub fn new() -> Self {
        Self {
            inner: InnerStrMap::new(),
        }
    }

    /// Number of strings stored in the map.
    pub fn __len__(&self) -> usize {
        self.inner.len() as usize
    }

    /// True when no strings are stored.
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
        let k = extract_str_key(key)?;
        Ok(self.inner.get(&k).is_some())
    }

    /// Returns True if key is present in the map.
    pub fn contains_key(&self, key: &Bound<'_, PyAny>) -> PyResult<bool> {
        self.__contains__(key)
    }

    /// Retrieves `val = map[key]`; raises `KeyError` if key is missing.
    pub fn __getitem__(&self, key: &Bound<'_, PyAny>) -> PyResult<u64> {
        let k = extract_str_key(key)?;
        self.inner
            .get(&k)
            .ok_or_else(|| PyKeyError::new_err("Key not found in ExpanseStrMap"))
    }

    /// Sets `map[key] = val`.
    pub fn __setitem__(&mut self, key: &Bound<'_, PyAny>, val: u64) -> PyResult<()> {
        let k = extract_str_key(key)?;
        self.inner.insert(&k, val);
        Ok(())
    }

    /// Deletes `del map[key]`; raises `KeyError` if key is missing.
    pub fn __delitem__(&mut self, key: &Bound<'_, PyAny>) -> PyResult<()> {
        let k = extract_str_key(key)?;
        if self.inner.remove(&k).is_some() {
            Ok(())
        } else {
            Err(PyKeyError::new_err("Key not found in ExpanseStrMap"))
        }
    }

    /// Look up `key`, returning `default` (or None) if absent.
    #[pyo3(signature = (key, default=None))]
    pub fn get(&self, key: &Bound<'_, PyAny>, default: Option<u64>) -> PyResult<Option<u64>> {
        let k = extract_str_key(key)?;
        Ok(self.inner.get(&k).or(default))
    }

    /// Inserts `key -> val`; returns previous value, if any.
    pub fn insert(&mut self, key: &Bound<'_, PyAny>, val: u64) -> PyResult<Option<u64>> {
        let k = extract_str_key(key)?;
        Ok(self.inner.insert(&k, val))
    }

    /// Removes `key`; returns its value or None if missing.
    pub fn remove(&mut self, key: &Bound<'_, PyAny>) -> PyResult<Option<u64>> {
        let k = extract_str_key(key)?;
        Ok(self.inner.remove(&k))
    }

    /// Removes `key` and returns its value. If `key` is absent, returns `default`
    /// if given, otherwise raises `KeyError`.
    #[pyo3(signature = (key, default=None))]
    pub fn pop(&mut self, key: &Bound<'_, PyAny>, default: Option<u64>) -> PyResult<u64> {
        let k = extract_str_key(key)?;
        match self.inner.remove(&k) {
            Some(v) => Ok(v),
            None => default.ok_or_else(|| PyKeyError::new_err("Key not found in ExpanseStrMap")),
        }
    }

    /// Removes all strings and frees allocated trie nodes.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Heap bytes used by the prefix trie.
    pub fn mem_used(&self) -> usize {
        self.inner.mem_used()
    }

    /// Smallest entry `(key, value)` in byte-lexicographical order.
    pub fn first(&mut self) -> Option<(String, u64)> {
        let (bytes, slot) = self.inner.first()?;
        // SAFETY: `slot` is guaranteed non-null and points to a valid value by `ExpanseStrMap`.
        let val = unsafe { *slot.as_ptr() };
        let s = String::from_utf8_lossy(&bytes).into_owned();
        Some((s, val))
    }

    /// Largest entry `(key, value)` in byte-lexicographical order.
    pub fn last(&mut self) -> Option<(String, u64)> {
        let (bytes, slot) = self.inner.last()?;
        // SAFETY: `slot` is guaranteed non-null and points to a valid value by `ExpanseStrMap`.
        let val = unsafe { *slot.as_ptr() };
        let s = String::from_utf8_lossy(&bytes).into_owned();
        Some((s, val))
    }

    /// Smallest entry with key `> key` (or `>= key` if `inclusive=True`).
    #[pyo3(signature = (key, inclusive=false))]
    pub fn next(
        &mut self,
        key: &Bound<'_, PyAny>,
        inclusive: bool,
    ) -> PyResult<Option<(String, u64)>> {
        let k = extract_str_key(key)?;
        let entry = if inclusive {
            self.inner.next_at_or_after(&k)
        } else {
            self.inner.next_after(&k)
        };
        Ok(entry.map(|(bytes, slot)| {
            // SAFETY: `slot` is guaranteed non-null and points to a valid value by `ExpanseStrMap`.
            let val = unsafe { *slot.as_ptr() };
            (String::from_utf8_lossy(&bytes).into_owned(), val)
        }))
    }

    /// Smallest entry with key `>= key`.
    pub fn next_at_or_after(&mut self, key: &Bound<'_, PyAny>) -> PyResult<Option<(String, u64)>> {
        self.next(key, true)
    }

    /// Smallest entry with key `> key`.
    pub fn next_after(&mut self, key: &Bound<'_, PyAny>) -> PyResult<Option<(String, u64)>> {
        self.next(key, false)
    }

    /// Largest entry with key `< key` (or `<= key` if `inclusive=True`).
    #[pyo3(signature = (key, inclusive=false))]
    pub fn prev(
        &mut self,
        key: &Bound<'_, PyAny>,
        inclusive: bool,
    ) -> PyResult<Option<(String, u64)>> {
        let k = extract_str_key(key)?;
        let entry = if inclusive {
            self.inner.prev_at_or_before(&k)
        } else {
            self.inner.prev_before(&k)
        };
        Ok(entry.map(|(bytes, slot)| {
            // SAFETY: `slot` is guaranteed non-null and points to a valid value by `ExpanseStrMap`.
            let val = unsafe { *slot.as_ptr() };
            (String::from_utf8_lossy(&bytes).into_owned(), val)
        }))
    }

    /// Largest entry with key `<= key`.
    pub fn prev_at_or_before(&mut self, key: &Bound<'_, PyAny>) -> PyResult<Option<(String, u64)>> {
        self.prev(key, true)
    }

    /// Largest entry with key `< key`.
    pub fn prev_before(&mut self, key: &Bound<'_, PyAny>) -> PyResult<Option<(String, u64)>> {
        self.prev(key, false)
    }

    fn collect_entries(&mut self) -> Vec<(String, u64)> {
        let mut items = Vec::with_capacity(self.inner.len() as usize);
        let mut cur: Option<Vec<u8>> = None;
        loop {
            let next_entry = match &cur {
                None => self.inner.first(),
                Some(prev_k) => self.inner.next_after(prev_k),
            };
            match next_entry {
                Some((bytes, slot)) => {
                    // SAFETY: `slot` is guaranteed non-null and points to a valid value by `ExpanseStrMap`.
                    let val = unsafe { *slot.as_ptr() };
                    cur = Some(bytes.clone());
                    items.push((String::from_utf8_lossy(&bytes).into_owned(), val));
                }
                None => break,
            }
        }
        items
    }

    /// Key iterator for Python `for key in map:`.
    pub fn __iter__(&mut self) -> ExpanseStrMapKeyIter {
        let keys: Vec<String> = self.collect_entries().into_iter().map(|(k, _)| k).collect();
        ExpanseStrMapKeyIter { keys, index: 0 }
    }

    /// Returns an iterator of all keys in lexicographical order.
    pub fn keys(&mut self) -> Vec<String> {
        self.collect_entries().into_iter().map(|(k, _)| k).collect()
    }

    /// Returns an iterator of all values in key order.
    pub fn values(&mut self) -> Vec<u64> {
        self.collect_entries().into_iter().map(|(_, v)| v).collect()
    }

    /// Returns an iterator of `(key, value)` pairs in lexicographical order.
    pub fn items(&mut self) -> Vec<(String, u64)> {
        self.collect_entries()
    }

    /// Range scan over string keys returning list of pairs.
    #[pyo3(signature = (start=None, end=None, inclusive=true))]
    pub fn range(
        &mut self,
        start: Option<&Bound<'_, PyAny>>,
        end: Option<&Bound<'_, PyAny>>,
        inclusive: bool,
    ) -> PyResult<Vec<(String, u64)>> {
        let start_bytes = start.map(extract_str_key).transpose()?;
        let end_bytes = end.map(extract_str_key).transpose()?;

        let mut items = Vec::new();
        let mut cur = match &start_bytes {
            Some(sb) => self.inner.next_at_or_after(sb),
            None => self.inner.first(),
        };

        while let Some((bytes, slot)) = cur {
            if let Some(ref eb) = end_bytes {
                let cmp = bytes.as_slice().cmp(eb.as_ref());
                if inclusive && cmp > std::cmp::Ordering::Equal {
                    break;
                }
                if !inclusive && cmp >= std::cmp::Ordering::Equal {
                    break;
                }
            }
            // SAFETY: `slot` is guaranteed non-null and points to a valid value by `ExpanseStrMap`.
            let val = unsafe { *slot.as_ptr() };
            let key_str = String::from_utf8_lossy(&bytes).into_owned();
            items.push((key_str, val));
            cur = self.inner.next_after(&bytes);
        }

        Ok(items)
    }

    /// Updates the map from another mapping or iterable of `(key, value)` pairs.
    pub fn update(&mut self, other: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Ok(dict) = other.cast::<pyo3::types::PyDict>() {
            for (k, v) in dict.iter() {
                let k_bytes = extract_str_key(&k)?;
                let v: u64 = v.extract()?;
                self.inner.insert(&k_bytes, v);
            }
            return Ok(());
        }

        for item in other.try_iter()? {
            let item = item?;
            let (k_obj, v_obj): (Bound<'_, PyAny>, u64) = item.extract()?;
            let k_bytes = extract_str_key(&k_obj)?;
            self.inner.insert(&k_bytes, v_obj);
        }
        Ok(())
    }

    /// String representation.
    pub fn __repr__(&self) -> String {
        format!("ExpanseStrMap(len={})", self.inner.len())
    }
}

impl Default for ExpanseStrMap {
    fn default() -> Self {
        Self::new()
    }
}

/// Key iterator for [`ExpanseStrMap`].
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct ExpanseStrMapKeyIter {
    pub(crate) keys: Vec<String>,
    pub(crate) index: usize,
}

#[pymethods]
impl ExpanseStrMapKeyIter {
    /// Returns the iterator instance.
    pub fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Returns next string key.
    pub fn __next__(&mut self) -> Option<String> {
        if self.index < self.keys.len() {
            let item = self.keys[self.index].clone();
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Returns remaining key count.
    pub fn __len__(&self) -> usize {
        self.keys.len().saturating_sub(self.index)
    }
}

/// Value iterator for [`ExpanseStrMap`].
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct ExpanseStrMapValueIter {
    pub(crate) values: Vec<u64>,
    pub(crate) index: usize,
}

#[pymethods]
impl ExpanseStrMapValueIter {
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

/// Item iterator for [`ExpanseStrMap`].
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct ExpanseStrMapItemIter {
    pub(crate) items: Vec<(String, u64)>,
    pub(crate) index: usize,
}

#[pymethods]
impl ExpanseStrMapItemIter {
    /// Returns the iterator instance.
    pub fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Returns next `(key, value)` pair.
    pub fn __next__(&mut self) -> Option<(String, u64)> {
        if self.index < self.items.len() {
            let item = self.items[self.index].clone();
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Returns remaining item count.
    pub fn __len__(&self) -> usize {
        self.items.len().saturating_sub(self.index)
    }
}
