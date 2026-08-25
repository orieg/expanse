//! Pillar 3: HdrHistogram P99.99 Tail-Latency & Ingestion Cliff
//!
//! Measures per-operation latency percentiles during dynamic un-preallocated table growth:
//! - P50, P75, P90, P95, P99, P99.9, P99.99, Max
//! - Captures the SwissTable table-doubling rehash cliff vs Expanse local subexpanse growth.

use expanse_trie::map::ExpanseMap;
use hashbrown::HashMap;
use hdrhistogram::Histogram;
use std::collections::BTreeMap;
use std::time::Instant;

struct XorShift64(u64);
impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 { 0x4D5E_6F70_8192_A3B4 } else { seed })
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

fn measure_growth_latency<F: FnMut(u64, u64)>(
    mut insert_fn: F,
    keys: &[u64],
) -> Histogram<u64> {
    // 3 significant figures, tracking up to 10 seconds (10_000_000_000 ns)
    let mut hist = Histogram::<u64>::new_with_max(10_000_000_000, 3).unwrap();

    for &k in keys {
        let start = Instant::now();
        insert_fn(k, k);
        let elapsed_ns = start.elapsed().as_nanos() as u64;
        let clamped = elapsed_ns.min(9_999_999_999).max(1);
        hist.record(clamped).unwrap();
    }
    hist
}

fn extract_percentiles(hist: &Histogram<u64>) -> serde_json::Value {
    serde_json::json!({
        "p50_ns": hist.value_at_quantile(0.50),
        "p75_ns": hist.value_at_quantile(0.75),
        "p90_ns": hist.value_at_quantile(0.90),
        "p95_ns": hist.value_at_quantile(0.95),
        "p99_ns": hist.value_at_quantile(0.99),
        "p99_9_ns": hist.value_at_quantile(0.999),
        "p99_99_ns": hist.value_at_quantile(0.9999),
        "max_ns": hist.max()
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let json_mode = args.iter().any(|a| a == "--json");

    let num_keys = if quick { 100_000 } else { 1_000_000 };
    let mut rng = XorShift64::new(0xCAFE_BABE_0123_4567);
    let mut keys = Vec::with_capacity(num_keys);
    for _ in 0..num_keys {
        keys.push(rng.next());
    }

    // 1. ExpanseMap Growth
    let mut expanse_map = ExpanseMap::new();
    let hist_exp = measure_growth_latency(|k, v| {
        expanse_map.insert(k, v);
    }, &keys);

    // 2. Hashbrown Growth
    let mut hashbrown_map = HashMap::new();
    let hist_hb = measure_growth_latency(|k, v| {
        hashbrown_map.insert(k, v);
    }, &keys);

    // 3. BTreeMap Growth
    let mut btree_map = BTreeMap::new();
    let hist_bt = measure_growth_latency(|k, v| {
        btree_map.insert(k, v);
    }, &keys);

    let output = serde_json::json!({
        "total_inserts": num_keys,
        "mode": "un_preallocated_dynamic_growth",
        "expanse": extract_percentiles(&hist_exp),
        "hashbrown": extract_percentiles(&hist_hb),
        "btree": extract_percentiles(&hist_bt)
    });

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("{:#?}", output);
    }
}
