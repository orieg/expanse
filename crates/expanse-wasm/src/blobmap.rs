use js_sys::Uint8Array;
use std::collections::BTreeMap;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmExpanseBlobMap {
    inner: BTreeMap<u64, (Vec<u8>, u32)>,
}

#[wasm_bindgen]
impl WasmExpanseBlobMap {
    #[wasm_bindgen(constructor)]
    pub fn new(_chunk_size: Option<usize>) -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    pub fn set(&mut self, key: u64, payload: &[u8], hot_meta: Option<u32>) -> bool {
        self.inner
            .insert(key, (payload.to_vec(), hot_meta.unwrap_or(0)));
        true
    }

    pub fn get(&self, key: u64) -> Option<Uint8Array> {
        self.inner
            .get(&key)
            .map(|(payload, _)| Uint8Array::from(&payload[..]))
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
}

impl Default for WasmExpanseBlobMap {
    fn default() -> Self {
        Self::new(None)
    }
}
