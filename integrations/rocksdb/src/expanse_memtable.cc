// Copyright (c) 2026 Expanse Authors. All rights reserved.
// Use of this source code is governed by an MIT/Apache-2.0 style license.
//
// expanse_memtable.cc — Implementation of RocksDB Pluggable MemTable backed by Expanse.

#include "expanse_memtable.h"

namespace rocksdb {

#if !defined(ROCKSDB_AVAILABLE) && !(defined(__has_include) && __has_include(<rocksdb/memtablerep.h>))

LookupKey::LookupKey(const Slice& user_key, SequenceNumber sequence) {
    size_t usize = user_key.size();
    size_t needed = usize + 13;
    char* dst;
    if (needed <= sizeof(space_)) {
        dst = space_;
    } else {
        dst = new char[needed];
    }
    start_ = dst;
    char* p = expanse_rocksdb::EncodeVarint32(dst, static_cast<uint32_t>(usize + 8));
    kstart_ = p;
    memcpy(p, user_key.data(), usize);
    p += usize;
    uint64_t trailer = (sequence << 8) | kTypeValue;
    for (int i = 0; i < 8; ++i) {
        p[i] = static_cast<char>((trailer >> (i * 8)) & 0xff);
    }
    p += 8;
    end_ = p;
}

LookupKey::~LookupKey() {
    if (start_ != space_) {
        delete[] start_;
    }
}

int MemTableRep::KeyComparator::operator()(const Slice& key1, const char* prefix_len_key2) const {
    Slice key2 = expanse_rocksdb::GetLengthPrefixedSlice(prefix_len_key2);
    return expanse_rocksdb::CompareInternalKeys(key1, key2);
}

#endif // ROCKSDB_AVAILABLE

// ============================================================================
// ExpanseMemTableRep Implementation
// ============================================================================

ExpanseMemTableRep::ExpanseMemTableRep(
    const MemTableRep::KeyComparator& compare,
    Allocator* allocator,
    const SliceTransform* transform,
    Logger* logger,
    size_t leaf_capacity
) : MemTableRep(allocator),
    compare_(compare),
    transform_(transform),
    logger_(logger),
    leaf_capacity_(leaf_capacity > 0 ? std::min(leaf_capacity, LeafBlock::kMaxCapacity) : LeafBlock::kMaxCapacity),
    trie_index_(expanse_map_new()),
    prefix_map_(expanse_bytesmap_new())
{
    (void)logger_;
    if (!allocator_) {
        own_arena_ = std::make_unique<Arena>(4096);
        allocator_ = own_arena_.get();
    }
    LeafBlock* root = new LeafBlock();
    head_.store(root, std::memory_order_release);
    tail_.store(root, std::memory_order_release);
    total_allocated_bytes_.fetch_add(sizeof(LeafBlock), std::memory_order_relaxed);
}

ExpanseMemTableRep::~ExpanseMemTableRep() {
    LeafBlock* curr = head_.load(std::memory_order_relaxed);
    while (curr != nullptr) {
        LeafBlock* next = curr->next_leaf.load(std::memory_order_relaxed);
        delete curr;
        curr = next;
    }
    if (trie_index_) {
        expanse_map_free(trie_index_);
        trie_index_ = nullptr;
    }
    if (prefix_map_) {
        expanse_bytesmap_free(prefix_map_);
        prefix_map_ = nullptr;
    }
}

ExpanseMemTableRep::LeafBlock* ExpanseMemTableRep::FindLeafBlockForInsert(const char* entry) {
    LeafBlock* h = head_.load(std::memory_order_relaxed);
    LeafBlock* t = tail_.load(std::memory_order_relaxed);
    if (!h || h == t) {
        return h;
    }

    uint64_t prefix = expanse_rocksdb::ExtractKeyPrefix64(entry);
    uint64_t out_k = 0;
    uint64_t out_v = 0;

    LeafBlock* candidate = h;
    if (expanse_map_prev_at_or_before(trie_index_, prefix, &out_k, &out_v)) {
        if (out_v != 0) {
            candidate = reinterpret_cast<LeafBlock*>(static_cast<uintptr_t>(out_v));
        }
    }

    // Step backward via prev_leaf if candidate is positioned after entry
    while (candidate->prev_leaf.load(std::memory_order_relaxed) != nullptr &&
           candidate->min_key() != nullptr &&
           compare_(candidate->min_key(), entry) > 0) {
        candidate = candidate->prev_leaf.load(std::memory_order_relaxed);
    }

    // Step forward via next_leaf to find the right leaf block
    while (candidate->next_leaf.load(std::memory_order_relaxed) != nullptr) {
        LeafBlock* nxt = candidate->next_leaf.load(std::memory_order_relaxed);
        if (nxt->count.load(std::memory_order_relaxed) > 0 &&
            nxt->min_key() != nullptr &&
            compare_(nxt->min_key(), entry) <= 0) {
            candidate = nxt;
        } else {
            break;
        }
    }
    return candidate;
}

const ExpanseMemTableRep::LeafBlock* ExpanseMemTableRep::FindLeafBlockForSeek(
    const Slice& internal_key,
    const char* memtable_key
) const {
    std::lock_guard<std::mutex> lock(mutex_);
    const LeafBlock* h = head_.load(std::memory_order_acquire);
    const LeafBlock* t = tail_.load(std::memory_order_acquire);
    if (!h || h == t) {
        return h;
    }

    uint64_t prefix = (memtable_key != nullptr)
        ? expanse_rocksdb::ExtractKeyPrefix64(memtable_key)
        : expanse_rocksdb::ExtractSlicePrefix64(internal_key);

    uint64_t out_k = 0;
    uint64_t out_v = 0;
    const LeafBlock* candidate = h;

    if (expanse_map_prev_at_or_before(trie_index_, prefix, &out_k, &out_v)) {
        if (out_v != 0) {
            candidate = reinterpret_cast<const LeafBlock*>(static_cast<uintptr_t>(out_v));
        }
    }

    // Step backward via prev_leaf if candidate is positioned after search target
    while (candidate->prev_leaf.load(std::memory_order_relaxed) != nullptr &&
           candidate->min_key() != nullptr) {
        bool is_after = (memtable_key != nullptr)
            ? (compare_(candidate->min_key(), memtable_key) > 0)
            : (compare_(internal_key, candidate->min_key()) < 0);
        if (is_after) {
            candidate = candidate->prev_leaf.load(std::memory_order_relaxed);
        } else {
            break;
        }
    }

    // Step forward via next_leaf
    while (candidate->next_leaf.load(std::memory_order_relaxed) != nullptr) {
        const LeafBlock* nxt = candidate->next_leaf.load(std::memory_order_relaxed);
        if (nxt->count.load(std::memory_order_relaxed) == 0) {
            candidate = nxt;
            continue;
        }
        if (nxt->min_key() == nullptr) {
            break;
        }
        if (memtable_key != nullptr) {
            if (compare_(nxt->min_key(), memtable_key) <= 0) {
                candidate = nxt;
            } else {
                break;
            }
        } else {
            if (compare_(internal_key, nxt->min_key()) > 0) {
                candidate = nxt;
            } else {
                break;
            }
        }
    }
    return candidate;
}

void ExpanseMemTableRep::SplitLeafBlock(LeafBlock* block) {
    LeafBlock* new_block = new LeafBlock();
    total_allocated_bytes_.fetch_add(sizeof(LeafBlock), std::memory_order_relaxed);

    uint32_t b_count = block->count.load(std::memory_order_relaxed);
    size_t mid = b_count / 2;
    size_t move_count = b_count - mid;

    block->version.fetch_add(1, std::memory_order_acquire);

    for (size_t i = 0; i < move_count; ++i) {
        new_block->entries[i].store(block->entries[mid + i].load(std::memory_order_relaxed), std::memory_order_relaxed);
    }
    new_block->count.store(static_cast<uint32_t>(move_count), std::memory_order_release);
    block->count.store(static_cast<uint32_t>(mid), std::memory_order_release);
    for (size_t i = 0; i < move_count; ++i) {
        block->entries[mid + i].store(nullptr, std::memory_order_relaxed);
    }

    LeafBlock* old_next = block->next_leaf.load(std::memory_order_relaxed);
    new_block->next.store(old_next, std::memory_order_relaxed);
    new_block->next_leaf.store(old_next, std::memory_order_relaxed);
    new_block->prev.store(block, std::memory_order_relaxed);
    new_block->prev_leaf.store(block, std::memory_order_relaxed);

    if (old_next != nullptr) {
        old_next->prev.store(new_block, std::memory_order_release);
        old_next->prev_leaf.store(new_block, std::memory_order_release);
    } else {
        tail_.store(new_block, std::memory_order_release);
    }
    block->next.store(new_block, std::memory_order_release);
    block->next_leaf.store(new_block, std::memory_order_release);

    if (new_block->count.load(std::memory_order_relaxed) > 0) {
        const char* first_entry = new_block->entries[0].load(std::memory_order_relaxed);
        uint64_t pfx = expanse_rocksdb::ExtractKeyPrefix64(first_entry);
        expanse_map_insert(trie_index_, pfx, reinterpret_cast<uintptr_t>(new_block), nullptr);
    }

    block->version.fetch_add(1, std::memory_order_release);
}

void ExpanseMemTableRep::Insert(KeyHandle handle) {
    const char* entry = static_cast<const char*>(handle);
    std::lock_guard<std::mutex> lock(mutex_);

    LeafBlock* block = FindLeafBlockForInsert(entry);
    if (!block) {
        block = head_.load(std::memory_order_relaxed);
    }

    uint32_t b_count = block->count.load(std::memory_order_relaxed);
    int left = 0;
    int right = static_cast<int>(b_count);
    while (left < right) {
        int mid = left + (right - left) / 2;
        int cmp = compare_(entry, block->entries[mid].load(std::memory_order_relaxed));
        if (cmp > 0) {
            left = mid + 1;
        } else {
            right = mid;
        }
    }

    block->version.fetch_add(1, std::memory_order_acquire);
    
    for (int i = static_cast<int>(b_count); i > left; --i) {
        block->entries[i].store(block->entries[i - 1].load(std::memory_order_relaxed), std::memory_order_relaxed);
    }
    block->entries[left].store(entry, std::memory_order_release);
    block->count.store(b_count + 1, std::memory_order_release);
    
    block->version.fetch_add(1, std::memory_order_release);
    total_keys_.fetch_add(1, std::memory_order_relaxed);

    if (left == 0) {
        uint64_t pfx = expanse_rocksdb::ExtractKeyPrefix64(entry);
        expanse_map_insert(trie_index_, pfx, reinterpret_cast<uintptr_t>(block), nullptr);
    }

    if (transform_ != nullptr) {
        Slice ikey = expanse_rocksdb::GetLengthPrefixedSlice(entry);
        if (ikey.size() >= 8) {
            Slice ukey(ikey.data(), ikey.size() - 8);
            if (transform_->InDomain(ukey)) {
                Slice pfx = transform_->Transform(ukey);
                expanse_bytesmap_insert(
                    prefix_map_,
                    pfx.data(),
                    pfx.size(),
                    reinterpret_cast<uintptr_t>(block),
                    nullptr
                );
            }
        }
    }

    if (block->count.load(std::memory_order_relaxed) >= leaf_capacity_) {
        SplitLeafBlock(block);
    }
}

void ExpanseMemTableRep::InsertConcurrently(KeyHandle handle) {
    Insert(handle);
}

bool ExpanseMemTableRep::Contains(const char* key) const {
    const LeafBlock* block = FindLeafBlockForSeek(Slice(), key);
    while (block != nullptr) {
        bool match = false;
        bool retry = false;
        while (true) {
            uint32_t v_start = block->version.load(std::memory_order_acquire);
            if (v_start & 1) {
                std::this_thread::yield();
                continue;
            }
            
            match = false;
            retry = false;
            int left = 0;
            int right = static_cast<int>(block->count.load(std::memory_order_acquire));
            while (left < right) {
                int mid = left + (right - left) / 2;
                const char* mid_entry = block->entries[mid].load(std::memory_order_relaxed);
                if (mid_entry == nullptr) {
                    retry = true;
                    break;
                }
                int cmp = compare_(key, mid_entry);
                if (cmp == 0) {
                    match = true;
                    break;
                } else if (cmp > 0) {
                    left = mid + 1;
                } else {
                    right = mid;
                }
            }
            if (retry) continue;

            uint32_t v_end = block->version.load(std::memory_order_acquire);
            if (v_start == v_end) {
                break;
            }
        }
        if (match) return true;
        
        if (block->count.load(std::memory_order_acquire) > 0) {
            const char* mx = block->max_key();
            if (mx != nullptr && compare_(key, mx) < 0) {
                break;
            }
        }
        block = block->next_leaf.load(std::memory_order_acquire);
    }
    return false;
}

void ExpanseMemTableRep::MarkReadOnly() {
    // MemTable marked immutable for flush
}

size_t ExpanseMemTableRep::ApproximateMemoryUsage() {
    std::lock_guard<std::mutex> lock(mutex_);
    size_t trie_bytes = trie_index_ ? expanse_map_mem_used(trie_index_) : 0;
    size_t pfx_bytes = prefix_map_ ? expanse_bytesmap_mem_used(prefix_map_) : 0;
    size_t leaf_bytes = total_allocated_bytes_.load(std::memory_order_relaxed);
    size_t arena_bytes = own_arena_ ? own_arena_->ApproximateMemoryUsage() : 0;
    return sizeof(ExpanseMemTableRep) + trie_bytes + pfx_bytes + leaf_bytes + arena_bytes;
}

void ExpanseMemTableRep::Get(
    const LookupKey& k,
    void* callback_args,
    bool (*callback_func)(void* arg, const char* entry)
) {
    Slice user_key = k.user_key();
    Slice internal_key = k.internal_key();
    const char* memtable_key = k.memtable_key().data();

    const LeafBlock* block = FindLeafBlockForSeek(internal_key, memtable_key);
    
    while (block != nullptr) {
        bool retry_block = false;
        bool out_of_bounds = false;
        const char* matches[32];
        int num_matches = 0;
        
        while (true) {
            uint32_t v_start = block->version.load(std::memory_order_acquire);
            if (v_start & 1) {
                std::this_thread::yield();
                continue;
            }
            
            retry_block = false;
            out_of_bounds = false;
            num_matches = 0;
            
            int left = 0;
            int right = static_cast<int>(block->count.load(std::memory_order_acquire));
            int count = right;
            
            while (left < right) {
                int mid = left + (right - left) / 2;
                const char* mid_entry = block->entries[mid].load(std::memory_order_relaxed);
                if (mid_entry == nullptr) {
                    retry_block = true;
                    break;
                }
                int cmp = compare_(mid_entry, memtable_key);
                if (cmp >= 0) {
                    right = mid;
                } else {
                    left = mid + 1;
                }
            }
            if (retry_block) continue;

            for (int i = left; i < count; ++i) {
                const char* entry = block->entries[i].load(std::memory_order_relaxed);
                if (entry == nullptr) {
                    retry_block = true;
                    break;
                }
                Slice entry_ikey = expanse_rocksdb::GetLengthPrefixedSlice(entry);
                if (entry_ikey.size() < 8) {
                    out_of_bounds = true;
                    break;
                }
                Slice entry_ukey(entry_ikey.data(), entry_ikey.size() - 8);
                if (entry_ukey != user_key) {
                    out_of_bounds = true;
                    break;
                }
                if (num_matches < 32) {
                    matches[num_matches++] = entry;
                }
            }
            if (retry_block) continue;

            uint32_t v_end = block->version.load(std::memory_order_acquire);
            if (v_start == v_end) {
                break;
            }
        }
        
        for (int i = 0; i < num_matches; ++i) {
            if (!callback_func(callback_args, matches[i])) {
                return;
            }
        }
        
        if (out_of_bounds) {
            return;
        }
        
        block = block->next_leaf.load(std::memory_order_acquire);
    }
}

MemTableRep::Iterator* ExpanseMemTableRep::GetIterator(Arena* arena, bool is_reverse) {
    (void)is_reverse;
    if (arena != nullptr) {
        void* mem = arena->AllocateAligned(sizeof(IteratorImpl));
        return new (mem) IteratorImpl(this);
    }
    return new IteratorImpl(this);
}

MemTableRep::Iterator* ExpanseMemTableRep::GetDynamicPrefixIterator(Arena* arena) {
    return GetIterator(arena);
}

MemTableRep::Iterator* ExpanseMemTableRep::GetPrefixIterator(
    const Slice& prefix,
    Arena* arena,
    bool is_reverse
) {
    (void)prefix;
    return GetIterator(arena, is_reverse);
}

void ExpanseMemTableRep::SuggestCompactRange(Slice* begin, Slice* end) {
    std::lock_guard<std::mutex> lock(mutex_);
    LeafBlock* h = head_.load(std::memory_order_relaxed);
    LeafBlock* t = tail_.load(std::memory_order_relaxed);
    if (h && h->count.load(std::memory_order_relaxed) > 0 && begin) {
        *begin = expanse_rocksdb::GetLengthPrefixedSlice(h->entries[0].load(std::memory_order_relaxed));
    }
    if (t && t->count.load(std::memory_order_relaxed) > 0 && end) {
        uint32_t tc = t->count.load(std::memory_order_relaxed);
        *end = expanse_rocksdb::GetLengthPrefixedSlice(t->entries[tc - 1].load(std::memory_order_relaxed));
    }
}

// ============================================================================
// ExpanseMemTableIterator Implementation
// ============================================================================

ExpanseMemTableRep::IteratorImpl::IteratorImpl(const ExpanseMemTableRep* rep)
    : rep_(rep), current_leaf_(nullptr), current_slot_(-1), valid_(false) {}

void ExpanseMemTableRep::IteratorImpl::EnsureKeyCached() const {
    if (cached_key_.valid) return;
    const char* entry = key();
    cached_key_.raw_entry = entry;
    if (entry == nullptr) {
        cached_key_.internal_key.clear();
        cached_key_.user_key.clear();
        cached_key_.value.clear();
        cached_key_.valid = true;
        return;
    }
    uint32_t ikey_len = 0;
    const char* p = expanse_rocksdb::GetVarint32Ptr(entry, entry + 5, &ikey_len);
    if (p != nullptr) {
        cached_key_.internal_key = Slice(p, ikey_len);
        if (ikey_len >= 8) {
            cached_key_.user_key = Slice(p, ikey_len - 8);
        } else {
            cached_key_.user_key = cached_key_.internal_key;
        }
        const char* val_p = p + ikey_len;
        uint32_t val_len = 0;
        const char* val_data = expanse_rocksdb::GetVarint32Ptr(val_p, val_p + 5, &val_len);
        if (val_data != nullptr) {
            cached_key_.value = Slice(val_data, val_len);
        } else {
            cached_key_.value.clear();
        }
    }
    cached_key_.valid = true;
}

Slice ExpanseMemTableRep::IteratorImpl::internal_key() const {
    EnsureKeyCached();
    return cached_key_.internal_key;
}

Slice ExpanseMemTableRep::IteratorImpl::user_key() const {
    EnsureKeyCached();
    return cached_key_.user_key;
}

Slice ExpanseMemTableRep::IteratorImpl::value() const {
    EnsureKeyCached();
    return cached_key_.value;
}

bool ExpanseMemTableRep::IteratorImpl::Valid() const {
    if (!valid_ || current_leaf_ == nullptr || current_slot_ < 0) return false;
    return current_slot_ < static_cast<int>(current_leaf_->count.load(std::memory_order_acquire));
}

const char* ExpanseMemTableRep::IteratorImpl::key() const {
    if (!valid_ || current_leaf_ == nullptr || current_slot_ < 0 ||
        current_slot_ >= static_cast<int>(current_leaf_->count.load(std::memory_order_acquire))) {
        return nullptr;
    }
    return current_leaf_->entries[current_slot_].load(std::memory_order_relaxed);
}

void ExpanseMemTableRep::IteratorImpl::Next() {
    if (!valid_ || current_leaf_ == nullptr || current_slot_ < 0) return;
    InvalidateCache();
    current_slot_++;
    uint32_t count = current_leaf_->count.load(std::memory_order_acquire);

    // Software SIMD prefetch hint for sibling leaf block when processing latter entries
    if (current_slot_ + 4 >= static_cast<int>(count)) {
        LeafBlock* nxt = current_leaf_->next_leaf.load(std::memory_order_relaxed);
        if (nxt != nullptr) {
            expanse_rocksdb::Prefetch<0, 3>(nxt);
            expanse_rocksdb::Prefetch<0, 3>(nxt->entries);
        }
    } else if (current_slot_ + 2 < static_cast<int>(count)) {
        const char* future_entry = current_leaf_->entries[current_slot_ + 2].load(std::memory_order_relaxed);
        if (future_entry != nullptr) {
            expanse_rocksdb::Prefetch<0, 1>(future_entry);
        }
    }

    if (current_slot_ >= static_cast<int>(count)) {
        // Advance directly via next_leaf intrusive pointer without re-seeking trie!
        current_leaf_ = current_leaf_->next_leaf.load(std::memory_order_acquire);
        while (current_leaf_ != nullptr && current_leaf_->count.load(std::memory_order_acquire) == 0) {
            current_leaf_ = current_leaf_->next_leaf.load(std::memory_order_acquire);
        }
        if (current_leaf_ != nullptr && current_leaf_->count.load(std::memory_order_acquire) > 0) {
            current_slot_ = 0;
            valid_ = true;
            LeafBlock* nxt_nxt = current_leaf_->next_leaf.load(std::memory_order_relaxed);
            if (nxt_nxt != nullptr) {
                expanse_rocksdb::Prefetch<0, 3>(nxt_nxt);
            }
        } else {
            current_slot_ = -1;
            valid_ = false;
        }
    }
}

void ExpanseMemTableRep::IteratorImpl::Prev() {
    if (!valid_ || current_leaf_ == nullptr || current_slot_ < 0) return;
    InvalidateCache();
    current_slot_--;

    // Prefetch prev sibling leaf when approaching beginning of leaf
    if (current_slot_ < 4) {
        LeafBlock* prv = current_leaf_->prev_leaf.load(std::memory_order_relaxed);
        if (prv != nullptr) {
            expanse_rocksdb::Prefetch<0, 3>(prv);
            expanse_rocksdb::Prefetch<0, 3>(prv->entries);
        }
    }

    if (current_slot_ < 0) {
        // Advance backwards via prev_leaf intrusive pointer!
        current_leaf_ = current_leaf_->prev_leaf.load(std::memory_order_acquire);
        while (current_leaf_ != nullptr && current_leaf_->count.load(std::memory_order_acquire) == 0) {
            current_leaf_ = current_leaf_->prev_leaf.load(std::memory_order_acquire);
        }
        if (current_leaf_ != nullptr && current_leaf_->count.load(std::memory_order_acquire) > 0) {
            current_slot_ = static_cast<int>(current_leaf_->count.load(std::memory_order_acquire)) - 1;
            valid_ = true;
        } else {
            current_slot_ = -1;
            valid_ = false;
        }
    }
}

void ExpanseMemTableRep::IteratorImpl::SeekToFirst() {
    InvalidateCache();
    current_leaf_ = rep_->head_.load(std::memory_order_acquire);
    while (current_leaf_ != nullptr && current_leaf_->count.load(std::memory_order_acquire) == 0) {
        current_leaf_ = current_leaf_->next_leaf.load(std::memory_order_acquire);
    }
    if (current_leaf_ != nullptr && current_leaf_->count.load(std::memory_order_acquire) > 0) {
        current_slot_ = 0;
        valid_ = true;
        const char* entry0 = current_leaf_->entries[0].load(std::memory_order_relaxed);
        if (entry0) expanse_rocksdb::Prefetch<0, 1>(entry0);
        LeafBlock* nxt = current_leaf_->next_leaf.load(std::memory_order_relaxed);
        if (nxt) expanse_rocksdb::Prefetch<0, 3>(nxt);
    } else {
        current_slot_ = -1;
        valid_ = false;
    }
}

void ExpanseMemTableRep::IteratorImpl::SeekToLast() {
    InvalidateCache();
    current_leaf_ = rep_->tail_.load(std::memory_order_acquire);
    while (current_leaf_ != nullptr && current_leaf_->count.load(std::memory_order_acquire) == 0) {
        current_leaf_ = current_leaf_->prev_leaf.load(std::memory_order_acquire);
    }
    if (current_leaf_ != nullptr && current_leaf_->count.load(std::memory_order_acquire) > 0) {
        current_slot_ = static_cast<int>(current_leaf_->count.load(std::memory_order_acquire)) - 1;
        valid_ = true;
    } else {
        current_slot_ = -1;
        valid_ = false;
    }
}

void ExpanseMemTableRep::IteratorImpl::Seek(const Slice& internal_key, const char* memtable_key) {
    InvalidateCache();
    const LeafBlock* block = rep_->FindLeafBlockForSeek(internal_key, memtable_key);
    while (block != nullptr) {
        int left = 0;
        int right = 0;
        bool found = false;
        while (true) {
            uint32_t v_start = block->version.load(std::memory_order_acquire);
            if (v_start & 1) {
                std::this_thread::yield();
                continue;
            }

            left = 0;
            right = static_cast<int>(block->count.load(std::memory_order_acquire));
            int orig_right = right;
            bool retry = false;

            while (left < right) {
                int mid = left + (right - left) / 2;
                const char* mid_entry = block->entries[mid].load(std::memory_order_relaxed);
                if (mid_entry == nullptr) {
                    retry = true;
                    break;
                }
                int cmp = (memtable_key != nullptr)
                    ? rep_->compare_(mid_entry, memtable_key)
                    : rep_->compare_(internal_key, mid_entry);
                if (cmp >= 0) {
                    right = mid;
                } else {
                    left = mid + 1;
                }
            }
            if (retry) continue;

            uint32_t v_end = block->version.load(std::memory_order_acquire);
            if (v_start == v_end) {
                if (left < orig_right) {
                    found = true;
                }
                break;
            }
        }
        
        if (found) {
            current_leaf_ = block;
            current_slot_ = left;
            valid_ = true;
            return;
        }
        block = block->next_leaf.load(std::memory_order_acquire);
    }
    current_leaf_ = nullptr;
    current_slot_ = -1;
    valid_ = false;
}

void ExpanseMemTableRep::IteratorImpl::SeekForPrev(const Slice& internal_key, const char* memtable_key) {
    Seek(internal_key, memtable_key);
    if (Valid()) {
        int cmp = (memtable_key != nullptr)
            ? rep_->compare_(key(), memtable_key)
            : rep_->compare_(internal_key, key());
        if (cmp != 0) {
            Prev();
        }
    } else {
        SeekToLast();
    }
}

size_t ExpanseMemTableRep::IteratorImpl::ScanBatch(
    size_t max_keys,
    Slice* out_keys,
    Slice* out_values
) {
    if (!valid_ || current_leaf_ == nullptr || current_slot_ < 0 || max_keys == 0) {
        return 0;
    }

    InvalidateCache();
    size_t extracted = 0;

    while (extracted < max_keys && current_leaf_ != nullptr) {
        uint32_t count = current_leaf_->count.load(std::memory_order_acquire);
        if (current_slot_ >= static_cast<int>(count)) {
            current_leaf_ = current_leaf_->next_leaf.load(std::memory_order_acquire);
            while (current_leaf_ != nullptr && current_leaf_->count.load(std::memory_order_acquire) == 0) {
                current_leaf_ = current_leaf_->next_leaf.load(std::memory_order_acquire);
            }
            if (current_leaf_ == nullptr) {
                current_slot_ = -1;
                valid_ = false;
                break;
            }
            current_slot_ = 0;
            count = current_leaf_->count.load(std::memory_order_acquire);
        }

        size_t available = count - current_slot_;
        size_t to_extract = std::min(available, max_keys - extracted);

        // Issue prefetch hint for next leaf block when scanning through current block
        LeafBlock* nxt = current_leaf_->next_leaf.load(std::memory_order_relaxed);
        if (nxt != nullptr) {
            expanse_rocksdb::Prefetch<0, 3>(nxt);
            expanse_rocksdb::Prefetch<0, 3>(nxt->entries);
        }

        for (size_t i = 0; i < to_extract; ++i) {
            const char* entry = current_leaf_->entries[current_slot_ + i].load(std::memory_order_relaxed);
            if (entry != nullptr) {
                // Prefetch entry payload 2 slots ahead
                if (i + 2 < to_extract) {
                    const char* ahead = current_leaf_->entries[current_slot_ + i + 2].load(std::memory_order_relaxed);
                    if (ahead) expanse_rocksdb::Prefetch<0, 1>(ahead);
                }

                uint32_t ikey_len = 0;
                const char* p = expanse_rocksdb::GetVarint32Ptr(entry, entry + 5, &ikey_len);
                if (out_keys != nullptr && p != nullptr) {
                    out_keys[extracted] = Slice(p, ikey_len);
                }
                if (out_values != nullptr && p != nullptr) {
                    const char* val_p = p + ikey_len;
                    uint32_t val_len = 0;
                    const char* val_data = expanse_rocksdb::GetVarint32Ptr(val_p, val_p + 5, &val_len);
                    if (val_data != nullptr) {
                        out_values[extracted] = Slice(val_data, val_len);
                    } else {
                        out_values[extracted].clear();
                    }
                }
            }
            extracted++;
        }

        current_slot_ += to_extract;
        if (current_slot_ >= static_cast<int>(count)) {
            current_leaf_ = current_leaf_->next_leaf.load(std::memory_order_acquire);
            while (current_leaf_ != nullptr && current_leaf_->count.load(std::memory_order_acquire) == 0) {
                current_leaf_ = current_leaf_->next_leaf.load(std::memory_order_acquire);
            }
            if (current_leaf_ != nullptr) {
                current_slot_ = 0;
                valid_ = true;
            } else {
                current_slot_ = -1;
                valid_ = false;
                break;
            }
        } else {
            valid_ = true;
        }
    }

    return extracted;
}

} // namespace rocksdb
