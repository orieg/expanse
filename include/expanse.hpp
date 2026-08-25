#pragma once

#if __cplusplus < 202002L
#error "expanse.hpp requires C++20 or later"
#endif

#include <cstddef>
#include <cstdint>
#include <cstring>
#include <string_view>
#include <string>
#include <span>
#include <concepts>
#include <memory>
#include <utility>
#include <optional>
#include <iterator>
#include <functional>
#include <stdexcept>
#include <type_traits>
#include <compare>
#include <vector>

#include "expanse.h"

namespace expanse {

/// Returns the version string of the linked libexpanse build (e.g., "0.4.0").
[[nodiscard]] inline std::string_view version() noexcept {
    return expanse_version();
}

namespace detail {
    template <typename T>
    inline std::span<const std::byte> to_byte_span(const T& val) noexcept {
        if constexpr (std::is_same_v<std::decay_t<T>, std::span<const std::byte>> ||
                      std::is_same_v<std::decay_t<T>, std::span<std::byte>>) {
            return val;
        } else if constexpr (std::is_same_v<std::decay_t<T>, std::span<const uint8_t>> ||
                             std::is_same_v<std::decay_t<T>, std::span<uint8_t>>) {
            return {reinterpret_cast<const std::byte*>(val.data()), val.size()};
        } else if constexpr (std::is_same_v<std::decay_t<T>, std::span<const char>> ||
                             std::is_same_v<std::decay_t<T>, std::span<char>>) {
            return {reinterpret_cast<const std::byte*>(val.data()), val.size()};
        } else if constexpr (std::is_convertible_v<T, std::string_view>) {
            std::string_view sv = val;
            return {reinterpret_cast<const std::byte*>(sv.data()), sv.size()};
        } else {
            return {reinterpret_cast<const std::byte*>(std::data(val)), std::size(val)};
        }
    }
} // namespace detail

// ============================================================================
// expanse::set — ordered bitset of uint64_t keys (wrapping expanse_set_t)
// ============================================================================

class set {
public:
    class const_iterator {
    public:
        using iterator_category = std::forward_iterator_tag;
        using value_type        = uint64_t;
        using difference_type   = std::ptrdiff_t;
        using pointer           = const uint64_t*;
        using reference         = uint64_t;

        constexpr const_iterator() noexcept : set_(nullptr), current_(0), is_end_(true) {}

        const_iterator(const expanse_set_t* s, uint64_t key, bool is_end) noexcept
            : set_(s), current_(key), is_end_(is_end) {}

        [[nodiscard]] uint64_t operator*() const noexcept {
            return current_;
        }

        const_iterator& operator++() noexcept {
            if (!is_end_ && set_) {
                uint64_t next_key = 0;
                if (expanse_set_next_after(set_, current_, &next_key)) {
                    current_ = next_key;
                } else {
                    is_end_  = true;
                    current_ = 0;
                }
            }
            return *this;
        }

        const_iterator operator++(int) noexcept {
            const_iterator tmp = *this;
            ++(*this);
            return tmp;
        }

        friend bool operator==(const const_iterator& a, const const_iterator& b) noexcept {
            if (a.is_end_ && b.is_end_) return true;
            if (a.is_end_ != b.is_end_) return false;
            return a.set_ == b.set_ && a.current_ == b.current_;
        }

        friend bool operator!=(const const_iterator& a, const const_iterator& b) noexcept {
            return !(a == b);
        }

    private:
        const expanse_set_t* set_{nullptr};
        uint64_t             current_{0};
        bool                 is_end_{true};
    };

    using iterator        = const_iterator;
    using value_type      = uint64_t;
    using size_type       = uint64_t;
    using difference_type = std::ptrdiff_t;

    set() noexcept : ptr_(expanse_set_new()) {}
    explicit set(expanse_set_t* ptr) noexcept : ptr_(ptr) {}

    ~set() noexcept {
        if (ptr_) {
            expanse_set_free(ptr_);
            ptr_ = nullptr;
        }
    }

    set(const set&) = delete;
    set& operator=(const set&) = delete;

    set(set&& other) noexcept : ptr_(other.ptr_) {
        other.ptr_ = nullptr;
    }

    set& operator=(set&& other) noexcept {
        if (this != &other) {
            if (ptr_) {
                expanse_set_free(ptr_);
            }
            ptr_       = other.ptr_;
            other.ptr_ = nullptr;
        }
        return *this;
    }

    bool insert(uint64_t key) noexcept {
        return expanse_set_insert(ptr_, key);
    }

    bool erase(uint64_t key) noexcept {
        return expanse_set_remove(ptr_, key);
    }

    bool remove(uint64_t key) noexcept {
        return erase(key);
    }

    [[nodiscard]] bool contains(uint64_t key) const noexcept {
        return expanse_set_contains(ptr_, key);
    }

    [[nodiscard]] uint64_t size() const noexcept {
        return expanse_set_len(ptr_);
    }

    [[nodiscard]] bool empty() const noexcept {
        return size() == 0;
    }

    [[nodiscard]] size_t mem_used() const noexcept {
        return expanse_set_mem_used(ptr_);
    }

    void clear() noexcept {
        expanse_set_clear(ptr_);
    }

    void swap(set& other) noexcept {
        std::swap(ptr_, other.ptr_);
    }

    [[nodiscard]] std::optional<uint64_t> first() const noexcept {
        uint64_t out = 0;
        if (expanse_set_first(ptr_, &out)) {
            return out;
        }
        return std::nullopt;
    }

    [[nodiscard]] std::optional<uint64_t> last() const noexcept {
        uint64_t out = 0;
        if (expanse_set_last(ptr_, &out)) {
            return out;
        }
        return std::nullopt;
    }

    [[nodiscard]] std::optional<uint64_t> next(uint64_t key) const noexcept {
        uint64_t out = 0;
        if (expanse_set_next_after(ptr_, key, &out)) {
            return out;
        }
        return std::nullopt;
    }

    [[nodiscard]] std::optional<uint64_t> next_at_or_after(uint64_t key) const noexcept {
        uint64_t out = 0;
        if (expanse_set_next_at_or_after(ptr_, key, &out)) {
            return out;
        }
        return std::nullopt;
    }

    [[nodiscard]] std::optional<uint64_t> prev(uint64_t key) const noexcept {
        uint64_t out = 0;
        if (expanse_set_prev_before(ptr_, key, &out)) {
            return out;
        }
        return std::nullopt;
    }

    [[nodiscard]] std::optional<uint64_t> prev_at_or_before(uint64_t key) const noexcept {
        uint64_t out = 0;
        if (expanse_set_prev_at_or_before(ptr_, key, &out)) {
            return out;
        }
        return std::nullopt;
    }

    [[nodiscard]] uint64_t count_below(uint64_t key) const noexcept {
        return expanse_set_count_below(ptr_, key);
    }

    [[nodiscard]] uint64_t rank(uint64_t key) const noexcept {
        return count_below(key);
    }

    [[nodiscard]] uint64_t count_range(uint64_t lo, uint64_t hi) const noexcept {
        return expanse_set_count_range(ptr_, lo, hi);
    }

    [[nodiscard]] std::optional<uint64_t> select(uint64_t n) const noexcept {
        uint64_t out = 0;
        if (expanse_set_by_count(ptr_, n, &out)) {
            return out;
        }
        return std::nullopt;
    }

    [[nodiscard]] std::optional<uint64_t> by_count(uint64_t n) const noexcept {
        return select(n);
    }

    [[nodiscard]] const_iterator begin() const noexcept {
        auto f = first();
        if (f.has_value()) {
            return const_iterator(ptr_, *f, false);
        }
        return end();
    }

    [[nodiscard]] const_iterator end() const noexcept {
        return const_iterator(ptr_, 0, true);
    }

    [[nodiscard]] const_iterator cbegin() const noexcept { return begin(); }
    [[nodiscard]] const_iterator cend() const noexcept { return end(); }

    [[nodiscard]] expanse_set_t* native_handle() noexcept { return ptr_; }
    [[nodiscard]] const expanse_set_t* native_handle() const noexcept { return ptr_; }
    [[nodiscard]] expanse_set_t* release() noexcept {
        expanse_set_t* tmp = ptr_;
        ptr_ = nullptr;
        return tmp;
    }

private:
    expanse_set_t* ptr_{nullptr};
};

// ============================================================================
// expanse::map<Key, Value> — ordered word map (wrapping expanse_map_t)
// ============================================================================

template <typename Key = uint64_t, typename Value = uint64_t>
class map {
    static_assert(sizeof(Key) <= sizeof(uint64_t), "Key must fit in uint64_t");
    static_assert(sizeof(Value) <= sizeof(uint64_t), "Value must fit in uint64_t");

public:
    class const_iterator {
    public:
        using iterator_category = std::forward_iterator_tag;
        using value_type        = std::pair<Key, Value>;
        using difference_type   = std::ptrdiff_t;
        using pointer           = const std::pair<Key, Value>*;
        using reference         = std::pair<Key, Value>;

        constexpr const_iterator() noexcept : map_(nullptr), current_{}, is_end_(true) {}

        const_iterator(const expanse_map_t* m, Key k, Value v, bool is_end) noexcept
            : map_(m), current_{k, v}, is_end_(is_end) {}

        [[nodiscard]] std::pair<Key, Value> operator*() const noexcept {
            return current_;
        }

        const_iterator& operator++() noexcept {
            if (!is_end_ && map_) {
                uint64_t next_k = 0, next_v = 0;
                if (expanse_map_next_after(map_, static_cast<uint64_t>(current_.first), &next_k, &next_v)) {
                    current_ = {static_cast<Key>(next_k), static_cast<Value>(next_v)};
                } else {
                    is_end_  = true;
                    current_ = {};
                }
            }
            return *this;
        }

        const_iterator operator++(int) noexcept {
            const_iterator tmp = *this;
            ++(*this);
            return tmp;
        }

        friend bool operator==(const const_iterator& a, const const_iterator& b) noexcept {
            if (a.is_end_ && b.is_end_) return true;
            if (a.is_end_ != b.is_end_) return false;
            return a.map_ == b.map_ && a.current_.first == b.current_.first;
        }

        friend bool operator!=(const const_iterator& a, const const_iterator& b) noexcept {
            return !(a == b);
        }

    private:
        const expanse_map_t* map_{nullptr};
        std::pair<Key, Value> current_{};
        bool                 is_end_{true};
    };

    using iterator        = const_iterator;
    using key_type        = Key;
    using mapped_type     = Value;
    using value_type      = std::pair<Key, Value>;
    using size_type       = uint64_t;
    using difference_type = std::ptrdiff_t;

    map() noexcept : ptr_(expanse_map_new()) {}
    explicit map(expanse_map_t* ptr) noexcept : ptr_(ptr) {}

    ~map() noexcept {
        if (ptr_) {
            expanse_map_free(ptr_);
            ptr_ = nullptr;
        }
    }

    map(const map&) = delete;
    map& operator=(const map&) = delete;

    map(map&& other) noexcept : ptr_(other.ptr_) {
        other.ptr_ = nullptr;
    }

    map& operator=(map&& other) noexcept {
        if (this != &other) {
            if (ptr_) {
                expanse_map_free(ptr_);
            }
            ptr_       = other.ptr_;
            other.ptr_ = nullptr;
        }
        return *this;
    }

    bool insert(Key key, Value value, Value* old_out = nullptr) noexcept {
        uint64_t old_v = 0;
        bool is_new = expanse_map_insert(
            ptr_,
            static_cast<uint64_t>(key),
            static_cast<uint64_t>(value),
            old_out ? &old_v : nullptr
        );
        if (old_out && !is_new) {
            *old_out = static_cast<Value>(old_v);
        }
        return is_new;
    }

    bool erase(Key key, Value* old_out = nullptr) noexcept {
        uint64_t old_v = 0;
        bool removed = expanse_map_remove(
            ptr_,
            static_cast<uint64_t>(key),
            old_out ? &old_v : nullptr
        );
        if (old_out && removed) {
            *old_out = static_cast<Value>(old_v);
        }
        return removed;
    }

    bool remove(Key key, Value* old_out = nullptr) noexcept {
        return erase(key, old_out);
    }

    [[nodiscard]] std::optional<Value> get(Key key) const noexcept {
        uint64_t val = 0;
        if (expanse_map_get(ptr_, static_cast<uint64_t>(key), &val)) {
            return static_cast<Value>(val);
        }
        return std::nullopt;
    }

    [[nodiscard]] bool contains(Key key) const noexcept {
        uint64_t val = 0;
        return expanse_map_get(ptr_, static_cast<uint64_t>(key), &val);
    }

    [[nodiscard]] uint64_t* slot(Key key) noexcept {
        return expanse_map_slot(ptr_, static_cast<uint64_t>(key));
    }

    [[nodiscard]] uint64_t* ins_slot(Key key) noexcept {
        return expanse_map_ins_slot(ptr_, static_cast<uint64_t>(key));
    }

    [[nodiscard]] Value& operator[](Key key) {
        uint64_t* s = ins_slot(key);
        if (!s) {
            throw std::bad_alloc();
        }
        return *reinterpret_cast<Value*>(s);
    }

    [[nodiscard]] uint64_t size() const noexcept {
        return expanse_map_len(ptr_);
    }

    [[nodiscard]] bool empty() const noexcept {
        return size() == 0;
    }

    [[nodiscard]] size_t mem_used() const noexcept {
        return expanse_map_mem_used(ptr_);
    }

    void clear() noexcept {
        expanse_map_clear(ptr_);
    }

    void swap(map& other) noexcept {
        std::swap(ptr_, other.ptr_);
    }

    [[nodiscard]] std::optional<std::pair<Key, Value>> first() const noexcept {
        uint64_t k = 0, v = 0;
        if (expanse_map_first(ptr_, &k, &v)) {
            return std::pair<Key, Value>{static_cast<Key>(k), static_cast<Value>(v)};
        }
        return std::nullopt;
    }

    [[nodiscard]] std::optional<std::pair<Key, Value>> last() const noexcept {
        uint64_t k = 0, v = 0;
        if (expanse_map_last(ptr_, &k, &v)) {
            return std::pair<Key, Value>{static_cast<Key>(k), static_cast<Value>(v)};
        }
        return std::nullopt;
    }

    [[nodiscard]] std::optional<std::pair<Key, Value>> next(Key key) const noexcept {
        uint64_t k = 0, v = 0;
        if (expanse_map_next_after(ptr_, static_cast<uint64_t>(key), &k, &v)) {
            return std::pair<Key, Value>{static_cast<Key>(k), static_cast<Value>(v)};
        }
        return std::nullopt;
    }

    [[nodiscard]] std::optional<std::pair<Key, Value>> next_at_or_after(Key key) const noexcept {
        uint64_t k = 0, v = 0;
        if (expanse_map_next_at_or_after(ptr_, static_cast<uint64_t>(key), &k, &v)) {
            return std::pair<Key, Value>{static_cast<Key>(k), static_cast<Value>(v)};
        }
        return std::nullopt;
    }

    [[nodiscard]] std::optional<std::pair<Key, Value>> prev(Key key) const noexcept {
        uint64_t k = 0, v = 0;
        if (expanse_map_prev_before(ptr_, static_cast<uint64_t>(key), &k, &v)) {
            return std::pair<Key, Value>{static_cast<Key>(k), static_cast<Value>(v)};
        }
        return std::nullopt;
    }

    [[nodiscard]] std::optional<std::pair<Key, Value>> prev_at_or_before(Key key) const noexcept {
        uint64_t k = 0, v = 0;
        if (expanse_map_prev_at_or_before(ptr_, static_cast<uint64_t>(key), &k, &v)) {
            return std::pair<Key, Value>{static_cast<Key>(k), static_cast<Value>(v)};
        }
        return std::nullopt;
    }

    [[nodiscard]] uint64_t count_below(Key key) const noexcept {
        return expanse_map_count_below(ptr_, static_cast<uint64_t>(key));
    }

    [[nodiscard]] uint64_t rank(Key key) const noexcept {
        return count_below(key);
    }

    [[nodiscard]] uint64_t count_range(Key lo, Key hi) const noexcept {
        return expanse_map_count_range(ptr_, static_cast<uint64_t>(lo), static_cast<uint64_t>(hi));
    }

    [[nodiscard]] std::optional<std::pair<Key, Value>> select(uint64_t n) const noexcept {
        uint64_t k = 0, v = 0;
        if (expanse_map_by_count(ptr_, n, &k, &v)) {
            return std::pair<Key, Value>{static_cast<Key>(k), static_cast<Value>(v)};
        }
        return std::nullopt;
    }

    [[nodiscard]] std::optional<std::pair<Key, Value>> by_count(uint64_t n) const noexcept {
        return select(n);
    }

    [[nodiscard]] const_iterator begin() const noexcept {
        auto f = first();
        if (f.has_value()) {
            return const_iterator(ptr_, f->first, f->second, false);
        }
        return end();
    }

    [[nodiscard]] const_iterator end() const noexcept {
        return const_iterator(ptr_, Key{}, Value{}, true);
    }

    [[nodiscard]] const_iterator cbegin() const noexcept { return begin(); }
    [[nodiscard]] const_iterator cend() const noexcept { return end(); }

    [[nodiscard]] expanse_map_t* native_handle() noexcept { return ptr_; }
    [[nodiscard]] const expanse_map_t* native_handle() const noexcept { return ptr_; }
    [[nodiscard]] expanse_map_t* release() noexcept {
        expanse_map_t* tmp = ptr_;
        ptr_ = nullptr;
        return tmp;
    }

private:
    expanse_map_t* ptr_{nullptr};
};

// ============================================================================
// expanse::str_map<Value> — ordered string trie (wrapping expanse_strmap_t)
// ============================================================================

namespace detail {

// Drives a truncation-aware `expanse_strmap_*_ex` navigation call in a retry loop,
// growing the output buffer until the key fits. `fn` is invoked as
// `fn(char* key_out, size_t buf_len, size_t* required_len, uint64_t* value_out)`
// and returns an `expanse_str_nav_status`. Returning `std::nullopt` means NOT_FOUND
// (a genuinely empty result) — never a silently-truncated long key, which the plain
// (non-`_ex`) navigation could not distinguish from end-of-map.
template <typename Fn>
[[nodiscard]] inline std::optional<std::pair<std::string, uint64_t>>
strmap_nav_retry(Fn&& fn, std::size_t initial_buf_len) {
    std::size_t buf_len = initial_buf_len == 0 ? 64 : initial_buf_len;
    for (;;) {
        std::string buf(buf_len, '\0');
        std::size_t required = 0;
        uint64_t v = 0;
        const expanse_str_nav_status st = fn(buf.data(), buf.size(), &required, &v);
        switch (st) {
            case EXPANSE_STR_NAV_OK:
                buf.resize(std::strlen(buf.c_str()));
                return std::pair<std::string, uint64_t>{std::move(buf), v};
            case EXPANSE_STR_NAV_NOT_FOUND:
                return std::nullopt;
            case EXPANSE_STR_NAV_BUFFER_TOO_SMALL:
                // `required` includes the NUL terminator; grow at least geometrically.
                buf_len = required > buf_len ? required : buf_len * 2;
                break;
            default:
                return std::nullopt;
        }
    }
}

}  // namespace detail

template <typename Value = uint64_t>
class str_map {
    static_assert(sizeof(Value) <= sizeof(uint64_t), "Value must fit in uint64_t");

public:
    class const_iterator {
    public:
        using iterator_category = std::forward_iterator_tag;
        using value_type        = std::pair<std::string, Value>;
        using difference_type   = std::ptrdiff_t;
        using pointer           = const std::pair<std::string, Value>*;
        using reference         = const std::pair<std::string, Value>&;

        constexpr const_iterator() noexcept : map_(nullptr), current_{}, is_end_(true) {}

        const_iterator(expanse_strmap_t* m, std::string k, Value v, bool is_end)
            : map_(m), current_{std::move(k), v}, is_end_(is_end) {}

        [[nodiscard]] const std::pair<std::string, Value>& operator*() const noexcept {
            return current_;
        }

        [[nodiscard]] const std::pair<std::string, Value>* operator->() const noexcept {
            return &current_;
        }

        const_iterator& operator++() {
            if (!is_end_ && map_) {
                // Use the truncation-aware _ex nav with a growing buffer so a key
                // longer than the scratch buffer never ends iteration early.
                std::string key = current_.first;
                auto next = detail::strmap_nav_retry(
                    [&](char* out, std::size_t buf_len, std::size_t* required, uint64_t* value_out) {
                        return expanse_strmap_next_after_ex(
                            map_, key.c_str(), out, buf_len, required, value_out);
                    },
                    4096);
                if (next.has_value()) {
                    current_ = {std::move(next->first), static_cast<Value>(next->second)};
                } else {
                    is_end_  = true;
                    current_ = {};
                }
            }
            return *this;
        }

        const_iterator operator++(int) {
            const_iterator tmp = *this;
            ++(*this);
            return tmp;
        }

        friend bool operator==(const const_iterator& a, const const_iterator& b) noexcept {
            if (a.is_end_ && b.is_end_) return true;
            if (a.is_end_ != b.is_end_) return false;
            return a.map_ == b.map_ && a.current_.first == b.current_.first;
        }

        friend bool operator!=(const const_iterator& a, const const_iterator& b) noexcept {
            return !(a == b);
        }

    private:
        expanse_strmap_t*              map_{nullptr};
        std::pair<std::string, Value> current_{};
        bool                          is_end_{true};
    };

    using iterator        = const_iterator;
    using key_type        = std::string;
    using mapped_type     = Value;
    using value_type      = std::pair<std::string, Value>;
    using size_type       = uint64_t;
    using difference_type = std::ptrdiff_t;

    str_map() noexcept : ptr_(expanse_strmap_new()) {}
    explicit str_map(expanse_strmap_t* ptr) noexcept : ptr_(ptr) {}

    ~str_map() noexcept {
        if (ptr_) {
            expanse_strmap_free(ptr_);
            ptr_ = nullptr;
        }
    }

    str_map(const str_map&) = delete;
    str_map& operator=(const str_map&) = delete;

    str_map(str_map&& other) noexcept : ptr_(other.ptr_) {
        other.ptr_ = nullptr;
    }

    str_map& operator=(str_map&& other) noexcept {
        if (this != &other) {
            if (ptr_) {
                expanse_strmap_free(ptr_);
            }
            ptr_       = other.ptr_;
            other.ptr_ = nullptr;
        }
        return *this;
    }

    bool insert(std::string_view key, Value value, Value* old_out = nullptr) {
        std::string k_str(key);
        uint64_t old_v = 0;
        bool is_new = expanse_strmap_insert(
            ptr_,
            k_str.c_str(),
            static_cast<uint64_t>(value),
            old_out ? &old_v : nullptr
        );
        if (old_out && !is_new) {
            *old_out = static_cast<Value>(old_v);
        }
        return is_new;
    }

    bool erase(std::string_view key, Value* old_out = nullptr) {
        std::string k_str(key);
        uint64_t old_v = 0;
        bool removed = expanse_strmap_remove(
            ptr_,
            k_str.c_str(),
            old_out ? &old_v : nullptr
        );
        if (old_out && removed) {
            *old_out = static_cast<Value>(old_v);
        }
        return removed;
    }

    bool remove(std::string_view key, Value* old_out = nullptr) {
        return erase(key, old_out);
    }

    [[nodiscard]] std::optional<Value> get(std::string_view key) const {
        std::string k_str(key);
        uint64_t val = 0;
        if (expanse_strmap_get(ptr_, k_str.c_str(), &val)) {
            return static_cast<Value>(val);
        }
        return std::nullopt;
    }

    [[nodiscard]] bool contains(std::string_view key) const {
        return get(key).has_value();
    }

    [[nodiscard]] uint64_t* slot(std::string_view key) {
        std::string k_str(key);
        return expanse_strmap_slot(ptr_, k_str.c_str());
    }

    [[nodiscard]] uint64_t* ins_slot(std::string_view key) {
        std::string k_str(key);
        return expanse_strmap_ins_slot(ptr_, k_str.c_str());
    }

    [[nodiscard]] Value& operator[](std::string_view key) {
        uint64_t* s = ins_slot(key);
        if (!s) {
            throw std::bad_alloc();
        }
        return *reinterpret_cast<Value*>(s);
    }

    [[nodiscard]] uint64_t size() const noexcept {
        return expanse_strmap_len(ptr_);
    }

    [[nodiscard]] bool empty() const noexcept {
        return size() == 0;
    }

    [[nodiscard]] size_t mem_used() const noexcept {
        return expanse_strmap_mem_used(ptr_);
    }

    void clear() noexcept {
        expanse_strmap_clear(ptr_);
    }

    void swap(str_map& other) noexcept {
        std::swap(ptr_, other.ptr_);
    }

    // The `max_buf_len` argument is the INITIAL scratch size; the truncation-aware
    // _ex navigation grows it as needed, so a key longer than it is still returned
    // (previously such a key was silently reported as "no entry").
    [[nodiscard]] std::optional<std::pair<std::string, Value>> first(size_t max_buf_len = 4096) const {
        auto r = detail::strmap_nav_retry(
            [&](char* o, std::size_t bl, std::size_t* rl, uint64_t* vo) {
                return expanse_strmap_first_ex(ptr_, o, bl, rl, vo);
            },
            max_buf_len);
        if (r.has_value()) return std::pair<std::string, Value>{std::move(r->first), static_cast<Value>(r->second)};
        return std::nullopt;
    }

    [[nodiscard]] std::optional<std::pair<std::string, Value>> last(size_t max_buf_len = 4096) const {
        auto r = detail::strmap_nav_retry(
            [&](char* o, std::size_t bl, std::size_t* rl, uint64_t* vo) {
                return expanse_strmap_last_ex(ptr_, o, bl, rl, vo);
            },
            max_buf_len);
        if (r.has_value()) return std::pair<std::string, Value>{std::move(r->first), static_cast<Value>(r->second)};
        return std::nullopt;
    }

    [[nodiscard]] std::optional<std::pair<std::string, Value>> next(std::string_view key, size_t max_buf_len = 4096) const {
        std::string k_str(key);
        auto r = detail::strmap_nav_retry(
            [&](char* o, std::size_t bl, std::size_t* rl, uint64_t* vo) {
                return expanse_strmap_next_after_ex(ptr_, k_str.c_str(), o, bl, rl, vo);
            },
            max_buf_len);
        if (r.has_value()) return std::pair<std::string, Value>{std::move(r->first), static_cast<Value>(r->second)};
        return std::nullopt;
    }

    [[nodiscard]] std::optional<std::pair<std::string, Value>> next_at_or_after(std::string_view key, size_t max_buf_len = 4096) const {
        std::string k_str(key);
        auto r = detail::strmap_nav_retry(
            [&](char* o, std::size_t bl, std::size_t* rl, uint64_t* vo) {
                return expanse_strmap_next_at_or_after_ex(ptr_, k_str.c_str(), o, bl, rl, vo);
            },
            max_buf_len);
        if (r.has_value()) return std::pair<std::string, Value>{std::move(r->first), static_cast<Value>(r->second)};
        return std::nullopt;
    }

    [[nodiscard]] std::optional<std::pair<std::string, Value>> prev(std::string_view key, size_t max_buf_len = 4096) const {
        std::string k_str(key);
        auto r = detail::strmap_nav_retry(
            [&](char* o, std::size_t bl, std::size_t* rl, uint64_t* vo) {
                return expanse_strmap_prev_before_ex(ptr_, k_str.c_str(), o, bl, rl, vo);
            },
            max_buf_len);
        if (r.has_value()) return std::pair<std::string, Value>{std::move(r->first), static_cast<Value>(r->second)};
        return std::nullopt;
    }

    [[nodiscard]] std::optional<std::pair<std::string, Value>> prev_at_or_before(std::string_view key, size_t max_buf_len = 4096) const {
        std::string k_str(key);
        auto r = detail::strmap_nav_retry(
            [&](char* o, std::size_t bl, std::size_t* rl, uint64_t* vo) {
                return expanse_strmap_prev_at_or_before_ex(ptr_, k_str.c_str(), o, bl, rl, vo);
            },
            max_buf_len);
        if (r.has_value()) return std::pair<std::string, Value>{std::move(r->first), static_cast<Value>(r->second)};
        return std::nullopt;
    }

    [[nodiscard]] const_iterator begin() const {
        auto f = first();
        if (f.has_value()) {
            return const_iterator(ptr_, std::move(f->first), f->second, false);
        }
        return end();
    }

    [[nodiscard]] const_iterator end() const noexcept {
        return const_iterator();
    }

    [[nodiscard]] const_iterator cbegin() const { return begin(); }
    [[nodiscard]] const_iterator cend() const noexcept { return end(); }

    [[nodiscard]] expanse_strmap_t* native_handle() noexcept { return ptr_; }
    [[nodiscard]] const expanse_strmap_t* native_handle() const noexcept { return ptr_; }
    [[nodiscard]] expanse_strmap_t* release() noexcept {
        expanse_strmap_t* tmp = ptr_;
        ptr_ = nullptr;
        return tmp;
    }

private:
    expanse_strmap_t* ptr_{nullptr};
};

// ============================================================================
// expanse::bytes_map<Value> — binary-safe byte map (wrapping expanse_bytesmap_t)
// ============================================================================

template <typename Value = uint64_t>
class bytes_map {
    static_assert(sizeof(Value) <= sizeof(uint64_t), "Value must fit in uint64_t");

public:
    using mapped_type = Value;
    using size_type   = uint64_t;

    bytes_map() noexcept : ptr_(expanse_bytesmap_new()) {}
    explicit bytes_map(expanse_bytesmap_t* ptr) noexcept : ptr_(ptr) {}

    ~bytes_map() noexcept {
        if (ptr_) {
            expanse_bytesmap_free(ptr_);
            ptr_ = nullptr;
        }
    }

    bytes_map(const bytes_map&) = delete;
    bytes_map& operator=(const bytes_map&) = delete;

    bytes_map(bytes_map&& other) noexcept : ptr_(other.ptr_) {
        other.ptr_ = nullptr;
    }

    bytes_map& operator=(bytes_map&& other) noexcept {
        if (this != &other) {
            if (ptr_) {
                expanse_bytesmap_free(ptr_);
            }
            ptr_       = other.ptr_;
            other.ptr_ = nullptr;
        }
        return *this;
    }

    template <typename KeyLike>
    bool insert(const KeyLike& key, Value value, Value* old_out = nullptr) noexcept {
        auto span = detail::to_byte_span(key);
        uint64_t old_v = 0;
        bool is_new = expanse_bytesmap_insert(
            ptr_,
            span.data(),
            span.size(),
            static_cast<uint64_t>(value),
            old_out ? &old_v : nullptr
        );
        if (old_out && !is_new) {
            *old_out = static_cast<Value>(old_v);
        }
        return is_new;
    }

    template <typename KeyLike>
    bool erase(const KeyLike& key, Value* old_out = nullptr) noexcept {
        auto span = detail::to_byte_span(key);
        uint64_t old_v = 0;
        bool removed = expanse_bytesmap_remove(
            ptr_,
            span.data(),
            span.size(),
            old_out ? &old_v : nullptr
        );
        if (old_out && removed) {
            *old_out = static_cast<Value>(old_v);
        }
        return removed;
    }

    template <typename KeyLike>
    bool remove(const KeyLike& key, Value* old_out = nullptr) noexcept {
        return erase(key, old_out);
    }

    template <typename KeyLike>
    [[nodiscard]] std::optional<Value> get(const KeyLike& key) const noexcept {
        auto span = detail::to_byte_span(key);
        uint64_t val = 0;
        if (expanse_bytesmap_get(ptr_, span.data(), span.size(), &val)) {
            return static_cast<Value>(val);
        }
        return std::nullopt;
    }

    template <typename KeyLike>
    [[nodiscard]] bool contains(const KeyLike& key) const noexcept {
        return get(key).has_value();
    }

    template <typename KeyLike>
    [[nodiscard]] uint64_t* slot(const KeyLike& key) noexcept {
        auto span = detail::to_byte_span(key);
        return expanse_bytesmap_slot(ptr_, span.data(), span.size());
    }

    template <typename KeyLike>
    [[nodiscard]] uint64_t* ins_slot(const KeyLike& key) noexcept {
        auto span = detail::to_byte_span(key);
        return expanse_bytesmap_ins_slot(ptr_, span.data(), span.size());
    }

    template <typename KeyLike>
    [[nodiscard]] Value& operator[](const KeyLike& key) {
        uint64_t* s = ins_slot(key);
        if (!s) {
            throw std::bad_alloc();
        }
        return *reinterpret_cast<Value*>(s);
    }

    [[nodiscard]] uint64_t size() const noexcept {
        return expanse_bytesmap_len(ptr_);
    }

    [[nodiscard]] bool empty() const noexcept {
        return size() == 0;
    }

    [[nodiscard]] size_t mem_used() const noexcept {
        return expanse_bytesmap_mem_used(ptr_);
    }

    void clear() noexcept {
        expanse_bytesmap_clear(ptr_);
    }

    void swap(bytes_map& other) noexcept {
        std::swap(ptr_, other.ptr_);
    }

    [[nodiscard]] expanse_bytesmap_t* native_handle() noexcept { return ptr_; }
    [[nodiscard]] const expanse_bytesmap_t* native_handle() const noexcept { return ptr_; }
    [[nodiscard]] expanse_bytesmap_t* release() noexcept {
        expanse_bytesmap_t* tmp = ptr_;
        ptr_ = nullptr;
        return tmp;
    }

private:
    expanse_bytesmap_t* ptr_{nullptr};
};

// ============================================================================
// expanse::blob_view & expanse::blob_map — off-heap large-value map
// ============================================================================

// A zero-copy view of a stored payload.
//
// INVALIDATION CONTRACT (mirrors the C ExpanseBlobView contract and the .NET
// TryGet span contract): the bytes returned by data()/as_u8()/as_string_view()
// alias memory owned by the blob_map — either an inline value slot or an arena
// slab. They stay valid ONLY until the next structural mutation of that map.
// Any insert / remove / clear / compact() — and destroying the map — invalidates
// every previously obtained blob_view; compact() in particular MOVES live payloads,
// so the pointer itself becomes stale. Reading a view after such a mutation is
// undefined behavior. Copy the bytes out (e.g. into a std::string or std::vector)
// before mutating if you need them to outlive the next mutation. A blob_view passed
// to a scan_filtered callback is valid only for the duration of that callback.
class blob_view {
public:
    constexpr blob_view() noexcept : data_{}, hot_meta_{0}, is_inline_{false} {}
    constexpr blob_view(std::span<const std::byte> data, uint32_t hot_meta, bool is_inline) noexcept
        : data_(data), hot_meta_(hot_meta), is_inline_(is_inline) {}

    [[nodiscard]] constexpr std::span<const std::byte> data() const noexcept { return data_; }
    [[nodiscard]] inline std::span<const uint8_t> as_u8() const noexcept {
        return {reinterpret_cast<const uint8_t*>(data_.data()), data_.size()};
    }
    [[nodiscard]] inline std::string_view as_string_view() const noexcept {
        return {reinterpret_cast<const char*>(data_.data()), data_.size()};
    }
    [[nodiscard]] constexpr const std::byte* data_ptr() const noexcept { return data_.data(); }
    [[nodiscard]] constexpr size_t size() const noexcept { return data_.size(); }
    [[nodiscard]] constexpr size_t len() const noexcept { return data_.size(); }
    [[nodiscard]] constexpr bool empty() const noexcept { return data_.empty(); }
    [[nodiscard]] constexpr uint32_t hot_meta() const noexcept { return hot_meta_; }
    [[nodiscard]] constexpr bool is_inline() const noexcept { return is_inline_; }

    constexpr operator std::span<const std::byte>() const noexcept { return data_; }
    inline operator std::string_view() const noexcept { return as_string_view(); }

    [[nodiscard]] constexpr const std::byte& operator[](size_t idx) const noexcept {
        return data_[idx];
    }

private:
    std::span<const std::byte> data_{};
    uint32_t                   hot_meta_{0};
    bool                       is_inline_{false};
};

class blob_map {
public:
    explicit blob_map(size_t chunk_size = 0) noexcept
        : ptr_(expanse_blob_map_new(chunk_size)) {}
    explicit blob_map(ExpanseBlobMap* ptr) noexcept : ptr_(ptr) {}

    ~blob_map() noexcept {
        if (ptr_) {
            expanse_blob_map_free(ptr_);
            ptr_ = nullptr;
        }
    }

    blob_map(const blob_map&) = delete;
    blob_map& operator=(const blob_map&) = delete;

    blob_map(blob_map&& other) noexcept : ptr_(other.ptr_) {
        other.ptr_ = nullptr;
    }

    blob_map& operator=(blob_map&& other) noexcept {
        if (this != &other) {
            if (ptr_) {
                expanse_blob_map_free(ptr_);
            }
            ptr_       = other.ptr_;
            other.ptr_ = nullptr;
        }
        return *this;
    }

    template <typename DataLike>
    bool insert(uint64_t key, const DataLike& data, uint32_t hot_meta = 0) noexcept {
        auto span = detail::to_byte_span(data);
        return expanse_blob_map_insert(
            ptr_,
            key,
            reinterpret_cast<const uint8_t*>(span.data()),
            span.size(),
            hot_meta
        );
    }

    bool erase(uint64_t key) noexcept {
        return expanse_blob_map_remove(ptr_, key);
    }

    bool remove(uint64_t key) noexcept {
        return erase(key);
    }

    // Returns a zero-copy view into the map's storage. See the blob_view invalidation
    // contract above: the returned view is only valid until the next mutation of this
    // map (insert/remove/clear/compact/destruction); copy the bytes out if you need them
    // to survive a mutation.
    [[nodiscard]] std::optional<blob_view> get(uint64_t key) const noexcept {
        ExpanseBlobView view{};
        if (expanse_blob_map_get(ptr_, key, &view)) {
            return blob_view{
                std::span<const std::byte>{
                    reinterpret_cast<const std::byte*>(view.ptr),
                    view.len
                },
                view.hot_meta,
                view.is_inline
            };
        }
        return std::nullopt;
    }

    [[nodiscard]] bool contains(uint64_t key) const noexcept {
        return expanse_blob_map_contains_key(ptr_, key);
    }

    [[nodiscard]] uint64_t size() const noexcept {
        return expanse_blob_map_len(ptr_);
    }

    [[nodiscard]] bool empty() const noexcept {
        return size() == 0;
    }

    [[nodiscard]] size_t mem_used() const noexcept {
        return expanse_blob_map_mem_used(ptr_);
    }

    void clear() noexcept {
        expanse_blob_map_clear(ptr_);
    }

    bool compact() noexcept {
        return expanse_blob_map_compact(ptr_);
    }

    template <typename Predicate>
    size_t prune(Predicate&& pred) {
        struct PruneContext {
            Predicate* p;
            std::vector<uint64_t> keys;
        } ctx{&pred, {}};

        auto pred_wrapper = [](uint64_t k, uint32_t meta, void* user_ctx) -> bool {
            auto* c = static_cast<PruneContext*>(user_ctx);
            return (*(c->p))(k, meta);
        };

        auto cb_wrapper = [](uint64_t k, ExpanseBlobView /*view*/, void* user_ctx) -> bool {
            auto* c = static_cast<PruneContext*>(user_ctx);
            c->keys.push_back(k);
            return true;
        };

        expanse_blob_map_scan_filtered(
            ptr_,
            0,
            UINT64_MAX,
            pred_wrapper,
            cb_wrapper,
            &ctx
        );

        size_t count = 0;
        for (uint64_t k : ctx.keys) {
            if (erase(k)) {
                ++count;
            }
        }
        return count;
    }

    template <typename Predicate, typename Callback>
    size_t scan_filtered(
        uint64_t start_key,
        uint64_t end_key,
        Predicate&& pred,
        Callback&& cb
    ) const {
        struct ScanContext {
            Predicate* p;
            Callback* c;
        } ctx{&pred, &cb};

        auto pred_wrapper = [](uint64_t k, uint32_t meta, void* user_ctx) -> bool {
            auto* c = static_cast<ScanContext*>(user_ctx);
            return (*(c->p))(k, meta);
        };

        auto cb_wrapper = [](uint64_t k, ExpanseBlobView view, void* user_ctx) -> bool {
            auto* c = static_cast<ScanContext*>(user_ctx);
            blob_view bv{
                std::span<const std::byte>{
                    reinterpret_cast<const std::byte*>(view.ptr),
                    view.len
                },
                view.hot_meta,
                view.is_inline
            };
            return (*(c->c))(k, bv);
        };

        return expanse_blob_map_scan_filtered(
            ptr_,
            start_key,
            end_key,
            pred_wrapper,
            cb_wrapper,
            &ctx
        );
    }

    void swap(blob_map& other) noexcept {
        std::swap(ptr_, other.ptr_);
    }

    [[nodiscard]] ExpanseBlobMap* native_handle() noexcept { return ptr_; }
    [[nodiscard]] const ExpanseBlobMap* native_handle() const noexcept { return ptr_; }
    [[nodiscard]] ExpanseBlobMap* release() noexcept {
        ExpanseBlobMap* tmp = ptr_;
        ptr_ = nullptr;
        return tmp;
    }

private:
    ExpanseBlobMap* ptr_{nullptr};
};

// ============================================================================
// Concurrent types — lock-free OCC readers (sync_set, sync_map)
// ============================================================================

class sync_set_reader {
public:
    sync_set_reader() noexcept : ptr_(nullptr) {}
    explicit sync_set_reader(expanse_sync_set_reader_t* ptr) noexcept : ptr_(ptr) {}

    ~sync_set_reader() noexcept {
        if (ptr_) {
            expanse_sync_set_reader_free(ptr_);
            ptr_ = nullptr;
        }
    }

    sync_set_reader(const sync_set_reader&) = delete;
    sync_set_reader& operator=(const sync_set_reader&) = delete;

    sync_set_reader(sync_set_reader&& other) noexcept : ptr_(other.ptr_) {
        other.ptr_ = nullptr;
    }

    sync_set_reader& operator=(sync_set_reader&& other) noexcept {
        if (this != &other) {
            if (ptr_) {
                expanse_sync_set_reader_free(ptr_);
            }
            ptr_       = other.ptr_;
            other.ptr_ = nullptr;
        }
        return *this;
    }

    [[nodiscard]] bool contains(uint64_t key) const noexcept {
        return expanse_sync_set_reader_contains(ptr_, key);
    }

    [[nodiscard]] expanse_sync_set_reader_t* native_handle() noexcept { return ptr_; }
    [[nodiscard]] const expanse_sync_set_reader_t* native_handle() const noexcept { return ptr_; }

private:
    expanse_sync_set_reader_t* ptr_{nullptr};
};

class sync_set {
public:
    sync_set() noexcept : ptr_(expanse_sync_set_new()) {}
    explicit sync_set(expanse_sync_set_t* ptr) noexcept : ptr_(ptr) {}

    ~sync_set() noexcept {
        if (ptr_) {
            expanse_sync_set_free(ptr_);
            ptr_ = nullptr;
        }
    }

    sync_set(const sync_set&) = delete;
    sync_set& operator=(const sync_set&) = delete;

    sync_set(sync_set&& other) noexcept : ptr_(other.ptr_) {
        other.ptr_ = nullptr;
    }

    sync_set& operator=(sync_set&& other) noexcept {
        if (this != &other) {
            if (ptr_) {
                expanse_sync_set_free(ptr_);
            }
            ptr_       = other.ptr_;
            other.ptr_ = nullptr;
        }
        return *this;
    }

    bool insert(uint64_t key) noexcept {
        return expanse_sync_set_insert(ptr_, key);
    }

    bool erase(uint64_t key) noexcept {
        return expanse_sync_set_remove(ptr_, key);
    }

    bool remove(uint64_t key) noexcept {
        return erase(key);
    }

    [[nodiscard]] bool contains(uint64_t key) const noexcept {
        return expanse_sync_set_contains(ptr_, key);
    }

    [[nodiscard]] uint64_t size() const noexcept {
        return expanse_sync_set_len(ptr_);
    }

    [[nodiscard]] bool empty() const noexcept {
        return size() == 0;
    }

    [[nodiscard]] sync_set_reader make_reader() const noexcept {
        return sync_set_reader(expanse_sync_set_reader_new(ptr_));
    }

    [[nodiscard]] sync_set_reader reader() const noexcept {
        return make_reader();
    }

    void swap(sync_set& other) noexcept {
        std::swap(ptr_, other.ptr_);
    }

    [[nodiscard]] expanse_sync_set_t* native_handle() noexcept { return ptr_; }
    [[nodiscard]] const expanse_sync_set_t* native_handle() const noexcept { return ptr_; }
    [[nodiscard]] expanse_sync_set_t* release() noexcept {
        expanse_sync_set_t* tmp = ptr_;
        ptr_ = nullptr;
        return tmp;
    }

private:
    expanse_sync_set_t* ptr_{nullptr};
};

class sync_map_reader {
public:
    sync_map_reader() noexcept : ptr_(nullptr) {}
    explicit sync_map_reader(expanse_sync_map_reader_t* ptr) noexcept : ptr_(ptr) {}

    ~sync_map_reader() noexcept {
        if (ptr_) {
            expanse_sync_map_reader_free(ptr_);
            ptr_ = nullptr;
        }
    }

    sync_map_reader(const sync_map_reader&) = delete;
    sync_map_reader& operator=(const sync_map_reader&) = delete;

    sync_map_reader(sync_map_reader&& other) noexcept : ptr_(other.ptr_) {
        other.ptr_ = nullptr;
    }

    sync_map_reader& operator=(sync_map_reader&& other) noexcept {
        if (this != &other) {
            if (ptr_) {
                expanse_sync_map_reader_free(ptr_);
            }
            ptr_       = other.ptr_;
            other.ptr_ = nullptr;
        }
        return *this;
    }

    [[nodiscard]] std::optional<uint64_t> get(uint64_t key) const noexcept {
        uint64_t out = 0;
        if (expanse_sync_map_reader_get(ptr_, key, &out)) {
            return out;
        }
        return std::nullopt;
    }

    [[nodiscard]] bool contains(uint64_t key) const noexcept {
        return get(key).has_value();
    }

    [[nodiscard]] expanse_sync_map_reader_t* native_handle() noexcept { return ptr_; }
    [[nodiscard]] const expanse_sync_map_reader_t* native_handle() const noexcept { return ptr_; }

private:
    expanse_sync_map_reader_t* ptr_{nullptr};
};

class sync_map {
public:
    sync_map() noexcept : ptr_(expanse_sync_map_new()) {}
    explicit sync_map(expanse_sync_map_t* ptr) noexcept : ptr_(ptr) {}

    ~sync_map() noexcept {
        if (ptr_) {
            expanse_sync_map_free(ptr_);
            ptr_ = nullptr;
        }
    }

    sync_map(const sync_map&) = delete;
    sync_map& operator=(const sync_map&) = delete;

    sync_map(sync_map&& other) noexcept : ptr_(other.ptr_) {
        other.ptr_ = nullptr;
    }

    sync_map& operator=(sync_map&& other) noexcept {
        if (this != &other) {
            if (ptr_) {
                expanse_sync_map_free(ptr_);
            }
            ptr_       = other.ptr_;
            other.ptr_ = nullptr;
        }
        return *this;
    }

    bool insert(uint64_t key, uint64_t value, uint64_t* old_out = nullptr) noexcept {
        return expanse_sync_map_insert(ptr_, key, value, old_out);
    }

    bool erase(uint64_t key, uint64_t* old_out = nullptr) noexcept {
        return expanse_sync_map_remove(ptr_, key, old_out);
    }

    bool remove(uint64_t key, uint64_t* old_out = nullptr) noexcept {
        return erase(key, old_out);
    }

    [[nodiscard]] std::optional<uint64_t> get(uint64_t key) const noexcept {
        uint64_t out = 0;
        if (expanse_sync_map_get(ptr_, key, &out)) {
            return out;
        }
        return std::nullopt;
    }

    [[nodiscard]] bool contains(uint64_t key) const noexcept {
        return get(key).has_value();
    }

    [[nodiscard]] uint64_t size() const noexcept {
        return expanse_sync_map_len(ptr_);
    }

    [[nodiscard]] bool empty() const noexcept {
        return size() == 0;
    }

    [[nodiscard]] sync_map_reader make_reader() const noexcept {
        return sync_map_reader(expanse_sync_map_reader_new(ptr_));
    }

    [[nodiscard]] sync_map_reader reader() const noexcept {
        return make_reader();
    }

    void swap(sync_map& other) noexcept {
        std::swap(ptr_, other.ptr_);
    }

    [[nodiscard]] expanse_sync_map_t* native_handle() noexcept { return ptr_; }
    [[nodiscard]] const expanse_sync_map_t* native_handle() const noexcept { return ptr_; }
    [[nodiscard]] expanse_sync_map_t* release() noexcept {
        expanse_sync_map_t* tmp = ptr_;
        ptr_ = nullptr;
        return tmp;
    }

private:
    expanse_sync_map_t* ptr_{nullptr};
};

} // namespace expanse
