use js_sys::Array;
use std::collections::BTreeMap;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmExpanseStrMap {
    inner: BTreeMap<String, u64>,
}

#[wasm_bindgen]
impl WasmExpanseStrMap {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    pub fn set(&mut self, key: &str, value: u64) {
        self.inner.insert(key.to_string(), value);
    }

    pub fn get(&self, key: &str) -> Option<u64> {
        self.inner.get(key).copied()
    }

    pub fn delete(&mut self, key: &str) -> bool {
        self.inner.remove(key).is_some()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.inner.contains_key(key)
    }

    pub fn size(&self) -> u64 {
        self.inner.len() as u64
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }

    pub fn first(&self) -> Option<Array> {
        self.inner.iter().next().map(|(k, &v)| {
            let arr = Array::new();
            arr.push(&JsValue::from_str(k));
            arr.push(&JsValue::from(v));
            arr
        })
    }

    #[wasm_bindgen(js_name = next)]
    pub fn next_after(&self, key: &str) -> Option<Array> {
        use std::ops::Bound::Excluded;
        self.inner
            .range((Excluded(key.to_string()), std::ops::Bound::Unbounded))
            .next()
            .map(|(k, &v)| {
                let arr = Array::new();
                arr.push(&JsValue::from_str(k));
                arr.push(&JsValue::from(v));
                arr
            })
    }
}

impl Default for WasmExpanseStrMap {
    fn default() -> Self {
        Self::new()
    }
}
