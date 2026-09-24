package io.github.orieg.expanse;

import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.OptionalLong;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.*;

class SyncExpanseConcurrentTest {

    @Test
    @DisplayName("SyncExpanseSet concurrent readers during mutation")
    void concurrentSetReaders() throws Exception {
        int numKeys = 1000;
        int numReaderThreads = 4;

        try (SyncExpanseSet set = new SyncExpanseSet()) {
            for (long i = 0; i < numKeys; i++) {
                set.insert(i);
            }
            assertEquals(numKeys, set.size());

            ExecutorService executor = Executors.newFixedThreadPool(numReaderThreads);
            CountDownLatch startLatch = new CountDownLatch(1);
            AtomicBoolean stop = new AtomicBoolean(false);
            AtomicInteger totalReads = new AtomicInteger(0);

            List<Future<?>> futures = new ArrayList<>();
            for (int t = 0; t < numReaderThreads; t++) {
                futures.add(executor.submit(() -> {
                    try (SyncExpanseSet.Reader reader = set.reader()) {
                        startLatch.await();
                        while (!stop.get()) {
                            for (long k = 0; k < numKeys; k++) {
                                if (reader.contains(k)) {
                                    totalReads.incrementAndGet();
                                }
                            }
                        }
                    } catch (Exception e) {
                        throw new RuntimeException(e);
                    }
                }));
            }

            startLatch.countDown();

            // Writer mutates while readers are scanning
            for (long k = numKeys; k < numKeys + 500; k++) {
                set.insert(k);
                Thread.yield();
            }

            Thread.sleep(100);
            stop.set(true);

            for (Future<?> f : futures) {
                f.get(5, TimeUnit.SECONDS);
            }
            executor.shutdown();

            assertTrue(totalReads.get() > 0);
            assertEquals(numKeys + 500, set.size());
        }
    }

    @Test
    @DisplayName("SyncExpanseMap concurrent readers during mutation")
    void concurrentMapReaders() throws Exception {
        int numKeys = 1000;
        int numReaderThreads = 4;

        try (SyncExpanseMap map = new SyncExpanseMap()) {
            for (long i = 0; i < numKeys; i++) {
                map.insert(i, i * 10);
            }
            assertEquals(numKeys, map.size());

            ExecutorService executor = Executors.newFixedThreadPool(numReaderThreads);
            CountDownLatch startLatch = new CountDownLatch(1);
            AtomicBoolean stop = new AtomicBoolean(false);
            AtomicInteger hits = new AtomicInteger(0);

            List<Future<?>> futures = new ArrayList<>();
            for (int t = 0; t < numReaderThreads; t++) {
                futures.add(executor.submit(() -> {
                    try (SyncExpanseMap.Reader reader = map.reader()) {
                        startLatch.await();
                        while (!stop.get()) {
                            for (long k = 0; k < numKeys; k++) {
                                OptionalLong val = reader.get(k);
                                if (val.isPresent() && val.getAsLong() == k * 10) {
                                    hits.incrementAndGet();
                                }
                            }
                        }
                    } catch (Exception e) {
                        throw new RuntimeException(e);
                    }
                }));
            }

            startLatch.countDown();

            // Writer mutates while readers scan
            for (long k = numKeys; k < numKeys + 500; k++) {
                map.insert(k, k * 10);
                Thread.yield();
            }

            Thread.sleep(100);
            stop.set(true);

            for (Future<?> f : futures) {
                f.get(5, TimeUnit.SECONDS);
            }
            executor.shutdown();

            assertTrue(hits.get() > 0);
            assertEquals(numKeys + 500, map.size());
        }
    }

    @Test
    @DisplayName("SyncExpanseMap.close() invalidates live readers and is idempotent")
    void mapCloseInvalidatesReaders() {
        SyncExpanseMap map = new SyncExpanseMap();
        map.insert(1L, 100L);
        SyncExpanseMap.Reader reader = map.reader();
        assertEquals(OptionalLong.of(100L), reader.get(1L));

        // Closing the map must invalidate the still-live reader; a subsequent use is
        // a use-after-free at the native level, so it must be refused, not executed.
        map.close();
        assertThrows(IllegalStateException.class, () -> reader.get(1L));

        // close() is idempotent: a second close (double-close race) must be a no-op.
        assertDoesNotThrow(map::close);
        // Closing an already-invalidated reader must also be safe (no double-free).
        assertDoesNotThrow(reader::close);
    }

    @Test
    @DisplayName("SyncExpanseSet.close() invalidates live readers and is idempotent")
    void setCloseInvalidatesReaders() {
        SyncExpanseSet set = new SyncExpanseSet();
        set.insert(7L);
        SyncExpanseSet.Reader reader = set.reader();
        assertTrue(reader.contains(7L));

        set.close();
        assertThrows(IllegalStateException.class, () -> reader.contains(7L));

        assertDoesNotThrow(set::close);
        assertDoesNotThrow(reader::close);
    }

    @Test
    @DisplayName("SyncExpanseMap/SyncExpanseSet memHeld and shrinkToFit after a half drain")
    void memHeldAndShrinkToFit() {
        // The epoch collector keeps the freed blocks. shrinkToFit returns those
        // past their grace period and memHeld falls by exactly what it reports;
        // blocks still in their grace period stay held.
        int n = 50_000;
        try (SyncExpanseMap map = new SyncExpanseMap(); SyncExpanseSet set = new SyncExpanseSet()) {
            for (long k = 0; k < n; k++) {
                map.insert(k * 0x9E3779B1L, k);
                set.insert(k * 0x9E3779B1L);
            }
            for (long k = 0; k < n; k += 2) {
                assertTrue(map.remove(k * 0x9E3779B1L));
                assertTrue(set.remove(k * 0x9E3779B1L));
            }
            long mapHeld = map.memHeld();
            assertTrue(mapHeld > 0);
            long mapReleased = map.shrinkToFit();
            assertTrue(mapReleased >= 0);
            assertEquals(mapHeld - mapReleased, map.memHeld());

            long setHeld = set.memHeld();
            assertTrue(setHeld > 0);
            long setReleased = set.shrinkToFit();
            assertTrue(setReleased >= 0);
            assertEquals(setHeld - setReleased, set.memHeld());

            for (long k = 1; k < n; k += 2) {
                assertEquals(OptionalLong.of(k), map.get(k * 0x9E3779B1L));
                assertTrue(set.contains(k * 0x9E3779B1L));
            }
        }
    }
}
