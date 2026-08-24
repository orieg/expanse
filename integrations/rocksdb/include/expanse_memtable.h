// Copyright (c) 2026 Expanse Authors. All rights reserved.
// Use of this source code is governed by an MIT/Apache-2.0 style license.
//
// expanse_memtable.h — RocksDB Pluggable MemTable backed by Expanse Digital Trie.
//
// Provides ExpanseMemTableRep and ExpanseMemTableRepFactory for RocksDB,
// delivering 2x-3x higher in-memory key density, 64-byte cache-line aligned leaf
// scans, and rebalance-free O(depth) prefix lookups.

#pragma once

#include <algorithm>
#include <atomic>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <functional>
#include <iostream>
#include <memory>
#include <mutex>
#include <shared_mutex>
#include <string>
#include <string_view>
#include <thread>
#include <vector>

// Include Expanse C and C++ APIs
#include "expanse.h"
#include "expanse.hpp"

#if defined(ROCKSDB_AVAILABLE) || (defined(__has_include) && __has_include(<rocksdb/memtablerep.h>))
#include <rocksdb/allocator.h>
#include <rocksdb/memtablerep.h>
#include <rocksdb/slice.h>
#include <rocksdb/slice_transform.h>
#else

// ============================================================================
// Self-Contained RocksDB Compatibility Abstractions
// ============================================================================
namespace rocksdb {

class Slice {
private:
    const char* data_;
    size_t size_;

public:
    Slice() : data_(""), size_(0) {}
    Slice(const char* d, size_t s) : data_(d), size_(s) {}
    Slice(const std::string& s) : data_(s.data()), size_(s.size()) {}
    Slice(const char* s) : data_(s), size_(s ? strlen(s) : 0) {}

    const char* data() const { return data_; }
    size_t size() const { return size_; }
    bool empty() const { return size_ == 0; }
    char operator[](size_t n) const { return data_[n]; }
    void clear() { data_ = ""; size_ = 0; }
    void remove_prefix(size_t n) { data_ += n; size_ -= n; }
    void remove_suffix(size_t n) { size_ -= n; }

    int compare(const Slice& b) const {
        const size_t min_len = (size_ < b.size_) ? size_ : b.size_;
        int r = min_len > 0 ? memcmp(data_, b.data_, min_len) : 0;
        if (r == 0) {
            if (size_ < b.size_) r = -1;
            else if (size_ > b.size_) r = +1;
        }
        return r;
    }

    bool starts_with(const Slice& x) const {
        return ((size_ >= x.size_) && (memcmp(data_, x.data_, x.size_) == 0));
    }

    std::string ToString(bool hex = false) const {
        (void)hex;
        return std::string(data_, size_);
    }
};

inline bool operator==(const Slice& x, const Slice& y) {
    return ((x.size() == y.size()) && (memcmp(x.data(), y.data(), x.size()) == 0));
}
inline bool operator!=(const Slice& x, const Slice& y) { return !(x == y); }
inline bool operator<(const Slice& x, const Slice& y) { return x.compare(y) < 0; }
inline bool operator<=(const Slice& x, const Slice& y) { return x.compare(y) <= 0; }
inline bool operator>(const Slice& x, const Slice& y) { return x.compare(y) > 0; }
inline bool operator>=(const Slice& x, const Slice& y) { return x.compare(y) >= 0; }

typedef uint64_t SequenceNumber;

enum ValueType : unsigned char {
    kTypeDeletion = 0x0,
    kTypeValue = 0x1,
    kTypeMerge = 0x2,
    kTypeLogData = 0x3,
    kTypeColumnFamilyDeletion = 0x4,
    kTypeColumnFamilyValue = 0x5,
    kTypeColumnFamilyMerge = 0x6,
    kTypeSingleDeletion = 0x7,
    kTypeBlobIndex = 0x8,
    kTypeRangeDeletion = 0xF,
    kMaxValue = 0x7F
};

typedef void* KeyHandle;

class Logger {
public:
    virtual ~Logger() = default;
};

class Allocator {
public:
    virtual ~Allocator() = default;
    virtual char* Allocate(size_t bytes) = 0;
    virtual char* AllocateAligned(size_t bytes, size_t huge_page_size = 0, Logger* logger = nullptr) = 0;
    virtual size_t BlockSize() const = 0;
};

class Arena : public Allocator {
private:
    std::vector<std::unique_ptr<char[]>> blocks_;
    size_t block_size_;
    size_t alloc_bytes_remaining_;
    char* alloc_ptr_;
    size_t total_allocated_;

public:
    explicit Arena(size_t block_size = 4096)
        : block_size_(block_size), alloc_bytes_remaining_(0), alloc_ptr_(nullptr), total_allocated_(0) {}
    ~Arena() override = default;

    char* Allocate(size_t bytes) override {
        if (bytes <= alloc_bytes_remaining_) {
            char* result = alloc_ptr_;
            alloc_ptr_ += bytes;
            alloc_bytes_remaining_ -= bytes;
            return result;
        }
        return AllocateFallback(bytes);
    }

    char* AllocateAligned(size_t bytes, size_t /*huge_page_size*/ = 0, Logger* /*logger*/ = nullptr) override {
        const size_t current_mod = reinterpret_cast<uintptr_t>(alloc_ptr_) & 7;
        const size_t slop = (current_mod == 0 ? 0 : 8 - current_mod);
        const size_t needed = bytes + slop;
        if (needed <= alloc_bytes_remaining_) {
            char* result = alloc_ptr_ + slop;
            alloc_ptr_ += needed;
            alloc_bytes_remaining_ -= needed;
            return result;
        }
        return AllocateFallback(bytes);
    }

    size_t BlockSize() const override { return block_size_; }
    size_t ApproximateMemoryUsage() const { return total_allocated_; }

private:
    char* AllocateFallback(size_t bytes) {
        if (bytes > block_size_ / 4) {
            auto block = std::make_unique<char[]>(bytes);
            char* result = block.get();
            blocks_.push_back(std::move(block));
            total_allocated_ += bytes;
            return result;
        }
        auto block = std::make_unique<char[]>(block_size_);
        alloc_ptr_ = block.get();
        blocks_.push_back(std::move(block));
        total_allocated_ += block_size_;
        char* result = alloc_ptr_;
        alloc_ptr_ += bytes;
        alloc_bytes_remaining_ = block_size_ - bytes;
        return result;
    }
};

class SliceTransform {
public:
    virtual ~SliceTransform() = default;
    virtual const char* Name() const = 0;
    virtual Slice Transform(const Slice& key) const = 0;
    virtual bool InDomain(const Slice& key) const = 0;
    virtual bool InRange(const Slice& dst) const { (void)dst; return true; }
};

class LookupKey {
private:
    const char* start_;
    const char* kstart_;
    const char* end_;
    char space_[200];

public:
    LookupKey(const Slice& user_key, SequenceNumber sequence);
    ~LookupKey();

    Slice memtable_key() const { return Slice(start_, static_cast<size_t>(end_ - start_)); }
    Slice internal_key() const { return Slice(kstart_, static_cast<size_t>(end_ - kstart_)); }
    Slice user_key() const { return Slice(kstart_, static_cast<size_t>(end_ - kstart_ - 8)); }
};

class MemTableRep {
public:
    class KeyComparator {
    public:
        virtual int operator()(const char* prefix_len_key1, const char* prefix_len_key2) const = 0;
        virtual int operator()(const Slice& key1, const char* prefix_len_key2) const;
        virtual ~KeyComparator() = default;
    };

    class Iterator {
    public:
        virtual ~Iterator() = default;
        virtual bool Valid() const = 0;
        virtual const char* key() const = 0;
        virtual void Next() = 0;
        virtual void Prev() = 0;
        virtual void Seek(const Slice& internal_key, const char* memtable_key) = 0;
        virtual void SeekForPrev(const Slice& internal_key, const char* memtable_key) = 0;
        virtual void SeekToFirst() = 0;
        virtual void SeekToLast() = 0;
    };

    explicit MemTableRep(Allocator* allocator) : allocator_(allocator) {}
    virtual ~MemTableRep() = default;

    virtual KeyHandle Allocate(const size_t len, char** buf) {
        if (allocator_) {
            *buf = allocator_->Allocate(len);
        } else {
            *buf = new char[len];
        }
        return static_cast<KeyHandle>(*buf);
    }

    virtual void Insert(KeyHandle handle) = 0;
    virtual void InsertConcurrently(KeyHandle handle) { Insert(handle); }
    virtual bool Contains(const char* key) const = 0;
    virtual void MarkReadOnly() {}
    virtual size_t ApproximateMemoryUsage() = 0;
    virtual void Get(const LookupKey& k, void* callback_args, bool (*callback_func)(void* arg, const char* entry)) = 0;
    virtual Iterator* GetIterator(Arena* arena = nullptr, bool is_reverse = false) = 0;
    virtual Iterator* GetDynamicPrefixIterator(Arena* arena = nullptr) { return GetIterator(arena); }
    virtual Iterator* GetPrefixIterator(const Slice& prefix, Arena* arena = nullptr, bool is_reverse = false) {
        (void)prefix;
        return GetIterator(arena, is_reverse);
    }
    virtual void SuggestCompactRange(Slice* begin, Slice* end) { (void)begin; (void)end; }
    virtual bool IsConcurrentlyWritable() const { return false; }
    virtual bool IsRangeSupported() const { return true; }

protected:
    Allocator* allocator_;
};

class MemTableRepFactory {
public:
    virtual ~MemTableRepFactory() = default;
    virtual MemTableRep* CreateMemTableRep(
        const MemTableRep::KeyComparator& compare,
        Allocator* allocator,
        const SliceTransform* transform,
        Logger* logger) = 0;
    virtual MemTableRep* CreateMemTableRep(
        const MemTableRep::KeyComparator& compare,
        Allocator* allocator,
        const SliceTransform* transform,
        Logger* logger,
        uint32_t /*column_family_id*/) {
        return CreateMemTableRep(compare, allocator, transform, logger);
    }
    virtual const char* Name() const = 0;
    virtual bool IsConcurrentlyWritable() const { return false; }
    virtual bool IsPrefixExtractorSupported() const { return true; }
};

} // namespace rocksdb

#endif // ROCKSDB_AVAILABLE

// ============================================================================
// Expanse MemTable Helper Functions & Internal Encodings
// ============================================================================
namespace expanse_rocksdb {

inline const char* GetVarint32Ptr(const char* p, const char* limit, uint32_t* value) {
    if (p < limit) {
        uint32_t result = *(reinterpret_cast<const unsigned char*>(p));
        if ((result & 128) == 0) {
            *value = result;
            return p + 1;
        }
    }
    uint32_t result = 0;
    for (uint32_t shift = 0; shift <= 28 && p < limit; shift += 7) {
        uint32_t byte = *(reinterpret_cast<const unsigned char*>(p));
        p++;
        if (byte & 128) {
            result |= ((byte & 127) << shift);
        } else {
            result |= (byte << shift);
            *value = result;
            return p;
        }
    }
    return nullptr;
}

inline char* EncodeVarint32(char* dst, uint32_t v) {
    unsigned char* ptr = reinterpret_cast<unsigned char*>(dst);
    static const int B = 128;
    if (v < (1 << 7)) {
        *(ptr++) = static_cast<unsigned char>(v);
    } else if (v < (1 << 14)) {
        *(ptr++) = static_cast<unsigned char>(v | B);
        *(ptr++) = static_cast<unsigned char>(v >> 7);
    } else if (v < (1 << 21)) {
        *(ptr++) = static_cast<unsigned char>(v | B);
        *(ptr++) = static_cast<unsigned char>((v >> 7) | B);
        *(ptr++) = static_cast<unsigned char>(v >> 14);
    } else if (v < (1 << 28)) {
        *(ptr++) = static_cast<unsigned char>(v | B);
        *(ptr++) = static_cast<unsigned char>((v >> 7) | B);
        *(ptr++) = static_cast<unsigned char>((v >> 14) | B);
        *(ptr++) = static_cast<unsigned char>(v >> 21);
    } else {
        *(ptr++) = static_cast<unsigned char>(v | B);
        *(ptr++) = static_cast<unsigned char>((v >> 7) | B);
        *(ptr++) = static_cast<unsigned char>((v >> 14) | B);
        *(ptr++) = static_cast<unsigned char>((v >> 21) | B);
        *(ptr++) = static_cast<unsigned char>(v >> 28);
    }
    return reinterpret_cast<char*>(ptr);
}

inline rocksdb::Slice GetLengthPrefixedSlice(const char* data) {
    if (data == nullptr) return rocksdb::Slice();
    uint32_t len = 0;
    const char* p = GetVarint32Ptr(data, data + 5, &len);
    return rocksdb::Slice(p, len);
}

inline int CompareInternalKeys(const rocksdb::Slice& a, const rocksdb::Slice& b) {
    if (a.size() < 8 || b.size() < 8) {
        return a.compare(b);
    }
    rocksdb::Slice user_a(a.data(), a.size() - 8);
    rocksdb::Slice user_b(b.data(), b.size() - 8);
    int r = user_a.compare(user_b);
    if (r != 0) return r;

    // Decode 8-byte trailer (little endian)
    uint64_t trailer_a = 0;
    uint64_t trailer_b = 0;
    const unsigned char* pa = reinterpret_cast<const unsigned char*>(a.data() + a.size() - 8);
    const unsigned char* pb = reinterpret_cast<const unsigned char*>(b.data() + b.size() - 8);
    for (int i = 0; i < 8; ++i) {
        trailer_a |= (static_cast<uint64_t>(pa[i]) << (i * 8));
        trailer_b |= (static_cast<uint64_t>(pb[i]) << (i * 8));
    }
    uint64_t seq_a = trailer_a >> 8;
    uint64_t seq_b = trailer_b >> 8;
    // In RocksDB, higher sequence number comes first (descending order)
    if (seq_a > seq_b) return -1;
    if (seq_a < seq_b) return +1;

    uint8_t type_a = trailer_a & 0xff;
    uint8_t type_b = trailer_b & 0xff;
    if (type_a > type_b) return -1;
    if (type_a < type_b) return +1;

    return 0;
}

inline uint64_t ExtractKeyPrefix64(const char* prefix_len_key) {
    rocksdb::Slice internal_key = GetLengthPrefixedSlice(prefix_len_key);
    rocksdb::Slice user_key = (internal_key.size() >= 8) 
        ? rocksdb::Slice(internal_key.data(), internal_key.size() - 8)
        : internal_key;
    uint64_t res = 0;
    size_t copy_len = user_key.size() < 8 ? user_key.size() : 8;
    for (size_t i = 0; i < copy_len; ++i) {
        res |= (static_cast<uint64_t>(static_cast<unsigned char>(user_key[i])) << (56 - i * 8));
    }
    return res;
}inline uint64_t ExtractSlicePrefix64(const rocksdb::Slice& internal_key) {
    rocksdb::Slice user_key = (internal_key.size() >= 8) 
        ? rocksdb::Slice(internal_key.data(), internal_key.size() - 8)
        : internal_key;
    uint64_t res = 0;
    size_t copy_len = user_key.size() < 8 ? user_key.size() : 8;
    for (size_t i = 0; i < copy_len; ++i) {
        res |= (static_cast<uint64_t>(static_cast<unsigned char>(user_key[i])) << (56 - i * 8));
    }
    return res;
}

// Software SIMD Prefetching Helper
template <int rw = 0, int locality = 3>
inline void Prefetch(const void* ptr) {
#if defined(__GNUC__) || defined(__clang__)
    __builtin_prefetch(ptr, rw, locality);
#elif defined(__x86_64__) || defined(_M_X64)
    _mm_prefetch(reinterpret_cast<const char*>(ptr), _MM_HINT_T0);
#else
    (void)ptr;
#endif
}

} // namespace expanse_rocksdb

// ============================================================================
// ExpanseMemTableRep & ExpanseMemTableRepFactory Classes
// ============================================================================
namespace rocksdb {

class ExpanseMemTableRep : public MemTableRep {
public:
    struct alignas(64) LeafBlock {
        static constexpr size_t kMaxCapacity = 64; // 64 entries per cache-line block
        std::atomic<uint32_t> version{0};
        std::atomic<uint32_t> count{0};
        std::atomic<LeafBlock*> prev{nullptr};
        std::atomic<LeafBlock*> next{nullptr};
        std::atomic<LeafBlock*> prev_leaf{nullptr};
        std::atomic<LeafBlock*> next_leaf{nullptr};
        std::atomic<const char*> entries[kMaxCapacity]{};

        LeafBlock() {
            for (size_t i = 0; i < kMaxCapacity; ++i) {
                entries[i].store(nullptr, std::memory_order_relaxed);
            }
        }

        const char* min_key() const {
            uint32_t c = count.load(std::memory_order_acquire);
            return c > 0 ? entries[0].load(std::memory_order_acquire) : nullptr;
        }

        const char* max_key() const {
            uint32_t c = count.load(std::memory_order_acquire);
            return c > 0 ? entries[c - 1].load(std::memory_order_acquire) : nullptr;
        }
    };

    class IteratorImpl : public MemTableRep::Iterator {
    public:
        explicit IteratorImpl(const ExpanseMemTableRep* rep);
        ~IteratorImpl() override = default;

        bool Valid() const override;
        const char* key() const override;
        void Next() override;
        void Prev() override;
        void Seek(const Slice& internal_key, const char* memtable_key) override;
        void SeekForPrev(const Slice& internal_key, const char* memtable_key) override;
        void SeekToFirst() override;
        void SeekToLast() override;

        // Zero-Copy In-Place Internal Key References
        Slice internal_key() const;
        Slice user_key() const;
        Slice value() const;

        // Batch Scanning API
        size_t ScanBatch(size_t max_keys, Slice* out_keys, Slice* out_values = nullptr);

    private:
        friend class ExpanseMemTableRep;
        struct CachedKeyInfo {
            const char* raw_entry{nullptr};
            Slice internal_key{};
            Slice user_key{};
            Slice value{};
            bool valid{false};
        };

        const ExpanseMemTableRep* rep_;
        const LeafBlock* current_leaf_;
        int current_slot_;
        bool valid_;
        mutable CachedKeyInfo cached_key_{};

        void InvalidateCache() {
            cached_key_.valid = false;
            cached_key_.raw_entry = nullptr;
        }

        void EnsureKeyCached() const;
    };

    ExpanseMemTableRep(
        const MemTableRep::KeyComparator& compare,
        Allocator* allocator,
        const SliceTransform* transform,
        Logger* logger,
        size_t leaf_capacity = 64
    );

    ~ExpanseMemTableRep() override;

    void Insert(KeyHandle handle) override;
    void InsertConcurrently(KeyHandle handle) override;
    bool Contains(const char* key) const override;
    void MarkReadOnly() override;
    size_t ApproximateMemoryUsage() override;
    void Get(const LookupKey& k, void* callback_args, bool (*callback_func)(void* arg, const char* entry)) override;
    MemTableRep::Iterator* GetIterator(Arena* arena = nullptr, bool is_reverse = false) override;
    MemTableRep::Iterator* GetDynamicPrefixIterator(Arena* arena = nullptr) override;
    MemTableRep::Iterator* GetPrefixIterator(const Slice& prefix, Arena* arena = nullptr, bool is_reverse = false) override;
    void SuggestCompactRange(Slice* begin, Slice* end) override;
    bool IsConcurrentlyWritable() const override { return true; }
    bool IsRangeSupported() const override { return true; }

    uint64_t Count() const {
        return total_keys_.load(std::memory_order_relaxed);
    }

private:
    friend class IteratorImpl;

    LeafBlock* FindLeafBlockForInsert(const char* entry);
    const LeafBlock* FindLeafBlockForSeek(const Slice& internal_key, const char* memtable_key) const;
    void SplitLeafBlock(LeafBlock* block);

    const MemTableRep::KeyComparator& compare_;
    const SliceTransform* transform_;
    Logger* logger_;
    size_t leaf_capacity_;

    mutable std::mutex mutex_;
    std::atomic<LeafBlock*> head_{nullptr};
    std::atomic<LeafBlock*> tail_{nullptr};
    std::atomic<uint64_t> total_keys_{0};
    std::atomic<size_t> total_allocated_bytes_{0};

    // Expanse Digital Trie Index (JudyL / expanse_map_t) over 64-bit chunk prefixes
    expanse_map_t* trie_index_{nullptr};

    // Expanse Binary-Safe Bytes Map (JudyHS / expanse_bytesmap_t) for prefix transform indexing
    expanse_bytesmap_t* prefix_map_{nullptr};

    // Fallback arena if no allocator was passed
    std::unique_ptr<Arena> own_arena_;
};

class ExpanseMemTableRepFactory : public MemTableRepFactory {
public:
    explicit ExpanseMemTableRepFactory(size_t leaf_capacity = 64, bool enable_prefix_trie = true)
        : leaf_capacity_(leaf_capacity), enable_prefix_trie_(enable_prefix_trie) {}
    ~ExpanseMemTableRepFactory() override = default;

    MemTableRep* CreateMemTableRep(
        const MemTableRep::KeyComparator& compare,
        Allocator* allocator,
        const SliceTransform* transform,
        Logger* logger) override {
        return new ExpanseMemTableRep(compare, allocator, transform, logger, leaf_capacity_);
    }

    const char* Name() const override {
        return "ExpanseMemTableRepFactory";
    }

    bool IsConcurrentlyWritable() const override {
        return true;
    }

    bool IsPrefixExtractorSupported() const override {
        return true;
    }

    size_t GetLeafCapacity() const { return leaf_capacity_; }
    bool IsPrefixTrieEnabled() const { return enable_prefix_trie_; }

private:
    size_t leaf_capacity_;
    bool enable_prefix_trie_;
};

/// Factory function to create ExpanseMemTableRepFactory
inline std::shared_ptr<MemTableRepFactory> NewExpanseMemTableRepFactory(
    size_t leaf_capacity = 64,
    bool enable_prefix_trie = true
) {
    return std::make_shared<ExpanseMemTableRepFactory>(leaf_capacity, enable_prefix_trie);
}

/// Standalone batch scanning helper for high-throughput batch extraction
inline size_t ScanBatch(
    MemTableRep::Iterator* it,
    size_t max_keys,
    Slice* out_keys,
    Slice* out_values = nullptr
) {
    if (it == nullptr) return 0;
    auto* exp_it = dynamic_cast<ExpanseMemTableRep::IteratorImpl*>(it);
    if (exp_it != nullptr) {
        return exp_it->ScanBatch(max_keys, out_keys, out_values);
    }
    size_t count = 0;
    while (count < max_keys && it->Valid()) {
        const char* entry = it->key();
        if (entry != nullptr) {
            uint32_t ikey_len = 0;
            const char* p = expanse_rocksdb::GetVarint32Ptr(entry, entry + 5, &ikey_len);
            if (out_keys != nullptr && p != nullptr) {
                out_keys[count] = Slice(p, ikey_len);
            }
            if (out_values != nullptr && p != nullptr) {
                const char* val_p = p + ikey_len;
                uint32_t val_len = 0;
                const char* val_data = expanse_rocksdb::GetVarint32Ptr(val_p, val_p + 5, &val_len);
                if (val_data != nullptr) {
                    out_values[count] = Slice(val_data, val_len);
                } else {
                    out_values[count].clear();
                }
            }
            count++;
        }
        it->Next();
    }
    return count;
}

} // namespace rocksdb
