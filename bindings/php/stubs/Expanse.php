<?php

namespace Expanse;

class Set implements \Countable, \IteratorAggregate {
    public function add(int $key): bool {}
    public function remove(int $key): bool {}
    public function contains(int $key): bool {}
    public function count(): int {}
    public function clear(): void {}
    public function first(): ?int {}
    public function next(int $key): ?int {}
    public function last(): ?int {}
    public function prev(int $key): ?int {}
    public function rank(int $key): int {}
    public function select(int $k): ?int {}
    public function countRange(int $start, int $end): int {}
    public function union(Set $other): Set {}
    public function intersect(Set $other): Set {}
    public function diff(Set $other): Set {}
    public function getIterator(): \Traversable {}
}

class Map implements \Countable, \ArrayAccess, \IteratorAggregate {
    public function set(int $key, int $value): void {}
    public function get(int $key): ?int {}
    public function delete(int $key): bool {}
    public function has(int $key): bool {}
    public function count(): int {}
    public function clear(): void {}
    public function first(): ?array {}
    public function next(int $key): ?array {}
    public function getIterator(): \Traversable {}
    public function offsetExists(mixed $offset): bool {}
    public function offsetGet(mixed $offset): mixed {}
    public function offsetSet(mixed $offset, mixed $value): void {}
    public function offsetUnset(mixed $offset): void {}
}

class StrMap implements \Countable, \ArrayAccess, \IteratorAggregate {
    public function set(string $key, int $value): void {}
    public function get(string $key): ?int {}
    public function delete(string $key): bool {}
    public function has(string $key): bool {}
    public function count(): int {}
    public function clear(): void {}
    public function first(): ?array {}
    public function next(string $key): ?array {}
    public function getIterator(): \Traversable {}
    public function offsetExists(mixed $offset): bool {}
    public function offsetGet(mixed $offset): mixed {}
    public function offsetSet(mixed $offset, mixed $value): void {}
    public function offsetUnset(mixed $offset): void {}
}

class BytesMap implements \Countable, \ArrayAccess, \IteratorAggregate {
    public function set(string $binaryKey, int $value): void {}
    public function get(string $binaryKey): ?int {}
    public function delete(string $binaryKey): bool {}
    public function has(string $binaryKey): bool {}
    public function count(): int {}
    public function clear(): void {}
    public function getIterator(): \Traversable {}
    public function offsetExists(mixed $offset): bool {}
    public function offsetGet(mixed $offset): mixed {}
    public function offsetSet(mixed $offset, mixed $value): void {}
    public function offsetUnset(mixed $offset): void {}
}

class BlobMap implements \Countable, \ArrayAccess, \IteratorAggregate {
    public function set(int $key, string $payload, int $hotMeta = 0): void {}
    public function get(int $key, int &$hotMeta = 0): ?string {}
    public function delete(int $key): bool {}
    public function has(int $key): bool {}
    public function count(): int {}
    public function clear(): void {}
    public function prune(callable $predicate): int {}
    public function saveImage(string $path): void {}
    public static function openImage(string $path, bool $mmap = true): self {}
    public function getIterator(): \Traversable {}
    public function offsetExists(mixed $offset): bool {}
    public function offsetGet(mixed $offset): mixed {}
    public function offsetSet(mixed $offset, mixed $value): void {}
    public function offsetUnset(mixed $offset): void {}
}

class SyncMap {
    public function set(int $key, int $value): void {}
    public function get(int $key): ?int {}
    public function delete(int $key): bool {}
}

class SyncSet {
    public function add(int $key): bool {}
    public function remove(int $key): bool {}
    public function contains(int $key): bool {}
}
