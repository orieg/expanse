package io.github.orieg.expanse;

import static io.github.orieg.expanse.internal.ExpanseNative.*;

import java.lang.foreign.MemorySegment;
import java.lang.invoke.MethodHandle;
import java.nio.charset.StandardCharsets;
import java.util.Objects;
import java.util.Optional;

/**
 * Multithreaded concurrent ordered map from strings to 64-bit values, with optimistic readers
 * (OCC) beside a serialized writer: the shared form of {@link ExpanseStrMap}.
 * <p>
 * Keys are encoded as UTF-8 and handed to the native map as NUL-terminated C strings, so a key
 * containing {@code U+0000} cannot be stored: every method taking a key rejects it with an
 * {@link IllegalArgumentException} (use {@link SyncExpanseBytesMap} or
 * {@link SyncExpanseOrderedBytesMap} for keys with embedded NULs). Keys order by their UTF-8 bytes.
 * <p>
 * Ordered reads are on a {@link Reader}. A walk built from repeated {@code higherEntry} calls is
 * not a snapshot: it sees every key present throughout it, in order, and may or may not see keys
 * inserted or removed meanwhile.
 */
public final class SyncExpanseStrMap extends SyncKeyedMap<String> {

    /**
     * One entry of the map.
     *
     * @param key the key
     * @param value its value
     */
    public record Entry(String key, long value) {}

    private static final MethodHandle FIRST = noKey(MH_expanse_sync_strmap_reader_first);
    private static final MethodHandle LAST = noKey(MH_expanse_sync_strmap_reader_last);
    private static final MethodHandle CEILING = skip(MH_expanse_sync_strmap_reader_next_at_or_after);
    private static final MethodHandle HIGHER = skip(MH_expanse_sync_strmap_reader_next_after);
    private static final MethodHandle FLOOR = skip(MH_expanse_sync_strmap_reader_prev_at_or_before);
    private static final MethodHandle LOWER = skip(MH_expanse_sync_strmap_reader_prev_before);

    /**
     * A registered reader handle: optimistic reads that never take the writer's lock. Used by
     * one thread at a time and closed before its map (closing the map closes it too).
     */
    public final class Reader extends SyncKeyedMap.BaseReader<String> {
        private Reader(MemorySegment readerHandle) {
            super(SyncExpanseStrMap.this, readerHandle);
        }

        private Optional<Entry> entry(MethodHandle mh, String key) {
            MemorySegment r = live();
            byte[] kb = key == null ? null : keyBytes(key);
            return nav(mh, r, kb, true).map(h -> new Entry(new String(h.key(), StandardCharsets.UTF_8), h.value()));
        }

        /**
         * The smallest entry.
         *
         * @return the entry, or empty if the map is empty
         */
        public Optional<Entry> firstEntry() {
            return entry(FIRST, null);
        }

        /**
         * The largest entry.
         *
         * @return the entry, or empty if the map is empty
         */
        public Optional<Entry> lastEntry() {
            return entry(LAST, null);
        }

        /**
         * The entry with the smallest key at or after {@code key}.
         *
         * @param key search key
         * @return the entry, or empty if there is none
         */
        public Optional<Entry> ceilingEntry(String key) {
            return entry(CEILING, Objects.requireNonNull(key, "key must not be null"));
        }

        /**
         * The entry with the smallest key strictly after {@code key}.
         *
         * @param key search key
         * @return the entry, or empty if there is none
         */
        public Optional<Entry> higherEntry(String key) {
            return entry(HIGHER, Objects.requireNonNull(key, "key must not be null"));
        }

        /**
         * The entry with the largest key at or before {@code key}.
         *
         * @param key search key
         * @return the entry, or empty if there is none
         */
        public Optional<Entry> floorEntry(String key) {
            return entry(FLOOR, Objects.requireNonNull(key, "key must not be null"));
        }

        /**
         * The entry with the largest key strictly before {@code key}; empty for the empty key.
         *
         * @param key search key
         * @return the entry, or empty if there is none
         */
        public Optional<Entry> lowerEntry(String key) {
            return entry(LOWER, Objects.requireNonNull(key, "key must not be null"));
        }
    }

    /** Creates a new empty concurrent string map. */
    public SyncExpanseStrMap() {
        super(new Handles(
                MH_expanse_sync_strmap_free,
                MH_expanse_sync_strmap_insert,
                MH_expanse_sync_strmap_get,
                MH_expanse_sync_strmap_contains,
                MH_expanse_sync_strmap_remove,
                MH_expanse_sync_strmap_len,
                MH_expanse_sync_strmap_mem_used,
                MH_expanse_sync_strmap_mem_held,
                MH_expanse_sync_strmap_shrink_to_fit,
                MH_expanse_sync_strmap_compare_exchange,
                MH_expanse_sync_strmap_get_or_insert,
                MH_expanse_sync_strmap_reader_new,
                MH_expanse_sync_strmap_reader_free,
                MH_expanse_sync_strmap_reader_get,
                MH_expanse_sync_strmap_reader_contains).ignoringLengths(),
                MH_expanse_sync_strmap_new, "expanse_sync_strmap_t", true);
    }

    /**
     * An estimate of {@link #memUsed()} that takes no lock and does not walk the map: equal to
     * it with no writer running, and beside writers within the bytes their overlapping
     * operations allocate or free.
     *
     * @return bytes
     */
    public long memUsedEstimate() {
        try {
            return (long) MH_expanse_sync_strmap_mem_used_estimate.invokeExact(nativeHandle());
        } catch (Throwable t) {
            throw rethrow(t);
        }
    }

    @Override
    byte[] keyBytes(String key) {
        Objects.requireNonNull(key, "key must not be null");
        byte[] kb = key.getBytes(StandardCharsets.UTF_8);
        for (byte b : kb) {
            if (b == 0) {
                throw new IllegalArgumentException("key must not contain an embedded NUL character");
            }
        }
        return kb;
    }

    /**
     * Registers and returns a new {@link Reader}. Close it before the map, or let the map's
     * {@link #close()} close it.
     *
     * @return new Reader instance
     */
    public Reader reader() {
        return register(Reader::new);
    }
}
