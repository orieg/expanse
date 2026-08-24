# Expanse PHP Bindings (`orieg/expanse`)

High-performance, modernized PHP bindings for **Expanse** with dual-driver runtime negotiation (Native Zend Extension via `ext-php-rs` + Portable `\FFI` fallback driver).

## Installation

### 1. Composer (Packagist)
```bash
composer require orieg/expanse
```

### 2. Native Zend Extension (PIE / PECL)
For maximum throughput and zero opcode overhead:
```bash
pie install orieg/php-expanse
```

## Quickstart

```php
use Expanse\Set;
use Expanse\Map;
use Expanse\StrMap;
use Expanse\BlobMap;

// 1. Dynamic sparse 64-bit integer set (Judy1)
$set = new Set();
$set->add(42);
$set->add(100_000);
echo $set->contains(42) ? "Found\n" : "Missing\n";
echo $set->rank(100_000); // O(depth) rank

// 2. Ordered 64-bit key-value map (JudyL)
$map = new Map();
$map->set(42, 1000);
$map[100] = 5000;
echo $map->get(42); // 1000

// 3. String trie with path folding (JudySL)
$strMap = new StrMap();
$strMap->set("/api/v1/users", 200);

// 4. Large-value blob map with hot metadata
$blobs = new BlobMap();
$blobs->set(1, "small inline payload"); // 0 heap allocations
$blobs->set(2, "large JSON payload", hotMeta: 0x42);

// 5. 1:1 legacy php-judy drop-in compatibility
$judy = new Judy(Judy::INT_TO_INT);
$judy[42] = 999;
echo $judy[42]; // 999
```

## Documentation
For complete API specifications and benchmarks, see [docs/BINDINGS_PHP.md](../../docs/BINDINGS_PHP.md).
