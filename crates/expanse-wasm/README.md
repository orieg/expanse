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

**Backed by the digital trie:**

- `WasmExpanseSet32`: 32-bit digital trie set with batch execution methods
- `WasmExpanseMap32`: 32-bit digital trie map with batch execution methods
- `WasmExpanseSet`, `WasmExpanseMap`: the 64-bit engine on `wasm64`; a
  `BTreeMap` / `BTreeSet` fallback on `wasm32`, where the 64-bit engine does not
  exist

**Not backed by the engine — `BTreeMap` placeholders**
([#757](https://github.com/orieg/expanse/issues/757)):

- `WasmExpanseStrMap`: `String` to `u64` map
- `WasmExpanseBytesMap`: `&[u8]` to `u64` map
- `WasmExpanseBlobMap`: `u64` to `&[u8]` map with hot metadata

These three carry `Expanse*` names but are `std::collections::BTreeMap`. They
have none of the trie's memory profile, `mem_used` accounting or node structure,
and **a figure measured through them says nothing about the engine**. Nothing
published depends on them: every WASM row in
[`docs/BINDINGS_BENCHMARKS.md`](../../docs/BINDINGS_BENCHMARKS.md) and every arm
in `scripts/wasm_fuel.py` is scoped to `WasmExpanseMap32` / `WasmExpanseSet32`.

They are placeholders rather than wrappers because the engine types they are
named for — `ExpanseStrMap`, `ExpanseBytesMap`, `ExpanseBlobMap` — are
`#[cfg(target_pointer_width = "64")]` and have no 32-bit twin, so they do not
exist on `wasm32`. `ExpanseBlobMap32` does exist, but takes no chunk size and
uses 32-bit keys, so adopting it would narrow this class's published API without
gaining the arena the name implies.

`WasmExpanseBlobMap`'s constructor **rejects** a `chunk_size` argument rather
than accepting and discarding it: there is no arena to size.

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
