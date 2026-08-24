use expanse_trie::map::ExpanseMap;
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
        self.inner.insert(key, value);
    }

    pub fn get(&self, key: u64) -> Option<u64> {
        self.inner.get(key)
    }

    pub fn delete(&mut self, key: u64) -> bool {
        self.inner.remove(key)
    }

    pub fn contains(&self, key: u64) -> bool {
        self.inner.contains(key)
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
            arr.push(&JsValue::from(k));
            arr.push(&JsValue::from(v));
            arr
        })
    }

    pub fn next(&self, key: u64) -> Option<js_sys::Array> {
        self.inner.next(key).map(|(k, v)| {
            let arr = js_sys::Array::new();
            arr.push(&JsValue::from(k));
            arr.push(&JsValue::from(v));
            arr
        })
    }
}
