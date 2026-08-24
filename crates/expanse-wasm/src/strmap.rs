use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmExpanseStrMap {}

#[wasm_bindgen]
impl WasmExpanseStrMap {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        panic!("ExpanseStrMap is not supported on 32-bit platforms (wasm32)");
    }

    pub fn set(&mut self, _key: &str, _value: u64) {
        unimplemented!()
    }

    pub fn get(&self, _key: &str) -> Option<u64> {
        unimplemented!()
    }

    pub fn delete(&mut self, _key: &str) -> bool {
        unimplemented!()
    }

    pub fn contains(&self, _key: &str) -> bool {
        unimplemented!()
    }

    pub fn size(&self) -> u64 {
        unimplemented!()
    }

    pub fn clear(&mut self) {
        unimplemented!()
    }

    pub fn first(&self) -> Option<js_sys::Array> {
        unimplemented!()
    }

    pub fn next(&self, _key: &str) -> Option<js_sys::Array> {
        unimplemented!()
    }

    #[wasm_bindgen(js_name = keysWithPrefix)]
    pub fn keys_with_prefix(&self, _prefix: &str) -> js_sys::Array {
        unimplemented!()
    }
}
