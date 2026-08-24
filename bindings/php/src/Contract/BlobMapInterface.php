<?php

declare(strict_types=1);

namespace Expanse\Contract;

use ArrayAccess;
use Countable;
use IteratorAggregate;

interface BlobMapInterface extends Countable, ArrayAccess, IteratorAggregate
{
    public function set(int $key, string $payload, int $hotMeta = 0): void;
    public function get(int $key, int &$hotMeta = 0): ?string;
    public function getMeta(int $key): ?int;
    public function delete(int $key): bool;
    public function has(int $key): bool;
    public function count(): int;
    public function clear(): void;
    public function memUsed(): int;
}
