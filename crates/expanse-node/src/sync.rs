//! Node.js / Bun / Deno N-API binding for SyncExpanseMap and SyncExpanseSet (concurrent OCC digital tries).

use crate::common::{KeyInput, MapEntry, key_to_u64};
use expanse_trie::sync::{SyncExpanseMap as InnerSyncMap, SyncExpanseSet as InnerSyncSet};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::sync::Arc;

/// A thread-safe concurrent 64-bit integer map with optimistic concurrency control (OCC).
///
/// Lookups and scans execute on the optimistic path with epoch-based reclamation.
#[napi]
#[derive(Clone)]
pub struct SyncExpanseMap {
    pub(crate) inner: Arc<InnerSyncMap>,
}

#[napi]
impl SyncExpanseMap {
    /// Creates an empty concurrent map.
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(InnerSyncMap::new()),
        }
    }

    /// Number of entries in the concurrent map.
    #[napi]
    pub fn size(&self) -> BigInt {
        BigInt::from(self.inner.len())
    }

    /// Returns `true` if the map contains no entries.
    #[napi]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Optimistic membership test `has(key)`.
    #[napi]
    pub fn has(&self, key: KeyInput) -> Result<bool> {
        let k = key_to_u64(key)?;
        Ok(self.inner.get(k).is_some())
    }

    /// Sets `map[key] = value`. Returns previous value if present, or `null`.
    #[napi]
    pub fn set(&self, key: KeyInput, value: KeyInput) -> Result<Option<BigInt>> {
        let k = key_to_u64(key)?;
        let v = key_to_u64(value)?;
        Ok(self.inner.insert(k, v).map(BigInt::from))
    }

    /// Optimistic retrieval of value for `key`, or `null` if absent.
    #[napi]
    pub fn get(&self, key: KeyInput) -> Result<Option<BigInt>> {
        let k = key_to_u64(key)?;
        Ok(self.inner.get(k).map(BigInt::from))
    }

    /// Deletes `key` from the map. Returns `true` if it was present, `false` otherwise.
    #[napi]
    pub fn delete(&self, key: KeyInput) -> Result<bool> {
        let k = key_to_u64(key)?;
        Ok(self.inner.remove(k).is_some())
    }

    /// Removes all entries from the concurrent map.
    #[napi]
    pub fn clear(&self) {
        self.inner.clear();
    }

    /// Smallest entry `(key, value)` in the map, or `null` if empty.
    #[napi]
    pub fn first(&self) -> Option<MapEntry> {
        self.inner.with_locked(|m| {
            m.first().map(|(k, v)| MapEntry {
                key: BigInt::from(k),
                value: BigInt::from(v),
            })
        })
    }

    /// Largest entry `(key, value)` in the map, or `null` if empty.
    #[napi]
    pub fn last(&self) -> Option<MapEntry> {
        self.inner.with_locked(|m| {
            m.last().map(|(k, v)| MapEntry {
                key: BigInt::from(k),
                value: BigInt::from(v),
            })
        })
    }

    /// Smallest entry with key strictly `> key` (or `>= key` if `inclusive` is `true`).
    #[napi]
    pub fn next(&self, key: KeyInput, inclusive: Option<bool>) -> Result<Option<MapEntry>> {
        let k = key_to_u64(key)?;
        let val = self.inner.with_locked(|m| {
            if inclusive.unwrap_or(false) {
                m.next_at_or_after(k)
            } else {
                m.next_after(k)
            }
        });
        Ok(val.map(|(ek, ev)| MapEntry {
            key: BigInt::from(ek),
            value: BigInt::from(ev),
        }))
    }

    /// Largest entry with key strictly `< key` (or `<= key` if `inclusive` is `true`).
    #[napi]
    pub fn prev(&self, key: KeyInput, inclusive: Option<bool>) -> Result<Option<MapEntry>> {
        let k = key_to_u64(key)?;
        let val = self.inner.with_locked(|m| {
            if inclusive.unwrap_or(false) {
                m.prev_at_or_before(k)
            } else {
                m.prev_before(k)
            }
        });
        Ok(val.map(|(ek, ev)| MapEntry {
            key: BigInt::from(ek),
            value: BigInt::from(ev),
        }))
    }

    /// Number of keys strictly below `key` (rank).
    #[napi]
    pub fn rank(&self, key: KeyInput) -> Result<BigInt> {
        let k = key_to_u64(key)?;
        Ok(BigInt::from(self.inner.with_locked(|m| m.count_below(k))))
    }

    /// The entry with `k` keys below it (0-based select), or `null` if out of bounds.
    #[napi]
    pub fn select(&self, k: KeyInput) -> Result<Option<MapEntry>> {
        let idx = key_to_u64(k)?;
        let val = self.inner.with_locked(|m| m.by_count(idx));
        Ok(val.map(|(ek, ev)| MapEntry {
            key: BigInt::from(ek),
            value: BigInt::from(ev),
        }))
    }

    /// Number of keys in the closed range `[start, end]`.
    #[napi(js_name = "countRange")]
    pub fn count_range(&self, start: KeyInput, end: KeyInput) -> Result<BigInt> {
        let a = key_to_u64(start)?;
        let b = key_to_u64(end)?;
        Ok(BigInt::from(
            self.inner.with_locked(|m| m.count_range(a..=b)),
        ))
    }

    /// Returns an array of all keys in ascending order.
    #[napi]
    pub fn keys(&self) -> Vec<BigInt> {
        self.inner
            .with_locked(|m| m.iter().map(|(k, _)| BigInt::from(k)).collect())
    }

    /// Returns an array of all values in key-ascending order.
    #[napi]
    pub fn values(&self) -> Vec<BigInt> {
        self.inner
            .with_locked(|m| m.iter().map(|(_, v)| BigInt::from(v)).collect())
    }

    /// Returns an array of all `{ key, value }` entries in ascending order.
    #[napi]
    pub fn entries(&self) -> Vec<MapEntry> {
        self.inner.with_locked(|m| {
            m.iter()
                .map(|(k, v)| MapEntry {
                    key: BigInt::from(k),
                    value: BigInt::from(v),
                })
                .collect()
        })
    }
}

impl Default for SyncExpanseMap {
    fn default() -> Self {
        Self::new()
    }
}

/// A thread-safe concurrent 64-bit integer set with optimistic concurrency control (OCC).
///
/// Membership lookups and scans execute on the optimistic path with epoch-based reclamation.
#[napi]
#[derive(Clone)]
pub struct SyncExpanseSet {
    pub(crate) inner: Arc<InnerSyncSet>,
}

#[napi]
impl SyncExpanseSet {
    /// Creates an empty concurrent set.
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(InnerSyncSet::new()),
        }
    }

    /// Number of elements in the set.
    #[napi]
    pub fn size(&self) -> BigInt {
        BigInt::from(self.inner.len())
    }

    /// Returns `true` if the set contains no elements.
    #[napi]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Optimistic membership test `has(key)`.
    #[napi]
    pub fn has(&self, key: KeyInput) -> Result<bool> {
        let k = key_to_u64(key)?;
        Ok(self.inner.contains(k))
    }

    /// Inserts `key` into the concurrent set. Returns `true` if newly inserted.
    #[napi]
    pub fn add(&self, key: KeyInput) -> Result<bool> {
        let k = key_to_u64(key)?;
        Ok(self.inner.insert(k))
    }

    /// Removes `key` from the concurrent set. Returns `true` if it was present.
    #[napi]
    pub fn remove(&self, key: KeyInput) -> Result<bool> {
        let k = key_to_u64(key)?;
        Ok(self.inner.remove(k))
    }

    /// Removes all elements from the concurrent set.
    #[napi]
    pub fn clear(&self) {
        self.inner.clear();
    }

    /// Smallest element in the set, or `null` if empty.
    #[napi]
    pub fn first(&self) -> Option<BigInt> {
        self.inner.with_locked(|s| s.first().map(BigInt::from))
    }

    /// Largest element in the set, or `null` if empty.
    #[napi]
    pub fn last(&self) -> Option<BigInt> {
        self.inner.with_locked(|s| s.last().map(BigInt::from))
    }

    /// Smallest element strictly `> key` (or `>= key` if `inclusive` is `true`).
    #[napi]
    pub fn next(&self, key: KeyInput, inclusive: Option<bool>) -> Result<Option<BigInt>> {
        let k = key_to_u64(key)?;
        let val = self.inner.with_locked(|s| {
            if inclusive.unwrap_or(false) {
                s.next_at_or_after(k)
            } else {
                s.next_after(k)
            }
        });
        Ok(val.map(BigInt::from))
    }

    /// Largest element strictly `< key` (or `<= key` if `inclusive` is `true`).
    #[napi]
    pub fn prev(&self, key: KeyInput, inclusive: Option<bool>) -> Result<Option<BigInt>> {
        let k = key_to_u64(key)?;
        let val = self.inner.with_locked(|s| {
            if inclusive.unwrap_or(false) {
                s.prev_at_or_before(k)
            } else {
                s.prev_before(k)
            }
        });
        Ok(val.map(BigInt::from))
    }

    /// Number of keys strictly below `key` (rank).
    #[napi]
    pub fn rank(&self, key: KeyInput) -> Result<BigInt> {
        let k = key_to_u64(key)?;
        Ok(BigInt::from(self.inner.with_locked(|s| s.count_below(k))))
    }

    /// The element with `k` keys below it (0-based select), or `null` if out of bounds.
    #[napi]
    pub fn select(&self, k: KeyInput) -> Result<Option<BigInt>> {
        let idx = key_to_u64(k)?;
        let val = self.inner.with_locked(|s| s.by_count(idx));
        Ok(val.map(BigInt::from))
    }

    /// Number of keys in the closed range `[start, end]`.
    #[napi(js_name = "countRange")]
    pub fn count_range(&self, start: KeyInput, end: KeyInput) -> Result<BigInt> {
        let a = key_to_u64(start)?;
        let b = key_to_u64(end)?;
        Ok(BigInt::from(
            self.inner.with_locked(|s| s.count_range(a..=b)),
        ))
    }

    /// Returns all elements in ascending order as an array of BigInts.
    #[napi(js_name = "toArray")]
    pub fn to_array(&self) -> Vec<BigInt> {
        self.inner
            .with_locked(|s| s.iter().map(BigInt::from).collect())
    }
}

impl Default for SyncExpanseSet {
    fn default() -> Self {
        Self::new()
    }
}
