// Copyright (c) 2026 Expanse Authors. All rights reserved.
// Use of this source code is governed by an MIT/Apache-2.0 style license.
//
// expanse_memtable.cc — Implementation of RocksDB Pluggable MemTable backed by Expanse.

#include <cassert>
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
    size_t leaf_capacity,
    SeekLockScope seek_lock_scope
) : MemTableRep(allocator),
    compare_(compare),
    transform_(transform),
    logger_(logger),
    leaf_capacity_(leaf_capacity > 0 ? std::min(leaf_capacity, LeafBlock::kMaxCapacity) : LeafBlock::kMaxCapacity),
    seek_lock_scope_(seek_lock_scope),
    trie_index_(expanse_map_new())
{
    (void)logger_;
    // Retained for the prefix-seek path that does not exist yet: the
    // MemTableRep interface hands it to us and GetPrefixIterator would
    // need it, so keep the member rather than change the constructor.
    (void)transform_;
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

static uint64_t SeekPrefix(const Slice& internal_key, const char* memtable_key) {
    return (memtable_key != nullptr)
        ? expanse_rocksdb::ExtractKeyPrefix64(memtable_key)
        : expanse_rocksdb::ExtractSlicePrefix64(internal_key);
}

const ExpanseMemTableRep::LeafBlock* ExpanseMemTableRep::FindLeafBlockForSeek(
    const Slice& internal_key,
    const char* memtable_key
) const {
    if (seek_lock_scope_ == SeekLockScope::kFullLocate) {
        std::lock_guard<std::mutex> lock(mutex_);
        const LeafBlock* h = head_.load(std::memory_order_acquire);
        const LeafBlock* t = tail_.load(std::memory_order_acquire);
        if (!h || h == t) {
            return h;
        }
        uint64_t out_k = 0;
        uint64_t out_v = 0;
        const LeafBlock* candidate = h;
        if (expanse_map_prev_at_or_before(trie_index_, SeekPrefix(internal_key, memtable_key), &out_k, &out_v)) {
            if (out_v != 0) {
                candidate = reinterpret_cast<const LeafBlock*>(static_cast<uintptr_t>(out_v));
            }
        }
        return SettleSeekCandidate(candidate, internal_key, memtable_key);
    }

    // kTrieCall: the prefix is computed before the lock and the walk runs
    // after it, so mutex_ covers only what expanse_map_t needs.
    const uint64_t prefix = SeekPrefix(internal_key, memtable_key);
    const LeafBlock* candidate = nullptr;
    {
        std::lock_guard<std::mutex> lock(mutex_);
        const LeafBlock* h = head_.load(std::memory_order_acquire);
        const LeafBlock* t = tail_.load(std::memory_order_acquire);
        if (!h || h == t) {
            return h;
        }
        uint64_t out_k = 0;
        uint64_t out_v = 0;
        candidate = h;
        if (expanse_map_prev_at_or_before(trie_index_, prefix, &out_k, &out_v)) {
            if (out_v != 0) {
                candidate = reinterpret_cast<const LeafBlock*>(static_cast<uintptr_t>(out_v));
            }
        }
    }
    return SettleSeekCandidate(candidate, internal_key, memtable_key);
}

// The leaf walk from the trie's candidate to the block a seek starts in.
//
// Under kFullLocate it runs with mutex_ held. Under kTrieCall it runs
// concurrently with Insert and SplitLeafBlock, and ends on a usable block
// because of four invariants of that writer path:
//
//   1. A LeafBlock is freed only by ~ExpanseMemTableRep, so every block
//      pointer loaded here stays valid for the rep's lifetime.
//   2. A block enters the prev_leaf/next_leaf chain in key order, after its
//      entries, count and own links are stored (SplitLeafBlock stores
//      block->next_leaf last), and never leaves it.
//   3. A block's min_key never increases. Insert replaces entries[0] only
//      with a smaller key, in one store after the shift, and SplitLeafBlock
//      moves the upper half out and keeps entries[0].
//   4. Entry pointers name write-once key bytes, and min_key() acquire-loads
//      both count and the entry.
//
// So the backward step stops on a block whose min_key is at or below the
// target, or on the head, and 3 keeps that true for the rest of the read.
// Every key below that block's min_key is in an earlier block, so a present
// target is reachable forward from it. A concurrent split can leave the walk
// on a block that is no longer the tightest, and Get, Contains and
// IteratorImpl::Seek each step forward from wherever it ends. The loads are
// acquire so each takes a happens-before edge from the writer's release
// store, the ordering mutex_ supplies under kFullLocate.
const ExpanseMemTableRep::LeafBlock* ExpanseMemTableRep::SettleSeekCandidate(
    const LeafBlock* candidate,
    const Slice& internal_key,
    const char* memtable_key
) const {
    // Step backward via prev_leaf if candidate is positioned after search target
    while (candidate->prev_leaf.load(std::memory_order_acquire) != nullptr &&
           candidate->min_key() != nullptr) {
        bool is_after = (memtable_key != nullptr)
            ? (compare_(candidate->min_key(), memtable_key) > 0)
            : (compare_(internal_key, candidate->min_key()) < 0);
        if (is_after) {
            candidate = candidate->prev_leaf.load(std::memory_order_acquire);
        } else {
            break;
        }
    }

    // Step forward via next_leaf
    while (candidate->next_leaf.load(std::memory_order_acquire) != nullptr) {
        const LeafBlock* nxt = candidate->next_leaf.load(std::memory_order_acquire);
        if (nxt->count.load(std::memory_order_acquire) == 0) {
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

    // Relaxed store plus a release fence, not an acquire RMW. Acquire on the
    // increment that opens the bracket orders nothing for the payload stores
    // that follow it; the fence is what guarantees a reader observing any
    // covered store must also observe the odd version. It also removes the
    // dependence on every covered store being individually a release store --
    // the nulling loop below is deliberately relaxed, and under the previous
    // spelling only the readers' ad-hoc null checks stood between that and a
    // bracket validating over a torn slot.
    //
    // There is one writer (mutex_ is held), so no read-modify-write is needed
    // to compute the next value. Boehm, "Can seqlocks get along with
    // programming language memory models?", MSPC 2012; this is the same
    // construction as SeqVersion::begin in crates/expanse/src/occ.rs.
    block->version.store(block->version.load(std::memory_order_relaxed) + 1,
                         std::memory_order_relaxed);
    std::atomic_thread_fence(std::memory_order_release);

    for (size_t i = 0; i < move_count; ++i) {
        // Release: a reader that acquire-loads this moved entry pointer must
        // gain a happens-before edge to the entry's (write-once) key bytes, which were
        // published under mutex_ by the original inserter.
        new_block->entries[i].store(block->entries[mid + i].load(std::memory_order_relaxed), std::memory_order_release);
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

    // Relaxed store plus a release fence, not an acquire RMW. Acquire on the
    // increment that opens the bracket orders nothing for the payload stores
    // that follow it; the fence is what guarantees a reader observing any
    // covered store must also observe the odd version. It also removes the
    // dependence on every covered store being individually a release store --
    // the nulling loop below is deliberately relaxed, and under the previous
    // spelling only the readers' ad-hoc null checks stood between that and a
    // bracket validating over a torn slot.
    //
    // There is one writer (mutex_ is held), so no read-modify-write is needed
    // to compute the next value. Boehm, "Can seqlocks get along with
    // programming language memory models?", MSPC 2012; this is the same
    // construction as SeqVersion::begin in crates/expanse/src/occ.rs.
    block->version.store(block->version.load(std::memory_order_relaxed) + 1,
                         std::memory_order_relaxed);
    std::atomic_thread_fence(std::memory_order_release);
    
    for (int i = static_cast<int>(b_count); i > left; --i) {
        // Release: shifting republishes an existing entry pointer into a new slot that
        // readers scan; the acquire-load on the reader side needs this release
        // to carry happens-before to that entry's key bytes.
        block->entries[i].store(block->entries[i - 1].load(std::memory_order_relaxed), std::memory_order_release);
    }
    block->entries[left].store(entry, std::memory_order_release);
    block->count.store(b_count + 1, std::memory_order_release);
    
    block->version.fetch_add(1, std::memory_order_release);
    total_keys_.fetch_add(1, std::memory_order_relaxed);

    if (left == 0) {
        uint64_t pfx = expanse_rocksdb::ExtractKeyPrefix64(entry);
        expanse_map_insert(trie_index_, pfx, reinterpret_cast<uintptr_t>(block), nullptr);
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
                // Acquire: synchronizes-with the release store that published this entry
                // pointer, establishing happens-before to the entry's key bytes before
                // the comparator dereferences them.
                const char* mid_entry = block->entries[mid].load(std::memory_order_acquire);
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

            // An acquire *fence* before a relaxed load, not an acquire load.
            // An acquire load orders what follows it; this re-read has to be
            // ordered after the bracket's payload reads, which is the opposite
            // direction. Mirrors SeqVersion::validate in
            // crates/expanse/src/occ.rs.
            std::atomic_thread_fence(std::memory_order_acquire);
            uint32_t v_end = block->version.load(std::memory_order_relaxed);
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

#ifdef EXPANSE_MEMTABLE_PARK_POINTS
size_t ExpanseMemTableRep::LeafBlockCountForTest() const {
    size_t n = 0;
    for (const LeafBlock* b = head_.load(std::memory_order_acquire); b != nullptr;
         b = b->next_leaf.load(std::memory_order_acquire)) {
        ++n;
    }
    return n;
}
#endif

void ExpanseMemTableRep::MarkReadOnly() {
    // MemTable marked immutable for flush
}

size_t ExpanseMemTableRep::ApproximateMemoryUsage() {
    std::lock_guard<std::mutex> lock(mutex_);
    size_t trie_bytes = trie_index_ ? expanse_map_mem_used(trie_index_) : 0;
    size_t leaf_bytes = total_allocated_bytes_.load(std::memory_order_relaxed);
    size_t arena_bytes = own_arena_ ? own_arena_->ApproximateMemoryUsage() : 0;
    return sizeof(ExpanseMemTableRep) + trie_bytes + leaf_bytes + arena_bytes;
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
        // Sized to the block, not to a guess. A block holds kMaxCapacity
        // entries and every one of them can carry the same user key with a
        // different sequence number, so a smaller buffer drops versions of a
        // heavily overwritten key -- and Get would then miss the sequence
        // number it was asked for and return a wrong answer rather than a slow
        // one. 512 bytes of stack is the cheaper side of that trade.
        const char* matches[LeafBlock::kMaxCapacity];
        size_t num_matches = 0;
        
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
                // Acquire: gain happens-before to this entry's key bytes (published via a
                // release store) before the comparator reads them.
                const char* mid_entry = block->entries[mid].load(std::memory_order_acquire);
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
                // Acquire: gain happens-before to this entry's key bytes before decoding them.
                const char* entry = block->entries[i].load(std::memory_order_acquire);
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
                // Cannot overflow: the loop is bounded by the block's own
                // count, which never exceeds kMaxCapacity. Asserted rather
                // than silently clamped, because a clamp here is a dropped
                // version and a wrong Get.
                assert(num_matches < LeafBlock::kMaxCapacity);
                matches[num_matches++] = entry;
            }
            if (retry_block) continue;

            // An acquire *fence* before a relaxed load, not an acquire load.
            // An acquire load orders what follows it; this re-read has to be
            // ordered after the bracket's payload reads, which is the opposite
            // direction. Mirrors SeqVersion::validate in
            // crates/expanse/src/occ.rs.
            std::atomic_thread_fence(std::memory_order_acquire);
            uint32_t v_end = block->version.load(std::memory_order_relaxed);
            if (v_start == v_end) {
                break;
            }
        }
        
        for (size_t i = 0; i < num_matches; ++i) {
            if (!callback_func(callback_args, matches[i])) {
                return;
            }
        }
        
        if (out_of_bounds) {
            return;
        }

        EXPANSE_MEMTABLE_PARK(kGetBeforeNextLeaf);
        if (num_matches > 0) {
            // The callbacks ran after this block validated and before its
            // next_leaf is loaded, and nothing holds mutex_ there under any
            // scope. A split in that window moves the delivered entries into
            // the successor, so scanning it from the lookup key again would
            // deliver them twice (METHODOLOGY section 5.16, G-O6). The rest of
            // the scan starts strictly after the last delivered entry instead.
            // It is a separate function so the loop above, which every Get
            // whose matches do not reach a block's end runs, is the loop it
            // was.
            GetAfterDelivered(block->next_leaf.load(std::memory_order_acquire), matches[num_matches - 1],
                              user_key, callback_args, callback_func);
            return;
        }
        block = block->next_leaf.load(std::memory_order_acquire);
    }
}

// Get's continuation once a block's matches have reached the callback: each
// block's scan starts at the first entry strictly after `last_delivered`.
// Entries are write-once and sorted, and memtable internal keys are unique, so
// every undelivered match lies after it wherever a split has moved them.
void ExpanseMemTableRep::GetAfterDelivered(
    const LeafBlock* block,
    const char* last_delivered,
    const Slice& user_key,
    void* callback_args,
    bool (*callback_func)(void* arg, const char* entry)
) const {
    while (block != nullptr) {
        bool retry_block = false;
        bool out_of_bounds = false;
        // Sized to the block, as in Get.
        const char* matches[LeafBlock::kMaxCapacity];
        size_t num_matches = 0;

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

            // An upper bound on the last delivered entry.
            while (left < right) {
                int mid = left + (right - left) / 2;
                // Acquire: gain happens-before to this entry's key bytes (published via a
                // release store) before the comparator reads them.
                const char* mid_entry = block->entries[mid].load(std::memory_order_acquire);
                if (mid_entry == nullptr) {
                    retry_block = true;
                    break;
                }
                if (compare_(mid_entry, last_delivered) > 0) {
                    right = mid;
                } else {
                    left = mid + 1;
                }
            }
            if (retry_block) continue;

            for (int i = left; i < count; ++i) {
                // Acquire: gain happens-before to this entry's key bytes before decoding them.
                const char* entry = block->entries[i].load(std::memory_order_acquire);
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
                assert(num_matches < LeafBlock::kMaxCapacity);
                matches[num_matches++] = entry;
            }
            if (retry_block) continue;

            // An acquire fence before a relaxed load, as in Get.
            std::atomic_thread_fence(std::memory_order_acquire);
            uint32_t v_end = block->version.load(std::memory_order_relaxed);
            if (v_start == v_end) {
                break;
            }
        }

        for (size_t i = 0; i < num_matches; ++i) {
            if (!callback_func(callback_args, matches[i])) {
                return;
            }
        }

        if (out_of_bounds) {
            return;
        }
        if (num_matches > 0) {
            last_delivered = matches[num_matches - 1];
        }

        EXPANSE_MEMTABLE_PARK(kGetBeforeNextLeaf);
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

void ExpanseMemTableRep::IteratorImpl::SetPosition(const LeafBlock* leaf, int slot, uint32_t version,
                                                   const char* entry) {
    current_leaf_ = leaf;
    current_slot_ = slot;
    valid_ = true;
    anchor_version_ = version;
    anchor_entry_ = entry;
}

void ExpanseMemTableRep::IteratorImpl::ClearPosition() {
    current_leaf_ = nullptr;
    current_slot_ = -1;
    valid_ = false;
    anchor_version_ = 0;
    anchor_entry_ = nullptr;
}

bool ExpanseMemTableRep::IteratorImpl::PositionAtEdge(const LeafBlock* block, bool forward) {
    while (block != nullptr) {
        uint32_t v = 0;
        uint32_t count = 0;
        const char* entry = nullptr;
        while (true) {
            v = block->version.load(std::memory_order_acquire);
            if (v & 1) {
                std::this_thread::yield();
                continue;
            }
            count = block->count.load(std::memory_order_acquire);
            // Acquire: gain happens-before to this entry's key bytes before the
            // cursor hands them out.
            entry = (count > 0)
                ? block->entries[forward ? 0 : count - 1].load(std::memory_order_acquire)
                : nullptr;
            // An acquire fence before a relaxed load, as in Seek.
            std::atomic_thread_fence(std::memory_order_acquire);
            if (block->version.load(std::memory_order_relaxed) == v && (count == 0 || entry != nullptr)) {
                break;
            }
        }
        if (count > 0) {
            SetPosition(block, forward ? 0 : static_cast<int>(count) - 1, v, entry);
            return true;
        }
        block = forward ? block->next_leaf.load(std::memory_order_acquire)
                        : block->prev_leaf.load(std::memory_order_acquire);
    }
    ClearPosition();
    return false;
}

bool ExpanseMemTableRep::IteratorImpl::RevalidatePosition() const {
    if (!valid_ || current_leaf_ == nullptr || current_slot_ < 0) return false;

    const uint32_t v = current_leaf_->version.load(std::memory_order_acquire);
    // Even and unchanged: the slot still names the entry we anchored on.
    if (v == anchor_version_ && (v & 1) == 0) return true;
    if (anchor_entry_ == nullptr) return false;

    // The leaf moved under us. Before this, the cursor kept its slot index and
    // simply reported whatever now sat there -- or nothing, when a split had
    // lowered `count` below it, which `Valid()` returned as false and RocksDB
    // read as end-of-memtable. Both are silent: a shift re-emits a key, a split
    // drops the whole moved half from the scan.
    //
    // Re-seek by the anchored entry instead. It still exists, so the seek lands
    // on it exactly, in this block or in the one the split carried it into.
    uint32_t ikey_len = 0;
    const char* entry = anchor_entry_;
    const char* p = expanse_rocksdb::GetVarint32Ptr(entry, entry + 5, &ikey_len);
    if (p == nullptr) return false;

    auto* self = const_cast<IteratorImpl*>(this);
    self->Seek(Slice(p, ikey_len), entry);
    return valid_;
}

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
    if (!RevalidatePosition()) return false;
    EXPANSE_MEMTABLE_PARK(kValidAfterRevalidate);
    // The anchor and its slot were read inside one validated bracket, so a
    // position RevalidatePosition() accepted names the anchored entry. Reading
    // the block's count here instead read a later state, and a split between
    // the two reported a live cursor as ended.
    return anchor_entry_ != nullptr;
}

const char* ExpanseMemTableRep::IteratorImpl::key() const {
    if (!RevalidatePosition()) {
        return nullptr;
    }
    EXPANSE_MEMTABLE_PARK(kKeyAfterRevalidate);
    // The anchored entry, not a reload of the slot: a shift after the
    // revalidation puts a different entry in the slot.
    return anchor_entry_;
}

void ExpanseMemTableRep::IteratorImpl::Next() {
    while (true) {
        // Recover the position before stepping off it, or the step is relative
        // to a slot index a writer has already invalidated.
        if (!RevalidatePosition()) {
            valid_ = false;
            return;
        }
        InvalidateCache();
        const LeafBlock* leaf = current_leaf_;
        // The version RevalidatePosition() just matched: the step and its entry
        // are read inside the bracket it opened, and validated against it
        // below, so the new anchor is the entry the step chose.
        const uint32_t v = anchor_version_;
        const int slot = current_slot_ + 1;
        const uint32_t count = leaf->count.load(std::memory_order_acquire);
        const char* entry = (slot < static_cast<int>(count))
            ? leaf->entries[slot].load(std::memory_order_acquire)
            : nullptr;

        // Software SIMD prefetch hint for sibling leaf block when processing latter entries
        if (slot + 4 >= static_cast<int>(count)) {
            LeafBlock* nxt = leaf->next_leaf.load(std::memory_order_relaxed);
            if (nxt != nullptr) {
                expanse_rocksdb::Prefetch<0, 3>(nxt);
                expanse_rocksdb::Prefetch<0, 3>(nxt->entries);
            }
        } else if (slot + 2 < static_cast<int>(count)) {
            const char* future_entry = leaf->entries[slot + 2].load(std::memory_order_relaxed);
            if (future_entry != nullptr) {
                expanse_rocksdb::Prefetch<0, 1>(future_entry);
            }
        }

        // An acquire fence before a relaxed load, as in Seek.
        std::atomic_thread_fence(std::memory_order_acquire);
        if (leaf->version.load(std::memory_order_relaxed) != v ||
            (slot < static_cast<int>(count) && entry == nullptr)) {
            continue;  // the block moved since it was revalidated: recover and step again
        }
        if (slot < static_cast<int>(count)) {
            EXPANSE_MEMTABLE_PARK(kNextBeforeAnchor);
            SetPosition(leaf, slot, v, entry);
            return;
        }
        // Advance directly via next_leaf intrusive pointer without re-seeking trie!
        if (PositionAtEdge(leaf->next_leaf.load(std::memory_order_acquire), true)) {
            LeafBlock* nxt_nxt = current_leaf_->next_leaf.load(std::memory_order_relaxed);
            if (nxt_nxt != nullptr) {
                expanse_rocksdb::Prefetch<0, 3>(nxt_nxt);
            }
        }
        EXPANSE_MEMTABLE_PARK(kNextBeforeAnchor);
        return;
    }
}

void ExpanseMemTableRep::IteratorImpl::Prev() {
    while (true) {
        // Recover the position before stepping off it, or the step is relative
        // to a slot index a writer has already invalidated.
        if (!RevalidatePosition()) {
            valid_ = false;
            return;
        }
        InvalidateCache();
        const LeafBlock* leaf = current_leaf_;
        // As in Next: read the step inside the bracket RevalidatePosition()
        // matched, and validate it against that version.
        const uint32_t v = anchor_version_;
        const int slot = current_slot_ - 1;
        const char* entry = (slot >= 0) ? leaf->entries[slot].load(std::memory_order_acquire) : nullptr;

        // Prefetch prev sibling leaf when approaching beginning of leaf
        if (slot < 4) {
            LeafBlock* prv = leaf->prev_leaf.load(std::memory_order_relaxed);
            if (prv != nullptr) {
                expanse_rocksdb::Prefetch<0, 3>(prv);
                expanse_rocksdb::Prefetch<0, 3>(prv->entries);
            }
        }

        // An acquire fence before a relaxed load, as in Seek.
        std::atomic_thread_fence(std::memory_order_acquire);
        if (leaf->version.load(std::memory_order_relaxed) != v || (slot >= 0 && entry == nullptr)) {
            continue;  // the block moved since it was revalidated: recover and step again
        }
        if (slot >= 0) {
            EXPANSE_MEMTABLE_PARK(kPrevBeforeAnchor);
            SetPosition(leaf, slot, v, entry);
            return;
        }
        // Advance backwards via prev_leaf intrusive pointer!
        PositionAtEdge(leaf->prev_leaf.load(std::memory_order_acquire), false);
        EXPANSE_MEMTABLE_PARK(kPrevBeforeAnchor);
        return;
    }
}

void ExpanseMemTableRep::IteratorImpl::SeekToFirst() {
    InvalidateCache();
    // The position and its anchor are read inside the block's validated bracket.
    if (PositionAtEdge(rep_->head_.load(std::memory_order_acquire), true)) {
        expanse_rocksdb::Prefetch<0, 1>(anchor_entry_);
        LeafBlock* nxt = current_leaf_->next_leaf.load(std::memory_order_relaxed);
        if (nxt) expanse_rocksdb::Prefetch<0, 3>(nxt);
    }
    EXPANSE_MEMTABLE_PARK(kSeekToFirstBeforeAnchor);
}

void ExpanseMemTableRep::IteratorImpl::SeekToLast() {
    InvalidateCache();
    // The position and its anchor are read inside the block's validated bracket.
    PositionAtEdge(rep_->tail_.load(std::memory_order_acquire), false);
    EXPANSE_MEMTABLE_PARK(kSeekToLastBeforeAnchor);
}

void ExpanseMemTableRep::IteratorImpl::Seek(const Slice& internal_key, const char* memtable_key) {
    InvalidateCache();
    const LeafBlock* block = rep_->FindLeafBlockForSeek(internal_key, memtable_key);
    while (block != nullptr) {
        int left = 0;
        int right = 0;
        bool found = false;
        const char* found_entry = nullptr;
        uint32_t found_version = 0;
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
                // Acquire: gain happens-before to this entry's key bytes before the
                // comparator dereferences them.
                const char* mid_entry = block->entries[mid].load(std::memory_order_acquire);
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

            // The entry the cursor will anchor on, read inside this bracket:
            // loading it after the bracket closed read whatever a later shift
            // put in the slot.
            const char* candidate = nullptr;
            if (left < orig_right) {
                candidate = block->entries[left].load(std::memory_order_acquire);
                if (candidate == nullptr) continue;
            }

            // An acquire *fence* before a relaxed load, not an acquire load.
            // An acquire load orders what follows it; this re-read has to be
            // ordered after the bracket's payload reads, which is the opposite
            // direction. Mirrors SeqVersion::validate in
            // crates/expanse/src/occ.rs.
            std::atomic_thread_fence(std::memory_order_acquire);
            uint32_t v_end = block->version.load(std::memory_order_relaxed);
            if (v_start == v_end) {
                if (left < orig_right) {
                    found = true;
                    found_entry = candidate;
                    found_version = v_start;
                }
                break;
            }
        }

        if (found) {
            EXPANSE_MEMTABLE_PARK(kSeekBeforeAnchor);
            SetPosition(block, left, found_version, found_entry);
            return;
        }
        block = block->next_leaf.load(std::memory_order_acquire);
    }
    ClearPosition();
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
    // Seek, Prev and SeekToLast each anchored the position they chose. A
    // second capture here re-read the version and the slot after they had
    // returned, and anchored on whatever a shift since had put in the slot.
    EXPANSE_MEMTABLE_PARK(kSeekForPrevBeforeAnchor);
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
    const char* taken[LeafBlock::kMaxCapacity];

    while (extracted < max_keys) {
        // Recover the position before stepping off it, for the same reason Next()
        // and Prev() do: the slot index alone does not survive a concurrent shift
        // or split, and a batch that starts from a stale index extracts from
        // wherever that index now points.
        if (!RevalidatePosition()) {
            valid_ = false;
            break;
        }
        const LeafBlock* leaf = current_leaf_;
        // The chunk, and the entry the cursor moves to after it, are read inside
        // the bracket RevalidatePosition() matched and validated against it, so
        // neither the delivered entries nor the next anchor can come from a
        // shifted slot.
        const uint32_t v = anchor_version_;
        const int slot = current_slot_;
        const uint32_t count = leaf->count.load(std::memory_order_acquire);
        const size_t take = (slot < static_cast<int>(count))
            ? std::min(static_cast<size_t>(count - slot), max_keys - extracted)
            : 0;
        bool torn = false;
        for (size_t i = 0; i < take && !torn; ++i) {
            // Acquire: gain happens-before to this entry's bytes before decoding them.
            taken[i] = leaf->entries[slot + i].load(std::memory_order_acquire);
            torn = taken[i] == nullptr;
        }
        const int next_slot = slot + static_cast<int>(take);
        const char* next_entry = nullptr;
        if (!torn && next_slot < static_cast<int>(count)) {
            next_entry = leaf->entries[next_slot].load(std::memory_order_acquire);
            torn = next_entry == nullptr;
        }

        // Issue prefetch hint for next leaf block when scanning through current block
        LeafBlock* nxt = leaf->next_leaf.load(std::memory_order_relaxed);
        if (nxt != nullptr) {
            expanse_rocksdb::Prefetch<0, 3>(nxt);
            expanse_rocksdb::Prefetch<0, 3>(nxt->entries);
        }

        // An acquire fence before a relaxed load, as in Seek.
        std::atomic_thread_fence(std::memory_order_acquire);
        if (torn || leaf->version.load(std::memory_order_relaxed) != v) {
            continue;  // the block moved since it was revalidated: recover and read again
        }
        // A position RevalidatePosition() accepted has its slot below the
        // block's count at the matched version, so a validated chunk is never
        // empty.
        assert(take > 0);

        for (size_t i = 0; i < take; ++i) {
            const char* entry = taken[i];
            // Prefetch entry payload 2 slots ahead (prefetch never dereferences)
            if (i + 2 < take) {
                expanse_rocksdb::Prefetch<0, 1>(taken[i + 2]);
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
            // Only a validated, non-null slot is taken, so every one counts.
            extracted++;
        }

        if (next_slot < static_cast<int>(count)) {
            SetPosition(leaf, next_slot, v, next_entry);
        } else if (!PositionAtEdge(leaf->next_leaf.load(std::memory_order_acquire), true)) {
            break;
        }
    }

    // The cursor is left anchored on the entry after the last one extracted,
    // so the documented `while (Valid()) ScanBatch(...)` loop advances rather
    // than re-extracting its first batch (#769 added the anchor mechanism to
    // Seek/Next/Prev but not here).
    EXPANSE_MEMTABLE_PARK(kScanBatchBeforeAnchor);
    return extracted;
}

} // namespace rocksdb
