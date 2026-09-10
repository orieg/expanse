//! Why the OLC fallback composition is a curve, not a number (Refs #568).
//!
//! Phase 0 asked which structural transition drives writers onto the serialized
//! root-covered path. A single measurement answered it differently depending on
//! how full the tree was, so the answer is a sweep.
//!
//! In a tree growing from empty, most fallbacks are `BranchB` subarray growth:
//! new digits keep arriving in subexpanses that do not yet hold them. In a
//! mature tree those subexpanses are already populated, fresh keys land in
//! existing structure, and what grows is the leaf holding them — so leaf
//! capacity expansion takes over. The crossover is not gradual: it tracks the
//! discrete expanse-occupancy transitions that put the sawtooth in
//! `bytes_per_key` (see `keyspace_density.rs`), so a share can move by two
//! orders of magnitude between adjacent populations, and non-monotonically.
//!
//! This matters because the concurrency gate cells prefill 2^20 keys and then
//! measure fresh inserts into that mature tree. Attribution taken on a small
//! growing tree does not describe them, and must not be quoted as if it did.
//!
//! Both structures are drawn from **one generator, one seed and one keyspace
//! width**, so the map and set columns are comparable to each other
//! (AGENTS.md §8.3). The comparative suites draw their map arm at 64 bits and
//! their set arm at 63; holding the width equal here is what makes a map-vs-set
//! difference a property of the structure rather than of the draw. The
//! generator is the repo's shared XorShift64 at the shared seed, replicated the
//! way `keyspace_density.rs` replicates it.
//!
//! Deterministic counter census only — exact `occ_stats` integers, no wall
//! clock, so no confidence interval is meaningful and none is reported (§8.4).
//! Nothing here is timed; the populations are chosen for coverage, not cost.
//!
//! Run:
//! ```text
//! cargo run --release -p expanse-trie --features occ-stats \
//!     --example fallback_maturity -- [--json <path>]
//! ```
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `example_fallback_maturity` |
//! | `group` | 5 |
//! | `population` | 0 / 10k / 100k / 1M prefill x 63- and 64-bit keyspace, 50k measured inserts per cell |
//! | `insertion_order` | generator draw order — the population is inserted as drawn, neither sorted nor shuffled |
//! | `probes_and_reuse` | none — insert-only, each key drawn once |
//! | `hit_rate` | n/a — no read probes |
//! | `miss_gen_method` | n/a — measured keys are the tail of the same single draw, so they are disjoint from the prefill by construction rather than by rejection |
//! | `value_dereference` | n/a — counter census, no payload read |
//! | `measured_region` | counter delta around the measured inserts; prefill and teardown are outside it |
//! | `arm_symmetry` | map and set share one generator and seed, and are swept over the same keyspace widths |
//! | `statistics` | exact deterministic event counters; no interval (§8.4) |
//! | `verdict` | **PASS** `[verified: CODE READ]`: deterministic attribution census. |

use expanse_trie::occ_stats::{self, Stat};
use expanse_trie::sync::{SyncExpanseMap, SyncExpanseSet};

/// XorShift64, the generator the comparative suites use. Replicated here rather
/// than imported because `expanse-hot-bench` cannot build without the HOT
/// submodule and this probe has nothing to do with HOT; the seed below is the
/// shared constant and must not diverge from it.
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

/// The suites' shared seed (`expanse_hot_bench::workload::XorShift::SEED`,
/// and `keyspace_density.rs`).
const SEED: u64 = 0x0DDB_1A5E_5EED_0001;

/// Keyspace widths swept. Width and population are one knob: what the engine
/// responds to is occupancy per expanse, so narrowing the domain by one bit is
/// arithmetically the same as doubling the population (`keyspace_density.rs`).
/// Sweeping both is what stops a map-vs-set difference being read off two cells
/// that were really at different densities — which is how the 64-bit map column
/// alone reads as leaf-dominated while the 63-bit one does not.
const WIDTHS: [u32; 2] = [63, 64];

/// Fresh inserts measured per cell.
const MEASURED: usize = 50_000;

/// Tree populations measured into. The last is the concurrency suites' own.
const PREFILLS: [usize; 4] = [0, 10_000, 100_000, 1_048_576];

const CAUSES: [(Stat, &str); 6] = [
    (Stat::FallbackCapExpansion, "cap_expansion"),
    (Stat::FallbackImmediateConversion, "immediate_conversion"),
    (Stat::FallbackBranchSplit, "branch_split"),
    (Stat::FallbackRootGrowth, "root_growth"),
    (Stat::FallbackContention, "contention"),
    (Stat::FallbackUnknownTag, "unknown_tag"),
];

struct Cell {
    arm: &'static str,
    keyspace_bits: u32,
    prefill: usize,
    inserts: u64,
    fallbacks: u64,
    causes: [u64; 6],
}

impl Cell {
    fn share(&self, i: usize) -> f64 {
        if self.fallbacks == 0 {
            0.0
        } else {
            self.causes[i] as f64 / self.fallbacks as f64 * 100.0
        }
    }
    fn fallback_rate(&self) -> f64 {
        self.fallbacks as f64 / MEASURED as f64 * 100.0
    }
}

/// One draw covering prefill and measured keys, so the measured keys are
/// disjoint from the prefill by construction.
fn draw(n: usize, keyspace_bits: u32) -> Vec<u64> {
    let mask = if keyspace_bits >= 64 {
        u64::MAX
    } else {
        (1u64 << keyspace_bits) - 1
    };
    let mut rng = XorShift(SEED);
    (0..n).map(|_| rng.next() & mask).collect()
}

fn cell(arm: &'static str, prefill: usize, keyspace_bits: u32) -> Cell {
    let keys = draw(prefill + MEASURED, keyspace_bits);
    let (warm, fresh) = keys.split_at(prefill);

    let (before, after) = if arm == "map" {
        let m = SyncExpanseMap::new();
        for (i, &k) in warm.iter().enumerate() {
            m.insert(k, i as u64);
        }
        let before = occ_stats::snapshot();
        for (i, &k) in fresh.iter().enumerate() {
            m.insert(k, i as u64);
        }
        (before, occ_stats::snapshot())
    } else {
        let s = SyncExpanseSet::new();
        for &k in warm {
            s.insert(k);
        }
        let before = occ_stats::snapshot();
        for &k in fresh {
            s.insert(k);
        }
        (before, occ_stats::snapshot())
    };

    let d = |s: Stat| after[s as usize] - before[s as usize];
    let mut causes = [0u64; 6];
    for (i, (s, _)) in CAUSES.iter().enumerate() {
        causes[i] = d(*s);
    }
    let fallbacks = d(Stat::LockFallbacks);
    let summed: u64 = causes.iter().sum();
    assert_eq!(
        summed,
        fallbacks,
        "{arm} @ prefill {prefill} / {keyspace_bits}-bit: causes must account for every fallback (unattributed {})",
        fallbacks as i64 - summed as i64
    );
    Cell {
        arm,
        keyspace_bits,
        prefill,
        inserts: d(Stat::Inserts),
        fallbacks,
        causes,
    }
}

fn emit_json(cells: &[Cell]) -> String {
    let mut o = String::from("{\n  \"probe\": \"fallback_maturity\",\n  \"issue\": 568,\n");
    o.push_str(&format!(
        "  \"workload_id\": \"example_fallback_maturity\",\n  \
         \"generator\": \"XorShift64 @ seed 0x0DDB1A5E5EED0001, shared with the comparative suites\",\n  \
         \"keyspace_bits_swept\": {WIDTHS:?},\n  \
         \"measured_inserts_per_cell\": {MEASURED},\n"
    ));
    o.push_str(
        "  \"estimators\": \"deterministic occ_stats counters; exact integers, no interval (AGENTS.md 8.4)\",\n",
    );
    o.push_str("  \"cells\": [\n");
    for (n, c) in cells.iter().enumerate() {
        o.push_str(&format!(
            "    {{ \"arm\": \"{}\", \"keyspace_bits\": {}, \"prefill\": {}, \"inserts\": {}, \"fallbacks\": {}, \"fallback_rate_pct\": {:.4}, \"causes\": {{",
            c.arm,
            c.keyspace_bits,
            c.prefill,
            c.inserts,
            c.fallbacks,
            c.fallback_rate()
        ));
        for (i, (_, name)) in CAUSES.iter().enumerate() {
            o.push_str(&format!(
                "{}\"{}\": {}",
                if i == 0 { " " } else { ", " },
                name,
                c.causes[i]
            ));
        }
        o.push_str(&format!(
            " }} }}{}\n",
            if n + 1 == cells.len() { "" } else { "," }
        ));
    }
    o.push_str("  ]\n}\n");
    o
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let json_path = args
        .iter()
        .position(|a| a == "--json")
        .and_then(|i| args.get(i + 1))
        .cloned();

    let mut cells = Vec::new();
    for arm in ["map", "set"] {
        for width in WIDTHS {
            for prefill in PREFILLS {
                cells.push(cell(arm, prefill, width));
            }
        }
    }

    println!(
        "fallback composition vs occupancy — {MEASURED} fresh inserts per cell, one generator and seed"
    );
    println!(
        "{:>4} {:>6} {:>10} {:>13} {:>9} {:>9} {:>9} {:>9} {:>8} {:>8}",
        "arm", "bits", "prefill", "fallback/ins", "cap", "immed", "branch", "root", "cont", "unk"
    );
    for c in &cells {
        println!(
            "{:>4} {:>6} {:>10} {:>12.2}% {:>8.2}% {:>8.2}% {:>8.2}% {:>8.2}% {:>7.2}% {:>7.2}%",
            c.arm,
            c.keyspace_bits,
            c.prefill,
            c.fallback_rate(),
            c.share(0),
            c.share(1),
            c.share(2),
            c.share(3),
            c.share(4),
            c.share(5),
        );
    }

    if let Some(p) = json_path {
        std::fs::write(&p, emit_json(&cells)).expect("write json artifact");
        println!("\nwrote {p}");
    }
}
