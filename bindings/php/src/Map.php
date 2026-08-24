<?php

declare(strict_types=1);

namespace Expanse;

use Expanse\Contract\MapInterface;
use Expanse\Driver\FFIDriver;
use Expanse\Driver\NativeDriver;
use ArrayIterator;
use FFI;
use Traversable;

if (!class_exists(Map::class)) {
    class Map implements MapInterface
    {
        private ?object $native = null;
        private mixed $handle = null;
        private array $data = [];

        public function __construct()
        {
            if (NativeDriver::isAvailable() && class_exists(\Expanse\ExpanseMap::class, false)) {
                $this->native = new \Expanse\ExpanseMap();
            } elseif (FFIDriver::isAvailable()) {
                $ffi = FFIDriver::getFFI();
                $this->handle = $ffi->expanse_map_new();
            }
        }

        public function __destruct()
        {
            if ($this->handle !== null) {
                try {
                    $ffi = FFIDriver::getFFI();
                    $ffi->expanse_map_free($this->handle);
                } catch (\Throwable) {}
                $this->handle = null;
            }
        }

        public function set(int $key, int $value): void
        {
            if ($this->native !== null) {
                $this->native->set($key, $value);
                return;
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                $ffi->expanse_map_insert($this->handle, $key, $value, null);
                return;
            }
            $this->data[$key] = $value;
            ksort($this->data);
        }

        public function get(int $key): ?int
        {
            if ($this->native !== null) {
                return $this->native->get($key);
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                $out = $ffi->new("uint64_t");
                if ($ffi->expanse_map_get($this->handle, $key, FFI::addr($out))) {
                    return (int) $out->cdata;
                }
                return null;
            }
            return $this->data[$key] ?? null;
        }

        public function delete(int $key): bool
        {
            if ($this->native !== null) {
                return $this->native->delete($key);
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                return (bool) $ffi->expanse_map_remove($this->handle, $key, null);
            }
            $res = isset($this->data[$key]);
            unset($this->data[$key]);
            return $res;
        }

        public function has(int $key): bool
        {
            if ($this->native !== null) {
                return $this->native->has($key);
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                $out = $ffi->new("uint64_t");
                return (bool) $ffi->expanse_map_get($this->handle, $key, FFI::addr($out));
            }
            return isset($this->data[$key]);
        }

        public function count(): int
        {
            if ($this->native !== null) {
                return (int) $this->native->count();
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                return (int) $ffi->expanse_map_len($this->handle);
            }
            return count($this->data);
        }

        public function clear(): void
        {
            if ($this->native !== null) {
                $this->native->clear();
                return;
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                $ffi->expanse_map_clear($this->handle);
                return;
            }
            $this->data = [];
        }

        public function first(): ?array
        {
            if ($this->native !== null) {
                return $this->native->first();
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                $kOut = $ffi->new("uint64_t");
                $vOut = $ffi->new("uint64_t");
                if ($ffi->expanse_map_first($this->handle, FFI::addr($kOut), FFI::addr($vOut))) {
                    return [(int) $kOut->cdata, (int) $vOut->cdata];
                }
                return null;
            }
            $k = array_key_first($this->data);
            return $k !== null ? [(int) $k, $this->data[$k]] : null;
        }

        public function last(): ?array
        {
            if ($this->native !== null) {
                return $this->native->last();
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                $kOut = $ffi->new("uint64_t");
                $vOut = $ffi->new("uint64_t");
                if ($ffi->expanse_map_last($this->handle, FFI::addr($kOut), FFI::addr($vOut))) {
                    return [(int) $kOut->cdata, (int) $vOut->cdata];
                }
                return null;
            }
            $k = array_key_last($this->data);
            return $k !== null ? [(int) $k, $this->data[$k]] : null;
        }

        public function next(int $key): ?array
        {
            if ($this->native !== null) {
                return $this->native->next($key);
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                $kOut = $ffi->new("uint64_t");
                $vOut = $ffi->new("uint64_t");
                if ($ffi->expanse_map_next_after($this->handle, $key, FFI::addr($kOut), FFI::addr($vOut))) {
                    return [(int) $kOut->cdata, (int) $vOut->cdata];
                }
                return null;
            }
            foreach ($this->data as $k => $v) {
                if ($k > $key) return [(int) $k, $v];
            }
            return null;
        }

        public function prev(int $key): ?array
        {
            if ($this->native !== null) {
                return $this->native->prev($key);
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                $kOut = $ffi->new("uint64_t");
                $vOut = $ffi->new("uint64_t");
                if ($ffi->expanse_map_prev_before($this->handle, $key, FFI::addr($kOut), FFI::addr($vOut))) {
                    return [(int) $kOut->cdata, (int) $vOut->cdata];
                }
                return null;
            }
            $last = null;
            foreach ($this->data as $k => $v) {
                if ($k >= $key) return $last;
                $last = [(int) $k, $v];
            }
            return $last;
        }

        public function rank(int $key): int
        {
            if ($this->native !== null) {
                return (int) $this->native->rank($key);
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                return (int) $ffi->expanse_map_count_below($this->handle, $key);
            }
            $c = 0;
            foreach ($this->data as $k => $v) {
                if ($k < $key) $c++;
            }
            return $c;
        }

        public function select(int $index): ?array
        {
            if ($this->native !== null) {
                return $this->native->select($index);
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                $kOut = $ffi->new("uint64_t");
                $vOut = $ffi->new("uint64_t");
                if ($ffi->expanse_map_by_count($this->handle, $index, FFI::addr($kOut), FFI::addr($vOut))) {
                    return [(int) $kOut->cdata, (int) $vOut->cdata];
                }
                return null;
            }
            $i = 0;
            foreach ($this->data as $k => $v) {
                if ($i === $index) return [(int) $k, $v];
                $i++;
            }
            return null;
        }

        public function countRange(int $start, int $end): int
        {
            if ($this->native !== null) {
                return (int) $this->native->countRange($start, $end);
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                return (int) $ffi->expanse_map_count_range($this->handle, $start, $end);
            }
            $c = 0;
            foreach ($this->data as $k => $v) {
                if ($k >= $start && $k <= $end) $c++;
            }
            return $c;
        }

        public function memUsed(): int
        {
            if ($this->native !== null) {
                return (int) $this->native->memUsed();
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                return (int) $ffi->expanse_map_mem_used($this->handle);
            }
            return count($this->data) * 16;
        }

        public function getIterator(): Traversable
        {
            $pairs = [];
            $p = $this->first();
            while ($p !== null) {
                $pairs[$p[0]] = $p[1];
                $p = $this->next($p[0]);
            }
            return new ArrayIterator($pairs);
        }

        public function offsetExists(mixed $offset): bool { return $this->has((int) $offset); }
        public function offsetGet(mixed $offset): mixed { return $this->get((int) $offset); }
        public function offsetSet(mixed $offset, mixed $value): void { $this->set((int) $offset, (int) $value); }
        public function offsetUnset(mixed $offset): void { $this->delete((int) $offset); }
    }
}
