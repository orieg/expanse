//! Node.js / Bun / Deno N-API binding for ExpanseSet (sparse 64-bit integer digital trie set).

use crate::common::{KeyInput, key_to_u64};
use expanse_trie::set::ExpanseSet as InnerSet;
use napi::bindgen_prelude::*;
use napi_derive::napi;

/// A sparse, dynamic 64-bit unsigned integer set (compat: Judy1).
///
/// Built on a 256-ary adaptive digital trie with cache-line-tuned node
/// geometries and zero-allocation root leaves for small populations.
#[napi]
pub struct ExpanseSet {
    pub(crate) inner: InnerSet,
}

#[napi]
impl ExpanseSet {
    /// Creates an empty set, optionally initialized from an array of keys.
    #[napi(constructor)]
    pub fn new(keys: Option<Vec<KeyInput>>) -> Result<Self> {
        let mut inner = InnerSet::new();
        if let Some(list) = keys {
            for item in list {
                let k = key_to_u64(item)?;
                inner.insert(k);
            }
        }
        Ok(Self { inner })
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

    /// Membership test `has(key)`. Returns `true` if `key` is present in the set.
    #[napi]
    pub fn has(&self, key: KeyInput) -> Result<bool> {
        let k = key_to_u64(key)?;
        Ok(self.inner.contains(k))
    }

    /// Checks membership for a batch of keys, returning an array of booleans.
    #[napi]
    pub fn contains_batch(&self, keys: Vec<KeyInput>) -> Result<Vec<bool>> {
        let mut u_keys = Vec::with_capacity(keys.len());
        for k in keys {
            u_keys.push(key_to_u64(k)?);
        }
        let mut out = vec![false; u_keys.len()];
        self.inner.contains_batch(&u_keys, &mut out);
        Ok(out)
    }

    /// Inserts `key` into the set. Returns `true` if newly inserted, `false` if already present.
    #[napi]
    pub fn add(&mut self, key: KeyInput) -> Result<bool> {
        let k = key_to_u64(key)?;
        Ok(self.inner.insert(k))
    }

    /// Removes `key` from the set. Returns `true` if it was present, `false` otherwise.
    #[napi]
    pub fn remove(&mut self, key: KeyInput) -> Result<bool> {
        let k = key_to_u64(key)?;
        Ok(self.inner.remove(k))
    }

    /// Removes all elements from the set.
    #[napi]
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Heap bytes used by the trie allocations.
    #[napi]
    pub fn mem_used(&self) -> BigInt {
        BigInt::from(self.inner.mem_used() as u64)
    }

    /// Smallest element in the set, or `null` if empty.
    #[napi]
    pub fn first(&self) -> Option<BigInt> {
        self.inner.first().map(BigInt::from)
    }

    /// Largest element in the set, or `null` if empty.
    #[napi]
    pub fn last(&self) -> Option<BigInt> {
        self.inner.last().map(BigInt::from)
    }

    /// Smallest element strictly `> key` (or `>= key` if `inclusive` is `true`).
    #[napi]
    pub fn next(&self, key: KeyInput, inclusive: Option<bool>) -> Result<Option<BigInt>> {
        let k = key_to_u64(key)?;
        let val = if inclusive.unwrap_or(false) {
            self.inner.next_at_or_after(k)
        } else {
            self.inner.next_after(k)
        };
        Ok(val.map(BigInt::from))
    }

    /// Largest element strictly `< key` (or `<= key` if `inclusive` is `true`).
    #[napi]
    pub fn prev(&self, key: KeyInput, inclusive: Option<bool>) -> Result<Option<BigInt>> {
        let k = key_to_u64(key)?;
        let val = if inclusive.unwrap_or(false) {
            self.inner.prev_at_or_before(k)
        } else {
            self.inner.prev_before(k)
        };
        Ok(val.map(BigInt::from))
    }

    /// Number of keys strictly below `key` (rank).
    #[napi]
    pub fn rank(&self, key: KeyInput) -> Result<BigInt> {
        let k = key_to_u64(key)?;
        Ok(BigInt::from(self.inner.count_below(k)))
    }

    /// The element with `k` keys below it (0-based select), or `null` if out of bounds.
    #[napi]
    pub fn select(&self, k: KeyInput) -> Result<Option<BigInt>> {
        let idx = key_to_u64(k)?;
        Ok(self.inner.by_count(idx).map(BigInt::from))
    }

    /// Number of keys in the closed range `[start, end]`.
    #[napi(js_name = "countRange")]
    pub fn count_range(&self, start: KeyInput, end: KeyInput) -> Result<BigInt> {
        let a = key_to_u64(start)?;
        let b = key_to_u64(end)?;
        Ok(BigInt::from(self.inner.count_range(a..=b)))
    }

    /// Returns an array of keys in the range `[start, end]`.
    #[napi]
    pub fn range(
        &self,
        start: Option<KeyInput>,
        end: Option<KeyInput>,
        inclusive: Option<bool>,
    ) -> Result<Vec<BigInt>> {
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
            .skip_while(|&k| k < from)
            .take_while(|&k| match max_k {
                Some(max) => {
                    if inc {
                        k <= max
                    } else {
                        k < max
                    }
                }
                None => true,
            })
            .map(BigInt::from)
            .collect();

        Ok(items)
    }

    /// Returns all elements in ascending order as an array of BigInts.
    #[napi(js_name = "toArray")]
    pub fn to_array(&self) -> Vec<BigInt> {
        self.inner.iter().map(BigInt::from).collect()
    }

    /// Bulk insertion from an array of numbers or BigInts.
    #[napi(js_name = "insertMany")]
    pub fn insert_many(&mut self, keys: Vec<KeyInput>) -> Result<u32> {
        let mut count = 0u32;
        for item in keys {
            let k = key_to_u64(item)?;
            if self.inner.insert(k) {
                count += 1;
            }
        }
        Ok(count)
    }
}
