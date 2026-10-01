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

        /** Status names setStatus() returns, by expanse_blob_status_t value. */
        private const STATUS = [
            0 => 'ok', 16 => 'meta_overflow', 17 => 'allocation_failed',
            18 => 'cap_refused', 19 => 'arena_full', 32 => 'invalid_argument',
        ];

        /**
         * @param int $chunkSize arena chunk size in bytes; 0 selects 2 MiB.
         * @param int $maxCapacity capacity cap on allocated chunk bytes; 0
         *     selects 1 GiB, otherwise clamped to [chunkSize, 64 GiB].
         * @throws \Exception when either is negative, or either is non-zero on
         *     the in-memory fallback, which has no arena to size.
         */
        public function __construct(int $chunkSize = 0, int $maxCapacity = 0)
        {
            if ($chunkSize < 0 || $maxCapacity < 0) {
                throw new \Exception("chunk size $chunkSize and capacity $maxCapacity must not be negative");
            }
            if (NativeDriver::isAvailable() && class_exists(\Expanse\ExpanseBlobMap::class, false)) {
                $this->native = new \Expanse\ExpanseBlobMap($chunkSize, $maxCapacity);
            } elseif (FFIDriver::isAvailable()) {
                $ffi = FFIDriver::getFFI();
                $this->handle = $ffi->expanse_blob_map_new_with_capacity($chunkSize, $maxCapacity);
            } elseif ($chunkSize !== 0 || $maxCapacity !== 0) {
                throw new \Exception('the in-memory BlobMap fallback has no arena to size');
            }
        }

        /**
         * Stores like set(), but returns why an engine refusal happened instead
         * of throwing: 'ok', 'meta_overflow', 'allocation_failed', 'cap_refused'
         * (the capacity cap refused a chunk and nothing was compacted, so
         * compaction may free dead bytes) or 'arena_full' (this insert compacted
         * and the record still does not fit). An out-of-range $hotMeta throws,
         * as in set().
         */
        public function setStatus(int $key, string $payload, int $hotMeta = 0): string
        {
            if ($hotMeta < 0 || $hotMeta > 0xFFFFFFFF) {
                throw new \Exception("hot_meta $hotMeta is outside the 32-bit unsigned range for key $key");
            }
            if ($this->native !== null) {
                return $this->native->setStatus($key, $payload, $hotMeta);
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                $rc = $ffi->expanse_blob_map_insert_ex($this->handle, $key, $payload, strlen($payload), $hotMeta);
                return self::STATUS[$rc] ?? 'error';
            }
            if (strlen($payload) > 7 && $hotMeta > 0xFFFFFF) {
                return 'meta_overflow';
            }
            $this->set($key, $payload, $hotMeta);
            return 'ok';
        }

        /**
         * Turns the reclaim at the capacity cap on (the default) or off. The
         * in-memory fallback has no arena, so there it does nothing.
         */
        public function setReclaimAtCap(bool $on): void
        {
            if ($this->native !== null) {
                $this->native->setReclaimAtCap($on);
            } elseif ($this->handle !== null) {
                FFIDriver::getFFI()->expanse_blob_map_set_reclaim_at_cap($this->handle, $on);
            }
        }

        /**
         * Arena accounting: live_bytes (payloads plus 8-byte headers),
         * allocated_bytes (what the cap counts), max_capacity, chunk_size and
         * reclaim_at_cap (1 or 0).
         *
         * @return array<string, int>
         * @throws \Exception on the in-memory fallback, which has no arena.
         */
        public function arenaStats(): array
        {
            if ($this->native !== null) {
                return $this->native->arenaStats();
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                $st = $ffi->new('expanse_blob_arena_stats_t');
                $ffi->expanse_blob_map_arena_stats($this->handle, FFI::addr($st), FFI::sizeof($st));
                return [
                    'live_bytes' => (int) $st->live_bytes,
                    'allocated_bytes' => (int) $st->allocated_bytes,
                    'max_capacity' => (int) $st->max_capacity,
                    'chunk_size' => (int) $st->chunk_size,
                    'reclaim_at_cap' => (int) $st->reclaim_at_cap,
                ];
            }
            throw new \Exception('the in-memory BlobMap fallback has no arena');
        }

        public function __destruct()
        {
            if ($this->handle !== null) {
                try {
                    $ffi = FFIDriver::getFFI();
                    $ffi->expanse_blob_map_free($this->handle);
                } catch (\Throwable $e) {
                    // A destructor cannot propagate, but it can say something.
                    // Swallowing here leaks the native allocation *and* clears
                    // the handle below, so nothing can retry or report it --
                    // an invisible leak is the one outcome worth avoiding
                    // (AGENTS.md section 8.1).
                    \error_log(
                        'Expanse: freeing the native BlobMap handle failed: '
                        . $e->getMessage()
                    );
                }
                $this->handle = null;
            }
        }

        /**
         * Stores $payload under $key with $hotMeta, replacing any previous value.
         *
         * @throws \Exception when the engine refuses the insert: a payload longer
         *     than 7 bytes with $hotMeta above 24 bits, the arena capacity cap
         *     reached, or an allocation failure. The key keeps its previous value.
         *     Every driver throws the same class; the native extension's message
         *     names the engine error, while the FFI driver's C call reports only
         *     that the insert was refused.
         * @throws \Exception when $hotMeta is outside 0..0xFFFFFFFF, on every
         *     driver, before any driver is called. The engine's field is a
         *     uint32_t: the FFI driver would wrap a negative value (-1 to
         *     0xFFFFFFFF) and truncate a wider one, and the in-memory fallback
         *     stored either as given, so the three backends disagreed.
         */
        public function set(int $key, string $payload, int $hotMeta = 0): void
        {
            if ($hotMeta < 0 || $hotMeta > 0xFFFFFFFF) {
                throw new \Exception("hot_meta $hotMeta is outside the 32-bit unsigned range for key $key");
            }
            if ($this->native !== null) {
                $this->native->set($key, $payload, $hotMeta);
                return;
            }
            if ($this->handle !== null) {
                $ffi = FFIDriver::getFFI();
                if (!$ffi->expanse_blob_map_insert($this->handle, $key, $payload, strlen($payload), $hotMeta)) {
                    throw new \Exception(
                        "expanse_blob_map_insert refused key $key (hot_meta above 24 bits, "
                        . 'arena capacity cap reached, or allocation failure)'
                    );
                }
                return;
            }
            // The in-memory fallback has no arena, but it keeps the one refusal a
            // caller can trigger by argument so the contract matches the drivers.
            if (strlen($payload) > 7 && $hotMeta > 0xFFFFFF) {
                throw new \Exception("MetaOverflow: hot_meta $hotMeta exceeds 24 bits for key $key");
            }
            $this->data[$key] = $payload;
            // The engine stores a payload of at most 7 bytes inside the value
            // slot, which has no metadata field, so it ignores hot_meta there
            // and reads it back as 0 (docs/design/large-values.md). Match it.
            $this->meta[$key] = strlen($payload) > 7 ? $hotMeta : 0;
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
