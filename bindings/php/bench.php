<?php

declare(strict_types=1);

/**
 * Cross-Runtime Comparative Benchmark Suite for Expanse PHP Bindings (FFI & Native).
 * Compares Expanse\Map and Expanse\Set against native PHP array (Zend HashTable).
 */

require_once __DIR__ . '/src/Expanse.php';

use Expanse\Map;
use Expanse\Set;

/**
 * Refuses to run against the pure-PHP array fallback driver (#373).
 *
 * Expanse\Map and Expanse\Set silently degrade to a ksort-per-insert PHP array
 * (with a synthetic count*16 memUsed()) when neither the native extension nor
 * FFI is available. That shim exists for library consumers, but a benchmark
 * that measures it while reporting `runtime: 'php'` is measuring Zend, not
 * Expanse. This mirrors the driver-selection logic in src/Map.php.
 */
function detectDriver(): string {
    if (\Expanse\Driver\NativeDriver::isAvailable() && class_exists(\Expanse\ExpanseMap::class, false)) {
        return 'native';
    }
    if (\Expanse\Driver\FFIDriver::isAvailable()) {
        return 'ffi';
    }
    return 'fallback';
}

function assertRealDriver(): string {
    $driver = detectDriver();
    if ($driver === 'fallback') {
        fwrite(STDERR, "Error: no real Expanse driver is available — refusing to benchmark the pure-PHP array fallback shim.\n");
        fwrite(STDERR, "Build the native C ABI library and enable FFI:\n");
        fwrite(STDERR, "  cargo build --release -p expanse-capi\n");
        fwrite(STDERR, "  php -d ffi.enable=1 bench.php --quick --json\n");
        exit(1);
    }
    return $driver;
}

function parseArgs(): array {
    global $argv;
    $pop = 50000;
    $quick = false;
    $json = false;

    for ($i = 1; $i < count($argv); $i++) {
        if ($argv[$i] === '--quick') {
            $quick = true;
            $pop = 10000;
        } elseif ($argv[$i] === '--pop' && isset($argv[$i + 1])) {
            $pop = (int)$argv[++$i];
        } elseif ($argv[$i] === '--json') {
            $json = true;
        }
    }
    return ['pop' => $pop, 'quick' => $quick, 'json' => $json];
}

/**
 * XorShift64, bit-identical to the node/wasm/ruby/java/dotnet/python/go
 * harnesses (seed 0x0DDB_1A5E_5EED_0001, shifts 13/7/17, logical right shift).
 * PHP ints are signed 64-bit: `<<` discards overflow bits (same wrap as u64),
 * and the arithmetic `>>` is masked back to a logical shift. Values whose top
 * bit is set come back negative, but the 64-bit pattern is identical — the
 * drivers pass them through as uint64_t.
 */
final class XorShift64 {
    private int $state;

    public function __construct(int $seed = 0x0DDB1A5E5EED0001) {
        $this->state = $seed;
    }

    public function next(): int {
        $x = $this->state;
        $x ^= ($x << 13);
        $x ^= (($x >> 7) & 0x01FFFFFFFFFFFFFF); // logical >> 7 (mask off 7 sign-propagated bits)
        $x ^= ($x << 17);
        $this->state = $x;
        return $x;
    }
}

function generateKeys(int $pop, string $dist = 'random'): array {
    $rng = new XorShift64();
    $keys = [];
    if ($dist === 'sequential') {
        for ($i = 0; $i < $pop; $i++) {
            $keys[] = $i;
        }
    } elseif ($dist === 'clustered') {
        $base = 0;
        for ($i = 0; $i < $pop; $i++) {
            if ($i % 256 === 0) {
                $base = ($rng->next() & ~0xFF);
            }
            $keys[] = $base + ($i % 256);
        }
    } else {
        // Full 64-bit key range, like every other harness (pre-#373 this used
        // mt_rand(1, 0x7FFFFFFF): 31-bit keys and a structurally shallower trie).
        for ($i = 0; $i < $pop; $i++) {
            $keys[] = $rng->next();
        }
    }
    return $keys;
}

/** Deterministic seeded Fisher-Yates (the global shuffle() is unseeded). */
function shuffleSeeded(array $arr, int $seed = 0x9E3779B9): array {
    $rng = new XorShift64($seed);
    for ($i = count($arr) - 1; $i > 0; $i--) {
        $j = ($rng->next() & PHP_INT_MAX) % ($i + 1);
        $tmp = $arr[$i];
        $arr[$i] = $arr[$j];
        $arr[$j] = $tmp;
    }
    return $arr;
}

function measure(callable $fn, int $rounds = 3): float {
    $best = INF;
    for ($r = 0; $r < $rounds; $r++) {
        $t0 = hrtime(true);
        $fn();
        $t1 = hrtime(true);
        $dt = ($t1 - $t0) / 1e9;
        if ($dt < $best) {
            $best = $dt;
        }
    }
    return $best;
}

function runSuite(int $pop, string $dist = 'random'): array {
    $keys = generateKeys($pop, $dist);
    $probeKeys = shuffleSeeded($keys);

    // 1. Expanse\Map
    gc_collect_cycles();
    $expMap = new Map();
    $expInsertS = measure(function () use ($expMap, $keys, $pop) {
        $expMap->clear();
        for ($i = 0; $i < $pop; $i++) {
            $expMap->set($keys[$i], $keys[$i] ^ 0x55);
        }
    });

    $expLookupS = measure(function () use ($expMap, $probeKeys, $pop) {
        $sink = 0;
        for ($i = 0; $i < $pop; $i++) {
            $v = $expMap->get($probeKeys[$i]);
            if ($v !== null) $sink ^= $v;
        }
        return $sink;
    });

    $expBytesPerKey = (float)$expMap->memUsed() / $pop;

    // 2. PHP native array
    gc_collect_cycles();
    $memBefore = memory_get_usage();
    $phpArr = [];
    $phpInsertS = measure(function () use (&$phpArr, $keys, $pop) {
        $phpArr = [];
        for ($i = 0; $i < $pop; $i++) {
            $phpArr[$keys[$i]] = $keys[$i] ^ 0x55;
        }
    });

    $phpLookupS = measure(function () use (&$phpArr, $probeKeys, $pop) {
        $sink = 0;
        for ($i = 0; $i < $pop; $i++) {
            if (isset($phpArr[$probeKeys[$i]])) {
                $sink ^= $phpArr[$probeKeys[$i]];
            }
        }
        return $sink;
    });
    $memAfter = memory_get_usage();
    $phpBytesPerKey = max(0.0, (float)($memAfter - $memBefore) / $pop);

    // 3. Expanse\Set
    $expSet = new Set();
    $expSetInsertS = measure(function () use ($expSet, $keys, $pop) {
        $expSet->clear();
        for ($i = 0; $i < $pop; $i++) {
            $expSet->add($keys[$i]);
        }
    });

    $expSetLookupS = measure(function () use ($expSet, $probeKeys, $pop) {
        $count = 0;
        for ($i = 0; $i < $pop; $i++) {
            if ($expSet->contains($probeKeys[$i])) $count++;
        }
        return $count;
    });

    // 4. PHP native Set (array with keys as values)
    $phpSet = [];
    $phpSetInsertS = measure(function () use (&$phpSet, $keys, $pop) {
        $phpSet = [];
        for ($i = 0; $i < $pop; $i++) {
            $phpSet[$keys[$i]] = true;
        }
    });

    $phpSetLookupS = measure(function () use (&$phpSet, $probeKeys, $pop) {
        $count = 0;
        for ($i = 0; $i < $pop; $i++) {
            if (isset($phpSet[$probeKeys[$i]])) $count++;
        }
        return $count;
    });

    $toMops = fn(float $s) => ($pop / $s) / 1e6;
    $toNs = fn(float $s) => ($s * 1e9) / $pop;

    return [
        'dist' => $dist,
        'pop' => $pop,
        'expanse_map' => [
            'insert_mops' => $toMops($expInsertS),
            'lookup_mops' => $toMops($expLookupS),
            'lookup_ns' => $toNs($expLookupS),
            'bytes_per_key' => $expBytesPerKey,
        ],
        'php_array' => [
            'insert_mops' => $toMops($phpInsertS),
            'lookup_mops' => $toMops($phpLookupS),
            'lookup_ns' => $toNs($phpLookupS),
            // memory_get_usage() delta; null (never a fabricated constant) when
            // the delta is non-positive and therefore not a usable measurement.
            'bytes_per_key' => $phpBytesPerKey > 0 ? $phpBytesPerKey : null,
        ],
        'expanse_set' => [
            'insert_mops' => $toMops($expSetInsertS),
            'lookup_mops' => $toMops($expSetLookupS),
            'lookup_ns' => $toNs($expSetLookupS),
        ],
        'php_set' => [
            'insert_mops' => $toMops($phpSetInsertS),
            'lookup_mops' => $toMops($phpSetLookupS),
            'lookup_ns' => $toNs($phpSetLookupS),
        ],
    ];
}

function renderTable(array $results): void {
    echo "\n================================================================================\n";
    echo "  Expanse PHP Bindings Comparative Performance Report\n";
    echo "================================================================================\n";

    foreach ($results as $r) {
        printf("\n[ Distribution: %s | Population: %s ]\n", $r['dist'], number_format($r['pop']));
        printf("%-20s | %11s | %13s | %13s | %8s\n", 'Target', 'Lookup (ns)', 'Lookup (Mops)', 'Insert (Mops)', 'B/key');
        printf("%s-+-%s-+-%s-+-%s-+-%s\n", str_repeat('-', 20), str_repeat('-', 11), str_repeat('-', 13), str_repeat('-', 13), str_repeat('-', 8));

        $em = $r['expanse_map'];
        printf("%-20s | %11.2f | %13.2f | %13.2f | %8.2f\n", 'Expanse\\Map', $em['lookup_ns'], $em['lookup_mops'], $em['insert_mops'], $em['bytes_per_key']);

        $pa = $r['php_array'];
        $paBytes = $pa['bytes_per_key'] !== null ? sprintf('%8.2f', $pa['bytes_per_key']) : sprintf('%8s', 'n/a');
        printf("%-20s | %11.2f | %13.2f | %13.2f | %s\n", 'PHP native array', $pa['lookup_ns'], $pa['lookup_mops'], $pa['insert_mops'], $paBytes);

        $es = $r['expanse_set'];
        printf("%-20s | %11.2f | %13.2f | %13.2f | %8s\n", 'Expanse\\Set', $es['lookup_ns'], $es['lookup_mops'], $es['insert_mops'], '—');

        $ps = $r['php_set'];
        printf("%-20s | %11.2f | %13.2f | %13.2f | %8s\n", 'PHP native set', $ps['lookup_ns'], $ps['lookup_mops'], $ps['insert_mops'], '—');
    }
    echo "\n================================================================================\n\n";
}

function main(): void {
    $driver = assertRealDriver();
    $args = parseArgs();
    $pop = $args['quick'] ? 10000 : $args['pop'];
    $dists = ['random', 'sequential', 'clustered'];
    $results = [];

    foreach ($dists as $d) {
        $results[] = runSuite($pop, $d);
    }

    if ($args['json']) {
        echo json_encode(['runtime' => 'php', 'driver' => $driver, 'results' => $results], JSON_PRETTY_PRINT) . "\n";
    } else {
        fprintf(STDOUT, "Driver: %s\n", $driver);
        renderTable($results);
    }
}

main();
