<?php

declare(strict_types=1);

namespace Expanse;

use Expanse\Contract\BytesMapInterface;
use Expanse\Driver\FFIDriver;
use Expanse\Driver\NativeDriver;
use ArrayIterator;
use FFI;
use Traversable;

if (!class_exists(BytesMap::class)) {
    class BytesMap implements BytesMapInterface
    {
        private ?object $native = null;
        private mixed $handle = null;
        private array $data = [];

        public function __construct()
        {
            if (NativeDriver::isAvailable() && class_exists(\Expanse\ExpanseBytesMap::class, false)) {
                $this->native = new \Expanse\ExpanseBytesMap();
            } elseif (FFIDriver::isAvailable()) {
                $ffi = FFIDriver::getFFI();
                $this->handle = $ffi->expanse_bytesmap_new();
            }
        }

        public function __destruct()
        {
            if ($this->handle !== null) {
                try {
                    $ffi = FFIDriver::getFFI();
                    $ffi->expanse_bytesmap_free($this->handle);
                } catch (\Throwable) {}
                $this->handle = null;
            }
        }

        public function set(string $key, int $value): void
        {
            if ($this->native !== null) {
                $this->native->set($key, $value);
                return;
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                $ffi->expanse_bytesmap_insert($this->handle, $key, strlen($key), $value, null);
                return;
            }
            $this->data[$key] = $value;
        }

        public function get(string $key): ?int
        {
            if ($this->native !== null) {
                return $this->native->get($key);
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                $out = $ffi->new("uint64_t");
                if ($ffi->expanse_bytesmap_get($this->handle, $key, strlen($key), FFI::addr($out))) {
                    return (int) $out->cdata;
                }
                return null;
            }
            return $this->data[$key] ?? null;
        }

        public function delete(string $key): bool
        {
            if ($this->native !== null) {
                return $this->native->delete($key);
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                return (bool) $ffi->expanse_bytesmap_remove($this->handle, $key, strlen($key), null);
            }
            $res = isset($this->data[$key]);
            unset($this->data[$key]);
            return $res;
        }

        public function has(string $key): bool
        {
            if ($this->native !== null) {
                return $this->native->has($key);
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                $out = $ffi->new("uint64_t");
                return (bool) $ffi->expanse_bytesmap_get($this->handle, $key, strlen($key), FFI::addr($out));
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
                return (int) $ffi->expanse_bytesmap_len($this->handle);
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
                $ffi->expanse_bytesmap_clear($this->handle);
                return;
            }
            $this->data = [];
        }

        public function memUsed(): int
        {
            if ($this->native !== null) {
                return (int) $this->native->memUsed();
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                return (int) $ffi->expanse_bytesmap_mem_used($this->handle);
            }
            return count($this->data) * 32;
        }

        public function getIterator(): Traversable
        {
            return new ArrayIterator($this->data);
        }

        public function offsetExists(mixed $offset): bool { return $this->has((string) $offset); }
        public function offsetGet(mixed $offset): mixed { return $this->get((string) $offset); }
        public function offsetSet(mixed $offset, mixed $value): void { $this->set((string) $offset, (int) $value); }
        public function offsetUnset(mixed $offset): void { $this->delete((string) $offset); }
    }
}
