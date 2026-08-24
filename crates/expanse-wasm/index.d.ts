/* tslint:disable */
/* eslint-disable */
export function init_panic_hook(): void;

export class WasmExpanseSet {
  free(): void;
  constructor();
  add(key: bigint): boolean;
  remove(key: bigint): boolean;
  contains(key: bigint): boolean;
  size(): bigint;
  clear(): void;
  first(): bigint | undefined;
  next(key: bigint): bigint | undefined;
  last(): bigint | undefined;
  prev(key: bigint): bigint | undefined;
  rank(key: bigint): bigint;
  select(k: bigint): bigint | undefined;
  countRange(start: bigint, end: bigint): bigint;
  toArray(): BigUint64Array;
}

export class WasmExpanseMap {
  free(): void;
  constructor();
  set(key: bigint, value: bigint): void;
  get(key: bigint): bigint | undefined;
  delete(key: bigint): boolean;
  contains(key: bigint): boolean;
  size(): bigint;
  clear(): void;
  first(): [bigint, bigint] | undefined;
  next(key: bigint): [bigint, bigint] | undefined;
}

export class WasmExpanseStrMap {
  free(): void;
  constructor();
  set(key: string, value: bigint): void;
  get(key: string): bigint | undefined;
  delete(key: string): boolean;
  contains(key: string): boolean;
  size(): bigint;
  clear(): void;
  first(): [string, bigint] | undefined;
  next(key: string): [string, bigint] | undefined;
  keysWithPrefix(prefix: string): string[];
}

export class WasmExpanseBytesMap {
  free(): void;
  constructor();
  set(key: Uint8Array, value: bigint): void;
  get(key: Uint8Array): bigint | undefined;
  delete(key: Uint8Array): boolean;
  contains(key: Uint8Array): boolean;
  size(): bigint;
  clear(): void;
}

export class WasmExpanseBlobMap {
  free(): void;
  constructor();
  set(key: bigint, payload: Uint8Array, hot_meta: number): void;
  get(key: bigint): Uint8Array | undefined;
  getWithMeta(key: bigint): [Uint8Array, number] | null;
  delete(key: bigint): boolean;
  contains(key: bigint): boolean;
  size(): bigint;
  clear(): void;
  prune(predicate: (key: bigint, meta: number) => boolean): number;
  saveImage(): Uint8Array;
  static fromImage(data: Uint8Array): WasmExpanseBlobMap;
}
