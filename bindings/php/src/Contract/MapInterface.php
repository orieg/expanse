<?php

declare(strict_types=1);

namespace Expanse\Contract;

use ArrayAccess;
use Countable;
use IteratorAggregate;

interface MapInterface extends Countable, ArrayAccess, IteratorAggregate
{
    public function set(int $key, int $value): void;
    public function get(int $key): ?int;
    public function delete(int $key): bool;
    public function has(int $key): bool;
    public function count(): int;
    public function clear(): void;
    public function first(): ?array;
    public function last(): ?array;
    public function next(int $key): ?array;
    public function prev(int $key): ?array;
    public function rank(int $key): int;
    public function select(int $index): ?array;
    public function countRange(int $start, int $end): int;
    public function memUsed(): int;
}
