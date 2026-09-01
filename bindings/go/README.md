# expanse-go

Native Go 1.22+ bindings for `libexpanse`, providing zero-GC off-heap ordered maps, sets, and concurrent optimistic-read collections via both **CGO** and **CGO-free PureGo** modes.

## Installation

```bash
go get github.com/orieg/expanse/bindings/go@v0.5.0
```

## Build Modes

`expanse-go` supports two interchangeable compilation modes with identical public APIs and 100% C ABI symbol parity:

### 1. PureGo Mode (`CGO_ENABLED=0` or `-tags expanse_purego`)

Builds with zero C toolchain dependencies via [`purego`](https://github.com/ebitengine/purego), dynamically resolving the native `libexpanse` shared library at runtime.

- **Use case**: Cross-compilation, `FROM scratch` / distroless Docker containers, environments without `gcc`/`clang`, `go install` from source.
- **Compilation**:
  ```bash
  CGO_ENABLED=0 go build .
  # or explicitly with build tag:
  go build -tags expanse_purego .
  ```

> **macOS + `-tags expanse_purego` with cgo enabled requires Go 1.24, or an external link.**
> The build tag keeps cgo enabled (that is the point of the tag — it forces the
> purego path on a machine that *has* a C toolchain). Go's internal linker
> emitted no `LC_UUID` load command on darwin until 1.24, and dyld on macOS 15+
> refuses to start a cgo-linked binary without one:
>
> ```
> dyld[...]: missing LC_UUID load command
> signal: abort trap
> ```
>
> It aborts before `main`, so it looks like a binding failure rather than a
> toolchain one. Measured on macOS 26.4.1 arm64: **1.22 and 1.23 abort; 1.24 and
> 1.25 are fine.** On Go < 1.24 either link externally —
>
> ```bash
> go build -tags expanse_purego -ldflags=-linkmode=external .
> ```
>
> — or use `CGO_ENABLED=0`, which is unaffected on every version because there
> is no cgo to link, and is the mode this binding is really for. Linux and
> Windows are unaffected throughout.

### 2. CGO Mode (Default with `CGO_ENABLED=1`)

Links `libexpanse.a` statically (or `libexpanse.so` / `.dylib` dynamically) through standard CGO downcalls.

- **Use case**: Maximum throughput for microsecond-sensitive scalar point lookups.
- **Compilation**:
  ```bash
  go build .
  ```

---

## Performance & Optimization Guide

Understanding the runtime characteristics of each build mode helps select the optimal configuration for your workload:

### Trade-Off Summary

| Dimension | CGO Mode (`CGO_ENABLED=1`) | PureGo Mode (`CGO_ENABLED=0`) |
|:---|:---|:---|
| **C Toolchain Needed** | Yes (`gcc`, `clang`, or mingw) | **No** (pure Go compiler) |
| **Cross-Compilation** | Requires cross-C toolchain | **Trivial** (`GOOS=... GOARCH=... go build`) |
| **Point Lookup Latency** | **~90 ns/op** (measured: Apple M1) | **~460 ns/op** (measured: Apple M1) |
| **Point Lookup Allocations** | **0 B/op, 0 allocs/op** | **320 B/op, 6 allocs/op** (reflection boxing) |
| **Batch Lookup Latency** | **<10 ns/key** | **<15 ns/key** (amortized FFI crossing) |
| **Container Targets** | glibc / musl dynamic linking | Scratch / Distroless static binaries |

### High-Throughput Best Practice: Batch Operations

In PureGo mode, scalar point lookups (`Get`, `Contains`) invoke dynamic reflection trampolines. For high-throughput pipelines, batch methods pass contiguous memory buffers across the FFI boundary once per slice rather than once per key, eliminating reflection overhead and triggering SIMD/MLP prefetching in the native engine:

```go
package main

import (
	"fmt"

	"github.com/orieg/expanse/bindings/go"
)

func main() {
	m := expanse.NewMap()
	defer m.Free()

	// Populate map
	for i := uint64(0); i < 1000; i++ {
		m.Set(i, i*10)
	}

	// Batch lookup: 1 FFI call for 1000 keys
	keys := make([]uint64, 1000)
	for i := range keys {
		keys[i] = uint64(i)
	}
	values := make([]uint64, 1000)
	found := make([]bool, 1000)

	foundCount := m.GetBatch(keys, values, found)
	fmt.Printf("Batch retrieved %d keys\n", foundCount)
}
```

Similarly, `Set.ContainsBatch(keys, outPresent)` checks membership across large key batches in a single FFI crossing.

---

## Memory & Lifecycle Management

Expanse collections allocate their nodes in **off-heap C arena memory**, completely outside the Go garbage collector's scan path (0 GC heap overhead).

### Explicit Deallocation (`defer m.Free()`)

Although Go finalizers (`runtime.SetFinalizer`) are attached as a safety net against memory leaks, finalizers execute non-deterministically across future GC cycles. **Always call `defer m.Free()` explicitly** when creating collections:

```go
m := expanse.NewMap()
defer m.Free() // Immediately frees native off-heap trie memory
```

For concurrent readers (`SyncMap.Reader()` / `SyncSet.Reader()`), call `defer reader.Free()` when finished with the reader snapshot handle.

---

## Native Library Discovery (PureGo Mode)

In PureGo mode, `expanse-go` searches for `libexpanse.so` (Linux), `libexpanse.dylib` (macOS), or `expanse.dll` (Windows) in order of priority:

1. **`EXPANSE_LIBRARY`**: Exact path to the library file, or directory containing it.
2. **`EXPANSE_LIBRARY_PATH` / `EXPANSE_LIB_DIR`**: Alternative directory search paths.
3. **Development Build Tree**: Relative workspace paths (`target/release/libexpanse.*`).
4. **Standard System Paths**: `/usr/local/lib`, `/usr/lib`, `/opt/homebrew/lib`, `/opt/local/lib`, or system dynamic linker search paths.

### Standalone Single-Binary Deployments (`go:embed`)

For self-contained deployments in `CGO_ENABLED=0` mode without pre-installing `libexpanse` on the host, embed the native shared library via `//go:embed`, extract it to a cache directory on initialization, and set `EXPANSE_LIBRARY`:

```go
package main

import (
	_ "embed"
	"os"
	"path/filepath"

	"github.com/orieg/expanse/bindings/go"
)

//go:embed libexpanse.so
var embeddedLib []byte

func init() {
	if os.Getenv("EXPANSE_LIBRARY") == "" {
		tmpPath := filepath.Join(os.TempDir(), "libexpanse.so")
		if _, err := os.Stat(tmpPath); os.IsNotExist(err) {
			_ = os.WriteFile(tmpPath, embeddedLib, 0755)
		}
		_ = os.Setenv("EXPANSE_LIBRARY", tmpPath)
	}
}
```

---

## Quickstart

```go
package main

import (
	"fmt"

	"github.com/orieg/expanse/bindings/go"
)

func main() {
	// 1. Ordered uint64 -> uint64 Map
	m := expanse.NewMap()
	defer m.Free()

	m.Set(42, 100)
	val, ok := m.Get(42)
	fmt.Printf("Get(42) = %d (found=%v)\n", val, ok)

	// Range count and ordered navigation
	m.Set(10, 200)
	m.Set(20, 300)
	fmt.Printf("Keys in [10, 42]: %d\n", m.CountRange(10, 42))

	// 2. Ordered uint64 Set
	s := expanse.NewSet()
	defer s.Free()

	s.Add(100)
	s.Add(200)
	fmt.Printf("Set contains 100: %v, size: %d\n", s.Contains(100), s.Size())

	// 3. Lock-Free Concurrent Reader (SyncMap)
	syncMap := expanse.NewSyncMap()
	defer syncMap.Free()

	syncMap.Set(1, 1000)

	reader := syncMap.Reader()
	defer reader.Free()
	rVal, rOk := reader.Get(1)
	fmt.Printf("Sync reader Get(1) = %d (found=%v)\n", rVal, rOk)
}
```

---

## API Surface

- **`Set`**: Ordered `uint64` set with rank, select, range counts, and batch membership queries (`ContainsBatch`).
- **`Map`**: Ordered `uint64 -> uint64` map with rank, select, range counts, and batch lookups (`GetBatch`).
- **`StrMap`**: Ordered NUL-terminated string map with truncation-safe navigation (`First`, `Next`, `Prev`, `Last`).
- **`BytesMap`**: Unordered arbitrary byte-slice map.
- **`BlobMap`**: Large-value blob arena map with predicate-based pruning (`Prune`) and memory compaction (`Compact`).
- **`SyncSet` / `SyncMap`**: Single-writer, optimistic concurrency control (OCC) reader collections.
