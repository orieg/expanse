use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmExpanseBytesMap {}

#[wasm_bindgen]
impl WasmExpanseBytesMap {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        panic!("ExpanseBytesMap is not supported on 32-bit platforms (wasm32)");
    }

    pub fn set(&mut self, _key: &[u8], _value: u64) {
        unimplemented!()
    }

    pub fn get(&self, _key: &[u8]) -> Option<u64> {
        unimplemented!()
    }

    pub fn delete(&mut self, _key: &[u8]) -> bool {
        unimplemented!()
    }

    pub fn contains(&self, _key: &[u8]) -> bool {
        unimplemented!()
    }

    pub fn size(&self) -> u64 {
        unimplemented!()
    }

    pub fn clear(&mut self) {
        unimplemented!()
    }
}
