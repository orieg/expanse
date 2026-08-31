//! PyO3 wrappers for SyncExpanseMap and SyncExpanseSet (GIL-free multithreaded concurrent structures).

use crate::buffer::{for_each_u64_key, for_each_u64_pair};
use expanse_trie::sync::OwnedMapReader;
use expanse_trie::sync::{SyncExpanseMap as InnerSyncMap, SyncExpanseSet as InnerSyncSet};
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use pyo3::types::PyDictMethods;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

// Per-thread reader cache (#554).
//
// `SyncExpanseMap::get` is the one-shot form: it registers a throwaway reader,
// which takes the collector's registry mutex to push a slot and takes it again
// on drop to `retain` the slot out. Paying that twice per lookup made concurrent
// Python readers serialise on a process-wide lock — measured at 0.02x scaling on
// 16 threads, worse than a GIL-bound `dict` (run 33447136200).
//
// A reader owns one epoch slot and its pins are not reentrant, so the cache is
// thread-local rather than shared: each thread registers once and reuses it.
// Keyed by the map's `Arc` address so several maps in one thread each get their
// own, and entries are dropped when the map is.
thread_local! {
    static MAP_READERS: RefCell<HashMap<usize, OwnedMapReader>> = RefCell::new(HashMap::new());
}

/// Runs `f` against this thread's cached reader for `map`, registering on first
/// use. Falls back to nothing else: every read path should route through here.
fn with_map_reader<R>(map: &Arc<InnerSyncMap>, f: impl FnOnce(&OwnedMapReader) -> R) -> R {
    let key = Arc::as_ptr(map) as usize;
    MAP_READERS.with(|c| {
        let mut cache = c.borrow_mut();
        let reader = cache.entry(key).or_insert_with(|| map.owned_reader());
        f(reader)
    })
}

/// A thread-safe, concurrent 64-bit integer map with optimistic concurrency control (OCC).
///
/// Lookups, scans, and range queries execute lock-free and release the Python GIL
/// via `py.detach(...)`, unlocking multithreaded read throughput across CPU cores.
#[pyclass(from_py_object, module = "expanse_trie._expanse")]
#[derive(Clone)]
pub struct SyncExpanseMap {
    pub(crate) inner: Arc<InnerSyncMap>,
}

#[pymethods]
impl SyncExpanseMap {
    /// Creates an empty concurrent map.
    #[new]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(InnerSyncMap::new()),
        }
    }

    /// Number of entries in the concurrent map (GIL-free read).
    pub fn __len__(&self, py: Python<'_>) -> usize {
        py.detach(|| self.inner.len() as usize)
    }

    /// True when no entries are present (GIL-free read).
    /// True when empty releasing the GIL.
    pub fn is_empty(&self, py: Python<'_>) -> bool {
        py.detach(|| self.inner.is_empty())
    }

    /// Property returning True if empty.
    #[getter]
    pub fn empty(&self, py: Python<'_>) -> bool {
        py.detach(|| self.inner.is_empty())
    }

    /// Truth value testing for Python (GIL-free read).
    pub fn __bool__(&self, py: Python<'_>) -> bool {
        py.detach(|| !self.inner.is_empty())
    }

    /// Lock-free membership test `key in map` releasing the GIL.
    pub fn __contains__(&self, py: Python<'_>, key: u64) -> bool {
        py.detach(|| self.inner.get(key).is_some())
    }

    /// Retrieves `val = map[key]` releasing the GIL; raises `KeyError` if key is missing.
    pub fn __getitem__(&self, py: Python<'_>, key: u64) -> PyResult<u64> {
        let val = py.detach(|| self.inner.get(key));
        val.ok_or_else(|| PyKeyError::new_err(format!("Key {key} not found in SyncExpanseMap")))
    }

    /// Sets `map[key] = val` releasing the GIL.
    pub fn __setitem__(&self, py: Python<'_>, key: u64, val: u64) {
        py.detach(|| {
            self.inner.insert(key, val);
        });
    }

    /// Deletes `del map[key]` releasing the GIL; raises `KeyError` if key is missing.
    pub fn __delitem__(&self, py: Python<'_>, key: u64) -> PyResult<()> {
        let prev = py.detach(|| self.inner.remove(key));
        if prev.is_some() {
            Ok(())
        } else {
            Err(PyKeyError::new_err(format!(
                "Key {key} not found in SyncExpanseMap"
            )))
        }
    }

    /// Look up `key` releasing the GIL, returning `default` (or None) if absent.
    #[pyo3(signature = (key, default=None))]
    pub fn get(&self, py: Python<'_>, key: u64, default: Option<u64>) -> Option<u64> {
        py.detach(|| with_map_reader(&self.inner, |r| r.get(key)))
            .or(default)
    }

    /// Inserts `key -> val` releasing the GIL; returns the previous value, if any.
    pub fn insert(&self, py: Python<'_>, key: u64, val: u64) -> Option<u64> {
        py.detach(|| self.inner.insert(key, val))
    }

    /// Removes `key` releasing the GIL; returns its previous value, or `None` if absent.
    ///
    /// This mirrors `ExpanseMap.remove` (which returns an `Optional[int]`); use
    /// `del map[key]` or `pop(key)` for the KeyError-on-missing semantics.
    pub fn remove(&self, py: Python<'_>, key: u64) -> Option<u64> {
        py.detach(|| self.inner.remove(key))
    }

    /// Removes `key` and returns its value. If `key` is absent, returns `default`
    /// if given, otherwise raises `KeyError`.
    #[pyo3(signature = (key, default=None))]
    pub fn pop(&self, py: Python<'_>, key: u64, default: Option<u64>) -> PyResult<u64> {
        match py.detach(|| self.inner.remove(key)) {
            Some(v) => Ok(v),
            None => default.ok_or_else(|| {
                PyKeyError::new_err(format!("Key {key} not found in SyncExpanseMap"))
            }),
        }
    }

    /// Removes all entries from the concurrent map.
    pub fn clear(&self, py: Python<'_>) {
        py.detach(|| {
            self.inner.clear();
        });
    }

    /// Smallest entry `(key, value)` releasing the GIL.
    pub fn first(&self, py: Python<'_>) -> Option<(u64, u64)> {
        py.detach(|| self.inner.with_locked(|m| m.first()))
    }

    /// Largest entry `(key, value)` releasing the GIL.
    pub fn last(&self, py: Python<'_>) -> Option<(u64, u64)> {
        py.detach(|| self.inner.with_locked(|m| m.last()))
    }

    /// Smallest entry with key `> key` (or `>= key` if `inclusive=True`) releasing the GIL.
    #[pyo3(signature = (key, inclusive=false))]
    pub fn next(&self, py: Python<'_>, key: u64, inclusive: bool) -> Option<(u64, u64)> {
        py.detach(|| {
            self.inner.with_locked(|m| {
                if inclusive {
                    m.next_at_or_after(key)
                } else {
                    m.next_after(key)
                }
            })
        })
    }

    /// Largest entry with key `< key` (or `<= key` if `inclusive=True`) releasing the GIL.
    #[pyo3(signature = (key, inclusive=false))]
    pub fn prev(&self, py: Python<'_>, key: u64, inclusive: bool) -> Option<(u64, u64)> {
        py.detach(|| {
            self.inner.with_locked(|m| {
                if inclusive {
                    m.prev_at_or_before(key)
                } else {
                    m.prev_before(key)
                }
            })
        })
    }

    /// Number of keys strictly below `key` releasing the GIL.
    pub fn count_below(&self, py: Python<'_>, key: u64) -> u64 {
        py.detach(|| self.inner.with_locked(|m| m.count_below(key)))
    }

    /// The entry `(key, value)` with `index` keys below it releasing the GIL.
    pub fn by_count(&self, py: Python<'_>, index: u64) -> Option<(u64, u64)> {
        py.detach(|| self.inner.with_locked(|m| m.by_count(index)))
    }

    /// Number of keys in the range `[start, end]` releasing the GIL.
    pub fn count_range(&self, py: Python<'_>, start: u64, end: u64) -> u64 {
        py.detach(|| self.inner.with_locked(|m| m.count_range(start..=end)))
    }

    /// Range scan releasing the GIL during traversal.
    ///
    /// `inclusive=True` (the default) includes `end`, matching `ExpanseMap.range`/
    /// `ExpanseSet.range`; pass `inclusive=False` for a half-open `[start, end)` range.
    #[pyo3(signature = (start=None, end=None, inclusive=true))]
    pub fn range(
        &self,
        py: Python<'_>,
        start: Option<u64>,
        end: Option<u64>,
        inclusive: bool,
    ) -> SyncExpanseMapRangeIter {
        let from = start.unwrap_or(0);
        let items: Vec<(u64, u64)> = py.detach(|| {
            self.inner.with_locked(|m| {
                m.iter()
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
            })
        });
        SyncExpanseMapRangeIter { items, index: 0 }
    }

    /// Key iterator for Python `for key in map:`.
    pub fn __iter__(&self, py: Python<'_>) -> SyncExpanseMapKeyIter {
        let keys: Vec<u64> = py.detach(|| {
            self.inner
                .with_locked(|m| m.iter().map(|(k, _)| k).collect())
        });
        SyncExpanseMapKeyIter { keys, index: 0 }
    }

    /// Returns all keys in ascending order.
    pub fn keys(&self, py: Python<'_>) -> SyncExpanseMapKeyIter {
        self.__iter__(py)
    }

    /// Returns all values in key-ascending order.
    pub fn values(&self, py: Python<'_>) -> SyncExpanseMapValueIter {
        let values: Vec<u64> = py.detach(|| {
            self.inner
                .with_locked(|m| m.iter().map(|(_, v)| v).collect())
        });
        SyncExpanseMapValueIter { values, index: 0 }
    }

    /// Returns all `(key, value)` pairs in ascending order.
    pub fn items(&self, py: Python<'_>) -> SyncExpanseMapItemIter {
        let items: Vec<(u64, u64)> = py.detach(|| self.inner.with_locked(|m| m.iter().collect()));
        SyncExpanseMapItemIter { items, index: 0 }
    }

    /// Updates the concurrent map from another mapping or iterable.
    pub fn update(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Ok(other_map) = other.extract::<PyRef<'_, SyncExpanseMap>>() {
            let other_inner = Arc::clone(&other_map.inner);
            let pairs: Vec<(u64, u64)> =
                py.detach(|| other_inner.with_locked(|m| m.iter().collect()));
            py.detach(|| {
                for (k, v) in pairs {
                    self.inner.insert(k, v);
                }
            });
            return Ok(());
        }

        if let Ok(other_map) = other.extract::<PyRef<'_, crate::map::ExpanseMap>>() {
            let pairs: Vec<(u64, u64)> = other_map.inner.iter().collect();
            py.detach(|| {
                for (k, v) in pairs {
                    self.inner.insert(k, v);
                }
            });
            return Ok(());
        }

        if let Ok(dict) = other.cast::<pyo3::types::PyDict>() {
            let mut pairs = Vec::with_capacity(dict.len());
            for (k, v) in dict.iter() {
                let k: u64 = k.extract()?;
                let v: u64 = v.extract()?;
                pairs.push((k, v));
            }
            py.detach(|| {
                for (k, v) in pairs {
                    self.inner.insert(k, v);
                }
            });
            return Ok(());
        }

        let mut pairs = Vec::new();
        for item in other.try_iter()? {
            let item = item?;
            let (k, v): (u64, u64) = item.extract()?;
            pairs.push((k, v));
        }
        py.detach(|| {
            for (k, v) in pairs {
                self.inner.insert(k, v);
            }
        });
        Ok(())
    }

    /// Bulk insertion from buffers (keys and values).
    pub fn insert_many(
        &self,
        _py: Python<'_>,
        keys: &Bound<'_, PyAny>,
        values: &Bound<'_, PyAny>,
    ) -> PyResult<usize> {
        for_each_u64_pair(keys, values, |k, v| {
            self.inner.insert(k, v);
        })
    }

    /// String representation.
    pub fn __repr__(&self, py: Python<'_>) -> String {
        let len = self.__len__(py);
        format!("SyncExpanseMap(len={len})")
    }
}

impl Default for SyncExpanseMap {
    fn default() -> Self {
        Self::new()
    }
}

/// A thread-safe, concurrent 64-bit integer set with optimistic concurrency control (OCC).
///
/// Lookups, scans, and range queries execute lock-free and release the Python GIL
/// via `py.detach(...)`, unlocking multithreaded read throughput across CPU cores.
#[pyclass(from_py_object, module = "expanse_trie._expanse")]
#[derive(Clone)]
pub struct SyncExpanseSet {
    pub(crate) inner: Arc<InnerSyncSet>,
}

#[pymethods]
impl SyncExpanseSet {
    /// Creates an empty concurrent set.
    #[new]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(InnerSyncSet::new()),
        }
    }

    /// Number of elements in the set releasing the GIL.
    pub fn __len__(&self, py: Python<'_>) -> usize {
        py.detach(|| self.inner.len() as usize)
    }

    /// True when no elements are in the set releasing the GIL.
    /// True when empty releasing the GIL.
    pub fn is_empty(&self, py: Python<'_>) -> bool {
        py.detach(|| self.inner.is_empty())
    }

    /// Property returning True if empty.
    #[getter]
    pub fn empty(&self, py: Python<'_>) -> bool {
        py.detach(|| self.inner.is_empty())
    }

    /// Truth value testing for Python releasing the GIL.
    pub fn __bool__(&self, py: Python<'_>) -> bool {
        py.detach(|| !self.inner.is_empty())
    }

    /// Lock-free membership test `key in set` releasing the GIL.
    pub fn __contains__(&self, py: Python<'_>, key: u64) -> bool {
        py.detach(|| self.inner.contains(key))
    }

    /// Inserts `key` into the set releasing the GIL; returns `True` if newly inserted.
    /// Check membership releasing the GIL.
    pub fn contains(&self, py: Python<'_>, key: u64) -> bool {
        py.detach(|| self.inner.contains(key))
    }

    /// Inserts key releasing the GIL.
    pub fn insert(&self, py: Python<'_>, key: u64) -> bool {
        py.detach(|| self.inner.insert(key))
    }

    /// Adds `key` to the set (standard Python set method). Returns `True` if newly inserted.
    pub fn add(&self, py: Python<'_>, key: u64) -> bool {
        py.detach(|| self.inner.insert(key))
    }

    /// Removes `key` from the set releasing the GIL; returns `True` if it was present.
    ///
    /// This mirrors `ExpanseSet.remove` (which returns a `bool`); use `discard` for the
    /// same non-raising semantics, or a future `del`/checked path for KeyError behaviour.
    pub fn remove(&self, py: Python<'_>, key: u64) -> bool {
        py.detach(|| self.inner.remove(key))
    }

    /// Removes `key` if present releasing the GIL; returns `True` if present.
    pub fn discard(&self, py: Python<'_>, key: u64) -> bool {
        py.detach(|| self.inner.remove(key))
    }

    /// Removes all elements from the concurrent set.
    pub fn clear(&self, py: Python<'_>) {
        py.detach(|| {
            self.inner.clear();
        });
    }

    /// Smallest element in the set releasing the GIL.
    pub fn first(&self, py: Python<'_>) -> Option<u64> {
        py.detach(|| self.inner.with_locked(|s| s.first()))
    }

    /// Largest element in the set releasing the GIL.
    pub fn last(&self, py: Python<'_>) -> Option<u64> {
        py.detach(|| self.inner.with_locked(|s| s.last()))
    }

    /// Smallest element with key `> key` (or `>= key` if `inclusive=True`) releasing the GIL.
    #[pyo3(signature = (key, inclusive=false))]
    pub fn next(&self, py: Python<'_>, key: u64, inclusive: bool) -> Option<u64> {
        py.detach(|| {
            self.inner.with_locked(|s| {
                if inclusive {
                    s.next_at_or_after(key)
                } else {
                    s.next_after(key)
                }
            })
        })
    }

    /// Largest element with key `< key` (or `<= key` if `inclusive=True`) releasing the GIL.
    #[pyo3(signature = (key, inclusive=false))]
    pub fn prev(&self, py: Python<'_>, key: u64, inclusive: bool) -> Option<u64> {
        py.detach(|| {
            self.inner.with_locked(|s| {
                if inclusive {
                    s.prev_at_or_before(key)
                } else {
                    s.prev_before(key)
                }
            })
        })
    }

    /// Number of keys strictly below `key` releasing the GIL.
    pub fn count_below(&self, py: Python<'_>, key: u64) -> u64 {
        py.detach(|| self.inner.with_locked(|s| s.count_below(key)))
    }

    /// The element with `index` keys below it releasing the GIL.
    pub fn by_count(&self, py: Python<'_>, index: u64) -> Option<u64> {
        py.detach(|| self.inner.with_locked(|s| s.by_count(index)))
    }

    /// Number of keys in the range `[start, end]` releasing the GIL.
    pub fn count_range(&self, py: Python<'_>, start: u64, end: u64) -> u64 {
        py.detach(|| self.inner.with_locked(|s| s.count_range(start..=end)))
    }

    /// Range scan releasing the GIL during traversal.
    ///
    /// `inclusive=True` (the default) includes `end`, matching `ExpanseMap.range`/
    /// `ExpanseSet.range`; pass `inclusive=False` for a half-open `[start, end)` range.
    #[pyo3(signature = (start=None, end=None, inclusive=true))]
    pub fn range(
        &self,
        py: Python<'_>,
        start: Option<u64>,
        end: Option<u64>,
        inclusive: bool,
    ) -> SyncExpanseSetRangeIter {
        let from = start.unwrap_or(0);
        let items: Vec<u64> = py.detach(|| {
            self.inner.with_locked(|s| {
                s.iter()
                    .skip_while(|&k| k < from)
                    .take_while(|&k| match end {
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
            })
        });
        SyncExpanseSetRangeIter { items, index: 0 }
    }

    /// Key iterator for Python `for key in set:`.
    pub fn __iter__(&self, py: Python<'_>) -> SyncExpanseSetIter {
        let items: Vec<u64> = py.detach(|| self.inner.with_locked(|s| s.iter().collect()));
        SyncExpanseSetIter { items, index: 0 }
    }

    /// Updates the set from another set or iterable.
    pub fn update(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Ok(other_set) = other.extract::<PyRef<'_, SyncExpanseSet>>() {
            let other_inner = Arc::clone(&other_set.inner);
            let keys: Vec<u64> = py.detach(|| other_inner.with_locked(|s| s.iter().collect()));
            py.detach(|| {
                for k in keys {
                    self.inner.insert(k);
                }
            });
            return Ok(());
        }

        if let Ok(other_set) = other.extract::<PyRef<'_, crate::set::ExpanseSet>>() {
            let keys: Vec<u64> = other_set.inner.iter().collect();
            py.detach(|| {
                for k in keys {
                    self.inner.insert(k);
                }
            });
            return Ok(());
        }

        let mut keys = Vec::new();
        for item in other.try_iter()? {
            let k: u64 = item?.extract()?;
            keys.push(k);
        }
        py.detach(|| {
            for k in keys {
                self.inner.insert(k);
            }
        });
        Ok(())
    }

    /// Bulk insertion from a buffer or iterable of integers.
    pub fn insert_many(&self, _py: Python<'_>, keys: &Bound<'_, PyAny>) -> PyResult<usize> {
        for_each_u64_key(keys, |k| {
            self.inner.insert(k);
        })
    }

    /// String representation.
    pub fn __repr__(&self, py: Python<'_>) -> String {
        let len = self.__len__(py);
        format!("SyncExpanseSet(len={len})")
    }
}

impl Default for SyncExpanseSet {
    fn default() -> Self {
        Self::new()
    }
}

/// Key iterator for [`SyncExpanseMap`].
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct SyncExpanseMapKeyIter {
    pub(crate) keys: Vec<u64>,
    pub(crate) index: usize,
}

#[pymethods]
impl SyncExpanseMapKeyIter {
    /// Returns the iterator instance.
    pub fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Returns the next key.
    pub fn __next__(&mut self) -> Option<u64> {
        if self.index < self.keys.len() {
            let item = self.keys[self.index];
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

/// Value iterator for [`SyncExpanseMap`].
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct SyncExpanseMapValueIter {
    pub(crate) values: Vec<u64>,
    pub(crate) index: usize,
}

#[pymethods]
impl SyncExpanseMapValueIter {
    /// Returns the iterator instance.
    pub fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Returns the next value.
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

/// Item iterator for [`SyncExpanseMap`].
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct SyncExpanseMapItemIter {
    pub(crate) items: Vec<(u64, u64)>,
    pub(crate) index: usize,
}

#[pymethods]
impl SyncExpanseMapItemIter {
    /// Returns the iterator instance.
    pub fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Returns the next `(key, value)` pair.
    pub fn __next__(&mut self) -> Option<(u64, u64)> {
        if self.index < self.items.len() {
            let item = self.items[self.index];
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

/// Range iterator for [`SyncExpanseMap`].
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct SyncExpanseMapRangeIter {
    pub(crate) items: Vec<(u64, u64)>,
    pub(crate) index: usize,
}

#[pymethods]
impl SyncExpanseMapRangeIter {
    /// Returns the iterator instance.
    pub fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Returns the next `(key, value)` pair in range.
    pub fn __next__(&mut self) -> Option<(u64, u64)> {
        if self.index < self.items.len() {
            let item = self.items[self.index];
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Returns remaining item count in range.
    pub fn __len__(&self) -> usize {
        self.items.len().saturating_sub(self.index)
    }
}

/// Key iterator for [`SyncExpanseSet`].
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct SyncExpanseSetIter {
    pub(crate) items: Vec<u64>,
    pub(crate) index: usize,
}

#[pymethods]
impl SyncExpanseSetIter {
    /// Returns the iterator instance.
    pub fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Returns the next element.
    pub fn __next__(&mut self) -> Option<u64> {
        if self.index < self.items.len() {
            let item = self.items[self.index];
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Returns remaining element count.
    pub fn __len__(&self) -> usize {
        self.items.len().saturating_sub(self.index)
    }
}

/// Range iterator for [`SyncExpanseSet`].
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct SyncExpanseSetRangeIter {
    pub(crate) items: Vec<u64>,
    pub(crate) index: usize,
}

#[pymethods]
impl SyncExpanseSetRangeIter {
    /// Returns the iterator instance.
    pub fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Returns the next element in range.
    pub fn __next__(&mut self) -> Option<u64> {
        if self.index < self.items.len() {
            let item = self.items[self.index];
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Returns remaining element count in range.
    pub fn __len__(&self) -> usize {
        self.items.len().saturating_sub(self.index)
    }
}
