package io.github.orieg.expanse;

import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.Timeout;

import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Optional;
import java.util.OptionalLong;

import static org.junit.jupiter.api.Assertions.*;

// A broken navigation wrapper can loop forever in forEach; fail instead of hanging.
@Timeout(value = 30, threadMode = Timeout.ThreadMode.SEPARATE_THREAD)
class ExpanseOrderedBytesMapTest {

    private static final byte[] EMPTY = new byte[0];
    private static final byte[] NUL = {0x00};
    private static final byte[] FF = {(byte) 0xFF};
    private static final byte[] A_NUL_B = {'a', 0x00, 'b'};
    private static final byte[] LONG = "0123456789abcdef-longer-than-eight-bytes".getBytes();

    private static void assertEntry(Optional<ExpanseOrderedBytesMap.Entry> e, byte[] key, long value) {
        assertTrue(e.isPresent(), "expected entry for " + Arrays.toString(key));
        assertArrayEquals(key, e.get().key());
        assertEquals(value, e.get().value());
    }

    @Test
    @DisplayName("Round trip: put/get/contains/remove, empty, binary and long keys")
    void roundTrip() {
        try (ExpanseOrderedBytesMap map = new ExpanseOrderedBytesMap()) {
            assertTrue(map.isEmpty());
            assertTrue(map.put(EMPTY, 1));
            assertTrue(map.put(NUL, 2));
            assertTrue(map.put(FF, 3));
            assertTrue(map.put(A_NUL_B, 4));
            assertTrue(map.put(LONG, 5));
            assertFalse(map.put(LONG, 50), "replace reports not-new");
            assertEquals(5, map.size());
            assertEquals(5, map.len());

            assertEquals(OptionalLong.of(1), map.get(EMPTY));
            assertEquals(OptionalLong.of(2), map.get(NUL));
            assertEquals(OptionalLong.of(3), map.get(FF));
            assertEquals(OptionalLong.of(4), map.get(A_NUL_B));
            assertEquals(OptionalLong.of(50), map.get(LONG));

            // Absent: a prefix and an extension of present keys are distinct keys.
            assertEquals(OptionalLong.empty(), map.get(new byte[] {'a'}));
            assertEquals(OptionalLong.empty(), map.get(new byte[] {'a', 0x00}));
            assertFalse(map.containsKey(new byte[] {0x00, 0x00}));
            assertTrue(map.containsKey(EMPTY));
            assertTrue(map.containsKey(A_NUL_B));

            assertTrue(map.remove(NUL));
            assertFalse(map.remove(NUL));
            assertFalse(map.containsKey(NUL));
            assertEquals(4, map.size());

            map.clear();
            assertTrue(map.isEmpty());
            assertEquals(OptionalLong.empty(), map.get(EMPTY));
        }
    }

    @Test
    @DisplayName("Ordering is unsigned lexicographic: empty < 0x00 < 'a'.. < 0xFF")
    void ordering() {
        try (ExpanseOrderedBytesMap map = new ExpanseOrderedBytesMap()) {
            // Insert out of order on purpose; value = rank in sorted order.
            map.put(FF, 5);
            map.put(A_NUL_B, 4);
            map.put(LONG, 3);
            map.put(NUL, 2);
            map.put(EMPTY, 1);

            List<byte[]> seen = new ArrayList<>();
            List<Long> vals = new ArrayList<>();
            map.forEach((k, v) -> {
                assertTrue(seen.size() < 100, "iteration did not terminate");
                seen.add(k);
                vals.add(v);
            });
            assertEquals(5, seen.size());
            assertArrayEquals(EMPTY, seen.get(0));
            assertArrayEquals(NUL, seen.get(1));
            assertArrayEquals(LONG, seen.get(2));
            assertArrayEquals(A_NUL_B, seen.get(3));
            assertArrayEquals(FF, seen.get(4));
            assertEquals(List.of(1L, 2L, 3L, 4L, 5L), vals);
        }
    }

    @Test
    @DisplayName("first/last navigation, including empty map")
    void firstLast() {
        try (ExpanseOrderedBytesMap map = new ExpanseOrderedBytesMap()) {
            assertTrue(map.firstEntry().isEmpty());
            assertTrue(map.lastEntry().isEmpty());

            map.put(EMPTY, 10);
            assertEntry(map.firstEntry(), EMPTY, 10);
            assertEntry(map.lastEntry(), EMPTY, 10);

            map.put(FF, 20);
            map.put(LONG, 30);
            assertEntry(map.firstEntry(), EMPTY, 10);
            assertEntry(map.lastEntry(), FF, 20);

            map.remove(EMPTY);
            assertEntry(map.firstEntry(), LONG, 30);
        }
    }

    @Test
    @DisplayName("ceiling/higher/floor/lower semantics at, between and beyond keys")
    void neighbours() {
        try (ExpanseOrderedBytesMap map = new ExpanseOrderedBytesMap()) {
            map.put(NUL, 1);
            map.put(A_NUL_B, 2);
            map.put(FF, 3);

            // At a present key.
            assertEntry(map.ceilingEntry(A_NUL_B), A_NUL_B, 2);
            assertEntry(map.higherEntry(A_NUL_B), FF, 3);
            assertEntry(map.floorEntry(A_NUL_B), A_NUL_B, 2);
            assertEntry(map.lowerEntry(A_NUL_B), NUL, 1);

            // Between present keys (absent search key).
            byte[] mid = {'a', 0x01};
            assertEntry(map.ceilingEntry(mid), FF, 3);
            assertEntry(map.higherEntry(mid), FF, 3);
            assertEntry(map.floorEntry(mid), A_NUL_B, 2);
            assertEntry(map.lowerEntry(mid), A_NUL_B, 2);

            // Empty search key sorts before everything.
            assertEntry(map.ceilingEntry(EMPTY), NUL, 1);
            assertEntry(map.higherEntry(EMPTY), NUL, 1);
            assertTrue(map.floorEntry(EMPTY).isEmpty());
            assertTrue(map.lowerEntry(EMPTY).isEmpty());

            // Past both ends.
            assertTrue(map.higherEntry(FF).isEmpty());
            assertTrue(map.ceilingEntry(new byte[] {(byte) 0xFF, 0x00}).isEmpty());
            assertEntry(map.floorEntry(new byte[] {(byte) 0xFF, 0x00}), FF, 3);
            assertTrue(map.lowerEntry(NUL).isEmpty());
            assertTrue(map.floorEntry(new byte[0]).isEmpty());

            // A prefix sorts before its extension: lower("a\0b") is not "a".
            assertEntry(map.ceilingEntry(new byte[] {'a'}), A_NUL_B, 2);
        }
    }

    @Test
    @DisplayName("Keys longer than the initial navigation buffer are returned whole")
    void longKeysGrowTheBuffer() {
        byte[] big = new byte[5000];
        Arrays.fill(big, (byte) 0xAB);
        big[0] = 0x01;
        big[4999] = 0x00;
        byte[] bigger = Arrays.copyOf(big, 70_000);
        Arrays.fill(bigger, 5000, 70_000, (byte) 0xFE);
        try (ExpanseOrderedBytesMap map = new ExpanseOrderedBytesMap()) {
            map.put(big, 7);
            map.put(bigger, 8);
            assertEntry(map.firstEntry(), big, 7);
            assertEntry(map.lastEntry(), bigger, 8);
            assertEntry(map.higherEntry(big), bigger, 8);
            assertEntry(map.lowerEntry(bigger), big, 7);
            assertEntry(map.ceilingEntry(EMPTY), big, 7);
            assertEntry(map.floorEntry(FF), bigger, 8);
        }
    }

    @Test
    @DisplayName("Iteration after mutation observes the mutation")
    void iterationAfterMutation() {
        try (ExpanseOrderedBytesMap map = new ExpanseOrderedBytesMap()) {
            for (int i = 0; i < 10; i++) {
                map.put(new byte[] {(byte) i, 0x00}, i);
            }
            assertTrue(map.remove(new byte[] {3, 0x00}));
            assertTrue(map.put(new byte[] {3, 0x01}, 33));
            assertTrue(map.put(new byte[] {(byte) 0xFF, (byte) 0xFF}, 99));

            List<Long> values = new ArrayList<>();
            map.forEach((k, v) -> {
                assertTrue(values.size() < 100, "iteration did not terminate");
                values.add(v);
            });
            assertEquals(List.of(0L, 1L, 2L, 33L, 4L, 5L, 6L, 7L, 8L, 9L, 99L), values);

            // Mutate while stepping: removing the cursor's key still lets higherEntry advance.
            Optional<ExpanseOrderedBytesMap.Entry> e = map.firstEntry();
            assertEntry(e, new byte[] {0, 0}, 0);
            map.remove(e.get().key());
            assertEntry(map.higherEntry(e.get().key()), new byte[] {1, 0}, 1);
            assertEntry(map.firstEntry(), new byte[] {1, 0}, 1);
            assertEquals(10, map.size());
        }
    }

    @Test
    @DisplayName("Value slots: slot is null when absent, insertSlot keeps existing, writes are visible")
    void slots() {
        try (ExpanseOrderedBytesMap map = new ExpanseOrderedBytesMap()) {
            assertNull(map.slot(A_NUL_B));
            MemorySegment s = map.insertSlot(A_NUL_B);
            assertEquals(0L, s.get(ValueLayout.JAVA_LONG, 0));
            s.set(ValueLayout.JAVA_LONG, 0, 777L);
            assertEquals(OptionalLong.of(777L), map.get(A_NUL_B));

            MemorySegment again = map.insertSlot(A_NUL_B);
            assertEquals(777L, again.get(ValueLayout.JAVA_LONG, 0), "existing value is kept");

            MemorySegment es = map.insertSlot(EMPTY);
            es.set(ValueLayout.JAVA_LONG, 0, 5L);
            assertEquals(OptionalLong.of(5L), map.get(EMPTY));
            assertNotNull(map.slot(EMPTY));
            assertEquals(2, map.size());
        }
    }

    @Test
    @DisplayName("Memory accounting: memUsed/memHeld positive when populated, shrinkToFit and clear are safe")
    void memory() {
        try (ExpanseOrderedBytesMap map = new ExpanseOrderedBytesMap()) {
            for (int i = 0; i < 2000; i++) {
                map.put(new byte[] {(byte) (i >> 8), (byte) i, 0x00, (byte) 0xFF}, i);
            }
            assertTrue(map.memUsed() > 0);
            assertTrue(map.memHeld() >= map.memUsed());
            map.clear();
            long before = map.memHeld();
            long released = map.shrinkToFit();
            assertTrue(released >= 0);
            assertTrue(map.memHeld() <= before);
            assertTrue(map.isEmpty());
        }
    }

    @Test
    @DisplayName("Use after close throws; close is idempotent; slot segments die with the map")
    void closeSemantics() {
        ExpanseOrderedBytesMap map = new ExpanseOrderedBytesMap();
        MemorySegment s = map.insertSlot(FF);
        map.close();
        map.close();
        assertThrows(IllegalStateException.class, () -> map.get(FF));
        assertThrows(IllegalStateException.class, map::firstEntry);
        assertThrows(IllegalStateException.class, () -> s.get(ValueLayout.JAVA_LONG, 0));
    }

    @Test
    @DisplayName("Null key is rejected")
    void nullKey() {
        try (ExpanseOrderedBytesMap map = new ExpanseOrderedBytesMap()) {
            assertThrows(NullPointerException.class, () -> map.put(null, 1));
            assertThrows(NullPointerException.class, () -> map.ceilingEntry(null));
        }
    }
}
