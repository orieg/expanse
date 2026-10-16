package io.github.orieg.expanse;

import static org.junit.jupiter.api.Assertions.*;

import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.OptionalInt;
import java.util.OptionalLong;
import java.util.concurrent.Callable;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicLong;
import java.util.function.Function;
import java.util.function.IntFunction;
import java.util.function.Supplier;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

/** Concurrent tests of the shared string, bytes, ordered-bytes and blob maps (#1538). */
class SyncExpanseSharedMapsConcurrentTest {

    private static final int PREFILL = 1000;
    private static final int EXTRA = 500;
    private static final int READERS = 4;

    private static long valueOf(int i) {
        return i * 10L + 7;
    }

    private static byte[] bytesKey(int i) {
        // Big-endian with leading zero bytes: embedded zeros are part of the key.
        return ByteBuffer.allocate(8).putLong(i).array();
    }

    /** A thread's point reader and what it takes to close it. */
    private record PointReader<K>(Function<K, OptionalLong> get, Runnable close) {}

    /**
     * Readers scan while the writer inserts more keys and rewrites some; every value a reader
     * sees must be the one its key carries. Then threads race read-modify-write on one key and
     * on one absent key.
     */
    private static <K> void pointWorkload(SyncKeyedMap<K> map, IntFunction<K> key, Supplier<PointReader<K>> newReader,
            Callable<Void> extra) throws Exception {
        for (int i = 0; i < PREFILL; i++) {
            map.insert(key.apply(i), valueOf(i));
        }
        assertEquals(PREFILL, map.size());

        ExecutorService executor = Executors.newFixedThreadPool(READERS + 1);
        CountDownLatch start = new CountDownLatch(1);
        AtomicBoolean stop = new AtomicBoolean(false);
        AtomicInteger hits = new AtomicInteger();
        List<Future<?>> futures = new ArrayList<>();
        for (int t = 0; t < READERS; t++) {
            futures.add(executor.submit(() -> {
                PointReader<K> reader = newReader.get();
                try {
                    start.await();
                    while (!stop.get()) {
                        for (int i = 0; i < PREFILL + EXTRA; i++) {
                            OptionalLong v = reader.get().apply(key.apply(i));
                            if (v.isPresent()) {
                                assertEquals(valueOf(i), v.getAsLong(), "torn or misrouted read of key " + i);
                                hits.incrementAndGet();
                            }
                        }
                    }
                } finally {
                    reader.close().run();
                }
                return null;
            }));
        }
        if (extra != null) {
            futures.add(executor.submit(() -> {
                start.await();
                extra.call();
                return null;
            }));
        }
        start.countDown();

        for (int i = PREFILL; i < PREFILL + EXTRA; i++) {
            map.insert(key.apply(i), valueOf(i));
            if (i % 3 == 0) {
                map.remove(key.apply(i - PREFILL)); // removals beside readers
                map.insert(key.apply(i - PREFILL), valueOf(i - PREFILL));
            }
            Thread.yield();
        }
        Thread.sleep(100);
        stop.set(true);
        for (Future<?> f : futures) {
            f.get(30, TimeUnit.SECONDS);
        }
        assertTrue(hits.get() > 0);
        assertEquals(PREFILL + EXTRA, map.size());

        // Threads race update on one key: the total is exact. Of any number racing getOrInsert
        // on one absent key, exactly one stores.
        int threads = 8;
        int perThread = 2000;
        K counter = key.apply(PREFILL + EXTRA + 1);
        K contested = key.apply(PREFILL + EXTRA + 2);
        CountDownLatch go = new CountDownLatch(1);
        AtomicInteger winners = new AtomicInteger();
        List<Future<?>> racers = new ArrayList<>();
        for (int t = 0; t < threads; t++) {
            final long id = t;
            racers.add(executor.submit(() -> {
                go.await();
                for (int i = 0; i < perThread; i++) {
                    map.update(counter, cur -> OptionalLong.of(cur.orElse(0) + 1));
                }
                if (map.getOrInsert(contested, id).isEmpty()) {
                    winners.incrementAndGet();
                }
                return null;
            }));
        }
        go.countDown();
        for (Future<?> f : racers) {
            f.get(60, TimeUnit.SECONDS);
        }
        executor.shutdown();
        assertEquals(OptionalLong.of((long) threads * perThread), map.get(counter));
        assertEquals(1, winners.get());
        assertTrue(map.get(contested).isPresent());
    }

    @Test
    @DisplayName("SyncExpanseStrMap concurrent readers, ordered reads and read-modify-write")
    void strMap() throws Exception {
        try (SyncExpanseStrMap map = new SyncExpanseStrMap()) {
            AtomicLong navigations = new AtomicLong();
            IntFunction<String> key = i -> String.format("key-%06d", i);
            pointWorkload(map, key, () -> {
                SyncExpanseStrMap.Reader r = map.reader();
                return new PointReader<>(r::get, r::close);
            }, () -> {
                // Ordered reads beside the writer: each entry is at or after the search key
                // and carries the value its key carries.
                try (SyncExpanseStrMap.Reader r = map.reader()) {
                    for (int round = 0; round < 200; round++) {
                        for (int i = 0; i < PREFILL; i += 37) {
                            Optional<SyncExpanseStrMap.Entry> e = r.ceilingEntry(key.apply(i));
                            if (e.isPresent()) {
                                assertTrue(e.get().key().compareTo(key.apply(i)) >= 0);
                                int n = Integer.parseInt(e.get().key().substring(4));
                                assertEquals(valueOf(n), e.get().value());
                                navigations.incrementAndGet();
                            }
                        }
                        assertTrue(r.firstEntry().isPresent());
                        assertTrue(r.lastEntry().isPresent());
                    }
                }
                return null;
            });
            assertTrue(navigations.get() > 0);
        }
    }

    @Test
    @DisplayName("SyncExpanseBytesMap concurrent readers and read-modify-write")
    void bytesMap() throws Exception {
        try (SyncExpanseBytesMap map = new SyncExpanseBytesMap()) {
            pointWorkload(map, SyncExpanseSharedMapsConcurrentTest::bytesKey, () -> {
                SyncExpanseBytesMap.Reader r = map.reader();
                return new PointReader<>(r::get, r::close);
            }, null);
        }
    }

    @Test
    @DisplayName("SyncExpanseOrderedBytesMap concurrent readers, ordered reads on the map, read-modify-write")
    void orderedBytesMap() throws Exception {
        try (SyncExpanseOrderedBytesMap map = new SyncExpanseOrderedBytesMap()) {
            AtomicLong navigations = new AtomicLong();
            AtomicLong writerProgress = new AtomicLong();
            pointWorkload(map, SyncExpanseSharedMapsConcurrentTest::bytesKey, () -> {
                SyncExpanseOrderedBytesMap.Reader r = map.reader();
                return new PointReader<>(r::get, r::close);
            }, () -> {
                // Ordered reads block writers, so they must still let the writer finish.
                for (int round = 0; round < 100; round++) {
                    for (int i = 0; i < PREFILL; i += 41) {
                        byte[] search = bytesKey(i);
                        Optional<SyncExpanseOrderedBytesMap.Entry> e = map.ceilingEntry(search);
                        if (e.isPresent()) {
                            assertTrue(java.util.Arrays.compareUnsigned(e.get().key(), search) >= 0);
                            int n = ByteBuffer.wrap(e.get().key()).getInt(4);
                            assertEquals(valueOf(n), e.get().value());
                            navigations.incrementAndGet();
                        }
                    }
                    assertTrue(map.firstEntry().isPresent());
                    assertTrue(map.lastEntry().isPresent());
                    writerProgress.set(map.len());
                }
                return null;
            });
            assertTrue(navigations.get() > 0);
            assertTrue(writerProgress.get() >= PREFILL);
        }
    }

    // ---- blob map -------------------------------------------------------------------------

    /** Length and fill byte are both functions of the version, so a torn read is visible. */
    private static byte[] versioned(int version) {
        byte[] b = new byte[1 + (version * 53) % 700]; // below and above the initial read buffer
        java.util.Arrays.fill(b, (byte) version);
        return b;
    }

    private static void assertConsistent(SyncExpanseBlobMap.Blob v) {
        byte[] d = v.data();
        assertTrue(d.length >= 1);
        for (byte x : d) {
            assertEquals(d[0], x, "torn payload");
        }
        int version = d[0] & 0xFF;
        // The fill byte is the version modulo 256; the length is a function of the whole version.
        boolean lengthMatches = false;
        for (int full = version; full < 100_000 && !lengthMatches; full += 256) {
            lengthMatches = versioned(full).length == d.length;
        }
        assertTrue(lengthMatches, "payload length does not match its fill byte");
        // Metadata is kept at every payload length (#1548).
        assertEquals(version, v.hotMeta(), "metadata does not match the payload");
    }

    @Test
    @DisplayName("SyncExpanseBlobMap concurrent readers see whole values; getOrInsert and metadata CAS are exact")
    void blobMap() throws Exception {
        int keys = 64;
        try (SyncExpanseBlobMap map = new SyncExpanseBlobMap(256 * 1024)) {
            for (int k = 0; k < keys; k++) {
                assertTrue(map.insert(k, versioned(0), 0));
            }
            ExecutorService executor = Executors.newFixedThreadPool(READERS + 1);
            CountDownLatch start = new CountDownLatch(1);
            AtomicBoolean stop = new AtomicBoolean(false);
            AtomicInteger reads = new AtomicInteger();
            AtomicInteger largeReads = new AtomicInteger();
            List<Future<?>> futures = new ArrayList<>();
            for (int t = 0; t < READERS; t++) {
                final boolean oneShot = t == 0; // one thread reads through the map, the rest through readers
                futures.add(executor.submit(() -> {
                    try (SyncExpanseBlobMap.Reader r = map.reader()) {
                        start.await();
                        while (!stop.get()) {
                            for (int k = 0; k < keys; k++) {
                                Optional<SyncExpanseBlobMap.Blob> v = oneShot ? map.get(k) : r.get(k);
                                if (v.isPresent()) {
                                    assertConsistent(v.get());
                                    reads.incrementAndGet();
                                    if (v.get().data().length > 256) {
                                        largeReads.incrementAndGet();
                                    }
                                }
                            }
                        }
                    }
                    return null;
                }));
            }
            start.countDown();
            // The writer rewrites each key with values of varying length, 1..700 bytes, and
            // removes and reinserts some.
            for (int version = 1; version <= 400; version++) {
                for (int k = 0; k < keys; k++) {
                    if ((version + k) % 11 == 0) {
                        map.remove(k);
                    }
                    assertTrue(map.insert(k, versioned(version), version & 0xFF));
                }
                Thread.yield();
            }
            Thread.sleep(100);
            stop.set(true);
            for (Future<?> f : futures) {
                f.get(60, TimeUnit.SECONDS);
            }
            assertTrue(reads.get() > 0);
            assertTrue(largeReads.get() > 0, "the too-small buffer path was exercised");
            assertEquals(keys, map.size());

            // Of any number of threads racing getOrInsert on one absent key, exactly one stores.
            // Racing metadata compare-and-swaps on one key count exactly.
            int threads = 8;
            int perThread = 1000;
            map.insert(1000, new byte[200], 0);
            CountDownLatch go = new CountDownLatch(1);
            AtomicInteger winners = new AtomicInteger();
            List<Future<?>> racers = new ArrayList<>();
            for (int t = 0; t < threads; t++) {
                final int id = t;
                racers.add(executor.submit(() -> {
                    go.await();
                    if (map.getOrInsert(2000, versioned(id + 1), id + 1).stored()) {
                        winners.incrementAndGet();
                    }
                    for (int i = 0; i < perThread; i++) {
                        int expected = map.getMeta(1000).getAsInt();
                        while (true) {
                            var x = map.compareExchangeMeta(1000, OptionalInt.of(expected), OptionalInt.of(expected + 1));
                            if (x.stored()) {
                                break;
                            }
                            expected = x.observed().orElseThrow();
                        }
                    }
                    return null;
                }));
            }
            go.countDown();
            for (Future<?> f : racers) {
                f.get(60, TimeUnit.SECONDS);
            }
            executor.shutdown();
            assertEquals(1, winners.get());
            assertEquals(OptionalInt.of(threads * perThread), map.getMeta(1000));
            assertConsistent(map.get(2000).orElseThrow());
        }
    }
}
