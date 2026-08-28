//! PyO3 wrapper for ExpanseMap (64-bit integer key/value trie map).

use crate::buffer::for_each_u64_pair;
use expanse_trie::map::ExpanseMap as InnerMap;
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use pyo3::types::PyDictMethods;

/// A sparse, dynamic 64-bit unsigned integer map (compat: JudyL).
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct ExpanseMap {
    pub(crate) inner: InnerMap,
}

#[pymethods]
impl ExpanseMap {
    /// Creates an empty map, optionally initialized from a dict or iterable of `(key, value)` pairs.
    #[new]
    #[pyo3(signature = (items=None))]
    pub fn new(items: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let mut map = InnerMap::new();
        if let Some(obj) = items {
            if let Ok(dict) = obj.cast::<pyo3::types::PyDict>() {
                for (k, v) in dict.iter() {
                    let key: u64 = k.extract()?;
                    let val: u64 = v.extract()?;
                    map.insert(key, val);
                }
            } else {
                for item in obj.try_iter()? {
                    let pair = item?;
                    let (key, val): (u64, u64) = pair.extract()?;
                    map.insert(key, val);
                }
            }
        }
        Ok(Self { inner: map })
    }

    /// Number of entries in the map.
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
    pub fn __contains__(&self, key: u64) -> bool {
        self.inner.contains_key(key)
    }

    /// Returns True if key is present in the map.
    pub fn contains_key(&self, key: u64) -> bool {
        self.inner.contains_key(key)
    }

    /// Retrieves `val = map[key]`; raises `KeyError` if key is missing.
    pub fn __getitem__(&self, key: u64) -> PyResult<u64> {
        self.inner
            .get(key)
            .ok_or_else(|| PyKeyError::new_err(format!("Key {key} not found in ExpanseMap")))
    }

    /// Sets `map[key] = val`.
    pub fn __setitem__(&mut self, key: u64, val: u64) {
        self.inner.insert(key, val);
    }

    /// Deletes `del map[key]`; raises `KeyError` if key is missing.
    pub fn __delitem__(&mut self, key: u64) -> PyResult<()> {
        if self.inner.remove(key).is_some() {
            Ok(())
        } else {
            Err(PyKeyError::new_err(format!(
                "Key {key} not found in ExpanseMap"
            )))
        }
    }

    /// Look up `key`, returning `default` (or None) if absent.
    #[pyo3(signature = (key, default=None))]
    pub fn get(&self, key: u64, default: Option<u64>) -> Option<u64> {
        self.inner.get(key).or(default)
    }

    /// Looks up a batch of keys, returning a list of values (or None for absent keys).
    pub fn get_batch(&self, keys: Vec<u64>) -> Vec<Option<u64>> {
        let mut out = vec![None; keys.len()];
        self.inner.get_batch(&keys, &mut out);
        out
    }

    /// Inserts `key -> val`; returns the previous value, if any.
    pub fn insert(&mut self, key: u64, val: u64) -> Option<u64> {
        self.inner.insert(key, val)
    }

    /// Removes `key`; returns its value or None if missing.
    pub fn remove(&mut self, key: u64) -> Option<u64> {
        self.inner.remove(key)
    }

    /// Removes `key` and returns its value. If `key` is absent, returns `default`
    /// if given, otherwise raises `KeyError`.
    #[pyo3(signature = (key, default=None))]
    pub fn pop(&mut self, key: u64, default: Option<u64>) -> PyResult<u64> {
        match self.inner.remove(key) {
            Some(v) => Ok(v),
            None => default
                .ok_or_else(|| PyKeyError::new_err(format!("Key {key} not found in ExpanseMap"))),
        }
    }

    /// Removes all entries from the map.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Heap bytes used by the map allocations.
    pub fn mem_used(&self) -> usize {
        self.inner.mem_used()
    }

    /// Total node allocations performed by the trie engine.
    pub fn total_node_allocs(&self) -> usize {
        self.inner.total_node_allocs()
    }

    /// Smallest entry `(key, value)` in the map.
    pub fn first(&self) -> Option<(u64, u64)> {
        self.inner.first()
    }

    /// Largest entry `(key, value)` in the map.
    pub fn last(&self) -> Option<(u64, u64)> {
        self.inner.last()
    }

    /// Smallest entry with key `> key` (or `>= key` if `inclusive=True`).
    #[pyo3(signature = (key, inclusive=false))]
    pub fn next(&self, key: u64, inclusive: bool) -> Option<(u64, u64)> {
        if inclusive {
            self.inner.next_at_or_after(key)
        } else {
            self.inner.next_after(key)
        }
    }

    /// Smallest entry with key `>= key`.
    pub fn next_at_or_after(&self, key: u64) -> Option<(u64, u64)> {
        self.inner.next_at_or_after(key)
    }

    /// Smallest entry with key `> key`.
    pub fn next_after(&self, key: u64) -> Option<(u64, u64)> {
        self.inner.next_after(key)
    }

    /// Largest entry with key `< key` (or `<= key` if `inclusive=True`).
    #[pyo3(signature = (key, inclusive=false))]
    pub fn prev(&self, key: u64, inclusive: bool) -> Option<(u64, u64)> {
        if inclusive {
            self.inner.prev_at_or_before(key)
        } else {
            self.inner.prev_before(key)
        }
    }

    /// Largest entry with key `<= key`.
    pub fn prev_at_or_before(&self, key: u64) -> Option<(u64, u64)> {
        self.inner.prev_at_or_before(key)
    }

    /// Largest entry with key `< key`.
    pub fn prev_before(&self, key: u64) -> Option<(u64, u64)> {
        self.inner.prev_before(key)
    }

    /// Number of keys strictly below `key` (rank).
    pub fn count_below(&self, key: u64) -> u64 {
        self.inner.count_below(key)
    }

    /// The entry `(key, value)` with `index` keys below it (0-based select).
    pub fn by_count(&self, index: u64) -> Option<(u64, u64)> {
        self.inner.by_count(index)
    }

    /// Number of keys in the range `[start, end]`.
    pub fn count_range(&self, start: u64, end: u64) -> u64 {
        self.inner.count_range(start..=end)
    }

    /// Returns a list of `(key, value)` pairs in the range `[start, end]` (inclusive).
    #[pyo3(signature = (start=None, end=None, inclusive=true))]
    pub fn range(&self, start: Option<u64>, end: Option<u64>, inclusive: bool) -> Vec<(u64, u64)> {
        let from = start.unwrap_or(0);
        self.inner
            .iter()
            .skip_while(|&(k, _)| k < from)
            .take_while(|&(k, _)| match end {
                Some(max_k) => {
                    if inclusive {
                        k <= max_k
                    } else {
                        k < max_k
                    }
                }
                None => true,
            })
            .collect()
    }

    /// Returns a list of all `(key, value)` pairs in ascending order.
    pub fn to_list(&self) -> Vec<(u64, u64)> {
        self.inner.iter().collect()
    }

    /// Key iterator for Python `for key in map:`.
    pub fn __iter__(&self) -> ExpanseMapKeyIter {
        let keys: Vec<u64> = self.inner.iter().map(|(k, _)| k).collect();
        ExpanseMapKeyIter { keys, index: 0 }
    }

    /// Returns a list of all keys in ascending order.
    pub fn keys(&self) -> Vec<u64> {
        self.inner.iter().map(|(k, _)| k).collect()
    }

    /// Returns a list of all values in key-ascending order.
    pub fn values(&self) -> Vec<u64> {
        self.inner.iter().map(|(_, v)| v).collect()
    }

    /// Returns a list of `(key, value)` pairs in ascending order.
    pub fn items(&self) -> Vec<(u64, u64)> {
        self.inner.iter().collect()
    }

    /// Updates the map from another mapping or iterable of `(key, value)` pairs.
    pub fn update(&mut self, other: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Ok(other_map) = other.extract::<PyRef<'_, ExpanseMap>>() {
            for (k, v) in other_map.inner.iter() {
                self.inner.insert(k, v);
            }
            return Ok(());
        }

        if let Ok(dict) = other.cast::<pyo3::types::PyDict>() {
            for (k, v) in dict.iter() {
                let k: u64 = k.extract()?;
                let v: u64 = v.extract()?;
                self.inner.insert(k, v);
            }
            return Ok(());
        }

        for item in other.try_iter()? {
            let item = item?;
            let (k, v): (u64, u64) = item.extract()?;
            self.inner.insert(k, v);
        }
        Ok(())
    }

    /// Bulk insertion from two buffers (keys and values). Zero-copy if C-contiguous buffers.
    pub fn insert_many(
        &mut self,
        keys: &Bound<'_, PyAny>,
        values: &Bound<'_, PyAny>,
    ) -> PyResult<usize> {
        for_each_u64_pair(keys, values, |k, v| {
            self.inner.insert(k, v);
        })
    }

    /// String representation.
    pub fn __repr__(&self) -> String {
        format!("ExpanseMap(len={})", self.inner.len())
    }
}

impl Default for ExpanseMap {
    fn default() -> Self {
        Self {
            inner: InnerMap::new(),
        }
    }
}

/// Iterator yielding keys of [`ExpanseMap`].
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct ExpanseMapKeyIter {
    pub(crate) keys: Vec<u64>,
    pub(crate) index: usize,
}

#[pymethods]
impl ExpanseMapKeyIter {
    /// Returns the iterator instance.
    pub fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Returns the next key in ascending order.
    pub fn __next__(&mut self) -> Option<u64> {
        if self.index < self.keys.len() {
            let item = self.keys[self.index];
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Returns the remaining count of keys.
    pub fn __len__(&self) -> usize {
        self.keys.len().saturating_sub(self.index)
    }
}

/// Iterator yielding values of [`ExpanseMap`].
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct ExpanseMapValueIter {
    pub(crate) values: Vec<u64>,
    pub(crate) index: usize,
}

#[pymethods]
impl ExpanseMapValueIter {
    /// Returns the iterator instance.
    pub fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Returns the next value in ascending key order.
    pub fn __next__(&mut self) -> Option<u64> {
        if self.index < self.values.len() {
            let item = self.values[self.index];
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Returns the remaining count of values.
    pub fn __len__(&self) -> usize {
        self.values.len().saturating_sub(self.index)
    }
}

/// Iterator yielding `(key, value)` pairs of [`ExpanseMap`].
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct ExpanseMapItemIter {
    pub(crate) items: Vec<(u64, u64)>,
    pub(crate) index: usize,
}

#[pymethods]
impl ExpanseMapItemIter {
    /// Returns the iterator instance.
    pub fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Returns the next `(key, value)` pair in ascending order.
    pub fn __next__(&mut self) -> Option<(u64, u64)> {
        if self.index < self.items.len() {
            let item = self.items[self.index];
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Returns the remaining count of items.
    pub fn __len__(&self) -> usize {
        self.items.len().saturating_sub(self.index)
    }
}

/// Range iterator yielding `(key, value)` pairs of [`ExpanseMap`].
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct ExpanseMapRangeIter {
    pub(crate) items: Vec<(u64, u64)>,
    pub(crate) index: usize,
}

#[pymethods]
impl ExpanseMapRangeIter {
    /// Returns the iterator instance.
    pub fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Returns the next `(key, value)` pair in the range.
    pub fn __next__(&mut self) -> Option<(u64, u64)> {
        if self.index < self.items.len() {
            let item = self.items[self.index];
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Returns the remaining count of items in the range.
    pub fn __len__(&self) -> usize {
        self.items.len().saturating_sub(self.index)
    }
}
