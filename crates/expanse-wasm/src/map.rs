use js_sys::Array;
use std::collections::BTreeMap;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmExpanseMap {
    inner: BTreeMap<u64, u64>,
}

#[wasm_bindgen]
impl WasmExpanseMap {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    pub fn set(&mut self, key: u64, value: u64) {
        self.inner.insert(key, value);
    }

    pub fn get(&self, key: u64) -> Option<u64> {
        self.inner.get(&key).copied()
    }

    pub fn delete(&mut self, key: u64) -> bool {
        self.inner.remove(&key).is_some()
    }

    pub fn contains(&self, key: u64) -> bool {
        self.inner.contains_key(&key)
    }

    pub fn size(&self) -> u64 {
        self.inner.len() as u64
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }

    pub fn first(&self) -> Option<Array> {
        self.inner.iter().next().map(|(&k, &v)| {
            let arr = Array::new();
            arr.push(&JsValue::from(k));
            arr.push(&JsValue::from(v));
            arr
        })
    }

    pub fn last(&self) -> Option<Array> {
        self.inner.iter().next_back().map(|(&k, &v)| {
            let arr = Array::new();
            arr.push(&JsValue::from(k));
            arr.push(&JsValue::from(v));
            arr
        })
    }

    #[wasm_bindgen(js_name = next)]
    pub fn next_after(&self, key: u64) -> Option<Array> {
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

    #[wasm_bindgen(js_name = prev)]
    pub fn prev_before(&self, key: u64) -> Option<Array> {
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

impl Default for WasmExpanseMap {
    fn default() -> Self {
        Self::new()
    }
}
