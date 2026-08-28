//! PyO3 wrapper for ExpanseSet (64-bit integer digital trie set).

use crate::buffer::for_each_u64_key;
use expanse_trie::set::ExpanseSet as InnerSet;
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;

/// A sparse, dynamic 64-bit unsigned integer set (compat: Judy1).
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct ExpanseSet {
    pub(crate) inner: InnerSet,
}

#[pymethods]
impl ExpanseSet {
    /// Creates an empty set, optionally initialized from an iterable of integers.
    #[new]
    #[pyo3(signature = (iterable=None))]
    pub fn new(iterable: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let mut set = InnerSet::new();
        if let Some(iter_obj) = iterable {
            for item in iter_obj.try_iter()? {
                let key: u64 = item?.extract()?;
                set.insert(key);
            }
        }
        Ok(Self { inner: set })
    }

    /// Number of elements in the set.
    pub fn __len__(&self) -> usize {
        self.inner.len() as usize
    }

    /// True when no elements are in the set.
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

    /// Membership test `key in set`.
    pub fn __contains__(&self, key: u64) -> bool {
        self.inner.contains(key)
    }

    /// Returns True if key is present in the set.
    pub fn contains(&self, key: u64) -> bool {
        self.inner.contains(key)
    }

    /// Checks membership for a batch of keys, returning a list of bools.
    pub fn contains_batch(&self, keys: Vec<u64>) -> Vec<bool> {
        let mut out = vec![false; keys.len()];
        self.inner.contains_batch(&keys, &mut out);
        out
    }

    /// Inserts `key` into the set; returns `True` if it was not present.
    pub fn insert(&mut self, key: u64) -> bool {
        self.inner.insert(key)
    }

    /// Adds `key` to the set (standard Python set method). Returns `True` if newly inserted.
    pub fn add(&mut self, key: u64) -> bool {
        self.inner.insert(key)
    }

    /// Removes `key` from the set; returns `True` if it was present, `False` otherwise.
    pub fn remove(&mut self, key: u64) -> bool {
        self.inner.remove(key)
    }

    /// Removes `key` if present; returns `True` if it was present, `False` otherwise.
    pub fn discard(&mut self, key: u64) -> bool {
        self.inner.remove(key)
    }

    /// Removes and returns the smallest element in the set; raises `KeyError` if empty.
    pub fn pop(&mut self) -> PyResult<u64> {
        match self.inner.first() {
            Some(key) => {
                self.inner.remove(key);
                Ok(key)
            }
            None => Err(PyKeyError::new_err("pop from an empty ExpanseSet")),
        }
    }

    /// Removes all elements from the set.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Heap bytes used by the set allocations.
    pub fn mem_used(&self) -> usize {
        self.inner.mem_used()
    }

    /// Total node allocations performed by the trie engine.
    pub fn total_node_allocs(&self) -> usize {
        self.inner.total_node_allocs()
    }

    /// Smallest element in the set.
    pub fn first(&self) -> Option<u64> {
        self.inner.first()
    }

    /// Largest element in the set.
    pub fn last(&self) -> Option<u64> {
        self.inner.last()
    }

    /// Smallest element with key `> key` (or `>= key` if `inclusive=True`).
    #[pyo3(signature = (key, inclusive=false))]
    pub fn next(&self, key: u64, inclusive: bool) -> Option<u64> {
        if inclusive {
            self.inner.next_at_or_after(key)
        } else {
            self.inner.next_after(key)
        }
    }

    /// Smallest element with key `>= key`.
    pub fn next_at_or_after(&self, key: u64) -> Option<u64> {
        self.inner.next_at_or_after(key)
    }

    /// Smallest element with key `> key`.
    pub fn next_after(&self, key: u64) -> Option<u64> {
        self.inner.next_after(key)
    }

    /// Largest element with key `< key` (or `<= key` if `inclusive=True`).
    #[pyo3(signature = (key, inclusive=false))]
    pub fn prev(&self, key: u64, inclusive: bool) -> Option<u64> {
        if inclusive {
            self.inner.prev_at_or_before(key)
        } else {
            self.inner.prev_before(key)
        }
    }

    /// Largest element with key `<= key`.
    pub fn prev_at_or_before(&self, key: u64) -> Option<u64> {
        self.inner.prev_at_or_before(key)
    }

    /// Largest element with key `< key`.
    pub fn prev_before(&self, key: u64) -> Option<u64> {
        self.inner.prev_before(key)
    }

    /// Number of keys strictly below `key` (rank).
    pub fn count_below(&self, key: u64) -> u64 {
        self.inner.count_below(key)
    }

    /// The element with `index` keys below it (0-based select).
    pub fn by_count(&self, index: u64) -> Option<u64> {
        self.inner.by_count(index)
    }

    /// Number of keys in the range `[start, end]`.
    pub fn count_range(&self, start: u64, end: u64) -> u64 {
        self.inner.count_range(start..=end)
    }

    /// Returns a list of elements in the range `[start, end]` (inclusive).
    #[pyo3(signature = (start=None, end=None, inclusive=true))]
    pub fn range(&self, start: Option<u64>, end: Option<u64>, inclusive: bool) -> Vec<u64> {
        let from = start.unwrap_or(0);
        self.inner
            .iter()
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
    }

    /// Returns all keys as a Python list in ascending order.
    pub fn to_list(&self) -> Vec<u64> {
        self.inner.iter().collect()
    }

    /// Key iterator for Python `for key in set:`.
    pub fn __iter__(&self) -> ExpanseSetIter {
        let items: Vec<u64> = self.inner.iter().collect();
        ExpanseSetIter { items, index: 0 }
    }

    /// Updates the set from another set or iterable.
    pub fn update(&mut self, other: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Ok(other_set) = other.extract::<PyRef<'_, ExpanseSet>>() {
            for k in other_set.inner.iter() {
                self.inner.insert(k);
            }
            return Ok(());
        }

        for item in other.try_iter()? {
            let k: u64 = item?.extract()?;
            self.inner.insert(k);
        }
        Ok(())
    }

    /// Bulk insertion from a buffer or iterable of integers. Zero-copy if C-contiguous.
    pub fn insert_many(&mut self, keys: &Bound<'_, PyAny>) -> PyResult<usize> {
        for_each_u64_key(keys, |k| {
            self.inner.insert(k);
        })
    }

    /// String representation.
    pub fn __repr__(&self) -> String {
        format!("ExpanseSet(len={})", self.inner.len())
    }
}

impl Default for ExpanseSet {
    fn default() -> Self {
        Self {
            inner: InnerSet::new(),
        }
    }
}

/// Key iterator for [`ExpanseSet`].
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct ExpanseSetIter {
    pub(crate) items: Vec<u64>,
    pub(crate) index: usize,
}

#[pymethods]
impl ExpanseSetIter {
    /// Returns the iterator instance.
    pub fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Returns the next element in ascending order.
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

/// Range iterator for [`ExpanseSet`].
#[pyclass(unsendable, module = "expanse_trie._expanse")]
pub struct ExpanseSetRangeIter {
    pub(crate) items: Vec<u64>,
    pub(crate) index: usize,
}

#[pymethods]
impl ExpanseSetRangeIter {
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
