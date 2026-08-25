import {
  ExpanseSet,
  ExpanseMap,
  ExpanseStrMap,
  ExpanseBytesMap,
  ExpanseBlobMap,
  SyncExpanseMap,
  SyncExpanseSet,
} from '../index';

describe('@orieg/expanse Node / Bun bindings', () => {
  test('ExpanseSet basic operations', () => {
    const set = new ExpanseSet();
    expect(set.size()).toBe(0n);
    expect(set.add(42n)).toBe(true);
    expect(set.has(42n)).toBe(true);
    expect(set.containsBatch([42n, 99n])).toEqual([true, false]);
    expect(set.remove(42n)).toBe(true);
    expect(set.size()).toBe(0n);
  });

  test('ExpanseMap basic operations', () => {
    const map = new ExpanseMap();
    map.set(1n, 100n);
    expect(map.get(1n)).toBe(100n);
    expect(map.has(1n)).toBe(true);
    expect(map.getBatch([1n, 2n])).toEqual([100n, null]);
    expect(map.delete(1n)).toBe(true);
  });

  test('ExpanseStrMap basic operations', () => {
    const strmap = new ExpanseStrMap();
    strmap.set('hello', 1n);
    expect(strmap.get('hello')).toBe(1n);
    expect(strmap.has('hello')).toBe(true);
  });

  test('ExpanseBytesMap basic operations', () => {
    const bytesmap = new ExpanseBytesMap();
    const k = Buffer.from([0, 1, 2]);
    bytesmap.set(k, 10n);
    expect(bytesmap.get(k)).toBe(10n);
  });

  test('ExpanseBlobMap basic operations', () => {
    const blobmap = new ExpanseBlobMap();
    blobmap.set(1n, Buffer.from('test-payload'), 42);
    const meta = blobmap.getWithMeta(1n);
    expect(meta?.hotMeta).toBe(42);
    expect(meta?.payload.toString('utf8')).toBe('test-payload');
  });

  test('SyncExpanseMap OCC operations', () => {
    const syncMap = new SyncExpanseMap();
    syncMap.set(10n, 100n);
    expect(syncMap.get(10n)).toBe(100n);
  });

  test('SyncExpanseSet OCC operations', () => {
    const syncSet = new SyncExpanseSet();
    syncSet.add(10n);
    expect(syncSet.has(10n)).toBe(true);
  });
});
