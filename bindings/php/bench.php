<?php

declare(strict_types=1);

/**
 * Cross-Runtime Comparative Benchmark Suite for Expanse PHP Bindings (FFI & Native).
 * Compares Expanse\Map and Expanse\Set against native PHP array (Zend HashTable).
 */

require_once __DIR__ . '/src/Expanse.php';

use Expanse\Map;
use Expanse\Set;

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

function generateKeys(int $pop, string $dist = 'random'): array {
    $keys = [];
    mt_srand(0x0DDB1A5E);
    if ($dist === 'sequential') {
        for ($i = 0; $i < $pop; $i++) {
            $keys[] = $i;
        }
    } elseif ($dist === 'clustered') {
        $base = 0;
        for ($i = 0; $i < $pop; $i++) {
            if ($i % 256 === 0) {
                $base = (mt_rand() & ~0xFF);
            }
            $keys[] = $base + ($i % 256);
        }
    } else {
        for ($i = 0; $i < $pop; $i++) {
            $keys[] = mt_rand(1, 0x7FFFFFFF);
        }
    }
    return $keys;
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
    $probeKeys = $keys;
    shuffle($probeKeys);

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
            'bytes_per_key' => $phpBytesPerKey > 0 ? $phpBytesPerKey : 64.0,
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
        printf("%-20s | %11.2f | %13.2f | %13.2f | %8.2f\n", 'PHP native array', $pa['lookup_ns'], $pa['lookup_mops'], $pa['insert_mops'], $pa['bytes_per_key']);

        $es = $r['expanse_set'];
        printf("%-20s | %11.2f | %13.2f | %13.2f | %8s\n", 'Expanse\\Set', $es['lookup_ns'], $es['lookup_mops'], $es['insert_mops'], '—');

        $ps = $r['php_set'];
        printf("%-20s | %11.2f | %13.2f | %13.2f | %8s\n", 'PHP native set', $ps['lookup_ns'], $ps['lookup_mops'], $ps['insert_mops'], '—');
    }
    echo "\n================================================================================\n\n";
}

function main(): void {
    $args = parseArgs();
    $pop = $args['quick'] ? 10000 : $args['pop'];
    $dists = ['random', 'sequential', 'clustered'];
    $results = [];

    foreach ($dists as $d) {
        $results[] = runSuite($pop, $d);
    }

    if ($args['json']) {
        echo json_encode(['runtime' => 'php', 'results' => $results], JSON_PRETTY_PRINT) . "\n";
    } else {
        renderTable($results);
    }
}

main();
