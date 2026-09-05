//! Memory pillar: bytes/key as a function of expanse occupancy, for both arms.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `hot_memory_curve` |
//! | `group` | 5 |
//! | `population` | swept; reported as λ, not as N (§9.6); `rowex_set` / `rowex_map` arms (feature `rowex`) census ROWEX against `SyncExpanseSet` / `SyncExpanseMap` on the same λ targets, build-only, single writer (§10.3 decision 1) |
//! | `probes_and_reuse` | N/A (memory census) |
//! | `hit_rate` | N/A |
//! | `miss_gen_method` | N/A |
//! | `value_dereference` | N/A — live bytes held from the C allocator |
//! | `measured_region` | build only; census armed around the build, teardown excluded |
//! | `arm_symmetry` | one allocator interposition measures both arms (§9.1); identical key streams within a pairing |
//! | `statistics` | exact byte counts, deterministic — no interval (§8.4) |
//! | `verdict` | pending measurement |
//!
//! ## Why a curve
//!
//! §9.4 measured a sawtooth in per-key cost for `ExpanseSet`: 7.64–21.02 B/key
//! under density alone, with a cascade at `λ ≈ LEAF_CAP`. A single-population
//! cell is a cherry-pick whichever side it lands on, so this pillar sweeps λ
//! and publishes the curve (§9.6).
//!
//! ## Why one process per cell
//!
//! HOT's node pool is a process-global `static` (§9.2): a build-and-drop leaves
//! reusable nodes on its free lists and the next build undercounts by up to
//! 3.3×. Each cell is therefore its own invocation, selected by argv, and the
//! runner drives the sweep. The binary refuses to census a warm pool.

use std::env;

#[cfg(feature = "rowex")]
use expanse_hot_bench::rowex::{RowexMap, RowexSet};
use expanse_hot_bench::{
    Census, HotMap, HotSet, hot_can_inline, require_cold_pool, validate_census,
};
use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
#[cfg(feature = "rowex")]
use expanse_trie::sync::{SyncExpanseMap, SyncExpanseSet};

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

/// Which pairing is being measured.
#[derive(Copy, Clone, PartialEq, Eq)]
enum Arm {
    /// HOT `IdentityKeyExtractor` vs `ExpanseSet`, 63-bit domain (§9.6).
    SetA,
    /// HOT `PairPointerKeyExtractor` vs `ExpanseMap`, full 64-bit domain.
    MapB,
    /// ROWEX `IdentityKeyExtractor` vs `SyncExpanseSet`, 63-bit domain (§10).
    #[cfg(feature = "rowex")]
    RowexSet,
    /// ROWEX `PairPointerKeyExtractor` vs `SyncExpanseMap`, 64-bit domain (§10).
    #[cfg(feature = "rowex")]
    RowexMap,
}

impl Arm {
    fn width(self) -> u32 {
        match self {
            // Arm A restricts its generator because HOT's inline payload is
            // 63 bits. Both of its sides draw from that domain, and the arm is
            // named for it — never mixed with Arm B's or the repo's cells.
            Arm::SetA => 63,
            Arm::MapB => 64,
            #[cfg(feature = "rowex")]
            Arm::RowexSet => 63,
            #[cfg(feature = "rowex")]
            Arm::RowexMap => 64,
        }
    }

    fn workload_id(self) -> &'static str {
        match self {
            Arm::SetA => "hot_set_63bit",
            Arm::MapB => "hot_map_64bit",
            #[cfg(feature = "rowex")]
            Arm::RowexSet => "hot_rowex_set_63bit",
            #[cfg(feature = "rowex")]
            Arm::RowexMap => "hot_rowex_map_64bit",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Arm::SetA => "set",
            Arm::MapB => "map",
            #[cfg(feature = "rowex")]
            Arm::RowexSet => "rowex_set",
            #[cfg(feature = "rowex")]
            Arm::RowexMap => "rowex_map",
        }
    }

    fn is_set(self) -> bool {
        self.width() == 63
    }
}

fn keys(n: usize, width: u32) -> Vec<u64> {
    let mask = if width >= 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    };
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
    let mut v: Vec<u64> = (0..n).map(|_| rng.next() & mask).collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Occupancy of a populated 2-byte-prefix expanse.
///
/// For uniform random keys over `width` bits the top two bytes saturate, so the
/// expanse count is `2^16` at 64 bits and halves per bit removed. This is the
/// axis the pillar publishes against, and the reason Arm A's restricted domain
/// stays comparable to Arm B's (§9.6).
fn lambda(n: usize, width: u32) -> f64 {
    let expanses = 2f64.powi(16 - (64 - width as i32));
    n as f64 / expanses
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: hot_memory_curve <set|map> <population>");
        eprintln!("  one cell per invocation — HOT's node pool is process-global (§9.2)");
        std::process::exit(2);
    }
    let arm = match args[1].as_str() {
        "set" => Arm::SetA,
        "map" => Arm::MapB,
        #[cfg(feature = "rowex")]
        "rowex_set" => Arm::RowexSet,
        #[cfg(feature = "rowex")]
        "rowex_map" => Arm::RowexMap,
        other => {
            eprintln!(
                "unknown arm {other:?}; expected `set` or `map` (or `rowex_set` / `rowex_map` \
                 with the `rowex` feature)"
            );
            std::process::exit(2);
        }
    };
    let n: usize = args[2].parse().unwrap_or_else(|_| {
        eprintln!("population must be an integer");
        std::process::exit(2);
    });

    let control = validate_census(1 << 20);
    if !control.is_valid() {
        eprintln!(
            "census control invalid (+{} B on {} B, residual {}); refusing to publish",
            control.alloc_delta, control.requested, control.residual
        );
        std::process::exit(1);
    }
    require_cold_pool("hot_memory_curve");

    let ks = keys(n, arm.width());
    let pop = ks.len();

    // Arm A cannot represent a key wider than HOT's inline payload. Its
    // generator already draws from the 63-bit domain, so this must hold — the
    // assertion is here because a silently smaller population is the failure
    // this suite was nearly published with.
    if arm.is_set() && !ks.iter().all(|k| hot_can_inline(*k)) {
        eprintln!("set arm generator produced a key outside HOT's inline payload");
        std::process::exit(1);
    }

    let (hot_pop, hot) = Census::measure(|| match arm {
        Arm::SetA => {
            let mut t = HotSet::new();
            for k in &ks {
                t.insert(*k);
            }
            let p = t.len();
            std::mem::forget(t);
            p
        }
        Arm::MapB => {
            let mut t = HotMap::new();
            for (i, k) in ks.iter().enumerate() {
                t.insert(*k, i as u64);
            }
            let p = t.len();
            std::mem::forget(t);
            p
        }
        // Single writer, build only: the census counters are process-global
        // atomics and cannot be armed under concurrent writers without
        // distorting them (§10.3, decision 1). What ROWEX's per-thread free
        // lists still hold at the end of the build is inside the number, as
        // HOT's pool retention is inside Arm A's (§3.2).
        #[cfg(feature = "rowex")]
        Arm::RowexSet => {
            let t = RowexSet::new();
            for k in &ks {
                t.insert(*k);
            }
            let p = t.len();
            std::mem::forget(t);
            p
        }
        #[cfg(feature = "rowex")]
        Arm::RowexMap => {
            let t = RowexMap::new();
            for (i, k) in ks.iter().enumerate() {
                t.insert(*k, i as u64);
            }
            let p = t.len();
            std::mem::forget(t);
            p
        }
    });

    // The floor of §10.3 decision 1: a map build must count at least one pair
    // and one node per key, a set build at least one node per key. Below it the
    // instrument is not seeing the arm and the cell is void.
    #[cfg(feature = "rowex")]
    if matches!(arm, Arm::RowexSet | Arm::RowexMap) {
        let floor = if arm.is_set() { pop } else { 2 * pop } as i64;
        if hot.allocs < floor {
            eprintln!(
                "ROWEX census counted {} allocations for {pop} keys, below the floor of {floor}; \
                 the instrument is not seeing the arm and the cell is void (§10.7)",
                hot.allocs
            );
            std::process::exit(1);
        }
    }

    let (exp_pop, exp) = Census::measure(|| match arm {
        Arm::SetA => {
            let mut t = ExpanseSet::new();
            for k in &ks {
                t.insert(*k);
            }
            let p = t.len() as usize;
            std::mem::forget(t);
            p
        }
        Arm::MapB => {
            let mut t = ExpanseMap::new();
            for (i, k) in ks.iter().enumerate() {
                t.insert(*k, i as u64);
            }
            let p = t.len() as usize;
            std::mem::forget(t);
            p
        }
        #[cfg(feature = "rowex")]
        Arm::RowexSet => {
            let t = SyncExpanseSet::new();
            for k in &ks {
                t.insert(*k);
            }
            let p = t.len() as usize;
            std::mem::forget(t);
            p
        }
        #[cfg(feature = "rowex")]
        Arm::RowexMap => {
            let t = SyncExpanseMap::new();
            for (i, k) in ks.iter().enumerate() {
                t.insert(*k, i as u64);
            }
            let p = t.len() as usize;
            std::mem::forget(t);
            p
        }
    });

    // §8: a cell whose final population differs from the intended population is
    // void. Counted by walking on the HOT side, never inferred from `insert`.
    if hot_pop != pop || exp_pop != pop {
        eprintln!(
            "population mismatch — HOT {hot_pop}, Expanse {exp_pop}, intended {pop}; cell is void"
        );
        std::process::exit(1);
    }

    // The engine's own accounting alongside the published allocator figure, so
    // the bridge of §9.3 is recorded per cell rather than argued once.
    let exp_mem_used = match arm {
        Arm::SetA => {
            let mut t = ExpanseSet::new();
            for k in &ks {
                t.insert(*k);
            }
            t.mem_used()
        }
        Arm::MapB => {
            let mut t = ExpanseMap::new();
            for (i, k) in ks.iter().enumerate() {
                t.insert(*k, i as u64);
            }
            t.mem_used()
        }
        #[cfg(feature = "rowex")]
        Arm::RowexSet => {
            let t = SyncExpanseSet::new();
            for k in &ks {
                t.insert(*k);
            }
            t.with_locked(ExpanseSet::mem_used)
        }
        #[cfg(feature = "rowex")]
        Arm::RowexMap => {
            let t = SyncExpanseMap::new();
            for (i, k) in ks.iter().enumerate() {
                t.insert(*k, i as u64);
            }
            t.with_locked(ExpanseMap::mem_used)
        }
    };

    let pf = pop as f64;
    println!(
        "{{\"workload_id\":\"{}\",\"arm\":\"{}\",\"keyspace_bits\":{},\"population\":{},\
         \"lambda\":{:.4},\"hot_alloc_bytes_per_key\":{:.4},\"expanse_alloc_bytes_per_key\":{:.4},\
         \"expanse_mem_used_bytes_per_key\":{:.4},\"hot_allocs\":{},\"expanse_allocs\":{}}}",
        arm.workload_id(),
        arm.label(),
        arm.width(),
        pop,
        lambda(pop, arm.width()),
        hot.live as f64 / pf,
        exp.live as f64 / pf,
        exp_mem_used as f64 / pf,
        hot.allocs,
        exp.allocs,
    );
}
