package io.github.orieg.expanse;

import static org.junit.jupiter.api.Assertions.*;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.OptionalLong;
import java.util.function.Supplier;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

/** Unit tests of the shared string, bytes and ordered-bytes maps (#1538). */
class SyncExpanseKeyedMapsTest {

    private static final OptionalLong ABSENT = OptionalLong.empty();

    private static void assertExchange(SyncExpanseMap.Exchange got, boolean stored, OptionalLong observed) {
        assertEquals(new SyncExpanseMap.Exchange(stored, observed), got);
    }

    /** Point operations and read-modify-write, common to all three maps. */
    private static <K> void pointOperations(SyncKeyedMap<K> m, K a, K b, K c) {
        assertEquals(0, m.len());
        assertTrue(m.insert(a, 10));
        assertFalse(m.insert(a, 11), "a second insert of the key replaces");
        assertEquals(OptionalLong.of(11), m.get(a));
        assertEquals(OptionalLong.of(11), m.putAndGetOld(a, 12));
        assertEquals(ABSENT, m.putAndGetOld(b, 20));
        assertTrue(m.put(c, 30));
        assertEquals(3, m.size());
        assertTrue(m.containsKey(b));
        assertEquals(ABSENT, m.get(keyAfter(m, c)));
        assertTrue(m.remove(b));
        assertFalse(m.remove(b));
        assertFalse(m.containsKey(b));
        assertEquals(2, m.len());
        assertTrue(m.memUsed() > 0);
        assertTrue(m.memHeld() > 0);
        assertTrue(m.shrinkToFit() >= 0);

        // compareExchange: every branch, and every long (0 and -1 included) is a value.
        assertExchange(m.compareExchange(b, ABSENT, ABSENT), true, ABSENT);
        assertFalse(m.containsKey(b));
        assertExchange(m.compareExchange(b, ABSENT, OptionalLong.of(1)), true, ABSENT);
        assertExchange(m.compareExchange(b, ABSENT, OptionalLong.of(2)), false, OptionalLong.of(1));
        assertExchange(m.compareExchange(b, OptionalLong.of(9), OptionalLong.of(2)), false, OptionalLong.of(1));
        assertExchange(m.compareExchange(b, OptionalLong.of(1), OptionalLong.of(0)), true, OptionalLong.of(1));
        assertExchange(m.compareExchange(b, OptionalLong.of(0), OptionalLong.of(-1L)), true, OptionalLong.of(0));
        assertEquals(OptionalLong.of(-1L), m.get(b));
        assertExchange(m.compareExchange(b, OptionalLong.of(5), ABSENT), false, OptionalLong.of(-1L));
        assertExchange(m.compareExchange(b, OptionalLong.of(-1L), ABSENT), true, OptionalLong.of(-1L));
        assertFalse(m.containsKey(b));

        // getOrInsert stores only into an absent key.
        assertEquals(ABSENT, m.getOrInsert(b, 70));
        assertEquals(OptionalLong.of(70), m.getOrInsert(b, 71));
        assertEquals(OptionalLong.of(70), m.get(b));
        assertEquals(ABSENT, m.update(keyAfter(m, c), cur -> OptionalLong.of(cur.orElse(0) + 1)));
        assertEquals(OptionalLong.of(1), m.update(keyAfter(m, c), cur -> OptionalLong.of(cur.orElse(0) + 1)));
    }

    /** A key not among the three used above. */
    @SuppressWarnings("unchecked")
    private static <K> K keyAfter(SyncKeyedMap<K> m, K like) {
        if (like instanceof String) {
            return (K) "zz-unused";
        }
        return (K) new byte[] {(byte) 0xEE, 0x01};
    }

    @Test
    @DisplayName("SyncExpanseStrMap point operations, read-modify-write and closed handle")
    void strMapPointOperations() {
        SyncExpanseStrMap m = new SyncExpanseStrMap();
        try (m) {
            pointOperations(m, "alpha", "beta", "gamma");
            // The empty string is an ordinary key; non-ASCII keys round-trip.
            assertTrue(m.insert("", 5));
            assertEquals(OptionalLong.of(5), m.get(""));
            assertTrue(m.insert("héllo wörld €", 6));
            assertEquals(OptionalLong.of(6), m.get("héllo wörld €"));
        }
        assertThrows(IllegalStateException.class, () -> m.get("alpha"));
        assertDoesNotThrow(m::close);
    }

    @Test
    @DisplayName("SyncExpanseStrMap rejects a key with an embedded NUL and a null key")
    void strMapRejectsNul() {
        try (SyncExpanseStrMap m = new SyncExpanseStrMap(); SyncExpanseStrMap.Reader r = m.reader()) {
            assertThrows(IllegalArgumentException.class, () -> m.insert("a\u0000b", 1));
            assertThrows(IllegalArgumentException.class, () -> m.get("a\u0000"));
            assertThrows(IllegalArgumentException.class, () -> m.compareExchange("\u0000", ABSENT, OptionalLong.of(1)));
            assertThrows(IllegalArgumentException.class, () -> r.containsKey("x\u0000y"));
            assertThrows(IllegalArgumentException.class, () -> r.ceilingEntry("x\u0000y"));
            assertThrows(NullPointerException.class, () -> m.insert((String) null, 1));
            assertEquals(0, m.len(), "a rejected key stores nothing");
            m.insert("a-key-longer-than-one-chunk", 1);
            assertEquals(m.memUsed(), m.memUsedEstimate(), "equal with no writer running");
            assertTrue(m.memUsedEstimate() > 0);
        }
    }

    @Test
    @DisplayName("SyncExpanseStrMap reader: point reads and ordered reads, long keys included")
    void strMapReaderOrderedReads() {
        String longKey = "k".repeat(300); // longer than the initial navigation buffer
        String[] keys = {"b", "d", "f", longKey, "z"};
        try (SyncExpanseStrMap m = new SyncExpanseStrMap()) {
            for (int i = 0; i < keys.length; i++) {
                m.insert(keys[i], i * 100L);
            }
            try (SyncExpanseStrMap.Reader r = m.reader()) {
                assertEquals(OptionalLong.of(100), r.get("d"));
                assertEquals(ABSENT, r.get("c"));
                assertTrue(r.containsKey("f"));
                assertFalse(r.containsKey("e"));

                assertEquals(new SyncExpanseStrMap.Entry("b", 0), r.firstEntry().orElseThrow());
                assertEquals(new SyncExpanseStrMap.Entry("z", 400), r.lastEntry().orElseThrow());
                assertEquals(new SyncExpanseStrMap.Entry("d", 100), r.ceilingEntry("c").orElseThrow());
                assertEquals(new SyncExpanseStrMap.Entry("d", 100), r.ceilingEntry("d").orElseThrow());
                assertEquals(new SyncExpanseStrMap.Entry("f", 200), r.higherEntry("d").orElseThrow());
                assertEquals(new SyncExpanseStrMap.Entry("d", 100), r.floorEntry("e").orElseThrow());
                assertEquals(new SyncExpanseStrMap.Entry("d", 100), r.floorEntry("d").orElseThrow());
                assertEquals(new SyncExpanseStrMap.Entry("b", 0), r.lowerEntry("d").orElseThrow());
                // Past either end, and before the empty string, there is nothing.
                assertTrue(r.higherEntry("z").isEmpty());
                assertTrue(r.ceilingEntry("zz").isEmpty());
                assertTrue(r.lowerEntry("b").isEmpty());
                assertTrue(r.lowerEntry("").isEmpty());
                assertTrue(r.floorEntry("a").isEmpty());

                // A key longer than the initial buffer comes back whole (the too-small retry).
                assertEquals(new SyncExpanseStrMap.Entry(longKey, 300), r.higherEntry("f").orElseThrow());
                assertEquals(new SyncExpanseStrMap.Entry(longKey, 300), r.lowerEntry("z").orElseThrow());
                assertEquals(new SyncExpanseStrMap.Entry("z", 400), r.higherEntry(longKey).orElseThrow());
            }
            try (SyncExpanseStrMap empty = new SyncExpanseStrMap(); SyncExpanseStrMap.Reader r = empty.reader()) {
                assertTrue(r.firstEntry().isEmpty());
                assertTrue(r.lastEntry().isEmpty());
            }
        }
    }

    @Test
    @DisplayName("SyncExpanseStrMap: a walk with higherEntry visits every key in order")
    void strMapWalk() {
        try (SyncExpanseStrMap m = new SyncExpanseStrMap(); SyncExpanseStrMap.Reader r = m.reader()) {
            for (int i = 0; i < 500; i++) {
                m.insert(String.format("key-%05d", i), i);
            }
            int n = 0;
            for (var e = r.firstEntry(); e.isPresent(); e = r.higherEntry(e.get().key())) {
                assertEquals(String.format("key-%05d", n), e.get().key());
                assertEquals(n, e.get().value());
                n++;
            }
            assertEquals(500, n);
        }
    }

    @Test
    @DisplayName("SyncExpanseBytesMap point operations, embedded NUL and empty keys")
    void bytesMapPointOperations() {
        SyncExpanseBytesMap m = new SyncExpanseBytesMap();
        try (m) {
            pointOperations(m, new byte[] {1, 2, 3}, new byte[] {4}, new byte[] {5, 0, 5});
            // Embedded zeros are ordinary; keys differing after a zero are distinct.
            assertTrue(m.insert(new byte[] {7, 0, 1}, 1));
            assertTrue(m.insert(new byte[] {7, 0, 2}, 2));
            assertTrue(m.insert(new byte[] {7, 0}, 3));
            assertEquals(OptionalLong.of(2), m.get(new byte[] {7, 0, 2}));
            assertEquals(OptionalLong.of(3), m.get(new byte[] {7, 0}));
            assertEquals(ABSENT, m.get(new byte[] {7}));
            // The empty key.
            assertEquals(ABSENT, m.get(new byte[0]));
            assertTrue(m.insert(new byte[0], 99));
            assertEquals(OptionalLong.of(99), m.get(new byte[0]));
            assertTrue(m.containsKey(new byte[0]));
            assertTrue(m.remove(new byte[0]));
            // A key array is copied on the way in.
            byte[] k = {9, 9};
            m.insert(k, 1);
            k[0] = 8;
            assertEquals(OptionalLong.of(1), m.get(new byte[] {9, 9}));
            assertEquals(ABSENT, m.get(new byte[] {8, 9}));
            assertThrows(NullPointerException.class, () -> m.insert((byte[]) null, 1));
        }
        assertThrows(IllegalStateException.class, () -> m.get(new byte[] {1}));
        assertDoesNotThrow(m::close);
    }

    @Test
    @DisplayName("SyncExpanseBytesMap reader reads, and the map closes its readers")
    void bytesMapReader() {
        SyncExpanseBytesMap m = new SyncExpanseBytesMap();
        m.insert(new byte[] {1, 0, 2}, 12);
        SyncExpanseBytesMap.Reader r = m.reader();
        assertEquals(OptionalLong.of(12), r.get(new byte[] {1, 0, 2}));
        assertEquals(ABSENT, r.get(new byte[] {1, 0}));
        assertTrue(r.containsKey(new byte[] {1, 0, 2}));
        assertFalse(r.containsKey(new byte[0]));
        m.close();
        assertThrows(IllegalStateException.class, () -> r.get(new byte[] {1, 0, 2}));
        assertDoesNotThrow(r::close);
        assertThrows(IllegalStateException.class, m::reader);
    }

    @Test
    @DisplayName("SyncExpanseOrderedBytesMap point operations and reader")
    void orderedBytesMapPointOperations() {
        SyncExpanseOrderedBytesMap m = new SyncExpanseOrderedBytesMap();
        try (m) {
            pointOperations(m, new byte[] {1, 2, 3}, new byte[] {4}, new byte[] {5, 0, 5});
            m.insert(new byte[] {0x30}, 1);
            try (SyncExpanseOrderedBytesMap.Reader r = m.reader()) {
                assertEquals(OptionalLong.of(1), r.get(new byte[] {0x30}));
                assertTrue(r.containsKey(new byte[] {0x30}));
                assertFalse(r.containsKey(new byte[] {0x31}));
            }
        }
        assertThrows(IllegalStateException.class, () -> m.firstEntry());
    }

    private static void assertEntry(byte[] key, long value, SyncExpanseOrderedBytesMap.Entry e) {
        assertArrayEquals(key, e.key());
        assertEquals(value, e.value());
    }

    @Test
    @DisplayName("SyncExpanseOrderedBytesMap ordered reads on the map handle")
    void orderedBytesMapOrderedReads() {
        byte[] empty = new byte[0];
        byte[] k1 = {0x01};
        byte[] k2 = {0x01, 0x00};
        byte[] k3 = {0x7F, 0x00, 0x01};
        byte[] big = new byte[300]; // longer than the initial navigation buffer
        Arrays.fill(big, (byte) 0x80);
        byte[] kff = {(byte) 0xFF};
        try (SyncExpanseOrderedBytesMap m = new SyncExpanseOrderedBytesMap()) {
            assertTrue(m.firstEntry().isEmpty());
            assertTrue(m.lastEntry().isEmpty());
            m.insert(k3, 3);
            m.insert(kff, 6);
            m.insert(k1, 1);
            m.insert(big, 5);
            m.insert(k2, 2);
            m.insert(empty, 0);

            assertEntry(empty, 0, m.firstEntry().orElseThrow()); // the empty key sorts first
            assertEntry(kff, 6, m.lastEntry().orElseThrow()); // bytes order as unsigned
            assertEntry(k1, 1, m.ceilingEntry(k1).orElseThrow());
            assertEntry(k2, 2, m.higherEntry(k1).orElseThrow()); // a prefix sorts before its extension
            assertEntry(k3, 3, m.higherEntry(k2).orElseThrow());
            assertEntry(big, 5, m.higherEntry(k3).orElseThrow()); // retried into a larger buffer
            assertEntry(kff, 6, m.higherEntry(big).orElseThrow());
            assertTrue(m.higherEntry(kff).isEmpty());
            assertEntry(big, 5, m.floorEntry(new byte[] {(byte) 0x90}).orElseThrow());
            assertEntry(big, 5, m.lowerEntry(kff).orElseThrow());
            assertEntry(k1, 1, m.floorEntry(new byte[] {0x01}).orElseThrow());
            assertEntry(empty, 0, m.lowerEntry(k1).orElseThrow());
            // Before the empty key there is nothing; the empty key itself is at-or-before itself.
            assertTrue(m.lowerEntry(empty).isEmpty());
            assertEntry(empty, 0, m.floorEntry(empty).orElseThrow());
            assertEntry(empty, 0, m.ceilingEntry(empty).orElseThrow());

            int n = 0;
            for (var e = m.firstEntry(); e.isPresent(); e = m.higherEntry(e.get().key())) {
                n++;
            }
            assertEquals(6, n);
            assertThrows(NullPointerException.class, () -> m.ceilingEntry(null));
        }
    }

    @Test
    @DisplayName("Readers of the three maps are refused after the reader or the map is closed")
    void readerLifetime() {
        Supplier<SyncExpanseStrMap> mk = SyncExpanseStrMap::new;
        SyncExpanseStrMap m = mk.get();
        m.insert("k", 1);
        SyncExpanseStrMap.Reader r1 = m.reader();
        SyncExpanseStrMap.Reader r2 = m.reader();
        r1.close();
        assertThrows(IllegalStateException.class, () -> r1.get("k"));
        assertDoesNotThrow(r1::close);
        assertEquals(OptionalLong.of(1), r2.get("k"));
        m.close();
        assertThrows(IllegalStateException.class, () -> r2.get("k"));
        assertThrows(IllegalStateException.class, () -> r2.firstEntry());

        SyncExpanseOrderedBytesMap o = new SyncExpanseOrderedBytesMap();
        SyncExpanseOrderedBytesMap.Reader or = o.reader();
        o.close();
        assertThrows(IllegalStateException.class, () -> or.get(new byte[] {1}));
    }

    @Test
    @DisplayName("Keys are UTF-8: string-map order follows the encoded bytes")
    void strMapOrderIsByUtf8Bytes() {
        try (SyncExpanseStrMap m = new SyncExpanseStrMap(); SyncExpanseStrMap.Reader r = m.reader()) {
            m.insert("a", 1);
            m.insert("é", 2); // 0xC3 0xA9
            m.insert("€", 3); // 0xE2 0x82 0xAC
            assertEquals("€", r.lastEntry().orElseThrow().key());
            assertEquals("é", r.lowerEntry("€").orElseThrow().key());
            assertArrayEquals("€".getBytes(StandardCharsets.UTF_8), new byte[] {(byte) 0xE2, (byte) 0x82, (byte) 0xAC});
        }
    }
}
