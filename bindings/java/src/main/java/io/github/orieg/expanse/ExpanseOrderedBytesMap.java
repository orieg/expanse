package io.github.orieg.expanse;

import io.github.orieg.expanse.internal.ExpanseNative;
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;
import java.util.Objects;
import java.util.Optional;
import java.util.OptionalLong;
import java.util.function.BiConsumer;

/**
 * High-performance off-heap <em>ordered</em> map of arbitrary byte slices to 64-bit values.
 * <p>
 * Keys are compared as unsigned bytes in lexicographic order, a shorter key sorting before any
 * longer key it prefixes. Keys may be empty and may contain any byte, including {@code 0x00} and
 * {@code 0xFF}. Unlike {@link ExpanseBytesMap} (unordered, cf. JudyHS) this map supports ordered
 * navigation: {@link #firstEntry()}, {@link #lastEntry()}, {@link #ceilingEntry(byte[])},
 * {@link #higherEntry(byte[])}, {@link #floorEntry(byte[])} and {@link #lowerEntry(byte[])}.
 * <p>
 * Navigation writes the matching key into a caller-allocated native buffer, so a key is never
 * truncated: when the buffer is too small the native call reports the exact length needed and the
 * wrapper retries with a buffer of that size.
 */
public final class ExpanseOrderedBytesMap implements AutoCloseable {

    private static final int DEFAULT_BUF_LEN = 256;
    private static final ThreadLocal<MemorySegment> SCRATCH =
            ThreadLocal.withInitial(() -> Arena.ofAuto().allocate(ValueLayout.JAVA_LONG, 2));

    // expanse_ordered_bytes_nav_status values (see expanse.h).
    private static final int NAV_OK = 0;
    private static final int NAV_NOT_FOUND = 1;
    private static final int NAV_BUFFER_TOO_SMALL = 2;

    /**
     * Scopes every slot segment this map hands out to the map's own lifetime, so reading a slot
     * after {@link #close()} throws {@link IllegalStateException} instead of returning freed
     * memory. This scopes <em>close</em>, not <em>mutation</em>: a slot is valid only until the
     * next structural mutation, and that contract remains the caller's to honour.
     */
    private final Arena slotLifetime = Arena.ofShared();

    private MemorySegment handle;
    private boolean closed = false;

    /**
     * Immutable key-value pair returned by ordered navigation.
     * <p>
     * The key array is owned by the entry; callers must not mutate it. Record equality on the
     * array component is by reference, so compare keys with {@link java.util.Arrays#equals(byte[], byte[])}.
     *
     * @param key byte key (a fresh array per entry)
     * @param value 64-bit value
     */
    public record Entry(byte[] key, long value) {}

    /**
     * Creates a new empty off-heap {@link ExpanseOrderedBytesMap}.
     */
    public ExpanseOrderedBytesMap() {
        try {
            this.handle = (MemorySegment) ExpanseNative.MH_expanse_ordered_bytesmap_new.invokeExact();
            if (handle.equals(MemorySegment.NULL)) {
                throw new OutOfMemoryError("Failed to allocate native expanse_ordered_bytesmap_t");
            }
        } catch (RuntimeException | Error e) {
            throw e;
        } catch (Throwable t) {
            throw new RuntimeException("Failed creating ExpanseOrderedBytesMap", t);
        }
    }

    private void checkOpen() {
        if (closed || handle.equals(MemorySegment.NULL)) {
            throw new IllegalStateException("ExpanseOrderedBytesMap has been closed");
        }
    }

    /**
     * Inserts or updates a key-to-value mapping.
     *
     * @param key byte array key
     * @param value 64-bit value
     * @return true if key was newly inserted, false if replaced
     */
    public boolean put(byte[] key, long value) {
        return insert(key, value);
    }

    /**
     * Inserts or updates a key-to-value mapping.
     *
     * @param key byte array key
     * @param value 64-bit value
     * @return true if key was newly inserted, false if replaced
     */
    public boolean insert(byte[] key, long value) {
        Objects.requireNonNull(key, "key must not be null");
        checkOpen();
        try (Arena arena = Arena.ofConfined()) {
            return insert(keySegment(arena, key), key.length, value);
        }
    }

    /**
     * Inserts or updates a {@link MemorySegment} key-to-value mapping.
     *
     * @param keySegment raw memory segment containing key bytes
     * @param len byte length
     * @param value 64-bit value
     * @return true if key was newly inserted
     */
    public boolean insert(MemorySegment keySegment, long len, long value) {
        checkOpen();
        try {
            return (boolean) ExpanseNative.MH_expanse_ordered_bytesmap_insert.invokeExact(
                    handle, keySegment, len, value, MemorySegment.NULL);
        } catch (RuntimeException | Error e) {
            throw e;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Retrieves the value for a byte array key.
     *
     * @param key byte array
     * @return OptionalLong containing the value if present
     */
    public OptionalLong get(byte[] key) {
        Objects.requireNonNull(key, "key must not be null");
        checkOpen();
        try (Arena arena = Arena.ofConfined()) {
            return get(keySegment(arena, key), key.length);
        }
    }

    /**
     * Retrieves the value for a {@link MemorySegment} key.
     *
     * @param keySegment raw memory segment
     * @param len byte length
     * @return OptionalLong containing the value if present
     */
    public OptionalLong get(MemorySegment keySegment, long len) {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        try {
            boolean found = (boolean) ExpanseNative.MH_expanse_ordered_bytesmap_get.invokeExact(
                    handle, keySegment, len, scratch);
            return found ? OptionalLong.of(scratch.get(ValueLayout.JAVA_LONG, 0)) : OptionalLong.empty();
        } catch (RuntimeException | Error e) {
            throw e;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Checks if the map contains the given byte array key.
     *
     * @param key byte array
     * @return true if present
     */
    public boolean containsKey(byte[] key) {
        Objects.requireNonNull(key, "key must not be null");
        checkOpen();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment seg = keySegment(arena, key);
            return (boolean) ExpanseNative.MH_expanse_ordered_bytesmap_contains.invokeExact(
                    handle, seg, (long) key.length);
        } catch (RuntimeException | Error e) {
            throw e;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Removes the byte array key from the map.
     *
     * @param key byte array
     * @return true if key was present and removed
     */
    public boolean remove(byte[] key) {
        Objects.requireNonNull(key, "key must not be null");
        checkOpen();
        try (Arena arena = Arena.ofConfined()) {
            return remove(keySegment(arena, key), key.length);
        }
    }

    /**
     * Removes the {@link MemorySegment} key from the map.
     *
     * @param keySegment memory segment
     * @param len byte length
     * @return true if key was present and removed
     */
    public boolean remove(MemorySegment keySegment, long len) {
        checkOpen();
        try {
            return (boolean) ExpanseNative.MH_expanse_ordered_bytesmap_remove.invokeExact(
                    handle, keySegment, len, MemorySegment.NULL);
        } catch (RuntimeException | Error e) {
            throw e;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns a direct writable {@link MemorySegment} (8 bytes) to the value slot of {@code key},
     * or {@code null} if absent. Valid until the next structural mutation of this map.
     *
     * @param key byte array
     * @return slot segment or null
     */
    public MemorySegment slot(byte[] key) {
        Objects.requireNonNull(key, "key must not be null");
        checkOpen();
        try (Arena arena = Arena.ofConfined()) {
            return slot(keySegment(arena, key), key.length);
        }
    }

    /**
     * Returns a direct writable {@link MemorySegment} (8 bytes) to the value slot, or {@code null}
     * if absent. Valid until the next structural mutation of this map.
     *
     * @param keySegment memory segment
     * @param len byte length
     * @return slot segment or null
     */
    public MemorySegment slot(MemorySegment keySegment, long len) {
        checkOpen();
        try {
            MemorySegment ptr = (MemorySegment) ExpanseNative.MH_expanse_ordered_bytesmap_slot.invokeExact(
                    handle, keySegment, len);
            return ptr.equals(MemorySegment.NULL)
                    ? null
                    : ptr.reinterpret(ValueLayout.JAVA_LONG.byteSize(), slotLifetime, null);
        } catch (RuntimeException | Error e) {
            throw e;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Inserts the key with value 0 if absent (an existing value is kept) and returns a direct
     * writable value slot. Valid until the next structural mutation of this map.
     *
     * @param key byte array
     * @return direct slot segment
     */
    public MemorySegment insertSlot(byte[] key) {
        Objects.requireNonNull(key, "key must not be null");
        checkOpen();
        try (Arena arena = Arena.ofConfined()) {
            return insertSlot(keySegment(arena, key), key.length);
        }
    }

    /**
     * Inserts the {@link MemorySegment} key with value 0 if absent and returns a direct writable
     * value slot. Valid until the next structural mutation of this map.
     *
     * @param keySegment memory segment
     * @param len byte length
     * @return direct slot segment
     */
    public MemorySegment insertSlot(MemorySegment keySegment, long len) {
        checkOpen();
        try {
            MemorySegment ptr = (MemorySegment) ExpanseNative.MH_expanse_ordered_bytesmap_ins_slot.invokeExact(
                    handle, keySegment, len);
            if (ptr.equals(MemorySegment.NULL)) {
                throw new OutOfMemoryError("Failed allocating slot");
            }
            return ptr.reinterpret(ValueLayout.JAVA_LONG.byteSize(), slotLifetime, null);
        } catch (RuntimeException | Error e) {
            throw e;
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
            return (long) ExpanseNative.MH_expanse_ordered_bytesmap_len.invokeExact(handle);
        } catch (RuntimeException | Error e) {
            throw e;
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
     * Returns native heap bytes used by the live structure of this map.
     *
     * @return byte count
     */
    public long memUsed() {
        checkOpen();
        try {
            return (long) ExpanseNative.MH_expanse_ordered_bytesmap_mem_used.invokeExact(handle);
        } catch (RuntimeException | Error e) {
            throw e;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns native heap bytes held by this map (live plus retained free capacity).
     *
     * @return byte count
     */
    public long memHeld() {
        checkOpen();
        try {
            return (long) ExpanseNative.MH_expanse_ordered_bytesmap_mem_held.invokeExact(handle);
        } catch (RuntimeException | Error e) {
            throw e;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Releases retained free capacity back to the allocator.
     *
     * @return bytes released
     */
    public long shrinkToFit() {
        checkOpen();
        try {
            return (long) ExpanseNative.MH_expanse_ordered_bytesmap_shrink_to_fit.invokeExact(handle);
        } catch (RuntimeException | Error e) {
            throw e;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Removes all entries from this map.
     */
    public void clear() {
        checkOpen();
        try {
            ExpanseNative.MH_expanse_ordered_bytesmap_clear.invokeExact(handle);
        } catch (RuntimeException | Error e) {
            throw e;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /**
     * Returns the entry with the lexicographically smallest key.
     *
     * @return first entry or empty
     */
    public Optional<Entry> firstEntry() {
        return navNoKey(ExpanseNative.MH_expanse_ordered_bytesmap_first);
    }

    /**
     * Returns the entry with the lexicographically largest key.
     *
     * @return last entry or empty
     */
    public Optional<Entry> lastEntry() {
        return navNoKey(ExpanseNative.MH_expanse_ordered_bytesmap_last);
    }

    /**
     * Returns the entry with the smallest key greater than or equal to {@code key}.
     *
     * @param key search key
     * @return matching entry or empty
     */
    public Optional<Entry> ceilingEntry(byte[] key) {
        return navFromKey(ExpanseNative.MH_expanse_ordered_bytesmap_next_at_or_after, key);
    }

    /**
     * Returns the entry with the smallest key strictly greater than {@code key}.
     *
     * @param key search key
     * @return matching entry or empty
     */
    public Optional<Entry> higherEntry(byte[] key) {
        return navFromKey(ExpanseNative.MH_expanse_ordered_bytesmap_next_after, key);
    }

    /**
     * Returns the entry with the largest key less than or equal to {@code key}.
     *
     * @param key search key
     * @return matching entry or empty
     */
    public Optional<Entry> floorEntry(byte[] key) {
        return navFromKey(ExpanseNative.MH_expanse_ordered_bytesmap_prev_at_or_before, key);
    }

    /**
     * Returns the entry with the largest key strictly less than {@code key}.
     *
     * @param key search key
     * @return matching entry or empty
     */
    public Optional<Entry> lowerEntry(byte[] key) {
        return navFromKey(ExpanseNative.MH_expanse_ordered_bytesmap_prev_before, key);
    }

    /**
     * Iterates over all entries in ascending key order.
     * <p>
     * Each step re-navigates from the previous key, so a mutation made between steps is observed
     * by the following steps.
     *
     * @param action consumer for key and value
     */
    public void forEach(BiConsumer<byte[], Long> action) {
        Objects.requireNonNull(action);
        Optional<Entry> opt = firstEntry();
        while (opt.isPresent()) {
            Entry e = opt.get();
            action.accept(e.key(), e.value());
            opt = higherEntry(e.key());
        }
    }

    /** Allocates a native copy of {@code key}; an empty key is the NULL segment. */
    private static MemorySegment keySegment(Arena arena, byte[] key) {
        if (key.length == 0) {
            return MemorySegment.NULL;
        }
        MemorySegment seg = arena.allocate(ValueLayout.JAVA_BYTE, key.length);
        MemorySegment.copy(key, 0, seg, ValueLayout.JAVA_BYTE, 0, key.length);
        return seg;
    }

    private static int growBuffer(int currentLen, long required) {
        if (required < 0 || required > Integer.MAX_VALUE) {
            throw new IllegalStateException(
                    "Byte key of " + Long.toUnsignedString(required)
                    + " bytes exceeds the maximum Java buffer size (" + Integer.MAX_VALUE + ")");
        }
        // required is exact; still grow at least geometrically so a map mutated between
        // retries cannot cause one-byte-at-a-time growth.
        int doubled = currentLen <= Integer.MAX_VALUE / 2 ? currentLen * 2 : Integer.MAX_VALUE;
        return Math.max((int) required, doubled);
    }

    /** Retry-loop driver for the key-less navigation calls (first/last). */
    private Optional<Entry> navNoKey(MethodHandle mh) {
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        int bufLen = DEFAULT_BUF_LEN;
        try {
            while (true) {
                try (Arena arena = Arena.ofConfined()) {
                    MemorySegment buf = arena.allocate(ValueLayout.JAVA_BYTE, bufLen);
                    MemorySegment reqLen = arena.allocate(ValueLayout.JAVA_LONG);
                    int status = (int) mh.invokeExact(handle, buf, (long) bufLen, reqLen, scratch);
                    switch (status) {
                        case NAV_OK -> {
                            return Optional.of(decode(buf, reqLen, scratch));
                        }
                        case NAV_NOT_FOUND -> {
                            return Optional.empty();
                        }
                        case NAV_BUFFER_TOO_SMALL ->
                            bufLen = growBuffer(bufLen, reqLen.get(ValueLayout.JAVA_LONG, 0));
                        default -> throw new IllegalStateException(
                                "Unknown expanse_ordered_bytes_nav_status: " + status);
                    }
                }
            }
        } catch (RuntimeException | Error e) {
            throw e;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /** Retry-loop driver for the key-taking navigation calls (next/prev). */
    private Optional<Entry> navFromKey(MethodHandle mh, byte[] key) {
        Objects.requireNonNull(key, "key must not be null");
        checkOpen();
        MemorySegment scratch = SCRATCH.get();
        int bufLen = DEFAULT_BUF_LEN;
        try {
            while (true) {
                try (Arena arena = Arena.ofConfined()) {
                    MemorySegment search = keySegment(arena, key);
                    MemorySegment buf = arena.allocate(ValueLayout.JAVA_BYTE, bufLen);
                    MemorySegment reqLen = arena.allocate(ValueLayout.JAVA_LONG);
                    int status = (int) mh.invokeExact(
                            handle, search, (long) key.length, buf, (long) bufLen, reqLen, scratch);
                    switch (status) {
                        case NAV_OK -> {
                            return Optional.of(decode(buf, reqLen, scratch));
                        }
                        case NAV_NOT_FOUND -> {
                            return Optional.empty();
                        }
                        case NAV_BUFFER_TOO_SMALL ->
                            bufLen = growBuffer(bufLen, reqLen.get(ValueLayout.JAVA_LONG, 0));
                        default -> throw new IllegalStateException(
                                "Unknown expanse_ordered_bytes_nav_status: " + status);
                    }
                }
            }
        } catch (RuntimeException | Error e) {
            throw e;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    private static Entry decode(MemorySegment buf, MemorySegment reqLen, MemorySegment scratch) {
        int n = (int) reqLen.get(ValueLayout.JAVA_LONG, 0);
        byte[] k = new byte[n];
        MemorySegment.copy(buf, ValueLayout.JAVA_BYTE, 0, k, 0, n);
        return new Entry(k, scratch.get(ValueLayout.JAVA_LONG, 0));
    }

    @Override
    public void close() {
        if (!closed && !handle.equals(MemorySegment.NULL)) {
            try {
                ExpanseNative.MH_expanse_ordered_bytesmap_free.invokeExact(handle);
            } catch (Throwable t) {
                throw new RuntimeException("Failed to free ExpanseOrderedBytesMap", t);
            } finally {
                handle = MemorySegment.NULL;
                closed = true;
                // After the native free, so a slot segment cannot outlive its memory.
                slotLifetime.close();
            }
        }
    }

    @Override
    public String toString() {
        return "ExpanseOrderedBytesMap{size=" + (closed ? "closed" : size()) + "}";
    }
}
