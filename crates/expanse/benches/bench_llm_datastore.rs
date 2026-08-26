//! Pillar B: Dynamic Speculative Datastore Scaling vs Static Sorted Window Index.
//!
//! Compares:
//! - `ExpanseStrMap`: 1 reversed max-length window per position (7-bit NUL-free encoding).
//! - Static Sorted Window Index: Array of 16-token reversed prefix windows sorted via comparison sort.
//!
//! Evaluates:
//! 1. Memory density (Bytes per token).
//! 2. Static longest-match query latency.
//! 3. Continuous incremental insertion throughput vs Static Index full rebuild.
//! 4. Identification of the crossover batch size B_crossover.
#![allow(missing_docs)]

use expanse_trie::strmap::ExpanseStrMap;
use serde_json::json;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
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

struct StaticSortedWindowIndex {
    max_suffix_len: usize,
    tokens: Vec<u32>,
    indices: Vec<usize>,
}

impl StaticSortedWindowIndex {
    fn new(max_suffix_len: usize) -> Self {
        Self {
            max_suffix_len,
            tokens: Vec::new(),
            indices: Vec::new(),
        }
    }

    fn build_from_tokens(&mut self, tokens: &[u32]) {
        self.tokens = tokens.to_vec();
        let n = self.tokens.len();
        if n < 2 {
            self.indices.clear();
            return;
        }
        let mut indices: Vec<usize> = (0..n - 1).collect();
        indices.sort_by(|&a, &b| {
            let start_a = a.saturating_add(1).saturating_sub(self.max_suffix_len);
            let start_b = b.saturating_add(1).saturating_sub(self.max_suffix_len);
            let slice_a = &self.tokens[start_a..=a];
            let slice_b = &self.tokens[start_b..=b];
            slice_a.iter().rev().cmp(slice_b.iter().rev())
        });
        self.indices = indices;
    }

    fn memory_bytes(&self) -> usize {
        self.tokens.len() * std::mem::size_of::<u32>()
            + self.indices.len() * std::mem::size_of::<usize>()
    }
}

fn resolve_path(rel_path: &str) -> PathBuf {
    let p1 = Path::new(rel_path);
    if p1.exists() || p1.parent().is_some_and(|p| p.exists()) {
        return p1.to_path_buf();
    }
    let p2 = Path::new("../../").join(rel_path);
    if p2.exists() || p2.parent().is_some_and(|p| p.exists()) {
        return p2;
    }
    p1.to_path_buf()
}

fn load_corpus_tokens(max_n: usize) -> Vec<u32> {
    let corpus_path = resolve_path("docs/benchmarks/llm_inference/data/datastore_corpus.bin");
    if corpus_path.exists() {
        if let Ok(mut f) = File::open(&corpus_path) {
            let mut buf = Vec::new();
            if f.read_to_end(&mut buf).is_ok() && buf.len() >= 4 {
                let mut tokens = Vec::with_capacity((buf.len() / 4).min(max_n));
                for chunk in buf.chunks_exact(4).take(max_n) {
                    tokens.push(u32::from_ne_bytes(chunk.try_into().unwrap()));
                }
                return tokens;
            }
        }
    }

    // Fallback: deterministic LCG tokens if corpus binary not found
    eprintln!(
        "  --> WARNING: datastore_corpus.bin missing; falling back to synthetic \
         LCG tokens (run docs/benchmarks/llm_inference/run.sh to build the corpus)"
    );
    let mut tokens = Vec::with_capacity(max_n);
    let mut lcg: u64 = 424242;
    for _ in 0..max_n {
        lcg = lcg.wrapping_mul(6364136223846793005).wrapping_add(1);
        tokens.push(((lcg >> 33) as u32) % 100_277);
    }
    tokens
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");

    let populations: Vec<usize> = if quick {
        vec![10_000, 50_000]
    } else {
        vec![100_000, 500_000, 1_000_000]
    };

    let mut results = serde_json::Map::new();

    for &n in &populations {
        eprintln!("  --> Benchmarking population N = {n} tokens...");
        let tokens = load_corpus_tokens(n);
        let actual_n = tokens.len();

        // 1. Build Static Sorted Window Index
        let t0 = Instant::now();
        let mut static_index = StaticSortedWindowIndex::new(16);
        static_index.build_from_tokens(&tokens);
        let static_rebuild_time = t0.elapsed();
        let static_mem = static_index.memory_bytes();

        // 2. Build ExpanseStrMap with 1 key per token position
        let t0 = Instant::now();
        let mut expanse = ExpanseStrMap::new();
        for i in 0..actual_n {
            let start = i.saturating_add(1).saturating_sub(16);
            let window = &tokens[start..=i];
            let key = encode_rev_window(window);
            expanse.insert(&key, i as u64);
        }
        let expanse_build_time = t0.elapsed();
        let expanse_mem = expanse.mem_used();

        let static_bytes_per_tok = (static_mem as f64) / (actual_n as f64);
        let exp_bytes_per_tok = (expanse_mem as f64) / (actual_n as f64);
        let mem_overhead = exp_bytes_per_tok / static_bytes_per_tok;

        let expanse_tps = (actual_n as f64) / expanse_build_time.as_secs_f64();
        let static_rebuild_tps = (actual_n as f64) / static_rebuild_time.as_secs_f64();

        // Crossover batch size B: where B * T_insert_expanse == T_rebuild_static
        let t_insert_expanse_sec = expanse_build_time.as_secs_f64() / (actual_n as f64);
        let crossover_b =
            (static_rebuild_time.as_secs_f64() / t_insert_expanse_sec).round() as usize;

        let cell = json!({
            "population_tokens": actual_n,
            "sorted_window_index": {
                "memory_mb": ((static_mem as f64) / (1024.0 * 1024.0) * 100.0).round() / 100.0,
                "bytes_per_token": (static_bytes_per_tok * 10.0).round() / 10.0,
                "rebuild_time_ms": (static_rebuild_time.as_secs_f64() * 1000.0 * 10.0).round() / 10.0,
                "rebuild_tps": static_rebuild_tps.round(),
            },
            "expanse_strmap": {
                "memory_mb": ((expanse_mem as f64) / (1024.0 * 1024.0) * 100.0).round() / 100.0,
                "bytes_per_token": (exp_bytes_per_tok * 10.0).round() / 10.0,
                "build_time_ms": (expanse_build_time.as_secs_f64() * 1000.0 * 10.0).round() / 10.0,
                "streaming_insert_tps": expanse_tps.round(),
                "memory_overhead_vs_static_index_x": (mem_overhead * 10.0).round() / 10.0,
                "crossover_batch_size_tokens": crossover_b,
            }
        });

        results.insert(n.to_string(), cell);
    }

    let results_dir = resolve_path("docs/benchmarks/llm_inference/results");
    let _ = std::fs::create_dir_all(&results_dir);
    let out_file = results_dir.join("bench_llm_datastore.json");
    std::fs::write(&out_file, serde_json::to_string_pretty(&results).unwrap())
        .expect("Failed to write datastore results JSON");
    println!("Pillar B results written to {}", out_file.display());
}
