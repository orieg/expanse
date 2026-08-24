mod blobmap;
mod bytesmap;
mod map;
mod set;
mod strmap;

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn init_panic_hook() {
    // Optional panic hook initialization
}
