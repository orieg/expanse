#!/usr/bin/env node
/**
 * WebAssembly Memory64 Runtime Smoke Test for Expanse (wasm64-unknown-unknown).
 *
 * Verifies that:
 * 1. The wasm64 binary compiled with 64-bit linear memory addresses instantiates cleanly in Node.js.
 * 2. 64-bit raw pointers, 16-byte Edge descriptors, and 8-level digital trie descent (ExpanseMap, ExpanseSet)
 *    execute without memory faults or address truncation in 64-bit WebAssembly address space.
 * 3. Exact deterministic checksums match between native 64-bit engine and wasm64 runtime.
 */

const fs = require('fs');
const path = require('path');

function findWasmBinary() {
  const candidates = [
    path.resolve(__dirname, '../../../target/wasm64-unknown-unknown/debug/expanse_wasm.wasm'),
    path.resolve(__dirname, '../../../target/wasm64-unknown-unknown/release/expanse_wasm.wasm'),
    path.resolve(process.cwd(), 'target/wasm64-unknown-unknown/debug/expanse_wasm.wasm'),
    path.resolve(process.cwd(), 'target/wasm64-unknown-unknown/release/expanse_wasm.wasm'),
  ];

  for (const p of candidates) {
    if (fs.existsSync(p)) {
      return p;
    }
  }
  return null;
}

async function main() {
  console.log('=== Expanse WebAssembly Memory64 (wasm64) Smoke Test ===\n');

  const wasmPath = findWasmBinary();
  if (!wasmPath) {
    console.error('Error: wasm64 binary not found.');
    console.error('Build it first using:');
    console.error('  cargo +nightly build -p expanse-wasm --target wasm64-unknown-unknown -Z build-std=std,panic_abort');
    process.exit(1);
  }

  console.log(`Loading wasm64 binary: ${wasmPath}`);
  const wasmBytes = fs.readFileSync(wasmPath);
  console.log(`Binary size: ${(wasmBytes.length / (1024 * 1024)).toFixed(2)} MB`);

  // Host imports required by wasm-bindgen cdylib
  const importObject = {
    '__wbindgen_placeholder__': {
      __wbindgen_describe: () => {},
      __wbg_push_adb0107829f02d75: () => {},
      __wbg_new_116be93542d39019: () => {},
      __wbg_new_from_slice_b61d590a0b3abdb3: () => {},
      __wbg_new_from_slice_3eea173078478cfe: () => {},
      __wbindgen_describe_cast: () => {},
      __wbg___wbindgen_throw_bb96b2010945f0bc: (msg, len) => {
        throw new Error(`wasm error (len: ${len})`);
      },
      __wbindgen_object_drop_ref: () => {}
    },
    '__wbindgen_externref_xform__': {
      __wbindgen_externref_table_grow: () => 0,
      __wbindgen_externref_table_set_null: () => {}
    }
  };

  try {
    const result = await WebAssembly.instantiate(wasmBytes, importObject);
    const exports = result.instance.exports;

    if (typeof exports.expanse_wasm64_smoke_test !== 'function') {
      console.error('Error: export `expanse_wasm64_smoke_test` not found in wasm64 exports.');
      process.exit(1);
    }

    console.log('Executing expanse_wasm64_smoke_test() inside WebAssembly Memory64...');
    const checksum = exports.expanse_wasm64_smoke_test();
    console.log(`Result: checksum = ${checksum} (type: ${typeof checksum})`);

    // Expected deterministic checksum for 64-bit trie operations in wasm64
    const expectedChecksum = 2204620018906824862n;

    if (checksum !== expectedChecksum) {
      console.error(`FAIL: checksum mismatch (expected ${expectedChecksum}, got ${checksum})`);
      process.exit(1);
    }

    console.log('\n✅ Memory64 (wasm64-unknown-unknown) Smoke Test PASSED.');
    console.log('   - 64-bit address space verified');
    console.log('   - 16-byte Edge descriptors verified');
    console.log('   - 64-bit digital trie hierarchy (ExpanseMap / ExpanseSet) verified');
    console.log('   - Deterministic bit-level checksum matched\n');
  } catch (err) {
    console.error('Execution failed:', err);
    process.exit(1);
  }
}

main().catch(err => {
  console.error(err);
  process.exit(1);
});
