use expanse_trie::ExpanseMap;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmExpanseMap {
    inner: ExpanseMap,
}

#[wasm_bindgen]
impl WasmExpanseMap {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: ExpanseMap::new(),
        }
    }

    pub fn set(&mut self, key: u64, value: u64) {
        self.inner.insert(key as u32, value as u32);
    }

    pub fn get(&self, key: u64) -> Option<u64> {
        self.inner.get(key as u32).map(|v| v as u64)
    }

    pub fn delete(&mut self, key: u64) -> bool {
        self.inner.remove(key as u32).is_some()
    }

    pub fn contains(&self, key: u64) -> bool {
        self.inner.contains_key(key as u32)
    }

    pub fn size(&self) -> u64 {
        self.inner.len() as u64
    }

    pub fn clear(&mut self) {
        self.inner.clear()
    }

    pub fn first(&self) -> Option<js_sys::Array> {
        self.inner.first().map(|(k, v)| {
            let arr = js_sys::Array::new();
            arr.push(&JsValue::from(k as u64));
            arr.push(&JsValue::from(v as u64));
            arr
        })
    }

    pub fn next(&self, key: u64) -> Option<js_sys::Array> {
        self.inner.next(key as u32).map(|(k, v)| {
            let arr = js_sys::Array::new();
            arr.push(&JsValue::from(k as u64));
            arr.push(&JsValue::from(v as u64));
            arr
        })
    }
}

