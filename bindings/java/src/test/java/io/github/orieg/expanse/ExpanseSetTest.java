package io.github.orieg.expanse;

import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.OptionalLong;
import java.util.PrimitiveIterator;

import static org.junit.jupiter.api.Assertions.*;

class ExpanseSetTest {

    @Test
    @DisplayName("Basic set operations: insert, contains, remove, size, memory")
    void basicOperations() {
        try (ExpanseSet set = new ExpanseSet()) {
            assertTrue(set.isEmpty());
            assertEquals(0, set.size());
            assertEquals(0, set.memoryUsed());

            for (long i = 0; i < 1000; i++) {
                assertTrue(set.add(i * 10), "Inserting " + (i * 10));
            }

            assertEquals(1000, set.size());
            assertFalse(set.isEmpty());
            assertTrue(set.memoryUsed() > 0);

            // Duplicate insert returns false
            assertFalse(set.add(0));
            assertFalse(set.add(500));
            assertEquals(1000, set.size());

            // Membership tests
            for (long i = 0; i < 1000; i++) {
                assertTrue(set.contains(i * 10));
                assertFalse(set.contains(i * 10 + 1));
            }

            // Remove operations
            assertTrue(set.remove(0));
            assertFalse(set.contains(0));
            assertFalse(set.remove(0)); // second remove returns false
            assertEquals(999, set.size());

            // Clear
            set.clear();
            assertEquals(0, set.size());
            assertTrue(set.isEmpty());
            assertEquals(0, set.memoryUsed());
        }
    }

    @Test
    @DisplayName("Ordered navigation: first, last, ceiling, floor, higher, lower")
    void navigation() {
        try (ExpanseSet set = new ExpanseSet()) {
            long[] keys = {10, 20, 30, 40, 50, 60, 70, 80, 90, 100};
            for (long k : keys) {
                set.add(k);
            }

            assertEquals(OptionalLong.of(10), set.first());
            assertEquals(OptionalLong.of(100), set.last());

            // Ceiling (>=)
            assertEquals(OptionalLong.of(30), set.ceiling(25));
            assertEquals(OptionalLong.of(30), set.ceiling(30));
            assertEquals(OptionalLong.empty(), set.ceiling(101));

            // Higher (>)
            assertEquals(OptionalLong.of(30), set.higher(20));
            assertEquals(OptionalLong.of(40), set.higher(30));
            assertEquals(OptionalLong.empty(), set.higher(100));

            // Floor (<=)
            assertEquals(OptionalLong.of(30), set.floor(35));
            assertEquals(OptionalLong.of(30), set.floor(30));
            assertEquals(OptionalLong.empty(), set.floor(5));

            // Lower (<)
            assertEquals(OptionalLong.of(20), set.lower(30));
            assertEquals(OptionalLong.of(10), set.lower(20));
            assertEquals(OptionalLong.empty(), set.lower(10));
        }
    }

    @Test
    @DisplayName("Rank and select: countBelow, countRange, byCount")
    void rankAndSelect() {
        try (ExpanseSet set = new ExpanseSet()) {
            for (long i = 0; i < 100; i++) {
                set.add(i * 7); // 0, 7, 14, 21, ..., 693
            }

            assertEquals(100, set.size());

            // countBelow
            assertEquals(0, set.countBelow(0));
            assertEquals(1, set.countBelow(7));
            assertEquals(10, set.countBelow(70));
            assertEquals(100, set.countBelow(1000));

            // countRange
            assertEquals(11, set.countRange(0, 70)); // 0..70 inclusive (0,7,14,21,28,35,42,49,56,63,70 = 11 keys)
            assertEquals(0, set.countRange(70, 0)); // lo > hi
            assertEquals(100, set.countRange(0, 1000));

            // byCount (0-based select)
            assertEquals(OptionalLong.of(0), set.byCount(0));
            assertEquals(OptionalLong.of(21), set.byCount(3));
            assertEquals(OptionalLong.of(693), set.byCount(99));
            assertEquals(OptionalLong.empty(), set.byCount(100));
        }
    }

    @Test
    @DisplayName("Iteration and streaming")
    void iterationAndStreams() {
        try (ExpanseSet set = new ExpanseSet()) {
            for (long i = 1; i <= 50; i++) {
                set.add(i * 2);
            }

            List<Long> collected = new ArrayList<>();
            set.forEach(collected::add);
            assertEquals(50, collected.size());
            assertEquals(2L, collected.get(0));
            assertEquals(100L, collected.get(49));

            // Primitive iterator
            PrimitiveIterator.OfLong it = set.iterator();
            long expected = 2;
            int count = 0;
            while (it.hasNext()) {
                assertEquals(expected, it.nextLong());
                expected += 2;
                count++;
            }
            assertEquals(50, count);

            // Stream operations
            long sum = set.stream().sum();
            assertEquals(50 * 52, sum); // sum of 2..100
        }
    }

    @Test
    @DisplayName("IllegalStateException after close")
    void closedSafety() {
        ExpanseSet set = new ExpanseSet();
        set.add(42);
        set.close();

        assertThrows(IllegalStateException.class, () -> set.add(10));
        assertThrows(IllegalStateException.class, () -> set.contains(42));
        assertThrows(IllegalStateException.class, () -> set.first());
        assertThrows(IllegalStateException.class, () -> set.size());
    }
}
