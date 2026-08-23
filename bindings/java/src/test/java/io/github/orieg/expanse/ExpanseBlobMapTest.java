package io.github.orieg.expanse;

import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.Optional;

import static org.junit.jupiter.api.Assertions.*;

class ExpanseBlobMapTest {

    @Test
    @DisplayName("Inline payloads (0..7 bytes) and arena slabs in Java bindings")
    void inlineAndArenaPayloads() {
        try (ExpanseBlobMap map = new ExpanseBlobMap(64 * 1024)) {
            assertTrue(map.isEmpty());
            assertEquals(0, map.len());

            // 0..7 bytes inline payloads
            byte[] empty = new byte[0];
            byte[] single = "a".getBytes(StandardCharsets.UTF_8);
            byte[] seven = "1234567".getBytes(StandardCharsets.UTF_8);

            assertTrue(map.insert(1, empty));
            assertTrue(map.insert(2, single));
            assertTrue(map.insert(3, seven));

            // Arena payloads (> 7 bytes)
            byte[] eight = "12345678".getBytes(StandardCharsets.UTF_8);
            byte[] large = new byte[1024];
            for (int i = 0; i < large.length; i++) {
                large[i] = (byte) (i & 0xFF);
            }

            assertTrue(map.insert(4, eight, 100));
            assertTrue(map.insert(5, large, 200));

            assertEquals(5, map.len());
            assertFalse(map.isEmpty());
            assertTrue(map.containsKey(1));
            assertTrue(map.containsKey(5));
            assertFalse(map.containsKey(99));

            // Verify inline payload 1
            Optional<ExpanseBlobMap.BlobRecord> r1 = map.get(1);
            assertTrue(r1.isPresent());
            assertTrue(r1.get().isInline());
            assertEquals(0, r1.get().data().length);
            assertEquals(0, r1.get().hotMeta());

            // Verify inline payload 2
            Optional<ExpanseBlobMap.BlobRecord> r2 = map.get(2);
            assertTrue(r2.isPresent());
            assertTrue(r2.get().isInline());
            assertArrayEquals(single, r2.get().data());

            // Verify inline payload 3
            Optional<ExpanseBlobMap.BlobRecord> r3 = map.get(3);
            assertTrue(r3.isPresent());
            assertTrue(r3.get().isInline());
            assertArrayEquals(seven, r3.get().data());

            // Verify arena payload 4
            Optional<ExpanseBlobMap.BlobRecord> r4 = map.get(4);
            assertTrue(r4.isPresent());
            assertFalse(r4.get().isInline());
            assertArrayEquals(eight, r4.get().data());
            assertEquals(100, r4.get().hotMeta());

            // Verify arena payload 5
            Optional<ExpanseBlobMap.BlobRecord> r5 = map.get(5);
            assertTrue(r5.isPresent());
            assertFalse(r5.get().isInline());
            assertArrayEquals(large, r5.get().data());
            assertEquals(200, r5.get().hotMeta());

            // getBytes helper
            assertArrayEquals(eight, map.getBytes(4));
            assertNull(map.getBytes(999));

            // Overwrite and remove
            byte[] updated = "updated-payload".getBytes(StandardCharsets.UTF_8);
            assertTrue(map.insert(2, updated, 50));
            assertArrayEquals(updated, map.getBytes(2));

            assertTrue(map.remove(2));
            assertFalse(map.containsKey(2));
            assertEquals(4, map.len());

            // Compaction
            assertTrue(map.compact());
            assertEquals(4, map.len());

            // Clear
            map.clear();
            assertEquals(0, map.len());
            assertTrue(map.isEmpty());
        }
    }
}
