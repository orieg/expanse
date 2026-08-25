# expanse-py (`expanse-trie`)

High-performance Python bindings for **Expanse** — the clean-room, pure-Rust Judy array and digital trie engine.

## Installation

```bash
pip install expanse-trie
```

## Quickstart

```python
from expanse_trie import ExpanseSet, ExpanseMap, SyncExpanseMap

# 1. Sparse Integer Set (Judy1 equivalent)
s = ExpanseSet([10, 20, 50, 100])
assert 20 in s
assert s.next_at_or_after(25) == 50
assert s.count_range(10, 50) == 3

# 2. Integer-to-Integer Map (JudyL equivalent)
m = ExpanseMap()
m[42] = 100
assert m[42] == 100
assert m.range(0, 100) == [(42, 100)]

# 3. Concurrent Lock-Free OCC Map (GIL-free queries)
sync_m = SyncExpanseMap({1: 100, 2: 200})
assert sync_m[1] == 100
```

See [docs/bindings/python.md](../../docs/bindings/python.md) for complete documentation.
