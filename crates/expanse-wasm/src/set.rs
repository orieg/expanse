use expanse_trie::ExpanseSet;
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
        self.inner.insert(key as u32)
    }

    pub fn remove(&mut self, key: u64) -> bool {
        self.inner.remove(key as u32)
    }

    pub fn contains(&self, key: u64) -> bool {
        self.inner.contains(key as u32)
    }

    pub fn size(&self) -> u64 {
        self.inner.len() as u64
    }

    pub fn clear(&mut self) {
        self.inner.clear()
    }

    pub fn first(&self) -> Option<u64> {
        self.inner.first().map(|v| v as u64)
    }

    pub fn next(&self, key: u64) -> Option<u64> {
        self.inner.next(key as u32).map(|v| v as u64)
    }

    pub fn last(&self) -> Option<u64> {
        self.inner.last().map(|v| v as u64)
    }

    pub fn prev(&self, key: u64) -> Option<u64> {
        self.inner.prev(key as u32).map(|v| v as u64)
    }

    pub fn rank(&self, _key: u64) -> u64 {
        unimplemented!()
    }

    pub fn select(&self, _k: u64) -> Option<u64> {
        unimplemented!()
    }

    #[wasm_bindgen(js_name = countRange)]
    pub fn count_range(&self, start: u64, end: u64) -> u64 {
        self.inner.count_range(start as u32, end as u32) as u64
    }

    #[wasm_bindgen(js_name = toArray)]
    pub fn to_array(&self) -> js_sys::BigUint64Array {
        let mut vec: Vec<u64> = Vec::new();
        let mut current = self.inner.first();
        while let Some(k) = current {
            vec.push(k as u64);
            current = self.inner.next(k);
        }
        let array = js_sys::BigUint64Array::new_with_length(vec.len() as u32);
        array.copy_from(&vec);
        array
    }
}
