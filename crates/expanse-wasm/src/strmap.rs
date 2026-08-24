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

    pub fn set(&mut self, key: &str, value: u64) -> Option<u64> {
        self.inner.insert(key.as_bytes(), value)
    }

    pub fn get(&self, key: &str) -> Option<u64> {
        self.inner.get(key.as_bytes())
    }

    pub fn delete(&mut self, key: &str) -> bool {
        self.inner.remove(key.as_bytes()).is_some()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.inner.get(key.as_bytes()).is_some()
    }

    pub fn size(&self) -> u64 {
        self.inner.len() as u64
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }

    pub fn first(&mut self) -> Option<js_sys::Array> {
        self.inner.first().map(|(k, slot)| {
            let val = unsafe { *slot.as_ptr() };
            let arr = js_sys::Array::new();
            let key_str = String::from_utf8_lossy(&k).into_owned();
            arr.push(&JsValue::from_str(&key_str));
            arr.push(&JsValue::from(val));
            arr
        })
    }

    pub fn next(&mut self, key: &str) -> Option<js_sys::Array> {
        self.inner.next_after(key.as_bytes()).map(|(k, slot)| {
            let val = unsafe { *slot.as_ptr() };
            let arr = js_sys::Array::new();
            let key_str = String::from_utf8_lossy(&k).into_owned();
            arr.push(&JsValue::from_str(&key_str));
            arr.push(&JsValue::from(val));
            arr
        })
    }
}

impl Default for WasmExpanseStrMap {
    fn default() -> Self {
        Self::new()
    }
}
