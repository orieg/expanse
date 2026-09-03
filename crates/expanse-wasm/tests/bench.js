#!/usr/bin/env node
/**
 * WebAssembly wall-clock harness for `@orieg/expanse-wasm` under Node (#629).
 *
 * Two families of rows. The **u32 rows** measure the 32-bit engine through the
 * `WasmExpanseMap32` / `WasmExpanseSet32` classes with plain JS numbers at the
 * boundary, against three baselines that each have a winning regime: the
 * in-wasm `std::BTreeMap` behind `WasmBTreeMap32` (same boundary cost, ordered),
 * the native JS `Map` / `Set` (unordered, no boundary), and a JS sorted array
 * with binary search (ordered, no boundary, O(n) removal). The **legacy rows**
 * (`wasm_map` / `js_map`, u64 `BigInt` keys through `WasmExpanseMap`) are kept
 * byte-for-byte in the JSON so the nightly bindings baseline keeps its history.
 *
 * This is an indicative wall-clock instrument. The deterministic one for the
 * wasm targets is `scripts/wasm_fuel.py` over `crates/expanse-wasm-fuel`
 * (exact fuel, gated in CI); nothing here gates anything.
 *
 * The real wasm module is REQUIRED (`wasm-pack build --release --target nodejs
 * crates/expanse-wasm`); benchmarking a JS fallback would silently emit
 * `runtime: 'wasm'` rows that measure V8, not Expanse (#373).
 *
 * # Workload shape
 *
 * | Property | Value |
 * |---|---|
 * | `workload_id` | `wasm_node_wallclock` |
 * | `group` | `6` |
 * | population | 50,000 keys (`--quick`: 10,000); `random` is XorShift64 seed `0x0DDB_1A5E_5EED_0001` (u32 rows: low 32 bits, duplicates dropped; legacy rows: full u64), `sequential` is 0..N, `clustered` is runs of 8 keys 4,096 apart |
 * | probes_and_reuse | N probes per arm in a seeded Fisher-Yates order, each once; reuse 1.0; `iter` walks every key once; `range` is 100 windows of N/100 entries that together cover the key space once |
 * | hit_rate | `lookup_hit50`: 50% hit / 50% miss; legacy `lookup`: 100% hit (kept for baseline continuity, labelled as such); `insert` and `remove`: every key once |
 * | miss_gen_method | misses drawn from the continuation of the same XorShift64 stream and rejected on membership; never a transform of a present key |
 * | value_dereference | every returned value, key and count is folded into `sink_checksum`, which is emitted in the JSON |
 * | measured_region | the arm's loop only; structures are built before the timer and dropped after it; the legacy `insert` row clears and refills inside the timer as it always has |
 * | arm_symmetry | identical keys, probe order and windows for every structure in a row. The Expanse `iter` cell crosses the JS boundary once per element (`first`/`next`; the package has no batch walk), so it measures the boundary as much as the engine; `WasmBTreeMap32` has no ordered walk in its JS surface so its `iter` cell is n/a and its `range` uses the same in-wasm `batch_range_scan` as the Expanse class; JS `Map`/`Set` have no ordered operations |
 * | statistics | min of `--rounds` (default 3) interleaved rounds per cell, single process; no interval and no gate — publish only with BCa 95% CIs from the quiet host (§8.4) |
 * | verdict | indicative only; the cross-language nightly reads the legacy rows, `docs/benchmarks/wasm/README.md` explains what each family can and cannot claim |
 */

const { performance } = require('perf_hooks');

let pkg;
try {
  pkg = require('../pkg/expanse_wasm.js');
} catch (e) {
  console.error('Error: ../pkg/expanse_wasm.js not found or not loadable — the wasm package is not built.');
  console.error('Build a Node-requirable package first:');
  console.error('  wasm-pack build --release --target nodejs crates/expanse-wasm');
  console.error(`(loader error: ${e.message})`);
  process.exit(1);
}
const REQUIRED = ['WasmExpanseMap', 'WasmExpanseSet', 'WasmExpanseMap32', 'WasmExpanseSet32', 'WasmBTreeMap32', 'WasmBTreeSet32'];
for (const name of REQUIRED) {
  if (typeof pkg[name] !== 'function') {
    console.error(`Error: ../pkg/expanse_wasm.js loaded but does not export ${name}.`);
    process.exit(1);
  }
}
const { WasmExpanseMap, WasmExpanseSet, WasmExpanseMap32, WasmExpanseSet32, WasmBTreeMap32, WasmBTreeSet32 } = pkg;

const SEED = 0x0DDB_1A5E_5EED_0001n;
const PROBE_SEED = 0x9E37_79B9_7F4A_7C15n;
const MASK64 = 0xFFFF_FFFF_FFFF_FFFFn;
const WINDOWS = 100;

function parseArgs() {
  const args = process.argv.slice(2);
  let pop = 50_000;
  let quick = false;
  let json = false;
  let rounds = 3;
  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--quick') {
      quick = true;
      pop = 10_000;
    } else if (args[i] === '--pop' && i + 1 < args.length) {
      pop = parseInt(args[++i], 10);
    } else if (args[i] === '--rounds' && i + 1 < args.length) {
      rounds = parseInt(args[++i], 10);
    } else if (args[i] === '--json') {
      json = true;
    }
  }
  return { pop, quick, json, rounds };
}

class XorShift64 {
  constructor(seed = SEED) {
    this.state = BigInt(seed);
  }
  next() {
    let x = this.state;
    x ^= (x << 13n) & MASK64;
    x ^= (x >> 7n) & MASK64;
    x ^= (x << 17n) & MASK64;
    this.state = x;
    return x;
  }
}

// ---------------------------------------------------------------- keys

// Legacy u64 keys (BigInt), exactly as the harness has always generated them.
function generateKeys64(pop, dist) {
  const rng = new XorShift64();
  const keys = new Array(pop);
  if (dist === 'sequential') {
    for (let i = 0; i < pop; i++) keys[i] = BigInt(i);
  } else {
    for (let i = 0; i < pop; i++) keys[i] = rng.next();
  }
  return keys;
}

// u32 keys as plain numbers; the random stream is the same XorShift64
// truncated to its low 32 bits, duplicates dropped, so it is the stream the
// fuel module uses on wasm32. Returns the generator so misses can continue it.
function generateKeys32(pop, dist) {
  const rng = new XorShift64();
  const keys = [];
  if (dist === 'sequential') {
    for (let i = 0; i < pop; i++) keys.push(i);
  } else if (dist === 'clustered') {
    for (let i = 0; i < pop; i++) keys.push(Math.floor(i / 8) * 4096 + (i % 8));
  } else {
    const seen = new Set();
    while (keys.length < pop) {
      const k = Number(rng.next() & 0xFFFF_FFFFn);
      if (!seen.has(k)) {
        seen.add(k);
        keys.push(k);
      }
    }
  }
  return { keys, rng };
}

function shuffled(keys, seed = PROBE_SEED) {
  const v = keys.slice();
  const rng = new XorShift64(seed);
  for (let i = v.length - 1; i > 0; i--) {
    const j = Number(rng.next() % BigInt(i + 1));
    const t = v[i];
    v[i] = v[j];
    v[j] = t;
  }
  return v;
}

// 50% hits (half the shuffled population) and 50% misses drawn from the
// continuation of the population's stream, rejected on membership.
function hitMissProbes(keys, rng) {
  const n = keys.length;
  const present = new Set(keys);
  const hits = shuffled(keys).slice(0, Math.floor(n / 2));
  const misses = [];
  const chosen = new Set();
  while (misses.length < n - Math.floor(n / 2)) {
    const k = Number(rng.next() & 0xFFFF_FFFFn);
    if (!present.has(k) && !chosen.has(k)) {
      chosen.add(k);
      misses.push(k);
    }
  }
  return shuffled(hits.concat(misses), PROBE_SEED ^ 0xA5A5n);
}

// WINDOWS start keys over the sorted population; each scan reads `limit`
// entries after the start key (exclusive), so the windows tile the key space.
function windows(keys) {
  const sorted = keys.slice().sort((a, b) => a - b);
  const n = sorted.length;
  const limit = Math.floor(n / WINDOWS);
  const starts = [];
  for (let w = 0; w < WINDOWS; w++) starts.push(sorted[Math.floor((w * n) / WINDOWS)]);
  return { starts, limit, sorted };
}

// ---------------------------------------------------------------- JS sorted array baseline

class SortedArray {
  constructor() {
    this.keys = [];
    this.vals = [];
  }
  // index of the first key >= k
  lowerBound(k) {
    let lo = 0;
    let hi = this.keys.length;
    while (lo < hi) {
      const mid = (lo + hi) >>> 1;
      if (this.keys[mid] < k) lo = mid + 1;
      else hi = mid;
    }
    return lo;
  }
  set(k, v) {
    const i = this.lowerBound(k);
    if (i < this.keys.length && this.keys[i] === k) {
      this.vals[i] = v;
    } else {
      this.keys.splice(i, 0, k);
      this.vals.splice(i, 0, v);
    }
  }
  get(k) {
    const i = this.lowerBound(k);
    return i < this.keys.length && this.keys[i] === k ? this.vals[i] : undefined;
  }
  delete(k) {
    const i = this.lowerBound(k);
    if (i < this.keys.length && this.keys[i] === k) {
      this.keys.splice(i, 1);
      this.vals.splice(i, 1);
      return true;
    }
    return false;
  }
  // same contract as batch_range_scan: entries strictly after `start`, up to `limit`
  rangeScan(start, limit) {
    let i = this.lowerBound(start);
    if (i < this.keys.length && this.keys[i] === start) i++;
    let sink = 0;
    const end = Math.min(this.keys.length, i + limit);
    for (; i < end; i++) sink ^= this.keys[i] ^ this.vals[i];
    return sink >>> 0;
  }
}

// ---------------------------------------------------------------- timing

function measure(fn, rounds) {
  let best = Infinity;
  for (let r = 0; r < rounds; r++) {
    const t0 = performance.now();
    fn();
    const dt = performance.now() - t0;
    if (dt < best) best = dt;
  }
  return best;
}

const toMops = (ms, n) => n / (ms / 1000) / 1e6;
const toNs = (ms, n) => (ms * 1e6) / n;

function heapDelta(build) {
  if (typeof global.gc === 'function') global.gc();
  const before = process.memoryUsage().heapUsed;
  const obj = build();
  if (typeof global.gc === 'function') global.gc();
  const after = process.memoryUsage().heapUsed;
  return { obj, bytes: Math.max(0, after - before) };
}

// ---------------------------------------------------------------- u32 rows

// One structure through one adapter: {make, set, get, del, iter?, range?, memBytes?}
function runMapArms(adapter, keys, probes, hm, win, rounds, sinks) {
  const n = keys.length;
  const vals = keys.map((k) => (k ^ 0x55) >>> 0);
  const row = {};

  // insert: fresh structure per round, loop only inside the timer
  let m = null;
  const insertMs = measure(() => {
    m = adapter.make();
    for (let i = 0; i < n; i++) adapter.set(m, keys[i], vals[i]);
  }, rounds);
  row.insert_mops = toMops(insertMs, n);
  row.insert_ns = toNs(insertMs, n);

  // lookup at 50% hit
  const lookupMs = measure(() => {
    let sink = 0;
    for (let i = 0; i < n; i++) {
      const v = adapter.get(m, hm[i]);
      if (v !== undefined && v !== null) sink ^= v;
      else sink = (sink + 1) >>> 0;
    }
    sinks.push(sink);
  }, rounds);
  row.lookup_hit50_mops = toMops(lookupMs, n);
  row.lookup_hit50_ns = toNs(lookupMs, n);

  if (adapter.iter) {
    const iterMs = measure(() => {
      sinks.push(adapter.iter(m));
    }, rounds);
    row.iter_mops = toMops(iterMs, n);
  } else {
    row.iter_mops = null;
  }

  if (adapter.range) {
    const rangeMs = measure(() => {
      let sink = 0;
      for (let w = 0; w < win.starts.length; w++) sink ^= adapter.range(m, win.starts[w], win.limit);
      sinks.push(sink >>> 0);
    }, rounds);
    row.range_mops = toMops(rangeMs, win.starts.length * win.limit);
  } else {
    row.range_mops = null;
  }

  if (adapter.memBytes) {
    row.bytes_per_key = adapter.memBytes(m) / n;
    row.bytes_per_key_method = 'engine mem_used()';
  } else if (adapter.inWasm) {
    // Lives in wasm linear memory, which heapUsed cannot see and whose class
    // exposes no mem_used: reported as n/a, never as 0.
    row.bytes_per_key = null;
    row.bytes_per_key_method = 'n/a: wasm linear memory, no mem_used on this class';
  } else {
    const { bytes } = heapDelta(() => {
      const x = adapter.make();
      for (let i = 0; i < n; i++) adapter.set(x, keys[i], vals[i]);
      return x;
    });
    row.bytes_per_key = bytes / n;
    row.bytes_per_key_method = typeof global.gc === 'function' ? 'heapUsed delta after gc' : 'heapUsed delta without gc (run node --expose-gc)';
  }

  // remove: rebuild before the timer each round, delete every key in the shuffled order inside it
  const removeMs = measureInner(() => {
    const x = adapter.make();
    for (let i = 0; i < n; i++) adapter.set(x, keys[i], vals[i]);
    const t0 = performance.now();
    let removed = 0;
    for (let i = 0; i < n; i++) if (adapter.del(x, probes[i])) removed++;
    sinks.push(removed);
    return performance.now() - t0;
  }, rounds);
  row.remove_mops = toMops(removeMs, n);
  row.remove_ns = toNs(removeMs, n);
  return row;
}

// measure variant whose fn returns its own elapsed time (build excluded)
function measureInner(fn, rounds) {
  let best = Infinity;
  for (let r = 0; r < rounds; r++) {
    const dt = fn();
    if (dt < best) best = dt;
  }
  return best;
}

function runSetArms(adapter, keys, probes, hm, win, rounds, sinks) {
  const n = keys.length;
  const row = {};
  let s = null;
  const insertMs = measure(() => {
    s = adapter.make();
    for (let i = 0; i < n; i++) adapter.add(s, keys[i]);
  }, rounds);
  row.insert_mops = toMops(insertMs, n);
  const lookupMs = measure(() => {
    let sink = 0;
    for (let i = 0; i < n; i++) sink = (sink + (adapter.has(s, hm[i]) ? 1 : 0)) >>> 0;
    sinks.push(sink);
  }, rounds);
  row.lookup_hit50_mops = toMops(lookupMs, n);
  row.lookup_hit50_ns = toNs(lookupMs, n);
  if (adapter.iter) {
    row.iter_mops = toMops(measure(() => sinks.push(adapter.iter(s)), rounds), n);
  } else {
    row.iter_mops = null;
  }
  if (adapter.range) {
    const rangeMs = measure(() => {
      let sink = 0;
      for (let w = 0; w < win.starts.length; w++) sink ^= adapter.range(s, win.starts[w], win.limit);
      sinks.push(sink >>> 0);
    }, rounds);
    row.range_mops = toMops(rangeMs, win.starts.length * win.limit);
  } else {
    row.range_mops = null;
  }
  if (adapter.memBytes) {
    row.bytes_per_key = adapter.memBytes(s) / n;
    row.bytes_per_key_method = 'engine mem_used()';
  } else if (adapter.inWasm) {
    row.bytes_per_key = null;
    row.bytes_per_key_method = 'n/a: wasm linear memory, no mem_used on this class';
  } else {
    const { bytes } = heapDelta(() => {
      const x = adapter.make();
      for (let i = 0; i < n; i++) adapter.add(x, keys[i]);
      return x;
    });
    row.bytes_per_key = bytes / n;
    row.bytes_per_key_method = typeof global.gc === 'function' ? 'heapUsed delta after gc' : 'heapUsed delta without gc (run node --expose-gc)';
  }
  const removeMs = measureInner(() => {
    const x = adapter.make();
    for (let i = 0; i < n; i++) adapter.add(x, keys[i]);
    const t0 = performance.now();
    let removed = 0;
    for (let i = 0; i < n; i++) if (adapter.del(x, probes[i])) removed++;
    sinks.push(removed);
    return performance.now() - t0;
  }, rounds);
  row.remove_mops = toMops(removeMs, n);
  return row;
}

const mapAdapters = {
  wasm_expanse_map32: {
    make: () => new WasmExpanseMap32(),
    set: (m, k, v) => m.set(k, v),
    get: (m, k) => m.get(k),
    del: (m, k) => m.delete(k),
    iter: (m) => {
      let sink = 0;
      let e = m.first();
      while (e) {
        sink ^= e[0] ^ e[1];
        e = m.next(e[0]);
      }
      return sink >>> 0;
    },
    range: (m, start, limit) => m.batch_range_scan(start, limit),
    memBytes: (m) => m.mem_used(),
  },
  wasm_btreemap32: {
    make: () => new WasmBTreeMap32(),
    set: (m, k, v) => m.set(k, v),
    get: (m, k) => m.get(k),
    del: (m, k) => m.delete(k),
    iter: null, // no first/next in its JS surface
    range: (m, start, limit) => m.batch_range_scan(start, limit),
    memBytes: null, // no mem_used in its JS surface
    inWasm: true,
  },
  js_map_u32: {
    make: () => new Map(),
    set: (m, k, v) => m.set(k, v),
    get: (m, k) => m.get(k),
    del: (m, k) => m.delete(k),
    iter: (m) => {
      let sink = 0;
      for (const [k, v] of m) sink ^= k ^ v;
      return sink >>> 0;
    },
    range: null,
    memBytes: null,
  },
  js_sorted_array: {
    make: () => new SortedArray(),
    set: (m, k, v) => m.set(k, v),
    get: (m, k) => m.get(k),
    del: (m, k) => m.delete(k),
    iter: (m) => {
      let sink = 0;
      for (let i = 0; i < m.keys.length; i++) sink ^= m.keys[i] ^ m.vals[i];
      return sink >>> 0;
    },
    range: (m, start, limit) => m.rangeScan(start, limit),
    memBytes: null,
  },
};

const setAdapters = {
  wasm_expanse_set32: {
    make: () => new WasmExpanseSet32(),
    add: (s, k) => s.add(k),
    has: (s, k) => s.contains(k),
    del: (s, k) => s.remove(k),
    iter: (s) => {
      let sink = 0;
      let k = s.first();
      while (k !== undefined && k !== null) {
        sink ^= k;
        k = s.next(k);
      }
      return sink >>> 0;
    },
    range: (s, start, limit) => s.batch_range_scan(start, limit),
    memBytes: (s) => s.mem_used(),
  },
  wasm_btreeset32: {
    make: () => new WasmBTreeSet32(),
    add: (s, k) => s.add(k),
    has: (s, k) => s.contains(k),
    del: (s, k) => s.remove(k),
    iter: null,
    range: (s, start, limit) => s.batch_range_scan(start, limit),
    memBytes: null,
    inWasm: true,
  },
  js_set_u32: {
    make: () => new Set(),
    add: (s, k) => s.add(k),
    has: (s, k) => s.has(k),
    del: (s, k) => s.delete(k),
    iter: (s) => {
      let sink = 0;
      for (const k of s) sink ^= k;
      return sink >>> 0;
    },
    range: null,
    memBytes: null,
  },
};

function runU32Suite(pop, dist, rounds) {
  const { keys, rng } = generateKeys32(pop, dist);
  const probes = shuffled(keys);
  const hm = hitMissProbes(keys, rng);
  const win = windows(keys);
  const sinks = [];
  const maps = {};
  for (const [name, ad] of Object.entries(mapAdapters)) maps[name] = runMapArms(ad, keys, probes, hm, win, rounds, sinks);
  const sets = {};
  for (const [name, ad] of Object.entries(setAdapters)) sets[name] = runSetArms(ad, keys, probes, hm, win, rounds, sinks);
  let checksum = 0;
  for (const s of sinks) checksum = (checksum ^ (s >>> 0)) >>> 0;
  return { dist, pop, sink_checksum: `0x${checksum.toString(16)}`, maps, sets };
}

// ---------------------------------------------------------------- legacy rows (unchanged)

function runLegacySuite(pop, dist, rounds) {
  const keys = generateKeys64(pop, dist);
  const probeKeys = keys.slice().reverse();
  let sinkGuard = 0n;

  const wasmMap = new WasmExpanseMap();
  const wasmInsertMs = measure(() => {
    wasmMap.clear();
    for (let i = 0; i < pop; i++) wasmMap.set(keys[i], keys[i] ^ 0x55n);
  }, rounds);
  const wasmLookupMs = measure(() => {
    let sink = 0n;
    for (let i = 0; i < pop; i++) {
      const v = wasmMap.get(probeKeys[i]);
      if (v !== undefined && v !== null) sink ^= BigInt(v);
    }
    sinkGuard ^= sink;
  }, rounds);

  const jsMap = new Map();
  const jsInsertMs = measure(() => {
    jsMap.clear();
    for (let i = 0; i < pop; i++) jsMap.set(keys[i], keys[i] ^ 0x55n);
  }, rounds);
  const jsLookupMs = measure(() => {
    let sink = 0n;
    for (let i = 0; i < pop; i++) {
      const v = jsMap.get(probeKeys[i]);
      if (v !== undefined) sink ^= v;
    }
    sinkGuard ^= sink;
  }, rounds);

  return {
    dist,
    pop,
    sink_checksum: `0x${sinkGuard.toString(16)}`,
    wasm_map: {
      insert_mops: toMops(wasmInsertMs, pop),
      lookup_mops: toMops(wasmLookupMs, pop),
      lookup_ns: toNs(wasmLookupMs, pop),
    },
    js_map: {
      insert_mops: toMops(jsInsertMs, pop),
      lookup_mops: toMops(jsLookupMs, pop),
      lookup_ns: toNs(jsLookupMs, pop),
    },
  };
}

// ---------------------------------------------------------------- main

function fmt(x, d = 2) {
  return x === null || x === undefined ? 'n/a' : x.toFixed(d);
}

function main() {
  const { pop, json, rounds } = parseArgs();
  const results = [];
  for (const d of ['random', 'sequential']) results.push(runLegacySuite(pop, d, rounds));
  const u32 = [];
  for (const d of ['random', 'sequential', 'clustered']) u32.push(runU32Suite(pop, d, rounds));

  if (json) {
    console.log(JSON.stringify({ runtime: 'wasm', harness_version: 2, rounds, results, u32 }, null, 2));
    return;
  }
  console.log('\n=== Expanse WebAssembly (Node) wall-clock harness — indicative, min of ' + rounds + ' rounds ===\n');
  for (const r of results) {
    console.log(`[ legacy u64 BigInt rows | ${r.dist} | N=${r.pop.toLocaleString()} ]`);
    console.log(`  WasmExpanseMap: ${fmt(r.wasm_map.lookup_ns)} ns/lookup (100% hit) | insert ${fmt(r.wasm_map.insert_mops)} Mops`);
    console.log(`  JS Map:         ${fmt(r.js_map.lookup_ns)} ns/lookup (100% hit) | insert ${fmt(r.js_map.insert_mops)} Mops`);
  }
  for (const r of u32) {
    console.log(`\n[ u32 rows | ${r.dist} | N=${r.pop.toLocaleString()} ]  insert Mops | lookup50 ns | iter Mops | range Mops | remove Mops | B/key`);
    for (const [name, row] of Object.entries(r.maps)) {
      console.log(`  ${name.padEnd(20)} ${fmt(row.insert_mops).padStart(8)} | ${fmt(row.lookup_hit50_ns).padStart(9)} | ${fmt(row.iter_mops).padStart(8)} | ${fmt(row.range_mops).padStart(8)} | ${fmt(row.remove_mops).padStart(8)} | ${fmt(row.bytes_per_key, 1)}`);
    }
    for (const [name, row] of Object.entries(r.sets)) {
      console.log(`  ${name.padEnd(20)} ${fmt(row.insert_mops).padStart(8)} | ${fmt(row.lookup_hit50_ns).padStart(9)} | ${fmt(row.iter_mops).padStart(8)} | ${fmt(row.range_mops).padStart(8)} | ${fmt(row.remove_mops).padStart(8)} | ${fmt(row.bytes_per_key, 1)}`);
    }
  }
  console.log('');
}

main();
