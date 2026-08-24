#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::*;
wasm_bindgen_test_configure!(run_in_browser);

use expanse_wasm::set::WasmExpanseSet;

#[wasm_bindgen_test]
fn test_set() {
    let mut set = WasmExpanseSet::new();
    set.add(42);
    assert!(set.contains(42));
    assert_eq!(set.size(), 1);
    set.remove(42);
    assert!(!set.contains(42));
}
