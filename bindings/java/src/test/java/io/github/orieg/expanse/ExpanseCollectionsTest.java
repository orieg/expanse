package io.github.orieg.expanse;

import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import java.util.Iterator;
import java.util.Map;
import java.util.NavigableMap;
import java.util.NavigableSet;

import static org.junit.jupiter.api.Assertions.*;

class ExpanseCollectionsTest {

    @Test
    @DisplayName("ExpanseJavaNavigableSet standard Set contract, subSet, headSet, tailSet, descendingSet")
    void navigableSetContract() {
        try (ExpanseSet rawSet = new ExpanseSet()) {
            NavigableSet<Long> set = rawSet.asJavaSet();
            assertTrue(set.isEmpty());
            assertEquals(0, set.size());

            for (long i = 1; i <= 10; i++) {
                assertTrue(set.add(i * 10)); // 10, 20, 30, ..., 100
            }

            assertEquals(10, set.size());
            assertTrue(set.contains(50L));
            assertFalse(set.contains(55L));

            // Navigation
            assertEquals(10L, set.first());
            assertEquals(100L, set.last());
            assertEquals(30L, set.ceiling(25L));
            assertEquals(30L, set.floor(35L));
            assertEquals(40L, set.higher(30L));
            assertEquals(20L, set.lower(30L));

            // subSet [30, 70]
            NavigableSet<Long> sub = set.subSet(30L, true, 70L, true);
            assertEquals(5, sub.size()); // 30, 40, 50, 60, 70
            assertTrue(sub.contains(30L));
            assertTrue(sub.contains(70L));
            assertFalse(sub.contains(20L));
            assertFalse(sub.contains(80L));

            // headSet (< 50)
            NavigableSet<Long> head = set.headSet(50L, false);
            assertEquals(4, head.size()); // 10, 20, 30, 40

            // tailSet (>= 70)
            NavigableSet<Long> tail = set.tailSet(70L, true);
            assertEquals(4, tail.size()); // 70, 80, 90, 100

            // descendingSet
            NavigableSet<Long> desc = set.descendingSet();
            assertEquals(100L, desc.first());
            assertEquals(10L, desc.last());
            Iterator<Long> descIt = desc.iterator();
            assertEquals(100L, descIt.next());
            assertEquals(90L, descIt.next());

            // Polling
            assertEquals(10L, set.pollFirst());
            assertEquals(100L, set.pollLast());
            assertEquals(8, set.size());
        }
    }

    @Test
    @DisplayName("ExpanseJavaNavigableMap standard Map contract, subMap, headMap, tailMap, descendingMap")
    void navigableMapContract() {
        try (ExpanseMap rawMap = new ExpanseMap()) {
            NavigableMap<Long, Long> map = rawMap.asJavaMap();
            assertTrue(map.isEmpty());

            for (long i = 1; i <= 10; i++) {
                assertNull(map.put(i * 10, i * 100));
            }

            assertEquals(10, map.size());
            assertEquals(500L, map.get(50L));
            assertTrue(map.containsKey(30L));
            assertTrue(map.containsValue(300L));

            // Navigation
            assertEquals(10L, map.firstKey());
            assertEquals(100L, map.lastKey());
            assertEquals(30L, map.ceilingKey(25L));
            assertEquals(30L, map.floorKey(35L));
            assertEquals(40L, map.higherKey(30L));
            assertEquals(20L, map.lowerKey(30L));

            // subMap [30, 70)
            NavigableMap<Long, Long> sub = map.subMap(30L, true, 70L, false);
            assertEquals(4, sub.size()); // 30, 40, 50, 60
            assertTrue(sub.containsKey(30L));
            assertFalse(sub.containsKey(70L));

            // descendingMap
            NavigableMap<Long, Long> desc = map.descendingMap();
            assertEquals(100L, desc.firstKey());
            assertEquals(10L, desc.lastKey());

            // Poll
            Map.Entry<Long, Long> first = map.pollFirstEntry();
            assertEquals(10L, first.getKey());
            assertEquals(100L, first.getValue());
            assertEquals(9, map.size());
        }
    }

    @Test
    @DisplayName("ExpanseJavaStrMap standard Map contract")
    void strMapContract() {
        try (ExpanseStrMap rawMap = new ExpanseStrMap()) {
            Map<String, Long> map = rawMap.asJavaMap();
            map.put("alpha", 1L);
            map.put("beta", 2L);
            map.put("gamma", 3L);

            assertEquals(3, map.size());
            assertEquals(1L, map.get("alpha"));
            assertEquals(2L, map.get("beta"));
            assertEquals(3L, map.get("gamma"));
            assertNull(map.get("delta"));

            assertEquals(1L, map.remove("alpha"));
            assertEquals(2, map.size());
        }
    }

    // --- Unsigned 64-bit ordering across the 2^63 boundary ------------------
    // The native trie orders keys as UNSIGNED u64. Keys >= 2^63 (whose signed
    // representation is negative) must therefore sort ABOVE small positive keys.
    // A signed wrapper would place -1L (== 2^64-1, the LARGEST key) first and
    // wrongly exclude it from tailSet(0L)/tailMap(0L). These tests would have
    // caught the pre-fix signed comparisons in ExpanseJavaNavigableSet/Map.

    private static final long K_SMALL = 1L;                 // 1
    private static final long K_MID = Long.MIN_VALUE;       // 2^63
    private static final long K_MAX = -1L;                  // 2^64 - 1 (largest unsigned)

    @Test
    @DisplayName("ExpanseJavaNavigableSet orders keys as unsigned u64 across 2^63")
    void unsignedOrderingSet() {
        try (ExpanseSet rawSet = new ExpanseSet()) {
            NavigableSet<Long> set = rawSet.asJavaSet();
            set.add(K_MAX);
            set.add(K_SMALL);
            set.add(K_MID);

            // Iteration order must be unsigned-ascending: 1, 2^63, 2^64-1.
            Iterator<Long> it = set.iterator();
            assertEquals(K_SMALL, it.next());
            assertEquals(K_MID, it.next());
            assertEquals(K_MAX, it.next());
            assertFalse(it.hasNext());

            assertEquals(K_SMALL, set.first());
            assertEquals(K_MAX, set.last());

            // Navigation across the signed/unsigned boundary.
            assertEquals(K_MID, set.higher(K_SMALL));
            assertEquals(K_MID, set.lower(K_MAX));
            assertEquals(K_MID, set.ceiling(K_MID));
            assertEquals(K_MID, set.floor(K_MID));
            assertEquals(K_MAX, set.ceiling(K_MAX));

            // tailSet(0, inclusive) must include EVERY key, including 2^64-1.
            NavigableSet<Long> tail = set.tailSet(0L, true);
            assertEquals(3, tail.size());
            assertTrue(tail.contains(K_MAX), "tailSet(0) must contain 2^64-1 under unsigned order");
            assertTrue(tail.contains(K_MID));

            // headSet(2^63, exclusive) contains only the small key.
            NavigableSet<Long> head = set.headSet(K_MID, false);
            assertEquals(1, head.size());
            assertTrue(head.contains(K_SMALL));

            // comparator must be unsigned: 1 < 2^64-1 (signed would give the reverse).
            assertTrue(set.comparator().compare(K_SMALL, K_MAX) < 0);
        }
    }

    @Test
    @DisplayName("ExpanseJavaNavigableMap orders keys as unsigned u64 across 2^63")
    void unsignedOrderingMap() {
        try (ExpanseMap rawMap = new ExpanseMap()) {
            NavigableMap<Long, Long> map = rawMap.asJavaMap();
            map.put(K_MAX, 300L);
            map.put(K_SMALL, 100L);
            map.put(K_MID, 200L);

            assertEquals(K_SMALL, map.firstKey());
            assertEquals(K_MAX, map.lastKey());
            assertEquals(K_MID, map.higherKey(K_SMALL));
            assertEquals(K_MID, map.lowerKey(K_MAX));
            assertEquals(K_MID, map.ceilingKey(K_MID));
            assertEquals(K_MID, map.floorKey(K_MID));

            // tailMap(0, inclusive) must include the 2^64-1 key.
            NavigableMap<Long, Long> tail = map.tailMap(0L, true);
            assertEquals(3, tail.size());
            assertEquals(300L, tail.get(K_MAX));

            assertTrue(map.comparator().compare(K_SMALL, K_MAX) < 0);
        }
    }
}
