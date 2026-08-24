use expanse_trie::strmap::ExpanseStrMap;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmExpanseStrMap {
    inner: ExpanseStrMap,
}

#[wasm_bindgen]
impl WasmExpanseStrMap {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: ExpanseStrMap::new(),
        }
    }

    pub fn set(&mut self, key: &str, value: u64) {
        self.inner.insert(key, value);
    }

    pub fn get(&self, key: &str) -> Option<u64> {
        self.inner.get(key)
    }

    pub fn delete(&mut self, key: &str) -> bool {
        self.inner.remove(key)
    }

    pub fn contains(&self, key: &str) -> bool {
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

    pub fn next(&self, key: &str) -> Option<js_sys::Array> {
        self.inner.next(key).map(|(k, v)| {
            let arr = js_sys::Array::new();
            arr.push(&JsValue::from(k));
            arr.push(&JsValue::from(v));
            arr
        })
    }

    #[wasm_bindgen(js_name = keysWithPrefix)]
    pub fn keys_with_prefix(&self, prefix: &str) -> js_sys::Array {
        let arr = js_sys::Array::new();
        for (k, _) in self.inner.iter_prefix(prefix) {
            arr.push(&JsValue::from(k));
        }
        arr
    }
}
