# Expanse PHP Bindings

Native PHP bindings for Expanse via PIE / C ABI / FFI.

## Installation

```bash
composer require orieg/expanse
```

## Usage

```php
use Expanse\Set;
use Expanse\Map;
use Expanse\StrMap;
use Expanse\BytesMap;
use Expanse\BlobMap;

$set = new Set();
$set->add(42);
echo $set->contains(42) ? 'Yes' : 'No';

$map = new Map();
$map->set(42, 100);
echo $map->get(42);
```

## Classes

- `Expanse\Set`
- `Expanse\Map`
- `Expanse\StrMap`
- `Expanse\BytesMap`
- `Expanse\BlobMap`
- `Expanse\SyncMap`
- `Expanse\SyncSet`
