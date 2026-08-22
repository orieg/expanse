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
}
