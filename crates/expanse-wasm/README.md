# @orieg/expanse-wasm

WebAssembly bindings for `expanse-trie`. Suitable for Edge runtimes (Cloudflare Workers, Fastly, Deno Deploy, Vercel) and modern browsers.

## Installation

```sh
npm install @orieg/expanse-wasm
```

## Quickstart

```js
import { WasmExpanseSet, init_panic_hook } from '@orieg/expanse-wasm';

// Initialize panic hook for better error messages
init_panic_hook();

const set = new WasmExpanseSet();
set.add(42n);
console.log(set.contains(42n)); // true
```

- `WasmExpanseSet`: `u64` values
- `WasmExpanseMap`: `u64` to `u64` map
- `WasmExpanseSet32`: 32-bit digital trie set with batch execution methods
- `WasmExpanseMap32`: 32-bit digital trie map with batch execution methods
- `WasmExpanseStrMap`: `String` to `u64` map
- `WasmExpanseBytesMap`: `&[u8]` to `u64` map
- `WasmExpanseBlobMap`: `u64` to `&[u8]` map with hot metadata

## WebAssembly Memory64 (`wasm64-unknown-unknown`) Support

Expanse supports 64-bit WebAssembly linear memory (`Memory64`), allowing the full 64-bit raw pointer engine (`ExpanseMap`, `ExpanseSet`, 16-byte `Edge` descriptors) to run directly inside WebAssembly runtimes supporting 64-bit memory spaces (e.g. Node.js with `--experimental-wasm-memory64` or modern V8 / Wasm runtimes).

To build for `wasm64-unknown-unknown`:
```sh
cargo +nightly build -p expanse-wasm --target wasm64-unknown-unknown -Z build-std=std,panic_abort
```

To run the Node.js Memory64 runtime smoke test:
```sh
node crates/expanse-wasm/tests/test_wasm64_smoke.js
```

## Interactive In-Browser Speed Arena

An interactive client-side benchmark comparing `WasmExpanseMap32` against Rust `BTreeMap` and native JavaScript `Map` is included in [`examples/speed_arena.html`](examples/speed_arena.html).

To run locally:

```sh
wasm-pack build crates/expanse-wasm --target web --out-dir crates/expanse-wasm/examples/pkg
python3 -m http.server 8080 --directory crates/expanse-wasm/examples
```
Open `http://localhost:8080/speed_arena.html`.
