<?php

declare(strict_types=1);

require_once __DIR__ . '/../src/Expanse.php';

use Expanse\Set;
use Expanse\Map;
use Expanse\StrMap;
use Expanse\BytesMap;
use Expanse\BlobMap;
use Expanse\SyncMap;
use Expanse\SyncSet;

if (!class_exists('PHPUnit\Framework\TestCase')) {
    abstract class TestCaseShim {
        public function assertTrue($condition, $msg = '') {
            if (!$condition) throw new \Exception("Expected true, got false. $msg");
        }
        public function assertFalse($condition, $msg = '') {
            if ($condition) throw new \Exception("Expected false, got true. $msg");
        }
        public function assertEquals($expected, $actual, $msg = '') {
            if ($expected !== $actual) {
                throw new \Exception("Expected " . var_export($expected, true) . ", got " . var_export($actual, true) . ". $msg");
            }
        }
    }
    class_alias('TestCaseShim', 'PHPUnit\Framework\TestCase');
}

class ExpanseTest extends PHPUnit\Framework\TestCase
{
    public function testSet()
    {
        $set = new Set();
        $this->assertTrue($set->add(42));
        $this->assertFalse($set->add(42));
        $this->assertTrue($set->contains(42));
        $this->assertEquals(1, count($set));

        $set->add(100);
        $this->assertEquals(42, $set->first());
        $this->assertEquals(100, $set->next(42));
        $this->assertEquals(100, $set->last());
        $this->assertEquals(42, $set->prev(100));

        $this->assertEquals(1, $set->rank(100));
        $this->assertEquals(42, $set->select(0));

        $this->assertEquals(2, $set->countRange(40, 110));

        $this->assertTrue($set->remove(42));
        $this->assertFalse($set->contains(42));

        $set->clear();
        $this->assertEquals(0, count($set));

        $s1 = new Set(); $s1->add(1); $s1->add(2);
        $s2 = new Set(); $s2->add(2); $s2->add(3);
        $this->assertEquals(3, count($s1->union($s2)));
        $this->assertEquals(1, count($s1->intersect($s2)));
        $this->assertEquals(1, count($s1->diff($s2)));
    }

    public function testMap()
    {
        $map = new Map();
        $map->set(42, 100);
        $this->assertTrue($map->has(42));
        $this->assertEquals(100, $map->get(42));
        $this->assertEquals([42, 100], $map->first());
        $map->set(50, 200);
        $this->assertEquals([50, 200], $map->next(42));
        $this->assertEquals([50, 200], $map->last());
        $this->assertEquals([42, 100], $map->prev(50));
        $this->assertEquals(1, $map->rank(50));
        $this->assertEquals([42, 100], $map->select(0));
        $this->assertEquals(2, $map->countRange(40, 60));
        $this->assertTrue($map->delete(42));
        $this->assertFalse($map->has(42));
        $map->clear();
        $this->assertEquals(0, count($map));
    }

    /**
     * memHeld()/shrinkToFit() hold the same contract on every driver:
     * shrinkToFit() releases exactly memHeld() - memUsed(), after which the
     * two agree and a second call releases nothing. (The pure-PHP fallback
     * holds nothing beyond memUsed(), so it releases 0.)
     */
    public function testMemHeldAndShrinkToFit()
    {
        $n = 20000;
        $map = new Map();
        $set = new Set();
        $str = new StrMap();
        for ($k = 0; $k < $n; $k++) {
            $map->set($k * 7, $k);
            $set->add($k * 7);
            $str->set(sprintf('key/%08d', $k), $k);
        }
        for ($k = 0; $k < $n; $k++) {
            $this->assertTrue($map->delete($k * 7));
            $this->assertTrue($set->remove($k * 7));
            $this->assertTrue($str->delete(sprintf('key/%08d', $k)));
        }
        foreach ([$map, $set, $str] as $c) {
            $held = $c->memHeld();
            $used = $c->memUsed();
            $this->assertTrue($held >= $used, "memHeld $held < memUsed $used");
            $this->assertEquals($held - $used, $c->shrinkToFit());
            $this->assertEquals($c->memUsed(), $c->memHeld());
            $this->assertEquals(0, $c->shrinkToFit());
        }
    }

    /**
     * The three StrMap backends -- native extension, \FFI, and the pure-PHP
     * array -- must agree about a key carrying an embedded NUL. They did not:
     * the \FFI driver passes the key as a char*, which the C ABI reads with
     * CStr::from_ptr and truncates at the first NUL, while the other two kept
     * it whole. The same PHP source stored a different key depending on which
     * backend happened to be available on the host.
     *
     * The contract is now "rejected everywhere", which is checkable from PHP
     * without knowing which backend is active -- and that is the point, since
     * `docs/bindings/php.md` promises identical contracts across drivers.
     */
    public function testStrMapRejectsEmbeddedNulOnEveryDriver()
    {
        $m = new StrMap();
        foreach (["abc\0def", "\0", "a\0"] as $bad) {
            $threw = false;
            try {
                $m->set($bad, 1);
            } catch (\InvalidArgumentException $e) {
                $threw = true;
            }
            $this->assertTrue($threw, 'set() must reject a key containing NUL');

            $threw = false;
            try {
                $m->get($bad);
            } catch (\InvalidArgumentException $e) {
                $threw = true;
            }
            $this->assertTrue($threw, 'get() must reject a key containing NUL');

            $threw = false;
            try {
                $m->has($bad);
            } catch (\InvalidArgumentException $e) {
                $threw = true;
            }
            $this->assertTrue($threw, 'has() must reject a key containing NUL');
        }

        // A NUL-free key that shares the truncation prefix must still work, so
        // the guard is rejecting the NUL and not the prefix.
        $m->set("abc", 7);
        $this->assertEquals(7, $m->get("abc"));
    }

    public function testStrMap()
    {
        $map = new StrMap();
        $map->set("foo", 100);
        $this->assertTrue($map->has("foo"));
        $this->assertEquals(100, $map->get("foo"));
        $this->assertEquals(1, count($map));
        $map->set("bar", 200);
        $this->assertTrue($map->delete("foo"));
        $this->assertFalse($map->has("foo"));
        $map->clear();
        $this->assertEquals(0, count($map));
    }

    public function testBytesMap()
    {
        $map = new BytesMap();
        $key = "foo\x00bar"; // binary key with embedded NUL
        $map->set($key, 42);
        $this->assertTrue($map->has($key));
        $this->assertEquals(42, $map->get($key));
        $this->assertTrue($map->delete($key));
        $this->assertFalse($map->has($key));
    }

    public function testBlobMap()
    {
        $map = new BlobMap();
        $map->set(42, "inline");
        $meta = 0;
        $this->assertEquals("inline", $map->get(42, $meta));

        // Arena allocation with hot metadata (> 7 bytes)
        $map->set(100, "large payload with hot metadata", 42);
        $this->assertTrue($map->has(100));
        $metaLarge = 0;
        $this->assertEquals("large payload with hot metadata", $map->get(100, $metaLarge));
        $this->assertEquals(42, $metaLarge);
        $this->assertEquals(42, $map->getMeta(100));
        $this->assertEquals(2, count($map));

        $this->assertTrue($map->delete(100));
        $this->assertFalse($map->has(100));
        $this->assertEquals(1, count($map));
    }

    /**
     * A refused blob insert must throw, not read as success, and must leave
     * the key's previous value in place. hot_meta above 24 bits on a payload
     * longer than 7 bytes is refused by the engine (MetaOverflow) before the
     * index is touched; every driver throws \Exception for it.
     */
    public function testBlobMapCapSwitchAndRefusalStatus()
    {
        // #1300. One 4 KiB chunk fits a 6000-byte cap, so after removals the
        // insert compacts and the 3100-byte record still does not fit.
        $probe = new BlobMap();
        $driver = (new \ReflectionProperty($probe, 'native'))->getValue($probe) !== null
            || (new \ReflectionProperty($probe, 'handle'))->getValue($probe) !== null;
        if (!$driver) {
            $threw = false;
            try {
                new BlobMap(4096, 6000);
            } catch (\Exception $e) {
                $threw = true;
            }
            $this->assertTrue($threw, 'the fallback must refuse a capacity it cannot honour');
            return;
        }
        $this->assertEquals(1 << 30, $probe->arenaStats()['max_capacity']);

        $map = new BlobMap(4096, 6000);
        $this->assertEquals(6000, $map->arenaStats()['max_capacity']);
        for ($k = 0; $k < 4; $k++) {
            $this->assertEquals('ok', $map->setStatus($k, str_repeat(chr(65 + $k), 1000), 1));
        }
        $big = str_repeat('z', 3100);
        $this->assertEquals('cap_refused', $map->setStatus(9, $big, 1));
        $st = $map->arenaStats();
        $this->assertEquals(4 * 1008, $st['live_bytes']);
        $this->assertEquals(4096, $st['allocated_bytes']);
        for ($k = 1; $k < 4; $k++) {
            $this->assertTrue($map->delete($k));
        }
        $this->assertEquals('arena_full', $map->setStatus(9, $big, 1));
        $this->assertEquals(1, count($map));
        $this->assertEquals('meta_overflow', $map->setStatus(1, str_repeat('m', 100), 1 << 24));
        $map->setReclaimAtCap(false);
        $this->assertEquals(0, $map->arenaStats()['reclaim_at_cap']);
    }

    public function testBlobMapRefusedInsertThrows()
    {
        $map = new BlobMap();
        $payload = "sixteen byte val";

        $threw = false;
        try {
            $map->set(1, $payload, 1 << 24);
        } catch (\Exception $e) {
            $threw = true;
        }
        $this->assertTrue($threw, 'set() with hot_meta 1<<24 must throw');
        $this->assertFalse($map->has(1), 'a refused insert must not store the key');
        $this->assertEquals(0, count($map));

        $map->set(2, $payload, 0xFFFFFF);
        $threw = false;
        try {
            $map->set(2, "replacement value", 1 << 24);
        } catch (\Exception $e) {
            $threw = true;
        }
        $this->assertTrue($threw, 'overwrite with hot_meta 1<<24 must throw');
        $meta = 0;
        $this->assertEquals($payload, $map->get(2, $meta));
        $this->assertEquals(0xFFFFFF, $meta);
    }

    /**
     * hot_meta is a uint32_t in the engine. A negative value, or one wider
     * than 32 bits, must be refused the same way on every driver: the \FFI
     * driver wrapped -1 to 0xFFFFFFFF and truncated 2^32 + 1 to 1, while the
     * in-memory fallback stored both as given. The engine also ignores
     * hot_meta for a payload of at most 7 bytes and reads it back as 0.
     */
    public function testBlobMapHotMetaRangeIsIdenticalOnEveryDriver()
    {
        $map = new BlobMap();
        foreach (["short", "sixteen byte val"] as $payload) {
            foreach ([-1, -(1 << 40), 0x100000001] as $bad) {
                $threw = false;
                try {
                    $map->set(1, $payload, $bad);
                } catch (\Exception $e) {
                    $threw = true;
                }
                $this->assertTrue($threw, "set() with hot_meta $bad must throw (payload " . strlen($payload) . " bytes)");
                $this->assertFalse($map->has(1), "a refused hot_meta $bad must not store the key");
            }
        }

        // The whole uint32 range is accepted for an inline payload, whose
        // metadata the engine does not keep.
        $map->set(3, "short", 0xFFFFFFFF);
        $meta = -1;
        $this->assertEquals("short", $map->get(3, $meta));
        $this->assertEquals(0, $meta, 'an inline payload reads back hot_meta 0');
        $this->assertEquals(0, $map->getMeta(3));
    }

    public function testSyncMapAndSet()
    {
        $set = new SyncSet();
        $this->assertTrue($set->add(42));
        $this->assertTrue($set->contains(42));
        $this->assertTrue($set->remove(42));

        $map = new SyncMap();
        $map->set(42, 100);
        $this->assertEquals(100, $map->get(42));
        $this->assertTrue($map->delete(42));
    }

    /**
     * After a half drain, shrinkToFit() on a concurrent container releases
     * the collector's freed blocks and memHeld() falls by exactly that much;
     * blocks still in their grace period stay held, so memHeld() is not
     * asserted to reach any used figure. (The pure-PHP fallback holds and
     * releases 0.)
     */
    public function testSyncMemHeldAndShrinkToFit()
    {
        $n = 50000;
        $map = new SyncMap();
        $set = new SyncSet();
        for ($k = 0; $k < $n; $k++) {
            $map->set($k * 0x9E3779B1, $k);
            $set->add($k * 0x9E3779B1);
        }
        for ($k = 0; $k < $n; $k += 2) {
            $this->assertTrue($map->delete($k * 0x9E3779B1));
            $this->assertTrue($set->remove($k * 0x9E3779B1));
        }
        foreach ([$map, $set] as $c) {
            $held = $c->memHeld();
            $this->assertTrue($held >= 0);
            $released = $c->shrinkToFit();
            $this->assertTrue($released >= 0);
            $this->assertEquals($held - $released, $c->memHeld());
        }
        for ($k = 1; $k < $n; $k += 2) {
            $this->assertEquals($k, $map->get($k * 0x9E3779B1));
            $this->assertTrue($set->contains($k * 0x9E3779B1));
        }
    }

    public function testJudyCompat()
    {
        $j1 = new Judy(Judy::BITSET);
        $this->assertEquals(Judy::BITSET, $j1->getType());
        $j1[42] = true;
        $this->assertTrue(isset($j1[42]));
        $this->assertEquals(1, count($j1));
        $this->assertEquals(42, $j1->first());
        $j1[100] = true;
        $this->assertEquals(100, $j1->next(42));
        $this->assertEquals(100, $j1->last());
        $this->assertEquals(42, $j1->prev(100));
        $this->assertEquals(42, $j1->byCount(1));
        unset($j1[42]);
        $this->assertFalse(isset($j1[42]));
        $j1->free();
        $this->assertEquals(0, count($j1));

        $jL = new Judy(Judy::INT_TO_INT);
        $this->assertEquals(Judy::INT_TO_INT, $jL->getType());
        $jL[42] = 999;
        $this->assertEquals(999, $jL[42]);
        $this->assertEquals(1, count($jL));
        $jL[100] = 888;
        $this->assertEquals(42, $jL->first());
        $this->assertEquals(100, $jL->next(42));
        $this->assertEquals(100, $jL->last());
        $this->assertEquals(42, $jL->prev(100));
        $this->assertEquals(42, $jL->byCount(1));
        unset($jL[42]);
        $this->assertFalse(isset($jL[42]));
    }
}

if (php_sapi_name() === 'cli' && basename(__FILE__) === basename($_SERVER['SCRIPT_FILENAME'] ?? '')) {
    // The driver the wrappers selected on this host: 'native', 'ffi' or
    // 'fallback'. A driver that fails to load degrades silently to the next
    // one (a missing extension is only a startup warning), so a CI run meant
    // for one driver states it in EXPANSE_PHP_DRIVER and the runner refuses
    // to report a pass for another.
    $probe = new BlobMap();
    $read = static fn(string $prop) => (new \ReflectionProperty($probe, $prop))->getValue($probe);
    $driver = $read('native') !== null ? 'native' : ($read('handle') !== null ? 'ffi' : 'fallback');
    unset($probe, $read);
    echo "Driver: $driver\n";
    $expected = getenv('EXPANSE_PHP_DRIVER');
    if ($expected !== false && $expected !== '' && $expected !== $driver) {
        fwrite(STDERR, "FAIL: EXPANSE_PHP_DRIVER=$expected but the active driver is $driver\n");
        exit(1);
    }
    $test = new ExpanseTest();
    $methods = get_class_methods($test);
    $count = 0;
    foreach ($methods as $m) {
        if (str_starts_with($m, 'test')) {
            $test->$m();
            $count++;
            echo "PASS: $m\n";
        }
    }
    echo "OK ($count tests passed)\n";
}
