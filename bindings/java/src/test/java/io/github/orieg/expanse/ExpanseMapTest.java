package io.github.orieg.expanse;

import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.OptionalLong;
import java.util.PrimitiveIterator;

import static org.junit.jupiter.api.Assertions.*;

class ExpanseMapTest {

    @Test
    @DisplayName("Basic map operations: put, get, remove, putAndGetOld, getOrDefault")
    void basicOperations() {
        try (ExpanseMap map = new ExpanseMap()) {
            assertTrue(map.isEmpty());
            assertEquals(0, map.size());
            assertEquals(0, map.memoryUsed());

            for (long i = 0; i < 500; i++) {
                assertTrue(map.put(i, i * 100));
            }

            assertEquals(500, map.size());
            assertFalse(map.isEmpty());
            assertTrue(map.memoryUsed() > 0);

            // Re-insert updates value and returns false
            assertFalse(map.put(0, 999));
            assertEquals(OptionalLong.of(999), map.get(0));

            // putAndGetOld
            OptionalLong old = map.putAndGetOld(0, 1234);
            assertEquals(OptionalLong.of(999), old);
            assertEquals(OptionalLong.of(1234), map.get(0));

            // getOrDefault
            assertEquals(1234L, map.getOrDefault(0, -1));
            assertEquals(-1L, map.getOrDefault(99999, -1));

            // Lookups
            for (long i = 1; i < 500; i++) {
                assertEquals(OptionalLong.of(i * 100), map.get(i));
                assertTrue(map.containsKey(i));
            }
            assertFalse(map.containsKey(1000));

            // Remove and removeAndGetOld
            OptionalLong removedOld = map.removeAndGetOld(0);
            assertEquals(OptionalLong.of(1234), removedOld);
            assertFalse(map.containsKey(0));
            assertEquals(499, map.size());

            assertTrue(map.remove(1));
            assertFalse(map.remove(1));
            assertEquals(498, map.size());

            map.clear();
            assertEquals(0, map.size());
            assertTrue(map.isEmpty());
            assertEquals(0, map.memoryUsed());
        }
    }

    @Test
    @DisplayName("Direct value slot manipulation (zero-overhead off-heap pointer writes)")
    void valueSlotOperations() {
        try (ExpanseMap map = new ExpanseMap()) {
            // slot on non-existent key is null
            assertNull(map.slot(42));

            // insertSlot creates entry with 0 and returns writable pointer
            MemorySegment slot = map.insertSlot(42);
            assertNotNull(slot);
            assertEquals(0L, slot.get(ValueLayout.JAVA_LONG, 0));

            // Mutate off-heap memory directly via segment
            slot.set(ValueLayout.JAVA_LONG, 0, 777L);
            assertEquals(OptionalLong.of(777L), map.get(42));

            // Re-calling insertSlot preserves existing value
            MemorySegment slot2 = map.insertSlot(42);
            assertEquals(777L, slot2.get(ValueLayout.JAVA_LONG, 0));

            // slot() returns the existing pointer
            MemorySegment slotGet = map.slot(42);
            assertNotNull(slotGet);
            assertEquals(777L, slotGet.get(ValueLayout.JAVA_LONG, 0));
        }
    }

    @Test
    @DisplayName("Ordered navigation: first, last, ceiling, floor, higher, lower")
    void orderedNavigation() {
        try (ExpanseMap map = new ExpanseMap()) {
            long[] keys = {10, 20, 30, 40, 50};
            for (long k : keys) {
                map.put(k, k * 10);
            }

            assertEquals(OptionalLong.of(10), map.firstKey());
            assertEquals(OptionalLong.of(50), map.lastKey());
            assertEquals(Optional.of(new ExpanseMap.Entry(10, 100)), map.firstEntry());
            assertEquals(Optional.of(new ExpanseMap.Entry(50, 500)), map.lastEntry());

            // Ceiling
            assertEquals(Optional.of(new ExpanseMap.Entry(30, 300)), map.ceilingEntry(25));
            assertEquals(Optional.of(new ExpanseMap.Entry(30, 300)), map.ceilingEntry(30));
            assertEquals(Optional.empty(), map.ceilingEntry(55));

            // Higher
            assertEquals(Optional.of(new ExpanseMap.Entry(30, 300)), map.higherEntry(20));
            assertEquals(Optional.empty(), map.higherEntry(50));

            // Floor
            assertEquals(Optional.of(new ExpanseMap.Entry(30, 300)), map.floorEntry(35));
            assertEquals(Optional.of(new ExpanseMap.Entry(30, 300)), map.floorEntry(30));
            assertEquals(Optional.empty(), map.floorEntry(5));

            // Lower
            assertEquals(Optional.of(new ExpanseMap.Entry(20, 200)), map.lowerEntry(30));
            assertEquals(Optional.empty(), map.lowerEntry(10));
        }
    }

    @Test
    @DisplayName("Rank and select: countBelow, countRange, byCount")
    void rankAndSelect() {
        try (ExpanseMap map = new ExpanseMap()) {
            for (long i = 0; i < 50; i++) {
                map.put(i * 3, i * 100);
            }

            assertEquals(50, map.size());
            assertEquals(0, map.countBelow(0));
            assertEquals(1, map.countBelow(3));
            assertEquals(10, map.countBelow(30));
            assertEquals(11, map.countRange(0, 30));

            // byCount select
            assertEquals(Optional.of(new ExpanseMap.Entry(0, 0)), map.byCount(0));
            assertEquals(Optional.of(new ExpanseMap.Entry(9, 300)), map.byCount(3));
            assertEquals(Optional.empty(), map.byCount(100));
        }
    }

    @Test
    @DisplayName("Iteration: forEach, keyIterator, entryIterator, streams")
    void iterationAndStreams() {
        try (ExpanseMap map = new ExpanseMap()) {
            for (long i = 1; i <= 20; i++) {
                map.put(i, i * 10);
            }

            List<Long> keys = new ArrayList<>();
            List<Long> values = new ArrayList<>();
            map.forEach((k, v) -> {
                keys.add(k);
                values.add(v);
            });

            assertEquals(20, keys.size());
            assertEquals(1L, keys.get(0));
            assertEquals(20L, keys.get(19));
            assertEquals(10L, values.get(0));
            assertEquals(200L, values.get(19));

            // Primitive key iterator
            PrimitiveIterator.OfLong kit = map.keyIterator();
            long expected = 1;
            while (kit.hasNext()) {
                assertEquals(expected++, kit.nextLong());
            }
            assertEquals(21, expected);

            // Streams
            long keySum = map.keyStream().sum();
            assertEquals(210, keySum); // sum of 1..20

            long valSum = map.entryStream().mapToLong(ExpanseMap.Entry::value).sum();
            assertEquals(2100, valSum);
        }
    }
}
