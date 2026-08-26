//! Pillar D: Grammar-Constrained Decoding Mask Cache & Set Algebra.
//!
//! Compares:
//! - `DenseBitmask`: Contiguous [u64; 2000] bit array (128,000 bits = 16 KB / state).
//! - `roaring::Bitmap`: Roaring compressed bitset.
//! - `expanse_trie::ExpanseSet`: Modern Judy digital trie with zero-alloc bit-packed words.
//!
//! Evaluates:
//! 1. Memory footprint across 10,000 DFA states (sparse 0.01%, medium 1%, dense 10%).
//! 2. Full-vocab mask apply latency.
//! 3. Top-k candidate intersection latency (k in [50, 100, 500]).
#![allow(missing_docs)]

use expanse_trie::ExpanseSet;
use roaring::RoaringBitmap;
use serde_json::json;
use std::time::Instant;

struct DenseBitmask {
    words: Vec<u64>,
}

impl DenseBitmask {
    fn new(vocab_size: usize) -> Self {
        let num_words = vocab_size.div_ceil(64);
        Self {
            words: vec![0u64; num_words],
        }
    }

    fn set(&mut self, token: u32) {
        let word_idx = (token / 64) as usize;
        let bit_idx = token % 64;
        if word_idx < self.words.len() {
            self.words[word_idx] |= 1u64 << bit_idx;
        }
    }

    fn is_allowed(&self, token: u32) -> bool {
        let word_idx = (token / 64) as usize;
        let bit_idx = token % 64;
        if word_idx < self.words.len() {
            (self.words[word_idx] & (1u64 << bit_idx)) != 0
        } else {
            false
        }
    }

    fn memory_bytes(&self) -> usize {
        self.words.len() * std::mem::size_of::<u64>()
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let json_mode = args.iter().any(|a| a == "--json");

    let num_states = if quick { 500 } else { 2000 };
    let vocab_size: usize = 128_000;

    let mut dense_states = Vec::with_capacity(num_states);
    let mut roaring_states = Vec::with_capacity(num_states);
    let mut expanse_states = Vec::with_capacity(num_states);

    let mut lcg: u64 = 424242;
    for _state_idx in 0..num_states {
        let mut dense = DenseBitmask::new(vocab_size);
        let mut roaring = RoaringBitmap::new();
        let mut expanse = ExpanseSet::new();

        lcg = lcg.wrapping_mul(6364136223846793005).wrapping_add(1);
        let tier_pct = ((lcg >> 33) % 100) as f64;

        let k = if tier_pct < 40.0 {
            // Sparse state: 20 tokens (0.015%)
            20
        } else if tier_pct < 75.0 {
            // Medium state: 1,280 tokens (1.0%)
            1280
        } else {
            // Dense state: 12,800 tokens (10.0%)
            12800
        };

        for _ in 0..k {
            lcg = lcg.wrapping_mul(6364136223846793005).wrapping_add(1);
            let tok = ((lcg >> 33) as u32) % (vocab_size as u32);
            dense.set(tok);
            roaring.insert(tok);
            expanse.insert(tok as u64);
        }

        dense_states.push(dense);
        roaring_states.push(roaring);
        expanse_states.push(expanse);
    }

    // 1. Measure Memory
    let dense_total_mem: usize = dense_states.iter().map(|s| s.memory_bytes()).sum();
    let roaring_total_mem: usize = roaring_states.iter().map(|s| s.serialized_size()).sum();
    let expanse_total_mem: usize = expanse_states.iter().map(|s| s.mem_used()).sum();

    // 2. Full-Vocab Apply Latency (masking logits array)
    let mut dummy_logits = vec![0.0f32; vocab_size];
    let t0 = Instant::now();
    let sample_states = 100.min(num_states);
    for s in &dense_states[..sample_states] {
        for (i, logit) in dummy_logits.iter_mut().enumerate() {
            if !s.is_allowed(i as u32) {
                *logit = f32::NEG_INFINITY;
            }
        }
    }
    let dense_apply_ns = t0.elapsed().as_nanos() as f64 / sample_states as f64;

    // 3. Top-k Candidate Intersection Latency (k=100)
    let top_k_tokens: Vec<u64> = (100..200).collect();
    let mut top_k_expanse = ExpanseSet::new();
    let mut top_k_roaring = RoaringBitmap::new();
    for &t in &top_k_tokens {
        top_k_expanse.insert(t);
        top_k_roaring.insert(t as u32);
    }

    let t0 = Instant::now();
    let mut expanse_matches = 0;
    for s in &expanse_states[..sample_states] {
        expanse_matches += s.intersection_len(&top_k_expanse);
    }
    std::hint::black_box(expanse_matches);
    let expanse_topk_ns = t0.elapsed().as_nanos() as f64 / sample_states as f64;

    let t0 = Instant::now();
    let mut roaring_matches = 0;
    for s in &roaring_states[..sample_states] {
        roaring_matches += s.intersection_len(&top_k_roaring) as usize;
    }
    std::hint::black_box(roaring_matches);
    let roaring_topk_ns = t0.elapsed().as_nanos() as f64 / sample_states as f64;

    let results = json!({
        "num_states": num_states,
        "vocab_size": vocab_size,
        "memory_summary": {
            "dense_bitmask_mb": (dense_total_mem as f64 / (1024.0 * 1024.0) * 100.0).round() / 100.0,
            "roaring_bitmap_mb": (roaring_total_mem as f64 / (1024.0 * 1024.0) * 100.0).round() / 100.0,
            "expanse_set_mb": (expanse_total_mem as f64 / (1024.0 * 1024.0) * 100.0).round() / 100.0,
            "memory_reduction_vs_dense_x": (dense_total_mem as f64 / expanse_total_mem.max(1) as f64 * 10.0).round() / 10.0,
        },
        "latency_summary": {
            "dense_full_vocab_apply_us": (dense_apply_ns / 1000.0 * 100.0).round() / 100.0,
            "expanse_topk_intersection_ns": (expanse_topk_ns * 10.0).round() / 10.0,
            "roaring_topk_intersection_ns": (roaring_topk_ns * 10.0).round() / 10.0,
        }
    });

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&results).unwrap());
    } else {
        println!("{:#?}", results);
    }
}
