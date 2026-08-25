//! Pillar 5: Runtime Memory Allocation Profiler
//!
//! Uses a custom GlobalAlloc hook to track exact live heap bytes allocated
//! for ExpanseMap vs hashbrown::HashMap vs std::collections::BTreeMap across
//! population scales: 10^3, 10^4, 10^5, 10^6.

use expanse_trie::map::ExpanseMap;
use hashbrown::HashMap;
use serde_json::json;
use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

struct TrackingAlloc;
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for TrackingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: Delegating directly to the System allocator
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            LIVE_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        // SAFETY: Delegating directly to the System allocator
        unsafe { System.dealloc(ptr, layout) };
    }
}

#[global_allocator]
static GLOBAL: TrackingAlloc = TrackingAlloc;

struct XorShift64(u64);
impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 { 0xFEEDBEEF_12345678 } else { seed })
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

fn measure_mem_random(pop: usize) -> (f64, f64, f64) {
    let mut rng = XorShift64::new(0xABCD_1234_5678_EF01);
    let mut keys = Vec::with_capacity(pop);
    for _ in 0..pop {
        keys.push(rng.next());
    }

    // 1. ExpanseMap
    let before_exp = LIVE_BYTES.load(Ordering::SeqCst);
    let mut exp_map = ExpanseMap::new();
    for &k in &keys {
        exp_map.insert(k, k);
    }
    let after_exp = LIVE_BYTES.load(Ordering::SeqCst);
    let exp_bytes = after_exp.saturating_sub(before_exp);
    let exp_bpk = exp_bytes as f64 / pop as f64;
    drop(exp_map);

    // 2. Hashbrown
    let before_hb = LIVE_BYTES.load(Ordering::SeqCst);
    let mut hb_map = HashMap::new();
    for &k in &keys {
        hb_map.insert(k, k);
    }
    let after_hb = LIVE_BYTES.load(Ordering::SeqCst);
    let hb_bytes = after_hb.saturating_sub(before_hb);
    let hb_bpk = hb_bytes as f64 / pop as f64;
    drop(hb_map);

    // 3. BTreeMap
    let before_bt = LIVE_BYTES.load(Ordering::SeqCst);
    let mut bt_map = BTreeMap::new();
    for &k in &keys {
        bt_map.insert(k, k);
    }
    let after_bt = LIVE_BYTES.load(Ordering::SeqCst);
    let bt_bytes = after_bt.saturating_sub(before_bt);
    let bt_bpk = bt_bytes as f64 / pop as f64;
    drop(bt_map);

    (exp_bpk, hb_bpk, bt_bpk)
}

fn measure_mem_sequential(pop: usize) -> (f64, f64, f64) {
    // 1. ExpanseMap
    let before_exp = LIVE_BYTES.load(Ordering::SeqCst);
    let mut exp_map = ExpanseMap::new();
    for i in 0..pop as u64 {
        exp_map.insert(i, i);
    }
    let after_exp = LIVE_BYTES.load(Ordering::SeqCst);
    let exp_bytes = after_exp.saturating_sub(before_exp);
    let exp_bpk = exp_bytes as f64 / pop as f64;
    drop(exp_map);

    // 2. Hashbrown
    let before_hb = LIVE_BYTES.load(Ordering::SeqCst);
    let mut hb_map = HashMap::new();
    for i in 0..pop as u64 {
        hb_map.insert(i, i);
    }
    let after_hb = LIVE_BYTES.load(Ordering::SeqCst);
    let hb_bytes = after_hb.saturating_sub(before_hb);
    let hb_bpk = hb_bytes as f64 / pop as f64;
    drop(hb_map);

    // 3. BTreeMap
    let before_bt = LIVE_BYTES.load(Ordering::SeqCst);
    let mut bt_map = BTreeMap::new();
    for i in 0..pop as u64 {
        bt_map.insert(i, i);
    }
    let after_bt = LIVE_BYTES.load(Ordering::SeqCst);
    let bt_bytes = after_bt.saturating_sub(before_bt);
    let bt_bpk = bt_bytes as f64 / pop as f64;
    drop(bt_map);

    (exp_bpk, hb_bpk, bt_bpk)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let json_mode = args.iter().any(|a| a == "--json");

    let populations = if quick {
        vec![1_000, 10_000, 100_000]
    } else {
        vec![1_000, 10_000, 100_000, 500_000]
    };

    let mut results = Vec::new();

    for &pop in &populations {
        let (rand_exp, rand_hb, rand_bt) = measure_mem_random(pop);
        let (seq_exp, seq_hb, seq_bt) = measure_mem_sequential(pop);

        results.push(json!({
            "population": pop,
            "random_keys_bytes_per_key": {
                "expanse": rand_exp,
                "hashbrown": rand_hb,
                "btree": rand_bt
            },
            "sequential_keys_bytes_per_key": {
                "expanse": seq_exp,
                "hashbrown": seq_hb,
                "btree": seq_bt
            }
        }));
    }

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&results).unwrap());
    } else {
        println!("{:#?}", results);
    }
}
