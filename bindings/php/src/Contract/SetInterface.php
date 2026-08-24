<?php

declare(strict_types=1);

namespace Expanse\Contract;

use Countable;
use IteratorAggregate;

interface SetInterface extends Countable, IteratorAggregate
{
    public function add(int $key): bool;
    public function remove(int $key): bool;
    public function contains(int $key): bool;
    public function count(): int;
    public function clear(): void;
    public function first(): ?int;
    public function last(): ?int;
    public function next(int $key): ?int;
    public function prev(int $key): ?int;
    public function rank(int $key): int;
    public function select(int $index): ?int;
    public function countRange(int $start, int $end): int;
    public function memUsed(): int;
}
