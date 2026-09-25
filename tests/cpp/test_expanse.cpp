#include <cassert>
#include <iostream>
#include <vector>
#include <string>
#include <string_view>
#include <span>
#include <thread>
#include <atomic>
#include <concepts>
#include <algorithm>
#include <ranges>
#include <numeric>

#include "expanse.hpp"

// Verify standard concept conformance at compile time
static_assert(std::forward_iterator<expanse::set::iterator>);
static_assert(std::forward_iterator<expanse::map<uint64_t, uint64_t>::iterator>);
static_assert(std::forward_iterator<expanse::str_map<uint64_t>::iterator>);

void test_version() {
    std::cout << "[RUN] test_version" << std::endl;
    auto ver = expanse::version();
    assert(!ver.empty());
    assert(ver.find('.') != std::string_view::npos);
    std::cout << "      libexpanse version: " << ver << std::endl;
    std::cout << "[PASS] test_version" << std::endl;
}

void test_set() {
    std::cout << "[RUN] test_set" << std::endl;
    expanse::set s;
    assert(s.empty());
    assert(s.size() == 0);
    assert(s.native_handle() != nullptr);

    // Insert 0, 10, 20, ..., 990
    for (uint64_t i = 0; i < 100; ++i) {
        assert(s.insert(i * 10));
    }
    assert(!s.insert(0)); // Duplicate
    assert(!s.empty());
    assert(s.size() == 100);
    assert(s.mem_used() > 0);

    for (uint64_t i = 0; i < 100; ++i) {
        assert(s.contains(i * 10));
        assert(!s.contains(i * 10 + 1));
    }

    // Edge keys
    assert(s.insert(UINT64_MAX));
    assert(s.contains(UINT64_MAX));
    assert(s.size() == 101);
    assert(s.last() == UINT64_MAX);
    assert(s.erase(UINT64_MAX));
    assert(!s.contains(UINT64_MAX));
    assert(s.size() == 100);

    // Navigation
    assert(s.first() == 0);
    assert(s.last() == 990);
    assert(s.next(20) == 30);
    assert(s.next_at_or_after(20) == 20);
    assert(s.next_at_or_after(25) == 30);
    assert(s.prev(30) == 20);
    assert(s.prev_at_or_before(30) == 30);
    assert(s.prev_at_or_before(25) == 20);
    assert(!s.next(990).has_value());
    assert(!s.prev(0).has_value());

    // Rank & select
    assert(s.count_below(30) == 3); // 0, 10, 20
    assert(s.rank(30) == 3);
    assert(s.count_range(20, 50) == 4); // 20, 30, 40, 50
    assert(s.count_range(50, 20) == 0);
    assert(s.select(0) == 0);
    assert(s.select(4) == 40);
    assert(s.by_count(4) == 40);
    assert(!s.select(1000).has_value());

    // Range-based for loop iteration
    std::vector<uint64_t> collected;
    for (uint64_t val : s) {
        collected.push_back(val);
    }
    assert(collected.size() == 100);
    for (size_t i = 0; i < 100; ++i) {
        assert(collected[i] == i * 10);
    }

    // std::ranges compatibility
    assert(std::ranges::distance(s) == 100);
    auto it = std::ranges::find(s, 500);
    assert(it != s.end());
    assert(*it == 500);

    // Erase
    assert(s.erase(0));
    assert(!s.erase(0));
    assert(!s.contains(0));
    assert(s.size() == 99);

    // Move semantics
    expanse::set s2 = std::move(s);
    assert(s.size() == 0);
    assert(s2.size() == 99);
    assert(s2.contains(10));

    expanse::set s3;
    s3 = std::move(s2);
    assert(s2.size() == 0);
    assert(s3.size() == 99);

    // Self move assignment
    #pragma clang diagnostic push
    #pragma clang diagnostic ignored "-Wself-move"
    s3 = std::move(s3);
    #pragma clang diagnostic pop
    assert(s3.size() == 99);

    s3.clear();
    assert(s3.empty());
    assert(s3.size() == 0);
    assert(s3.validate());

    std::cout << "[PASS] test_set" << std::endl;
}

void test_map() {
    std::cout << "[RUN] test_map" << std::endl;
    expanse::map<uint64_t, uint64_t> m;
    assert(m.empty());
    assert(m.size() == 0);

    uint64_t old_val = 0;
    assert(m.insert(100, 1000, &old_val));
    assert(!m.insert(100, 1001, &old_val));
    assert(old_val == 1000);

    assert(m.size() == 1);
    assert(m.contains(100));
    assert(!m.contains(200));
    assert(m.get(100) == 1001);
    assert(!m.get(200).has_value());

    // operator[]
    m[200] = 2000;
    assert(m.size() == 2);
    assert(m.get(200) == 2000);
    m[200] += 5;
    assert(m[200] == 2005);

    // Default initialization via operator[]
    assert(m[555] == 0);
    assert(m.size() == 3);

    // Populate more entries
    for (uint64_t k = 1000; k < 2000; ++k) {
        m[k] = k * 10;
    }
    assert(m.size() == 1003);

    // Navigation
    auto first_entry = m.first();
    assert(first_entry.has_value());
    assert(first_entry->first == 100 && first_entry->second == 1001);

    auto last_entry = m.last();
    assert(last_entry.has_value());
    assert(last_entry->first == 1999 && last_entry->second == 19990);

    auto next_entry = m.next(100);
    assert(next_entry.has_value());
    assert(next_entry->first == 200 && next_entry->second == 2005);

    auto next_aoa = m.next_at_or_after(150);
    assert(next_aoa.has_value());
    assert(next_aoa->first == 200);

    auto prev_entry = m.prev(200);
    assert(prev_entry.has_value());
    assert(prev_entry->first == 100);

    // Rank, select, count_range
    assert(m.count_below(200) == 1);
    assert(m.rank(200) == 1);
    assert(m.count_range(1000, 1999) == 1000);

    auto sel = m.select(1);
    assert(sel.has_value());
    assert(sel->first == 200);

    // Range-based iteration
    size_t iter_count = 0;
    uint64_t prev_k = 0;
    for (auto [key, val] : m) {
        if (iter_count > 0) {
            assert(key > prev_k);
        }
        prev_k = key;
        ++iter_count;
    }
    assert(iter_count == 1003);

    // Erase
    assert(m.erase(100, &old_val));
    assert(old_val == 1001);
    assert(!m.erase(100));
    assert(m.size() == 1002);

    // Move semantics
    expanse::map<uint64_t, uint64_t> m2 = std::move(m);
    assert(m.empty());
    assert(m2.size() == 1002);
    assert(m2[200] == 2005);

    // Typed map (e.g. enum / uint32_t)
    enum class Status : uint32_t { Active = 1, Idle = 2, Suspended = 3 };
    expanse::map<uint32_t, Status> status_map;
    status_map.insert(42, Status::Active);
    assert(status_map.get(42) == Status::Active);
    status_map[42] = Status::Suspended;
    assert(status_map.get(42) == Status::Suspended);

    assert(m2.validate());
    char err[256] = {0};
    assert(m2.validate_explain(err, sizeof(err)));

    std::cout << "[PASS] test_map" << std::endl;
}

void test_str_map() {
    std::cout << "[RUN] test_str_map" << std::endl;
    expanse::str_map<uint64_t> sm;
    assert(sm.empty());
    assert(sm.size() == 0);

    uint64_t old = 0;
    assert(sm.insert("apple", 10, &old));
    assert(!sm.insert("apple", 15, &old));
    assert(old == 10);

    assert(sm.insert("banana", 20));
    assert(sm.insert("cherry", 30));
    assert(sm.insert("date", 40));
    assert(sm.size() == 4);

    assert(sm.contains("banana"));
    assert(!sm.contains("blueberry"));
    assert(sm.get("cherry") == 30);
    assert(!sm.get("fig").has_value());

    // operator[]
    sm["elderberry"] = 50;
    assert(sm.size() == 5);
    assert(sm["elderberry"] == 50);
    sm["elderberry"] += 5;
    assert(sm["elderberry"] == 55);

    // Navigation
    auto f = sm.first();
    assert(f.has_value());
    assert(f->first == "apple" && f->second == 15);

    auto l = sm.last();
    assert(l.has_value());
    assert(l->first == "elderberry" && l->second == 55);

    auto n = sm.next("banana");
    assert(n.has_value());
    assert(n->first == "cherry" && n->second == 30);

    auto p = sm.prev("date");
    assert(p.has_value());
    assert(p->first == "cherry" && p->second == 30);

    // Range-based for loop
    std::vector<std::string> keys;
    for (const auto& [k, v] : sm) {
        keys.push_back(k);
    }
    assert(keys.size() == 5);
    assert(keys[0] == "apple");
    assert(keys[1] == "banana");
    assert(keys[2] == "cherry");
    assert(keys[3] == "date");
    assert(keys[4] == "elderberry");

    // Erase
    assert(sm.erase("banana", &old));
    assert(old == 20);
    assert(!sm.erase("banana"));
    assert(sm.size() == 4);

    // Move semantics
    expanse::str_map<uint64_t> sm2 = std::move(sm);
    assert(sm.empty());
    assert(sm2.size() == 4);
    assert(sm2.get("apple") == 15);

    sm2.clear();
    assert(sm2.empty());

    std::cout << "[PASS] test_str_map" << std::endl;
}

// Keys longer than the default 4096-byte scratch buffer must not be silently
// dropped by navigation/iteration. The _ex-based retry loop must grow the buffer.
void test_str_map_long_keys() {
    std::cout << "[RUN] test_str_map_long_keys" << std::endl;
    expanse::str_map<uint64_t> sm;

    const std::string short_key = "aaa";
    const std::string long_key(10000, 'b');   // ~10 KiB, far past the 4 KiB default
    const std::string longer_key(20000, 'c');
    sm.insert(short_key, 1);
    sm.insert(long_key, 2);
    sm.insert(longer_key, 3);
    assert(sm.size() == 3);

    // first()/last() must see the long keys ('a' < 'b' < 'c'), not report empty.
    auto f = sm.first();
    assert(f.has_value());
    assert(f->first == short_key && f->second == 1);
    auto l = sm.last();
    assert(l.has_value());
    assert(l->first == longer_key);
    assert(l->first.size() == 20000);
    assert(l->second == 3);

    // Forward navigation must step INTO and OUT OF the 10 KiB key.
    auto after_short = sm.next(short_key);
    assert(after_short.has_value());
    assert(after_short->first == long_key);
    assert(after_short->first.size() == 10000);
    assert(after_short->second == 2);
    auto after_long = sm.next(long_key);
    assert(after_long.has_value());
    assert(after_long->first == longer_key);

    // Reverse navigation likewise.
    auto before_longer = sm.prev(longer_key);
    assert(before_longer.has_value());
    assert(before_longer->first == long_key);

    // Full iteration must visit all three keys in order (the iterator uses _ex too).
    std::vector<std::string> visited;
    for (const auto& [k, v] : sm) {
        (void)v;
        visited.push_back(k);
    }
    assert(visited.size() == 3);
    assert(visited[0] == short_key);
    assert(visited[1] == long_key);
    assert(visited[2] == longer_key);

    std::cout << "[PASS] test_str_map_long_keys" << std::endl;
}

void test_bytes_map() {
    std::cout << "[RUN] test_bytes_map" << std::endl;
    expanse::bytes_map<uint64_t> bm;
    assert(bm.empty());
    assert(bm.size() == 0);

    // Binary keys with embedded NUL bytes
    const char key1_raw[] = "binary\0key\0alpha";
    std::span<const std::byte> key1{reinterpret_cast<const std::byte*>(key1_raw), sizeof(key1_raw) - 1};

    const char key2_raw[] = "binary\0key\0beta";
    std::span<const std::byte> key2{reinterpret_cast<const std::byte*>(key2_raw), sizeof(key2_raw) - 1};

    uint64_t old = 0;
    assert(bm.insert(key1, 1001, &old));
    assert(!bm.insert(key1, 1002, &old));
    assert(old == 1001);

    assert(bm.insert(key2, 2002));
    assert(bm.size() == 2);

    assert(bm.contains(key1));
    assert(bm.contains(key2));
    assert(!bm.contains(std::string_view("binary"))); // Prefix is NOT a match

    assert(bm.get(key1) == 1002);
    assert(bm.get(key2) == 2002);

    // Support std::string_view
    std::string_view sv_key = "normal_ascii_key";
    assert(bm.insert(sv_key, 3003));
    assert(bm.get(sv_key) == 3003);
    assert(bm.size() == 3);

    // Support std::span<const uint8_t>
    std::vector<uint8_t> u8_key = {0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01};
    assert(bm.insert(std::span<const uint8_t>(u8_key), 4004));
    assert(bm.get(std::span<const uint8_t>(u8_key)) == 4004);
    assert(bm.size() == 4);

    // operator[]
    bm[sv_key] = 3005;
    assert(bm[sv_key] == 3005);

    // Erase
    assert(bm.erase(key1, &old));
    assert(old == 1002);
    assert(!bm.erase(key1));
    assert(bm.size() == 3);

    // Move semantics
    expanse::bytes_map<uint64_t> bm2 = std::move(bm);
    assert(bm.empty());
    assert(bm2.size() == 3);
    assert(bm2.get(key2) == 2002);

    bm2.clear();
    assert(bm2.empty());

    // Test for_each
    bm2.insert(std::string_view("alpha"), 10);
    bm2.insert(std::string_view("beta"), 20);
    std::vector<std::string> collected_keys;
    bm2.for_each([&collected_keys](std::span<const std::byte> k, uint64_t) {
        std::string s(reinterpret_cast<const char*>(k.data()), k.size());
        collected_keys.push_back(s);
        return true;
    });
    assert(collected_keys.size() == 2);

    // Early termination on false
    size_t count = 0;
    bm2.for_each([&count](std::span<const std::byte>, uint64_t) {
        count++;
        return false;
    });
    assert(count == 1);

    // Exception rethrow
    try {
        bm2.for_each([](std::span<const std::byte>, uint64_t) -> bool {
            throw std::runtime_error("for_each exception");
        });
        assert(false);
    } catch (const std::runtime_error& e) {
        assert(std::string_view(e.what()) == "for_each exception");
    }

    std::cout << "[PASS] test_bytes_map" << std::endl;
}

void test_blob_map() {
    std::cout << "[RUN] test_blob_map" << std::endl;
    expanse::blob_map bm(64 * 1024);
    assert(bm.empty());
    assert(bm.size() == 0);

    // 1. Inline payloads (<= 7 bytes)
    std::string_view inline_str = "hello"; // 5 bytes
    assert(bm.insert(1, inline_str, 0x1111));

    // 2. Arena payloads (> 7 bytes)
    std::string arena_str = "This is a longer payload allocated in the arena buffer";
    assert(bm.insert(2, arena_str, 0x2222));

    // 3. Exact 7-byte inline boundary
    std::string_view exact_7 = "1234567";
    assert(bm.insert(3, exact_7, 0x3333));

    // 4. 8-byte arena payload
    std::string_view exact_8 = "12345678";
    assert(bm.insert(4, exact_8, 0x4444));

    // 5. Zero-byte empty payload
    std::string_view empty_str = "";
    assert(bm.insert(5, empty_str, 0x5555));

    // 6. Large multi-KB arena payload
    std::vector<uint8_t> large_data(4096, 0xAB);
    assert(bm.insert(6, std::span<const uint8_t>(large_data), 0x6666));

    assert(bm.size() == 6);
    assert(bm.contains(1));
    assert(bm.contains(2));
    assert(bm.contains(3));
    assert(bm.contains(4));
    assert(bm.contains(5));
    assert(bm.contains(6));
    assert(!bm.contains(7));

    // Inspect zero-copy views
    auto v1 = bm.get(1);
    assert(v1.has_value());
    assert(v1->as_string_view() == "hello");
    assert(v1->size() == 5);
    assert(v1->is_inline());

    auto v2 = bm.get(2);
    assert(v2.has_value());
    assert(v2->as_string_view() == arena_str);
    assert(v2->size() == arena_str.size());
    assert(!v2->is_inline());
    assert(v2->hot_meta() == 0x2222);

    auto v3 = bm.get(3);
    assert(v3.has_value() && v3->is_inline() && v3->as_string_view() == "1234567");

    auto v4 = bm.get(4);
    assert(v4.has_value() && !v4->is_inline() && v4->as_string_view() == "12345678");

    auto v5 = bm.get(5);
    assert(v5.has_value() && v5->is_inline() && v5->empty());

    auto v6 = bm.get(6);
    assert(v6.has_value() && !v6->is_inline() && v6->size() == 4096);
    auto u8_view = v6->as_u8();
    for (size_t i = 0; i < 4096; ++i) {
        assert(u8_view[i] == 0xAB);
    }

    // Filtered scan
    std::vector<uint64_t> scanned_keys;
    size_t scanned_count = bm.scan_filtered(
        1, 6,
        [](uint64_t /*k*/, uint32_t meta) {
            // Match items with hot_meta == 0x2222 or 0x4444
            return meta == 0x2222 || meta == 0x4444;
        },
        [&scanned_keys](uint64_t k, expanse::blob_view view) {
            scanned_keys.push_back(k);
            return true;
        }
    );
    assert(scanned_count == 2);
    assert(scanned_keys.size() == 2);
    assert(scanned_keys[0] == 2 && scanned_keys[1] == 4);

    // Prune entries matching predicate
    // Prune entry 2 (hot_meta == 0x2222) and entry 6 (hot_meta == 0x6666)
    size_t pruned = bm.prune([](uint64_t /*k*/, uint32_t meta) {
        return meta == 0x2222 || meta == 0x6666;
    });
    assert(pruned == 2);
    assert(bm.size() == 4);
    assert(!bm.contains(2));
    assert(!bm.contains(6));

    // Compaction
    assert(bm.compact());
    assert(bm.size() == 4);

    // Erase
    assert(bm.erase(1));
    assert(!bm.erase(1));
    assert(bm.size() == 3);

    // Move semantics
    expanse::blob_map bm2 = std::move(bm);
    assert(bm.empty());
    assert(bm2.size() == 3);
    assert(bm2.contains(3));

    bm2.clear();
    assert(bm2.empty());

    // Test compressed inline payload: 14-byte numeric string compresses to <= 8 bytes inline
    std::string_view ts_str = "20260829125825";
    assert(bm2.insert(100, ts_str));
    // Zero-copy get() returns nullopt for compressed inline as view.ptr == nullptr
    auto v100 = bm2.get(100);
    assert(!v100.has_value());
    // get_into successfully decompresses into caller-supplied buffer
    uint8_t buf[32];
    size_t out_len = 0;
    uint32_t out_meta = 0;
    assert(bm2.get_into(100, std::span<uint8_t>(buf, sizeof(buf)), &out_len, &out_meta));
    assert(out_len == 14);
    assert(std::string_view(reinterpret_cast<const char*>(buf), out_len) == ts_str);

    // Regular uncompressed payload supports zero-copy blob_view copy and move
    std::string_view uncompressed_str = "hello_uncompressed";
    assert(bm2.insert(101, uncompressed_str));
    auto v101 = bm2.get(101);
    assert(v101.has_value());
    assert(v101->as_string_view() == uncompressed_str);
    auto v101_copy = *v101;
    assert(v101_copy.as_string_view() == uncompressed_str);
    auto v101_move = std::move(v101_copy);
    assert(v101_move.as_string_view() == uncompressed_str);

    // Test scan_filtered exception propagation
    try {
        bm2.scan_filtered(1, 200, [](uint64_t, uint32_t) { return true; },
            [](uint64_t, expanse::blob_view) -> bool {
                throw std::runtime_error("scan exception");
            });
        assert(false);
    } catch (const std::runtime_error& e) {
        assert(std::string_view(e.what()) == "scan exception");
    }

    std::cout << "[PASS] test_blob_map" << std::endl;
}

void test_sync_set() {
    std::cout << "[RUN] test_sync_set" << std::endl;
    expanse::sync_set ss;
    assert(ss.empty());
    assert(ss.size() == 0);

    for (uint64_t i = 0; i < 500; ++i) {
        assert(ss.insert(i));
    }
    assert(ss.size() == 500);
    assert(ss.contains(250));
    assert(!ss.contains(999));

    // Multi-threaded lock-free OCC readers
    std::atomic<bool> stop_flag{false};
    std::vector<std::thread> reader_threads;

    for (int t = 0; t < 4; ++t) {
        reader_threads.emplace_back([&ss, &stop_flag]() {
            auto reader = ss.make_reader();
            while (!stop_flag.load(std::memory_order_relaxed)) {
                for (uint64_t k = 0; k < 500; ++k) {
                    assert(reader.contains(k));
                }
            }
        });
    }

    // Concurrent writer mutating concurrently
    for (uint64_t i = 500; i < 1000; ++i) {
        assert(ss.insert(i));
    }
    assert(ss.size() == 1000);

    stop_flag.store(true, std::memory_order_relaxed);
    for (auto& th : reader_threads) {
        th.join();
    }

    // Move semantics
    expanse::sync_set ss2 = std::move(ss);
    assert(ss2.size() == 1000);
    assert(ss2.contains(999));

    std::cout << "[PASS] test_sync_set" << std::endl;
}

void test_sync_map() {
    std::cout << "[RUN] test_sync_map" << std::endl;
    expanse::sync_map sm;
    assert(sm.empty());
    assert(sm.size() == 0);

    for (uint64_t i = 0; i < 500; ++i) {
        assert(sm.insert(i, i * 10));
    }
    assert(sm.size() == 500);
    assert(sm.mem_used() > 0);
    assert(sm.get(250) == 2500);

    // Multi-threaded OCC readers
    std::atomic<bool> stop_flag{false};
    std::vector<std::thread> reader_threads;

    for (int t = 0; t < 4; ++t) {
        reader_threads.emplace_back([&sm, &stop_flag]() {
            auto reader = sm.make_reader();
            while (!stop_flag.load(std::memory_order_relaxed)) {
                for (uint64_t k = 0; k < 500; ++k) {
                    auto val = reader.get(k);
                    assert(val.has_value());
                    assert(*val == k * 10);
                }
            }
        });
    }

    // Concurrent writer
    for (uint64_t i = 500; i < 1000; ++i) {
        assert(sm.insert(i, i * 10));
    }
    assert(sm.size() == 1000);

    stop_flag.store(true, std::memory_order_relaxed);
    for (auto& th : reader_threads) {
        th.join();
    }

    // Ordered reads through a reader handle (#900): keys 0..999, value k * 10.
    {
        auto reader = sm.make_reader();
        using entry = std::pair<uint64_t, uint64_t>;
        assert(reader.first() == entry(0, 0));
        assert(reader.last() == entry(999, 9990));
        assert(reader.next(10) == entry(11, 110));
        assert(reader.next_at_or_after(10) == entry(10, 100));
        assert(reader.prev(10) == entry(9, 90));
        assert(reader.prev_at_or_before(10) == entry(10, 100));
        assert(!reader.next(999).has_value());
        assert(!reader.prev(0).has_value());
        assert(!reader.next_at_or_after(1000).has_value());
        assert(reader.prev_at_or_before(UINT64_MAX) == entry(999, 9990));
    }

    // Erase
    uint64_t old = 0;
    assert(sm.erase(0, &old));
    assert(old == 0);
    assert(!sm.contains(0));
    assert(sm.size() == 999);

    // Move semantics
    expanse::sync_map sm2 = std::move(sm);
    assert(sm2.size() == 999);
    assert(sm2.get(500) == 5000);

    std::cout << "[PASS] test_sync_map" << std::endl;
}

// A map, set or string map drained by erase keeps its freed blocks; shrink_to_fit()
// returns exactly mem_held() - mem_used(), after which the two agree.
template <typename C>
void check_shrink(C& c, bool retains) {
    const size_t held = c.mem_held();
    const size_t used = c.mem_used();
    assert(held >= used);
    if (retains) {
        assert(used == 0 && held > 0);
    }
    assert(c.shrink_to_fit() == held - used);
    assert(c.mem_held() == c.mem_used());
    assert(c.mem_used() == used);
    assert(c.shrink_to_fit() == 0);
}

void test_shrink_to_fit() {
    constexpr uint64_t n = 20000;
    expanse::map<uint64_t, uint64_t> m;
    expanse::set s;
    expanse::str_map<uint64_t> sm;
    std::vector<std::string> keys;
    keys.reserve(n);
    for (uint64_t k = 0; k < n; ++k) {
        keys.push_back("key/" + std::to_string(k));
        m.insert(k * 7, k);
        s.insert(k * 7);
        sm.insert(keys.back(), k);
    }
    for (uint64_t k = 0; k < n; ++k) {
        assert(m.erase(k * 7));
        assert(s.erase(k * 7));
        assert(sm.erase(keys[k]));
    }
    check_shrink(m, true);
    check_shrink(s, true);
    check_shrink(sm, true);

    std::cout << "[PASS] test_shrink_to_fit" << std::endl;
}

// A half-drained concurrent map holds more than it uses: its epoch collector
// keeps the freed blocks. shrink_to_fit() returns those past their grace
// period and mem_held() falls by exactly what it reports; blocks still in
// their grace period stay held, so mem_held() is not asserted to reach
// mem_used().
void test_sync_shrink_to_fit() {
    constexpr uint64_t n = 50000;
    expanse::sync_map sm;
    expanse::sync_set ss;
    for (uint64_t k = 0; k < n; ++k) {
        sm.insert(k * 0x9E3779B1ULL, k);
        ss.insert(k * 0x9E3779B1ULL);
    }
    for (uint64_t k = 0; k < n; k += 2) {
        assert(sm.erase(k * 0x9E3779B1ULL));
        assert(ss.erase(k * 0x9E3779B1ULL));
    }
    assert(sm.mem_held() > sm.mem_used());
    const size_t map_held = sm.mem_held();
    const size_t map_released = sm.shrink_to_fit();
    assert(sm.mem_held() == map_held - map_released);
    const size_t set_held = ss.mem_held();
    assert(set_held > 0);
    const size_t set_released = ss.shrink_to_fit();
    assert(ss.mem_held() == set_held - set_released);
    for (uint64_t k = 1; k < n; k += 2) {
        assert(sm.get(k * 0x9E3779B1ULL) == std::optional<uint64_t>(k));
        assert(ss.contains(k * 0x9E3779B1ULL));
    }

    std::cout << "[PASS] test_sync_shrink_to_fit" << std::endl;
}

int main() {
    std::cout << "========================================" << std::endl;
    std::cout << "Running Expanse C++20 Header Unit Tests" << std::endl;
    std::cout << "========================================" << std::endl;

    test_version();
    test_set();
    test_map();
    test_str_map();
    test_str_map_long_keys();
    test_bytes_map();
    test_blob_map();
    test_sync_set();
    test_sync_map();
    test_shrink_to_fit();
    test_sync_shrink_to_fit();

    std::cout << "========================================" << std::endl;
    std::cout << "All C++20 unit tests passed successfully!" << std::endl;
    std::cout << "========================================" << std::endl;
    return 0;
}
