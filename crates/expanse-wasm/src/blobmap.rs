use expanse_trie::ExpanseBlobMap;
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
        self.inner.insert(key as u32, payload, hot_meta.try_into().unwrap_or(0));
    }

    pub fn get(&self, key: u64) -> Option<Uint8Array> {
        self.inner.get(key as u32).map(|v| {
            let slice = v.0.as_bytes();
            let arr = Uint8Array::new_with_length(slice.len() as u32);
            arr.copy_from(slice);
            arr
        })
    }

    #[wasm_bindgen(js_name = getWithMeta)]
    pub fn get_with_meta(&self, key: u64) -> JsValue {
        if let Some((view, meta)) = self.inner.get(key as u32) {
            let slice = view.as_bytes();
            let arr = js_sys::Array::new();
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
        self.inner.remove(key as u32)
    }

    pub fn contains(&self, key: u64) -> bool {
        self.inner.get(key as u32).is_some()
    }

    pub fn size(&self) -> u64 {
        self.inner.len() as u64
    }

    pub fn clear(&mut self) {
        unimplemented!()
    }

    pub fn prune(&mut self, _predicate: &js_sys::Function) -> Result<usize, JsValue> {
        unimplemented!()
    }

    #[wasm_bindgen(js_name = saveImage)]
    pub fn save_image(&self) -> Uint8Array {
        unimplemented!()
    }

    #[wasm_bindgen(js_name = fromImage)]
    pub fn from_image(_data: &[u8]) -> Result<WasmExpanseBlobMap, JsValue> {
        unimplemented!()
    }
}
