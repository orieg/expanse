//! Per-key memory across expanse occupancy: the density sawtooth
//! (`docs/ARCHITECTURE.md` §3.5), as committed data.
//!
//! Sweeps `ExpanseSet` and `ExpanseMap` over uniform random keys at nine
//! populations and three keyspace widths (64, 63 and 62 bits, by masking the
//! top key bits), plus the three construction-fixed distributions of the
//! `memory-budget` census, and reports bytes per key from the engine's own
//! byte-exact accounting. Deterministic and host-independent: no wall clock is
//! involved, so the cells reproduce to the decimal on any machine.
//!
//! Clearing one top key bit halves the number of 2-byte-prefix expanses,
//! which is arithmetically the same as doubling N; the three keyspace columns
//! are one curve in λ = N / 2^(w−48). `--json PATH` writes that curve as the
//! `density_sweep` block `scripts/generate_asset_svgs.py` renders into
//! `docs/assets/bench_density_sawtooth.svg`, and
//! `tests/test_visualizer_sync.rs` recomputes a subset of the cells from the
//! engine so the committed data cannot drift from the code.
//!
//! Run: `cargo run --release -p expanse-trie --example keyspace_density [-- --json docs/assets/data/density_sweep.json]`
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `example_keyspace_density` |
//! | `group` | 5 |
//! | `population` | 100k to 2M, at 64-, 63- and 62-bit keyspaces |
//! | `probes_and_reuse` | N/A (Memory) |
//! | `hit_rate` | N/A |
//! | `miss_gen_method` | N/A |
//! | `value_dereference` | `mem_used()` accounting |
//! | `measured_region` | Clean |
//! | `arm_symmetry` | Same PRNG and seed at every width; the width is the only variable |
//! | `statistics` | Exact byte count |
//! | `verdict` | **PASS** `[verified: RUN (fcca1c0d)]`: Deterministic density sweep; reproduces the §9.4 sweep of the HOT suite. |

use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
use expanse_trie::types::LEAF_CAP;
use std::fmt::Write as _;

struct XorShift(u64);
impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

const SEED: u64 = 0x0DDB_1A5E_5EED_0001;
const POPULATIONS: [usize; 9] = [
    100_000, 200_000, 400_000, 600_000, 800_000, 900_000, 1_000_000, 1_200_000, 2_000_000,
];
const WIDTHS: [u32; 3] = [64, 63, 62];
const ANCHOR_POPULATIONS: [usize; 4] = [100_000, 400_000, 1_000_000, 2_000_000];

/// Bytes per key for `n` uniform keys drawn at `bits` width: (set, map, distinct keys).
fn random_cell(n: usize, bits: u32) -> (f64, f64, usize) {
    let mask = if bits == 64 {
        u64::MAX
    } else {
        (1u64 << bits) - 1
    };
    let mut rng = XorShift(SEED);
    let mut set = ExpanseSet::new();
    let mut map = ExpanseMap::new();
    for _ in 0..n {
        let k = rng.next() & mask;
        set.insert(k);
        map.insert(k, !k);
    }
    let len = set.len().max(1);
    (
        set.mem_used() as f64 / len as f64,
        map.mem_used() as f64 / map.len().max(1) as f64,
        set.len() as usize,
    )
}

/// The census's construction-fixed distributions (same generators as
/// `bytes_per_key.rs`), set flavor.
fn anchor_cell(dist: &str, n: usize) -> f64 {
    let mut rng = XorShift(SEED);
    let mut set = ExpanseSet::new();
    let mut base = 0u64;
    for i in 0..n as u64 {
        let k = match dist {
            "sequential" => i,
            "sparse" => i << 40,
            "clustered" => {
                if i % 256 == 0 {
                    base = rng.next() & !0xFF;
                }
                base + (i % 256)
            }
            _ => unreachable!(),
        };
        set.insert(k);
    }
    set.mem_used() as f64 / set.len().max(1) as f64
}

fn r2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}
fn r4(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}

fn main() {
    let json_path = {
        let args: Vec<String> = std::env::args().collect();
        args.iter()
            .position(|a| a == "--json")
            .and_then(|i| args.get(i + 1).cloned())
    };

    println!("bytes/key across expanse occupancy (set flavor / map flavor); LEAF_CAP = {LEAF_CAP}");
    println!(
        "{:>10} {:>5} {:>9} {:>8} {:>10} {:>10}",
        "N", "bits", "λ", "λ/cap", "set B/key", "map B/key"
    );
    let mut cells = String::new();
    for &n in &POPULATIONS {
        for &bits in &WIDTHS {
            let lambda = n as f64 / (1u64 << (bits - 48)) as f64;
            let (set_bpk, map_bpk, distinct) = random_cell(n, bits);
            println!(
                "{n:>10} {bits:>5} {lambda:>9.2} {:>7.0}% {set_bpk:>10.2} {map_bpk:>10.2}",
                100.0 * lambda / LEAF_CAP as f64
            );
            let _ = writeln!(
                cells,
                "    {{\"n\": {n}, \"bits\": {bits}, \"lambda\": {}, \"lambda_over_leaf_cap\": {}, \
                 \"set_bpk\": {}, \"map_bpk\": {}, \"distinct_keys\": {distinct}}},",
                r4(lambda),
                r4(lambda / LEAF_CAP as f64),
                r2(set_bpk),
                r2(map_bpk)
            );
        }
    }
    println!("\nconstruction-fixed occupancy (set B/key; flat across N by construction)");
    println!(
        "{:>10} {:>12} {:>12} {:>12}",
        "N", "sequential", "clustered", "sparse i<<40"
    );
    let mut anchors = String::new();
    for &n in &ANCHOR_POPULATIONS {
        let mut row = Vec::new();
        for dist in ["sequential", "clustered", "sparse"] {
            let v = anchor_cell(dist, n);
            row.push(v);
            let _ = writeln!(
                anchors,
                "    {{\"dist\": \"{dist}\", \"n\": {n}, \"set_bpk\": {}}},",
                r2(v)
            );
        }
        println!("{n:>10} {:>12.2} {:>12.2} {:>12.2}", row[0], row[1], row[2]);
    }

    if let Some(path) = json_path {
        let commit = std::env::var("EXPANSE_COMMIT").unwrap_or_else(|_| "unknown".into());
        let doc = format!(
            "{{\n  \"meta\": {{\n    \"source\": \"crates/expanse/examples/keyspace_density.rs --json; the mechanism is docs/ARCHITECTURE.md §3.5 and the original sweep docs/benchmarks/hot_comparison/METHODOLOGY.md §9.4\",\n    \"instrument\": \"mem_used() deterministic byte accounting — host-independent; no wall clock\",\n    \"generator\": \"XorShift64(0x0DDB_1A5E_5EED_0001), each draw masked to the keyspace width; same seed at every width\",\n    \"lambda\": \"N / 2^(bits - 48): mean population of a 2-byte-prefix expanse once the top two key bytes saturate\",\n    \"leaf_cap\": {LEAF_CAP},\n    \"commit\": \"{commit}\",\n    \"workload_id\": \"example_keyspace_density\"\n  }},\n  \"cells\": [\n{}  ],\n  \"anchors\": [\n{}  ]\n}}\n",
            cells.trim_end_matches(",\n").to_string() + "\n",
            anchors.trim_end_matches(",\n").to_string() + "\n"
        );
        std::fs::write(&path, doc).expect("write --json output");
        println!("\nwrote {path}");
    }
}
