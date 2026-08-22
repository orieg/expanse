# Expanse Java & Scala Bindings (`io.github.orieg:expanse-java`)

High-performance, **zero-GC**, off-heap associative trie collections for Java and Scala powered by **Project Panama Foreign Function & Memory (FFM) API** (`java.lang.foreign`) and `libexpanse`.

[![Maven Central](https://img.shields.io/maven-central/v/io.github.orieg/expanse-java.svg?style=flat-square)](https://central.sonatype.com/artifact/io.github.orieg/expanse-java)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg?style=flat-square)](../../LICENSE-MIT)

---

## Highlights

- **Zero JVM Heap & Zero GC Overhead**: Collections store keys and nodes completely off-heap in native 64-byte cache lines. Zero GC pause impact even with hundreds of millions of keys.
- **Project Panama FFM (Java 22+ / 21 LTS)**: Uses native `java.lang.foreign` downcalls directly into `libexpanse`, eliminating JNI transition penalties.
- **Direct Value Slots**: Directly read and write off-heap 64-bit value slots via `MemorySegment` with zero tree re-traversals.
- **Rich Collection Set**:
  - `ExpanseSet`: High-performance ordered 64-bit set (cf. Judy1) with $O(\text{depth})$ rank & select.
  - `ExpanseMap`: High-performance ordered 64-bit $\to$ 64-bit map (cf. JudyL).
  - `ExpanseStrMap`: High-performance ordered String $\to$ 64-bit map (cf. JudySL).
  - `ExpanseBytesMap`: High-performance unordered byte slice $\to$ 64-bit map (cf. JudyHS).
  - `SyncExpanseMap` / `SyncExpanseSet`: Multithreaded OCC concurrent collections with lock-free readers.
- **Standard Java Collection Wrappers**: Standard `java.util.NavigableMap<Long, Long>`, `java.util.NavigableSet<Long>`, and `java.util.Map<String, Long>` interfaces.
- **Big Data Ready**: Designed for Apache Spark, Apache Flink, Kafka Streams, and high-frequency trading where GC pauses are unacceptable.

---

## Installation

### Maven
```xml
<dependency>
    <groupId>io.github.orieg</groupId>
    <artifactId>expanse-java</artifactId>
    <version>0.2.0</version>
</dependency>
```

### Gradle (Kotlin / Groovy)
```groovy
implementation 'io.github.orieg:expanse-java:0.2.0'
```

### sbt (Scala)
```scala
libraryDependencies += "io.github.orieg" % "expanse-java" % "0.2.0"
```

---

## Quick Start

### 1. Ordered Map (`ExpanseMap`)
```java
import io.github.orieg.expanse.ExpanseMap;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;

try (ExpanseMap map = new ExpanseMap()) {
    // Fast inserts and lookups
    map.put(100L, 5000L);
    map.put(200L, 8000L);

    long val = map.getOrDefault(100L, -1L);
    System.out.println("Key 100: " + val);

    // Direct off-heap value slot mutation (zero traversal overhead)
    MemorySegment slot = map.insertSlot(300L);
    slot.set(ValueLayout.JAVA_LONG, 0, 9999L);
    System.out.println("Key 300: " + map.get(300L).getAsLong());

    // O(depth) rank and range queries
    long countBelow = map.countBelow(200L); // 1 key (< 200)
    long countRange = map.countRange(100L, 300L); // 3 keys

    // Ordered navigation
    map.ceilingEntry(150L).ifPresent(e -> 
        System.out.println("Ceiling >= 150: " + e.key() + " -> " + e.value())
    );

    // Exact off-heap memory accounting
    System.out.printf("Native memory used: %d bytes%n", map.memoryUsed());
}
```

### 2. High-Performance Bit Set (`ExpanseSet`)
```java
import io.github.orieg.expanse.ExpanseSet;

try (ExpanseSet set = new ExpanseSet()) {
    set.add(10L);
    set.add(20L);
    set.add(30L);

    if (set.contains(20L)) {
        System.out.println("Set contains 20");
    }

    // 0-based rank select
    set.byCount(1).ifPresent(key -> System.out.println("2nd smallest key: " + key));

    // Primitive zero-allocation streaming
    long sum = set.stream().sum();
}
```

### 3. Multithreaded OCC Readers (`SyncExpanseMap`)
```java
import io.github.orieg.expanse.SyncExpanseMap;

try (SyncExpanseMap map = new SyncExpanseMap()) {
    // Writer thread
    map.put(42L, 1000L);

    // Reader thread (lock-free, zero writer contention)
    try (SyncExpanseMap.Reader reader = map.reader()) {
        reader.get(42L).ifPresent(v -> System.out.println("Read value: " + v));
    }
}
```

---

## Native Library Resolution

`NativeLoader` automatically extracts and loads the precompiled native library bundled inside the JAR for:
- Linux (`x86_64`, `aarch64`)
- macOS (`aarch64` Apple Silicon, `x86_64` Intel)
- Windows (`x86_64`)

To supply an external custom build of `libexpanse`, specify `-Dexpanse.library.path=/path/to/libexpanse.so` or the `EXPANSE_LIBRARY_PATH` environment variable.

---

## Documentation

See [docs/BINDINGS_JAVA.md](../../docs/BINDINGS_JAVA.md) for full architectural deep dive, Spark/Flink integration patterns, and GC benchmarking.
