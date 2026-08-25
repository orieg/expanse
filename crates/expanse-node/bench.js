#!/usr/bin/env node
/**
 * Cross-Runtime Comparative Benchmark Suite for @orieg/expanse Node.js Bindings.
 * Compares ExpanseMap / ExpanseSet against native V8 Map and Set.
 */

const { ExpanseMap, ExpanseSet } = require('./index.js');
const { performance } = require('perf_hooks');

function parseArgs() {
  const args = process.argv.slice(2);
  let pop = 100_000;
  let quick = false;
  let json = false;

  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--quick') {
      quick = true;
      pop = 20_000;
    } else if (args[i] === '--pop' && i + 1 < args.length) {
      pop = parseInt(args[++i], 10);
    } else if (args[i] === '--json') {
      json = true;
    }
  }
  return { pop, quick, json };
}

// XorShift64 PRNG for deterministic key generation
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
  } else if (dist === 'clustered') {
    let base = 0n;
    for (let i = 0; i < pop; i++) {
      if (i % 256 === 0) base = rng.next() & ~0xFFn;
      keys[i] = base + BigInt(i % 256);
    }
  } else {
    for (let i = 0; i < pop; i++) keys[i] = rng.next();
  }
  return keys;
}

function shuffle(arr) {
  const out = arr.slice();
  const rng = new XorShift64(0x9E37_79B9n);
  for (let i = out.length - 1; i > 0; i--) {
    const j = Number(rng.next() % BigInt(i + 1));
    const tmp = out[i];
    out[i] = out[j];
    out[j] = tmp;
  }
  return out;
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
  const probeKeys = shuffle(keys);

  // 1. ExpanseMap
  if (global.gc) global.gc();
  const memBeforeExp = process.memoryUsage().heapUsed;
  const expMap = new ExpanseMap();

  const expInsertMs = measure(() => {
    expMap.clear();
    for (let i = 0; i < pop; i++) {
      expMap.set(keys[i], keys[i] ^ 0x55n);
    }
  });

  const expLookupMs = measure(() => {
    let sink = 0n;
    for (let i = 0; i < pop; i++) {
      const v = expMap.get(probeKeys[i]);
      if (v !== null && v !== undefined) sink ^= v;
    }
    return sink;
  });

  const expIterMs = measure(() => {
    const entries = expMap.entries();
    return entries.length;
  });

  const expRangeMs = measure(() => {
    let count = 0n;
    const step = Math.floor(pop / 100);
    for (let i = 0; i < pop; i += step) {
      const start = keys[i];
      const end = start + 100n;
      count += expMap.countRange(start, end);
    }
    return count;
  });

  const memAfterExp = process.memoryUsage().heapUsed;
  const expBytesPerKey = Number(expMap.memUsed()) / pop;

  // 2. JavaScript Native Map
  if (global.gc) global.gc();
  const memBeforeJs = process.memoryUsage().heapUsed;
  const jsMap = new Map();

  const jsInsertMs = measure(() => {
    jsMap.clear();
    for (let i = 0; i < pop; i++) {
      jsMap.set(keys[i], keys[i] ^ 0x55n);
    }
  });

  const jsLookupMs = measure(() => {
    let sink = 0n;
    for (let i = 0; i < pop; i++) {
      const v = jsMap.get(probeKeys[i]);
      if (v !== undefined) sink ^= v;
    }
    return sink;
  });

  const jsIterMs = measure(() => {
    let count = 0;
    for (const [k, v] of jsMap) {
      count++;
    }
    return count;
  });

  const memAfterJs = process.memoryUsage().heapUsed;
  const jsBytesPerKey = Math.max(0, (memAfterJs - memBeforeJs) / pop);

  // 3. ExpanseSet
  const expSet = new ExpanseSet();
  const expSetInsertMs = measure(() => {
    expSet.clear();
    for (let i = 0; i < pop; i++) expSet.add(keys[i]);
  });
  const expSetLookupMs = measure(() => {
    let count = 0;
    for (let i = 0; i < pop; i++) if (expSet.has(probeKeys[i])) count++;
    return count;
  });

  // 4. Native Set
  const jsSet = new Set();
  const jsSetInsertMs = measure(() => {
    jsSet.clear();
    for (let i = 0; i < pop; i++) jsSet.add(keys[i]);
  });
  const jsSetLookupMs = measure(() => {
    let count = 0;
    for (let i = 0; i < pop; i++) if (jsSet.has(probeKeys[i])) count++;
    return count;
  });

  const toMops = (ms) => (pop / (ms / 1000) / 1e6);
  const toNs = (ms) => (ms * 1e6 / pop);

  return {
    dist,
    pop,
    expanse_map: {
      insert_mops: toMops(expInsertMs),
      lookup_mops: toMops(expLookupMs),
      lookup_ns: toNs(expLookupMs),
      iter_mops: toMops(expIterMs),
      range_mops: toMops(expRangeMs),
      bytes_per_key: expBytesPerKey,
    },
    js_map: {
      insert_mops: toMops(jsInsertMs),
      lookup_mops: toMops(jsLookupMs),
      lookup_ns: toNs(jsLookupMs),
      iter_mops: toMops(jsIterMs),
      bytes_per_key: jsBytesPerKey,
    },
    expanse_set: {
      insert_mops: toMops(expSetInsertMs),
      lookup_mops: toMops(expSetLookupMs),
      lookup_ns: toNs(expSetLookupMs),
    },
    js_set: {
      insert_mops: toMops(jsSetInsertMs),
      lookup_mops: toMops(jsSetLookupMs),
      lookup_ns: toNs(jsSetLookupMs),
    },
  };
}

function renderTable(results) {
  console.log(`\n================================================================================`);
  console.log(`  Expanse Node.js Bindings Comparative Performance Report`);
  console.log(`================================================================================`);

  for (const r of results) {
    console.log(`\n[ Distribution: ${r.dist} | Population: ${r.pop.toLocaleString()} ]`);
    console.log(`${'Target'.padEnd(20)} | ${'Lookup (ns)'.padStart(11)} | ${'Lookup (Mops)'.padStart(13)} | ${'Insert (Mops)'.padStart(13)} | ${'Iter (Mops)'.padStart(11)} | ${'B/key'.padStart(8)}`);
    console.log(`${'-'.repeat(20)}-+-${'-'.repeat(11)}-+-${'-'.repeat(13)}-+-${'-'.repeat(13)}-+-${'-'.repeat(11)}-+-${'-'.repeat(8)}`);

    const em = r.expanse_map;
    console.log(`${'ExpanseMap'.padEnd(20)} | ${em.lookup_ns.toFixed(2).padStart(11)} | ${em.lookup_mops.toFixed(2).padStart(13)} | ${em.insert_mops.toFixed(2).padStart(13)} | ${em.iter_mops.toFixed(2).padStart(11)} | ${em.bytes_per_key.toFixed(2).padStart(8)}`);

    const jm = r.js_map;
    console.log(`${'JS native Map'.padEnd(20)} | ${jm.lookup_ns.toFixed(2).padStart(11)} | ${jm.lookup_mops.toFixed(2).padStart(13)} | ${jm.insert_mops.toFixed(2).padStart(13)} | ${jm.iter_mops.toFixed(2).padStart(11)} | ${(jm.bytes_per_key > 0 ? jm.bytes_per_key.toFixed(2) : '~64.00').padStart(8)}`);

    const es = r.expanse_set;
    console.log(`${'ExpanseSet'.padEnd(20)} | ${es.lookup_ns.toFixed(2).padStart(11)} | ${es.lookup_mops.toFixed(2).padStart(13)} | ${es.insert_mops.toFixed(2).padStart(13)} | ${'—'.padStart(11)} | ${'—'.padStart(8)}`);

    const js = r.js_set;
    console.log(`${'JS native Set'.padEnd(20)} | ${js.lookup_ns.toFixed(2).padStart(11)} | ${js.lookup_mops.toFixed(2).padStart(13)} | ${js.insert_mops.toFixed(2).padStart(13)} | ${'—'.padStart(11)} | ${'—'.padStart(8)}`);
  }
  console.log(`\n================================================================================\n`);
}

function main() {
  const { pop, json } = parseArgs();
  const dists = ['random', 'sequential', 'clustered'];
  const results = [];

  for (const d of dists) {
    results.push(runSuite(pop, d));
  }

  if (json) {
    console.log(JSON.stringify({ runtime: 'node', results }, null, 2));
  } else {
    renderTable(results);
  }
}

main();
