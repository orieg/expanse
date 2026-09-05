//! Latency pillars: point lookup (hit and 50/50), insert, and ordered scan.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `hot_latency` |
//! | `group` | 5 |
//! | `population` | selected per invocation; `random` cells also carry λ |
//! | `probes_and_reuse` | shuffled stream, one pass, `population`-many probes |
//! | `hit_rate` | 100% for `lookup_hit`, 50% for `lookup_miss`, n/a otherwise |
//! | `miss_gen_method` | same-generator rejection sampling (§8.6) |
//! | `value_dereference` | Arm B fetches the stored value on both sides and sinks it (§9.8) |
//! | `measured_region` | probe loop only; build and teardown outside the timed window |
//! | `arm_symmetry` | identical key and probe streams within a pairing; same ISA target |
//! | `statistics` | per-round medians emitted raw; BCa 95% CIs computed by the harvester (§8.4) |
//! | `verdict` | pending measurement |
//!
//! ## One cell per invocation
//!
//! HOT's node pool is a process-global `static` (§9.2), so a build in one
//! process changes what a later build in the same process allocates. The
//! latency pillars do not census memory, but they do build tries, and sharing a
//! process would let one pillar's leftovers change another's node shapes. The
//! runner drives the sweep; each cell is its own invocation.
//!
//! ## Why this emits rounds rather than a verdict
//!
//! Wall-clock claims pass on the BCa 95% bootstrap CI lower bound, never on a
//! point estimate (§8.4), and this binary must not assert on a local timing
//! (§8.4's prohibition on hard panics over continuous point estimates). It
//! prints one JSON line per round and leaves interval estimation to the
//! harvester, which is where the repo's other suites do it.

use std::env;
use std::hint::black_box;
use std::time::Instant;

use expanse_hot_bench::workload::{self, Dist};
use expanse_hot_bench::{HotMap, HotSet, InlineInsert, hot_can_inline};
use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;

/// Rounds per cell. The harvester bootstraps over these.
const ROUNDS: usize = 15;

#[derive(Copy, Clone, PartialEq, Eq)]
enum Arm {
    /// HOT `IdentityKeyExtractor` vs `ExpanseSet`, 63-bit domain (§9.6).
    SetA,
    /// HOT `PairPointerKeyExtractor` vs `ExpanseMap`, full 64-bit domain.
    MapB,
}

impl Arm {
    fn width(self) -> u32 {
        match self {
            Arm::SetA => 63,
            Arm::MapB => 64,
        }
    }
    fn workload_id(self) -> &'static str {
        match self {
            Arm::SetA => "hot_set_63bit",
            Arm::MapB => "hot_map_64bit",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Arm::SetA => "set",
            Arm::MapB => "map",
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Pillar {
    LookupHit,
    LookupMiss,
    Insert,
    Scan,
}

impl Pillar {
    fn name(self) -> &'static str {
        match self {
            Pillar::LookupHit => "lookup_hit",
            Pillar::LookupMiss => "lookup_miss",
            Pillar::Insert => "insert",
            Pillar::Scan => "scan",
        }
    }
    fn hit_rate(self) -> f64 {
        match self {
            Pillar::LookupHit => 1.0,
            Pillar::LookupMiss => 0.5,
            // Insert and scan do not probe; the stream is unused.
            _ => 1.0,
        }
    }
}

fn usage() -> ! {
    eprintln!(
        "usage: hot_latency <set|map> <lookup_hit|lookup_miss|insert|scan> \
         <sequential|clustered|sparse|random> <population> [scan_k]"
    );
    eprintln!("  one cell per invocation — HOT's node pool is process-global (§9.2)");
    std::process::exit(2);
}

/// Nanoseconds per operation for one timed round.
fn ns_per_op(elapsed_ns: u128, ops: usize) -> f64 {
    elapsed_ns as f64 / ops as f64
}

fn main() {
    let a: Vec<String> = env::args().collect();
    if a.len() < 5 || a.len() > 6 {
        usage();
    }
    let arm = match a[1].as_str() {
        "set" => Arm::SetA,
        "map" => Arm::MapB,
        _ => usage(),
    };
    let pillar = match a[2].as_str() {
        "lookup_hit" => Pillar::LookupHit,
        "lookup_miss" => Pillar::LookupMiss,
        "insert" => Pillar::Insert,
        "scan" => Pillar::Scan,
        _ => usage(),
    };
    let dist = match a[3].as_str() {
        "sequential" => Dist::Sequential,
        "clustered" => Dist::Clustered,
        "sparse" => Dist::Sparse,
        "random" => Dist::Random,
        _ => usage(),
    };
    let n: usize = a[4].parse().unwrap_or_else(|_| usage());
    let scan_k: usize = if a.len() == 6 {
        a[5].parse().unwrap_or_else(|_| usage())
    } else {
        100
    };

    let w = workload::build(dist, n, arm.width(), pillar.hit_rate());
    let pop = w.population.len();

    // Arm A's generator already draws from the 63-bit domain; assert rather than
    // assume, because a silently smaller population is the failure this suite
    // was nearly published with (§9.4).
    if arm == Arm::SetA && !w.population.iter().all(|k| hot_can_inline(*k)) {
        eprintln!("Arm A generator produced a key outside HOT's inline payload");
        std::process::exit(1);
    }

    // Build once, outside the timed window, for every pillar but insert. §8.6:
    // construction and teardown stay out of the measured region, and the tries
    // are leaked deliberately so no round times a destructor.
    let (hot_set, exp_set, hot_map, exp_map) = if pillar == Pillar::Insert {
        (None, None, None, None)
    } else {
        match arm {
            Arm::SetA => {
                let mut h = HotSet::new();
                let mut e = ExpanseSet::new();
                for k in &w.population {
                    if h.insert(*k) == InlineInsert::NotRepresentable {
                        eprintln!("Arm A: key not representable during build");
                        std::process::exit(1);
                    }
                    e.insert(*k);
                }
                if h.len() != pop || e.len() as usize != pop {
                    eprintln!(
                        "population mismatch — HOT {}, Expanse {}, intended {pop}; cell is void",
                        h.len(),
                        e.len()
                    );
                    std::process::exit(1);
                }
                (Some(h), Some(e), None, None)
            }
            Arm::MapB => {
                let mut h = HotMap::new();
                let mut e = ExpanseMap::new();
                for (i, k) in w.population.iter().enumerate() {
                    let v = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
                    h.insert(*k, v);
                    e.insert(*k, v);
                }
                if h.len() != pop || e.len() as usize != pop {
                    eprintln!(
                        "population mismatch — HOT {}, Expanse {}, intended {pop}; cell is void",
                        h.len(),
                        e.len()
                    );
                    std::process::exit(1);
                }
                (None, None, Some(h), Some(e))
            }
        }
    };

    for round in 0..ROUNDS {
        let (hot_ns, exp_ns, ops) = match pillar {
            Pillar::LookupHit | Pillar::LookupMiss => match arm {
                Arm::SetA => {
                    let h = hot_set.as_ref().unwrap();
                    let e = exp_set.as_ref().unwrap();
                    let t0 = Instant::now();
                    let mut sink = 0u64;
                    for p in &w.probes {
                        sink ^= u64::from(h.contains(*p));
                    }
                    let hot_t = t0.elapsed().as_nanos();
                    black_box(sink);

                    let t1 = Instant::now();
                    let mut sink = 0u64;
                    for p in &w.probes {
                        sink ^= u64::from(e.contains(*p));
                    }
                    let exp_t = t1.elapsed().as_nanos();
                    black_box(sink);
                    (hot_t, exp_t, w.probes.len())
                }
                Arm::MapB => {
                    let h = hot_map.as_ref().unwrap();
                    let e = exp_map.as_ref().unwrap();
                    // Both sides fetch the stored value and sink it, so neither
                    // arm's value read can be elided and HOT's pointer chase is
                    // billed rather than hidden (§9.8).
                    let t0 = Instant::now();
                    let mut sink = 0u64;
                    for p in &w.probes {
                        sink ^= h.get(*p).unwrap_or(0);
                    }
                    let hot_t = t0.elapsed().as_nanos();
                    black_box(sink);

                    let t1 = Instant::now();
                    let mut sink = 0u64;
                    for p in &w.probes {
                        sink ^= e.get(*p).unwrap_or(0);
                    }
                    let exp_t = t1.elapsed().as_nanos();
                    black_box(sink);
                    (hot_t, exp_t, w.probes.len())
                }
            },
            Pillar::Insert => match arm {
                Arm::SetA => {
                    let t0 = Instant::now();
                    let mut h = HotSet::new();
                    for k in &w.population {
                        black_box(h.insert(*k));
                    }
                    let hot_t = t0.elapsed().as_nanos();
                    let built = h.len();
                    std::mem::forget(h);

                    let t1 = Instant::now();
                    let mut e = ExpanseSet::new();
                    for k in &w.population {
                        black_box(e.insert(*k));
                    }
                    let exp_t = t1.elapsed().as_nanos();
                    let built_e = e.len() as usize;
                    std::mem::forget(e);

                    if built != pop || built_e != pop {
                        eprintln!("insert round {round}: population mismatch; cell is void");
                        std::process::exit(1);
                    }
                    (hot_t, exp_t, pop)
                }
                Arm::MapB => {
                    let t0 = Instant::now();
                    let mut h = HotMap::new();
                    for (i, k) in w.population.iter().enumerate() {
                        black_box(h.insert(*k, i as u64));
                    }
                    let hot_t = t0.elapsed().as_nanos();
                    let built = h.len();
                    std::mem::forget(h);

                    let t1 = Instant::now();
                    let mut e = ExpanseMap::new();
                    for (i, k) in w.population.iter().enumerate() {
                        black_box(e.insert(*k, i as u64));
                    }
                    let exp_t = t1.elapsed().as_nanos();
                    let built_e = e.len() as usize;
                    std::mem::forget(e);

                    if built != pop || built_e != pop {
                        eprintln!("insert round {round}: population mismatch; cell is void");
                        std::process::exit(1);
                    }
                    (hot_t, exp_t, pop)
                }
            },
            Pillar::Scan => {
                // Scan starts are drawn from the probe stream, so the two arms
                // walk from identical positions.
                let starts: Vec<u64> = w.probes.iter().copied().take(1_000).collect();
                match arm {
                    Arm::SetA => {
                        let h = hot_set.as_ref().unwrap();
                        let e = exp_set.as_ref().unwrap();
                        let t0 = Instant::now();
                        let mut visited = 0usize;
                        for s in &starts {
                            visited += h.scan(*s, scan_k);
                        }
                        let hot_t = t0.elapsed().as_nanos();
                        black_box(visited);

                        let t1 = Instant::now();
                        let mut visited_e = 0usize;
                        for s in &starts {
                            let mut c = 0usize;
                            let mut sink = 0u64;
                            for k in e.range(*s..=u64::MAX) {
                                sink ^= k;
                                c += 1;
                                if c == scan_k {
                                    break;
                                }
                            }
                            black_box(sink);
                            visited_e += c;
                        }
                        let exp_t = t1.elapsed().as_nanos();
                        black_box(visited_e);
                        (hot_t, exp_t, visited.max(1))
                    }
                    Arm::MapB => {
                        let h = hot_map.as_ref().unwrap();
                        let e = exp_map.as_ref().unwrap();
                        let t0 = Instant::now();
                        let mut visited = 0usize;
                        for s in &starts {
                            visited += h.scan(*s, scan_k);
                        }
                        let hot_t = t0.elapsed().as_nanos();
                        black_box(visited);

                        let t1 = Instant::now();
                        let mut visited_e = 0usize;
                        for s in &starts {
                            let mut c = 0usize;
                            let mut sink = 0u64;
                            for (_, v) in e.range(*s..=u64::MAX) {
                                sink ^= v;
                                c += 1;
                                if c == scan_k {
                                    break;
                                }
                            }
                            black_box(sink);
                            visited_e += c;
                        }
                        let exp_t = t1.elapsed().as_nanos();
                        black_box(visited_e);
                        (hot_t, exp_t, visited.max(1))
                    }
                }
            }
        };

        println!(
            "{{\"workload_id\":\"{}\",\"pillar\":\"{}\",\"arm\":\"{}\",\"dist\":\"{}\",\
             \"keyspace_bits\":{},\"population\":{},\"lambda\":{:.4},\"scan_k\":{},\
             \"round\":{},\"ops\":{},\"hot_ns_per_op\":{:.4},\"expanse_ns_per_op\":{:.4}}}",
            arm.workload_id(),
            pillar.name(),
            arm.label(),
            dist.name(),
            arm.width(),
            pop,
            w.lambda(),
            if pillar == Pillar::Scan { scan_k } else { 0 },
            round,
            ops,
            ns_per_op(hot_ns, ops),
            ns_per_op(exp_ns, ops),
        );
    }
}
