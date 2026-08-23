//! Node.js / Bun / Deno N-API binding for ExpanseStrMap (sorted string trie map, compat: JudySL).

use crate::common::{KeyInput, StrMapEntry, key_to_u64, str_to_nul_free_bytes};
use expanse_trie::strmap::ExpanseStrMap as InnerStrMap;
use napi::bindgen_prelude::*;
use napi_derive::napi;

/// A sorted map from NUL-free UTF-8 strings to 64-bit unsigned integers (compat: JudySL).
///
/// Iteration order is byte-lexicographical. Keys are prefix-compressed across 8-byte boundaries.
#[napi]
pub struct ExpanseStrMap {
    pub(crate) inner: InnerStrMap,
}

#[napi]
impl ExpanseStrMap {
    /// Creates an empty string map.
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            inner: InnerStrMap::new(),
        }
    }

    /// Number of strings stored in the map.
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
    pub fn has(&self, key: String) -> Result<bool> {
        let bytes = str_to_nul_free_bytes(&key)?;
        Ok(self.inner.get(bytes).is_some())
    }

    /// Sets `map[key] = value`. Returns previous value as BigInt if present, or `null`.
    #[napi]
    pub fn set(&mut self, key: String, value: KeyInput) -> Result<Option<BigInt>> {
        let bytes = str_to_nul_free_bytes(&key)?;
        let v = key_to_u64(value)?;
        Ok(self.inner.insert(bytes, v).map(BigInt::from))
    }

    /// Gets the value for `key`, or `null` if absent.
    #[napi]
    pub fn get(&self, key: String) -> Result<Option<BigInt>> {
        let bytes = str_to_nul_free_bytes(&key)?;
        Ok(self.inner.get(bytes).map(BigInt::from))
    }

    /// Deletes `key` from the map. Returns `true` if it was present, `false` otherwise.
    #[napi]
    pub fn delete(&mut self, key: String) -> Result<bool> {
        let bytes = str_to_nul_free_bytes(&key)?;
        Ok(self.inner.remove(bytes).is_some())
    }

    /// Removes all strings and frees allocated trie nodes.
    #[napi]
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Heap bytes used by the prefix trie.
    #[napi]
    pub fn mem_used(&self) -> BigInt {
        BigInt::from(self.inner.mem_used() as u64)
    }

    /// Smallest entry `(key, value)` in byte-lexicographical order, or `null` if empty.
    #[napi]
    pub fn first(&mut self) -> Option<StrMapEntry> {
        let (bytes, slot) = self.inner.first()?;
        // SAFETY: slot is guaranteed valid and non-null by ExpanseStrMap.
        let val = unsafe { *slot.as_ptr() };
        let s = String::from_utf8_lossy(&bytes).into_owned();
        Some(StrMapEntry {
            key: s,
            value: BigInt::from(val),
        })
    }

    /// Largest entry `(key, value)` in byte-lexicographical order, or `null` if empty.
    #[napi]
    pub fn last(&mut self) -> Option<StrMapEntry> {
        let (bytes, slot) = self.inner.last()?;
        // SAFETY: slot is guaranteed valid and non-null by ExpanseStrMap.
        let val = unsafe { *slot.as_ptr() };
        let s = String::from_utf8_lossy(&bytes).into_owned();
        Some(StrMapEntry {
            key: s,
            value: BigInt::from(val),
        })
    }

    /// Smallest entry with key strictly `> key` (or `>= key` if `inclusive` is `true`).
    #[napi]
    pub fn next(&mut self, key: String, inclusive: Option<bool>) -> Result<Option<StrMapEntry>> {
        let k = str_to_nul_free_bytes(&key)?;
        let entry = if inclusive.unwrap_or(false) {
            self.inner.next_at_or_after(k)
        } else {
            self.inner.next_after(k)
        };
        Ok(entry.map(|(bytes, slot)| {
            // SAFETY: slot is guaranteed valid and non-null by ExpanseStrMap.
            let val = unsafe { *slot.as_ptr() };
            StrMapEntry {
                key: String::from_utf8_lossy(&bytes).into_owned(),
                value: BigInt::from(val),
            }
        }))
    }

    /// Largest entry with key strictly `< key` (or `<= key` if `inclusive` is `true`).
    #[napi]
    pub fn prev(&mut self, key: String, inclusive: Option<bool>) -> Result<Option<StrMapEntry>> {
        let k = str_to_nul_free_bytes(&key)?;
        let entry = if inclusive.unwrap_or(false) {
            self.inner.prev_at_or_before(k)
        } else {
            self.inner.prev_before(k)
        };
        Ok(entry.map(|(bytes, slot)| {
            // SAFETY: slot is guaranteed valid and non-null by ExpanseStrMap.
            let val = unsafe { *slot.as_ptr() };
            StrMapEntry {
                key: String::from_utf8_lossy(&bytes).into_owned(),
                value: BigInt::from(val),
            }
        }))
    }

    fn collect_entries(&mut self) -> Vec<StrMapEntry> {
        let mut items = Vec::with_capacity(self.inner.len() as usize);
        let mut cur: Option<Vec<u8>> = None;
        loop {
            let next_entry = match &cur {
                None => self.inner.first(),
                Some(prev_k) => self.inner.next_after(prev_k),
            };
            match next_entry {
                Some((bytes, slot)) => {
                    // SAFETY: slot is guaranteed valid and non-null by ExpanseStrMap.
                    let val = unsafe { *slot.as_ptr() };
                    cur = Some(bytes.clone());
                    items.push(StrMapEntry {
                        key: String::from_utf8_lossy(&bytes).into_owned(),
                        value: BigInt::from(val),
                    });
                }
                None => break,
            }
        }
        items
    }

    /// Returns an array of all keys in byte-lexicographical order.
    #[napi]
    pub fn keys(&mut self) -> Vec<String> {
        self.collect_entries().into_iter().map(|e| e.key).collect()
    }

    /// Returns an array of all values in key-ascending order.
    #[napi]
    pub fn values(&mut self) -> Vec<BigInt> {
        self.collect_entries()
            .into_iter()
            .map(|e| e.value)
            .collect()
    }

    /// Returns an array of all `{ key, value }` entries in byte-lexicographical order.
    #[napi]
    pub fn entries(&mut self) -> Vec<StrMapEntry> {
        self.collect_entries()
    }
}

impl Default for ExpanseStrMap {
    fn default() -> Self {
        Self::new()
    }
}
