package io.github.orieg.expanse;

import java.util.*;

/**
 * Cross-Runtime Comparative Benchmark Suite for Expanse Java 22+ Panama FFM Bindings.
 * Compares ExpanseMap and ExpanseSet against java.util.HashMap and java.util.HashSet.
 */
public class ExpanseBenchmark {

    static class XorShift64 {
        long state;
        XorShift64(long seed) { this.state = seed; }
        long next() {
            long x = state;
            x ^= (x << 13);
            x ^= (x >>> 7);
            x ^= (x << 17);
            state = x;
            return x;
        }
    }

    private static long[] generateKeys(int pop, String dist) {
        XorShift64 rng = new XorShift64(0x0DDB_1A5E_5EED_0001L);
        long[] keys = new long[pop];
        if ("sequential".equals(dist)) {
            for (int i = 0; i < pop; i++) keys[i] = i;
        } else if ("clustered".equals(dist)) {
            long base = 0;
            for (int i = 0; i < pop; i++) {
                if (i % 256 == 0) base = rng.next() & ~0xFFL;
                keys[i] = base + (i % 256);
            }
        } else {
            for (int i = 0; i < pop; i++) keys[i] = rng.next();
        }
        return keys;
    }

    private static long[] shuffle(long[] arr) {
        long[] copy = arr.clone();
        Random r = new Random(0x9E37_79B9L);
        for (int i = copy.length - 1; i > 0; i--) {
            int j = r.nextInt(i + 1);
            long tmp = copy[i];
            copy[i] = copy[j];
            copy[j] = tmp;
        }
        return copy;
    }

    @FunctionalInterface
    interface BenchmarkOp {
        void run();
    }

    private static double measure(BenchmarkOp op, int rounds) {
        double best = Double.POSITIVE_INFINITY;
        for (int r = 0; r < rounds; r++) {
            long t0 = System.nanoTime();
            op.run();
            long t1 = System.nanoTime();
            double dt = (t1 - t0) / 1e9;
            if (dt < best) best = dt;
        }
        return best;
    }

    public static void runBenchmark(int pop, boolean json) {
        String[] dists = {"random", "sequential", "clustered"};

        if (!json) {
            System.out.println("\n================================================================================");
            System.out.println("  Expanse Java 22+ Panama FFM Comparative Performance Report");
            System.out.println("================================================================================");
        }

        List<String> jsonResults = new ArrayList<>();

        for (String dist : dists) {
            long[] keys = generateKeys(pop, dist);
            long[] probeKeys = shuffle(keys);

            // 1. ExpanseMap
            System.gc();
            double expInsertS;
            double expLookupS;
            double expBytesPerKey;

            try (ExpanseMap map = new ExpanseMap()) {
                expInsertS = measure(() -> {
                    map.clear();
                    for (int i = 0; i < pop; i++) {
                        map.put(keys[i], keys[i] ^ 0x55L);
                    }
                }, 3);

                expLookupS = measure(() -> {
                    long sink = 0;
                    for (int i = 0; i < pop; i++) {
                        OptionalLong v = map.get(probeKeys[i]);
                        if (v.isPresent()) sink ^= v.getAsLong();
                    }
                }, 3);

                expBytesPerKey = (double) map.memoryUsed() / pop;
            }

            // 2. java.util.HashMap
            System.gc();
            Map<Long, Long> jMap = new HashMap<>();
            double jInsertS = measure(() -> {
                jMap.clear();
                for (int i = 0; i < pop; i++) {
                    jMap.put(keys[i], keys[i] ^ 0x55L);
                }
            }, 3);

            double jLookupS = measure(() -> {
                long sink = 0;
                for (int i = 0; i < pop; i++) {
                    Long v = jMap.get(probeKeys[i]);
                    if (v != null) sink ^= v;
                }
            }, 3);

            // 3. ExpanseSet
            double expSetInsertS;
            double expSetLookupS;
            try (ExpanseSet set = new ExpanseSet()) {
                expSetInsertS = measure(() -> {
                    set.clear();
                    for (int i = 0; i < pop; i++) set.add(keys[i]);
                }, 3);

                expSetLookupS = measure(() -> {
                    int count = 0;
                    for (int i = 0; i < pop; i++) if (set.contains(probeKeys[i])) count++;
                }, 3);
            }

            // 4. java.util.HashSet
            Set<Long> jSet = new HashSet<>();
            double jSetInsertS = measure(() -> {
                jSet.clear();
                for (int i = 0; i < pop; i++) jSet.add(keys[i]);
            }, 3);

            double jSetLookupS = measure(() -> {
                int count = 0;
                for (int i = 0; i < pop; i++) if (jSet.contains(probeKeys[i])) count++;
            }, 3);

            double toMops = (pop / 1e6);
            double expInsertMops = toMops / expInsertS;
            double expLookupMops = toMops / expLookupS;
            double expLookupNs = (expLookupS * 1e9) / pop;

            double jInsertMops = toMops / jInsertS;
            double jLookupMops = toMops / jLookupS;
            double jLookupNs = (jLookupS * 1e9) / pop;

            double expSetInsertMops = toMops / expSetInsertS;
            double expSetLookupMops = toMops / expSetLookupS;
            double expSetLookupNs = (expSetLookupS * 1e9) / pop;

            double jSetInsertMops = toMops / jSetInsertS;
            double jSetLookupMops = toMops / jSetLookupS;
            double jSetLookupNs = (jSetLookupS * 1e9) / pop;

            if (json) {
                jsonResults.add(String.format(Locale.US,
                    "{\"dist\": \"%s\", \"pop\": %d, \"expanse_map\": {\"insert_mops\": %.2f, \"lookup_mops\": %.2f, \"lookup_ns\": %.2f, \"bytes_per_key\": %.2f}, \"java_hashmap\": {\"insert_mops\": %.2f, \"lookup_mops\": %.2f, \"lookup_ns\": %.2f, \"bytes_per_key\": 64.0}}",
                    dist, pop, expInsertMops, expLookupMops, expLookupNs, expBytesPerKey, jInsertMops, jLookupMops, jLookupNs));
            } else {
                System.out.printf("\n[ Distribution: %s | Population: %,d ]\n", dist, pop);
                System.out.printf("%-20s | %11s | %13s | %13s | %8s\n", "Target", "Lookup (ns)", "Lookup (Mops)", "Insert (Mops)", "B/key");
                System.out.printf("%s-+-%s-+-%s-+-%s-+-%s\n", "-".repeat(20), "-".repeat(11), "-".repeat(13), "-".repeat(13), "-".repeat(8));
                System.out.printf("%-20s | %11.2f | %13.2f | %13.2f | %8.2f\n", "ExpanseMap (Panama)", expLookupNs, expLookupMops, expInsertMops, expBytesPerKey);
                System.out.printf("%-20s | %11.2f | %13.2f | %13.2f | %8.2f\n", "java.util.HashMap", jLookupNs, jLookupMops, jInsertMops, 64.0);
                System.out.printf("%-20s | %11.2f | %13.2f | %13.2f | %8s\n", "ExpanseSet (Panama)", expSetLookupNs, expSetLookupMops, expSetInsertMops, "—");
                System.out.printf("%-20s | %11.2f | %13.2f | %13.2f | %8s\n", "java.util.HashSet", jSetLookupNs, jSetLookupMops, jSetInsertMops, "—");
            }
        }

        if (json) {
            System.out.println("{\"runtime\": \"java\", \"results\": [" + String.join(",", jsonResults) + "]}");
        } else {
            System.out.println("\n================================================================================\n");
        }
    }

    public static void main(String[] args) {
        int pop = 50_000;
        boolean json = false;
        for (int i = 0; i < args.length; i++) {
            if ("--quick".equals(args[i])) {
                pop = 10_000;
            } else if ("--pop".equals(args[i]) && i + 1 < args.length) {
                pop = Integer.parseInt(args[++i]);
            } else if ("--json".equals(args[i])) {
                json = true;
            }
        }
        runBenchmark(pop, json);
    }
}
