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

## Supported Data Structures

- `WasmExpanseSet`: `u64` values
- `WasmExpanseMap`: `u64` to `u64` map
- `WasmExpanseStrMap`: `String` to `u64` map
- `WasmExpanseBytesMap`: `&[u8]` to `u64` map
- `WasmExpanseBlobMap`: `u64` to `&[u8]` map with hot metadata
