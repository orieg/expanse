# Expanse Java & Scala Bindings: Zero-GC Off-Heap Architecture

This document details the architecture, design, performance characteristics, and integration patterns of the **Expanse Java & Scala bindings** (`io.github.orieg:expanse-java`).

---

## 1. Executive Summary

Standard Java collections (`java.util.HashMap`, `java.util.TreeMap`, `ConcurrentHashMap`) and third-party off-heap libraries suffer from two structural bottlenecks when scaling to tens of millions of keys:

1. **JVM Heap Garbage Collection Pressure**: Every `java.lang.Long` or map entry node (`Node<K,V>`) is a heap object with 16–24 bytes of object header overhead plus pointer indirection. A map of 100 million entries requires over **4–6 GB of heap memory just for object headers**, causing frequent Stop-The-World (STW) GC pauses under G1, ParallelGC, or ZGC.
2. **JNI Crossing Overhead**: Traditional JNI wrappers require marshalling arguments across the native barrier, pinning heap buffers, acquiring thread-state locks, and preventing C2 JIT inlining.

`expanse-java` solves both bottlenecks:
- **Project Panama Foreign Function & Memory (FFM) API** (`java.lang.foreign`, Java 22+ / 21 LTS): Replaces JNI with direct native downcalls compiled to native assembly stubs that the C2 JIT compiler can inline directly into Java bytecode.
- **Pure Off-Heap Digital Trie Storage**: Nodes, branches, bitmap leaves, and values live exclusively in native memory aligned to 64-byte cache lines. Zero objects allocated on the JVM heap during lookups, inserts, and iteration.
- **Byte-Exact Memory Accounting**: Down to **0.07–0.36 bytes per key** on dense/clustered integer sets, with exact off-heap bytes reported at any time via `map.memoryUsed()`.

---

## 2. Project Panama FFM vs Legacy JNI

| Characteristic | Legacy JNI (`System.loadLibrary`) | Project Panama FFM (`java.lang.foreign`) |
|---|---|---|
| **Downcall Dispatch** | Dynamic symbol lookup table, C stub boilerplate, `JNIEnv*` passing | Direct machine code stub via `Linker.downcallHandle()` |
| **C2 JIT Inlining** | ❌ Opaque barrier; JIT cannot inline across JNI | 🟢 Native downcalls inlined directly into JIT-compiled hot loops |
| **Argument Marshalling** | JNI type conversions (`jlong`, `jobject`, `jbyteArray`) | Direct CPU registers via `ValueLayout.JAVA_LONG`, `ValueLayout.ADDRESS` |
| **Memory Management** | `ByteBuffer.allocateDirect()`, Unsafe memory addresses | Typed `MemorySegment` and scoped `Arena` (`ofConfined`, `ofShared`, `ofAuto`) |
| **Safety & Leak Prevention** | Manual pointer tracking, memory leaks on unreleased global refs | Deterministic deterministic lifecycle via `AutoCloseable` & Arena lifecycle |
| **Off-Heap Pointer Writes** | Multiple round-trip JNI calls for lookup and update | Direct 8-byte `MemorySegment` value slot pointer writes with zero traversal |

### Panama Downcall Invocations in `expanse-java`
`expanse-java` encapsulates downcall method handles in `io.github.orieg.expanse.internal.ExpanseNative`:
```java
Linker linker = Linker.nativeLinker();
SymbolLookup lookup = NativeLoader.getSymbolLookup();

// MethodHandle for expanse_map_get(map_ptr, key, &val_out) -> bool
MethodHandle MH_expanse_map_get = linker.downcallHandle(
    lookup.find("expanse_map_get").orElseThrow(),
    FunctionDescriptor.of(
        ValueLayout.JAVA_BOOLEAN, // return bool
        ValueLayout.ADDRESS,      // expanse_map_t*
        ValueLayout.JAVA_LONG,     // uint64_t key
        ValueLayout.ADDRESS       // uint64_t* value_out
    )
);
```

To eliminate even out-parameter heap allocations in Java, `ExpanseMap` and `ExpanseSet` utilize reusable per-thread scratch segments (`ThreadLocal<MemorySegment>`), enabling **100% zero-allocation lookups**.

---

## 3. GC Elimination & Off-Heap Memory Layout

### Heap Churn Comparison on 50,000,000 Key-Value Entries

| Metric | `java.util.HashMap<Long, Long>` | `java.util.TreeMap<Long, Long>` | `io.github.orieg.expanse.ExpanseMap` |
|---|---:|---:|---:|
| **JVM Heap Used** | ~3,200 MB | ~2,400 MB | **0 MB (Zero Heap)** |
| **Off-Heap Memory** | 0 MB | 0 MB | **~430 MB (Clustered) / ~835 MB (Random)** |
| **Heap Objects Allocated** | 100,000,000+ objects | 50,000,000+ objects | **1 instance (Handle Wrapper)** |
| **Young Gen GC Pauses** | Frequent (15–40 ms) | Frequent (20–60 ms) | **0 ms (Zero GC Pause)** |
| **Full GC STW Pause Time** | 450–1,200 ms | 600–1,800 ms | **0 ms (Completely Immune)** |

---

## 4. Value Slots: Zero-Traversal Off-Heap Pointer Mutation

In traditional maps, updating an existing key requires two separate tree traversals or hash lookups (`get` + `put`).

Expanse provides the **value slot contract** (`slot` / `insertSlot`):
1. `insertSlot(key)` traverses the 256-ary digital trie once, ensuring the key exists with 0 if new.
2. Returns a direct writable `MemorySegment` pointing directly to the 64-bit value in off-heap memory.
3. Subsequent in-place updates mutate the 64-bit word directly through CPU memory writes without traversing the tree again.

```java
try (ExpanseMap counterMap = new ExpanseMap()) {
    // 1. Get or create slot in one single trie walk
    MemorySegment slot = counterMap.insertSlot(42L);

    // 2. High-frequency in-place mutation (e.g. counter increment)
    for (int i = 0; i < 1_000_000; i++) {
        long current = slot.get(ValueLayout.JAVA_LONG, 0);
        slot.set(ValueLayout.JAVA_LONG, 0, current + 1);
    }

    assertEquals(1_000_000L, counterMap.get(42L).getAsLong());
}
```

---

## 5. Big Data & Streaming Integration Patterns

### 5.1 Apache Spark: Off-Heap Broadcast State & Deduplication

In distributed Spark executors, storing large deduplication sets or lookup maps in JVM heap memory causes severe executor GC thrashing and OOMs.

```scala
import io.github.orieg.expanse.ExpanseSet
import org.apache.spark.sql.functions._

// Spark Partition RDD Processing with Zero GC Churn
val processedRDD = rawRDD.mapPartitions { iterator =>
  // Allocate off-heap set per executor partition task
  val seenSet = new ExpanseSet()
  try {
    iterator.filter { record =>
      val key = record.userId
      // Fast point lookup & insert with zero heap allocation
      seenSet.add(key)
    }
  } finally {
    seenSet.close() // Free off-heap native memory deterministically
  }
}
```

### 5.2 Apache Flink: Low-Latency Streaming State & Sliding Windows

In high-throughput Flink streaming pipelines, RocksDB state backends introduce heavy serialization overhead, while JVM heap state backends cause GC pauses that break latency SLAs.

```java
import io.github.orieg.expanse.ExpanseMap;
import org.apache.flink.api.common.functions.RichFlatMapFunction;
import org.apache.flink.configuration.Configuration;
import org.apache.flink.util.Collector;

public class OffHeapStreamingJoinFunction extends RichFlatMapFunction<Event, EnrichedEvent> {
    private transient ExpanseMap offHeapState;

    @Override
    public void open(Configuration parameters) {
        // High-density off-heap associative state store
        offHeapState = new ExpanseMap();
    }

    @Override
    public void flatMap(Event event, Collector<EnrichedEvent> out) {
        long key = event.getDeviceId();
        long timestamp = event.getTimestamp();

        // O(depth) rank check: check how many events occurred in range
        long eventsInRange = offHeapState.countRange(timestamp - 3600_000L, timestamp);

        offHeapState.put(timestamp, event.getValue());
        out.collect(new EnrichedEvent(event, eventsInRange));
    }

    @Override
    public void close() {
        if (offHeapState != null) {
            offHeapState.close();
        }
    }
}
```

### 5.3 Low-Latency Financial Order Books & Market Depth

In ultra-low latency trading systems, `ExpanseMap` provides ordered price-level traversal (`ceilingEntry`, `floorEntry`, `higherKey`, `lowerKey`) and direct memory slot updates without Java memory locks.

```java
try (ExpanseMap bidBook = new ExpanseMap();
     ExpanseMap askBook = new ExpanseMap()) {

    // Insert bid price level (price in cents -> cumulative quantity)
    bidBook.put(150_25L, 500L);
    bidBook.put(150_20L, 1200L);
    bidBook.put(150_10L, 3400L);

    // Fast Best Bid lookup
    bidBook.lastEntry().ifPresent(bestBid -> {
        System.out.printf("Best Bid: $%.2f (Qty: %d)%n", bestBid.key() / 100.0, bestBid.value());
    });

    // Level-2 depth iteration
    bidBook.forEach((price, qty) -> {
        // In-order traversal from lowest to highest bid
    });
}
```

---

## 6. Multi-Core Optimistic Concurrency Control (OCC)

`SyncExpanseMap` and `SyncExpanseSet` provide thread-safe concurrent access designed for read-heavy workloads:
- **Lock-Free Readers**: Readers take an optimistic snapshot and validate subtrees without taking any read lock, avoiding cache line bouncing.
- **Single-Writer Serialization**: Writers serialize internally without locking out concurrent readers.
- **Reader Reuse**: Create a `SyncExpanseMap.Reader` per worker thread and reuse it across requests.

```java
SyncExpanseMap concurrentMap = new SyncExpanseMap();

// Worker thread:
try (SyncExpanseMap.Reader reader = concurrentMap.reader()) {
    while (running) {
        long key = pollNextKey();
        OptionalLong val = reader.get(key);
        // ...
    }
}
```

---

## 7. Scala Language Integration

`expanse-java` works seamlessly with Scala 2.13 and Scala 3:

```scala
import io.github.orieg.expanse.{ExpanseMap, ExpanseSet}
import scala.util.Using

// Automatic resource management via Scala Using
Using.resource(new ExpanseMap()) { map =>
  map.put(100L, 1L)
  map.put(200L, 2L)
  map.put(300L, 3L)

  // Rank query
  val count = map.countRange(100L, 250L) // 2

  // Key navigation
  val next = map.higherKey(100L) // OptionalLong.of(200)
}
```

---

## 8. Artifact Coordinates, Packaging & Deployment

### Maven Central Dependency
```xml
<dependency>
    <groupId>io.github.orieg</groupId>
    <artifactId>expanse-java</artifactId>
    <version>0.5.0</version>
</dependency>
```

### Self-Contained Multi-Arch Native JAR Packaging
The Java binding is distributed as a **self-contained multi-arch JAR** (`io.github.orieg:expanse-java`). Built during the release DAG (`.github/workflows/release.yml` job `package-maven`), the package bundles precompiled, hardware-optimized shared libraries directly inside the JAR under classpath resources:
```
/native/linux-x86_64/libexpanse.so
/native/linux-aarch64/libexpanse.so
/native/darwin-aarch64/libexpanse.dylib
/native/darwin-x86_64/libexpanse.dylib
/native/windows-x86_64/expanse.dll
```

### Dynamic Extraction & Resolution Protocol
On first use, `NativeLoader` performs resolution in deterministic priority order:
1. **JVM System Property**: `-Dexpanse.library.path=/path/to/libexpanse.so` (explicit user override).
2. **Environment Variable**: `EXPANSE_LIBRARY_PATH=/path/to/libexpanse.so`.
3. **Local Dev Tree**: In-repo build artifacts under `crates/expanse-capi/target/release/`.
4. **Bundled Resource Extraction**: Extracts `/native/{classifier}/{libName}` from the classpath to an OS temporary directory and binds it to `Arena.global()`.
5. **System Library Path**: `System.loadLibrary` fallback via `java.library.path` / `LD_LIBRARY_PATH`.

### Supported Platform & CI Matrix

| OS / Architecture | Classifier | Native Binary | Validation Channel |
|---|---|---|---|
| Linux x86_64 | `linux-x86_64` | `libexpanse.so` | **Active CI** (`ubuntu-latest`) |
| Linux aarch64 | `linux-aarch64` | `libexpanse.so` | Release build (cross-compiled `aarch64-unknown-linux-gnu`) |
| macOS ARM64 | `darwin-aarch64` | `libexpanse.dylib` | **Active CI** (`macos-latest` Apple Silicon) |
| macOS x86_64 | `darwin-x86_64` | `libexpanse.dylib` | Release build (`x86_64-apple-darwin`) |
| Windows x86_64 | `windows-x86_64` | `expanse.dll` | **Active CI** (`windows-latest`) |

---

## 9. JDK Compatibility & Baseline Discipline

| JDK Baseline | Support Status | Required JVM Flags | Architectural Details |
|---|---|---|---|
| **JDK 22+** | **Primary Baseline** | `--enable-native-access=ALL-UNNAMED` | Finalized Project Panama Foreign Function & Memory (FFM) API ([JEP 454](https://openjdk.org/jeps/454)). Stable downcall handles and scoped memory segments. |
| **JDK 21 LTS** | **Source Build (Preview)** | `--enable-preview --enable-native-access=ALL-UNNAMED` | Preview FFM implementation ([JEP 442](https://openjdk.org/jeps/442)). Requires source compilation targeting `--release 21 --enable-preview`. |
| **JDK 17 LTS** | **Unsupported** | — | Contains only early incubator module (`jdk.incubator.foreign` - [JEP 412](https://openjdk.org/jeps/412)), which uses fundamentally distinct package structures and incompatible segment abstractions. |

### JVM Launch Configuration
Because Panama downcalls perform raw off-heap dereferences and native linker bindings, the JVM requires native access authorization:
```bash
java --enable-native-access=ALL-UNNAMED -jar app.jar
```
