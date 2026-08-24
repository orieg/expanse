use js_sys::BigUint64Array;
use std::collections::BTreeSet;
use wasm_bindgen::prelude::*;

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
