package io.github.orieg.expanse;

import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.OptionalLong;

import static org.junit.jupiter.api.Assertions.*;

class ExpanseStrMapTest {

    @Test
    @DisplayName("Basic string map operations: insert, get, remove, putAndGetOld")
    void basicOperations() {
        try (ExpanseStrMap map = new ExpanseStrMap()) {
            assertTrue(map.isEmpty());
            assertEquals(0, map.size());

            assertTrue(map.put("apple", 100));
            assertTrue(map.put("banana", 200));
            assertTrue(map.put("cherry", 300));
            assertEquals(3, map.size());
            assertTrue(map.memoryUsed() > 0);

            // Re-insert
            assertFalse(map.put("apple", 105));
            assertEquals(3, map.size());
            assertEquals(OptionalLong.of(105), map.get("apple"));

            // putAndGetOld
            OptionalLong old = map.putAndGetOld("apple", 110);
            assertEquals(OptionalLong.of(105), old);
            assertEquals(OptionalLong.of(110), map.get("apple"));

            // Lookups
            assertEquals(OptionalLong.of(200), map.get("banana"));
            assertEquals(OptionalLong.of(300), map.get("cherry"));
            assertEquals(OptionalLong.empty(), map.get("durian"));
            assertTrue(map.containsKey("banana"));
            assertFalse(map.containsKey("durian"));

            // Value slot operations
            MemorySegment slot = map.insertSlot("date");
            assertNotNull(slot);
            assertEquals(0L, slot.get(ValueLayout.JAVA_LONG, 0));
            slot.set(ValueLayout.JAVA_LONG, 0, 400L);
            assertEquals(OptionalLong.of(400), map.get("date"));
            assertEquals(4, map.size());

            // Remove
            assertTrue(map.remove("apple"));
            assertFalse(map.containsKey("apple"));
            assertEquals(3, map.size());

            map.clear();
            assertEquals(0, map.size());
            assertTrue(map.isEmpty());
        }
    }

    @Test
    @DisplayName("Ordered string navigation: firstEntry, lastEntry, nextAfter, prevBefore")
    void stringNavigation() {
        try (ExpanseStrMap map = new ExpanseStrMap()) {
            map.put("cat", 1);
            map.put("apple", 2);
            map.put("dog", 3);
            map.put("banana", 4);

            assertEquals(Optional.of(new ExpanseStrMap.Entry("apple", 2)), map.firstEntry());
            assertEquals(Optional.of(new ExpanseStrMap.Entry("dog", 3)), map.lastEntry());

            // nextAfter
            assertEquals(Optional.of(new ExpanseStrMap.Entry("banana", 4)), map.nextAfter("apple"));
            assertEquals(Optional.of(new ExpanseStrMap.Entry("cat", 1)), map.nextAfter("banana"));
            assertEquals(Optional.of(new ExpanseStrMap.Entry("dog", 3)), map.nextAfter("cat"));
            assertEquals(Optional.empty(), map.nextAfter("dog"));

            // prevBefore
            assertEquals(Optional.of(new ExpanseStrMap.Entry("cat", 1)), map.prevBefore("dog"));
            assertEquals(Optional.of(new ExpanseStrMap.Entry("banana", 4)), map.prevBefore("cat"));
            assertEquals(Optional.empty(), map.prevBefore("apple"));

            // Iteration
            List<String> keys = new ArrayList<>();
            map.forEach((k, v) -> keys.add(k));
            assertEquals(List.of("apple", "banana", "cat", "dog"), keys);
        }
    }

    @Test
    @DisplayName("Navigation returns keys longer than the default scratch buffer (>1 KiB)")
    void longKeysAreNotTruncated() {
        // The pre-fix nav used a fixed 1024-byte buffer and treated the native
        // 'false' (buffer-too-small) as 'no more keys', so a key >= 1024 bytes made
        // the whole map look empty from first()/last()/next()/prev(). The _ex-based
        // retry loop must grow the buffer and return the full key.
        try (ExpanseStrMap map = new ExpanseStrMap()) {
            String shortKey = "aaa";
            String longKey = "b".repeat(4096);   // 4 KiB, far past the 1024 default
            String longerKey = "c".repeat(8192);
            map.put(shortKey, 1L);
            map.put(longKey, 2L);
            map.put(longerKey, 3L);

            // firstEntry / lastEntry must see the long keys, not report empty.
            Optional<ExpanseStrMap.Entry> first = map.firstEntry();
            assertTrue(first.isPresent());
            assertEquals(shortKey, first.get().key());   // 'a' < 'b' < 'c'
            Optional<ExpanseStrMap.Entry> last = map.lastEntry();
            assertTrue(last.isPresent());
            assertEquals(longerKey, last.get().key());
            assertEquals(8192, last.get().key().length());
            assertEquals(3L, last.get().value());

            // Forward navigation must step INTO and OUT OF the 4 KiB key.
            Optional<ExpanseStrMap.Entry> afterShort = map.nextAfter(shortKey);
            assertTrue(afterShort.isPresent());
            assertEquals(longKey, afterShort.get().key());
            assertEquals(4096, afterShort.get().key().length());
            assertEquals(2L, afterShort.get().value());

            assertEquals(longerKey, map.nextAfter(longKey).map(ExpanseStrMap.Entry::key).orElse(null));

            // Reverse navigation likewise.
            assertEquals(longKey, map.prevBefore(longerKey).map(ExpanseStrMap.Entry::key).orElse(null));

            // Full iteration must visit all three keys in order.
            List<String> keys = new ArrayList<>();
            map.forEach((k, v) -> keys.add(k));
            assertEquals(List.of(shortKey, longKey, longerKey), keys);
        }
    }
}
