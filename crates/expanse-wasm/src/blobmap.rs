use expanse_trie::blobmap::ExpanseBlobMap;
use wasm_bindgen::prelude::*;
use js_sys::Uint8Array;

#[wasm_bindgen]
pub struct WasmExpanseBlobMap {
    inner: ExpanseBlobMap,
}

#[wasm_bindgen]
impl WasmExpanseBlobMap {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: ExpanseBlobMap::new(),
        }
    }

    pub fn set(&mut self, key: u64, payload: &[u8], hot_meta: u32) {
        self.inner.insert(key, payload, hot_meta);
    }

    pub fn get(&self, key: u64) -> Option<Uint8Array> {
        self.inner.get(key).map(|v| {
            let arr = Uint8Array::new_with_length(v.len() as u32);
            arr.copy_from(v);
            arr
        })
    }

    #[wasm_bindgen(js_name = getWithMeta)]
    pub fn get_with_meta(&self, key: u64) -> JsValue {
        if let Some((payload, meta)) = self.inner.get_with_meta(key) {
            let arr = js_sys::Array::new();
            let uint8arr = Uint8Array::new_with_length(payload.len() as u32);
            uint8arr.copy_from(payload);
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
        self.inner.contains(key)
    }

    pub fn size(&self) -> u64 {
        self.inner.len() as u64
    }

    pub fn clear(&mut self) {
        self.inner.clear()
    }

    pub fn prune(&mut self, predicate: &js_sys::Function) -> Result<usize, JsValue> {
        let mut count = 0;
        let mut keys_to_remove = Vec::new();
        for (k, _payload, meta) in self.inner.iter() {
            let this = JsValue::null();
            let key_val = JsValue::from(k);
            let meta_val = JsValue::from(meta);
            let res = predicate.call2(&this, &key_val, &meta_val)?;
            if res.is_truthy() {
                keys_to_remove.push(k);
            }
        }
        for k in keys_to_remove {
            self.inner.remove(k);
            count += 1;
        }
        Ok(count)
    }

    #[wasm_bindgen(js_name = saveImage)]
    pub fn save_image(&self) -> Uint8Array {
        let image = self.inner.save_image();
        let arr = Uint8Array::new_with_length(image.len() as u32);
        arr.copy_from(&image);
        arr
    }

    #[wasm_bindgen(js_name = fromImage)]
    pub fn from_image(data: &[u8]) -> Result<WasmExpanseBlobMap, JsValue> {
        match ExpanseBlobMap::from_image(data) {
            Ok(inner) => Ok(WasmExpanseBlobMap { inner }),
            Err(e) => Err(JsValue::from_str(&e.to_string())),
        }
    }
}
