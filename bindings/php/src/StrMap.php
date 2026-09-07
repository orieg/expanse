<?php

declare(strict_types=1);

namespace Expanse;

use Expanse\Contract\StrMapInterface;
use Expanse\Driver\FFIDriver;
use Expanse\Driver\NativeDriver;
use ArrayIterator;
use FFI;
use Traversable;

if (!class_exists(StrMap::class)) {
    class StrMap implements StrMapInterface
    {
        private ?object $native = null;
        private mixed $handle = null;
        private array $data = [];

        public function __construct()
        {
            if (NativeDriver::isAvailable() && class_exists(\Expanse\ExpanseStrMap::class, false)) {
                $this->native = new \Expanse\ExpanseStrMap();
            } elseif (FFIDriver::isAvailable()) {
                $ffi = FFIDriver::getFFI();
                $this->handle = $ffi->expanse_strmap_new();
            }
        }

        public function __destruct()
        {
            if ($this->handle !== null) {
                try {
                    $ffi = FFIDriver::getFFI();
                    $ffi->expanse_strmap_free($this->handle);
                } catch (\Throwable $e) {
                    // A destructor cannot propagate, but it can say something.
                    // Swallowing here leaks the native allocation *and* clears
                    // the handle below, so nothing can retry or report it --
                    // an invisible leak is the one outcome worth avoiding
                    // (AGENTS.md section 8.1).
                    \error_log(
                        'Expanse: freeing the native StrMap handle failed: '
                        . $e->getMessage()
                    );
                }
                $this->handle = null;
            }
        }

        /**
         * Rejects a key carrying an embedded NUL byte.
         *
         * ExpanseStrMap documents a NUL-free key domain and it is load-bearing:
         * a trailing NUL is how the encoding terminates a string, so a
         * NUL-bearing key is stored and returned by get() but cannot be
         * addressed by the ordered surface. PHP strings are binary-safe, so
         * this class is the boundary that has to hold the line.
         *
         * It is checked here, above the driver switch, because the three
         * backends disagreed without it. The native extension received the full
         * binary-safe string; the \FFI driver passes the key as a char*, which
         * the C ABI reads with CStr::from_ptr and truncates at the first NUL;
         * the pure-PHP array keeps it whole. The same PHP source therefore
         * stored a different key depending on which backend happened to be
         * available on the host, silently. One check above the switch is what
         * makes "identical contracts" true rather than aspirational.
         */
        private static function assertNulFree(string $key): void
        {
            if (\strpos($key, "\0") !== false) {
                throw new \InvalidArgumentException(
                    'NUL bytes ("\0") are not allowed in ExpanseStrMap keys'
                );
            }
        }

        public function set(string $key, int $value): void
        {
            self::assertNulFree($key);
            if ($this->native !== null) {
                $this->native->set($key, $value);
                return;
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                $ffi->expanse_strmap_insert($this->handle, $key, $value, null);
                return;
            }
            $this->data[$key] = $value;
            ksort($this->data);
        }

        public function get(string $key): ?int
        {
            self::assertNulFree($key);
            if ($this->native !== null) {
                return $this->native->get($key);
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                $out = $ffi->new("uint64_t");
                if ($ffi->expanse_strmap_get($this->handle, $key, FFI::addr($out))) {
                    return (int) $out->cdata;
                }
                return null;
            }
            return $this->data[$key] ?? null;
        }

        public function delete(string $key): bool
        {
            self::assertNulFree($key);
            if ($this->native !== null) {
                return $this->native->delete($key);
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                return (bool) $ffi->expanse_strmap_remove($this->handle, $key, null);
            }
            $res = isset($this->data[$key]);
            unset($this->data[$key]);
            return $res;
        }

        public function has(string $key): bool
        {
            self::assertNulFree($key);
            if ($this->native !== null) {
                return $this->native->has($key);
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                $out = $ffi->new("uint64_t");
                return (bool) $ffi->expanse_strmap_get($this->handle, $key, FFI::addr($out));
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
                return (int) $ffi->expanse_strmap_len($this->handle);
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
                $ffi->expanse_strmap_clear($this->handle);
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
                return (int) $ffi->expanse_strmap_mem_used($this->handle);
            }
            return count($this->data) * 24;
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
