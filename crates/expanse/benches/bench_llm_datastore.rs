//! Pillar B: Dynamic Speculative Datastore Scaling vs Native Suffix Array.
//!
//! Compares:
//! - `ExpanseStrMap`: 1 reversed max-length window per position (7-bit NUL-free encoding).
//! - Native Suffix Array: Static contiguous array sorted by reversed prefix windows.
//!
//! Evaluates:
//! 1. Memory density (Bytes per token).
//! 2. Static longest-match query latency.
//! 3. Continuous incremental insertion throughput vs Suffix Array periodic rebuild.
//! 4. Identification of the crossover batch size B_crossover.
#![allow(missing_docs)]

use expanse_trie::strmap::ExpanseStrMap;
use serde_json::json;
use std::time::Instant;

fn encode_token_7bit(tok: u32) -> [u8; 3] {
    let b0 = (((tok >> 14) & 0x7F) + 1) as u8;
    let b1 = (((tok >> 7) & 0x7F) + 1) as u8;
    let b2 = ((tok & 0x7F) + 1) as u8;
    [b0, b1, b2]
}

fn encode_rev_window(tokens: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(tokens.len() * 3);
    for &tok in tokens.iter().rev() {
        out.extend_from_slice(&encode_token_7bit(tok));
    }
    out
}

struct NativeSuffixArray {
    max_suffix_len: usize,
    tokens: Vec<u32>,
    sa: Vec<usize>,
}

impl NativeSuffixArray {
    fn new(max_suffix_len: usize) -> Self {
        Self {
            max_suffix_len,
            tokens: Vec::new(),
            sa: Vec::new(),
        }
    }

    fn build_from_tokens(&mut self, tokens: &[u32]) {
        self.tokens = tokens.to_vec();
        let n = self.tokens.len();
        if n < 2 {
            self.sa.clear();
            return;
        }
        let mut sa: Vec<usize> = (0..n - 1).collect();
        sa.sort_by(|&a, &b| {
            let start_a = a.saturating_add(1).saturating_sub(self.max_suffix_len);
            let start_b = b.saturating_add(1).saturating_sub(self.max_suffix_len);
            let slice_a = &self.tokens[start_a..=a];
            let slice_b = &self.tokens[start_b..=b];
            slice_a.iter().rev().cmp(slice_b.iter().rev())
        });
        self.sa = sa;
    }

    fn memory_bytes(&self) -> usize {
        self.tokens.len() * std::mem::size_of::<u32>()
            + self.sa.len() * std::mem::size_of::<usize>()
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let json_mode = args.iter().any(|a| a == "--json");

    let populations: Vec<usize> = if quick {
        vec![10_000, 50_000]
    } else {
        vec![100_000, 500_000, 1_000_000]
    };

    let mut results = serde_json::Map::new();

    for &n in &populations {
        eprintln!("  --> Benchmarking population N = {n} tokens...");
        // Generate pseudo-random token stream
        let mut tokens: Vec<u32> = Vec::with_capacity(n);
        let mut lcg: u64 = 424242;
        for _ in 0..n {
            lcg = lcg.wrapping_mul(6364136223846793005).wrapping_add(1);
            tokens.push((lcg >> 33) as u32 % 128_000);
        }

        // 1. Build Suffix Array
        let t0 = Instant::now();
        let mut sa = NativeSuffixArray::new(16);
        sa.build_from_tokens(&tokens);
        let sa_rebuild_time = t0.elapsed();
        let sa_mem = sa.memory_bytes();
        let sa_bytes_per_tok = sa_mem as f64 / n as f64;

        // 2. Build ExpanseStrMap (1 key per position)
        let mut exp_map = ExpanseStrMap::new();
        let t0 = Instant::now();
        for i in 0..n - 1 {
            let start = i.saturating_add(1).saturating_sub(16);
            let k = encode_rev_window(&tokens[start..=i]);
            exp_map.insert(&k, i as u64);
        }
        let exp_build_time = t0.elapsed();
        let exp_mem = exp_map.mem_used();
        let exp_bytes_per_tok = exp_mem as f64 / n as f64;

        // 3. Incremental Streaming Insertion
        let num_stream_inserts = 1000.min(n - 2);
        let t0 = Instant::now();
        for i in 0..num_stream_inserts {
            let start = i.saturating_add(1).saturating_sub(16);
            let mut w = tokens[start..=i].to_vec();
            if let Some(last) = w.last_mut() {
                *last ^= 0x1F;
            }
            let k = encode_rev_window(&w);
            exp_map.insert(&k, i as u64);
        }
        let exp_stream_elapsed = t0.elapsed();
        let exp_streaming_tps =
            num_stream_inserts as f64 / exp_stream_elapsed.as_secs_f64().max(1e-9);

        // 4. Suffix Array Rebuild TPS (amortized single-token insert by full rebuild)
        let sa_rebuild_tps = n as f64 / sa_rebuild_time.as_secs_f64().max(1e-9);

        // 5. Crossover batch size B_crossover:
        // Expanse insert time per token = exp_stream_elapsed / num_stream_inserts
        // SA rebuild time = sa_rebuild_time
        // Crossover occurs where B * t_expanse_insert = t_sa_rebuild
        let t_exp_per_insert = exp_stream_elapsed.as_secs_f64() / num_stream_inserts as f64;
        let b_crossover = (sa_rebuild_time.as_secs_f64() / t_exp_per_insert.max(1e-12)) as u64;

        results.insert(
            n.to_string(),
            json!({
                "population_tokens": n,
                "suffix_array": {
                    "memory_mb": (sa_mem as f64 / (1024.0 * 1024.0) * 100.0).round() / 100.0,
                    "bytes_per_token": (sa_bytes_per_tok * 10.0).round() / 10.0,
                    "rebuild_time_ms": (sa_rebuild_time.as_secs_f64() * 1000.0 * 10.0).round() / 10.0,
                    "rebuild_tps": sa_rebuild_tps.round(),
                },
                "expanse_strmap": {
                    "memory_mb": (exp_mem as f64 / (1024.0 * 1024.0) * 100.0).round() / 100.0,
                    "bytes_per_token": (exp_bytes_per_tok * 10.0).round() / 10.0,
                    "build_time_ms": (exp_build_time.as_secs_f64() * 1000.0 * 10.0).round() / 10.0,
                    "streaming_insert_tps": exp_streaming_tps.round(),
                    "memory_overhead_vs_sa_x": (exp_mem as f64 / sa_mem.max(1) as f64 * 10.0).round() / 10.0,
                    "crossover_batch_size_tokens": b_crossover,
                }
            }),
        );
    }

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&results).unwrap());
    } else {
        println!("{:#?}", results);
    }
}
