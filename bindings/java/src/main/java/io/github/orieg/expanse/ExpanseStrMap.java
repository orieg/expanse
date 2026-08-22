package io.github.orieg.expanse;

import io.github.orieg.expanse.collections.ExpanseJavaStrMap;
import io.github.orieg.expanse.internal.ExpanseNative;
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.nio.charset.StandardCharsets;
import java.util.Objects;
import java.util.Optional;
import java.util.OptionalLong;
import java.util.function.BiConsumer;

/**
 * High-performance off-heap ordered map of null-terminated C strings to 64-bit values (cf. JudySL).
 */
public final class ExpanseStrMap implements AutoCloseable {

    private static final int DEFAULT_BUF_LEN = 1024;
    private static final ThreadLocal<MemorySegment> SCRATCH =
            ThreadLocal.withInitial(() -> Arena.ofAuto().allocate(ValueLayout.JAVA_LONG, 2));

    private MemorySegment handle;
    private boolean closed = false;

    /**
     * Immutable String-value pair record.
     *
     * @param key String key
     * @param value 64-bit integer value
     */
    public record Entry(String key, long value) {}

    /**
     * Creates a new empty off-heap {@link ExpanseStrMap}.
     */
    public ExpanseStrMap() {
        try {
            this.handle = (MemorySegment) ExpanseNative.MH_expanse_strmap_new.invokeExact();
            if (handle.equals(MemorySegment.NULL)) {
                throw new OutOfMemoryError("Failed to allocate native expanse_strmap_t");
            }
        } catch (Throwable t) {
            throw new RuntimeException("Failed creating ExpanseStrMap", t);
        }
    }

    private void checkOpen() {
        if (closed || handle.equals(MemorySegment.NULL)) {
            throw new IllegalStateException("ExpanseStrMap has been closed");
        }
    }

    /**
     * Inserts or updates a string-to-value mapping.
     *
     * @param key string key (must not contain embedded null characters)
     * @param value 64-bit value
     * @return true if key was newly inserted, false if previous value was replaced
     */
    public boolean put(String key, long value) {
        return insert(key, value);
    }

    /**
     * Inserts or updates a string-to-value mapping.
     *
     * @param key string key
     * @param value 64-bit value
     * @return true if key was newly inserted, false if previous value was replaced
     */
    public boolean insert(String key, long value) {
        Objects.requireNonNull(key, "key must not be null");
        checkOpen();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment cstr = arena.allocateFrom(key, StandardCharsets.UTF_8);
            return (boolean) ExpanseNative.MH_expanse_strmap_insert.invokeExact(handle, cstr, value, MemorySegment.NULL);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Inserts or updates a string-to-value mapping, returning the replaced value if present.
     *
     * @param key string key
     * @param value 64-bit value
     * @return OptionalLong containing previous value if present
     */
    public OptionalLong putAndGetOld(String key, long value) {
        Objects.requireNonNull(key, "key must not be null");
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment cstr = arena.allocateFrom(key, StandardCharsets.UTF_8);
            boolean isNew = (boolean) ExpanseNative.MH_expanse_strmap_insert.invokeExact(handle, cstr, value, scratch);
            return isNew ? OptionalLong.empty() : OptionalLong.of(scratch.get(ValueLayout.JAVA_LONG, 0));
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Retrieves the 64-bit value for the given string key.
     *
     * @param key search key
     * @return OptionalLong containing value if present
     */
    public OptionalLong get(String key) {
        Objects.requireNonNull(key, "key must not be null");
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment cstr = arena.allocateFrom(key, StandardCharsets.UTF_8);
            boolean found = (boolean) ExpanseNative.MH_expanse_strmap_get.invokeExact(handle, cstr, scratch);
            return found ? OptionalLong.of(scratch.get(ValueLayout.JAVA_LONG, 0)) : OptionalLong.empty();
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Checks whether the map contains the given string key.
     *
     * @param key search key
     * @return true if present
     */
    public boolean containsKey(String key) {
        return get(key).isPresent();
    }

    /**
     * Removes the string key from the map.
     *
     * @param key key to remove
     * @return true if key was present and removed
     */
    public boolean remove(String key) {
        Objects.requireNonNull(key, "key must not be null");
        checkOpen();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment cstr = arena.allocateFrom(key, StandardCharsets.UTF_8);
            return (boolean) ExpanseNative.MH_expanse_strmap_remove.invokeExact(handle, cstr, MemorySegment.NULL);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Removes the string key from the map, returning its former value.
     *
     * @param key key to remove
     * @return OptionalLong containing removed value if present
     */
    public OptionalLong removeAndGetOld(String key) {
        Objects.requireNonNull(key, "key must not be null");
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment cstr = arena.allocateFrom(key, StandardCharsets.UTF_8);
            boolean found = (boolean) ExpanseNative.MH_expanse_strmap_remove.invokeExact(handle, cstr, scratch);
            return found ? OptionalLong.of(scratch.get(ValueLayout.JAVA_LONG, 0)) : OptionalLong.empty();
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns a direct writable {@link MemorySegment} (8 bytes) to the value slot of {@code key},
     * or {@code null} if the key is absent.
     * <p>
     * <b>SAFETY:</b> Valid only until the next structural mutation on this map.
     *
     * @param key search key
     * @return direct slot segment or null
     */
    public MemorySegment slot(String key) {
        Objects.requireNonNull(key, "key must not be null");
        checkOpen();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment cstr = arena.allocateFrom(key, StandardCharsets.UTF_8);
            MemorySegment ptr = (MemorySegment) ExpanseNative.MH_expanse_strmap_slot.invokeExact(handle, cstr);
            return ptr.equals(MemorySegment.NULL) ? null : ptr.reinterpret(ValueLayout.JAVA_LONG.byteSize());
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Inserts {@code key} with value 0 if absent and returns a direct writable {@link MemorySegment} (8 bytes).
     *
     * @param key key to ensure
     * @return direct slot segment
     */
    public MemorySegment insertSlot(String key) {
        Objects.requireNonNull(key, "key must not be null");
        checkOpen();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment cstr = arena.allocateFrom(key, StandardCharsets.UTF_8);
            MemorySegment ptr = (MemorySegment) ExpanseNative.MH_expanse_strmap_ins_slot.invokeExact(handle, cstr);
            if (ptr.equals(MemorySegment.NULL)) {
                throw new OutOfMemoryError("Failed allocating slot for key " + key);
            }
            return ptr.reinterpret(ValueLayout.JAVA_LONG.byteSize());
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Number of entries in this map.
     *
     * @return entry count
     */
    public long size() {
        return len();
    }

    /**
     * Number of entries in this map.
     *
     * @return entry count
     */
    public long len() {
        checkOpen();
        try {
            return (long) ExpanseNative.MH_expanse_strmap_len.invokeExact(handle);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Checks whether the map is empty.
     *
     * @return true if size == 0
     */
    public boolean isEmpty() {
        return size() == 0;
    }

    /**
     * Returns exact native heap memory in bytes used by this map.
     *
     * @return byte count
     */
    public long memoryUsed() {
        return memUsed();
    }

    /**
     * Returns exact native heap memory in bytes used by this map.
     *
     * @return byte count
     */
    public long memUsed() {
        checkOpen();
        try {
            return (long) ExpanseNative.MH_expanse_strmap_mem_used.invokeExact(handle);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Clears all entries from this map.
     */
    public void clear() {
        checkOpen();
        try {
            ExpanseNative.MH_expanse_strmap_clear.invokeExact(handle);
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns the lexicographically smallest entry in the map.
     *
     * @return first entry or empty
     */
    public Optional<Entry> firstEntry() {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment buf = arena.allocate(ValueLayout.JAVA_BYTE, DEFAULT_BUF_LEN);
            boolean found = (boolean) ExpanseNative.MH_expanse_strmap_first.invokeExact(
                    handle, buf, (long) DEFAULT_BUF_LEN, scratch);
            if (found) {
                String k = buf.getString(0, StandardCharsets.UTF_8);
                long v = scratch.get(ValueLayout.JAVA_LONG, 0);
                return Optional.of(new Entry(k, v));
            }
            return Optional.empty();
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns the lexicographically largest entry in the map.
     *
     * @return last entry or empty
     */
    public Optional<Entry> lastEntry() {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment buf = arena.allocate(ValueLayout.JAVA_BYTE, DEFAULT_BUF_LEN);
            boolean found = (boolean) ExpanseNative.MH_expanse_strmap_last.invokeExact(
                    handle, buf, (long) DEFAULT_BUF_LEN, scratch);
            if (found) {
                String k = buf.getString(0, StandardCharsets.UTF_8);
                long v = scratch.get(ValueLayout.JAVA_LONG, 0);
                return Optional.of(new Entry(k, v));
            }
            return Optional.empty();
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns the entry with the smallest key strictly greater than {@code key}.
     *
     * @param key search key
     * @return next entry or empty
     */
    public Optional<Entry> nextAfter(String key) {
        Objects.requireNonNull(key, "key must not be null");
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment cstr = arena.allocateFrom(key, StandardCharsets.UTF_8);
            MemorySegment buf = arena.allocate(ValueLayout.JAVA_BYTE, DEFAULT_BUF_LEN);
            boolean found = (boolean) ExpanseNative.MH_expanse_strmap_next_after.invokeExact(
                    handle, cstr, buf, (long) DEFAULT_BUF_LEN, scratch);
            if (found) {
                String k = buf.getString(0, StandardCharsets.UTF_8);
                long v = scratch.get(ValueLayout.JAVA_LONG, 0);
                return Optional.of(new Entry(k, v));
            }
            return Optional.empty();
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns the entry with the largest key strictly less than {@code key}.
     *
     * @param key search key
     * @return prev entry or empty
     */
    public Optional<Entry> prevBefore(String key) {
        Objects.requireNonNull(key, "key must not be null");
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment cstr = arena.allocateFrom(key, StandardCharsets.UTF_8);
            MemorySegment buf = arena.allocate(ValueLayout.JAVA_BYTE, DEFAULT_BUF_LEN);
            boolean found = (boolean) ExpanseNative.MH_expanse_strmap_prev_before.invokeExact(
                    handle, cstr, buf, (long) DEFAULT_BUF_LEN, scratch);
            if (found) {
                String k = buf.getString(0, StandardCharsets.UTF_8);
                long v = scratch.get(ValueLayout.JAVA_LONG, 0);
                return Optional.of(new Entry(k, v));
            }
            return Optional.empty();
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Iterates over all entries in ascending lexicographical order.
     *
     * @param action consumer for key and value
     */
    public void forEach(BiConsumer<String, Long> action) {
        Objects.requireNonNull(action);
        checkOpen();
        Optional<Entry> opt = firstEntry();
        while (opt.isPresent()) {
            Entry e = opt.get();
            action.accept(e.key(), e.value());
            opt = nextAfter(e.key());
        }
    }

    /**
     * Returns a standard {@link java.util.Map} view backed by this native string map.
     *
     * @return Map wrapper
     */
    public java.util.Map<String, Long> asJavaMap() {
        return new ExpanseJavaStrMap(this);
    }

    @Override
    public void close() {
        if (!closed && !handle.equals(MemorySegment.NULL)) {
            try {
                ExpanseNative.MH_expanse_strmap_free.invokeExact(handle);
            } catch (Throwable t) {
                throw new RuntimeException("Failed to free ExpanseStrMap", t);
            } finally {
                handle = MemorySegment.NULL;
                closed = true;
            }
        }
    }

    @Override
    public String toString() {
        return "ExpanseStrMap{size=" + (closed ? "closed" : size()) + ", memUsed=" + (closed ? 0 : memoryUsed()) + "B}";
    }
}
