/**
 * @orieg/expanse — Clean-room, pure-Rust modern Judy arrays and digital tries for Node.js, Bun, and Deno.
 */

export type KeyInput = bigint | number;
export type BytesInput = Buffer | Uint8Array | string;

/**
 * Key-value pair with 64-bit unsigned integer key and value.
 */
export interface MapEntry {
  key: bigint;
  value: bigint;
}

/** The outcome of `SyncExpanseMap.compareExchange`. */
export interface CompareExchangeResult {
  /** `true` when the new value was stored. */
  stored: boolean;
  /** The value the compare saw, or `null` if the key was absent. */
  observed: bigint | null;
}

/**
 * Key-value pair with string key and 64-bit unsigned integer value.
 */
export interface StrMapEntry {
  key: string;
  value: bigint;
}

/**
 * Key-value pair with byte Buffer key and 64-bit unsigned integer value.
 */
export interface BytesMapEntry {
  key: Buffer;
  value: bigint;
}

/**
 * Metadata and payload returned by BlobMap lookup.
 */
export interface BlobMetaResult {
  /** The blob payload bytes as a Node Buffer. */
  payload: Buffer;
  /** 32-bit hot metadata word stored directly in the trie index. */
  hotMeta: number;
  /** True if the payload was stored inline in the 64-bit value slot (≤ 7 bytes, written with `hotMeta` 0). */
  isInline: boolean;
}

/**
 * Compaction statistics returned by BlobMap in-place garbage collection.
 */
export interface BlobArenaStatsResult {
  /** Live payload bytes plus an 8-byte header per record. */
  liveBytes: bigint;
  /** Allocated chunk bytes, dead and live: what the capacity cap counts. */
  allocatedBytes: bigint;
  /** The capacity cap, as clamped. */
  maxCapacity: bigint;
  /** The arena's chunk size. */
  chunkSize: bigint;
  /** Whether an insert the cap refuses may compact the arena. */
  reclaimAtCap: boolean;
}

/** Status returned by `ExpanseBlobMap.setStatus`. */
export type BlobInsertStatus =
  | 'ok'
  | 'meta_overflow'
  | 'allocation_failed'
  | 'cap_refused'
  | 'arena_full'
  | 'error';

export interface CompactionStatsResult {
  /** Live payload bytes before compaction. */
  liveBytesBefore: bigint;
  /** Live payload bytes after compaction. */
  liveBytesAfter: bigint;
  /** Total arena bytes allocated before compaction. */
  totalAllocatedBefore: bigint;
  /** Total arena bytes allocated after compaction. */
  totalAllocatedAfter: bigint;
}

/**
 * A sparse, dynamic 64-bit unsigned integer set (compat: Judy1).
 */
export class ExpanseSet {
  /**
   * Creates an empty set, optionally initialized from an array of keys.
   */
  constructor(keys?: KeyInput[]);

  /**
   * Number of elements in the set.
   */
  size(): bigint;

  /**
   * Returns true if the set contains no elements.
   */
  isEmpty(): boolean;

  /**
   * Checks whether `key` is present in the set.
   */
  has(key: KeyInput): boolean;

  /**
   * Inserts `key` into the set. Returns true if newly inserted, false if already present.
   */
  add(key: KeyInput): boolean;

  /**
   * Removes `key` from the set. Returns true if present, false otherwise.
   */
  remove(key: KeyInput): boolean;

  /**
   * Removes all elements from the set.
   */
  clear(): void;

  /**
   * Returns heap bytes used by trie node allocations.
   */
  memUsed(): bigint;

  /**
   * Returns heap bytes held from the system allocator: memUsed() plus
   * freed blocks kept for reuse and unused slab space.
   */
  memHeld(): bigint;

  /**
   * Returns retained freed blocks to the system allocator and returns the
   * bytes released. Nothing moves; memUsed() is unchanged.
   */
  shrinkToFit(): bigint;

  /**
   * Returns the smallest element in the set, or null if empty.
   */
  first(): bigint | null;

  /**
   * Returns the largest element in the set, or null if empty.
   */
  last(): bigint | null;

  /**
   * Returns the smallest element strictly > key (or >= key if inclusive is true), or null.
   */
  next(key: KeyInput, inclusive?: boolean): bigint | null;

  /**
   * Returns the largest element strictly < key (or <= key if inclusive is true), or null.
   */
  prev(key: KeyInput, inclusive?: boolean): bigint | null;

  /**
   * Returns the number of keys strictly below `key` (0-based rank).
   */
  rank(key: KeyInput): bigint;

  /**
   * Selects the element with `k` keys below it (0-based select), or null if out of bounds.
   */
  select(k: KeyInput): bigint | null;

  /**
   * Counts the number of keys in the closed range `[start, end]`.
   */
  countRange(start: KeyInput, end: KeyInput): bigint;

  /**
   * Scans keys in the range `[start, end]`.
   */
  range(start?: KeyInput, end?: KeyInput, inclusive?: boolean): bigint[];

  /**
   * Returns all keys in ascending order as an array of BigInts.
   */
  toArray(): bigint[];

  /**
   * Ingests an array of integer keys. Returns the count of newly inserted keys.
   */
  insertMany(keys: KeyInput[]): number;
}

/**
 * A sparse, dynamic 64-bit unsigned integer key/value map (compat: JudyL).
 */
export class ExpanseMap {
  constructor();

  /**
   * Number of entries in the map.
   */
  size(): bigint;

  /**
   * Returns true if the map contains no entries.
   */
  isEmpty(): boolean;

  /**
   * Checks whether `key` is present in the map.
   */
  has(key: KeyInput): boolean;

  /**
   * Sets `map[key] = value`. Returns the previous value as BigInt if present, or null.
   */
  set(key: KeyInput, value: KeyInput): bigint | null;

  /**
   * Retrieves the value for `key`, or null if absent.
   */
  get(key: KeyInput): bigint | null;

  /**
   * Deletes `key` from the map. Returns true if present, false otherwise.
   */
  delete(key: KeyInput): boolean;

  /**
   * Removes all entries from the map.
   */
  clear(): void;

  /**
   * Returns heap bytes used by trie node allocations.
   */
  memUsed(): bigint;

  /**
   * Returns heap bytes held from the system allocator: memUsed() plus
   * freed blocks kept for reuse and unused slab space.
   */
  memHeld(): bigint;

  /**
   * Returns retained freed blocks to the system allocator and returns the
   * bytes released. Nothing moves; memUsed() is unchanged.
   */
  shrinkToFit(): bigint;

  /**
   * Smallest entry (key, value) in the map, or null if empty.
   */
  first(): MapEntry | null;

  /**
   * Largest entry (key, value) in the map, or null if empty.
   */
  last(): MapEntry | null;

  /**
   * Smallest entry with key > key (or >= key if inclusive is true), or null.
   */
  next(key: KeyInput, inclusive?: boolean): MapEntry | null;

  /**
   * Largest entry with key < key (or <= key if inclusive is true), or null.
   */
  prev(key: KeyInput, inclusive?: boolean): MapEntry | null;

  /**
   * Number of keys strictly below `key` (0-based rank).
   */
  rank(key: KeyInput): bigint;

  /**
   * Entry with `k` keys below it (0-based select), or null if out of bounds.
   */
  select(k: KeyInput): MapEntry | null;

  /**
   * Counts the number of keys in the closed range `[start, end]`.
   */
  countRange(start: KeyInput, end: KeyInput): bigint;

  /**
   * Returns all keys in ascending order.
   */
  keys(): bigint[];

  /**
   * Returns all values in key-ascending order.
   */
  values(): bigint[];

  /**
   * Returns all key-value entries in ascending order.
   */
  entries(): MapEntry[];

  /**
   * Scans entries in the range `[start, end]`.
   */
  range(start?: KeyInput, end?: KeyInput, inclusive?: boolean): MapEntry[];
}

/**
 * A sorted map from NUL-free UTF-8 strings to 64-bit unsigned integers (compat: JudySL).
 */
export class ExpanseStrMap {
  constructor();

  /**
   * Number of strings in the map.
   */
  size(): bigint;

  /**
   * Returns true if the map contains no strings.
   */
  isEmpty(): boolean;

  /**
   * Checks whether `key` is present in the map.
   */
  has(key: string): boolean;

  /**
   * Sets `map[key] = value`. Returns previous value if present, or null.
   */
  set(key: string, value: KeyInput): bigint | null;

  /**
   * Retrieves value for `key`, or null if absent.
   */
  get(key: string): bigint | null;

  /**
   * Deletes `key` from the map. Returns true if present, false otherwise.
   */
  delete(key: string): boolean;

  /**
   * Removes all strings and frees trie nodes.
   */
  clear(): void;

  /**
   * Heap bytes used by the prefix trie.
   */
  memUsed(): bigint;

  /**
   * Returns heap bytes held from the system allocator: memUsed() plus
   * freed blocks kept for reuse and unused slab space.
   */
  memHeld(): bigint;

  /**
   * Returns retained freed blocks to the system allocator and returns the
   * bytes released. Nothing moves; memUsed() is unchanged.
   */
  shrinkToFit(): bigint;

  /**
   * Smallest entry in byte-lexicographical order, or null if empty.
   */
  first(): StrMapEntry | null;

  /**
   * Largest entry in byte-lexicographical order, or null if empty.
   */
  last(): StrMapEntry | null;

  /**
   * Smallest entry with key > key (or >= key if inclusive is true), or null.
   */
  next(key: string, inclusive?: boolean): StrMapEntry | null;

  /**
   * Largest entry with key < key (or <= key if inclusive is true), or null.
   */
  prev(key: string, inclusive?: boolean): StrMapEntry | null;

  /**
   * Returns all keys in byte-lexicographical order.
   */
  keys(): string[];

  /**
   * Returns all values in key order.
   */
  values(): bigint[];

  /**
   * Returns all key-value entries in byte-lexicographical order.
   */
  entries(): StrMapEntry[];
}

/**
 * A sparse, dynamic map from arbitrary byte keys (including NUL bytes) to 64-bit unsigned integers (compat: JudyHS).
 */
export class ExpanseBytesMap {
  constructor();

  /**
   * Number of entries stored in the map.
   */
  size(): bigint;

  /**
   * Returns true if the map is empty.
   */
  isEmpty(): boolean;

  /**
   * Checks whether `key` exists in the map.
   */
  has(key: BytesInput): boolean;

  /**
   * Sets `map[key] = value`. Returns previous value if present, or null.
   */
  set(key: BytesInput, value: KeyInput): bigint | null;

  /**
   * Retrieves value for `key`, or null if absent.
   */
  get(key: BytesInput): bigint | null;

  /**
   * Deletes `key` from the map. Returns true if present, false otherwise.
   */
  delete(key: BytesInput): boolean;

  /**
   * Removes all entries and releases memory.
   */
  clear(): void;

  /**
   * Heap bytes used by hash trie and buckets.
   */
  memUsed(): bigint;

  /**
   * Returns all byte keys as Node Buffers.
   */
  keys(): Buffer[];

  /**
   * Returns all values.
   */
  values(): bigint[];

  /**
   * Returns all key-value entries.
   */
  entries(): BytesMapEntry[];
}

/**
 * An ordered map from arbitrary byte keys (any length, including empty, `0x00` and `0xFF`) to 64-bit unsigned integers.
 * Keys are ordered as unsigned bytes, lexicographically; a key sorts before any longer key it prefixes.
 */
export class ExpanseOrderedBytesMap {
  constructor();

  /**
   * Number of entries stored in the map.
   */
  size(): bigint;

  /**
   * Returns true if the map is empty.
   */
  isEmpty(): boolean;

  /**
   * Checks whether `key` exists in the map.
   */
  has(key: BytesInput): boolean;

  /**
   * Sets `map[key] = value`. Returns the previous value if present, or null.
   */
  set(key: BytesInput, value: KeyInput): bigint | null;

  /**
   * Retrieves the value for `key`, or null if absent.
   */
  get(key: BytesInput): bigint | null;

  /**
   * Returns the value for `key`, inserting it with value 0 first if absent. An existing value is kept.
   */
  insertSlot(key: BytesInput): bigint;

  /**
   * Deletes `key` from the map. Returns true if present, false otherwise.
   */
  delete(key: BytesInput): boolean;

  /**
   * Removes all entries and releases memory.
   */
  clear(): void;

  /**
   * Heap bytes used by the trie nodes and value slots.
   */
  memUsed(): bigint;

  /**
   * Heap bytes held by the map, including spare capacity.
   */
  memHeld(): bigint;

  /**
   * Releases spare capacity and returns the number of bytes released.
   */
  shrinkToFit(): bigint;

  /**
   * Smallest key and its value, or null if the map is empty. The key is a Buffer, never truncated.
   */
  first(): BytesMapEntry | null;

  /**
   * Largest key and its value, or null if the map is empty.
   */
  last(): BytesMapEntry | null;

  /**
   * Smallest entry whose key is `>= key`, or null if none.
   */
  ceiling(key: BytesInput): BytesMapEntry | null;

  /**
   * Smallest entry whose key is `> key`, or null if none.
   */
  higher(key: BytesInput): BytesMapEntry | null;

  /**
   * Largest entry whose key is `<= key`, or null if none.
   */
  floor(key: BytesInput): BytesMapEntry | null;

  /**
   * Largest entry whose key is `< key`, or null if none.
   */
  lower(key: BytesInput): BytesMapEntry | null;
}

/**
 * A high-performance map from 64-bit integer keys to arbitrary-length byte payloads
 * backed by inline polymorphic 64-bit value slots and chunked slab arenas.
 */
export class ExpanseBlobMap {
  /**
   * Creates an empty blob map, optionally with custom arena chunk size in bytes
   * and an arena capacity cap `maxCapacity` in bytes of allocated chunks
   * (default 1 GiB; clamped to `[chunkSize, 64 GiB]`).
   */
  constructor(chunkSize?: number, maxCapacity?: number);

  /**
   * Turns the reclaim at the capacity cap on (the default) or off. Off, an
   * insert the cap refuses compacts nothing.
   */
  setReclaimAtCap(on: boolean): void;

  /**
   * Whether an insert the capacity cap refuses may compact the arena.
   */
  reclaimAtCap(): boolean;

  /**
   * Arena accounting: live bytes, allocated chunk bytes, the cap, the chunk size and the switch.
   */
  arenaStats(): BlobArenaStatsResult;

  /**
   * Inserts like `set`, but returns a status instead of throwing on a refusal.
   * `'cap_refused'`: the capacity cap refused a chunk and nothing was compacted.
   * `'arena_full'`: this insert compacted and the record still does not fit.
   */
  setStatus(key: KeyInput, payload: BytesInput, hotMeta?: number): BlobInsertStatus;

  /**
   * Number of entries in the blob map.
   */
  size(): bigint;

  /**
   * Returns true if the map contains no entries.
   */
  isEmpty(): boolean;

  /**
   * Checks whether `key` exists in the map.
   */
  has(key: KeyInput): boolean;

  /**
   * Inserts a key-blob pair with optional 32-bit hot metadata.
   */
  set(key: KeyInput, payload: BytesInput, hotMeta?: number): void;

  /**
   * Retrieves only the byte payload for `key`, or null if absent.
   */
  get(key: KeyInput): Buffer | null;

  /**
   * Retrieves `{ payload: Buffer, hotMeta: number, isInline: boolean }` for `key`, or null if absent.
   */
  getWithMeta(key: KeyInput): BlobMetaResult | null;

  /**
   * Deletes `key` from the map. Returns true if present, false otherwise.
   */
  delete(key: KeyInput): boolean;

  /**
   * Removes all entries and resets the slab arena.
   */
  clear(): void;

  /**
   * Returns total heap memory used by index and slab arena.
   */
  memUsed(): bigint;

  /**
   * Evaluates `predicate(key, hotMeta)` and deletes matching keys. Returns count of pruned entries.
   */
  prune(predicate: (key: bigint, hotMeta: number) => boolean): number;

  /**
   * Runs in-place garbage collection and compaction, returning memory statistics.
   */
  compact(): CompactionStatsResult;

  /**
   * Saves the map to a relocatable binary image file. Returns the number of bytes
   * written as a BigInt (an image can exceed 4 GiB, so it does not fit in a 32-bit number).
   */
  saveImage(path: string): bigint;

  /**
   * Loads a map from a relocatable binary image file.
   *
   * The whole file is read into memory and the index is rebuilt; the image is NOT
   * memory-mapped (no lazy-fault SIGBUS hazard, at the cost of the image being resident).
   */
  static openImage(path: string): ExpanseBlobMap;
}

/**
 * A thread-safe concurrent 64-bit integer map with optimistic concurrency control (OCC).
 */
export class SyncExpanseMap {
  constructor();

  size(): bigint;
  isEmpty(): boolean;
  memUsed(): bigint;
  /**
   * Heap bytes held from the global allocator: the tree's own share plus
   * the blocks its epoch collector keeps for reuse or is waiting to reclaim.
   */
  memHeld(): bigint;
  /**
   * Returns the collector's freed blocks to the global allocator and returns
   * the bytes released; memHeld() falls by exactly that much.
   */
  shrinkToFit(): bigint;
  has(key: KeyInput): boolean;
  set(key: KeyInput, value: KeyInput): bigint | null;
  get(key: KeyInput): bigint | null;
  /**
   * Atomic compare-and-swap: stores `newValue` only if the key's current value is
   * `expected`. `null` is "absent" on both sides. `observed` is the value the compare
   * saw, equal to `expected` when `stored`.
   */
  compareExchange(
    key: KeyInput,
    expected: KeyInput | null,
    newValue: KeyInput | null,
  ): CompareExchangeResult;
  /** Stores `value` only if `key` is absent; returns the existing value, or `null` if stored. */
  getOrInsert(key: KeyInput, value: KeyInput): bigint | null;
  /**
   * Retry loop over `compareExchange`: stores `fn(current)` (`null` removes the key)
   * and returns the replaced value. `fn` may run more than once per call and must be
   * free of side effects; an exception from `fn` propagates with the key unchanged.
   */
  update(key: KeyInput, fn: (current: bigint | null) => KeyInput | null): bigint | null;
  delete(key: KeyInput): boolean;
  clear(): void;
  first(): MapEntry | null;
  last(): MapEntry | null;
  next(key: KeyInput, inclusive?: boolean): MapEntry | null;
  prev(key: KeyInput, inclusive?: boolean): MapEntry | null;
  rank(key: KeyInput): bigint;
  select(k: KeyInput): MapEntry | null;
  countRange(start: KeyInput, end: KeyInput): bigint;
  keys(): bigint[];
  values(): bigint[];
  entries(): MapEntry[];
}

/**
 * A thread-safe concurrent 64-bit integer set with optimistic concurrency control (OCC).
 */
export class SyncExpanseSet {
  constructor();

  size(): bigint;
  isEmpty(): boolean;
  /**
   * Heap bytes held from the global allocator: the tree's own share plus
   * the blocks its epoch collector keeps for reuse or is waiting to reclaim.
   */
  memHeld(): bigint;
  /**
   * Returns the collector's freed blocks to the global allocator and returns
   * the bytes released; memHeld() falls by exactly that much.
   */
  shrinkToFit(): bigint;
  has(key: KeyInput): boolean;
  add(key: KeyInput): boolean;
  remove(key: KeyInput): boolean;
  clear(): void;
  first(): bigint | null;
  last(): bigint | null;
  next(key: KeyInput, inclusive?: boolean): bigint | null;
  prev(key: KeyInput, inclusive?: boolean): bigint | null;
  rank(key: KeyInput): bigint;
  select(k: KeyInput): bigint | null;
  countRange(start: KeyInput, end: KeyInput): bigint;
  toArray(): bigint[];
}

/**
 * A thread-safe concurrent map from NUL-free UTF-8 strings to 64-bit unsigned integers (OCC). A key with an embedded NUL throws.
 */
export class SyncExpanseStrMap {
  constructor();

  size(): bigint;
  isEmpty(): boolean;
  /** Heap bytes used by the map: a sum of the allocator's per-writer counters, taken without excluding writers, so no writer waits for it. Exact once every write has finished; while writers run it can differ by the bytes of the operations in flight. */
  memUsed(): bigint;
  /** Heap bytes held from the global allocator, including blocks the epoch collector keeps for reuse. Read with writers excluded, so every writer waits for it; `memUsed()` does not wait. */
  memHeld(): bigint;
  /** Returns the collector's freed blocks to the global allocator; returns the bytes released. */
  shrinkToFit(): bigint;
  has(key: string): boolean;
  set(key: string, value: KeyInput): bigint | null;
  get(key: string): bigint | null;
  /**
   * Atomic compare-and-swap: stores `newValue` only if the key's current value is
   * `expected`. `null` is "absent" on both sides. `observed` is the value the compare saw.
   */
  compareExchange(
    key: string,
    expected: KeyInput | null,
    newValue: KeyInput | null,
  ): CompareExchangeResult;
  /** Stores `value` only if `key` is absent; returns the existing value, or `null` if stored. */
  getOrInsert(key: string, value: KeyInput): bigint | null;
  /**
   * Retry loop over `compareExchange`: stores `fn(current)` (`null` removes the key)
   * and returns the replaced value. `fn` may run more than once per call and must be
   * free of side effects; an exception from `fn` propagates with the key unchanged.
   */
  update(key: string, fn: (current: bigint | null) => KeyInput | null): bigint | null;
  delete(key: string): boolean;
  clear(): void;
  /** Smallest entry in byte-lexicographical order, or `null` if empty. Optimistic reads: no writer lock unless a read exhausts its retries. */
  first(): StrMapEntry | null;
  /** Largest entry in byte-lexicographical order, or `null` if empty. Optimistic reads: no writer lock unless a read exhausts its retries. */
  last(): StrMapEntry | null;
  /** Smallest entry with key `> key` (`>= key` if `inclusive`). Optimistic reads: no writer lock unless a read exhausts its retries. */
  next(key: string, inclusive?: boolean): StrMapEntry | null;
  /** Largest entry with key `< key` (`<= key` if `inclusive`). Optimistic reads: no writer lock unless a read exhausts its retries. */
  prev(key: string, inclusive?: boolean): StrMapEntry | null;
}

/**
 * A thread-safe concurrent, unordered map from arbitrary byte keys to 64-bit unsigned integers (OCC).
 */
export class SyncExpanseBytesMap {
  constructor();

  size(): bigint;
  isEmpty(): boolean;
  /** Heap bytes used by the map, read with writers excluded. */
  memUsed(): bigint;
  /** Heap bytes held from the global allocator, including blocks the epoch collector keeps for reuse. */
  memHeld(): bigint;
  /** Returns the collector's freed blocks to the global allocator; returns the bytes released. */
  shrinkToFit(): bigint;
  has(key: BytesInput): boolean;
  set(key: BytesInput, value: KeyInput): bigint | null;
  get(key: BytesInput): bigint | null;
  /**
   * Atomic compare-and-swap: stores `newValue` only if the key's current value is
   * `expected`. `null` is "absent" on both sides. `observed` is the value the compare saw.
   */
  compareExchange(
    key: BytesInput,
    expected: KeyInput | null,
    newValue: KeyInput | null,
  ): CompareExchangeResult;
  /** Stores `value` only if `key` is absent; returns the existing value, or `null` if stored. */
  getOrInsert(key: BytesInput, value: KeyInput): bigint | null;
  /**
   * Retry loop over `compareExchange`: stores `fn(current)` (`null` removes the key)
   * and returns the replaced value. `fn` may run more than once per call and must be
   * free of side effects; an exception from `fn` propagates with the key unchanged.
   */
  update(key: BytesInput, fn: (current: bigint | null) => KeyInput | null): bigint | null;
  delete(key: BytesInput): boolean;
  clear(): void;
}

/**
 * A thread-safe concurrent map from arbitrary byte keys to 64-bit unsigned integers, ordered byte-lexicographically (OCC). Point operations are optimistic; the ordered reads block writers.
 */
export class SyncExpanseOrderedBytesMap {
  constructor();

  size(): bigint;
  isEmpty(): boolean;
  /** Heap bytes used by the map: a sum of the allocator's per-writer counters, taken without excluding writers, so no writer waits for it. Exact once every write has finished; while writers run it can differ by the bytes of the operations in flight. */
  memUsed(): bigint;
  /** Heap bytes held from the global allocator, including blocks the epoch collector keeps for reuse. Read with writers excluded, so every writer waits for it; `memUsed()` does not wait. */
  memHeld(): bigint;
  /** Returns the collector's freed blocks to the global allocator; returns the bytes released. */
  shrinkToFit(): bigint;
  has(key: BytesInput): boolean;
  set(key: BytesInput, value: KeyInput): bigint | null;
  get(key: BytesInput): bigint | null;
  /**
   * Atomic compare-and-swap: stores `newValue` only if the key's current value is
   * `expected`. `null` is "absent" on both sides. `observed` is the value the compare saw.
   */
  compareExchange(
    key: BytesInput,
    expected: KeyInput | null,
    newValue: KeyInput | null,
  ): CompareExchangeResult;
  /** Stores `value` only if `key` is absent; returns the existing value, or `null` if stored. */
  getOrInsert(key: BytesInput, value: KeyInput): bigint | null;
  /**
   * Retry loop over `compareExchange`: stores `fn(current)` (`null` removes the key)
   * and returns the replaced value. `fn` may run more than once per call and must be
   * free of side effects; an exception from `fn` propagates with the key unchanged.
   */
  update(key: BytesInput, fn: (current: bigint | null) => KeyInput | null): bigint | null;
  delete(key: BytesInput): boolean;
  clear(): void;
  /** Smallest entry in byte-lexicographical order, or `null` if empty. Run with writers excluded: each call blocks concurrent writers for its duration. */
  first(): BytesMapEntry | null;
  /** Largest entry in byte-lexicographical order, or `null` if empty. Run with writers excluded: each call blocks concurrent writers for its duration. */
  last(): BytesMapEntry | null;
  /** Smallest entry with key `> key` (`>= key` if `inclusive`). Run with writers excluded: each call blocks concurrent writers for its duration. */
  next(key: BytesInput, inclusive?: boolean): BytesMapEntry | null;
  /** Largest entry with key `< key` (`<= key` if `inclusive`). Run with writers excluded: each call blocks concurrent writers for its duration. */
  prev(key: BytesInput, inclusive?: boolean): BytesMapEntry | null;
}

/** A blob payload and its 24-bit hot metadata, as the shared blob map's owned reads report it. */
export interface BlobValue {
  payload: Buffer;
  /** The metadata as stored, at every payload length (#1548). */
  hotMeta: number;
}

/** A blob value offered to `SyncExpanseBlobMap.compareExchange`. */
export interface BlobValueInput {
  payload: BytesInput;
  /** Defaults to `0`; at most 2^24 - 1 on an arena payload. */
  hotMeta?: number;
}

/** The outcome of `SyncExpanseBlobMap.compareExchange`. */
export interface BlobCompareExchangeResult {
  stored: boolean;
  /** The value the compare saw, or `null` if the key was absent. */
  observed: BlobValue | null;
}

/** The outcome of `SyncExpanseBlobMap.compareExchangeMeta`. */
export interface BlobMetaExchangeResult {
  stored: boolean;
  /** The metadata the compare saw, or `null` if the key was absent. */
  observed: number | null;
}

/**
 * A thread-safe concurrent map from 64-bit integer keys to byte payloads with 24-bit
 * hot metadata (OCC). Reads return owned copies. A payload of any length keeps the
 * metadata it was written with (#1548); written with `hotMeta` 0, a short one is stored inline.
 */
export class SyncExpanseBlobMap {
  constructor(chunkSize?: number, maxCapacity?: number);

  size(): bigint;
  isEmpty(): boolean;
  /** Heap bytes used by the index and arena, read with writers excluded. Waits for a running compaction and makes writers wait; poll with `memUsedEstimate()` instead. */
  memUsed(): bigint;
  /** A bounded-cost estimate of `memUsed()` that takes no lock and waits for nothing; it may lag a running compaction. */
  memUsedEstimate(): bigint;
  /** Heap bytes held from the global allocator, including the collector's blocks. Read with writers excluded; waits for a running compaction and has no non-blocking form. */
  memHeld(): bigint;
  shrinkToFit(): bigint;
  setReclaimAtCap(on: boolean): void;
  reclaimAtCap(): boolean;
  /** Compacts the arena; concurrent writers wait for it, reads continue. */
  compact(): CompactionStatsResult;
  has(key: KeyInput): boolean;
  contains(key: KeyInput): boolean;
  /** Throws if the arena refuses the insert (metadata overflow, capacity cap, allocation failure). */
  set(key: KeyInput, payload: BytesInput, hotMeta?: number): void;
  get(key: KeyInput): Buffer | null;
  getWithMeta(key: KeyInput): BlobMetaResult | null;
  /** Metadata only, without touching the payload. */
  getMeta(key: KeyInput): number | null;
  delete(key: KeyInput): boolean;
  clear(): void;
  /**
   * Atomic compare-and-swap over `{ payload, hotMeta }` values; `null` is "absent" on
   * both sides. Throws when the values compare equal but `newValue` cannot be stored.
   */
  compareExchange(
    key: KeyInput,
    expected: BlobValueInput | null,
    newValue: BlobValueInput | null,
  ): BlobCompareExchangeResult;
  /**
   * Atomic compare-and-swap on the hot metadata alone. Throws when the metadata compares
   * equal but `newMeta` cannot be stored (above 24 bits, or non-zero on an absent key or
   * a payload stored inline with metadata 0).
   */
  compareExchangeMeta(
    key: KeyInput,
    expected: number | null,
    newMeta: number | null,
  ): BlobMetaExchangeResult;
  /** Stores only if `key` is absent; returns the existing value, or `null` if stored. */
  getOrInsert(key: KeyInput, payload: BytesInput, hotMeta?: number): BlobValue | null;
}
