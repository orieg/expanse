package io.github.orieg.expanse;

import static org.junit.jupiter.api.Assertions.*;

import io.github.orieg.expanse.SyncExpanseBlobMap.Blob;
import io.github.orieg.expanse.SyncExpanseBlobMap.Exchange;
import io.github.orieg.expanse.SyncExpanseBlobMap.ExchangeStatus;
import io.github.orieg.expanse.SyncExpanseBlobMap.MetaExchange;
import java.util.Optional;
import java.util.OptionalInt;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

/** Unit tests of the shared blob map (#1538). */
class SyncExpanseBlobMapTest {

    private static final int META = 0x123456;

    /** A payload of {@code n} bytes that is not a run, so no codec stores it in a slot word. */
    private static byte[] payload(int n, int seed) {
        byte[] b = new byte[n];
        for (int i = 0; i < n; i++) {
            b[i] = (byte) (i * 37 + seed * 11 + (i >>> 5));
        }
        return b;
    }

    private static void assertBlob(byte[] data, int meta, Optional<Blob> got) {
        assertTrue(got.isPresent(), "value present");
        assertArrayEquals(data, got.get().data());
        assertEquals(meta, got.get().hotMeta());
    }

    private static Blob blob(byte[] data, int meta) {
        return new Blob(data, meta);
    }

    @Test
    @DisplayName("insert and get round-trip payloads smaller and larger than the initial buffer")
    void roundTripSizes() {
        try (SyncExpanseBlobMap m = new SyncExpanseBlobMap(64 * 1024)) {
            int[] sizes = {0, 1, 7, 8, 100, 255, 256, 257, 1000, 40_000};
            for (int i = 0; i < sizes.length; i++) {
                assertTrue(m.insert(i, payload(sizes[i], i), 0), "size " + sizes[i]);
            }
            assertEquals(sizes.length, m.len());
            try (SyncExpanseBlobMap.Reader r = m.reader()) {
                for (int i = 0; i < sizes.length; i++) {
                    assertBlob(payload(sizes[i], i), 0, m.get(i));
                    assertBlob(payload(sizes[i], i), 0, r.get(i));
                    assertArrayEquals(payload(sizes[i], i), m.getBytes(i));
                    assertTrue(m.containsKey(i));
                    assertTrue(r.containsKey(i));
                }
                assertTrue(m.get(99).isEmpty());
                assertTrue(r.get(99).isEmpty());
                assertNull(m.getBytes(99));
                assertFalse(r.containsKey(99));
            }
        }
    }

    @Test
    @DisplayName("a value that grows while a reader holds an undersized buffer is read whole")
    void growingValueIsReadWhole() {
        // The read starts with a 256-byte buffer, learns the real length from the too-small
        // status and reads again with a buffer of that length.
        try (SyncExpanseBlobMap m = new SyncExpanseBlobMap(64 * 1024); SyncExpanseBlobMap.Reader r = m.reader()) {
            m.insert(1, payload(10, 1), META);
            assertBlob(payload(10, 1), META, r.get(1));
            m.insert(1, payload(30_000, 2), META);
            assertBlob(payload(30_000, 2), META, r.get(1));
            assertBlob(payload(30_000, 2), META, m.get(1));
            m.insert(1, payload(5, 3), 0);
            assertBlob(payload(5, 3), 0, r.get(1));
        }
    }

    @Test
    @DisplayName("hot metadata round-trips for arena values and reads 0 for slot-word values")
    void metadata() {
        try (SyncExpanseBlobMap m = new SyncExpanseBlobMap(); SyncExpanseBlobMap.Reader r = m.reader()) {
            m.insert(1, payload(100, 1), META);
            m.insert(2, payload(3, 2), META); // 7 bytes or fewer: stored without metadata
            assertEquals(OptionalInt.of(META), m.getMeta(1));
            assertEquals(OptionalInt.of(META), r.getMeta(1));
            assertEquals(OptionalInt.of(0), m.getMeta(2));
            assertEquals(OptionalInt.empty(), m.getMeta(3));
            assertEquals(OptionalInt.empty(), r.getMeta(3));
            assertBlob(payload(100, 1), META, m.get(1));
            assertBlob(payload(3, 2), 0, m.get(2));
        }
    }

    @Test
    @DisplayName("insertStatus reports a refusal; a refused insert leaves the map unchanged")
    void insertRefusals() {
        try (SyncExpanseBlobMap m = new SyncExpanseBlobMap(4096)) {
            assertEquals(ExpanseBlobMap.InsertStatus.OK, m.insertStatus(1, payload(100, 1), 5));
            assertEquals(ExpanseBlobMap.InsertStatus.META_OVERFLOW, m.insertStatus(2, payload(100, 1), 0x1000000));
            assertFalse(m.insert(2, payload(100, 1), 0x1000000));
            assertEquals(ExpanseBlobMap.InsertStatus.ALLOCATION_FAILED, m.insertStatus(3, payload(5000, 1), 0));
            assertFalse(m.containsKey(2));
            assertFalse(m.containsKey(3));
            assertEquals(1, m.len());
            assertThrows(NullPointerException.class, () -> m.insert(4, null));
        }
    }

    @Test
    @DisplayName("remove, accounting and compaction")
    void removeAndAccounting() {
        try (SyncExpanseBlobMap m = new SyncExpanseBlobMap(64 * 1024, 1 << 20)) {
            for (int i = 0; i < 100; i++) {
                assertTrue(m.insert(i, payload(200, i), 0));
            }
            assertTrue(m.memUsed() > 0);
            assertTrue(m.memUsedEstimate() > 0);
            assertTrue(m.memHeld() > 0);
            for (int i = 0; i < 100; i += 2) {
                assertTrue(m.remove(i));
                assertFalse(m.remove(i));
            }
            assertEquals(50, m.size());
            assertEquals(ExpanseBlobMap.InsertStatus.OK, m.compact());
            m.setReclaimAtCap(false);
            m.setReclaimAtCap(true);
            assertTrue(m.shrinkToFit() >= 0);
            for (int i = 1; i < 100; i += 2) {
                assertBlob(payload(200, i), 0, m.get(i));
            }
        }
    }

    @Test
    @DisplayName("compareExchange: every branch, payload and metadata compared, large values included")
    void compareExchangeBranches() {
        byte[] small = payload(100, 1);
        byte[] big = payload(5000, 2); // larger than the initial buffer
        byte[] other = payload(120, 3);
        try (SyncExpanseBlobMap m = new SyncExpanseBlobMap(64 * 1024)) {
            Optional<Blob> none = Optional.empty();
            // absent == absent: nothing to store.
            Exchange x = m.compareExchange(1, none, none);
            assertTrue(x.stored());
            assertEquals(ExchangeStatus.OK, x.status());
            assertTrue(x.observed().isEmpty());
            assertFalse(m.containsKey(1));
            // insert if absent.
            assertTrue(m.compareExchange(1, none, Optional.of(blob(small, META))).stored());
            assertBlob(small, META, m.get(1));
            // insert-if-absent against a present key mismatches and reports the value seen.
            x = m.compareExchange(1, none, Optional.of(blob(other, META)));
            assertFalse(x.stored());
            assertEquals(ExchangeStatus.MISMATCH, x.status());
            assertBlob(small, META, x.observed());
            // wrong payload, wrong metadata: mismatch, value unchanged.
            x = m.compareExchange(1, Optional.of(blob(other, META)), Optional.of(blob(big, META)));
            assertEquals(ExchangeStatus.MISMATCH, x.status());
            assertBlob(small, META, x.observed());
            x = m.compareExchange(1, Optional.of(blob(small, META + 1)), Optional.of(blob(big, META)));
            assertEquals(ExchangeStatus.MISMATCH, x.status());
            assertBlob(small, META, x.observed());
            assertBlob(small, META, m.get(1));
            // expected present against an absent key: mismatch observing absent.
            x = m.compareExchange(2, Optional.of(blob(small, META)), Optional.of(blob(big, META)));
            assertEquals(ExchangeStatus.MISMATCH, x.status());
            assertTrue(x.observed().isEmpty());
            // replace on match, with a value larger than the initial buffer.
            x = m.compareExchange(1, Optional.of(blob(small, META)), Optional.of(blob(big, 7)));
            assertTrue(x.stored());
            assertBlob(small, META, x.observed());
            assertBlob(big, 7, m.get(1));
            // a mismatch against the large value reads it whole (the too-small retry).
            x = m.compareExchange(1, Optional.of(blob(small, META)), Optional.of(blob(other, 0)));
            assertEquals(ExchangeStatus.MISMATCH, x.status());
            assertBlob(big, 7, x.observed());
            // remove-if-equal: mismatch leaves it, match removes it.
            assertFalse(m.compareExchange(1, Optional.of(blob(big, 8)), none).stored());
            assertTrue(m.containsKey(1));
            assertTrue(m.compareExchange(1, Optional.of(blob(big, 7)), none).stored());
            assertFalse(m.containsKey(1));
            // empty payloads are values too.
            assertTrue(m.compareExchange(3, none, Optional.of(blob(new byte[0], 0))).stored());
            assertBlob(new byte[0], 0, m.get(3));
            assertEquals(ExchangeStatus.MISMATCH, m.compareExchange(3, none, none).status());
        }
    }

    @Test
    @DisplayName("compareExchange reports a refusal only when the comparison held")
    void compareExchangeRefusal() {
        try (SyncExpanseBlobMap m = new SyncExpanseBlobMap(4096)) {
            byte[] v = payload(100, 1);
            m.insert(1, v, 0);
            Exchange x = m.compareExchange(1, Optional.of(blob(v, 0)), Optional.of(blob(v, 0x1000000)));
            assertEquals(ExchangeStatus.META_OVERFLOW, x.status());
            assertFalse(x.stored());
            assertBlob(v, 0, m.get(1)); // unchanged
            // The comparison failed, so the refusal is not reached.
            x = m.compareExchange(1, Optional.of(blob(payload(100, 9), 0)), Optional.of(blob(v, 0x1000000)));
            assertEquals(ExchangeStatus.MISMATCH, x.status());
            // A payload larger than a chunk cannot be allocated.
            x = m.compareExchange(2, Optional.empty(), Optional.of(blob(payload(5000, 1), 0)));
            assertEquals(ExchangeStatus.ALLOCATION_FAILED, x.status());
            assertFalse(m.containsKey(2));
        }
    }

    @Test
    @DisplayName("getOrInsert stores only into an absent key and returns a large existing value whole")
    void getOrInsert() {
        byte[] small = payload(50, 1);
        byte[] big = payload(9000, 2);
        try (SyncExpanseBlobMap m = new SyncExpanseBlobMap(64 * 1024)) {
            Exchange x = m.getOrInsert(1, big, META);
            assertEquals(ExchangeStatus.OK, x.status());
            assertTrue(x.observed().isEmpty());
            x = m.getOrInsert(1, small, 0);
            assertEquals(ExchangeStatus.MISMATCH, x.status());
            assertBlob(big, META, x.observed()); // the existing value, left untouched
            assertBlob(big, META, m.get(1));
            x = m.getOrInsert(2, small, 0x1000000);
            assertEquals(ExchangeStatus.META_OVERFLOW, x.status());
            assertTrue(x.observed().isEmpty());
            assertFalse(m.containsKey(2));
            assertEquals(ExchangeStatus.OK, m.getOrInsert(3, new byte[0], 0).status());
            assertBlob(new byte[0], 0, m.get(3));
        }
    }

    @Test
    @DisplayName("compareExchangeMeta swaps metadata alone")
    void compareExchangeMeta() {
        try (SyncExpanseBlobMap m = new SyncExpanseBlobMap(64 * 1024)) {
            byte[] v = payload(100, 1);
            m.insert(1, v, 5);
            MetaExchange x = m.compareExchangeMeta(1, OptionalInt.of(5), OptionalInt.of(6));
            assertTrue(x.stored());
            assertBlob(v, 6, m.get(1)); // payload untouched
            x = m.compareExchangeMeta(1, OptionalInt.of(5), OptionalInt.of(7));
            assertEquals(ExchangeStatus.MISMATCH, x.status());
            assertEquals(OptionalInt.of(6), x.observed());
            x = m.compareExchangeMeta(2, OptionalInt.of(5), OptionalInt.of(7));
            assertEquals(ExchangeStatus.MISMATCH, x.status());
            assertEquals(OptionalInt.empty(), x.observed());
            assertEquals(ExchangeStatus.META_OVERFLOW,
                    m.compareExchangeMeta(1, OptionalInt.of(6), OptionalInt.of(0x1000000)).status());
            // Metadata alone cannot create a key.
            assertEquals(ExchangeStatus.NO_META_FIELD,
                    m.compareExchangeMeta(3, OptionalInt.empty(), OptionalInt.of(1)).status());
            // A payload stored in its slot word has no field for non-zero metadata.
            m.insert(4, payload(3, 1), 0);
            assertEquals(ExchangeStatus.NO_META_FIELD,
                    m.compareExchangeMeta(4, OptionalInt.of(0), OptionalInt.of(1)).status());
            // remove-if-equal.
            assertTrue(m.compareExchangeMeta(1, OptionalInt.of(6), OptionalInt.empty()).stored());
            assertFalse(m.containsKey(1));
            assertTrue(m.compareExchangeMeta(1, OptionalInt.empty(), OptionalInt.empty()).stored());
        }
    }

    @Test
    @DisplayName("closing: reader and map are refused afterwards, closes are idempotent")
    void lifetime() {
        SyncExpanseBlobMap m = new SyncExpanseBlobMap();
        m.insert(1, payload(100, 1), 0);
        SyncExpanseBlobMap.Reader r1 = m.reader();
        SyncExpanseBlobMap.Reader r2 = m.reader();
        r1.close();
        assertThrows(IllegalStateException.class, () -> r1.get(1));
        assertDoesNotThrow(r1::close);
        assertTrue(r2.get(1).isPresent());
        m.close();
        assertThrows(IllegalStateException.class, () -> r2.get(1));
        assertThrows(IllegalStateException.class, () -> r2.getMeta(1));
        assertThrows(IllegalStateException.class, () -> r2.containsKey(1));
        assertDoesNotThrow(r2::close);
        assertThrows(IllegalStateException.class, () -> m.get(1));
        assertThrows(IllegalStateException.class, m::reader);
        assertDoesNotThrow(m::close);
    }
}
