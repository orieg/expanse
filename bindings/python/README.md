# Expanse Python Bindings (`expanse-trie`)

High-performance, pure-Rust Judy arrays and digital trie engine for Python (PyO3 + `abi3` binary wheels).

## Installation

```bash
pip install expanse-trie
```

## Quickstart

```python
from expanse_trie import ExpanseSet, ExpanseMap, ExpanseStrMap, ExpanseBlobMap, SyncExpanseMap

# 1. Dynamic sparse 64-bit integer set (Judy1)
s = ExpanseSet([10, 20, 50, 100])
assert 20 in s
assert s.next_at_or_after(25) == 50
assert s.count_range(10, 50) == 3

# 2. Key-value map (JudyL)
m = ExpanseMap()
m[42] = 1000
assert m[42] == 1000

# 3. String trie (JudySL)
sm = ExpanseStrMap()
sm["/api/v1/users"] = 200

# 4. Large-value off-heap blob map with inline packing
blob = ExpanseBlobMap()
blob.insert(1, b"inline or arena payload", hot_meta=0x01)
payload, meta = blob.get(1)

# 5. Multithreaded lock-free OCC concurrent map (GIL-free queries)
sync_map = SyncExpanseMap()
sync_map.insert(100, 5000)
assert sync_map.get(100) == 5000
```

## Documentation

For full API specifications, type stubs, and concurrency architecture, see [docs/BINDINGS_PYTHON.md](../../docs/BINDINGS_PYTHON.md).
