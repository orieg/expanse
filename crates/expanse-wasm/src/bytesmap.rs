use std::collections::BTreeMap;
use wasm_bindgen::prelude::*;

/// **Not backed by the Expanse engine.** This is a `BTreeMap<Vec<u8>, u64>` placeholder that
/// carries an `Expanse*` name; it has none of the trie's memory profile,
/// `mem_used` accounting, or node structure, and a figure measured through it
/// says nothing about the engine ([#757](https://github.com/orieg/expanse/issues/757)).
///
/// The engine's `ExpanseBytesMap` is `#[cfg(target_pointer_width = "64")]` in
/// `expanse-trie` and has no 32-bit twin, so it does not exist on `wasm32`.
/// `WasmExpanseMap32` and `WasmExpanseSet32` are the trie-backed classes on
/// this target; `WasmExpanseMap` uses the real engine on `wasm64` and falls
/// back to `BTreeMap` on `wasm32`, and says so.
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
