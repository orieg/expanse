<?php

namespace Expanse;

use FFI;

class Set implements \Countable, \IteratorAggregate {
    private array $data = [];
    public function add(int $key): bool { $res = !isset($this->data[$key]); $this->data[$key] = true; ksort($this->data); return $res; }
    public function remove(int $key): bool { $res = isset($this->data[$key]); unset($this->data[$key]); return $res; }
    public function contains(int $key): bool { return isset($this->data[$key]); }
    public function count(): int { return count($this->data); }
    public function clear(): void { $this->data = []; }
    public function first(): ?int { $k = array_key_first($this->data); return $k; }
    public function next(int $key): ?int { 
        foreach ($this->data as $k => $v) { if ($k > $key) return $k; }
        return null;
    }
    public function last(): ?int { $k = array_key_last($this->data); return $k; }
    public function prev(int $key): ?int { 
        $last = null;
        foreach ($this->data as $k => $v) { if ($k >= $key) return $last; $last = $k; }
        return $last;
    }
    public function rank(int $key): int { 
        $c = 0; foreach ($this->data as $k => $v) { if ($k < $key) $c++; } return $c;
    }
    public function select(int $k): ?int { 
        $i = 0; foreach ($this->data as $key => $v) { if ($i === $k) return $key; $i++; } return null;
    }
    public function countRange(int $start, int $end): int { 
        $c = 0; foreach ($this->data as $k => $v) { if ($k >= $start && $k <= $end) $c++; } return $c;
    }
    public function union(Set $other): Set { $s = new Set(); $s->data = $this->data + $other->data; return $s; }
    public function intersect(Set $other): Set { $s = new Set(); $s->data = array_intersect_key($this->data, $other->data); return $s; }
    public function diff(Set $other): Set { $s = new Set(); $s->data = array_diff_key($this->data, $other->data); return $s; }
    public function getIterator(): \Traversable { return new \ArrayIterator(array_keys($this->data)); }
}

class Map implements \Countable, \ArrayAccess, \IteratorAggregate {
    private array $data = [];
    public function set(int $key, int $value): void { $this->data[$key] = $value; ksort($this->data); }
    public function get(int $key): ?int { return $this->data[$key] ?? null; }
    public function delete(int $key): bool { $res = isset($this->data[$key]); unset($this->data[$key]); return $res; }
    public function has(int $key): bool { return isset($this->data[$key]); }
    public function count(): int { return count($this->data); }
    public function clear(): void { $this->data = []; }
    public function first(): ?array { $k = array_key_first($this->data); return $k !== null ? [$k, $this->data[$k]] : null; }
    public function next(int $key): ?array { 
        foreach ($this->data as $k => $v) { if ($k > $key) return [$k, $v]; }
        return null;
    }
    public function getIterator(): \Traversable { return new \ArrayIterator($this->data); }
    
    public function offsetExists(mixed $offset): bool { return $this->has($offset); }
    public function offsetGet(mixed $offset): mixed { return $this->get($offset); }
    public function offsetSet(mixed $offset, mixed $value): void { $this->set($offset, $value); }
    public function offsetUnset(mixed $offset): void { $this->delete($offset); }
}

class StrMap implements \Countable, \ArrayAccess, \IteratorAggregate {
    private array $data = [];
    public function set(string $key, int $value): void { $this->data[$key] = $value; ksort($this->data); }
    public function get(string $key): ?int { return $this->data[$key] ?? null; }
    public function delete(string $key): bool { $res = isset($this->data[$key]); unset($this->data[$key]); return $res; }
    public function has(string $key): bool { return isset($this->data[$key]); }
    public function count(): int { return count($this->data); }
    public function clear(): void { $this->data = []; }
    public function first(): ?array { $k = array_key_first($this->data); return $k !== null ? [$k, $this->data[$k]] : null; }
    public function next(string $key): ?array { 
        foreach ($this->data as $k => $v) { if (strcmp($k, $key) > 0) return [$k, $v]; }
        return null;
    }
    public function getIterator(): \Traversable { return new \ArrayIterator($this->data); }
    
    public function offsetExists(mixed $offset): bool { return $this->has($offset); }
    public function offsetGet(mixed $offset): mixed { return $this->get($offset); }
    public function offsetSet(mixed $offset, mixed $value): void { $this->set($offset, $value); }
    public function offsetUnset(mixed $offset): void { $this->delete($offset); }
}

class BytesMap implements \Countable, \ArrayAccess, \IteratorAggregate {
    private array $data = [];
    public function set(string $binaryKey, int $value): void { $this->data[$binaryKey] = $value; }
    public function get(string $binaryKey): ?int { return $this->data[$binaryKey] ?? null; }
    public function delete(string $binaryKey): bool { $res = isset($this->data[$binaryKey]); unset($this->data[$binaryKey]); return $res; }
    public function has(string $binaryKey): bool { return isset($this->data[$binaryKey]); }
    public function count(): int { return count($this->data); }
    public function clear(): void { $this->data = []; }
    public function getIterator(): \Traversable { return new \ArrayIterator($this->data); }
    
    public function offsetExists(mixed $offset): bool { return $this->has($offset); }
    public function offsetGet(mixed $offset): mixed { return $this->get($offset); }
    public function offsetSet(mixed $offset, mixed $value): void { $this->set($offset, $value); }
    public function offsetUnset(mixed $offset): void { $this->delete($offset); }
}

class BlobMap implements \Countable, \ArrayAccess, \IteratorAggregate {
    private array $data = [];
    private array $meta = [];
    public function set(int $key, string $payload, int $hotMeta = 0): void { $this->data[$key] = $payload; $this->meta[$key] = $hotMeta; }
    public function get(int $key, int &$hotMeta = 0): ?string { if(isset($this->data[$key])) { $hotMeta = $this->meta[$key]; return $this->data[$key]; } return null; }
    public function delete(int $key): bool { $res = isset($this->data[$key]); unset($this->data[$key]); unset($this->meta[$key]); return $res; }
    public function has(int $key): bool { return isset($this->data[$key]); }
    public function count(): int { return count($this->data); }
    public function clear(): void { $this->data = []; $this->meta = []; }
    public function prune(callable $predicate): int { 
        $c = 0;
        foreach ($this->data as $k => $v) {
            if ($predicate($k, $this->meta[$k])) {
                unset($this->data[$k]); unset($this->meta[$k]); $c++;
            }
        }
        return $c;
    }
    public function saveImage(string $path): void { file_put_contents($path, serialize([$this->data, $this->meta])); }
    public static function openImage(string $path, bool $mmap = true): self { 
        $b = new self(); 
        list($b->data, $b->meta) = unserialize(file_get_contents($path));
        return $b;
    }
    public function getIterator(): \Traversable { return new \ArrayIterator($this->data); }
    
    public function offsetExists(mixed $offset): bool { return $this->has($offset); }
    public function offsetGet(mixed $offset): mixed { return $this->get($offset); }
    public function offsetSet(mixed $offset, mixed $value): void { $this->set($offset, $value); }
    public function offsetUnset(mixed $offset): void { $this->delete($offset); }
}

class SyncSet {
    private array $data = [];
    public function add(int $key): bool { $res = !isset($this->data[$key]); $this->data[$key] = true; return $res; }
    public function remove(int $key): bool { $res = isset($this->data[$key]); unset($this->data[$key]); return $res; }
    public function contains(int $key): bool { return isset($this->data[$key]); }
}

class SyncMap {
    private array $data = [];
    public function set(int $key, int $value): void { $this->data[$key] = $value; }
    public function get(int $key): ?int { return $this->data[$key] ?? null; }
    public function delete(int $key): bool { $res = isset($this->data[$key]); unset($this->data[$key]); return $res; }
}
