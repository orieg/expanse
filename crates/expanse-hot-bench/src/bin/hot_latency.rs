//! Latency pillars: point lookup (hit and 50/50), insert, and ordered scan.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `hot_latency` |
//! | `group` | 5 |
//! | `population` | selected per invocation; `random` cells also carry λ |
//! | `probes_and_reuse` | shuffled stream, one pass, `population`-many probes; `scan` takes `max(1000, 10⁶ / k)` starts from that stream, cycling it when shorter (§12.1) |
//! | `hit_rate` | 100% for `lookup_hit`, 50% for `lookup_miss`, n/a otherwise |
//! | `miss_gen_method` | same-generator rejection sampling (§8.6) |
//! | `value_dereference` | Arm B fetches the stored value on both sides and sinks it (§9.8) |
//! | `measured_region` | probe loop only; build and teardown outside the timed window; the arm timed first alternates per round and is recorded as `first_arm` (§12.1) |
//! | `arm_symmetry` | identical key and probe streams within a pairing; same ISA target; both arms see the same insertion order, `sorted` unless the cell says `shuffled` (§12.2) |
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

use expanse_hot_bench::workload::{self, Dist, Order, ordered, scan_starts};
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
         <sequential|clustered|sparse|random> <population> [scan_k] [sorted|shuffled]"
    );
    eprintln!("  insertion order defaults to `sorted` (§12.2), the order the generator produces");
    eprintln!("  one cell per invocation — HOT's node pool is process-global (§9.2)");
    std::process::exit(2);
}

/// Refuses a round whose two arms did not do the same work.
fn void(msg: &str) -> ! {
    eprintln!("{msg}; cell is void");
    std::process::exit(1);
}

/// Nanoseconds per operation for one timed round.
fn ns_per_op(elapsed_ns: u128, ops: usize) -> f64 {
    elapsed_ns as f64 / ops as f64
}

fn main() {
    let mut a: Vec<String> = env::args().collect();
    // Trailing token: insertion order (§12.2). It defaults to `sorted`, which is
    // the order the shared generator hands both arms and the order every
    // registered cell in this suite was measured in.
    let mut order = Order::Sorted;
    while let Some(last) = a.last().map(String::as_str) {
        match Order::parse(last) {
            Some(o) => order = o,
            None => break,
        }
        a.pop();
    }
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

    let mut w = workload::build(dist, n, arm.width(), pillar.hit_rate());
    // §12.2: the generator sorts the population, so every insert verdict in
    // this suite is a sorted-order verdict. The permutation is Fisher–Yates
    // from the suite seed, so a shuffled cell is as reproducible as a sorted
    // one; only the build order changes, never the key set.
    if order == Order::Shuffled {
        workload::shuffle_in_place(&mut w.population);
    }
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
        // §12.1: the arm timed first runs on the cache and clock state the
        // second then inherits. Alternating per round lands that inheritance on
        // both arms equally instead of on whichever arm the source lists first.
        let hot_first = round % 2 == 0;
        let (hot_ns, exp_ns, ops) = match pillar {
            Pillar::LookupHit | Pillar::LookupMiss => match arm {
                Arm::SetA => {
                    let h = hot_set.as_ref().unwrap();
                    let e = exp_set.as_ref().unwrap();
                    let run_hot = || {
                        let t0 = Instant::now();
                        let mut sink = 0u64;
                        for p in &w.probes {
                            sink ^= u64::from(h.contains(*p));
                        }
                        let t = t0.elapsed().as_nanos();
                        black_box(sink);
                        (t, sink)
                    };
                    let run_exp = || {
                        let t0 = Instant::now();
                        let mut sink = 0u64;
                        for p in &w.probes {
                            sink ^= u64::from(e.contains(*p));
                        }
                        let t = t0.elapsed().as_nanos();
                        black_box(sink);
                        (t, sink)
                    };
                    let ((hot_t, hot_sink), (exp_t, exp_sink)) =
                        ordered(hot_first, run_hot, run_exp);
                    if hot_sink != exp_sink {
                        void(&format!("round {round}: HOT and Expanse sinks differ"));
                    }
                    (hot_t, exp_t, w.probes.len())
                }
                Arm::MapB => {
                    let h = hot_map.as_ref().unwrap();
                    let e = exp_map.as_ref().unwrap();
                    // Both sides fetch the stored value and sink it, so neither
                    // arm's value read can be elided and HOT's pointer chase is
                    // billed rather than hidden (§9.8).
                    let run_hot = || {
                        let t0 = Instant::now();
                        let mut sink = 0u64;
                        for p in &w.probes {
                            sink ^= h.get(*p).unwrap_or(0);
                        }
                        let t = t0.elapsed().as_nanos();
                        black_box(sink);
                        (t, sink)
                    };
                    let run_exp = || {
                        let t0 = Instant::now();
                        let mut sink = 0u64;
                        for p in &w.probes {
                            sink ^= e.get(*p).unwrap_or(0);
                        }
                        let t = t0.elapsed().as_nanos();
                        black_box(sink);
                        (t, sink)
                    };
                    let ((hot_t, hot_sink), (exp_t, exp_sink)) =
                        ordered(hot_first, run_hot, run_exp);
                    if hot_sink != exp_sink {
                        void(&format!("round {round}: HOT and Expanse sinks differ"));
                    }
                    (hot_t, exp_t, w.probes.len())
                }
            },
            Pillar::Insert => match arm {
                Arm::SetA => {
                    let run_hot = || {
                        let t0 = Instant::now();
                        let mut h = HotSet::new();
                        for k in &w.population {
                            black_box(h.insert(*k));
                        }
                        let t = t0.elapsed().as_nanos();
                        let built = h.len();
                        std::mem::forget(h);
                        (t, built)
                    };
                    let run_exp = || {
                        let t0 = Instant::now();
                        let mut e = ExpanseSet::new();
                        for k in &w.population {
                            black_box(e.insert(*k));
                        }
                        let t = t0.elapsed().as_nanos();
                        let built = e.len() as usize;
                        std::mem::forget(e);
                        (t, built)
                    };
                    let ((hot_t, built), (exp_t, built_e)) = ordered(hot_first, run_hot, run_exp);
                    if built != pop || built_e != pop {
                        void(&format!(
                            "insert round {round}: HOT {built}, Expanse {built_e}, intended {pop}"
                        ));
                    }
                    (hot_t, exp_t, pop)
                }
                Arm::MapB => {
                    let run_hot = || {
                        let t0 = Instant::now();
                        let mut h = HotMap::new();
                        for (i, k) in w.population.iter().enumerate() {
                            black_box(h.insert(*k, i as u64));
                        }
                        let t = t0.elapsed().as_nanos();
                        let built = h.len();
                        std::mem::forget(h);
                        (t, built)
                    };
                    let run_exp = || {
                        let t0 = Instant::now();
                        let mut e = ExpanseMap::new();
                        for (i, k) in w.population.iter().enumerate() {
                            black_box(e.insert(*k, i as u64));
                        }
                        let t = t0.elapsed().as_nanos();
                        let built = e.len() as usize;
                        std::mem::forget(e);
                        (t, built)
                    };
                    let ((hot_t, built), (exp_t, built_e)) = ordered(hot_first, run_hot, run_exp);
                    if built != pop || built_e != pop {
                        void(&format!(
                            "insert round {round}: HOT {built}, Expanse {built_e}, intended {pop}"
                        ));
                    }
                    (hot_t, exp_t, pop)
                }
            },
            Pillar::Scan => {
                // Starts are drawn from the probe stream, so both arms walk
                // from identical positions. §12.1: the count scales with 1/k so
                // every k visits about 10⁶ elements per round instead of
                // leaving k = 10 the shortest, warmest timed window in the
                // suite; the stream is cycled when it is shorter than that.
                let starts: Vec<u64> = w
                    .probes
                    .iter()
                    .copied()
                    .cycle()
                    .take(scan_starts(scan_k))
                    .collect();
                match arm {
                    Arm::SetA => {
                        let h = hot_set.as_ref().unwrap();
                        let e = exp_set.as_ref().unwrap();
                        let run_hot = || {
                            let t0 = Instant::now();
                            let mut visited = 0usize;
                            for s in &starts {
                                visited += h.scan(*s, scan_k);
                            }
                            let t = t0.elapsed().as_nanos();
                            black_box(visited);
                            (t, visited)
                        };
                        let run_exp = || {
                            let t0 = Instant::now();
                            let mut visited = 0usize;
                            let mut sink = 0u64;
                            for s in &starts {
                                let mut c = 0usize;
                                for k in e.range(*s..=u64::MAX) {
                                    sink ^= k;
                                    c += 1;
                                    if c == scan_k {
                                        break;
                                    }
                                }
                                visited += c;
                            }
                            let t = t0.elapsed().as_nanos();
                            black_box(sink);
                            black_box(visited);
                            (t, visited)
                        };
                        let ((hot_t, visited), (exp_t, visited_e)) =
                            ordered(hot_first, run_hot, run_exp);
                        if visited != visited_e {
                            void(&format!(
                                "scan round {round}: HOT visited {visited}, Expanse {visited_e}"
                            ));
                        }
                        (hot_t, exp_t, visited.max(1))
                    }
                    Arm::MapB => {
                        let h = hot_map.as_ref().unwrap();
                        let e = exp_map.as_ref().unwrap();
                        let run_hot = || {
                            let t0 = Instant::now();
                            let mut visited = 0usize;
                            for s in &starts {
                                visited += h.scan(*s, scan_k);
                            }
                            let t = t0.elapsed().as_nanos();
                            black_box(visited);
                            (t, visited)
                        };
                        let run_exp = || {
                            let t0 = Instant::now();
                            let mut visited = 0usize;
                            let mut sink = 0u64;
                            for s in &starts {
                                let mut c = 0usize;
                                for (_, v) in e.range(*s..=u64::MAX) {
                                    sink ^= v;
                                    c += 1;
                                    if c == scan_k {
                                        break;
                                    }
                                }
                                visited += c;
                            }
                            let t = t0.elapsed().as_nanos();
                            black_box(sink);
                            black_box(visited);
                            (t, visited)
                        };
                        let ((hot_t, visited), (exp_t, visited_e)) =
                            ordered(hot_first, run_hot, run_exp);
                        if visited != visited_e {
                            void(&format!(
                                "scan round {round}: HOT visited {visited}, Expanse {visited_e}"
                            ));
                        }
                        (hot_t, exp_t, visited.max(1))
                    }
                }
            }
        };

        println!(
            "{{\"workload_id\":\"{}\",\"pillar\":\"{}\",\"arm\":\"{}\",\"dist\":\"{}\",\
             \"order\":\"{}\",\"keyspace_bits\":{},\"population\":{},\"lambda\":{:.4},\"scan_k\":{},\
             \"round\":{},\"first_arm\":\"{}\",\"ops\":{},\"hot_ns_per_op\":{:.4},\
             \"expanse_ns_per_op\":{:.4}}}",
            arm.workload_id(),
            pillar.name(),
            arm.label(),
            dist.name(),
            order.name(),
            arm.width(),
            pop,
            w.lambda(),
            if pillar == Pillar::Scan { scan_k } else { 0 },
            round,
            if hot_first { "hot" } else { "expanse" },
            ops,
            ns_per_op(hot_ns, ops),
            ns_per_op(exp_ns, ops),
        );
    }
}
