<?php

declare(strict_types=1);

namespace Expanse\Contract;

use ArrayAccess;
use Countable;
use IteratorAggregate;

interface StrMapInterface extends Countable, ArrayAccess, IteratorAggregate
{
    public function set(string $key, int $value): void;
    public function get(string $key): ?int;
    public function delete(string $key): bool;
    public function has(string $key): bool;
    public function count(): int;
    public function clear(): void;
    public function memUsed(): int;
}
