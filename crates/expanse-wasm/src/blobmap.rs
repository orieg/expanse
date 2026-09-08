use js_sys::Uint8Array;
use std::collections::BTreeMap;
use wasm_bindgen::prelude::*;

/// **Not backed by the Expanse engine.** This is a `BTreeMap<u64, (Vec<u8>, u32)>` placeholder that
/// carries an `Expanse*` name; it has none of the trie's memory profile,
/// `mem_used` accounting, or node structure, and a figure measured through it
/// says nothing about the engine ([#757](https://github.com/orieg/expanse/issues/757)).
///
/// The engine's `ExpanseBlobMap` is `#[cfg(target_pointer_width = "64")]` in
/// `expanse-trie` and has no 32-bit twin, so it does not exist on `wasm32`.
/// `WasmExpanseMap32` and `WasmExpanseSet32` are the trie-backed classes on
/// this target; `WasmExpanseMap` uses the real engine on `wasm64` and falls
/// back to `BTreeMap` on `wasm32`, and says so.
/// The constructor's whole contract, as a plain function.
///
/// Split out from the constructor so it can be tested: `JsValue` aborts on a
/// non-wasm target, so a native test cannot reach the error path through
/// `new`. CI runs `cargo test -p expanse-wasm` natively, so a check that lived
/// only inside the constructor would have no test that actually executes.
fn check_chunk_size(chunk_size: Option<usize>) -> Result<(), &'static str> {
    if chunk_size.is_some() {
        return Err(
            "WasmExpanseBlobMap: chunk_size is not supported — this class is a BTreeMap \
             placeholder with no arena to size (see issue #757). Construct it with no argument.",
        );
    }
    Ok(())
}

#[wasm_bindgen]
#[derive(Default)]
pub struct WasmExpanseBlobMap {
    inner: BTreeMap<u64, (Vec<u8>, u32)>,
}

#[wasm_bindgen]
impl WasmExpanseBlobMap {
    /// Rejects `chunk_size` rather than accepting and discarding it (#757).
    ///
    /// The parameter documented an arena tuning knob. There is no arena — the
    /// backing is a `BTreeMap` — so a caller sizing it got no error and no
    /// effect. Backing this class with `expanse_trie::blobmap32::ExpanseBlobMap32`
    /// would not fix that either: that type takes no chunk size (its arena is a
    /// `Vec<Option<Vec<u8>>>`, not chunked) and its keys are 32-bit against this
    /// class's 64-bit, so adopting it would narrow a published API without
    /// honouring the parameter. Refusing is the honest option until an arena
    /// exists to size.
    #[wasm_bindgen(constructor)]
    pub fn new(chunk_size: Option<usize>) -> Result<WasmExpanseBlobMap, JsValue> {
        check_chunk_size(chunk_size).map_err(JsValue::from_str)?;
        Ok(Self {
            inner: BTreeMap::new(),
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The constructor must refuse `chunk_size`, not accept and drop it (#757).
    ///
    /// The defect this pins is silent: the old signature took
    /// `_chunk_size: Option<usize>` and ignored it, so a caller sizing the
    /// arena got no error and no effect. A test that only checked the map
    /// works would have passed then and passes now, which is why this asserts
    /// on the refusal itself.
    #[test]
    fn chunk_size_is_refused_rather_than_silently_dropped() {
        assert!(
            check_chunk_size(None).is_ok(),
            "the no-argument form is the supported one"
        );
        let err = check_chunk_size(Some(4096))
            .expect_err("chunk_size must be refused: there is no arena to size");
        assert!(
            err.contains("chunk_size"),
            "the refusal must name the parameter: {err}"
        );
        assert!(err.contains("#757"), "the refusal must cite why: {err}");
    }

    #[test]
    fn the_placeholder_still_stores_and_returns_what_it_was_given() {
        let mut m = WasmExpanseBlobMap::default();
        assert!(m.set(7, b"payload", Some(3)));
        assert!(m.contains(7));
        assert_eq!(m.size(), 1);
        assert!(m.delete(7));
        assert!(!m.contains(7));
        assert_eq!(m.size(), 0);
    }
}
