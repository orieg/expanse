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

    private static final OptionalLong ABSENT = OptionalLong.empty();

    private static void assertExchange(SyncExpanseMap.Exchange got, boolean stored, OptionalLong observed) {
        assertEquals(new SyncExpanseMap.Exchange(stored, observed), got);
    }

    @Test
    @DisplayName("SyncExpanseMap.compareExchange covers every branch")
    void compareExchangeBranches() {
        try (SyncExpanseMap m = new SyncExpanseMap()) {
            // absent == absent with nothing to store: succeeds, observes absent.
            assertExchange(m.compareExchange(1, ABSENT, ABSENT), true, ABSENT);
            assertFalse(m.containsKey(1));
            // insert-if-absent on an absent key.
            assertExchange(m.compareExchange(1, ABSENT, OptionalLong.of(10)), true, ABSENT);
            assertEquals(OptionalLong.of(10), m.get(1));
            // insert-if-absent against a present key fails and reports the word seen.
            assertExchange(m.compareExchange(1, ABSENT, OptionalLong.of(99)), false, OptionalLong.of(10));
            // absent == absent against a present key fails.
            assertExchange(m.compareExchange(1, ABSENT, ABSENT), false, OptionalLong.of(10));
            // expected present, wrong value: fails, value unchanged.
            assertExchange(m.compareExchange(1, OptionalLong.of(11), OptionalLong.of(12)), false, OptionalLong.of(10));
            assertEquals(OptionalLong.of(10), m.get(1));
            // expected present against an absent key: fails, observes absent.
            assertExchange(m.compareExchange(2, OptionalLong.of(5), OptionalLong.of(6)), false, ABSENT);
            assertFalse(m.containsKey(2));
            // replace on match.
            assertExchange(m.compareExchange(1, OptionalLong.of(10), OptionalLong.of(20)), true, OptionalLong.of(10));
            assertEquals(OptionalLong.of(20), m.get(1));
            // remove-if-equals: mismatch leaves it, match removes it.
            assertExchange(m.compareExchange(1, OptionalLong.of(21), ABSENT), false, OptionalLong.of(20));
            assertEquals(OptionalLong.of(20), m.get(1));
            assertExchange(m.compareExchange(1, OptionalLong.of(20), ABSENT), true, OptionalLong.of(20));
            assertFalse(m.containsKey(1));
            // Every long is a legal value, 0 and -1 (all bits set) included.
            assertExchange(m.compareExchange(3, ABSENT, OptionalLong.of(0)), true, ABSENT);
            assertExchange(m.compareExchange(3, OptionalLong.of(0), OptionalLong.of(-1L)), true, OptionalLong.of(0));
            assertEquals(OptionalLong.of(-1L), m.get(3));
        }
    }

    @Test
    @DisplayName("SyncExpanseMap.getOrInsert stores only into an absent key")
    void getOrInsertStoresOnlyIfAbsent() {
        try (SyncExpanseMap m = new SyncExpanseMap()) {
            assertEquals(ABSENT, m.getOrInsert(7, 70));
            assertEquals(OptionalLong.of(70), m.get(7));
            assertEquals(OptionalLong.of(70), m.getOrInsert(7, 71));
            assertEquals(OptionalLong.of(70), m.get(7));
            // Zero is a value, not absence.
            assertEquals(ABSENT, m.getOrInsert(8, 0));
            assertEquals(OptionalLong.of(0), m.getOrInsert(8, 5));
        }
    }

    @Test
    @DisplayName("SyncExpanseMap.update stores, removes and propagates exceptions")
    void updateStoresRemovesAndPropagates() {
        try (SyncExpanseMap m = new SyncExpanseMap()) {
            List<OptionalLong> seen = new ArrayList<>();
            java.util.function.UnaryOperator<OptionalLong> inc = cur -> {
                seen.add(cur);
                return OptionalLong.of(cur.orElse(0) + 1);
            };
            assertEquals(ABSENT, m.update(5, inc));
            assertEquals(OptionalLong.of(1), m.update(5, inc));
            assertEquals(OptionalLong.of(2), m.get(5));
            assertEquals(List.of(ABSENT, OptionalLong.of(1)), seen);
            // An empty result removes the key and reports the replaced value.
            assertEquals(OptionalLong.of(2), m.update(5, cur -> ABSENT));
            assertFalse(m.containsKey(5));
            assertEquals(ABSENT, m.update(5, cur -> ABSENT));
            // An exception from fn propagates and leaves the key unchanged.
            m.insert(6, 60);
            assertThrows(IllegalStateException.class, () -> m.update(6, cur -> {
                throw new IllegalStateException("fn failed");
            }));
            assertEquals(OptionalLong.of(60), m.get(6));
        }
    }

    @Test
    @DisplayName("SyncExpanseMap.update retries on interference")
    void updateRetriesOnInterference() {
        try (SyncExpanseMap m = new SyncExpanseMap()) {
            m.insert(1, 100);
            List<OptionalLong> calls = new ArrayList<>();
            OptionalLong prev = m.update(1, cur -> {
                calls.add(cur);
                if (calls.size() == 1) {
                    m.insert(1, 200); // a competing write lands between read and exchange
                }
                return OptionalLong.of(cur.getAsLong() + 1);
            });
            assertEquals(OptionalLong.of(200), prev);
            assertEquals(List.of(OptionalLong.of(100), OptionalLong.of(200)), calls);
            assertEquals(OptionalLong.of(201), m.get(1));
        }
    }

    @Test
    @DisplayName("SyncExpanseMap.update from many threads ends at the exact total")
    void updateConcurrentCounterIsExact() throws Exception {
        int threads = 8;
        int perThread = 5000;
        try (SyncExpanseMap m = new SyncExpanseMap()) {
            ExecutorService executor = Executors.newFixedThreadPool(threads);
            CountDownLatch start = new CountDownLatch(1);
            AtomicInteger winners = new AtomicInteger();
            List<Future<?>> futures = new ArrayList<>();
            for (int t = 0; t < threads; t++) {
                final long id = t;
                futures.add(executor.submit(() -> {
                    start.await();
                    for (int i = 0; i < perThread; i++) {
                        m.update(0, cur -> OptionalLong.of(cur.orElse(0) + 1));
                    }
                    // Of any number of threads racing on one absent key, exactly one stores.
                    if (m.getOrInsert(42, id).isEmpty()) {
                        winners.incrementAndGet();
                    }
                    return null;
                }));
            }
            start.countDown();
            for (Future<?> f : futures) {
                f.get(60, TimeUnit.SECONDS);
            }
            executor.shutdown();
            assertEquals(OptionalLong.of((long) threads * perThread), m.get(0));
            assertEquals(1, winners.get());
        }
    }
}
