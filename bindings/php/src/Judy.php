<?php

declare(strict_types=1);

use Expanse\Set;
use Expanse\Map;
use Expanse\StrMap;

if (!class_exists('Judy')) {
    class Judy implements ArrayAccess, Countable, IteratorAggregate
    {
        public const BITSET = 1;
        public const INT_TO_INT = 2;
        public const INT_TO_MIXED = 3;
        public const STRING_TO_INT = 4;
        public const STRING_TO_MIXED = 5;

        private int $type;
        private Set|Map|StrMap $inner;

        public function __construct(int $judy_type)
        {
            $this->type = $judy_type;
            $this->inner = match ($judy_type) {
                self::BITSET => new Set(),
                self::INT_TO_INT, self::INT_TO_MIXED => new Map(),
                self::STRING_TO_INT, self::STRING_TO_MIXED => new StrMap(),
                default => throw new InvalidArgumentException("Unsupported Judy type: $judy_type"),
            };
        }

        public function getType(): int
        {
            return $this->type;
        }

        public function count(?int $nth = -1, ?int $max = -1): int
        {
            return $this->inner->count();
        }

        public function memoryUsage(): int
        {
            return $this->inner->memUsed();
        }

        public function first(): mixed
        {
            if ($this->inner instanceof Set) {
                return $this->inner->first();
            }
            $p = $this->inner->first();
            return $p !== null ? $p[0] : null;
        }

        public function next(int|string $index): mixed
        {
            if ($this->inner instanceof Set) {
                return $this->inner->next((int) $index);
            }
            if ($this->inner instanceof Map) {
                $p = $this->inner->next((int) $index);
                return $p !== null ? $p[0] : null;
            }
            $p = $this->inner->next((string) $index);
            return $p !== null ? $p[0] : null;
        }

        public function last(): mixed
        {
            if ($this->inner instanceof Set) {
                return $this->inner->last();
            }
            $p = $this->inner->last();
            return $p !== null ? $p[0] : null;
        }

        public function prev(int|string $index): mixed
        {
            if ($this->inner instanceof Set) {
                return $this->inner->prev((int) $index);
            }
            if ($this->inner instanceof Map) {
                $p = $this->inner->prev((int) $index);
                return $p !== null ? $p[0] : null;
            }
            return null;
        }

        public function byCount(int $nth): mixed
        {
            if ($this->inner instanceof Set) {
                return $this->inner->select($nth - 1);
            }
            if ($this->inner instanceof Map) {
                $p = $this->inner->select($nth - 1);
                return $p !== null ? $p[0] : null;
            }
            return null;
        }

        public function free(): int
        {
            $mem = $this->memoryUsage();
            $this->inner->clear();
            return $mem;
        }

        public function offsetExists(mixed $offset): bool
        {
            if ($this->inner instanceof Set) {
                return $this->inner->contains((int) $offset);
            }
            if ($this->inner instanceof Map) {
                return $this->inner->has((int) $offset);
            }
            return $this->inner->has((string) $offset);
        }

        public function offsetGet(mixed $offset): mixed
        {
            if ($this->inner instanceof Set) {
                return $this->inner->contains((int) $offset) ? 1 : null;
            }
            if ($this->inner instanceof Map) {
                return $this->inner->get((int) $offset);
            }
            return $this->inner->get((string) $offset);
        }

        public function offsetSet(mixed $offset, mixed $value): void
        {
            if ($this->inner instanceof Set) {
                if ($value) {
                    $this->inner->add((int) $offset);
                } else {
                    $this->inner->remove((int) $offset);
                }
                return;
            }
            if ($this->inner instanceof Map) {
                $this->inner->set((int) $offset, (int) $value);
                return;
            }
            $this->inner->set((string) $offset, (int) $value);
        }

        public function offsetUnset(mixed $offset): void
        {
            if ($this->inner instanceof Set) {
                $this->inner->remove((int) $offset);
                return;
            }
            if ($this->inner instanceof Map) {
                $this->inner->delete((int) $offset);
                return;
            }
            $this->inner->delete((string) $offset);
        }

        public function getIterator(): Traversable
        {
            return $this->inner->getIterator();
        }
    }
}
