#!/usr/bin/env node
/**
 * Cross-Runtime Comparative Benchmark Suite for @orieg/expanse-wasm (WebAssembly / Edge).
 * Compares WasmExpanseMap and WasmExpanseSet against native JavaScript Map and Set.
 */

const { performance } = require('perf_hooks');
let WasmExpanseMap, WasmExpanseSet;

// The real wasm module is REQUIRED. Benchmarking a JS fallback here would
// silently emit `runtime: 'wasm'` rows that measure V8, not Expanse (#373).
try {
  const pkg = require('../pkg/expanse_wasm.js');
  WasmExpanseMap = pkg.WasmExpanseMap;
  WasmExpanseSet = pkg.WasmExpanseSet;
} catch (e) {
  console.error('Error: ../pkg/expanse_wasm.js not found or not loadable — the wasm package is not built.');
  console.error('Build a Node-requirable package first:');
  console.error('  wasm-pack build --release --target nodejs crates/expanse-wasm');
  console.error(`(loader error: ${e.message})`);
  process.exit(1);
}
if (typeof WasmExpanseMap !== 'function' || typeof WasmExpanseSet !== 'function') {
  console.error('Error: ../pkg/expanse_wasm.js loaded but does not export WasmExpanseMap/WasmExpanseSet.');
  process.exit(1);
}

function parseArgs() {
  const args = process.argv.slice(2);
  let pop = 50_000;
  let quick = false;
  let json = false;

  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--quick') {
      quick = true;
      pop = 10_000;
    } else if (args[i] === '--pop' && i + 1 < args.length) {
      pop = parseInt(args[++i], 10);
    } else if (args[i] === '--json') {
      json = true;
    }
  }
  return { pop, quick, json };
}

class XorShift64 {
  constructor(seed = 0x0DDB_1A5E_5EED_0001n) {
    this.state = BigInt(seed);
  }
  next() {
    let x = this.state;
    x ^= (x << 13n) & 0xFFFF_FFFF_FFFF_FFFFn;
    x ^= (x >> 7n) & 0xFFFF_FFFF_FFFF_FFFFn;
    x ^= (x << 17n) & 0xFFFF_FFFF_FFFF_FFFFn;
    this.state = x;
    return x;
  }
}

function generateKeys(pop, dist = 'random') {
  const rng = new XorShift64();
  const keys = new Array(pop);
  if (dist === 'sequential') {
    for (let i = 0; i < pop; i++) keys[i] = BigInt(i);
  } else {
    for (let i = 0; i < pop; i++) keys[i] = rng.next();
  }
  return keys;
}

function measure(fn, rounds = 3) {
  let best = Infinity;
  for (let r = 0; r < rounds; r++) {
    const t0 = performance.now();
    fn();
    const t1 = performance.now();
    const dt = t1 - t0;
    if (dt < best) best = dt;
  }
  return best;
}

function runSuite(pop, dist = 'random') {
  const keys = generateKeys(pop, dist);
  const probeKeys = keys.slice().reverse();

  // Lookup sinks escape into this accumulator, which is emitted in the result
  // (sink_checksum), so the JIT cannot dead-code-eliminate the probed lookups.
  let sinkGuard = 0n;

  // 1. WASM ExpanseMap
  const wasmMap = new WasmExpanseMap();
  const wasmInsertMs = measure(() => {
    wasmMap.clear();
    for (let i = 0; i < pop; i++) wasmMap.set(keys[i], keys[i] ^ 0x55n);
  });
  const wasmLookupMs = measure(() => {
    let sink = 0n;
    for (let i = 0; i < pop; i++) {
      const v = wasmMap.get(probeKeys[i]);
      if (v !== undefined && v !== null) sink ^= BigInt(v);
    }
    sinkGuard ^= sink;
    return sink;
  });

  // 2. JS Map
  const jsMap = new Map();
  const jsInsertMs = measure(() => {
    jsMap.clear();
    for (let i = 0; i < pop; i++) jsMap.set(keys[i], keys[i] ^ 0x55n);
  });
  const jsLookupMs = measure(() => {
    let sink = 0n;
    for (let i = 0; i < pop; i++) {
      const v = jsMap.get(probeKeys[i]);
      if (v !== undefined) sink ^= v;
    }
    sinkGuard ^= sink;
    return sink;
  });

  const toMops = (ms) => (pop / (ms / 1000) / 1e6);
  const toNs = (ms) => (ms * 1e6 / pop);

  return {
    dist,
    pop,
    sink_checksum: `0x${sinkGuard.toString(16)}`,
    wasm_map: {
      insert_mops: toMops(wasmInsertMs),
      lookup_mops: toMops(wasmLookupMs),
      lookup_ns: toNs(wasmLookupMs),
    },
    js_map: {
      insert_mops: toMops(jsInsertMs),
      lookup_mops: toMops(jsLookupMs),
      lookup_ns: toNs(jsLookupMs),
    },
  };
}

function main() {
  const { pop, json } = parseArgs();
  const dists = ['random', 'sequential'];
  const results = [];

  for (const d of dists) {
    results.push(runSuite(pop, d));
  }

  if (json) {
    console.log(JSON.stringify({ runtime: 'wasm', results }, null, 2));
  } else {
    console.log(`\n=== Expanse WebAssembly (Edge) Comparative Report ===\n`);
    for (const r of results) {
      console.log(`[ Distribution: ${r.dist} | Pop: ${r.pop.toLocaleString()} ]`);
      console.log(`  WasmExpanseMap: ${r.wasm_map.lookup_ns.toFixed(2)} ns/lookup (${r.wasm_map.lookup_mops.toFixed(2)} Mops) | Insert: ${r.wasm_map.insert_mops.toFixed(2)} Mops`);
      console.log(`  JS native Map:  ${r.js_map.lookup_ns.toFixed(2)} ns/lookup (${r.js_map.lookup_mops.toFixed(2)} Mops) | Insert: ${r.js_map.insert_mops.toFixed(2)} Mops`);
    }
    console.log('');
  }
}

main();
