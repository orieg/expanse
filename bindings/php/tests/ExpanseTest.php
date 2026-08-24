<?php

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

class ExpanseTest extends PHPUnit\Framework\TestCase {
    public function testSet() {
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
    
    public function testMap() {
        $map = new Map();
        $map->set(42, 100);
        $this->assertTrue($map->has(42));
        $this->assertEquals(100, $map->get(42));
        $this->assertEquals([42, 100], $map->first());
        $map->set(50, 200);
        $this->assertEquals([50, 200], $map->next(42));
        $this->assertTrue($map->delete(42));
        $this->assertFalse($map->has(42));
        $map->clear();
        $this->assertEquals(0, count($map));
    }
    
    public function testStrMap() {
        $map = new StrMap();
        $map->set("foo", 100);
        $this->assertTrue($map->has("foo"));
        $this->assertEquals(100, $map->get("foo"));
        $this->assertEquals(["foo", 100], $map->first());
        $map->set("bar", 200); // bar is before foo lexicographically
        $this->assertEquals(["foo", 100], $map->next("bar"));
        $this->assertTrue($map->delete("foo"));
        $this->assertFalse($map->has("foo"));
        $map->clear();
        $this->assertEquals(0, count($map));
    }
    
    public function testBytesMap() {
        $map = new BytesMap();
        $key = "foo\x00bar"; // binary key with NUL
        $map->set($key, 42);
        $this->assertTrue($map->has($key));
        $this->assertEquals(42, $map->get($key));
        $this->assertTrue($map->delete($key));
        $this->assertFalse($map->has($key));
    }
    
    public function testBlobMap() {
        $map = new BlobMap();
        $map->set(42, "payload", 1);
        $meta = 0;
        $this->assertEquals("payload", $map->get(42, $meta));
        $this->assertEquals(1, $meta);
        
        $map->set(100, "data", 2);
        $pruned = $map->prune(function($k, $m) { return $m === 2; });
        $this->assertEquals(1, $pruned);
        $this->assertFalse($map->has(100));
        
        $path = tempnam(sys_get_temp_dir(), 'blob');
        $map->saveImage($path);
        $map2 = BlobMap::openImage($path);
        $this->assertTrue($map2->has(42));
        unlink($path);
    }
    
    public function testSyncMapAndSet() {
        $set = new SyncSet();
        $this->assertTrue($set->add(42));
        $this->assertTrue($set->contains(42));
        
        $map = new SyncMap();
        $map->set(42, 100);
        $this->assertEquals(100, $map->get(42));
    }
}

if (php_sapi_name() === 'cli' && basename(__FILE__) === basename($_SERVER['SCRIPT_FILENAME'] ?? '')) {
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
