# @orieg/expanse (expanse-node)

> Native Node.js, Bun, and Deno bindings for **Expanse**: clean-room, pure-Rust Judy arrays and 256-ary digital tries with optimistic concurrency control (OCC).

[![npm version](https://img.shields.io/npm/v/@orieg/expanse.svg)](https://www.npmjs.com/package/@orieg/expanse)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://github.com/orieg/expanse/blob/main/LICENSE-MIT)

## Overview

`@orieg/expanse` provides drop-in, zero-dependency, ultra-fast sparse data structures for JavaScript and TypeScript runtimes:

- **`ExpanseSet`**: 64-bit unsigned integer set (compat: Judy1) with rank/select and range queries.
- **`ExpanseMap`**: 64-bit integer key/value map (compat: JudyL).
- **`ExpanseStrMap`**: Byte-lexicographically sorted string map (compat: JudySL) with prefix compression.
- **`ExpanseBytesMap`**: Unordered map over arbitrary binary keys (including `\0` bytes) (compat: JudyHS).
- **`ExpanseBlobMap`**: Polymorphic blob storage with inline value packing (≤ 7 bytes stored inline with 0 heap allocations), chunked slab arenas, in-place compaction, and mmap image persistence.
- **`SyncExpanseMap` / `SyncExpanseSet`**: Thread-safe concurrent structures with optimistic concurrency control (OCC) and epoch-based memory reclamation.

## Installation

```bash
npm install @orieg/expanse
# or
bun add @orieg/expanse
# or
pnpm add @orieg/expanse
```

## Quick Start

### 64-bit Integer Set (`ExpanseSet`)

```typescript
import { ExpanseSet } from '@orieg/expanse';

const set = new ExpanseSet();
set.add(42n);
set.add(100n);
set.add(500n);

console.log(set.has(42n)); // true
console.log(set.size());   // 3n

// Rank / Select
console.log(set.rank(100n)); // 1n (1 element strictly < 100)
console.log(set.select(0n));  // 42n (0-th element)

// Navigation
console.log(set.next(42n));  // 100n
console.log(set.prev(500n)); // 100n

// Range queries
console.log(set.countRange(40n, 200n)); // 2n
console.log(set.range(40n, 200n));      // [42n, 100n]
```

### 64-bit Integer Map (`ExpanseMap`)

```typescript
import { ExpanseMap } from '@orieg/expanse';

const map = new ExpanseMap();
map.set(100n, 9999n);
map.set(200n, 8888n);

console.log(map.get(100n)); // 9999n
console.log(map.first());   // { key: 100n, value: 9999n }
```

### Prefix-Compressed String Map (`ExpanseStrMap`)

```typescript
import { ExpanseStrMap } from '@orieg/expanse';

const strmap = new ExpanseStrMap();
strmap.set('apple', 1n);
strmap.set('banana', 2n);
strmap.set('cherry', 3n);

console.log(strmap.get('banana')); // 2n
console.log(strmap.first());        // { key: 'apple', value: 1n }
console.log(strmap.next('apple'));  // { key: 'banana', value: 2n }
```

### Arbitrary Binary Map (`ExpanseBytesMap`)

```typescript
import { ExpanseBytesMap } from '@orieg/expanse';

const bytesmap = new ExpanseBytesMap();
const binaryKey = Buffer.from([0x00, 0x01, 0x02, 0xff]);
bytesmap.set(binaryKey, 42n);

console.log(bytesmap.get(binaryKey)); // 42n
```

### High-Performance Blob Map (`ExpanseBlobMap`)

```typescript
import { ExpanseBlobMap } from '@orieg/expanse';

const blobmap = new ExpanseBlobMap();

// Short payloads (<= 7 bytes) are stored inline in the value slot (0 heap allocations)
blobmap.set(1n, Buffer.from('inline'), 10 /* 32-bit hot metadata */);

// Larger payloads are allocated in contiguous slab arenas
blobmap.set(2n, Buffer.from('large payload allocated in slab arena'), 20);

// Lookup with metadata
const res = blobmap.getWithMeta(1n);
console.log(res);
// { payload: <Buffer 69 6e 6c 69 6e 65>, hotMeta: 10, isInline: true }

// Prune based on hot metadata
blobmap.prune((key, hotMeta) => hotMeta > 15);

// Relocatable binary image persistence
blobmap.saveImage('./data.expanse');
const reloaded = ExpanseBlobMap.openImage('./data.expanse');
```

### Concurrent Structures (`SyncExpanseMap` / `SyncExpanseSet`)

```typescript
import { SyncExpanseMap, SyncExpanseSet } from '@orieg/expanse';

const syncMap = new SyncExpanseMap();
syncMap.set(1000n, 42n);
console.log(syncMap.get(1000n)); // 42n
```

## License

Dual-licensed under MIT and Apache-2.0.
