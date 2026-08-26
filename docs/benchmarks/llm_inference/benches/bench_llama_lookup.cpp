/**
 * Pillar 3: Native C++ llama.cpp Lookup Decoding Benchmark.
 *
 * Compares:
 * 1. Stock llama.cpp ngram-cache (std::unordered_map<string, unordered_map<int32_t, int32_t>>)
 * 2. Expanse C++20 str_map (expanse::str_map via include/expanse.hpp with 7-bit NUL-free encoding)
 *
 * Measures:
 * - Candidate draft latency (ns/query)
 * - Cache update latency (ns/update)
 * - Memory footprint (MB) across context lengths (4k, 32k, 128k tokens)
 */

#include <iostream>
#include <vector>
#include <string>
#include <unordered_map>
#include <chrono>
#include <random>
#include <fstream>
#include <iomanip>
#include <cstdint>
#include <algorithm>

#include "expanse.hpp"

// -----------------------------------------------------------------------------
// 7-Bit NUL-Free Token Byte Encoding for 1..4 Token N-Grams
// -----------------------------------------------------------------------------
static inline std::string encode_ngram_7bit(const int32_t* tokens, size_t n) {
    std::string s;
    s.reserve(n * 3);
    for (size_t i = 0; i < n; ++i) {
        uint32_t tok = static_cast<uint32_t>(tokens[i]);
        uint8_t b0 = static_cast<uint8_t>(((tok >> 14) & 0x7F) + 1);
        uint8_t b1 = static_cast<uint8_t>(((tok >> 7) & 0x7F) + 1);
        uint8_t b2 = static_cast<uint8_t>((tok & 0x7F) + 1);
        s.push_back(static_cast<char>(b0));
        s.push_back(static_cast<char>(b1));
        s.push_back(static_cast<char>(b2));
    }
    return s;
}

// -----------------------------------------------------------------------------
// 1. Stock llama.cpp Ngram Cache (replicated from common/ngram-cache.cpp)
// -----------------------------------------------------------------------------
class StockLlamaNgramCache {
public:
    using token_t = int32_t;
    using token_hash = std::unordered_map<token_t, int32_t>;
    using ngram_hash = std::unordered_map<std::string, token_hash>;

    ngram_hash map;
    int32_t ngram_min = 1;
    int32_t ngram_max = 4;

    void update(const std::vector<token_t>& history, size_t pos) {
        if (pos < 1 || pos >= history.size()) return;
        token_t next_tok = history[pos];
        for (int32_t n = ngram_min; n <= ngram_max; ++n) {
            if (static_cast<int32_t>(pos) < n) break;
            std::string key = encode_ngram_7bit(&history[pos - n], n);
            map[key][next_tok]++;
        }
    }

    std::vector<token_t> draft(const std::vector<token_t>& context, size_t draft_len = 4) {
        std::vector<token_t> result;
        std::vector<token_t> cur = context;

        for (size_t d = 0; d < draft_len; ++d) {
            token_t best_tok = -1;
            int32_t max_count = 0;

            for (int32_t n = std::min<int32_t>(ngram_max, cur.size()); n >= ngram_min; --n) {
                std::string key = encode_ngram_7bit(&cur[cur.size() - n], n);
                auto it = map.find(key);
                if (it != map.end() && !it->second.empty()) {
                    for (const auto& [tok, cnt] : it->second) {
                        if (cnt > max_count) {
                            max_count = cnt;
                            best_tok = tok;
                        }
                    }
                    if (best_tok != -1) break;
                }
            }

            if (best_tok != -1) {
                result.push_back(best_tok);
                cur.push_back(best_tok);
            } else {
                break;
            }
        }
        return result;
    }
};

// -----------------------------------------------------------------------------
// 2. Expanse C++20 Ngram Cache
// -----------------------------------------------------------------------------
class ExpanseLlamaNgramCache {
public:
    using token_t = int32_t;
    expanse::str_map<uint64_t> map;
    int32_t ngram_min = 1;
    int32_t ngram_max = 4;

    void update(const std::vector<token_t>& history, size_t pos) {
        if (pos < 1 || pos >= history.size()) return;
        token_t next_tok = history[pos];
        for (int32_t n = ngram_min; n <= ngram_max; ++n) {
            if (static_cast<int32_t>(pos) < n) break;
            std::string key = encode_ngram_7bit(&history[pos - n], n);
            // Append next token to key to store exact (ngram + token) frequency
            std::string full_key = key + encode_ngram_7bit(&next_tok, 1);
            uint64_t cnt = map.get(full_key).value_or(0);
            map.insert(full_key, cnt + 1);
        }
    }

    std::vector<token_t> draft(const std::vector<token_t>& context, size_t draft_len = 4) {
        std::vector<token_t> result;
        std::vector<token_t> cur = context;

        for (size_t d = 0; d < draft_len; ++d) {
            token_t best_tok = -1;
            uint64_t max_count = 0;

            for (int32_t n = std::min<int32_t>(ngram_max, cur.size()); n >= ngram_min; --n) {
                std::string pfx = encode_ngram_7bit(&cur[cur.size() - n], n);
                // Scan subexpanse matching pfx
                auto it = map.next_at_or_after(pfx);
                while (it.has_value()) {
                    const auto& [matched_k, cnt] = *it;
                    if (matched_k.size() != pfx.size() + 3 || matched_k.compare(0, pfx.size(), pfx) != 0) {
                        break; // left prefix range
                    }
                    if (cnt > max_count) {
                        max_count = cnt;
                        // Decode token from last 3 bytes
                        uint8_t b0 = static_cast<uint8_t>(matched_k[pfx.size()]);
                        uint8_t b1 = static_cast<uint8_t>(matched_k[pfx.size() + 1]);
                        uint8_t b2 = static_cast<uint8_t>(matched_k[pfx.size() + 2]);
                        best_tok = static_cast<token_t>(((b0 - 1) << 14) | ((b1 - 1) << 7) | (b2 - 1));
                    }
                    it = map.next(matched_k);
                }

                if (best_tok != -1) break;
            }

            if (best_tok != -1) {
                result.push_back(best_tok);
                cur.push_back(best_tok);
            } else {
                break;
            }
        }
        return result;
    }

    size_t memory_used() const noexcept {
        return map.mem_used();
    }
};

int main(int argc, char** argv) {
    bool quick = false;
    for (int i = 1; i < argc; ++i) {
        if (std::string(argv[i]) == "--quick") quick = true;
    }

    std::vector<size_t> contexts = quick ? std::vector<size_t>{4000, 16000} : std::vector<size_t>{4000, 32000, 128000};

    std::cout << "Running Pillar 3 Native C++ llama.cpp Lookup Decoding Benchmark...\n";

    std::mt19937_64 rng(42);
    std::uniform_int_distribution<int32_t> tok_dist(0, 32000);

    std::ofstream out("docs/benchmarks/llm_inference/results/bench_llama_lookup.json");
    out << "{\n";

    for (size_t ci = 0; ci < contexts.size(); ++ci) {
        size_t N = contexts[ci];
        std::cout << "  --> Testing context length N = " << N << " tokens...\n";

        // Generate synthetic token stream with repeated n-grams
        std::vector<int32_t> tokens(N);
        for (size_t i = 0; i < N; ++i) {
            if (i > 50 && (rng() % 100) < 60) {
                // Repeat earlier pattern
                size_t prev = rng() % (i - 20);
                tokens[i] = tokens[prev];
            } else {
                tokens[i] = tok_dist(rng);
            }
        }

        // 1. Stock llama.cpp Cache Benchmark
        StockLlamaNgramCache stock_cache;
        auto t0 = std::chrono::high_resolution_clock::now();
        for (size_t i = 1; i < N; ++i) {
            stock_cache.update(tokens, i);
        }
        auto t1 = std::chrono::high_resolution_clock::now();
        double stock_update_ns = std::chrono::duration_cast<std::chrono::nanoseconds>(t1 - t0).count() / static_cast<double>(N);

        // Benchmark Draft Query
        size_t n_queries = std::min<size_t>(1000, N - 10);
        t0 = std::chrono::high_resolution_clock::now();
        size_t stock_drafted = 0;
        for (size_t q = 0; q < n_queries; ++q) {
            std::vector<int32_t> ctx(tokens.begin() + q, tokens.begin() + q + 10);
            auto drafts = stock_cache.draft(ctx, 4);
            stock_drafted += drafts.size();
        }
        t1 = std::chrono::high_resolution_clock::now();
        double stock_draft_us = (std::chrono::duration_cast<std::chrono::nanoseconds>(t1 - t0).count() / static_cast<double>(n_queries)) / 1000.0;

        // 2. Expanse Cache Benchmark
        ExpanseLlamaNgramCache expanse_cache;
        t0 = std::chrono::high_resolution_clock::now();
        for (size_t i = 1; i < N; ++i) {
            expanse_cache.update(tokens, i);
        }
        t1 = std::chrono::high_resolution_clock::now();
        double expanse_update_ns = std::chrono::duration_cast<std::chrono::nanoseconds>(t1 - t0).count() / static_cast<double>(N);

        t0 = std::chrono::high_resolution_clock::now();
        size_t expanse_drafted = 0;
        for (size_t q = 0; q < n_queries; ++q) {
            std::vector<int32_t> ctx(tokens.begin() + q, tokens.begin() + q + 10);
            auto drafts = expanse_cache.draft(ctx, 4);
            expanse_drafted += drafts.size();
        }
        t1 = std::chrono::high_resolution_clock::now();
        double expanse_draft_us = (std::chrono::duration_cast<std::chrono::nanoseconds>(t1 - t0).count() / static_cast<double>(n_queries)) / 1000.0;

        double expanse_mem_mb = expanse_cache.memory_used() / (1024.0 * 1024.0);

        out << "  \"" << N << "\": {\n"
            << "    \"context_tokens\": " << N << ",\n"
            << "    \"stock_llama_cache\": {\n"
            << "      \"update_latency_ns\": " << std::fixed << std::setprecision(1) << stock_update_ns << ",\n"
            << "      \"draft_latency_us\": " << std::fixed << std::setprecision(2) << stock_draft_us << ",\n"
            << "      \"total_drafted\": " << stock_drafted << "\n"
            << "    },\n"
            << "    \"expanse_llama_cache\": {\n"
            << "      \"update_latency_ns\": " << std::fixed << std::setprecision(1) << expanse_update_ns << ",\n"
            << "      \"draft_latency_us\": " << std::fixed << std::setprecision(2) << expanse_draft_us << ",\n"
            << "      \"memory_mb\": " << std::fixed << std::setprecision(2) << expanse_mem_mb << ",\n"
            << "      \"total_drafted\": " << expanse_drafted << "\n"
            << "    }\n"
            << "  }" << (ci + 1 < contexts.size() ? "," : "") << "\n";
    }

    out << "}\n";
    out.close();

    std::cout << "Pillar 3 results written to docs/benchmarks/llm_inference/results/bench_llama_lookup.json\n";
    return 0;
}
