use expanse_trie::blobmap::ExpanseBlobMap;
use js_sys::Uint8Array;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmExpanseBlobMap {
    inner: ExpanseBlobMap,
}

#[wasm_bindgen]
impl WasmExpanseBlobMap {
    #[wasm_bindgen(constructor)]
    pub fn new(chunk_size: Option<u32>) -> Self {
        let inner = match chunk_size {
            Some(sz) => ExpanseBlobMap::with_chunk_size(sz as usize),
            None => ExpanseBlobMap::new(),
        };
        Self { inner }
    }

    pub fn set(&mut self, key: u64, payload: &[u8], hot_meta: Option<u32>) -> Result<(), JsValue> {
        let meta = hot_meta.unwrap_or(0);
        self.inner
            .insert(key, payload, meta)
            .map_err(|e| JsValue::from_str(&format!("Blob insertion error: {e}")))
    }

    pub fn get(&self, key: u64) -> Option<Uint8Array> {
        self.inner.get(key).map(|(v, _meta)| {
            let slice: &[u8] = &v;
            let arr = Uint8Array::new_with_length(slice.len() as u32);
            arr.copy_from(slice);
            arr
        })
    }

    #[wasm_bindgen(js_name = getWithMeta)]
    pub fn get_with_meta(&self, key: u64) -> JsValue {
        if let Some((payload, meta)) = self.inner.get(key) {
            let arr = js_sys::Array::new();
            let slice: &[u8] = &payload;
            let uint8arr = Uint8Array::new_with_length(slice.len() as u32);
            uint8arr.copy_from(slice);
            arr.push(&uint8arr);
            arr.push(&JsValue::from(meta));
            JsValue::from(arr)
        } else {
            JsValue::NULL
        }
    }

    pub fn delete(&mut self, key: u64) -> bool {
        self.inner.remove(key)
    }

    pub fn contains(&self, key: u64) -> bool {
        self.inner.contains_key(key)
    }

    pub fn size(&self) -> u64 {
        self.inner.len() as u64
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }

    pub fn prune(&mut self, predicate: &js_sys::Function) -> Result<usize, JsValue> {
        let mut keys_to_remove = Vec::new();
        for (k, _payload, meta) in self.inner.iter() {
            let this = JsValue::null();
            let key_val = JsValue::from(k);
            let meta_val = JsValue::from(meta);
            if let Ok(res) = predicate.call2(&this, &key_val, &meta_val) {
                if res.is_truthy() {
                    keys_to_remove.push(k);
                }
            }
        }
        let count = keys_to_remove.len();
        for k in keys_to_remove {
            self.inner.remove(k);
        }
        Ok(count)
    }
}

impl Default for WasmExpanseBlobMap {
    fn default() -> Self {
        Self::new(None)
    }
}
