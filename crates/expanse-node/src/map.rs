//! Node.js / Bun / Deno N-API binding for ExpanseMap (sparse 64-bit integer digital trie map).

use crate::common::{KeyInput, MapEntry, key_to_u64};
use expanse_trie::map::ExpanseMap as InnerMap;
use napi::bindgen_prelude::*;
use napi_derive::napi;

/// A sparse, dynamic 64-bit unsigned integer key/value map (compat: JudyL).
///
/// Adaptive expanse-partitioned trie: memory stays near-proportional to population
/// and lookups finish in at most 8 digit steps without hash collisions.
#[napi]
pub struct ExpanseMap {
    pub(crate) inner: InnerMap,
}

#[napi]
impl ExpanseMap {
    /// Creates an empty integer map.
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            inner: InnerMap::new(),
        }
    }

    /// Number of entries in the map.
    #[napi]
    pub fn size(&self) -> BigInt {
        BigInt::from(self.inner.len())
    }

    /// Returns `true` if the map contains no entries.
    #[napi]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Membership test `has(key)`. Returns `true` if `key` exists in the map.
    #[napi]
    pub fn has(&self, key: KeyInput) -> Result<bool> {
        let k = key_to_u64(key)?;
        Ok(self.inner.contains_key(k))
    }

    /// Sets `map[key] = value`. Returns previous value as BigInt if present, or `null`.
    #[napi]
    pub fn set(&mut self, key: KeyInput, value: KeyInput) -> Result<Option<BigInt>> {
        let k = key_to_u64(key)?;
        let v = key_to_u64(value)?;
        Ok(self.inner.insert(k, v).map(BigInt::from))
    }

    /// Gets the value for `key`, or `null` if absent.
    #[napi]
    pub fn get(&self, key: KeyInput) -> Result<Option<BigInt>> {
        let k = key_to_u64(key)?;
        Ok(self.inner.get(k).map(BigInt::from))
    }

    /// Gets values for a batch of keys, returning an array with BigInt values or null.
    #[napi]
    pub fn get_batch(&self, keys: Vec<KeyInput>) -> Result<Vec<Option<BigInt>>> {
        let mut u_keys = Vec::with_capacity(keys.len());
        for k in keys {
            u_keys.push(key_to_u64(k)?);
        }
        let mut out = vec![None; u_keys.len()];
        self.inner.get_batch(&u_keys, &mut out);
        Ok(out.into_iter().map(|opt| opt.map(BigInt::from)).collect())
    }

    /// Deletes `key` from the map. Returns `true` if it was present, `false` otherwise.
    #[napi]
    pub fn delete(&mut self, key: KeyInput) -> Result<bool> {
        let k = key_to_u64(key)?;
        Ok(self.inner.remove(k).is_some())
    }

    /// Removes all entries from the map.
    #[napi]
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Heap bytes used by the trie allocations.
    #[napi]
    pub fn mem_used(&self) -> BigInt {
        BigInt::from(self.inner.mem_used() as u64)
    }

    /// Smallest entry `(key, value)` in the map, or `null` if empty.
    #[napi]
    pub fn first(&self) -> Option<MapEntry> {
        self.inner.first().map(|(k, v)| MapEntry {
            key: BigInt::from(k),
            value: BigInt::from(v),
        })
    }

    /// Largest entry `(key, value)` in the map, or `null` if empty.
    #[napi]
    pub fn last(&self) -> Option<MapEntry> {
        self.inner.last().map(|(k, v)| MapEntry {
            key: BigInt::from(k),
            value: BigInt::from(v),
        })
    }

    /// Smallest entry with key strictly `> key` (or `>= key` if `inclusive` is `true`).
    #[napi]
    pub fn next(&self, key: KeyInput, inclusive: Option<bool>) -> Result<Option<MapEntry>> {
        let k = key_to_u64(key)?;
        let val = if inclusive.unwrap_or(false) {
            self.inner.next_at_or_after(k)
        } else {
            self.inner.next_after(k)
        };
        Ok(val.map(|(ek, ev)| MapEntry {
            key: BigInt::from(ek),
            value: BigInt::from(ev),
        }))
    }

    /// Largest entry with key strictly `< key` (or `<= key` if `inclusive` is `true`).
    #[napi]
    pub fn prev(&self, key: KeyInput, inclusive: Option<bool>) -> Result<Option<MapEntry>> {
        let k = key_to_u64(key)?;
        let val = if inclusive.unwrap_or(false) {
            self.inner.prev_at_or_before(k)
        } else {
            self.inner.prev_before(k)
        };
        Ok(val.map(|(ek, ev)| MapEntry {
            key: BigInt::from(ek),
            value: BigInt::from(ev),
        }))
    }

    /// Number of keys strictly below `key` (rank).
    #[napi]
    pub fn rank(&self, key: KeyInput) -> Result<BigInt> {
        let k = key_to_u64(key)?;
        Ok(BigInt::from(self.inner.count_below(k)))
    }

    /// The entry with `k` keys below it (0-based select), or `null` if out of bounds.
    #[napi]
    pub fn select(&self, k: KeyInput) -> Result<Option<MapEntry>> {
        let idx = key_to_u64(k)?;
        Ok(self.inner.by_count(idx).map(|(ek, ev)| MapEntry {
            key: BigInt::from(ek),
            value: BigInt::from(ev),
        }))
    }

    /// Number of keys in the closed range `[start, end]`.
    #[napi(js_name = "countRange")]
    pub fn count_range(&self, start: KeyInput, end: KeyInput) -> Result<BigInt> {
        let a = key_to_u64(start)?;
        let b = key_to_u64(end)?;
        Ok(BigInt::from(self.inner.count_range(a..=b)))
    }

    /// Returns an array of all keys in ascending order.
    #[napi]
    pub fn keys(&self) -> Vec<BigInt> {
        self.inner.iter().map(|(k, _)| BigInt::from(k)).collect()
    }

    /// Returns an array of all values in key-ascending order.
    #[napi]
    pub fn values(&self) -> Vec<BigInt> {
        self.inner.iter().map(|(_, v)| BigInt::from(v)).collect()
    }

    /// Returns an array of all `{ key, value }` entries in ascending order.
    #[napi]
    pub fn entries(&self) -> Vec<MapEntry> {
        self.inner
            .iter()
            .map(|(k, v)| MapEntry {
                key: BigInt::from(k),
                value: BigInt::from(v),
            })
            .collect()
    }

    /// Range query returning entries in `[start, end]`.
    #[napi]
    pub fn range(
        &self,
        start: Option<KeyInput>,
        end: Option<KeyInput>,
        inclusive: Option<bool>,
    ) -> Result<Vec<MapEntry>> {
        let from = match start {
            Some(s) => key_to_u64(s)?,
            None => 0,
        };
        let max_k = match end {
            Some(e) => Some(key_to_u64(e)?),
            None => None,
        };
        let inc = inclusive.unwrap_or(true);

        let items = self
            .inner
            .iter()
            .skip_while(|&(k, _)| k < from)
            .take_while(|&(k, _)| match max_k {
                Some(max) => {
                    if inc {
                        k <= max
                    } else {
                        k < max
                    }
                }
                None => true,
            })
            .map(|(k, v)| MapEntry {
                key: BigInt::from(k),
                value: BigInt::from(v),
            })
            .collect();

        Ok(items)
    }
}

impl Default for ExpanseMap {
    fn default() -> Self {
        Self::new()
    }
}
