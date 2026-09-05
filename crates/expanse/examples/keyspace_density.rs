//! Per-key memory across expanse occupancy: the density sawtooth
//! (`docs/ARCHITECTURE.md` §3.5), as committed data.
//!
//! Sweeps `ExpanseSet` and `ExpanseMap` over uniform random keys across
//! populations and keyspace widths (by masking the top key bits), plus the
//! three construction-fixed distributions of the `memory-budget` census, and
//! reports bytes per key from the engine's own byte-exact accounting.
//! Deterministic and host-independent: no wall clock is involved, so the
//! cells reproduce to the decimal on any machine.
//!
//! Clearing one top key bit halves the number of 2-byte-prefix expanses,
//! which is arithmetically the same as doubling N; every keyspace column is
//! one curve in λ = N / 2^(w−48). The sweep has three parts:
//!
//! * the original 64/63/62-bit grid, λ from 1.53 to 122;
//! * a finer 64-bit ladder through the first tooth (λ 16.8 to 39.7), so the
//!   trough and the knee are located to ±0.1M rather than the 1.2M → 2M gap;
//! * exact-λ cells at 27, 40 and 58 (63- and 62-bit equivalents), where a
//!   `LEAF_CAP = 48` build is predicted to put its trough and its cascade ramp;
//! * 2M keys at 58-, 57-, 56- and 55-bit widths (λ 1,953 to 15,625) plus 1.2M,
//!   1.7M and 2.7M at 56 bits, one byte level down, where the sub-expanses
//!   below a cascaded expanse approach `LEAF_CAP` in turn — the second tooth,
//!   at λ ≈ 256 × `LEAF_CAP`.
//!
//! Three cells (1M @64, 2M @64, 800k @62) and the seven second-tooth cells
//! additionally emit the node census — `ExpanseStats` counts, per-level
//! branch and leaf histograms, the leaf population histogram, and
//! `NodeBytes`, the per-form byte attribution that sums to `mem_used()`
//! exactly. The first three are also re-run under a second PRNG seed.
//!
//! `--json PATH` writes all of it as the `density_sweep` block
//! `scripts/generate_asset_svgs.py` renders into
//! `docs/assets/bench_density_sawtooth.svg`, and
//! `tests/test_visualizer_sync.rs` recomputes a subset of the cells from the
//! engine so the committed data cannot drift from the code. `--census-only`
//! skips the sweep and prints just the three census cells.
//!
//! Run: `cargo run --release -p expanse-trie --example keyspace_density [-- --json docs/assets/data/density_sweep.json]`
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `example_keyspace_density` |
//! | `group` | 5 |
//! | `population` | 100k to 2.6M at 64 bits; 100k to 2M at 63 and 62 bits (plus exact-λ 27/40/58 cells); 2M at 58, 57 and 55 bits and 1.2M to 2.7M at 56 bits |
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
use expanse_trie::types::{BITMAP_TO_UNCOMPRESSED_THRESHOLD, LEAF_CAP};
use expanse_trie::validate::ExpanseStats;
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

/// The seed every committed random cell in this repository draws from.
const SEED: u64 = 0x0DDB_1A5E_5EED_0001;
/// A second seed for the sensitivity cells: same generator, different stream.
const SEED_B: u64 = 0x5EED_B0B5_C0FF_EE02;

/// The original grid: nine populations at three widths.
const POPULATIONS: [usize; 9] = [
    100_000, 200_000, 400_000, 600_000, 800_000, 900_000, 1_000_000, 1_200_000, 2_000_000,
];
const WIDTHS: [u32; 3] = [64, 63, 62];
/// The finer 64-bit ladder through the first tooth.
const FINE_64: [usize; 8] = [
    1_100_000, 1_300_000, 1_400_000, 1_600_000, 1_800_000, 2_200_000, 2_400_000, 2_600_000,
];
/// Exact-λ cells at 27, 40 and 58 (63- and 62-bit equivalents of 1.77M,
/// 2.62M and 3.8M @64), placed where the `LEAF_CAP = 48` control predicts its
/// trough (λ ≈ 27) and its 10%→90% cascade ramp (λ ≈ 40 → 58).
const EXACT_LAMBDA: [(usize, u32); 3] = [(884_736, 63), (1_310_720, 63), (950_272, 62)];
/// The second tooth: one byte level down. 2M at 58..55 bits spans λ 1,953 to
/// 15,625; 1.2M, 1.7M and 2.7M at 56 bits (λ 4,688, 6,641 and 10,547) bracket
/// the predicted second trough near λ ≈ 4,700 and the ramp between λ ≈ 6,600
/// and 10,400, where the sub-expanse occupancy λ / 256 crosses `LEAF_CAP`.
const SECOND_CLIFF: [(usize, u32); 7] = [
    (2_000_000, 58),
    (2_000_000, 57),
    (1_200_000, 56),
    (1_700_000, 56),
    (2_000_000, 56),
    (2_700_000, 56),
    (2_000_000, 55),
];
/// The seed-sensitivity cells (N, bits): the three first-tooth census cells.
const SEEDS: [(usize, u32); 3] = [(1_000_000, 64), (2_000_000, 64), (800_000, 62)];
/// The census cells: the three above, then the four second-tooth cells, so
/// whether `BranchU` nodes appear one byte level down is counted, not argued.
const CENSUS: [(usize, u32); 10] = [
    (1_000_000, 64),
    (2_000_000, 64),
    (800_000, 62),
    (2_000_000, 58),
    (2_000_000, 57),
    (1_200_000, 56),
    (1_700_000, 56),
    (2_000_000, 56),
    (2_700_000, 56),
    (2_000_000, 55),
];
const ANCHOR_POPULATIONS: [usize; 4] = [100_000, 400_000, 1_000_000, 2_000_000];

fn mask(bits: u32) -> u64 {
    if bits == 64 {
        u64::MAX
    } else {
        (1u64 << bits) - 1
    }
}

fn lambda(n: usize, bits: u32) -> f64 {
    n as f64 / (1u64 << (bits - 48)) as f64
}

/// Builds both flavors from `n` draws at `bits` width under `seed`.
fn build(n: usize, bits: u32, seed: u64) -> (ExpanseSet, ExpanseMap) {
    let m = mask(bits);
    let mut rng = XorShift(seed);
    let mut set = ExpanseSet::new();
    let mut map = ExpanseMap::new();
    for _ in 0..n {
        let k = rng.next() & m;
        set.insert(k);
        map.insert(k, !k);
    }
    (set, map)
}

/// Bytes per key for `n` uniform keys drawn at `bits` width: (set, map, distinct keys).
fn random_cell(n: usize, bits: u32, seed: u64) -> (f64, f64, usize) {
    let (set, map) = build(n, bits, seed);
    (
        set.mem_used() as f64 / set.len().max(1) as f64,
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

fn hist_json(h: &[usize]) -> String {
    let parts: Vec<String> = h.iter().map(|v| v.to_string()).collect();
    format!("[{}]", parts.join(", "))
}

/// Sparse `[[population, count], …]` for the 257-bin leaf histogram.
fn sparse_hist_json(h: &[usize; 257]) -> String {
    let parts: Vec<String> = h
        .iter()
        .enumerate()
        .filter(|(_, c)| **c > 0)
        .map(|(p, c)| format!("[{p}, {c}]"))
        .collect();
    format!("[{}]", parts.join(", "))
}

/// One flavor's census as a JSON object; `mem_used` is included so the
/// `NodeBytes` decomposition is checkable against it inside the artifact.
fn census_json(
    flavor: &str,
    n: usize,
    bits: u32,
    keys: usize,
    mem_used: usize,
    s: &ExpanseStats,
) -> String {
    let c = &s.node_counts;
    let b = &s.node_bytes;
    format!(
        "    {{\"flavor\": \"{flavor}\", \"n\": {n}, \"bits\": {bits}, \"lambda\": {}, \"distinct_keys\": {keys}, \
         \"mem_used\": {mem_used}, \"bytes_per_key\": {},\n      \
         \"node_counts\": {{\"immed\": {}, \"leaf_linear\": {}, \"leaf_bitmap\": {}, \"branch_l3\": {}, \
         \"branch_l7\": {}, \"branch_b\": {}, \"branch_u\": {}, \"full_expanse\": {}}},\n      \
         \"node_bytes\": {{\"immed_values\": {}, \"leaf_linear\": {}, \"leaf_bitmap\": {}, \"branch_l3\": {}, \
         \"branch_l7\": {}, \"branch_b\": {}, \"branch_u\": {}, \"total\": {}}},\n      \
         \"depth_histogram\": {}, \"branch_depth_histogram\": {}, \"leaf_depth_histogram\": {},\n      \
         \"leaf_pop_histogram\": {}}}",
        r4(lambda(n, bits)),
        r4(mem_used as f64 / keys.max(1) as f64),
        c.immed,
        c.leaf_linear,
        c.leaf_bitmap,
        c.branch_l3,
        c.branch_l7,
        c.branch_b,
        c.branch_u,
        c.full_expanse,
        b.immed_values,
        b.leaf_linear,
        b.leaf_bitmap,
        b.branch_l3,
        b.branch_l7,
        b.branch_b,
        b.branch_u,
        b.total(),
        hist_json(&s.depth_histogram),
        hist_json(&s.branch_depth_histogram),
        hist_json(&s.leaf_depth_histogram),
        sparse_hist_json(&s.leaf_pop_histogram),
    )
}

fn print_census(flavor: &str, n: usize, bits: u32, keys: usize, mem_used: usize, s: &ExpanseStats) {
    let c = &s.node_counts;
    let b = &s.node_bytes;
    let expanses = 1u64 << (bits - 48);
    println!(
        "  {flavor:<3} N={n} @{bits} λ={:.2}  {} distinct keys, mem_used {} B = {:.2} B/key",
        lambda(n, bits),
        keys,
        mem_used,
        mem_used as f64 / keys as f64
    );
    println!(
        "      nodes: immed {} · leaf_linear {} · leaf_bitmap {} · branch_l3 {} · branch_l7 {} · branch_b {} · branch_u {}",
        c.immed, c.leaf_linear, c.leaf_bitmap, c.branch_l3, c.branch_l7, c.branch_b, c.branch_u
    );
    println!(
        "      bytes: immed_values {} · leaf_linear {} · leaf_bitmap {} · branch_l3 {} · branch_l7 {} · branch_b {} · branch_u {} · total {} (mem_used {})",
        b.immed_values,
        b.leaf_linear,
        b.leaf_bitmap,
        b.branch_l3,
        b.branch_l7,
        b.branch_b,
        b.branch_u,
        b.total(),
        mem_used
    );
    println!(
        "      branches by level: {:?}  leaves by level: {:?}",
        s.branch_depth_histogram, s.leaf_depth_histogram
    );
    let cascaded = s.branch_depth_histogram[6];
    println!(
        "      cascaded 2-byte expanses (branches at level 6): {cascaded} of {expanses} = {:.4}; cascaded sub-expanses at level 5: {}; BranchU present: {}",
        cascaded as f64 / expanses as f64,
        s.branch_depth_histogram[5],
        c.branch_u > 0
    );
    let max_pop = s
        .leaf_pop_histogram
        .iter()
        .rposition(|c| *c > 0)
        .unwrap_or(0);
    let over: usize = s.leaf_pop_histogram[LEAF_CAP + 1..].iter().sum();
    println!("      leaf populations: max {max_pop}, leaves above LEAF_CAP={LEAF_CAP}: {over}");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let json_path = args
        .iter()
        .position(|a| a == "--json")
        .and_then(|i| args.get(i + 1).cloned());
    let census_only = args.iter().any(|a| a == "--census-only");

    println!(
        "bytes/key across expanse occupancy (set flavor / map flavor); LEAF_CAP = {LEAF_CAP}, \
         BITMAP_TO_UNCOMPRESSED_THRESHOLD = {BITMAP_TO_UNCOMPRESSED_THRESHOLD}"
    );

    let mut cells = String::new();
    let mut anchors = String::new();
    if !census_only {
        println!(
            "{:>10} {:>5} {:>10} {:>8} {:>10} {:>10}",
            "N", "bits", "λ", "λ/cap", "set B/key", "map B/key"
        );
        let mut grid: Vec<(usize, u32)> = Vec::new();
        for &n in &POPULATIONS {
            for &bits in &WIDTHS {
                grid.push((n, bits));
            }
        }
        grid.extend(FINE_64.iter().map(|&n| (n, 64)));
        grid.extend(EXACT_LAMBDA);
        grid.extend(SECOND_CLIFF);
        for (n, bits) in grid {
            let lam = lambda(n, bits);
            let (set_bpk, map_bpk, distinct) = random_cell(n, bits, SEED);
            println!(
                "{n:>10} {bits:>5} {lam:>10.2} {:>7.0}% {set_bpk:>10.2} {map_bpk:>10.2}",
                100.0 * lam / LEAF_CAP as f64
            );
            let _ = writeln!(
                cells,
                "    {{\"n\": {n}, \"bits\": {bits}, \"lambda\": {}, \"lambda_over_leaf_cap\": {}, \
                 \"set_bpk\": {}, \"map_bpk\": {}, \"distinct_keys\": {distinct}}},",
                r4(lam),
                r4(lam / LEAF_CAP as f64),
                r2(set_bpk),
                r2(map_bpk)
            );
        }
        println!("\nconstruction-fixed occupancy (set B/key; flat across N by construction)");
        println!(
            "{:>10} {:>12} {:>12} {:>12}",
            "N", "sequential", "clustered", "sparse i<<40"
        );
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
    }

    // Node census at the three cells, both flavors.
    println!(
        "\nnode census (ExpanseStats + NodeBytes; the byte attribution sums to mem_used exactly)"
    );
    let mut census = String::new();
    for (n, bits) in CENSUS {
        let (set, map) = build(n, bits, SEED);
        let s = set.stats();
        let m = map.stats();
        assert_eq!(
            s.node_bytes.total(),
            set.mem_used(),
            "set NodeBytes must sum to mem_used"
        );
        assert_eq!(
            m.node_bytes.total(),
            map.mem_used(),
            "map NodeBytes must sum to mem_used"
        );
        print_census("set", n, bits, set.len() as usize, set.mem_used(), &s);
        print_census("map", n, bits, map.len() as usize, map.mem_used(), &m);
        let _ = writeln!(
            census,
            "{},",
            census_json("set", n, bits, set.len() as usize, set.mem_used(), &s)
        );
        let _ = writeln!(
            census,
            "{},",
            census_json("map", n, bits, map.len() as usize, map.mem_used(), &m)
        );
    }

    // Seed sensitivity: the same cells under a second stream.
    println!(
        "\nseed sensitivity (set B/key / map B/key): seed A = {SEED:#x}, seed B = {SEED_B:#x}"
    );
    let mut seeds = String::new();
    for (n, bits) in SEEDS {
        let (sa, ma, da) = random_cell(n, bits, SEED);
        let (sb, mb, db) = random_cell(n, bits, SEED_B);
        println!(
            "  N={n} @{bits} λ={:.2}: A {sa:.2} / {ma:.2} ({da} keys) · B {sb:.2} / {mb:.2} ({db} keys) · Δset {:+.2} · Δmap {:+.2}",
            lambda(n, bits),
            sb - sa,
            mb - ma
        );
        let _ = writeln!(
            seeds,
            "    {{\"n\": {n}, \"bits\": {bits}, \"lambda\": {}, \"seed_a\": \"{SEED:#x}\", \"seed_b\": \"{SEED_B:#x}\", \
             \"set_bpk_a\": {}, \"map_bpk_a\": {}, \"distinct_keys_a\": {da}, \
             \"set_bpk_b\": {}, \"map_bpk_b\": {}, \"distinct_keys_b\": {db}}},",
            r4(lambda(n, bits)),
            r2(sa),
            r2(ma),
            r2(sb),
            r2(mb)
        );
    }

    if let Some(path) = json_path {
        let commit = std::env::var("EXPANSE_COMMIT").unwrap_or_else(|_| "unknown".into());
        let doc = format!(
            "{{\n  \"meta\": {{\n    \"source\": \"crates/expanse/examples/keyspace_density.rs --json; the mechanism is docs/ARCHITECTURE.md §3.5 and the original sweep docs/benchmarks/hot_comparison/METHODOLOGY.md §9.4\",\n    \"instrument\": \"mem_used() deterministic byte accounting — host-independent; no wall clock\",\n    \"generator\": \"XorShift64(0x0DDB_1A5E_5EED_0001), each draw masked to the keyspace width; same seed at every width\",\n    \"lambda\": \"N / 2^(bits - 48): mean population of a 2-byte-prefix expanse once the top two key bytes saturate\",\n    \"leaf_cap\": {LEAF_CAP},\n    \"bitmap_to_uncompressed_threshold\": {BITMAP_TO_UNCOMPRESSED_THRESHOLD},\n    \"commit\": \"{commit}\",\n    \"workload_id\": \"example_keyspace_density\",\n    \"census\": \"ExpanseStats per flavor at three cells; node_bytes is the per-form byte attribution and its total equals mem_used; branch_depth_histogram[6] counts cascaded 2-byte expanses, [5] cascaded sub-expanses below them\",\n    \"seed_sensitivity\": \"the census cells re-drawn under a second XorShift64 seed; the delta is generator variance at fixed λ\"\n  }},\n  \"cells\": [\n{}  ],\n  \"anchors\": [\n{}  ],\n  \"census\": [\n{}  ],\n  \"seed_sensitivity\": [\n{}  ]\n}}\n",
            cells.trim_end_matches(",\n").to_string() + "\n",
            anchors.trim_end_matches(",\n").to_string() + "\n",
            census.trim_end_matches(",\n").to_string() + "\n",
            seeds.trim_end_matches(",\n").to_string() + "\n"
        );
        std::fs::write(&path, doc).expect("write --json output");
        println!("\nwrote {path}");
    }
}
