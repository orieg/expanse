<?php

declare(strict_types=1);

namespace Expanse;

use Expanse\Driver\FFIDriver;
use Expanse\Driver\NativeDriver;

if (!class_exists(SyncSet::class)) {
    class SyncSet
    {
        private ?object $native = null;
        private mixed $handle = null;
        private array $data = [];

        public function __construct()
        {
            if (NativeDriver::isAvailable() && class_exists(\Expanse\SyncSet::class, false)) {
                $this->native = new \Expanse\SyncSet();
            } elseif (FFIDriver::isAvailable()) {
                $ffi = FFIDriver::getFFI();
                $this->handle = $ffi->expanse_sync_set_new();
            }
        }

        public function __destruct()
        {
            if ($this->handle !== null) {
                try {
                    $ffi = FFIDriver::getFFI();
                    $ffi->expanse_sync_set_free($this->handle);
                } catch (\Throwable $e) {
                    // A destructor cannot propagate, but it can say something.
                    // Swallowing here leaks the native allocation *and* clears
                    // the handle below, so nothing can retry or report it --
                    // an invisible leak is the one outcome worth avoiding
                    // (AGENTS.md section 8.1).
                    \error_log(
                        'Expanse: freeing the native SyncSet handle failed: '
                        . $e->getMessage()
                    );
                }
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
                return (bool) $ffi->expanse_sync_set_insert($this->handle, $key);
            }
            $res = !isset($this->data[$key]);
            $this->data[$key] = true;
            return $res;
        }

        public function remove(int $key): bool
        {
            if ($this->native !== null) {
                return $this->native->remove($key);
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                return (bool) $ffi->expanse_sync_set_remove($this->handle, $key);
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
                return (bool) $ffi->expanse_sync_set_contains($this->handle, $key);
            }
            return isset($this->data[$key]);
        }
    }
}
