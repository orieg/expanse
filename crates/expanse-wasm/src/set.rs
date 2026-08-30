use js_sys::BigUint64Array;
use std::collections::BTreeSet;
use wasm_bindgen::prelude::*;

use expanse_trie::set32::ExpanseSet32;

/// 32-Bit Expanse Digital Trie Set compiled to WebAssembly.
/// Real 256-ary digital tree with adaptive node compression, bitset leaves, and zero-alloc immediates.
#[wasm_bindgen]
pub struct WasmExpanseSet32 {
    inner: ExpanseSet32,
}

#[wasm_bindgen]
impl WasmExpanseSet32 {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: ExpanseSet32::new(),
        }
    }

    pub fn add(&mut self, key: u32) -> bool {
        self.inner.insert(key)
    }

    pub fn remove(&mut self, key: u32) -> bool {
        self.inner.remove(key)
    }

    pub fn contains(&self, key: u32) -> bool {
        self.inner.contains(key)
    }

    pub fn size(&self) -> u32 {
        self.inner.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn mem_used(&self) -> usize {
        self.inner.mem_used()
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }

    pub fn first(&self) -> Option<u32> {
        self.inner.first()
    }

    pub fn last(&self) -> Option<u32> {
        self.inner.last()
    }

    #[wasm_bindgen(js_name = next)]
    pub fn next_after(&self, key: u32) -> Option<u32> {
        self.inner.next(key)
    }

    #[wasm_bindgen(js_name = prev)]
    pub fn prev_before(&self, key: u32) -> Option<u32> {
        self.inner.prev(key)
    }

    pub fn rank(&self, key: u32) -> u32 {
        if key == 0 {
            0
        } else {
            self.inner.count_range(0, key - 1) as u32
        }
    }

    #[wasm_bindgen(js_name = countRange)]
    pub fn count_range(&self, start: u32, end: u32) -> u32 {
        self.inner.count_range(start, end) as u32
    }

    /// Batch insert: runs the insert loop purely inside WebAssembly.
    pub fn batch_insert(&mut self, keys: &[u32]) -> u32 {
        for &k in keys {
            self.inner.insert(k);
        }
        self.inner.len() as u32
    }

    /// Batch contains: runs the lookup loop inside WebAssembly, returning hit count.
    pub fn batch_contains(&self, keys: &[u32]) -> u32 {
        let mut hits = 0u32;
        for &k in keys {
            if self.inner.contains(k) {
                hits += 1;
            }
        }
        hits
    }

    /// Batch range scan: scans forward up to `limit` keys starting at `start_key`.
    pub fn batch_range_scan(&self, start_key: u32, limit: u32) -> u32 {
        let mut checksum = 0u32;
        let mut count = 0u32;
        let mut curr = self.inner.next(start_key);
        while let Some(k) = curr {
            checksum ^= k;
            count += 1;
            if count >= limit {
                break;
            }
            curr = self.inner.next(k);
        }
        checksum
    }
}

impl Default for WasmExpanseSet32 {
    fn default() -> Self {
        Self::new()
    }
}

/// Comparative Baseline: Rust std `BTreeSet<u32>` in WebAssembly.
#[wasm_bindgen]
pub struct WasmBTreeSet32 {
    inner: BTreeSet<u32>,
}

#[wasm_bindgen]
impl WasmBTreeSet32 {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: BTreeSet::new(),
        }
    }

    pub fn add(&mut self, key: u32) -> bool {
        self.inner.insert(key)
    }

    pub fn remove(&mut self, key: u32) -> bool {
        self.inner.remove(&key)
    }

    pub fn contains(&self, key: u32) -> bool {
        self.inner.contains(&key)
    }

    pub fn size(&self) -> u32 {
        self.inner.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }

    pub fn batch_insert(&mut self, keys: &[u32]) -> u32 {
        for &k in keys {
            self.inner.insert(k);
        }
        self.inner.len() as u32
    }

    pub fn batch_contains(&self, keys: &[u32]) -> u32 {
        let mut hits = 0u32;
        for &k in keys {
            if self.inner.contains(&k) {
                hits += 1;
            }
        }
        hits
    }

    pub fn batch_range_scan(&self, start_key: u32, limit: u32) -> u32 {
        use std::ops::Bound::Excluded;
        let mut checksum = 0u32;
        let mut count = 0u32;
        for &k in self
            .inner
            .range((Excluded(start_key), std::ops::Bound::Unbounded))
        {
            checksum ^= k;
            count += 1;
            if count >= limit {
                break;
            }
        }
        checksum
    }
}

impl Default for WasmBTreeSet32 {
    fn default() -> Self {
        Self::new()
    }
}

/// Legacy 64-bit Set Wrapper.
#[wasm_bindgen]
pub struct WasmExpanseSet {
    inner: BTreeSet<u64>,
}

#[wasm_bindgen]
impl WasmExpanseSet {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: BTreeSet::new(),
        }
    }

    pub fn add(&mut self, key: u64) -> bool {
        self.inner.insert(key)
    }

    pub fn remove(&mut self, key: u64) -> bool {
        self.inner.remove(&key)
    }

    pub fn contains(&self, key: u64) -> bool {
        self.inner.contains(&key)
    }

    pub fn size(&self) -> u64 {
        self.inner.len() as u64
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }

    pub fn first(&self) -> Option<u64> {
        self.inner.iter().next().copied()
    }

    pub fn last(&self) -> Option<u64> {
        self.inner.iter().next_back().copied()
    }

    #[wasm_bindgen(js_name = next)]
    pub fn next_after(&self, key: u64) -> Option<u64> {
        use std::ops::Bound::Excluded;
        self.inner
            .range((Excluded(key), std::ops::Bound::Unbounded))
            .next()
            .copied()
    }

    #[wasm_bindgen(js_name = prev)]
    pub fn prev_before(&self, key: u64) -> Option<u64> {
        use std::ops::Bound::Excluded;
        self.inner
            .range((std::ops::Bound::Unbounded, Excluded(key)))
            .next_back()
            .copied()
    }

    pub fn rank(&self, key: u64) -> u64 {
        self.inner.range(..key).count() as u64
    }

    pub fn select(&self, k: u64) -> Option<u64> {
        self.inner.iter().nth(k as usize).copied()
    }

    #[wasm_bindgen(js_name = countRange)]
    pub fn count_range(&self, start: u64, end: u64) -> u64 {
        self.inner.range(start..=end).count() as u64
    }

    #[wasm_bindgen(js_name = toArray)]
    pub fn to_array(&self) -> BigUint64Array {
        let items: Vec<u64> = self.inner.iter().copied().collect();
        BigUint64Array::from(&items[..])
    }
}

impl Default for WasmExpanseSet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_expanse_set32_basic() {
        let mut set = WasmExpanseSet32::new();
        assert!(set.is_empty());
        assert_eq!(set.size(), 0);

        assert!(set.add(100));
        assert!(set.add(200));
        assert!(!set.add(100)); // duplicate
        assert_eq!(set.size(), 2);
        assert!(set.contains(100));
        assert!(set.contains(200));
        assert!(!set.contains(300));

        assert_eq!(set.first(), Some(100));
        assert_eq!(set.last(), Some(200));
        assert_eq!(set.next_after(100), Some(200));
        assert_eq!(set.prev_before(200), Some(100));

        assert_eq!(set.count_range(50, 150), 1);
        assert_eq!(set.count_range(50, 250), 2);
        assert_eq!(set.rank(150), 1);

        assert!(set.remove(100));
        assert_eq!(set.size(), 1);
        assert!(!set.contains(100));
    }

    #[test]
    fn test_wasm_expanse_set32_batch() {
        let mut set = WasmExpanseSet32::new();
        let keys = [10, 20, 30, 40, 50];

        assert_eq!(set.batch_insert(&keys), 5);
        assert_eq!(set.size(), 5);

        let probe = [10, 25, 30, 45, 50];
        let hits = set.batch_contains(&probe);
        assert_eq!(hits, 3);

        let scan_cs = set.batch_range_scan(20, 3);
        assert_ne!(scan_cs, 0);
    }

    #[test]
    fn test_wasm_btree_set32_batch() {
        let mut set = WasmBTreeSet32::new();
        let keys = [10, 20, 30, 40, 50];

        assert_eq!(set.batch_insert(&keys), 5);
        let probe = [10, 25, 30, 45, 50];
        let hits = set.batch_contains(&probe);
        assert_eq!(hits, 3);

        let scan_cs = set.batch_range_scan(20, 3);
        assert_ne!(scan_cs, 0);
    }
}
