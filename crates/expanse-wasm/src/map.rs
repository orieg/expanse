use js_sys::Array;
use std::collections::BTreeMap;
use wasm_bindgen::prelude::*;

use expanse_trie::map32::ExpanseMap32;

/// 32-Bit Expanse Digital Trie Map compiled to WebAssembly.
/// Real 256-ary digital tree with adaptive node compression and zero-alloc immediates.
#[wasm_bindgen]
pub struct WasmExpanseMap32 {
    inner: ExpanseMap32,
}

#[wasm_bindgen]
impl WasmExpanseMap32 {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: ExpanseMap32::new(),
        }
    }

    pub fn set(&mut self, key: u32, value: u32) {
        self.inner.insert(key, value);
    }

    pub fn get(&self, key: u32) -> Option<u32> {
        self.inner.get(key)
    }

    pub fn contains(&self, key: u32) -> bool {
        self.inner.contains_key(key)
    }

    pub fn delete(&mut self, key: u32) -> bool {
        self.inner.remove(key).is_some()
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

    pub fn first(&self) -> Option<Array> {
        self.inner.first().map(|(k, v)| {
            let arr = Array::new();
            arr.push(&JsValue::from(k));
            arr.push(&JsValue::from(v));
            arr
        })
    }

    pub fn last(&self) -> Option<Array> {
        self.inner.last().map(|(k, v)| {
            let arr = Array::new();
            arr.push(&JsValue::from(k));
            arr.push(&JsValue::from(v));
            arr
        })
    }

    #[wasm_bindgen(js_name = next)]
    pub fn next_after(&self, key: u32) -> Option<Array> {
        self.inner.next(key).map(|(k, v)| {
            let arr = Array::new();
            arr.push(&JsValue::from(k));
            arr.push(&JsValue::from(v));
            arr
        })
    }

    #[wasm_bindgen(js_name = prev)]
    pub fn prev_before(&self, key: u32) -> Option<Array> {
        self.inner.prev(key).map(|(k, v)| {
            let arr = Array::new();
            arr.push(&JsValue::from(k));
            arr.push(&JsValue::from(v));
            arr
        })
    }

    /// Batch insert: runs the insert loop purely inside WebAssembly,
    /// eliminating the 250,000 JS-to-WASM FFI boundary transitions.
    pub fn batch_insert(&mut self, keys: &[u32], values: &[u32]) -> u32 {
        let count = keys.len().min(values.len());
        for i in 0..count {
            self.inner.insert(keys[i], values[i]);
        }
        self.inner.len() as u32
    }

    /// Batch lookup: runs the lookup loop purely inside WebAssembly,
    /// returning a non-zero XOR sink checksum to guarantee against dead-code elimination.
    pub fn batch_lookup(&self, keys: &[u32]) -> u32 {
        let mut checksum = 0u32;
        for &k in keys {
            if let Some(v) = self.inner.get(k) {
                checksum ^= v;
            }
        }
        checksum
    }

    /// Batch range scan: traverses ordered entries starting at `start_key`.
    pub fn batch_range_scan(&self, start_key: u32, limit: u32) -> u32 {
        let mut checksum = 0u32;
        let mut count = 0u32;
        let mut curr = self.inner.next(start_key);
        while let Some((k, v)) = curr {
            checksum ^= k ^ v;
            count += 1;
            if count >= limit {
                break;
            }
            curr = self.inner.next(k);
        }
        checksum
    }
}

impl Default for WasmExpanseMap32 {
    fn default() -> Self {
        Self::new()
    }
}

/// Comparative Baseline: Rust std `BTreeMap<u32, u32>` in WebAssembly.
#[wasm_bindgen]
pub struct WasmBTreeMap32 {
    inner: BTreeMap<u32, u32>,
}

#[wasm_bindgen]
impl WasmBTreeMap32 {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    pub fn set(&mut self, key: u32, value: u32) {
        self.inner.insert(key, value);
    }

    pub fn get(&self, key: u32) -> Option<u32> {
        self.inner.get(&key).copied()
    }

    pub fn contains(&self, key: u32) -> bool {
        self.inner.contains_key(&key)
    }

    pub fn delete(&mut self, key: u32) -> bool {
        self.inner.remove(&key).is_some()
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

    pub fn batch_insert(&mut self, keys: &[u32], values: &[u32]) -> u32 {
        let count = keys.len().min(values.len());
        for i in 0..count {
            self.inner.insert(keys[i], values[i]);
        }
        self.inner.len() as u32
    }

    pub fn batch_lookup(&self, keys: &[u32]) -> u32 {
        let mut checksum = 0u32;
        for &k in keys {
            if let Some(&v) = self.inner.get(&k) {
                checksum ^= v;
            }
        }
        checksum
    }

    pub fn batch_range_scan(&self, start_key: u32, limit: u32) -> u32 {
        use std::ops::Bound::Excluded;
        let mut checksum = 0u32;
        let mut count = 0u32;
        for (&k, &v) in self
            .inner
            .range((Excluded(start_key), std::ops::Bound::Unbounded))
        {
            checksum ^= k ^ v;
            count += 1;
            if count >= limit {
                break;
            }
        }
        checksum
    }
}

impl Default for WasmBTreeMap32 {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_pointer_width = "64")]
use expanse_trie::map::ExpanseMap;

/// 64-bit Map Wrapper (ExpanseMap on wasm64 / 64-bit targets, BTreeMap fallback on 32-bit).
#[wasm_bindgen]
pub struct WasmExpanseMap {
    #[cfg(target_pointer_width = "64")]
    inner: ExpanseMap,
    #[cfg(not(target_pointer_width = "64"))]
    inner: BTreeMap<u64, u64>,
}

#[wasm_bindgen]
impl WasmExpanseMap {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        #[cfg(target_pointer_width = "64")]
        {
            Self {
                inner: ExpanseMap::new(),
            }
        }
        #[cfg(not(target_pointer_width = "64"))]
        {
            Self {
                inner: BTreeMap::new(),
            }
        }
    }

    pub fn set(&mut self, key: u64, value: u64) {
        self.inner.insert(key, value);
    }

    pub fn get(&self, key: u64) -> Option<u64> {
        #[cfg(target_pointer_width = "64")]
        {
            self.inner.get(key)
        }
        #[cfg(not(target_pointer_width = "64"))]
        {
            self.inner.get(&key).copied()
        }
    }

    pub fn delete(&mut self, key: u64) -> bool {
        #[cfg(target_pointer_width = "64")]
        {
            self.inner.remove(key).is_some()
        }
        #[cfg(not(target_pointer_width = "64"))]
        {
            self.inner.remove(&key).is_some()
        }
    }

    pub fn contains(&self, key: u64) -> bool {
        #[cfg(target_pointer_width = "64")]
        {
            self.inner.contains_key(key)
        }
        #[cfg(not(target_pointer_width = "64"))]
        {
            self.inner.contains_key(&key)
        }
    }

    pub fn size(&self) -> u64 {
        #[cfg(target_pointer_width = "64")]
        {
            self.inner.len()
        }
        #[cfg(not(target_pointer_width = "64"))]
        {
            self.inner.len() as u64
        }
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }

    pub fn first(&self) -> Option<Array> {
        #[cfg(target_pointer_width = "64")]
        {
            self.inner.first().map(|(k, v)| {
                let arr = Array::new();
                arr.push(&JsValue::from(k));
                arr.push(&JsValue::from(v));
                arr
            })
        }
        #[cfg(not(target_pointer_width = "64"))]
        {
            self.inner.iter().next().map(|(&k, &v)| {
                let arr = Array::new();
                arr.push(&JsValue::from(k));
                arr.push(&JsValue::from(v));
                arr
            })
        }
    }

    pub fn last(&self) -> Option<Array> {
        #[cfg(target_pointer_width = "64")]
        {
            self.inner.last().map(|(k, v)| {
                let arr = Array::new();
                arr.push(&JsValue::from(k));
                arr.push(&JsValue::from(v));
                arr
            })
        }
        #[cfg(not(target_pointer_width = "64"))]
        {
            self.inner.iter().next_back().map(|(&k, &v)| {
                let arr = Array::new();
                arr.push(&JsValue::from(k));
                arr.push(&JsValue::from(v));
                arr
            })
        }
    }

    #[wasm_bindgen(js_name = next)]
    pub fn next_after(&self, key: u64) -> Option<Array> {
        #[cfg(target_pointer_width = "64")]
        {
            self.inner.next_after(key).map(|(k, v)| {
                let arr = Array::new();
                arr.push(&JsValue::from(k));
                arr.push(&JsValue::from(v));
                arr
            })
        }
        #[cfg(not(target_pointer_width = "64"))]
        {
            use std::ops::Bound::Excluded;
            self.inner
                .range((Excluded(key), std::ops::Bound::Unbounded))
                .next()
                .map(|(&k, &v)| {
                    let arr = Array::new();
                    arr.push(&JsValue::from(k));
                    arr.push(&JsValue::from(v));
                    arr
                })
        }
    }

    #[wasm_bindgen(js_name = prev)]
    pub fn prev_before(&self, key: u64) -> Option<Array> {
        #[cfg(target_pointer_width = "64")]
        {
            self.inner.prev_before(key).map(|(k, v)| {
                let arr = Array::new();
                arr.push(&JsValue::from(k));
                arr.push(&JsValue::from(v));
                arr
            })
        }
        #[cfg(not(target_pointer_width = "64"))]
        {
            use std::ops::Bound::Excluded;
            self.inner
                .range((std::ops::Bound::Unbounded, Excluded(key)))
                .next_back()
                .map(|(&k, &v)| {
                    let arr = Array::new();
                    arr.push(&JsValue::from(k));
                    arr.push(&JsValue::from(v));
                    arr
                })
        }
    }
}

impl Default for WasmExpanseMap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_expanse_map32_basic() {
        let mut map = WasmExpanseMap32::new();
        assert!(map.is_empty());
        assert_eq!(map.size(), 0);

        map.set(100, 500);
        map.set(200, 600);
        assert_eq!(map.size(), 2);
        assert_eq!(map.get(100), Some(500));
        assert_eq!(map.get(200), Some(600));
        assert_eq!(map.get(300), None);
        assert!(map.contains(100));
        assert!(!map.contains(300));

        assert!(map.delete(100));
        assert_eq!(map.size(), 1);
        assert_eq!(map.get(100), None);
    }

    #[test]
    fn test_wasm_expanse_map32_batch() {
        let mut map = WasmExpanseMap32::new();
        let keys = [10, 20, 30, 40, 50];
        let vals = [100, 200, 300, 400, 500];

        assert_eq!(map.batch_insert(&keys, &vals), 5);
        assert_eq!(map.size(), 5);

        let checksum = map.batch_lookup(&keys);
        let expected = 100 ^ 200 ^ 300 ^ 400 ^ 500;
        assert_eq!(checksum, expected);

        let scan_cs = map.batch_range_scan(20, 3);
        assert_ne!(scan_cs, 0);
    }

    #[test]
    fn test_wasm_btree_map32_batch() {
        let mut map = WasmBTreeMap32::new();
        let keys = [10, 20, 30, 40, 50];
        let vals = [100, 200, 300, 400, 500];

        assert_eq!(map.batch_insert(&keys, &vals), 5);
        let checksum = map.batch_lookup(&keys);
        let expected = 100 ^ 200 ^ 300 ^ 400 ^ 500;
        assert_eq!(checksum, expected);

        let scan_cs = map.batch_range_scan(20, 3);
        assert_ne!(scan_cs, 0);
    }
}
