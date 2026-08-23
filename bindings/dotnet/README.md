# Expanse.NET — Modern .NET & C# Bindings for Expanse

High-performance, **zero-GC**, off-heap associative trie collections for **.NET 8.0 / 9.0+** and C# powered by native `libexpanse` via modern P/Invoke and `SafeHandle`.

[![NuGet](https://img.shields.io/nuget/v/Expanse.NET.svg?style=flat-square)](https://www.nuget.org/packages/Expanse.NET)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg?style=flat-square)](../../LICENSE-MIT)

---

## Highlights

- **Zero-GC Allocation & Off-Heap Storage**: Keys and nodes are stored entirely off-heap in native 64-byte cache lines. Massive datasets with hundreds of millions of keys cause zero .NET GC pause latency.
- **Modern .NET Interop**: Safe unmanaged memory lifecycle management with `SafeHandle` / `IDisposable`, fast P/Invoke, and zero-copy `ReadOnlySpan<byte>` / `ReadOnlySpan<char>` APIs.
- **Rich Collection Suite**:
  - `ExpanseSet`: High-performance ordered 64-bit set (cf. Judy1) with $O(\text{depth})$ rank & select, range counting, and bidirectional navigation.
  - `ExpanseMap`: High-performance ordered 64-bit $\to$ 64-bit map (cf. JudyL).
  - `ExpanseStrMap`: High-performance ordered string trie supporting `string` and `ReadOnlySpan<char>` (cf. JudySL).
  - `ExpanseBytesMap`: High-performance binary-safe hash map supporting arbitrary byte keys and embedded NUL (`0x00`) bytes (cf. JudyHS).
  - `ExpanseBlobMap`: Polymorphic off-heap large-value map with inline packing ($\le 7$ bytes stored in 64-bit slot), chunked arena slabs, 32-bit hot metadata filtering, zero-copy span access, compaction, and predicate pruning.
  - `ExpanseSyncSet` / `ExpanseSyncMap`: Multithreaded concurrent collections with serialized writers and lock-free readers.

---

## Installation

Install via the .NET CLI:

```bash
dotnet add package Expanse.NET
```

Or via Package Manager Console in Visual Studio:

```powershell
Install-Package Expanse.NET
```

---

## Quickstart

### 1. Ordered Bit Set (`ExpanseSet`)

```csharp
using Expanse;

using var set = new ExpanseSet();

// Insert keys
set.Add(10);
set.Add(20);
set.Add(30);

// Fast membership test
if (set.Contains(20))
{
    Console.WriteLine("Key 20 is present.");
}

// O(depth) Rank & Select
ulong rank = set.Rank(25); // Number of keys < 25 (returns 2: 10, 20)
ulong? secondKey = set.Select(1); // 0-based rank select (returns 20)

// Bidirectional navigation
ulong? next = set.Next(10); // Smallest key > 10 (returns 20)
ulong? prev = set.Prev(30); // Largest key < 30 (returns 20)

// LINQ and IEnumerable iteration
foreach (ulong key in set)
{
    Console.WriteLine($"Key: {key}");
}
```

### 2. Ordered Word Map (`ExpanseMap`)

```csharp
using Expanse;

using var map = new ExpanseMap();

// Indexer and assignment
map[100] = 5000;
map[200] = 8000;

if (map.TryGet(100, out ulong value))
{
    Console.WriteLine($"Key 100 -> {value}");
}

// Navigation
var first = map.First(); // (100, 5000)
var next = map.Next(100); // (200, 8000)

// Range queries
ulong count = map.CountRange(100, 200); // 2
```

### 3. String Trie (`ExpanseStrMap`)

```csharp
using Expanse;

using var strmap = new ExpanseStrMap();

strmap["apple"] = 1;
strmap["banana".AsSpan()] = 2;
strmap["cherry"] = 3;

if (strmap.TryGet("banana", out ulong id))
{
    Console.WriteLine($"banana -> {id}");
}

// Lexicographical navigation
var nextEntry = strmap.Next("apple"); // ("banana", 2)
```

### 4. Off-Heap Blob Map (`ExpanseBlobMap`)

```csharp
using System.Text;
using Expanse;

using var blobMap = new ExpanseBlobMap();

// Small payloads (<= 7 bytes) are packed inline inside 64-bit value slots
blobMap.Set(1, "inline"u8, hotMeta: 10);

// Large payloads (> 7 bytes) are allocated in chunked arena slabs
byte[] largePayload = Encoding.UTF8.GetBytes("A large binary blob payload stored in off-heap slab arenas.");
blobMap.Set(2, largePayload, hotMeta: 20);

// Zero-copy span lookup
if (blobMap.TryGet(2, out ReadOnlySpan<byte> span, out uint meta))
{
    Console.WriteLine($"Retrieved {span.Length} bytes with meta={meta}");
}

// Pruning with custom predicate and automatic slab compaction
ulong pruned = blobMap.Prune((key, hotMeta) => hotMeta == 10);
Console.WriteLine($"Pruned {pruned} entries.");
```

### 5. Concurrent One-Writer / Lock-Free Readers (`ExpanseSyncMap`)

```csharp
using Expanse;

using var syncMap = new ExpanseSyncMap();

// Writer
syncMap.Set(42, 1000);

// Lock-free reader in another thread
Task.Run(() =>
{
    using var reader = syncMap.CreateReader();
    if (reader.TryGet(42, out ulong val))
    {
        Console.WriteLine($"Lock-free read: {val}");
    }
});
```

---

## Native Library Resolution

`Expanse.NET` automatically resolves the native `libexpanse` shared library across Linux (`.so`), macOS (`.dylib`), and Windows (`.dll`).

Custom library locations can be provided via environment variables:
- `EXPANSE_CDYLIB`: Absolute path to `libexpanse.so` / `libexpanse.dylib` / `expanse.dll`.
- `EXPANSE_LIB_DIR`: Directory containing `libexpanse`.

---

## License

Licensed under either of [MIT License](../../LICENSE-MIT) or [Apache License, Version 2.0](../../LICENSE-APACHE) at your option.
