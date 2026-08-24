package io.github.orieg.expanse;

import io.github.orieg.expanse.collections.ExpanseJavaNavigableMap;
import io.github.orieg.expanse.internal.ExpanseNative;
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.util.Iterator;
import java.util.NoSuchElementException;
import java.util.Objects;
import java.util.Optional;
import java.util.OptionalLong;
import java.util.PrimitiveIterator;
import java.util.Spliterator;
import java.util.Spliterators;
import java.util.stream.LongStream;
import java.util.stream.Stream;
import java.util.stream.StreamSupport;

/**
 * High-performance off-heap ordered map of 64-bit integer keys to 64-bit values (cf. JudyL).
 * <p>
 * Zero-allocation primitive lookups, direct value-slot pointer operations,
 * exact memory accounting, O(depth) rank/select, and full bidirectional navigation.
 */
public final class ExpanseMap implements AutoCloseable {

    private static final ThreadLocal<MemorySegment> SCRATCH =
            ThreadLocal.withInitial(() -> Arena.ofAuto().allocate(ValueLayout.JAVA_LONG, 2));

    private MemorySegment handle;
    private boolean closed = false;

    /**
     * Immutable key-value pair record.
     *
     * @param key 64-bit integer key
     * @param value 64-bit integer value
     */
    public record Entry(long key, long value) {}

    /**
     * Primitive functional interface for entry iteration without boxing.
     */
    @FunctionalInterface
    public interface EntryConsumer {
        void accept(long key, long value);
    }

    /**
     * Creates a new empty off-heap {@link ExpanseMap}.
     */
    public ExpanseMap() {
        try {
            this.handle = (MemorySegment) ExpanseNative.MH_expanse_map_new.invokeExact();
            if (handle.equals(MemorySegment.NULL)) {
                throw new OutOfMemoryError("Failed to allocate native expanse_map_t");
            }
        } catch (Throwable t) {
            throw new RuntimeException("Failed creating ExpanseMap", t);
        }
    }

    private void checkOpen() {
        if (closed || handle.equals(MemorySegment.NULL)) {
            throw new IllegalStateException("ExpanseMap has been closed");
        }
    }

    /**
     * Inserts or updates a key-value mapping.
     *
     * @param key the 64-bit key
     * @param value the 64-bit value
     * @return true if key was newly inserted, false if an existing entry was replaced
     */
    public boolean put(long key, long value) {
        return insert(key, value);
    }

    /**
     * Inserts or updates a key-value mapping.
     *
     * @param key the 64-bit key
     * @param value the 64-bit value
     * @return true if key was newly inserted, false if an existing entry was replaced
     */
    public boolean insert(long key, long value) {
        checkOpen();
        try {
            return (boolean) ExpanseNative.MH_expanse_map_insert.invokeExact(handle, key, value, MemorySegment.NULL);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Inserts or updates a key-value mapping, returning the replaced value if present.
     *
     * @param key the 64-bit key
     * @param value the 64-bit value
     * @return OptionalLong with the previous value if present
     */
    public OptionalLong putAndGetOld(long key, long value) {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        try {
            boolean isNew = (boolean) ExpanseNative.MH_expanse_map_insert.invokeExact(handle, key, value, scratch);
            return isNew ? OptionalLong.empty() : OptionalLong.of(scratch.get(ValueLayout.JAVA_LONG, 0));
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Retrieves the 64-bit value associated with the given key.
     *
     * @param key the search key
     * @return OptionalLong containing the value if present
     */
    public OptionalLong get(long key) {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        try {
            boolean found = (boolean) ExpanseNative.MH_expanse_map_get.invokeExact(handle, key, scratch);
            return found ? OptionalLong.of(scratch.get(ValueLayout.JAVA_LONG, 0)) : OptionalLong.empty();
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Retrieves the 64-bit value associated with key, or returns defaultValue if absent.
     *
     * @param key the search key
     * @param defaultValue value to return if key is absent
     * @return found value or defaultValue
     */
    public long getOrDefault(long key, long defaultValue) {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        try {
            boolean found = (boolean) ExpanseNative.MH_expanse_map_get.invokeExact(handle, key, scratch);
            return found ? scratch.get(ValueLayout.JAVA_LONG, 0) : defaultValue;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Tests if the map contains the given key.
     *
     * @param key search key
     * @return true if present
     */
    public boolean containsKey(long key) {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        try {
            return (boolean) ExpanseNative.MH_expanse_map_get.invokeExact(handle, key, scratch);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Removes the mapping for the given key.
     *
     * @param key the key to remove
     * @return true if key was present and removed
     */
    public boolean remove(long key) {
        checkOpen();
        try {
            return (boolean) ExpanseNative.MH_expanse_map_remove.invokeExact(handle, key, MemorySegment.NULL);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Removes the mapping for the given key, returning its former value.
     *
     * @param key the key to remove
     * @return OptionalLong containing the removed value if present
     */
    public OptionalLong removeAndGetOld(long key) {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        try {
            boolean found = (boolean) ExpanseNative.MH_expanse_map_remove.invokeExact(handle, key, scratch);
            return found ? OptionalLong.of(scratch.get(ValueLayout.JAVA_LONG, 0)) : OptionalLong.empty();
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns a direct writable {@link MemorySegment} (8 bytes) to the value slot of {@code key},
     * or {@code null} if the key is absent.
     * <p>
     * <b>SAFETY:</b> The returned segment pointer remains valid only until the next structural mutation
     * (insert or remove) on this map.
     *
     * @param key search key
     * @return direct slot segment or null
     */
    public MemorySegment slot(long key) {
        checkOpen();
        try {
            MemorySegment ptr = (MemorySegment) ExpanseNative.MH_expanse_map_slot.invokeExact(handle, key);
            return ptr.equals(MemorySegment.NULL) ? null : ptr.reinterpret(ValueLayout.JAVA_LONG.byteSize());
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Inserts {@code key} with value 0 if absent (keeping existing value) and returns
     * a direct writable {@link MemorySegment} (8 bytes) to the value slot.
     * <p>
     * <b>SAFETY:</b> The returned segment pointer remains valid only until the next structural mutation
     * (insert or remove) on this map.
     *
     * @param key key to ensure and lookup
     * @return direct slot segment
     */
    public MemorySegment insertSlot(long key) {
        checkOpen();
        try {
            MemorySegment ptr = (MemorySegment) ExpanseNative.MH_expanse_map_ins_slot.invokeExact(handle, key);
            if (ptr.equals(MemorySegment.NULL)) {
                throw new OutOfMemoryError("Failed to allocate slot for key " + key);
            }
            return ptr.reinterpret(ValueLayout.JAVA_LONG.byteSize());
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns the number of key-value mappings in this map.
     *
     * @return entry count
     */
    public long size() {
        return len();
    }

    /**
     * Returns the number of key-value mappings in this map.
     *
     * @return entry count
     */
    public long len() {
        checkOpen();
        try {
            return (long) ExpanseNative.MH_expanse_map_len.invokeExact(handle);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Checks if the map is empty.
     *
     * @return true if size == 0
     */
    public boolean isEmpty() {
        return size() == 0;
    }

    /**
     * Returns the exact off-heap memory in bytes used by this map.
     *
     * @return bytes of native heap memory used
     */
    public long memoryUsed() {
        return memUsed();
    }

    /**
     * Returns the exact off-heap memory in bytes used by this map.
     *
     * @return bytes of native heap memory used
     */
    public long memUsed() {
        checkOpen();
        try {
            return (long) ExpanseNative.MH_expanse_map_mem_used.invokeExact(handle);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Removes all key-value mappings from this map.
     */
    public void clear() {
        checkOpen();
        try {
            ExpanseNative.MH_expanse_map_clear.invokeExact(handle);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns the smallest key in the map.
     *
     * @return first key or empty
     */
    public OptionalLong firstKey() {
        return firstEntry().map(e -> OptionalLong.of(e.key())).orElseGet(OptionalLong::empty);
    }

    /**
     * Returns the smallest entry in the map.
     *
     * @return first entry or empty
     */
    public Optional<Entry> firstEntry() {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        MemorySegment keyOut = scratch.asSlice(0, ValueLayout.JAVA_LONG.byteSize());
        MemorySegment valOut = scratch.asSlice(ValueLayout.JAVA_LONG.byteSize(), ValueLayout.JAVA_LONG.byteSize());
        try {
            boolean found = (boolean) ExpanseNative.MH_expanse_map_first.invokeExact(handle, keyOut, valOut);
            return found ? Optional.of(new Entry(keyOut.get(ValueLayout.JAVA_LONG, 0), valOut.get(ValueLayout.JAVA_LONG, 0))) : Optional.empty();
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns the largest key in the map.
     *
     * @return last key or empty
     */
    public OptionalLong lastKey() {
        return lastEntry().map(e -> OptionalLong.of(e.key())).orElseGet(OptionalLong::empty);
    }

    /**
     * Returns the largest entry in the map.
     *
     * @return last entry or empty
     */
    public Optional<Entry> lastEntry() {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        MemorySegment keyOut = scratch.asSlice(0, ValueLayout.JAVA_LONG.byteSize());
        MemorySegment valOut = scratch.asSlice(ValueLayout.JAVA_LONG.byteSize(), ValueLayout.JAVA_LONG.byteSize());
        try {
            boolean found = (boolean) ExpanseNative.MH_expanse_map_last.invokeExact(handle, keyOut, valOut);
            return found ? Optional.of(new Entry(keyOut.get(ValueLayout.JAVA_LONG, 0), valOut.get(ValueLayout.JAVA_LONG, 0))) : Optional.empty();
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns the smallest key &gt;= {@code key} (ceiling).
     *
     * @param key search key
     * @return ceiling key
     */
    public OptionalLong ceilingKey(long key) {
        return ceilingEntry(key).map(e -> OptionalLong.of(e.key())).orElseGet(OptionalLong::empty);
    }

    /**
     * Returns the entry with smallest key &gt;= {@code key} (ceiling).
     *
     * @param key search key
     * @return ceiling entry
     */
    public Optional<Entry> ceilingEntry(long key) {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        MemorySegment keyOut = scratch.asSlice(0, ValueLayout.JAVA_LONG.byteSize());
        MemorySegment valOut = scratch.asSlice(ValueLayout.JAVA_LONG.byteSize(), ValueLayout.JAVA_LONG.byteSize());
        try {
            boolean found = (boolean) ExpanseNative.MH_expanse_map_next_at_or_after.invokeExact(handle, key, keyOut, valOut);
            return found ? Optional.of(new Entry(keyOut.get(ValueLayout.JAVA_LONG, 0), valOut.get(ValueLayout.JAVA_LONG, 0))) : Optional.empty();
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns the smallest key &gt; {@code key} (higher).
     *
     * @param key search key
     * @return higher key
     */
    public OptionalLong higherKey(long key) {
        return higherEntry(key).map(e -> OptionalLong.of(e.key())).orElseGet(OptionalLong::empty);
    }

    /**
     * Returns the entry with smallest key &gt; {@code key} (higher).
     *
     * @param key search key
     * @return higher entry
     */
    public Optional<Entry> higherEntry(long key) {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        MemorySegment keyOut = scratch.asSlice(0, ValueLayout.JAVA_LONG.byteSize());
        MemorySegment valOut = scratch.asSlice(ValueLayout.JAVA_LONG.byteSize(), ValueLayout.JAVA_LONG.byteSize());
        try {
            boolean found = (boolean) ExpanseNative.MH_expanse_map_next_after.invokeExact(handle, key, keyOut, valOut);
            return found ? Optional.of(new Entry(keyOut.get(ValueLayout.JAVA_LONG, 0), valOut.get(ValueLayout.JAVA_LONG, 0))) : Optional.empty();
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns the largest key &lt;= {@code key} (floor).
     *
     * @param key search key
     * @return floor key
     */
    public OptionalLong floorKey(long key) {
        return floorEntry(key).map(e -> OptionalLong.of(e.key())).orElseGet(OptionalLong::empty);
    }

    /**
     * Returns the entry with largest key &lt;= {@code key} (floor).
     *
     * @param key search key
     * @return floor entry
     */
    public Optional<Entry> floorEntry(long key) {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        MemorySegment keyOut = scratch.asSlice(0, ValueLayout.JAVA_LONG.byteSize());
        MemorySegment valOut = scratch.asSlice(ValueLayout.JAVA_LONG.byteSize(), ValueLayout.JAVA_LONG.byteSize());
        try {
            boolean found = (boolean) ExpanseNative.MH_expanse_map_prev_at_or_before.invokeExact(handle, key, keyOut, valOut);
            return found ? Optional.of(new Entry(keyOut.get(ValueLayout.JAVA_LONG, 0), valOut.get(ValueLayout.JAVA_LONG, 0))) : Optional.empty();
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns the largest key &lt; {@code key} (lower).
     *
     * @param key search key
     * @return lower key
     */
    public OptionalLong lowerKey(long key) {
        return lowerEntry(key).map(e -> OptionalLong.of(e.key())).orElseGet(OptionalLong::empty);
    }

    /**
     * Returns the entry with largest key &lt; {@code key} (lower).
     *
     * @param key search key
     * @return lower entry
     */
    public Optional<Entry> lowerEntry(long key) {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        MemorySegment keyOut = scratch.asSlice(0, ValueLayout.JAVA_LONG.byteSize());
        MemorySegment valOut = scratch.asSlice(ValueLayout.JAVA_LONG.byteSize(), ValueLayout.JAVA_LONG.byteSize());
        try {
            boolean found = (boolean) ExpanseNative.MH_expanse_map_prev_before.invokeExact(handle, key, keyOut, valOut);
            return found ? Optional.of(new Entry(keyOut.get(ValueLayout.JAVA_LONG, 0), valOut.get(ValueLayout.JAVA_LONG, 0))) : Optional.empty();
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Number of entries strictly below {@code key} (O(depth) rank).
     *
     * @param key threshold key
     * @return count of keys &lt; key
     */
    public long countBelow(long key) {
        checkOpen();
        try {
            return (long) ExpanseNative.MH_expanse_map_count_below.invokeExact(handle, key);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Number of entries in inclusive range {@code [lo, hi]} (O(depth) rank).
     *
     * @param lo lower bound (inclusive)
     * @param hi upper bound (inclusive)
     * @return count of keys in range
     */
    public long countRange(long lo, long hi) {
        checkOpen();
        try {
            return (long) ExpanseNative.MH_expanse_map_count_range.invokeExact(handle, lo, hi);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns the entry with exactly {@code n} entries below it (0-based select, O(depth)).
     *
     * @param n 0-based index
     * @return n-th entry
     */
    public Optional<Entry> byCount(long n) {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        MemorySegment keyOut = scratch.asSlice(0, ValueLayout.JAVA_LONG.byteSize());
        MemorySegment valOut = scratch.asSlice(ValueLayout.JAVA_LONG.byteSize(), ValueLayout.JAVA_LONG.byteSize());
        try {
            boolean found = (boolean) ExpanseNative.MH_expanse_map_by_count.invokeExact(handle, n, keyOut, valOut);
            return found ? Optional.of(new Entry(keyOut.get(ValueLayout.JAVA_LONG, 0), valOut.get(ValueLayout.JAVA_LONG, 0))) : Optional.empty();
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Iterates through all key-value entries in ascending key order without heap allocation.
     *
     * @param action consumer for each key-value pair
     */
    public void forEach(EntryConsumer action) {
        Objects.requireNonNull(action);
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        MemorySegment keyOut = scratch.asSlice(0, ValueLayout.JAVA_LONG.byteSize());
        MemorySegment valOut = scratch.asSlice(ValueLayout.JAVA_LONG.byteSize(), ValueLayout.JAVA_LONG.byteSize());
        try {
            boolean hasNext = (boolean) ExpanseNative.MH_expanse_map_first.invokeExact(handle, keyOut, valOut);
            while (hasNext) {
                long k = keyOut.get(ValueLayout.JAVA_LONG, 0);
                long v = valOut.get(ValueLayout.JAVA_LONG, 0);
                action.accept(k, v);
                hasNext = (boolean) ExpanseNative.MH_expanse_map_next_after.invokeExact(handle, k, keyOut, valOut);
            }
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns a zero-allocation primitive {@link PrimitiveIterator.OfLong} over keys.
     *
     * @return key iterator
     */
    public PrimitiveIterator.OfLong keyIterator() {
        checkOpen();
        return new PrimitiveIterator.OfLong() {
            private long nextKey;
            private boolean hasNext;
            private boolean initialized = false;

            private void advance() {
                if (!initialized) {
                    OptionalLong first = firstKey();
                    hasNext = first.isPresent();
                    if (hasNext) {
                        nextKey = first.getAsLong();
                    }
                    initialized = true;
                }
            }

            @Override
            public boolean hasNext() {
                advance();
                return hasNext;
            }

            @Override
            public long nextLong() {
                advance();
                if (!hasNext) {
                    throw new NoSuchElementException();
                }
                long result = nextKey;
                OptionalLong after = higherKey(result);
                hasNext = after.isPresent();
                if (hasNext) {
                    nextKey = after.getAsLong();
                }
                return result;
            }
        };
    }

    /**
     * Returns an iterator over {@link Entry} records.
     *
     * @return entry iterator
     */
    public Iterator<Entry> entryIterator() {
        checkOpen();
        return new Iterator<>() {
            private Entry nextEntry;
            private boolean hasNext;
            private boolean initialized = false;

            private void advance() {
                if (!initialized) {
                    Optional<Entry> first = firstEntry();
                    hasNext = first.isPresent();
                    if (hasNext) {
                        nextEntry = first.get();
                    }
                    initialized = true;
                }
            }

            @Override
            public boolean hasNext() {
                advance();
                return hasNext;
            }

            @Override
            public Entry next() {
                advance();
                if (!hasNext) {
                    throw new NoSuchElementException();
                }
                Entry result = nextEntry;
                Optional<Entry> after = higherEntry(result.key());
                hasNext = after.isPresent();
                if (hasNext) {
                    nextEntry = after.get();
                }
                return result;
            }
        };
    }

    /**
     * Returns a primitive {@link LongStream} of keys.
     *
     * @return LongStream of keys
     */
    public LongStream keyStream() {
        // NOTE: keys are emitted in UNSIGNED 64-bit order. We must NOT advertise
        // Spliterator.SORTED here: a primitive LongStream spliterator cannot carry a
        // custom comparator, so SORTED implies natural (signed) order and would cause
        // LongStream.sorted() to be wrongly elided, leaving keys >= 2^63 misordered.
        return StreamSupport.longStream(
                Spliterators.spliterator(keyIterator(), size(),
                        Spliterator.DISTINCT | Spliterator.ORDERED | Spliterator.NONNULL),
                false);
    }

    /**
     * Returns a {@link Stream} of {@link Entry} records.
     *
     * @return Entry stream
     */
    public Stream<Entry> entryStream() {
        return StreamSupport.stream(
                Spliterators.spliterator(entryIterator(), size(),
                        Spliterator.DISTINCT | Spliterator.ORDERED | Spliterator.NONNULL),
                false);
    }

    /**
     * Returns a standard {@link java.util.NavigableMap} view backed by this native map.
     *
     * @return NavigableMap wrapper
     */
    public java.util.NavigableMap<Long, Long> asJavaMap() {
        return new ExpanseJavaNavigableMap(this);
    }

    @Override
    public void close() {
        if (!closed && !handle.equals(MemorySegment.NULL)) {
            try {
                ExpanseNative.MH_expanse_map_free.invokeExact(handle);
            } catch (Throwable t) {
                throw new RuntimeException("Failed to free ExpanseMap", t);
            } finally {
                handle = MemorySegment.NULL;
                closed = true;
            }
        }
    }

    @Override
    public String toString() {
        return "ExpanseMap{size=" + (closed ? "closed" : size()) + ", memUsed=" + (closed ? 0 : memoryUsed()) + "B}";
    }
}
