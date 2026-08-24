use std::collections::BTreeMap;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmExpanseBytesMap {
    inner: BTreeMap<Vec<u8>, u64>,
}

#[wasm_bindgen]
impl WasmExpanseBytesMap {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    pub fn set(&mut self, key: &[u8], value: u64) {
        self.inner.insert(key.to_vec(), value);
    }

    pub fn get(&self, key: &[u8]) -> Option<u64> {
        self.inner.get(key).copied()
    }

    pub fn delete(&mut self, key: &[u8]) -> bool {
        self.inner.remove(key).is_some()
    }

    pub fn contains(&self, key: &[u8]) -> bool {
        self.inner.contains_key(key)
    }

    pub fn size(&self) -> u64 {
        self.inner.len() as u64
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

impl Default for WasmExpanseBytesMap {
    fn default() -> Self {
        Self::new()
    }
}
