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
  /** True if the payload was stored inline in the 64-bit value slot (≤ 7 bytes). */
  isInline: boolean;
}

/**
 * Compaction statistics returned by BlobMap in-place garbage collection.
 */
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
 * A high-performance map from 64-bit integer keys to arbitrary-length byte payloads
 * backed by inline polymorphic 64-bit value slots and chunked slab arenas.
 */
export class ExpanseBlobMap {
  /**
   * Creates an empty blob map, optionally with custom arena chunk size in bytes.
   */
  constructor(chunkSize?: number);

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
   * Saves the map to a relocatable binary image file. Returns bytes written.
   */
  saveImage(path: string): number;

  /**
   * Loads a map from a relocatable binary image file.
   */
  static openImage(path: string, mmap?: boolean): ExpanseBlobMap;
}

/**
 * A thread-safe concurrent 64-bit integer map with optimistic concurrency control (OCC).
 */
export class SyncExpanseMap {
  constructor();

  size(): bigint;
  isEmpty(): boolean;
  has(key: KeyInput): boolean;
  set(key: KeyInput, value: KeyInput): bigint | null;
  get(key: KeyInput): bigint | null;
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
