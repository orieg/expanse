<?php

declare(strict_types=1);

namespace Expanse;

use Expanse\Driver\FFIDriver;
use Expanse\Driver\NativeDriver;
use FFI;

if (!class_exists(SyncMap::class)) {
    class SyncMap
    {
        private ?object $native = null;
        private mixed $handle = null;
        private array $data = [];

        public function __construct()
        {
            if (NativeDriver::isAvailable() && class_exists(\Expanse\SyncMap::class, false)) {
                $this->native = new \Expanse\SyncMap();
            } elseif (FFIDriver::isAvailable()) {
                $ffi = FFIDriver::getFFI();
                $this->handle = $ffi->expanse_sync_map_new();
            }
        }

        public function __destruct()
        {
            if ($this->handle !== null) {
                try {
                    $ffi = FFIDriver::getFFI();
                    $ffi->expanse_sync_map_free($this->handle);
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
                $ffi->expanse_sync_map_insert($this->handle, $key, $value);
                return;
            }
            $this->data[$key] = $value;
        }

        public function get(int $key): ?int
        {
            if ($this->native !== null) {
                return $this->native->get($key);
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                $out = $ffi->new("uint64_t");
                if ($ffi->expanse_sync_map_get($this->handle, $key, FFI::addr($out))) {
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
                return (bool) $ffi->expanse_sync_map_remove($this->handle, $key);
            }
            $res = isset($this->data[$key]);
            unset($this->data[$key]);
            return $res;
        }
    }
}
