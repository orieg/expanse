<?php

declare(strict_types=1);

namespace Expanse;

use Expanse\Contract\SetInterface;
use Expanse\Driver\FFIDriver;
use Expanse\Driver\NativeDriver;
use ArrayIterator;
use FFI;
use Traversable;

if (!class_exists(Set::class)) {
    class Set implements SetInterface
    {
        private ?object $native = null;
        private mixed $handle = null;
        private array $data = [];

        public function __construct()
        {
            if (NativeDriver::isAvailable() && class_exists(\Expanse\ExpanseSet::class, false)) {
                $this->native = new \Expanse\ExpanseSet();
            } elseif (FFIDriver::isAvailable()) {
                $ffi = FFIDriver::getFFI();
                $this->handle = $ffi->expanse_set_new();
            }
        }

        public function __destruct()
        {
            if ($this->handle !== null) {
                try {
                    $ffi = FFIDriver::getFFI();
                    $ffi->expanse_set_free($this->handle);
                } catch (\Throwable) {}
                $this->handle = null;
            }
        }

        public function add(int $key): bool
        {
            if ($this->native !== null) {
                return $this->native->add($key);
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                return (bool) $ffi->expanse_set_insert($this->handle, $key);
            }
            $res = !isset($this->data[$key]);
            $this->data[$key] = true;
            ksort($this->data);
            return $res;
        }

        public function remove(int $key): bool
        {
            if ($this->native !== null) {
                return $this->native->remove($key);
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                return (bool) $ffi->expanse_set_remove($this->handle, $key);
            }
            $res = isset($this->data[$key]);
            unset($this->data[$key]);
            return $res;
        }

        public function contains(int $key): bool
        {
            if ($this->native !== null) {
                return $this->native->contains($key);
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                return (bool) $ffi->expanse_set_contains($this->handle, $key);
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
                return (int) $ffi->expanse_set_len($this->handle);
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
                $ffi->expanse_set_clear($this->handle);
                return;
            }
            $this->data = [];
        }

        public function first(): ?int
        {
            if ($this->native !== null) {
                return $this->native->first();
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                $out = $ffi->new("uint64_t");
                if ($ffi->expanse_set_first($this->handle, FFI::addr($out))) {
                    return (int) $out->cdata;
                }
                return null;
            }
            $k = array_key_first($this->data);
            return $k !== null ? (int) $k : null;
        }

        public function last(): ?int
        {
            if ($this->native !== null) {
                return $this->native->last();
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                $out = $ffi->new("uint64_t");
                if ($ffi->expanse_set_last($this->handle, FFI::addr($out))) {
                    return (int) $out->cdata;
                }
                return null;
            }
            $k = array_key_last($this->data);
            return $k !== null ? (int) $k : null;
        }

        public function next(int $key): ?int
        {
            if ($this->native !== null) {
                return $this->native->next($key);
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                $out = $ffi->new("uint64_t");
                if ($ffi->expanse_set_next_after($this->handle, $key, FFI::addr($out))) {
                    return (int) $out->cdata;
                }
                return null;
            }
            foreach ($this->data as $k => $v) {
                if ($k > $key) return (int) $k;
            }
            return null;
        }

        public function prev(int $key): ?int
        {
            if ($this->native !== null) {
                return $this->native->prev($key);
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                $out = $ffi->new("uint64_t");
                if ($ffi->expanse_set_prev_before($this->handle, $key, FFI::addr($out))) {
                    return (int) $out->cdata;
                }
                return null;
            }
            $last = null;
            foreach ($this->data as $k => $v) {
                if ($k >= $key) return $last;
                $last = (int) $k;
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
                return (int) $ffi->expanse_set_count_below($this->handle, $key);
            }
            $c = 0;
            foreach ($this->data as $k => $v) {
                if ($k < $key) $c++;
            }
            return $c;
        }

        public function select(int $index): ?int
        {
            if ($this->native !== null) {
                return $this->native->select($index);
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                $out = $ffi->new("uint64_t");
                if ($ffi->expanse_set_by_count($this->handle, $index, FFI::addr($out))) {
                    return (int) $out->cdata;
                }
                return null;
            }
            $i = 0;
            foreach ($this->data as $key => $v) {
                if ($i === $index) return (int) $key;
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
                return (int) $ffi->expanse_set_count_range($this->handle, $start, $end);
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
                return (int) $ffi->expanse_set_mem_used($this->handle);
            }
            return count($this->data) * 8;
        }

        public function union(Set $other): Set
        {
            $res = new Set();
            foreach ($this as $k) {
                $res->add($k);
            }
            foreach ($other as $k) {
                $res->add($k);
            }
            return $res;
        }

        public function intersect(Set $other): Set
        {
            $res = new Set();
            foreach ($this as $k) {
                if ($other->contains($k)) {
                    $res->add($k);
                }
            }
            return $res;
        }

        public function diff(Set $other): Set
        {
            $res = new Set();
            foreach ($this as $k) {
                if (!$other->contains($k)) {
                    $res->add($k);
                }
            }
            return $res;
        }

        public function getIterator(): Traversable
        {
            $keys = [];
            $k = $this->first();
            while ($k !== null) {
                $keys[] = $k;
                $k = $this->next($k);
            }
            return new ArrayIterator($keys);
        }
    }
}
