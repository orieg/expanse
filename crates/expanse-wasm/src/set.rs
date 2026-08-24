use expanse_trie::set::ExpanseSet;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmExpanseSet {
    inner: ExpanseSet,
}

#[wasm_bindgen]
impl WasmExpanseSet {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: ExpanseSet::new(),
        }
    }

    pub fn add(&mut self, key: u64) -> bool {
        self.inner.insert(key)
    }

    pub fn remove(&mut self, key: u64) -> bool {
        self.inner.remove(key)
    }

    pub fn contains(&self, key: u64) -> bool {
        self.inner.contains(key)
    }

    pub fn size(&self) -> u64 {
        self.inner.len() as u64
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }

    pub fn first(&self) -> Option<u64> {
        self.inner.first()
    }

    pub fn next(&self, key: u64) -> Option<u64> {
        self.inner.next_after(key)
    }

    pub fn last(&self) -> Option<u64> {
        self.inner.last()
    }

    pub fn prev(&self, key: u64) -> Option<u64> {
        self.inner.prev_before(key)
    }

    pub fn rank(&self, key: u64) -> u64 {
        self.inner.count_below(key)
    }

    pub fn select(&self, k: u64) -> Option<u64> {
        self.inner.by_count(k)
    }

    #[wasm_bindgen(js_name = countRange)]
    pub fn count_range(&self, start: u64, end: u64) -> u64 {
        self.inner.count_range(start..=end)
    }

    #[wasm_bindgen(js_name = toArray)]
    pub fn to_array(&self) -> js_sys::BigUint64Array {
        let vec: Vec<u64> = self.inner.iter().collect();
        let array = js_sys::BigUint64Array::new_with_length(vec.len() as u32);
        array.copy_from(&vec);
        array
    }
}

impl Default for WasmExpanseSet {
    fn default() -> Self {
        Self::new()
    }
}
