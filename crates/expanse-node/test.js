/**
 * Comprehensive Node.js / Bun test suite for @orieg/expanse.
 */

const assert = require('assert');
const path = require('path');
const fs = require('fs');
const os = require('os');
const {
  ExpanseSet,
  ExpanseMap,
  ExpanseStrMap,
  ExpanseBytesMap,
  ExpanseOrderedBytesMap,
  ExpanseBlobMap,
  SyncExpanseMap,
  SyncExpanseSet,
  SyncExpanseStrMap,
  SyncExpanseBytesMap,
  SyncExpanseOrderedBytesMap,
  SyncExpanseBlobMap,
} = require('./index.js');

function test(name, fn) {
  try {
    fn();
    console.log(`  ✓ ${name}`);
  } catch (err) {
    console.error(`  ✗ ${name}`);
    console.error(err);
    process.exit(1);
  }
}

console.log('Running @orieg/expanse Node.js binding test suite...\n');

// 1. ExpanseSet Tests
console.log('--- ExpanseSet Tests ---');

test('ExpanseSet basic operations (add, has, remove, size, clear)', () => {
  const set = new ExpanseSet();
  assert.strictEqual(set.size(), 0n);
  assert.strictEqual(set.isEmpty(), true);

  assert.strictEqual(set.add(42), true);
  assert.strictEqual(set.add(42n), false); // duplicate
  assert.strictEqual(set.add(100n), true);
  assert.strictEqual(set.add(1000n), true);

  assert.strictEqual(set.size(), 3n);
  assert.strictEqual(set.isEmpty(), false);
  assert.strictEqual(set.has(42), true);
  assert.strictEqual(set.has(100n), true);
  assert.strictEqual(set.has(999n), false);

  assert.strictEqual(set.remove(42), true);
  assert.strictEqual(set.remove(42), false);
  assert.strictEqual(set.size(), 2n);

  set.clear();
  assert.strictEqual(set.size(), 0n);
  assert.strictEqual(set.isEmpty(), true);
});

test('ExpanseSet constructor with initial keys', () => {
  const set = new ExpanseSet([10n, 20n, 30n, 20n]);
  assert.strictEqual(set.size(), 3n);
  assert.strictEqual(set.has(10n), true);
  assert.strictEqual(set.has(20), true);
  assert.strictEqual(set.has(30n), true);
});

test('ExpanseSet navigation (first, last, next, prev)', () => {
  const set = new ExpanseSet([100n, 200n, 300n, 500n]);
  assert.strictEqual(set.first(), 100n);
  assert.strictEqual(set.last(), 500n);

  // next
  assert.strictEqual(set.next(100n), 200n);
  assert.strictEqual(set.next(100n, true), 100n); // inclusive
  assert.strictEqual(set.next(250n), 300n);
  assert.strictEqual(set.next(500n), null);

  // prev
  assert.strictEqual(set.prev(500n), 300n);
  assert.strictEqual(set.prev(500n, true), 500n); // inclusive
  assert.strictEqual(set.prev(250n), 200n);
  assert.strictEqual(set.prev(100n), null);
});

test('ExpanseSet rank, select, countRange, range, toArray', () => {
  const keys = [10n, 20n, 30n, 40n, 50n];
  const set = new ExpanseSet(keys);

  assert.strictEqual(set.rank(30n), 2n); // strictly below 30: 10, 20 -> 2
  assert.strictEqual(set.rank(10n), 0n);
  assert.strictEqual(set.rank(100n), 5n);

  assert.strictEqual(set.select(0n), 10n);
  assert.strictEqual(set.select(2n), 30n);
  assert.strictEqual(set.select(4n), 50n);
  assert.strictEqual(set.select(5n), null);

  assert.strictEqual(set.countRange(20n, 40n), 3n);
  assert.strictEqual(set.countRange(25n, 28n), 0n);

  const arr = set.toArray();
  assert.deepStrictEqual(arr, [10n, 20n, 30n, 40n, 50n]);

  const ranged = set.range(20n, 40n);
  assert.deepStrictEqual(ranged, [20n, 30n, 40n]);

  const inserted = set.insertMany([50n, 60n, 70n]);
  assert.strictEqual(inserted, 2);
  assert.strictEqual(set.size(), 7n);
});

test('ExpanseSet 64-Bit Boundary Value Test Vectors', () => {
  const set = new ExpanseSet();
  const keys = [0n, 1n, (1n << 53n) - 1n, 1n << 53n, 9223372036854775807n, 18446744073709551615n];
  for (const k of keys) {
    set.add(k);
  }
  for (const k of keys) {
    assert.strictEqual(set.has(k), true);
  }
  assert.strictEqual(set.size(), BigInt(keys.length));
});

// 2. ExpanseMap Tests
console.log('\n--- ExpanseMap Tests ---');

test('ExpanseMap basic operations (set, get, has, delete, size, clear)', () => {
  const map = new ExpanseMap();
  assert.strictEqual(map.size(), 0n);

  assert.strictEqual(map.set(1n, 100n), null);
  assert.strictEqual(map.set(1n, 200n), 100n); // returns old value
  assert.strictEqual(map.set(2, 300), null);

  assert.strictEqual(map.size(), 2n);
  assert.strictEqual(map.get(1n), 200n);
  assert.strictEqual(map.get(2), 300n);
  assert.strictEqual(map.get(999), null);

  assert.strictEqual(map.has(1n), true);
  assert.strictEqual(map.has(999), false);

  assert.strictEqual(map.delete(1n), true);
  assert.strictEqual(map.delete(1n), false);
  assert.strictEqual(map.size(), 1n);

  map.clear();
  assert.strictEqual(map.size(), 0n);
});

test('ExpanseMap navigation and queries (first, next, rank, select, countRange, entries)', () => {
  const map = new ExpanseMap();
  map.set(10n, 1000n);
  map.set(20n, 2000n);
  map.set(30n, 3000n);
  map.set(40n, 4000n);

  assert.deepStrictEqual(map.first(), { key: 10n, value: 1000n });
  assert.deepStrictEqual(map.last(), { key: 40n, value: 4000n });

  assert.deepStrictEqual(map.next(20n), { key: 30n, value: 3000n });
  assert.deepStrictEqual(map.next(20n, true), { key: 20n, value: 2000n });
  assert.deepStrictEqual(map.prev(30n), { key: 20n, value: 2000n });
  assert.deepStrictEqual(map.prev(30n, true), { key: 30n, value: 3000n });

  assert.strictEqual(map.rank(30n), 2n);
  assert.deepStrictEqual(map.select(1n), { key: 20n, value: 2000n });
  assert.strictEqual(map.countRange(15n, 35n), 2n);

  assert.deepStrictEqual(map.keys(), [10n, 20n, 30n, 40n]);
  assert.deepStrictEqual(map.values(), [1000n, 2000n, 3000n, 4000n]);

  const entries = map.entries();
  assert.strictEqual(entries.length, 4);
  assert.deepStrictEqual(entries[0], { key: 10n, value: 1000n });
});

test('ExpanseMap 64-Bit Boundary Value Test Vectors', () => {
  const map = new ExpanseMap();
  const keys = [0n, 1n, (1n << 53n) - 1n, 1n << 53n, 9223372036854775807n, 18446744073709551615n];
  for (const k of keys) {
    const val = (k ^ 0x5555555555555555n) & 0xFFFFFFFFFFFFFFFFn;
    map.set(k, val);
  }
  for (const k of keys) {
    const expectedVal = (k ^ 0x5555555555555555n) & 0xFFFFFFFFFFFFFFFFn;
    assert.strictEqual(map.has(k), true);
    assert.strictEqual(map.get(k), expectedVal);
  }
  assert.strictEqual(map.size(), BigInt(keys.length));
});

// 3. ExpanseStrMap Tests
console.log('\n--- ExpanseStrMap Tests ---');

test('ExpanseStrMap basic operations and ordered navigation', () => {
  const strmap = new ExpanseStrMap();
  assert.strictEqual(strmap.size(), 0n);

  assert.strictEqual(strmap.set('apple', 1n), null);
  assert.strictEqual(strmap.set('banana', 2n), null);
  assert.strictEqual(strmap.set('cherry', 3n), null);
  assert.strictEqual(strmap.set('apple', 10n), 1n);

  assert.strictEqual(strmap.size(), 3n);
  assert.strictEqual(strmap.get('apple'), 10n);
  assert.strictEqual(strmap.get('banana'), 2n);
  assert.strictEqual(strmap.get('unknown'), null);
  assert.strictEqual(strmap.has('cherry'), true);

  assert.deepStrictEqual(strmap.first(), { key: 'apple', value: 10n });
  assert.deepStrictEqual(strmap.last(), { key: 'cherry', value: 3n });
  assert.deepStrictEqual(strmap.next('apple'), { key: 'banana', value: 2n });
  assert.deepStrictEqual(strmap.prev('cherry'), { key: 'banana', value: 2n });

  assert.deepStrictEqual(strmap.keys(), ['apple', 'banana', 'cherry']);
  assert.deepStrictEqual(strmap.values(), [10n, 2n, 3n]);

  assert.strictEqual(strmap.delete('banana'), true);
  assert.strictEqual(strmap.size(), 2n);

  // NUL byte rejection
  assert.throws(() => {
    strmap.set('invalid\0key', 99n);
  }, /NUL bytes/);
});

// 4. ExpanseBytesMap Tests
console.log('\n--- ExpanseBytesMap Tests ---');

test('ExpanseBytesMap supporting arbitrary binary keys with NUL bytes', () => {
  const bytesmap = new ExpanseBytesMap();

  const k1 = Buffer.from([0x00, 0x01, 0x02]);
  const k2 = Buffer.from([0x00, 0x00, 0xff]);
  const k3 = Buffer.from('hello-world');

  assert.strictEqual(bytesmap.set(k1, 100n), null);
  assert.strictEqual(bytesmap.set(k2, 200n), null);
  assert.strictEqual(bytesmap.set(k3, 300n), null);

  assert.strictEqual(bytesmap.size(), 3n);
  assert.strictEqual(bytesmap.get(k1), 100n);
  assert.strictEqual(bytesmap.get(k2), 200n);
  assert.strictEqual(bytesmap.get(k3), 300n);
  assert.strictEqual(bytesmap.has(k1), true);

  assert.strictEqual(bytesmap.delete(k2), true);
  assert.strictEqual(bytesmap.size(), 2n);
  assert.strictEqual(bytesmap.get(k2), null);

  const keys = bytesmap.keys();
  assert.strictEqual(keys.length, 2);
  assert(Buffer.isBuffer(keys[0]));
});

// 4b. ExpanseOrderedBytesMap Tests (C ABI: expanse_ordered_bytesmap_*)
console.log('\n--- ExpanseOrderedBytesMap Tests ---');

test('ExpanseOrderedBytesMap round-trip: set returns the previous value, get, has, size, delete', () => {
  const map = new ExpanseOrderedBytesMap();
  const k = Buffer.from([0x10, 0x20, 0x30]);

  assert.strictEqual(map.isEmpty(), true);
  assert.strictEqual(map.set(k, 5n), null);
  assert.strictEqual(map.set(k, 6n), 5n);
  assert.strictEqual(map.get(k), 6n);
  assert.strictEqual(map.has(k), true);
  assert.strictEqual(map.size(), 1n);
  assert.strictEqual(map.isEmpty(), false);
  assert.strictEqual(map.delete(k), true);
  assert.strictEqual(map.get(k), null);
  assert.strictEqual(map.size(), 0n);
});

test('ExpanseOrderedBytesMap orders keys as unsigned bytes, lexicographically', () => {
  const map = new ExpanseOrderedBytesMap();
  // Inserted out of order; 0x80 and 0xFF must sort after 0x7F (unsigned, not signed).
  const keys = [[0xff], [0x80], [0x7f], [0x00], [0x01], [0x00, 0x00], [0x00, 0xff], [0x80, 0x00]];
  keys.forEach((bytes, i) => map.set(Buffer.from(bytes), BigInt(i)));

  const sorted = keys.map((b) => Buffer.from(b)).sort(Buffer.compare);
  assert.deepStrictEqual(map.first().key, sorted[0]);
  assert.deepStrictEqual(map.last().key, sorted[sorted.length - 1]);
  assert.deepStrictEqual(map.first().key, Buffer.from([0x00]));
  assert.deepStrictEqual(map.last().key, Buffer.from([0xff]));

  // Walking higher() from the first key visits every key in ascending order.
  const walked = [];
  let cur = map.first();
  while (cur !== null) {
    walked.push(cur.key);
    cur = map.higher(cur.key);
  }
  assert.strictEqual(walked.length, sorted.length);
  walked.forEach((k, i) => assert.deepStrictEqual(k, sorted[i]));

  // A key sorts before any longer key it prefixes: [0x00] < [0x00,0x00] < [0x00,0xff] < [0x01].
  assert.deepStrictEqual(map.higher(Buffer.from([0x00])).key, Buffer.from([0x00, 0x00]));
  assert.deepStrictEqual(map.higher(Buffer.from([0x00, 0x00])).key, Buffer.from([0x00, 0xff]));
  assert.deepStrictEqual(map.higher(Buffer.from([0x00, 0xff])).key, Buffer.from([0x01]));
});

test('ExpanseOrderedBytesMap navigation: ceiling, higher, floor, lower on present and absent probes', () => {
  const map = new ExpanseOrderedBytesMap();
  map.set(Buffer.from([0x02]), 20n);
  map.set(Buffer.from([0x04]), 40n);
  map.set(Buffer.from([0x06]), 60n);

  // Present probe: ceiling/floor include it, higher/lower exclude it.
  assert.deepStrictEqual(map.ceiling(Buffer.from([0x04])), { key: Buffer.from([0x04]), value: 40n });
  assert.deepStrictEqual(map.floor(Buffer.from([0x04])), { key: Buffer.from([0x04]), value: 40n });
  assert.deepStrictEqual(map.higher(Buffer.from([0x04])), { key: Buffer.from([0x06]), value: 60n });
  assert.deepStrictEqual(map.lower(Buffer.from([0x04])), { key: Buffer.from([0x02]), value: 20n });

  // Absent probe between keys.
  assert.deepStrictEqual(map.ceiling(Buffer.from([0x03])), { key: Buffer.from([0x04]), value: 40n });
  assert.deepStrictEqual(map.floor(Buffer.from([0x03])), { key: Buffer.from([0x02]), value: 20n });

  // Out-of-range probes return null.
  assert.strictEqual(map.ceiling(Buffer.from([0x07])), null);
  assert.strictEqual(map.higher(Buffer.from([0x06])), null);
  assert.strictEqual(map.floor(Buffer.from([0x01])), null);
  assert.strictEqual(map.lower(Buffer.from([0x02])), null);
});

test('ExpanseOrderedBytesMap empty key is valid, sorts first, and round-trips', () => {
  const map = new ExpanseOrderedBytesMap();
  const empty = Buffer.alloc(0);

  assert.strictEqual(map.set(empty, 7n), null);
  assert.strictEqual(map.get(empty), 7n);
  assert.strictEqual(map.has(empty), true);
  assert.strictEqual(map.size(), 1n);

  map.set(Buffer.from([0x00]), 8n);
  const first = map.first();
  assert.strictEqual(first.key.length, 0);
  assert.strictEqual(first.value, 7n);
  assert.strictEqual(map.lower(Buffer.from([0x00])).key.length, 0);
  assert.strictEqual(map.lower(Buffer.from([0x00])).value, 7n);
  assert.strictEqual(map.floor(empty).value, 7n);
  assert.strictEqual(map.higher(empty).value, 8n);

  assert.strictEqual(map.delete(empty), true);
  assert.deepStrictEqual(map.first().key, Buffer.from([0x00]));
});

test('ExpanseOrderedBytesMap keys containing 0x00 and 0xFF round-trip byte-exact', () => {
  const map = new ExpanseOrderedBytesMap();
  const a = Buffer.from([0x00, 0x00, 0x00]);
  const b = Buffer.from([0xff, 0x00, 0xff]);
  const c = Buffer.from([0x00, 0xff]);
  map.set(a, 1n);
  map.set(b, 2n);
  map.set(c, 3n);

  assert.strictEqual(map.get(a), 1n);
  assert.strictEqual(map.get(b), 2n);
  assert.strictEqual(map.get(c), 3n);
  assert.deepStrictEqual(map.first().key, a);
  assert.deepStrictEqual(map.last().key, b);
  assert.deepStrictEqual(map.higher(a).key, c);
  // The returned key is the exact stored bytes, not a truncated or C-string-cut prefix.
  assert.deepStrictEqual(map.floor(Buffer.from([0x00, 0xff, 0x00])).key, c);
});

test('ExpanseOrderedBytesMap returns keys longer than 8 bytes without truncation', () => {
  const map = new ExpanseOrderedBytesMap();
  const shared = Buffer.from('prefix-shared-across-keys:');
  const long1 = Buffer.concat([shared, Buffer.from('alpha-suffix-that-is-long')]);
  const long2 = Buffer.concat([shared, Buffer.from('beta-suffix-that-is-longer-still')]);
  const huge = Buffer.alloc(300, 0xab);
  huge[0] = 0x00;
  huge[299] = 0xff;

  map.set(long1, 11n);
  map.set(long2, 22n);
  map.set(huge, 33n);

  assert.strictEqual(map.get(long1), 11n);
  assert.strictEqual(map.get(long2), 22n);
  assert.strictEqual(map.get(huge), 33n);

  const first = map.first();
  assert.strictEqual(first.key.length, 300);
  assert.deepStrictEqual(first.key, huge);
  assert.deepStrictEqual(map.last().key, long2);
  assert.deepStrictEqual(map.ceiling(long1).key, long1);
  assert.deepStrictEqual(map.higher(long1).key, long2);
});

test('ExpanseOrderedBytesMap absent key: get and delete report absence; insertSlot inserts zero once', () => {
  const map = new ExpanseOrderedBytesMap();
  const absent = Buffer.from([0x42, 0x42]);

  assert.strictEqual(map.get(absent), null);
  assert.strictEqual(map.has(absent), false);
  assert.strictEqual(map.delete(absent), false);
  assert.strictEqual(map.size(), 0n);
  assert.strictEqual(map.first(), null);
  assert.strictEqual(map.last(), null);

  // insertSlot inserts 0 for an absent key and keeps an existing value.
  assert.strictEqual(map.insertSlot(absent), 0n);
  assert.strictEqual(map.get(absent), 0n);
  map.set(absent, 9n);
  assert.strictEqual(map.insertSlot(absent), 9n);
  assert.strictEqual(map.get(absent), 9n);
  assert.strictEqual(map.size(), 1n);
});

test('ExpanseOrderedBytesMap iteration observes mutations made between steps', () => {
  const map = new ExpanseOrderedBytesMap();
  map.set(Buffer.from([0x01]), 1n);
  map.set(Buffer.from([0x02]), 2n);
  map.set(Buffer.from([0x03]), 3n);

  assert.deepStrictEqual(map.first().key, Buffer.from([0x01]));
  map.delete(Buffer.from([0x01]));
  assert.deepStrictEqual(map.first().key, Buffer.from([0x02]));

  // A key inserted below the current first becomes the new first.
  map.set(Buffer.from([0x00, 0x09]), 9n);
  assert.deepStrictEqual(map.first(), { key: Buffer.from([0x00, 0x09]), value: 9n });
  assert.deepStrictEqual(map.higher(Buffer.from([0x00, 0x09])), { key: Buffer.from([0x02]), value: 2n });

  // Overwriting a value is visible to the navigation that returns it.
  map.set(Buffer.from([0x03]), 33n);
  assert.deepStrictEqual(map.last(), { key: Buffer.from([0x03]), value: 33n });
});

test('ExpanseOrderedBytesMap memory accounting and clear', () => {
  const map = new ExpanseOrderedBytesMap();
  for (let i = 0; i < 64; i++) {
    map.set(Buffer.from([i, i ^ 0x5a]), BigInt(i));
  }
  assert.strictEqual(map.size(), 64n);
  assert.strictEqual(typeof map.memUsed(), 'bigint');
  assert.ok(map.memUsed() > 0n);
  assert.ok(map.memHeld() >= map.memUsed());
  assert.strictEqual(typeof map.shrinkToFit(), 'bigint');

  map.clear();
  assert.strictEqual(map.size(), 0n);
  assert.strictEqual(map.isEmpty(), true);
  assert.strictEqual(map.first(), null);
  assert.strictEqual(map.get(Buffer.from([0, 0x5a])), null);
});

// 5. ExpanseBlobMap Tests
console.log('\n--- ExpanseBlobMap Tests ---');

test('ExpanseBlobMap inline and arena payloads, metadata, prune, compact, and serialization', () => {
  const blobmap = new ExpanseBlobMap();

  // Short inline payload (<= 7 bytes, metadata 0)
  const shortBuf = Buffer.from('inline');
  blobmap.set(1n, shortBuf, 0);

  // Large arena payload (> 7 bytes)
  const largeBuf = Buffer.from('This is a large arena payload that exceeds 7 bytes!');
  blobmap.set(2n, largeBuf, 20);

  assert.strictEqual(blobmap.size(), 2n);
  assert.strictEqual(blobmap.has(1n), true);
  assert.strictEqual(blobmap.has(2n), true);

  // get
  const gotShort = blobmap.get(1n);
  assert(Buffer.isBuffer(gotShort));
  assert.strictEqual(gotShort.toString('utf8'), 'inline');

  const gotLarge = blobmap.get(2n);
  assert.strictEqual(gotLarge.toString('utf8'), 'This is a large arena payload that exceeds 7 bytes!');

  // getWithMeta
  const metaShort = blobmap.getWithMeta(1n);
  assert.strictEqual(metaShort.isInline, true);
  assert.strictEqual(metaShort.hotMeta, 0);
  assert.strictEqual(metaShort.payload.toString('utf8'), 'inline');

  // A short payload with metadata is an arena record that keeps it (#1548).
  blobmap.set(5n, Buffer.from('tiny'), 10);
  const metaTiny = blobmap.getWithMeta(5n);
  assert.strictEqual(metaTiny.isInline, false);
  assert.strictEqual(metaTiny.hotMeta, 10);
  assert.strictEqual(blobmap.delete(5n), true);

  const metaLarge = blobmap.getWithMeta(2n);
  assert.strictEqual(metaLarge.isInline, false);
  assert.strictEqual(metaLarge.hotMeta, 20);
  assert.strictEqual(metaLarge.payload.toString('utf8'), 'This is a large arena payload that exceeds 7 bytes!');

  // Pruning
  blobmap.set(3n, Buffer.from('arena payload item 3'), 100);
  blobmap.set(4n, Buffer.from('arena payload item 4'), 200);

  const pruned = blobmap.prune((key, hotMeta) => {
    return hotMeta >= 100;
  });
  assert.strictEqual(pruned, 2);
  assert.strictEqual(blobmap.size(), 2n);
  assert.strictEqual(blobmap.has(3n), false);
  assert.strictEqual(blobmap.has(4n), false);

  // Compact
  const stats = blobmap.compact();
  assert(stats.liveBytesBefore >= 0n);
  assert(stats.liveBytesAfter >= 0n);

  // Save to image file and reopen
  const tmpFile = path.join(os.tmpdir(), `expanse_test_${Date.now()}.img`);
  const bytesWritten = blobmap.saveImage(tmpFile);
  assert(bytesWritten > 0);

  const reloaded = ExpanseBlobMap.openImage(tmpFile);
  assert.strictEqual(reloaded.size(), 2n);
  assert.strictEqual(reloaded.get(1n).toString('utf8'), 'inline');
  assert.strictEqual(reloaded.get(2n).toString('utf8'), 'This is a large arena payload that exceeds 7 bytes!');

  try {
    fs.unlinkSync(tmpFile);
  } catch (_) {}
});

// #1300: one 4 KiB chunk fits a 6000-byte cap, so after removals the insert
// compacts and the 3100-byte record still does not fit the chunk's tail.
test('ExpanseBlobMap capacity cap, reclaim switch, arena stats and refusal status', () => {
  const dflt = new ExpanseBlobMap().arenaStats();
  assert.strictEqual(dflt.maxCapacity, 1n << 30n);
  assert.strictEqual(dflt.reclaimAtCap, true);

  const bm = new ExpanseBlobMap(4096, 6000);
  assert.strictEqual(bm.arenaStats().maxCapacity, 6000n);
  for (let k = 0; k < 4; k++) {
    assert.strictEqual(bm.setStatus(BigInt(k), Buffer.alloc(1000, k), 1), 'ok');
  }
  const big = Buffer.alloc(3100, 9);
  assert.strictEqual(bm.setStatus(9n, big, 1), 'cap_refused');
  const st = bm.arenaStats();
  assert.strictEqual(st.liveBytes, 4n * 1008n);
  assert.strictEqual(st.allocatedBytes, 4096n);
  for (let k = 1; k < 4; k++) assert.strictEqual(bm.delete(BigInt(k)), true);
  assert.strictEqual(bm.setStatus(9n, big, 1), 'arena_full');
  assert.strictEqual(bm.size(), 1n);
  assert.strictEqual(bm.setStatus(1n, Buffer.alloc(100, 1), 1 << 24), 'meta_overflow');

  const off = new ExpanseBlobMap(4096, 6000);
  off.setReclaimAtCap(false);
  assert.strictEqual(off.reclaimAtCap(), false);
  assert.throws(() => new ExpanseBlobMap(4096, -1));
});

// 6. SyncExpanseMap & SyncExpanseSet Tests
console.log('\n--- SyncExpanseMap & SyncExpanseSet Tests ---');

test('SyncExpanseMap concurrent map operations', () => {
  const syncMap = new SyncExpanseMap();
  assert.strictEqual(syncMap.size(), 0n);

  syncMap.set(100n, 1000n);
  syncMap.set(200n, 2000n);
  syncMap.set(300n, 3000n);

  assert.strictEqual(syncMap.size(), 3n);
  assert.strictEqual(syncMap.get(200n), 2000n);
  assert.strictEqual(syncMap.has(100n), true);

  assert.deepStrictEqual(syncMap.first(), { key: 100n, value: 1000n });
  assert.deepStrictEqual(syncMap.last(), { key: 300n, value: 3000n });
  assert.deepStrictEqual(syncMap.next(100n), { key: 200n, value: 2000n });
  assert.deepStrictEqual(syncMap.next(100n, true), { key: 100n, value: 1000n });
  assert.deepStrictEqual(syncMap.next(150n, true), { key: 200n, value: 2000n });
  assert.deepStrictEqual(syncMap.prev(300n), { key: 200n, value: 2000n });
  assert.deepStrictEqual(syncMap.prev(300n, true), { key: 300n, value: 3000n });
  assert.deepStrictEqual(syncMap.prev(250n, true), { key: 200n, value: 2000n });
  assert.strictEqual(syncMap.next(300n), null);
  assert.strictEqual(syncMap.prev(100n), null);
  assert.strictEqual(syncMap.countRange(150n, 350n), 2n);

  assert.strictEqual(syncMap.delete(200n), true);
  assert.strictEqual(syncMap.size(), 2n);
  assert.deepStrictEqual(syncMap.next(100n), { key: 300n, value: 3000n });
  assert.deepStrictEqual(syncMap.prev(300n), { key: 100n, value: 1000n });
});

test('SyncExpanseMap compareExchange covers every branch', () => {
  const m = new SyncExpanseMap();
  // absent == absent with nothing to store: succeeds, observes absent.
  assert.deepStrictEqual(m.compareExchange(1n, null, null), { stored: true, observed: null });
  assert.strictEqual(m.has(1n), false);
  // insert-if-absent on an absent key (a Number is accepted like everywhere else).
  assert.deepStrictEqual(m.compareExchange(1, null, 10), { stored: true, observed: null });
  assert.strictEqual(m.get(1n), 10n);
  // insert-if-absent against a present key fails and reports the word seen.
  assert.deepStrictEqual(m.compareExchange(1n, null, 99n), { stored: false, observed: 10n });
  assert.strictEqual(m.get(1n), 10n);
  // absent == absent against a present key fails.
  assert.deepStrictEqual(m.compareExchange(1n, undefined, undefined), { stored: false, observed: 10n });
  // expected present, wrong value: fails, value unchanged.
  assert.deepStrictEqual(m.compareExchange(1n, 11n, 12n), { stored: false, observed: 10n });
  assert.strictEqual(m.get(1n), 10n);
  // expected present against an absent key: fails, observes absent.
  assert.deepStrictEqual(m.compareExchange(2n, 5n, 6n), { stored: false, observed: null });
  assert.strictEqual(m.has(2n), false);
  // replace on match.
  assert.deepStrictEqual(m.compareExchange(1n, 10n, 20n), { stored: true, observed: 10n });
  assert.strictEqual(m.get(1n), 20n);
  // remove-if-equals: mismatch leaves it, match removes it.
  assert.deepStrictEqual(m.compareExchange(1n, 21n, null), { stored: false, observed: 20n });
  assert.strictEqual(m.get(1n), 20n);
  assert.deepStrictEqual(m.compareExchange(1n, 20n, null), { stored: true, observed: 20n });
  assert.strictEqual(m.has(1n), false);
  // Every u64 is a legal value, 0 and the maximum included.
  const top = 2n ** 64n - 1n;
  assert.deepStrictEqual(m.compareExchange(3n, null, 0n), { stored: true, observed: null });
  assert.deepStrictEqual(m.compareExchange(3n, 0n, top), { stored: true, observed: 0n });
  assert.strictEqual(m.get(3n), top);
  assert.throws(() => m.compareExchange(3n, null, 2n ** 64n));
  assert.throws(() => m.compareExchange(3n, -1n, null));
});

test('SyncExpanseMap getOrInsert stores only into an absent key', () => {
  const m = new SyncExpanseMap();
  assert.strictEqual(m.getOrInsert(7n, 70n), null); // stored
  assert.strictEqual(m.get(7n), 70n);
  assert.strictEqual(m.getOrInsert(7n, 71n), 70n); // present: existing value, untouched
  assert.strictEqual(m.get(7n), 70n);
  assert.strictEqual(m.getOrInsert(8n, 0n), null); // zero is a value, not absence
  assert.strictEqual(m.getOrInsert(8n, 5n), 0n);
});

test('SyncExpanseMap update stores, removes and propagates errors', () => {
  const m = new SyncExpanseMap();
  const seen = [];
  const inc = (cur) => {
    seen.push(cur);
    return (cur ?? 0n) + 1n;
  };
  assert.strictEqual(m.update(5n, inc), null); // absent: replaced value is null
  assert.strictEqual(m.update(5n, inc), 1n);
  assert.strictEqual(m.get(5n), 2n);
  assert.deepStrictEqual(seen, [null, 1n]);
  // Returning null removes the key and reports the replaced value; undefined removes too.
  assert.strictEqual(m.update(5n, () => null), 2n);
  assert.strictEqual(m.has(5n), false);
  assert.strictEqual(m.update(5n, () => null), null);
  m.set(6n, 60n);
  assert.strictEqual(m.update(6n, () => undefined), 60n);
  assert.strictEqual(m.has(6n), false);
  // A Number result is accepted.
  assert.strictEqual(m.update(9n, () => 4), null);
  assert.strictEqual(m.get(9n), 4n);
  // An exception from fn propagates and leaves the key unchanged.
  m.set(6n, 60n);
  assert.throws(() => m.update(6n, () => { throw new Error('fn failed'); }), /fn failed/);
  assert.strictEqual(m.get(6n), 60n);
  // A result that is not a u64 is an error, not a store.
  assert.throws(() => m.update(6n, () => -1n));
  assert.strictEqual(m.get(6n), 60n);
});

test('SyncExpanseMap update retries on interference and counts exactly', () => {
  const m = new SyncExpanseMap();
  m.set(1n, 100n);
  const calls = [];
  // A competing write lands between fn's read and the exchange.
  const prev = m.update(1n, (cur) => {
    calls.push(cur);
    if (calls.length === 1) m.set(1n, 200n);
    return cur + 1n;
  });
  assert.strictEqual(prev, 200n);
  assert.deepStrictEqual(calls, [100n, 200n]);
  assert.strictEqual(m.get(1n), 201n);
  // The addon's objects live on one JS thread, so there is no multi-threaded
  // form of this test; a long run of updates must still end at the exact total.
  const counter = new SyncExpanseMap();
  for (let i = 0; i < 20000; i++) counter.update(0n, (cur) => (cur ?? 0n) + 1n);
  assert.strictEqual(counter.get(0n), 20000n);
});

test('SyncExpanseMap memUsed grows with the population', () => {
  const syncMap = new SyncExpanseMap();
  const empty = syncMap.memUsed();
  assert.strictEqual(typeof empty, 'bigint');
  for (let k = 0n; k < 20000n; k += 3n) {
    syncMap.set(k, k);
  }
  assert.ok(syncMap.memUsed() > empty);
});

test('memHeld / shrinkToFit return the blocks a drained container keeps', () => {
  const n = 20000n;
  const map = new ExpanseMap();
  const set = new ExpanseSet();
  const strMap = new ExpanseStrMap();
  for (let k = 0n; k < n; k++) {
    map.set(k * 7n, k);
    set.add(k * 7n);
    strMap.set(`key/${String(k).padStart(8, '0')}`, k);
  }
  for (const c of [map, set, strMap]) {
    assert.strictEqual(typeof c.memHeld(), 'bigint');
    assert.ok(c.memHeld() >= c.memUsed());
  }
  for (let k = 0n; k < n; k++) {
    assert.ok(map.delete(k * 7n));
    assert.ok(set.remove(k * 7n));
    assert.ok(strMap.delete(`key/${String(k).padStart(8, '0')}`));
  }
  for (const [c, retains] of [[map, true], [set, true], [strMap, true]]) {
    const held = c.memHeld();
    const used = c.memUsed();
    if (retains) {
      assert.strictEqual(used, 0n);
      assert.ok(held > 0n);
    }
    assert.strictEqual(c.shrinkToFit(), held - used);
    assert.strictEqual(c.memHeld(), c.memUsed());
    assert.strictEqual(c.memUsed(), used);
    assert.strictEqual(c.shrinkToFit(), 0n);
  }
});

test('SyncExpanseMap / SyncExpanseSet memHeld and shrinkToFit after a half drain', () => {
  // The epoch collector keeps the freed blocks, so the map holds more than it
  // uses. shrinkToFit returns those past their grace period and memHeld falls
  // by exactly what it reports; blocks still in their grace period stay held,
  // so memHeld is not asserted to reach memUsed.
  const n = 50000n;
  const syncMap = new SyncExpanseMap();
  const syncSet = new SyncExpanseSet();
  for (let k = 0n; k < n; k++) {
    syncMap.set(k * 0x9e3779b1n, k);
    syncSet.add(k * 0x9e3779b1n);
  }
  for (let k = 0n; k < n; k += 2n) {
    assert.ok(syncMap.delete(k * 0x9e3779b1n));
    assert.ok(syncSet.remove(k * 0x9e3779b1n));
  }
  assert.ok(syncMap.memHeld() > syncMap.memUsed());
  for (const c of [syncMap, syncSet]) {
    const held = c.memHeld();
    assert.strictEqual(typeof held, 'bigint');
    assert.ok(held > 0n);
    const released = c.shrinkToFit();
    assert.strictEqual(typeof released, 'bigint');
    assert.ok(released >= 0n);
    assert.strictEqual(c.memHeld(), held - released);
  }
  for (let k = 1n; k < n; k += 2n) {
    assert.strictEqual(syncMap.get(k * 0x9e3779b1n), k);
    assert.ok(syncSet.has(k * 0x9e3779b1n));
  }
});

test('SyncExpanseMap ordered reads at the ends of the key space', () => {
  const top = 2n ** 64n - 1n;
  const syncMap = new SyncExpanseMap();
  assert.strictEqual(syncMap.first(), null);
  assert.strictEqual(syncMap.last(), null);
  assert.strictEqual(syncMap.next(0n, true), null);
  assert.strictEqual(syncMap.prev(top, true), null);

  syncMap.set(0n, 1n);
  syncMap.set(top, 2n);
  assert.deepStrictEqual(syncMap.first(), { key: 0n, value: 1n });
  assert.deepStrictEqual(syncMap.last(), { key: top, value: 2n });
  assert.deepStrictEqual(syncMap.next(0n), { key: top, value: 2n });
  assert.deepStrictEqual(syncMap.prev(top), { key: 0n, value: 1n });
  assert.deepStrictEqual(syncMap.next(top, true), { key: top, value: 2n });
  assert.deepStrictEqual(syncMap.prev(0n, true), { key: 0n, value: 1n });
  assert.strictEqual(syncMap.next(top), null);
  assert.strictEqual(syncMap.prev(0n), null);
});

test('SyncExpanseSet concurrent set operations', () => {
  const syncSet = new SyncExpanseSet();
  assert.strictEqual(syncSet.size(), 0n);

  assert.strictEqual(syncSet.add(10n), true);
  assert.strictEqual(syncSet.add(20n), true);
  assert.strictEqual(syncSet.add(30n), true);

  assert.strictEqual(syncSet.size(), 3n);
  assert.strictEqual(syncSet.has(20n), true);
  assert.strictEqual(syncSet.has(99n), false);

  assert.strictEqual(syncSet.first(), 10n);
  assert.strictEqual(syncSet.last(), 30n);
  assert.strictEqual(syncSet.next(10n), 20n);
  assert.strictEqual(syncSet.prev(30n), 20n);
  assert.strictEqual(syncSet.rank(30n), 2n);
  assert.strictEqual(syncSet.select(1n), 20n);
  assert.strictEqual(syncSet.countRange(15n, 35n), 2n);

  assert.deepStrictEqual(syncSet.toArray(), [10n, 20n, 30n]);

  assert.strictEqual(syncSet.remove(20n), true);
  assert.strictEqual(syncSet.size(), 2n);
});

// 7. Key conversion correctness (BigInt lossless + Number precision)
console.log('\n--- Key Conversion Tests ---');

test('rejects a BigInt that does not fit in u64 instead of silently truncating', () => {
  const map = new ExpanseMap();
  // 2^64 + 1 previously wrapped to 1 (lossless flag ignored). Must throw now.
  assert.throws(() => map.set(2n ** 64n + 1n, 5n), /unsigned 64-bit|out of u64 range/i);
  assert.throws(() => map.set(2n ** 100n, 5n));
  // The largest valid u64 key must still round-trip exactly.
  const maxU64 = 2n ** 64n - 1n;
  map.set(maxU64, 9n);
  assert.strictEqual(map.get(maxU64), 9n);
  // A distinct large key must not collide with maxU64.
  map.set(maxU64 - 1n, 8n);
  assert.strictEqual(map.get(maxU64), 9n);
  assert.strictEqual(map.get(maxU64 - 1n), 8n);
});

test('rejects negative BigInt keys', () => {
  const map = new ExpanseMap();
  assert.throws(() => map.set(-1n, 5n), /negative/i);
});

test('rejects fractional and >2^53 Number keys', () => {
  const map = new ExpanseMap();
  assert.throws(() => map.set(1.5, 5n), /integer/i);
  // 2^53 + 2 is not exactly representable as a distinct integer -> reject.
  assert.throws(() => map.set(9007199254740994, 5n), /2\^53|precision/i);
  // Values at or below 2^53 are fine.
  map.set(9007199254740992, 7n); // 2^53 exactly
  assert.strictEqual(map.get(9007199254740992n), 7n);
});

test('saveImage returns a BigInt byte count', () => {
  const blobmap = new ExpanseBlobMap();
  blobmap.set(1n, Buffer.from('hello'));
  const tmpFile = path.join(os.tmpdir(), `expanse_saveimg_${Date.now()}.img`);
  const bytesWritten = blobmap.saveImage(tmpFile);
  assert.strictEqual(typeof bytesWritten, 'bigint');
  assert(bytesWritten > 0n);
  // openImage takes a single path argument (the former no-op mmap arg was removed).
  const reloaded = ExpanseBlobMap.openImage(tmpFile);
  assert.strictEqual(reloaded.get(1n).toString('utf8'), 'hello');
  try {
    fs.unlinkSync(tmpFile);
  } catch (_) {}
});

// 6b. Shared string, bytes, ordered-bytes and blob maps (#1538)
console.log('\n--- Shared string / bytes / ordered-bytes / blob maps ---');
// The addon's objects belong to one JS thread, so none of these has a
// multi-threaded form; interleaved writes (from inside `update`'s callback) and
// long retry loops are the concurrency coverage here.

const TOP = 2n ** 64n - 1n;

// The word-valued maps share one contract; run it over each with its own key type.
const wordMaps = [
  ['SyncExpanseStrMap', () => new SyncExpanseStrMap(), (n) => `key-${n}`],
  ['SyncExpanseBytesMap', () => new SyncExpanseBytesMap(), (n) => Buffer.from(`key-${n}`)],
  ['SyncExpanseOrderedBytesMap', () => new SyncExpanseOrderedBytesMap(), (n) => Buffer.from(`key-${n}`)],
];
for (const [name, make, key] of wordMaps) {
  test(`${name} point operations`, () => {
    const m = make();
    assert.strictEqual(m.size(), 0n);
    assert.strictEqual(m.isEmpty(), true);
    assert.strictEqual(m.set(key(1), 10n), null);
    assert.strictEqual(m.set(key(1), 11), 10n); // a Number value is accepted
    assert.strictEqual(m.set(key(2), TOP), null);
    assert.strictEqual(m.get(key(1)), 11n);
    assert.strictEqual(m.get(key(2)), TOP);
    assert.strictEqual(m.get(key(3)), null);
    assert.strictEqual(m.has(key(1)), true);
    assert.strictEqual(m.has(key(3)), false);
    assert.strictEqual(m.size(), 2n);
    assert.strictEqual(m.delete(key(1)), true);
    assert.strictEqual(m.delete(key(1)), false);
    assert.strictEqual(m.has(key(1)), false);
    assert.throws(() => m.set(key(9), -1n));
    assert.throws(() => m.set(key(9), 2n ** 64n));
    assert.strictEqual(m.has(key(9)), false);
    m.clear();
    assert.strictEqual(m.size(), 0n);
    assert.strictEqual(m.isEmpty(), true);
  });

  test(`${name} accounting reads`, () => {
    const m = make();
    for (let i = 0; i < 2000; i++) m.set(key(i), BigInt(i));
    const used = m.memUsed();
    assert(used > 0n);
    assert(m.memHeld() >= used);
    for (let i = 0; i < 2000; i++) m.delete(key(i));
    m.shrinkToFit();
    assert(m.memUsed() < used);
  });

  test(`${name} compareExchange covers every branch`, () => {
    const m = make();
    const k = key(1);
    assert.deepStrictEqual(m.compareExchange(k, null, null), { stored: true, observed: null });
    assert.strictEqual(m.has(k), false);
    assert.deepStrictEqual(m.compareExchange(k, null, 10), { stored: true, observed: null });
    assert.deepStrictEqual(m.compareExchange(k, null, 99n), { stored: false, observed: 10n });
    assert.deepStrictEqual(m.compareExchange(k, undefined, undefined), { stored: false, observed: 10n });
    assert.deepStrictEqual(m.compareExchange(k, 11n, 12n), { stored: false, observed: 10n });
    assert.deepStrictEqual(m.compareExchange(key(2), 5n, 6n), { stored: false, observed: null });
    assert.strictEqual(m.has(key(2)), false);
    assert.deepStrictEqual(m.compareExchange(k, 10n, 20n), { stored: true, observed: 10n });
    assert.strictEqual(m.get(k), 20n);
    assert.deepStrictEqual(m.compareExchange(k, 21n, null), { stored: false, observed: 20n });
    assert.deepStrictEqual(m.compareExchange(k, 20n, null), { stored: true, observed: 20n });
    assert.strictEqual(m.has(k), false);
    assert.deepStrictEqual(m.compareExchange(k, null, 0n), { stored: true, observed: null });
    assert.deepStrictEqual(m.compareExchange(k, 0n, TOP), { stored: true, observed: 0n });
    assert.strictEqual(m.get(k), TOP);
    assert.throws(() => m.compareExchange(k, null, 2n ** 64n));
    assert.throws(() => m.compareExchange(k, -1n, null));
  });

  test(`${name} getOrInsert stores only into an absent key`, () => {
    const m = make();
    assert.strictEqual(m.getOrInsert(key(7), 70n), null);
    assert.strictEqual(m.get(key(7)), 70n);
    assert.strictEqual(m.getOrInsert(key(7), 71n), 70n);
    assert.strictEqual(m.get(key(7)), 70n);
    assert.strictEqual(m.getOrInsert(key(8), 0n), null); // zero is a value, not absence
    assert.strictEqual(m.getOrInsert(key(8), 5n), 0n);
  });

  test(`${name} update retries on interference and counts exactly`, () => {
    const m = make();
    const k = key(1);
    assert.strictEqual(m.update(k, (cur) => (cur ?? 0n) + 1n), null);
    assert.strictEqual(m.update(k, (cur) => cur + 1n), 1n);
    assert.strictEqual(m.update(k, () => null), 2n); // null removes
    assert.strictEqual(m.has(k), false);
    m.set(k, 100n);
    const calls = [];
    const prev = m.update(k, (cur) => {
      calls.push(cur);
      if (calls.length === 1) m.set(k, 200n); // a competing write between the read and the exchange
      return cur + 1n;
    });
    assert.strictEqual(prev, 200n);
    assert.deepStrictEqual(calls, [100n, 200n]);
    assert.strictEqual(m.get(k), 201n);
    assert.throws(() => m.update(k, () => { throw new Error('fn failed'); }), /fn failed/);
    assert.strictEqual(m.get(k), 201n);
    assert.throws(() => m.update(k, () => -1n));
    assert.strictEqual(m.get(k), 201n);
    const counter = make();
    for (let i = 0; i < 20000; i++) counter.update(key(0), (cur) => (cur ?? 0n) + 1n);
    assert.strictEqual(counter.get(key(0)), 20000n);
  });
}

test('SyncExpanseStrMap rejects an embedded NUL on every key-taking method', () => {
  const m = new SyncExpanseStrMap();
  const bad = 'a\0b';
  assert.throws(() => m.set(bad, 1n), /NUL/);
  assert.throws(() => m.get(bad), /NUL/);
  assert.throws(() => m.has(bad), /NUL/);
  assert.throws(() => m.delete(bad), /NUL/);
  assert.throws(() => m.compareExchange(bad, null, 1n), /NUL/);
  assert.throws(() => m.getOrInsert(bad, 1n), /NUL/);
  assert.throws(() => m.update(bad, () => 1n), /NUL/);
  assert.throws(() => m.next(bad), /NUL/);
  assert.throws(() => m.prev(bad), /NUL/);
  assert.strictEqual(m.size(), 0n);
});

test('SyncExpanseStrMap ordered reads', () => {
  const m = new SyncExpanseStrMap();
  assert.strictEqual(m.first(), null);
  assert.strictEqual(m.last(), null);
  const keys = ['b', 'a', 'ab', 'abc', 'zz', 'é', 'q'.repeat(40)];
  keys.forEach((k, i) => m.set(k, BigInt(i)));
  const sorted = [...keys].sort((x, y) => Buffer.compare(Buffer.from(x), Buffer.from(y)));
  assert.strictEqual(m.first().key, sorted[0]);
  assert.strictEqual(m.last().key, sorted[sorted.length - 1]);
  assert.strictEqual(m.first().value, BigInt(keys.indexOf(sorted[0])));
  // Walk forward and backward through every key.
  let walked = [];
  for (let e = m.first(); e; e = m.next(e.key)) walked.push(e.key);
  assert.deepStrictEqual(walked, sorted);
  walked = [];
  for (let e = m.last(); e; e = m.prev(e.key)) walked.push(e.key);
  assert.deepStrictEqual(walked, [...sorted].reverse());
  // Inclusive vs exclusive, including a probe that is not a key.
  assert.strictEqual(m.next('ab').key, 'abc');
  assert.strictEqual(m.next('ab', true).key, 'ab');
  assert.strictEqual(m.prev('ab').key, 'a');
  assert.strictEqual(m.prev('ab', true).key, 'ab');
  assert.strictEqual(m.next('aa').key, 'ab');
  assert.strictEqual(m.prev('aa', true).key, 'a');
  assert.strictEqual(m.next(sorted[sorted.length - 1]), null);
  assert.strictEqual(m.prev(sorted[0]), null);
});

test('SyncExpanseBytesMap keys are bytes: NUL, empty and non-UTF-8 are distinct keys', () => {
  const m = new SyncExpanseBytesMap();
  const ks = [Buffer.alloc(0), Buffer.from([0]), Buffer.from([0, 0]), Buffer.from([0xff, 0xfe]), Buffer.from('a\0b')];
  ks.forEach((k, i) => assert.strictEqual(m.set(k, BigInt(i)), null));
  assert.strictEqual(m.size(), BigInt(ks.length));
  ks.forEach((k, i) => assert.strictEqual(m.get(k), BigInt(i)));
  // Buffer, Uint8Array and string spell the same key.
  assert.strictEqual(m.get(new Uint8Array([0, 0])), 2n);
  assert.strictEqual(m.get('a\0b'), 4n);
  assert.strictEqual(m.has(Buffer.from([0, 0, 0])), false);
});

test('SyncExpanseOrderedBytesMap ordered reads (they block writers) and key decode', () => {
  const m = new SyncExpanseOrderedBytesMap();
  assert.strictEqual(m.first(), null);
  assert.strictEqual(m.last(), null);
  // Keys with the bytes 0x00 and 0x01 are escaped by the engine; order and decode must survive.
  const ks = [
    Buffer.alloc(0),
    Buffer.from([0]),
    Buffer.from([0, 0]),
    Buffer.from([0, 1]),
    Buffer.from([1]),
    Buffer.from([1, 0]),
    Buffer.from('a'),
    Buffer.from('a\0b'),
    Buffer.from('ab'),
    Buffer.from([0xff]),
  ];
  const shuffled = [...ks].reverse();
  shuffled.forEach((k, i) => m.set(k, BigInt(i)));
  const sorted = [...ks].sort(Buffer.compare);
  let walked = [];
  for (let e = m.first(); e; e = m.next(e.key)) walked.push(e.key);
  assert.deepStrictEqual(walked, sorted);
  walked = [];
  for (let e = m.last(); e; e = m.prev(e.key)) walked.push(e.key);
  assert.deepStrictEqual(walked, [...sorted].reverse());
  assert.deepStrictEqual(m.next(Buffer.from([0]), true).key, Buffer.from([0]));
  assert.deepStrictEqual(m.next(Buffer.from([0])).key, Buffer.from([0, 0]));
  assert.deepStrictEqual(m.prev(Buffer.from([1]), true).key, Buffer.from([1]));
  assert.deepStrictEqual(m.prev(Buffer.from([1])).key, Buffer.from([0, 1]));
  assert.strictEqual(m.next(Buffer.from([0xff])), null);
  assert.strictEqual(m.prev(Buffer.alloc(0)), null);
  // Point reads agree with the ordered ones on the escaped keys.
  assert.strictEqual(m.get(Buffer.from([0, 1])), BigInt(shuffled.findIndex((k) => k.equals(Buffer.from([0, 1])))));
});

test('SyncExpanseBlobMap point operations across payload sizes', () => {
  const m = new SyncExpanseBlobMap();
  assert.strictEqual(m.isEmpty(), true);
  const sizes = [0, 1, 7, 8, 100, 4096, 1 << 20]; // inline, boundary, arena, a chunk-sized payload
  sizes.forEach((n, i) => {
    const payload = Buffer.alloc(n, i + 1);
    m.set(i, payload, i < 3 ? 0 : 7);
    const got = m.get(i);
    assert(Buffer.isBuffer(got));
    assert(got.equals(payload), `payload of ${n} bytes round-trips`);
  });
  assert.strictEqual(m.size(), BigInt(sizes.length));
  assert.strictEqual(m.get(999), null);
  assert.strictEqual(m.has(2), true);
  assert.strictEqual(m.contains(999), false);
  // Metadata: kept at every payload length (#1548); a short payload written with
  // metadata 0 stays inline.
  assert.strictEqual(m.getMeta(4), 7);
  assert.strictEqual(m.getMeta(2), 0);
  assert.strictEqual(m.getMeta(999), null);
  const arena = m.getWithMeta(4);
  assert.strictEqual(arena.hotMeta, 7);
  assert.strictEqual(arena.isInline, false);
  assert(arena.payload.equals(Buffer.alloc(100, 5)));
  const inline = m.getWithMeta(2);
  assert.strictEqual(inline.isInline, true);
  assert.strictEqual(inline.hotMeta, 0);
  m.set(50, Buffer.from('short'), 9); // a short payload with metadata: an arena record keeping it
  assert.strictEqual(m.getMeta(50), 9);
  assert.strictEqual(m.getWithMeta(50).isInline, false);
  assert.throws(() => m.set(51, Buffer.from('short'), 1 << 24)); // over 24 bits, at any length
  assert.strictEqual(m.has(51), false);
  m.set(51, 'a string payload longer than seven', 9);
  assert.strictEqual(m.get(51).toString(), 'a string payload longer than seven');
  m.set(52, new Uint8Array([1, 2, 3]));
  assert.deepStrictEqual([...m.get(52)], [1, 2, 3]);
  assert.throws(() => m.set(53, Buffer.alloc(100), 1 << 24));
  assert.strictEqual(m.has(53), false);
  // Replace, delete, clear.
  m.set(4, Buffer.from('replaced with a payload longer than seven'), 3);
  assert.strictEqual(m.get(4).toString(), 'replaced with a payload longer than seven');
  assert.strictEqual(m.delete(4), true);
  assert.strictEqual(m.delete(4), false);
  assert.strictEqual(m.get(4), null);
  assert(m.memUsed() > 0n);
  assert(m.memUsedEstimate() > 0n);
  assert(m.memHeld() >= 0n);
  assert.strictEqual(m.memHeldEstimate(), m.memHeld());
  m.shrinkToFit();
  m.clear();
  assert.strictEqual(m.size(), 0n);
  assert.strictEqual(m.get(0), null);
});

test('SyncExpanseBlobMap constructor options, reclaim switch and compact', () => {
  const m = new SyncExpanseBlobMap(4096, 1 << 20);
  assert.strictEqual(m.reclaimAtCap(), true);
  m.setReclaimAtCap(false);
  assert.strictEqual(m.reclaimAtCap(), false);
  m.setReclaimAtCap(true);
  assert.throws(() => new SyncExpanseBlobMap(4096, -1), /negative/i);
  for (let i = 0; i < 200; i++) m.set(i, Buffer.alloc(100, 1));
  for (let i = 0; i < 200; i += 2) m.delete(i);
  const stats = m.compact();
  assert(stats.liveBytesAfter <= stats.liveBytesBefore);
  assert(stats.totalAllocatedAfter <= stats.totalAllocatedBefore);
  for (let i = 1; i < 200; i += 2) assert(m.get(i).equals(Buffer.alloc(100, 1)));
  assert.strictEqual(m.size(), 100n);
});

test('SyncExpanseBlobMap compareExchange covers every branch', () => {
  const m = new SyncExpanseBlobMap();
  const big = (c) => Buffer.alloc(64, c);
  const v = (c, hotMeta) => ({ payload: big(c), hotMeta });
  const seen = (r) => (r.observed === null ? null : { p: r.observed.payload.toString('hex'), h: r.observed.hotMeta });
  const exp = (c, h) => ({ p: big(c).toString('hex'), h });
  assert.deepStrictEqual(m.compareExchange(1, null, null), { stored: true, observed: null });
  assert.strictEqual(m.has(1), false);
  // insert if absent
  let r = m.compareExchange(1, null, v(1, 5));
  assert.strictEqual(r.stored, true);
  assert.strictEqual(r.observed, null);
  assert.strictEqual(m.getMeta(1), 5);
  // insert against a present key: fails and reports what is there
  r = m.compareExchange(1, null, v(2, 6));
  assert.strictEqual(r.stored, false);
  assert.deepStrictEqual(seen(r), exp(1, 5));
  // absent == absent against a present key
  r = m.compareExchange(1, undefined, undefined);
  assert.strictEqual(r.stored, false);
  // payload matches, metadata does not
  r = m.compareExchange(1, v(1, 4), v(3, 7));
  assert.strictEqual(r.stored, false);
  assert.deepStrictEqual(seen(r), exp(1, 5));
  // metadata matches, payload does not
  r = m.compareExchange(1, v(9, 5), v(3, 7));
  assert.strictEqual(r.stored, false);
  // expected present against an absent key
  r = m.compareExchange(2, v(1, 0), v(1, 0));
  assert.deepStrictEqual({ stored: r.stored, observed: r.observed }, { stored: false, observed: null });
  // replace on match; hotMeta defaults to 0 when omitted
  r = m.compareExchange(1, v(1, 5), { payload: big(3) });
  assert.strictEqual(r.stored, true);
  assert.deepStrictEqual(seen(r), exp(1, 5));
  assert(m.get(1).equals(big(3)));
  assert.strictEqual(m.getMeta(1), 0);
  // remove if equal
  r = m.compareExchange(1, v(4, 0), null);
  assert.strictEqual(r.stored, false);
  r = m.compareExchange(1, { payload: big(3) }, null);
  assert.strictEqual(r.stored, true);
  assert.strictEqual(m.has(1), false);
  // an unstorable replacement throws and leaves the map unchanged
  m.set(3, big(1), 2);
  assert.throws(() => m.compareExchange(3, v(1, 2), v(5, 1 << 24)));
  assert(m.get(3).equals(big(1)));
  assert.strictEqual(m.getMeta(3), 2);
  // a retry loop that stops on a refusal ends
  let expected = v(1, 2);
  let outcome = 'running';
  for (let i = 0; i < 4 && outcome === 'running'; i++) {
    try {
      const x = m.compareExchange(3, expected, v(6, 1 << 24));
      expected = x.observed;
    } catch (_) {
      outcome = 'refused';
    }
  }
  assert.strictEqual(outcome, 'refused');
});

test('SyncExpanseBlobMap compareExchangeMeta and short payloads with metadata (#1548)', () => {
  const m = new SyncExpanseBlobMap();
  assert.deepStrictEqual(m.compareExchangeMeta(1, null, null), { stored: true, observed: null });
  // metadata alone cannot create a key
  assert.throws(() => m.compareExchangeMeta(1, null, 5));
  assert.strictEqual(m.has(1), false);
  m.set(1, Buffer.alloc(64, 1), 5);
  assert.deepStrictEqual(m.compareExchangeMeta(1, 4, 6), { stored: false, observed: 5 });
  assert.deepStrictEqual(m.compareExchangeMeta(1, 5, 6), { stored: true, observed: 5 });
  assert.strictEqual(m.getMeta(1), 6);
  assert(m.get(1).equals(Buffer.alloc(64, 1))); // the payload is untouched
  assert.deepStrictEqual(m.compareExchangeMeta(1, null, 7), { stored: false, observed: 6 });
  assert.throws(() => m.compareExchangeMeta(1, 6, 1 << 24)); // over 24 bits
  assert.strictEqual(m.getMeta(1), 6);
  assert.deepStrictEqual(m.compareExchangeMeta(1, 6, null), { stored: true, observed: 6 }); // removes
  assert.strictEqual(m.has(1), false);
  // a payload of 7 bytes or fewer keeps the metadata it was written with
  m.set(2, Buffer.from('tiny'), 9);
  assert.strictEqual(m.getMeta(2), 9);
  assert.deepStrictEqual(m.compareExchangeMeta(2, 0, 1), { stored: false, observed: 9 });
  assert.deepStrictEqual(m.compareExchangeMeta(2, 9, 1), { stored: true, observed: 9 });
  assert.strictEqual(m.getMeta(2), 1);
  // written with metadata 0 it is stored inline, with no metadata field to write
  m.set(3, Buffer.from('tiny'), 0);
  assert.strictEqual(m.getMeta(3), 0);
  assert.throws(() => m.compareExchangeMeta(3, 0, 1)); // no metadata field to hold it
  assert.deepStrictEqual(m.compareExchangeMeta(3, 0, 0), { stored: true, observed: 0 });
});

test('SyncExpanseBlobMap getOrInsert stores only into an absent key', () => {
  const m = new SyncExpanseBlobMap();
  assert.strictEqual(m.getOrInsert(1, Buffer.alloc(64, 1), 3), null); // stored
  assert(m.get(1).equals(Buffer.alloc(64, 1)));
  assert.strictEqual(m.getMeta(1), 3);
  const seen = m.getOrInsert(1, Buffer.alloc(64, 2), 4);
  assert(seen.payload.equals(Buffer.alloc(64, 1)));
  assert.strictEqual(seen.hotMeta, 3);
  assert(m.get(1).equals(Buffer.alloc(64, 1))); // untouched
  assert.strictEqual(m.getOrInsert(2, Buffer.alloc(0)), null); // empty is a value, not absence
  assert.strictEqual(m.getOrInsert(2, 'x').payload.length, 0);
  assert.strictEqual(m.getOrInsert(3, 'small', 0), null);
  assert.strictEqual(m.getOrInsert(3, 'other').payload.toString(), 'small');
  assert.throws(() => m.getOrInsert(4, Buffer.alloc(64), 1 << 24));
  assert.strictEqual(m.has(4), false);
});

test('SyncExpanseBlobMap retry loop over compareExchangeMeta counts exactly', () => {
  const m = new SyncExpanseBlobMap();
  m.set(1, Buffer.alloc(64, 1), 0);
  for (let i = 0; i < 5000; i++) {
    let expected = m.getMeta(1);
    for (;;) {
      const r = m.compareExchangeMeta(1, expected, expected + 1);
      if (r.stored) break;
      expected = r.observed;
    }
  }
  assert.strictEqual(m.getMeta(1), 5000);
  // an interleaved write between the read and the exchange makes the exchange lose
  const read = m.getMeta(1);
  m.set(1, Buffer.alloc(64, 1), read + 10);
  const lost = m.compareExchangeMeta(1, read, read + 1);
  assert.deepStrictEqual(lost, { stored: false, observed: read + 10 });
});

console.log('\nAll 11 test suites passed successfully! 🎉');
