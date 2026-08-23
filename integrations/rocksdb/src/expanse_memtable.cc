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
    head_ = new LeafBlock();
    tail_ = head_;
    total_allocated_bytes_.fetch_add(sizeof(LeafBlock), std::memory_order_relaxed);
}

ExpanseMemTableRep::~ExpanseMemTableRep() {
    LeafBlock* curr = head_;
    while (curr != nullptr) {
        LeafBlock* next = curr->next;
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
    if (!head_ || head_ == tail_) {
        return head_;
    }

    uint64_t prefix = expanse_rocksdb::ExtractKeyPrefix64(entry);
    uint64_t out_k = 0;
    uint64_t out_v = 0;

    LeafBlock* candidate = head_;
    if (expanse_map_prev_at_or_before(trie_index_, prefix, &out_k, &out_v)) {
        if (out_v != 0) {
            candidate = reinterpret_cast<LeafBlock*>(static_cast<uintptr_t>(out_v));
        }
    }

    while (candidate->next != nullptr && candidate->next->count > 0 &&
           compare_(candidate->next->min_key(), entry) <= 0) {
        candidate = candidate->next;
    }
    return candidate;
}

const ExpanseMemTableRep::LeafBlock* ExpanseMemTableRep::FindLeafBlockForSeek(
    const Slice& internal_key,
    const char* memtable_key
) const {
    if (!head_ || head_ == tail_) {
        return head_;
    }

    uint64_t prefix = (memtable_key != nullptr)
        ? expanse_rocksdb::ExtractKeyPrefix64(memtable_key)
        : expanse_rocksdb::ExtractSlicePrefix64(internal_key);

    uint64_t out_k = 0;
    uint64_t out_v = 0;
    const LeafBlock* candidate = head_;

    if (expanse_map_prev_at_or_before(trie_index_, prefix, &out_k, &out_v)) {
        if (out_v != 0) {
            candidate = reinterpret_cast<const LeafBlock*>(static_cast<uintptr_t>(out_v));
        }
    }

    while (candidate->next != nullptr && candidate->next->count > 0) {
        if (memtable_key != nullptr) {
            if (compare_(candidate->next->min_key(), memtable_key) <= 0) {
                candidate = candidate->next;
            } else {
                break;
            }
        } else {
            if (compare_(internal_key, candidate->next->min_key()) > 0) {
                candidate = candidate->next;
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

    size_t mid = block->count / 2;
    size_t move_count = block->count - mid;

    for (size_t i = 0; i < move_count; ++i) {
        new_block->entries[i] = block->entries[mid + i];
        block->entries[mid + i] = nullptr;
    }
    new_block->count = static_cast<uint32_t>(move_count);
    block->count = static_cast<uint32_t>(mid);

    new_block->next = block->next;
    new_block->prev = block;
    if (block->next) {
        block->next->prev = new_block;
    } else {
        tail_ = new_block;
    }
    block->next = new_block;

    if (new_block->count > 0) {
        uint64_t pfx = expanse_rocksdb::ExtractKeyPrefix64(new_block->entries[0]);
        expanse_map_insert(trie_index_, pfx, reinterpret_cast<uintptr_t>(new_block), nullptr);
    }
}

void ExpanseMemTableRep::Insert(KeyHandle handle) {
    const char* entry = static_cast<const char*>(handle);
    std::lock_guard<std::mutex> lock(mutex_);

    LeafBlock* block = FindLeafBlockForInsert(entry);
    if (!block) {
        block = head_;
    }

    int left = 0;
    int right = static_cast<int>(block->count);
    while (left < right) {
        int mid = left + (right - left) / 2;
        int cmp = compare_(entry, block->entries[mid]);
        if (cmp > 0) {
            left = mid + 1;
        } else {
            right = mid;
        }
    }

    for (int i = static_cast<int>(block->count); i > left; --i) {
        block->entries[i] = block->entries[i - 1];
    }
    block->entries[left] = entry;
    block->count++;
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

    if (block->count >= leaf_capacity_) {
        SplitLeafBlock(block);
    }
}

void ExpanseMemTableRep::InsertConcurrently(KeyHandle handle) {
    Insert(handle);
}

bool ExpanseMemTableRep::Contains(const char* key) const {
    std::lock_guard<std::mutex> lock(mutex_);
    const LeafBlock* block = FindLeafBlockForSeek(Slice(), key);
    while (block != nullptr) {
        int left = 0;
        int right = static_cast<int>(block->count);
        while (left < right) {
            int mid = left + (right - left) / 2;
            int cmp = compare_(key, block->entries[mid]);
            if (cmp == 0) {
                return true;
            } else if (cmp > 0) {
                left = mid + 1;
            } else {
                right = mid;
            }
        }
        if (block->count > 0 && compare_(key, block->max_key()) < 0) {
            break;
        }
        block = block->next;
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
    auto it = std::make_unique<IteratorImpl>(this);
    it->Seek(k.internal_key(), k.memtable_key().data());
    Slice user_key = k.user_key();
    while (it->Valid()) {
        const char* entry = it->key();
        Slice entry_ikey = expanse_rocksdb::GetLengthPrefixedSlice(entry);
        if (entry_ikey.size() < 8) break;
        Slice entry_ukey(entry_ikey.data(), entry_ikey.size() - 8);
        if (entry_ukey != user_key) {
            break;
        }
        if (!callback_func(callback_args, entry)) {
            break;
        }
        it->Next();
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
    if (head_ && head_->count > 0 && begin) {
        *begin = expanse_rocksdb::GetLengthPrefixedSlice(head_->entries[0]);
    }
    if (tail_ && tail_->count > 0 && end) {
        *end = expanse_rocksdb::GetLengthPrefixedSlice(tail_->entries[tail_->count - 1]);
    }
}

// ============================================================================
// ExpanseMemTableIterator Implementation
// ============================================================================

ExpanseMemTableRep::IteratorImpl::IteratorImpl(const ExpanseMemTableRep* rep)
    : rep_(rep), curr_block_(nullptr), curr_idx_(-1), valid_(false) {}

bool ExpanseMemTableRep::IteratorImpl::Valid() const {
    if (!valid_ || curr_block_ == nullptr || curr_idx_ < 0) return false;
    std::lock_guard<std::mutex> lock(rep_->mutex_);
    return curr_idx_ < static_cast<int>(curr_block_->count);
}

const char* ExpanseMemTableRep::IteratorImpl::key() const {
    std::lock_guard<std::mutex> lock(rep_->mutex_);
    if (!valid_ || curr_block_ == nullptr || curr_idx_ < 0 ||
        curr_idx_ >= static_cast<int>(curr_block_->count)) {
        return nullptr;
    }
    return curr_block_->entries[curr_idx_];
}

void ExpanseMemTableRep::IteratorImpl::Next() {
    std::lock_guard<std::mutex> lock(rep_->mutex_);
    if (!valid_ || curr_block_ == nullptr || curr_idx_ < 0) return;
    curr_idx_++;
    if (curr_idx_ >= static_cast<int>(curr_block_->count)) {
        curr_block_ = curr_block_->next;
        while (curr_block_ != nullptr && curr_block_->count == 0) {
            curr_block_ = curr_block_->next;
        }
        if (curr_block_ != nullptr && curr_block_->count > 0) {
            curr_idx_ = 0;
            valid_ = true;
        } else {
            curr_idx_ = -1;
            valid_ = false;
        }
    }
}

void ExpanseMemTableRep::IteratorImpl::Prev() {
    std::lock_guard<std::mutex> lock(rep_->mutex_);
    if (!valid_ || curr_block_ == nullptr || curr_idx_ < 0) return;
    curr_idx_--;
    if (curr_idx_ < 0) {
        curr_block_ = curr_block_->prev;
        while (curr_block_ != nullptr && curr_block_->count == 0) {
            curr_block_ = curr_block_->prev;
        }
        if (curr_block_ != nullptr && curr_block_->count > 0) {
            curr_idx_ = static_cast<int>(curr_block_->count) - 1;
            valid_ = true;
        } else {
            curr_idx_ = -1;
            valid_ = false;
        }
    }
}

void ExpanseMemTableRep::IteratorImpl::SeekToFirst() {
    std::lock_guard<std::mutex> lock(rep_->mutex_);
    curr_block_ = rep_->head_;
    while (curr_block_ != nullptr && curr_block_->count == 0) {
        curr_block_ = curr_block_->next;
    }
    if (curr_block_ != nullptr && curr_block_->count > 0) {
        curr_idx_ = 0;
        valid_ = true;
    } else {
        curr_idx_ = -1;
        valid_ = false;
    }
}

void ExpanseMemTableRep::IteratorImpl::SeekToLast() {
    std::lock_guard<std::mutex> lock(rep_->mutex_);
    curr_block_ = rep_->tail_;
    while (curr_block_ != nullptr && curr_block_->count == 0) {
        curr_block_ = curr_block_->prev;
    }
    if (curr_block_ != nullptr && curr_block_->count > 0) {
        curr_idx_ = static_cast<int>(curr_block_->count) - 1;
        valid_ = true;
    } else {
        curr_idx_ = -1;
        valid_ = false;
    }
}

void ExpanseMemTableRep::IteratorImpl::Seek(const Slice& internal_key, const char* memtable_key) {
    std::lock_guard<std::mutex> lock(rep_->mutex_);
    const LeafBlock* block = rep_->FindLeafBlockForSeek(internal_key, memtable_key);
    while (block != nullptr) {
        int left = 0;
        int right = static_cast<int>(block->count);
        while (left < right) {
            int mid = left + (right - left) / 2;
            int cmp = (memtable_key != nullptr)
                ? rep_->compare_(block->entries[mid], memtable_key)
                : rep_->compare_(internal_key, block->entries[mid]);
            if (cmp >= 0) {
                right = mid;
            } else {
                left = mid + 1;
            }
        }
        if (left < static_cast<int>(block->count)) {
            curr_block_ = block;
            curr_idx_ = left;
            valid_ = true;
            return;
        }
        block = block->next;
    }
    curr_block_ = nullptr;
    curr_idx_ = -1;
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

} // namespace rocksdb
