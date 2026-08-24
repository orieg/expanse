<?php

declare(strict_types=1);

namespace Expanse\Driver;

use FFI;
use RuntimeException;

class FFIDriver
{
    private static ?FFI $ffi = null;

    private const C_DEFINITIONS = <<<'CDEF'
    typedef struct expanse_set expanse_set_t;
    expanse_set_t *expanse_set_new(void);
    void           expanse_set_free(expanse_set_t *set);
    bool     expanse_set_insert(expanse_set_t *set, uint64_t key);
    bool     expanse_set_remove(expanse_set_t *set, uint64_t key);
    bool     expanse_set_contains(const expanse_set_t *set, uint64_t key);
    uint64_t expanse_set_len(const expanse_set_t *set);
    size_t   expanse_set_mem_used(const expanse_set_t *set);
    void     expanse_set_clear(expanse_set_t *set);
    bool expanse_set_first(const expanse_set_t *set, uint64_t *key_out);
    bool expanse_set_last(const expanse_set_t *set, uint64_t *key_out);
    bool expanse_set_next_after(const expanse_set_t *set, uint64_t key, uint64_t *key_out);
    bool expanse_set_prev_before(const expanse_set_t *set, uint64_t key, uint64_t *key_out);
    uint64_t expanse_set_count_below(const expanse_set_t *set, uint64_t key);
    uint64_t expanse_set_count_range(const expanse_set_t *set, uint64_t lo, uint64_t hi);
    bool     expanse_set_by_count(const expanse_set_t *set, uint64_t n, uint64_t *key_out);

    typedef struct expanse_map expanse_map_t;
    expanse_map_t *expanse_map_new(void);
    void           expanse_map_free(expanse_map_t *map);
    bool     expanse_map_insert(expanse_map_t *map, uint64_t key, uint64_t value, uint64_t *old_out);
    bool     expanse_map_get(const expanse_map_t *map, uint64_t key, uint64_t *value_out);
    bool     expanse_map_remove(expanse_map_t *map, uint64_t key, uint64_t *old_out);
    uint64_t expanse_map_len(const expanse_map_t *map);
    size_t   expanse_map_mem_used(const expanse_map_t *map);
    void     expanse_map_clear(expanse_map_t *map);
    bool expanse_map_first(const expanse_map_t *map, uint64_t *key_out, uint64_t *value_out);
    bool expanse_map_last(const expanse_map_t *map, uint64_t *key_out, uint64_t *value_out);
    bool expanse_map_next_after(const expanse_map_t *map, uint64_t key, uint64_t *key_out, uint64_t *value_out);
    bool expanse_map_prev_before(const expanse_map_t *map, uint64_t key, uint64_t *key_out, uint64_t *value_out);
    uint64_t expanse_map_count_below(const expanse_map_t *map, uint64_t key);
    uint64_t expanse_map_count_range(const expanse_map_t *map, uint64_t lo, uint64_t hi);
    bool     expanse_map_by_count(const expanse_map_t *map, uint64_t n, uint64_t *key_out, uint64_t *value_out);

    typedef struct expanse_strmap expanse_strmap_t;
    expanse_strmap_t *expanse_strmap_new(void);
    void             expanse_strmap_free(expanse_strmap_t *map);
    bool     expanse_strmap_insert(expanse_strmap_t *map, const char *key, uint64_t value, uint64_t *old_out);
    bool     expanse_strmap_get(const expanse_strmap_t *map, const char *key, uint64_t *value_out);
    bool     expanse_strmap_remove(expanse_strmap_t *map, const char *key, uint64_t *old_out);
    uint64_t expanse_strmap_len(const expanse_strmap_t *map);
    size_t   expanse_strmap_mem_used(const expanse_strmap_t *map);
    void     expanse_strmap_clear(expanse_strmap_t *map);

    typedef struct expanse_bytesmap expanse_bytesmap_t;
    expanse_bytesmap_t *expanse_bytesmap_new(void);
    void               expanse_bytesmap_free(expanse_bytesmap_t *map);
    bool     expanse_bytesmap_insert(expanse_bytesmap_t *map, const char *key, size_t key_len, uint64_t value, uint64_t *old_out);
    bool     expanse_bytesmap_get(const expanse_bytesmap_t *map, const char *key, size_t key_len, uint64_t *value_out);
    bool     expanse_bytesmap_remove(expanse_bytesmap_t *map, const char *key, size_t key_len, uint64_t *old_out);
    uint64_t expanse_bytesmap_len(const expanse_bytesmap_t *map);
    size_t   expanse_bytesmap_mem_used(const expanse_bytesmap_t *map);
    void     expanse_bytesmap_clear(expanse_bytesmap_t *map);

    typedef struct ExpanseBlobMap ExpanseBlobMap;
    typedef struct {
        const char    *ptr;
        size_t         len;
        uint32_t       hot_meta;
        bool           is_inline;
    } ExpanseBlobView;

    ExpanseBlobMap *expanse_blob_map_new(size_t chunk_size);
    void            expanse_blob_map_free(ExpanseBlobMap *map);
    bool expanse_blob_map_insert(ExpanseBlobMap *map, uint64_t key, const char *data, size_t len, uint32_t hot_meta);
    bool expanse_blob_map_get(const ExpanseBlobMap *map, uint64_t key, ExpanseBlobView *out_view);
    bool expanse_blob_map_remove(ExpanseBlobMap *map, uint64_t key);
    bool expanse_blob_map_compact(ExpanseBlobMap *map);
    uint64_t expanse_blob_map_len(const ExpanseBlobMap *map);
    size_t   expanse_blob_map_mem_used(const ExpanseBlobMap *map);
    void     expanse_blob_map_clear(ExpanseBlobMap *map);
    bool     expanse_blob_map_contains_key(const ExpanseBlobMap *map, uint64_t key);

    typedef struct expanse_sync_set expanse_sync_set_t;
    expanse_sync_set_t *expanse_sync_set_new(void);
    void                expanse_sync_set_free(expanse_sync_set_t *set);
    bool     expanse_sync_set_insert(expanse_sync_set_t *set, uint64_t key);
    bool     expanse_sync_set_remove(expanse_sync_set_t *set, uint64_t key);
    bool     expanse_sync_set_contains(const expanse_sync_set_t *set, uint64_t key);

    typedef struct expanse_sync_map expanse_sync_map_t;
    expanse_sync_map_t *expanse_sync_map_new(void);
    void                expanse_sync_map_free(expanse_sync_map_t *map);
    bool     expanse_sync_map_insert(expanse_sync_map_t *map, uint64_t key, uint64_t value, uint64_t *old_out);
    bool     expanse_sync_map_get(const expanse_sync_map_t *map, uint64_t key, uint64_t *value_out);
    bool     expanse_sync_map_remove(expanse_sync_map_t *map, uint64_t key, uint64_t *old_out);
    CDEF;

    public static function getFFI(): FFI
    {
        if (self::$ffi !== null) {
            return self::$ffi;
        }

        if (!extension_loaded('ffi')) {
            throw new RuntimeException("PHP ext-ffi extension is required to use the Expanse FFI fallback driver.");
        }

        $libPath = self::resolveLibraryPath();
        self::$ffi = FFI::cdef(self::C_DEFINITIONS, $libPath);
        return self::$ffi;
    }

    public static function isAvailable(): bool
    {
        if (!extension_loaded('ffi')) {
            return false;
        }
        try {
            self::getFFI();
            return true;
        } catch (\Throwable) {
            return false;
        }
    }

    private static function resolveLibraryPath(): string
    {
        if ($envPath = getenv('EXPANSE_LIBRARY_PATH')) {
            if (file_exists($envPath)) {
                return $envPath;
            }
        }

        $isDarwin = PHP_OS_FAMILY === 'Darwin';
        $isWindows = PHP_OS_FAMILY === 'Windows';
        $ext = $isWindows ? 'dll' : ($isDarwin ? 'dylib' : 'so');
        $prefix = $isWindows ? '' : 'lib';

        $candidates = [
            __DIR__ . "/../../../../target/release/{$prefix}expanse.{$ext}",
            __DIR__ . "/../../../../target/debug/{$prefix}expanse.{$ext}",
            __DIR__ . "/../../lib/{$prefix}expanse.{$ext}",
            "/usr/local/lib/{$prefix}expanse.{$ext}",
            "/usr/lib/{$prefix}expanse.{$ext}",
            "{$prefix}expanse.{$ext}",
        ];

        foreach ($candidates as $candidate) {
            if (file_exists($candidate)) {
                return realpath($candidate) ?: $candidate;
            }
        }

        return "{$prefix}expanse.{$ext}";
    }
}
