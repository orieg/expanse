<?php

declare(strict_types=1);

namespace Expanse;

use Expanse\Contract\BlobMapInterface;
use Expanse\Driver\FFIDriver;
use Expanse\Driver\NativeDriver;
use ArrayIterator;
use FFI;
use Traversable;

if (!class_exists(BlobMap::class)) {
    class BlobMap implements BlobMapInterface
    {
        private ?object $native = null;
        private mixed $handle = null;
        private array $data = [];
        private array $meta = [];

        public function __construct()
        {
            if (NativeDriver::isAvailable() && class_exists(\Expanse\ExpanseBlobMap::class, false)) {
                $this->native = new \Expanse\ExpanseBlobMap();
            } elseif (FFIDriver::isAvailable()) {
                $ffi = FFIDriver::getFFI();
                $this->handle = $ffi->expanse_blob_map_new(0);
            }
        }

        public function __destruct()
        {
            if ($this->handle !== null) {
                try {
                    $ffi = FFIDriver::getFFI();
                    $ffi->expanse_blob_map_free($this->handle);
                } catch (\Throwable) {}
                $this->handle = null;
            }
        }

        public function set(int $key, string $payload, int $hotMeta = 0): void
        {
            if ($this->native !== null) {
                $this->native->set($key, $payload, $hotMeta);
                return;
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                $ffi->expanse_blob_map_insert($this->handle, $key, $payload, strlen($payload), $hotMeta);
                return;
            }
            $this->data[$key] = $payload;
            $this->meta[$key] = $hotMeta;
        }

        public function get(int $key, int &$hotMeta = 0): ?string
        {
            if ($this->native !== null) {
                $res = $this->native->get($key);
                if ($res !== null) {
                    $hotMeta = $this->native->getMeta($key) ?? 0;
                }
                return $res;
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                $view = $ffi->new("ExpanseBlobView");
                if ($ffi->expanse_blob_map_get($this->handle, $key, FFI::addr($view))) {
                    $hotMeta = (int) $view->hot_meta;
                    return FFI::string($view->ptr, (int) $view->len);
                }
                return null;
            }
            if (isset($this->data[$key])) {
                $hotMeta = $this->meta[$key] ?? 0;
                return $this->data[$key];
            }
            return null;
        }

        public function getMeta(int $key): ?int
        {
            if ($this->native !== null) {
                return $this->native->getMeta($key);
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                $view = $ffi->new("ExpanseBlobView");
                if ($ffi->expanse_blob_map_get($this->handle, $key, FFI::addr($view))) {
                    return (int) $view->hot_meta;
                }
                return null;
            }
            return $this->meta[$key] ?? null;
        }

        public function delete(int $key): bool
        {
            if ($this->native !== null) {
                return $this->native->delete($key);
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                return (bool) $ffi->expanse_blob_map_remove($this->handle, $key);
            }
            $res = isset($this->data[$key]);
            unset($this->data[$key], $this->meta[$key]);
            return $res;
        }

        public function has(int $key): bool
        {
            if ($this->native !== null) {
                return $this->native->has($key);
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                return (bool) $ffi->expanse_blob_map_contains_key($this->handle, $key);
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
                return (int) $ffi->expanse_blob_map_len($this->handle);
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
                $ffi->expanse_blob_map_clear($this->handle);
                return;
            }
            $this->data = [];
            $this->meta = [];
        }

        public function memUsed(): int
        {
            if ($this->native !== null) {
                return (int) $this->native->memUsed();
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                return (int) $ffi->expanse_blob_map_mem_used($this->handle);
            }
            return count($this->data) * 48;
        }

        public function prune(callable $predicate): int
        {
            $c = 0;
            foreach ($this->data as $k => $v) {
                if ($predicate($k, $this->meta[$k] ?? 0)) {
                    $this->delete($k);
                    $c++;
                }
            }
            return $c;
        }

        public function saveImage(string $path): void
        {
            file_put_contents($path, serialize([$this->data, $this->meta]));
        }

        public static function openImage(string $path, bool $mmap = true): self
        {
            $b = new self();
            list($b->data, $b->meta) = unserialize(file_get_contents($path));
            return $b;
        }

        public function getIterator(): Traversable
        {
            return new ArrayIterator($this->data);
        }

        public function offsetExists(mixed $offset): bool { return $this->has((int) $offset); }
        public function offsetGet(mixed $offset): mixed { return $this->get((int) $offset); }
        public function offsetSet(mixed $offset, mixed $value): void { $this->set((int) $offset, (string) $value); }
        public function offsetUnset(mixed $offset): void { $this->delete((int) $offset); }
    }
}
