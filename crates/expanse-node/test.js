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
  ExpanseBlobMap,
  SyncExpanseMap,
  SyncExpanseSet,
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

// 5. ExpanseBlobMap Tests
console.log('\n--- ExpanseBlobMap Tests ---');

test('ExpanseBlobMap inline and arena payloads, metadata, prune, compact, and serialization', () => {
  const blobmap = new ExpanseBlobMap();

  // Short inline payload (<= 7 bytes)
  const shortBuf = Buffer.from('inline');
  blobmap.set(1n, shortBuf, 10);

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
  assert.strictEqual(syncMap.countRange(150n, 350n), 2n);

  assert.strictEqual(syncMap.delete(200n), true);
  assert.strictEqual(syncMap.size(), 2n);
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

console.log('\nAll 11 test suites passed successfully! 🎉');
