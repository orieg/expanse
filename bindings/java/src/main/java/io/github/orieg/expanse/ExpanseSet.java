package io.github.orieg.expanse;

import io.github.orieg.expanse.collections.ExpanseJavaNavigableSet;
import io.github.orieg.expanse.internal.ExpanseNative;
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.util.NoSuchElementException;
import java.util.Objects;
import java.util.OptionalLong;
import java.util.PrimitiveIterator;
import java.util.Spliterator;
import java.util.Spliterators;
import java.util.function.LongConsumer;
import java.util.function.LongPredicate;
import java.util.stream.LongStream;
import java.util.stream.StreamSupport;

/**
 * High-performance off-heap ordered set of 64-bit integer keys (cf. Judy1).
 * <p>
 * Backed directly by native {@code expanse_set_t} with zero JVM heap allocations
 * for keys, deterministic memory layout, O(depth) rank/select, and bidirectional navigation.
 */
public final class ExpanseSet implements AutoCloseable, LongPredicate {

    private static final ThreadLocal<MemorySegment> SCRATCH =
            ThreadLocal.withInitial(() -> Arena.ofAuto().allocate(ValueLayout.JAVA_LONG, 2));

    private MemorySegment handle;
    private boolean closed = false;

    /**
     * Creates a new empty off-heap {@link ExpanseSet}.
     */
    public ExpanseSet() {
        try {
            this.handle = (MemorySegment) ExpanseNative.MH_expanse_set_new.invokeExact();
            if (handle.equals(MemorySegment.NULL)) {
                throw new OutOfMemoryError("Failed to allocate native expanse_set_t");
            }
        } catch (Throwable t) {
            throw new RuntimeException("Failed creating ExpanseSet", t);
        }
    }

    private void checkOpen() {
        if (closed || handle.equals(MemorySegment.NULL)) {
            throw new IllegalStateException("ExpanseSet has been closed");
        }
    }

    /**
     * Inserts a 64-bit key into the set.
     *
     * @param key the key to insert
     * @return true if the key was newly inserted, false if already present
     */
    public boolean add(long key) {
        return insert(key);
    }

    /**
     * Inserts a 64-bit key into the set.
     *
     * @param key the key to insert
     * @return true if the key was newly inserted, false if already present
     */
    public boolean insert(long key) {
        checkOpen();
        try {
            return (boolean) ExpanseNative.MH_expanse_set_insert.invokeExact(handle, key);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Removes a 64-bit key from the set.
     *
     * @param key the key to remove
     * @return true if the key was present and removed, false otherwise
     */
    public boolean remove(long key) {
        checkOpen();
        try {
            return (boolean) ExpanseNative.MH_expanse_set_remove.invokeExact(handle, key);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Tests if the set contains the given 64-bit key.
     *
     * @param key the key to check
     * @return true if present, false otherwise
     */
    public boolean contains(long key) {
        checkOpen();
        try {
            return (boolean) ExpanseNative.MH_expanse_set_contains.invokeExact(handle, key);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Checks membership for a batch of keys simultaneously with memory-level parallelism prefetching.
     *
     * @param keys the keys to check
     * @param outPresent boolean array to store presence flags (must be at least keys.length)
     * @return number of keys found
     */
    public long containsBatch(long[] keys, boolean[] outPresent) {
        checkOpen();
        Objects.requireNonNull(keys, "keys array cannot be null");
        Objects.requireNonNull(outPresent, "outPresent array cannot be null");
        if (outPresent.length < keys.length) {
            throw new IllegalArgumentException("outPresent array length must be >= keys length");
        }
        if (keys.length == 0) {
            return 0;
        }
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment kSeg = arena.allocateFrom(ValueLayout.JAVA_LONG, keys);
            MemorySegment pSeg = arena.allocate(ValueLayout.JAVA_BOOLEAN, keys.length);
            long foundCount = (long) ExpanseNative.MH_expanse_set_contains_batch.invokeExact(
                handle, kSeg, pSeg, (long) keys.length
            );
            for (int i = 0; i < keys.length; i++) {
                outPresent[i] = pSeg.get(ValueLayout.JAVA_BOOLEAN, i);
            }
            return foundCount;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    @Override
    public boolean test(long value) {
        return contains(value);
    }

    /**
     * Returns the number of keys in the set.
     *
     * @return key count
     */
    public long size() {
        return len();
    }

    /**
     * Returns the number of keys in the set.
     *
     * @return key count
     */
    public long len() {
        checkOpen();
        try {
            return (long) ExpanseNative.MH_expanse_set_len.invokeExact(handle);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Checks whether the set is empty.
     *
     * @return true if size == 0
     */
    public boolean isEmpty() {
        return size() == 0;
    }

    /**
     * Returns the exact off-heap memory in bytes used by this set.
     *
     * @return bytes of native heap memory used
     */
    public long memoryUsed() {
        return memUsed();
    }

    /**
     * Returns the exact off-heap memory in bytes used by this set.
     *
     * @return bytes of native heap memory used
     */
    public long memUsed() {
        checkOpen();
        try {
            return (long) ExpanseNative.MH_expanse_set_mem_used.invokeExact(handle);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Removes all keys from this set, freeing off-heap nodes.
     */
    public void clear() {
        checkOpen();
        try {
            ExpanseNative.MH_expanse_set_clear.invokeExact(handle);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns the smallest key in the set, or empty if the set is empty.
     *
     * @return OptionalLong containing the first key
     */
    public OptionalLong first() {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        try {
            boolean found = (boolean) ExpanseNative.MH_expanse_set_first.invokeExact(handle, scratch);
            return found ? OptionalLong.of(scratch.get(ValueLayout.JAVA_LONG, 0)) : OptionalLong.empty();
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns the largest key in the set, or empty if the set is empty.
     *
     * @return OptionalLong containing the last key
     */
    public OptionalLong last() {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        try {
            boolean found = (boolean) ExpanseNative.MH_expanse_set_last.invokeExact(handle, scratch);
            return found ? OptionalLong.of(scratch.get(ValueLayout.JAVA_LONG, 0)) : OptionalLong.empty();
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns the smallest key &gt;= {@code key}, or empty if no such key exists.
     *
     * @param key search key
     * @return ceiling key
     */
    public OptionalLong nextAtOrAfter(long key) {
        return ceiling(key);
    }

    /**
     * Returns the smallest key &gt;= {@code key} (ceiling).
     *
     * @param key search key
     * @return ceiling key
     */
    public OptionalLong ceiling(long key) {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        try {
            boolean found = (boolean) ExpanseNative.MH_expanse_set_next_at_or_after.invokeExact(handle, key, scratch);
            return found ? OptionalLong.of(scratch.get(ValueLayout.JAVA_LONG, 0)) : OptionalLong.empty();
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
    public OptionalLong nextAfter(long key) {
        return higher(key);
    }

    /**
     * Returns the smallest key &gt; {@code key} (higher).
     *
     * @param key search key
     * @return higher key
     */
    public OptionalLong higher(long key) {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        try {
            boolean found = (boolean) ExpanseNative.MH_expanse_set_next_after.invokeExact(handle, key, scratch);
            return found ? OptionalLong.of(scratch.get(ValueLayout.JAVA_LONG, 0)) : OptionalLong.empty();
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
    public OptionalLong prevAtOrBefore(long key) {
        return floor(key);
    }

    /**
     * Returns the largest key &lt;= {@code key} (floor).
     *
     * @param key search key
     * @return floor key
     */
    public OptionalLong floor(long key) {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        try {
            boolean found = (boolean) ExpanseNative.MH_expanse_set_prev_at_or_before.invokeExact(handle, key, scratch);
            return found ? OptionalLong.of(scratch.get(ValueLayout.JAVA_LONG, 0)) : OptionalLong.empty();
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
    public OptionalLong prevBefore(long key) {
        return lower(key);
    }

    /**
     * Returns the largest key &lt; {@code key} (lower).
     *
     * @param key search key
     * @return lower key
     */
    public OptionalLong lower(long key) {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        try {
            boolean found = (boolean) ExpanseNative.MH_expanse_set_prev_before.invokeExact(handle, key, scratch);
            return found ? OptionalLong.of(scratch.get(ValueLayout.JAVA_LONG, 0)) : OptionalLong.empty();
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Number of keys strictly below {@code key} (O(depth) rank).
     *
     * @param key threshold key
     * @return count of keys &lt; key
     */
    public long countBelow(long key) {
        checkOpen();
        try {
            return (long) ExpanseNative.MH_expanse_set_count_below.invokeExact(handle, key);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Number of keys in the inclusive range {@code [lo, hi]} (O(depth) rank).
     *
     * @param lo lower bound (inclusive)
     * @param hi upper bound (inclusive)
     * @return count of keys in range
     */
    public long countRange(long lo, long hi) {
        checkOpen();
        try {
            return (long) ExpanseNative.MH_expanse_set_count_range.invokeExact(handle, lo, hi);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns the key with exactly {@code n} keys below it (0-based select, O(depth)).
     *
     * @param n 0-based rank index
     * @return the n-th key
     */
    public OptionalLong byCount(long n) {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        try {
            boolean found = (boolean) ExpanseNative.MH_expanse_set_by_count.invokeExact(handle, n, scratch);
            return found ? OptionalLong.of(scratch.get(ValueLayout.JAVA_LONG, 0)) : OptionalLong.empty();
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Iterates through all keys in ascending order without heap allocation.
     *
     * @param action consumer for each key
     */
    public void forEach(LongConsumer action) {
        Objects.requireNonNull(action);
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        try {
            boolean hasNext = (boolean) ExpanseNative.MH_expanse_set_first.invokeExact(handle, scratch);
            while (hasNext) {
                long current = scratch.get(ValueLayout.JAVA_LONG, 0);
                action.accept(current);
                hasNext = (boolean) ExpanseNative.MH_expanse_set_next_after.invokeExact(handle, current, scratch);
            }
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns a zero-allocation primitive {@link PrimitiveIterator.OfLong} over keys.
     *
     * @return primitive iterator
     */
    public PrimitiveIterator.OfLong iterator() {
        checkOpen();
        return new PrimitiveIterator.OfLong() {
            private long nextKey;
            private boolean hasNext;
            private boolean initialized = false;

            private void advance() {
                if (!initialized) {
                    OptionalLong first = first();
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
                OptionalLong after = nextAfter(result);
                hasNext = after.isPresent();
                if (hasNext) {
                    nextKey = after.getAsLong();
                }
                return result;
            }
        };
    }

    /**
     * Returns a primitive {@link LongStream} of keys.
     *
     * @return sequential LongStream
     */
    public LongStream stream() {
        return StreamSupport.longStream(
                Spliterators.spliterator(iterator(), size(),
                        Spliterator.DISTINCT | Spliterator.ORDERED | Spliterator.SORTED | Spliterator.NONNULL),
                false);
    }

    /**
     * Returns a standard {@link java.util.NavigableSet} view backed by this native set.
     *
     * @return NavigableSet wrapper
     */
    public java.util.NavigableSet<Long> asJavaSet() {
        return new ExpanseJavaNavigableSet(this);
    }

    @Override
    public void close() {
        if (!closed && !handle.equals(MemorySegment.NULL)) {
            try {
                ExpanseNative.MH_expanse_set_free.invokeExact(handle);
            } catch (Throwable t) {
                throw new RuntimeException("Failed to free ExpanseSet", t);
            } finally {
                handle = MemorySegment.NULL;
                closed = true;
            }
        }
    }

    @Override
    public String toString() {
        return "ExpanseSet{size=" + (closed ? "closed" : size()) + ", memUsed=" + (closed ? 0 : memoryUsed()) + "B}";
    }
}
