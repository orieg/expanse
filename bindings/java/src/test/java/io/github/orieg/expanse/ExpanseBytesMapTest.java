package io.github.orieg.expanse;

import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.nio.charset.StandardCharsets;
import java.util.OptionalLong;

import static org.junit.jupiter.api.Assertions.*;

class ExpanseBytesMapTest {

    @Test
    @DisplayName("Arbitrary byte array keys, embedded nulls, and slot manipulation")
    void byteArrayOperations() {
        try (ExpanseBytesMap map = new ExpanseBytesMap()) {
            assertTrue(map.isEmpty());
            assertEquals(0, map.size());

            byte[] k1 = new byte[]{1, 2, 3};
            byte[] k2 = "hello\0world".getBytes(StandardCharsets.ISO_8859_1); // embedded null
            byte[] emptyKey = new byte[0];

            assertTrue(map.put(k1, 100));
            assertTrue(map.put(k2, 200));
            assertTrue(map.put(emptyKey, 300));
            assertEquals(3, map.size());

            // Get
            assertEquals(OptionalLong.of(100), map.get(k1));
            assertEquals(OptionalLong.of(200), map.get(k2));
            assertEquals(OptionalLong.of(300), map.get(emptyKey));

            // Prefix key is distinct from full key
            byte[] prefix = "hello".getBytes(StandardCharsets.ISO_8859_1);
            assertEquals(OptionalLong.empty(), map.get(prefix));

            // Value slot operations
            MemorySegment slot = map.insertSlot(prefix);
            assertNotNull(slot);
            assertEquals(0L, slot.get(ValueLayout.JAVA_LONG, 0));
            slot.set(ValueLayout.JAVA_LONG, 0, 999L);
            assertEquals(OptionalLong.of(999L), map.get(prefix));
            assertEquals(4, map.size());

            // Remove
            assertTrue(map.remove(k1));
            assertFalse(map.containsKey(k1));
            assertEquals(3, map.size());

            map.clear();
            assertEquals(0, map.size());
            assertTrue(map.isEmpty());
        }
    }
}
