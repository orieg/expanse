//! Masstree arm, integer latency pillars (#661, METHODOLOGY §5): point lookup
//! (hit and 50/50), insert, and ordered scan — Masstree `u64 → u64` against
//! `ExpanseMap` (pairing M1).
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `masstree_latency` |
//! | `group` | 5 |
//! | `population` | selected per invocation; `random` cells also carry λ |
//! | `probes_and_reuse` | shuffled stream, one pass, `population`-many probes |
//! | `hit_rate` | 100% for `lookup_hit`, 50% for `lookup_miss`, n/a otherwise |
//! | `miss_gen_method` | same-generator rejection sampling (§8.6) |
//! | `value_dereference` | both sides fetch the stored value and fold it; the two sinks must agree |
//! | `measured_region` | probe loop only; build and teardown outside the timed window; Masstree's per-64-op `quiesce` inside its loop (§3.2) |
//! | `arm_symmetry` | identical key and probe streams, full 64-bit domain; same ISA target (§3.5) |
//! | `statistics` | per-round medians emitted raw; BCa 95% CIs computed by the harvester (§8.4) |
//! | `verdict` | pending measurement |
//!
//! ## One cell per invocation
//!
//! Masstree's node pools and limbo lists are per thread slot and outlive every
//! table (§3.6); the runner drives the sweep and each cell is its own process.
//!
//! ## Why this emits rounds rather than a verdict
//!
//! Wall-clock claims pass on the BCa 95% bootstrap CI lower bound, never on a
//! point estimate (§8.4), and this binary must not assert on a local timing.
//! It prints one JSON line per round; the harvester computes the interval.

use std::env;
use std::hint::black_box;
use std::time::Instant;

use expanse_hot_bench::masstree::{Masstree, MtThread, QUIESCE_EVERY, Table};
use expanse_hot_bench::workload::{self, Dist, Order};
use expanse_trie::map::ExpanseMap;

/// Rounds per cell. The harvester bootstraps over these.
const ROUNDS: usize = 15;
const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;
/// The single-threaded pillars run on one slot for the whole process.
const SLOT: u32 = 0;

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
            Pillar::LookupMiss => 0.5,
            _ => 1.0,
        }
    }
}

fn usage() -> ! {
    eprintln!(
        "usage: masstree_latency <lookup_hit|lookup_miss|insert|scan> \
         <sequential|clustered|sparse|random> <population> [scan_k] [sorted|shuffled] [single|concurrent]"
    );
    eprintln!(
        "  insertion order defaults to `sorted` (§10.2); the table to `single`, the M1 twin (§10.3)"
    );
    eprintln!("  one cell per invocation — Masstree's pools are per thread slot (§3.6)");
    std::process::exit(2);
}

fn void(msg: &str) -> ! {
    eprintln!("{msg}; cell is void");
    std::process::exit(1);
}

#[inline]
fn value_of(i: usize) -> u64 {
    (i as u64).wrapping_mul(GOLDEN)
}

fn ns_per_op(elapsed_ns: u128, ops: usize) -> f64 {
    elapsed_ns as f64 / ops as f64
}

fn build_mt(ti: MtThread, table: Table, pop: &[u64]) -> Masstree {
    let t = Masstree::new(ti, table);
    for (i, k) in pop.iter().enumerate() {
        t.insert(ti, *k, value_of(i));
        if (i as u64 + 1).is_multiple_of(QUIESCE_EVERY) {
            ti.quiesce();
        }
    }
    t
}

fn build_exp(pop: &[u64]) -> ExpanseMap {
    let mut e = ExpanseMap::new();
    for (i, k) in pop.iter().enumerate() {
        e.insert(*k, value_of(i));
    }
    e
}

fn main() {
    let mut a: Vec<String> = env::args().collect();
    // Trailing tokens, in any order: insertion order (§10.2) and table
    // configuration (§10.3); both default to the pairing's registered choice.
    let (mut order, mut table) = (Order::Sorted, Table::Single);
    while let Some(last) = a.last().map(String::as_str) {
        if let Some(o) = Order::parse(last) {
            order = o;
        } else if let Some(t) = Table::parse(last) {
            table = t;
        } else {
            break;
        }
        a.pop();
    }
    if a.len() < 4 || a.len() > 5 {
        usage();
    }
    let pillar = match a[1].as_str() {
        "lookup_hit" => Pillar::LookupHit,
        "lookup_miss" => Pillar::LookupMiss,
        "insert" => Pillar::Insert,
        "scan" => Pillar::Scan,
        _ => usage(),
    };
    let dist = match a[2].as_str() {
        "sequential" => Dist::Sequential,
        "clustered" => Dist::Clustered,
        "sparse" => Dist::Sparse,
        "random" => Dist::Random,
        _ => usage(),
    };
    let n: usize = a[3].parse().unwrap_or_else(|_| usage());
    let scan_k: usize = if a.len() == 5 {
        a[4].parse().unwrap_or_else(|_| usage())
    } else {
        100
    };

    // Full 64-bit domain: §2 found no payload predicate on integer keys.
    let mut w = workload::build(dist, n, 64, pillar.hit_rate());
    if order == Order::Shuffled {
        workload::shuffle_in_place(&mut w.population);
    }
    let pop = w.population.len();

    let ti = MtThread::slot(SLOT);
    ti.enter();

    // Build once, outside the timed window, for every pillar but insert (§8.6);
    // the structures are leaked deliberately so no round times a destructor.
    let (mt, exp) = if pillar == Pillar::Insert {
        (None, None)
    } else {
        let m = build_mt(ti, table, &w.population);
        let e = build_exp(&w.population);
        if m.len(ti) != pop || e.len() as usize != pop {
            void(&format!(
                "population mismatch — Masstree {}, Expanse {}, intended {pop}",
                m.len(ti),
                e.len()
            ));
        }
        (Some(m), Some(e))
    };

    for round in 0..ROUNDS {
        let (mt_ns, exp_ns, ops): (u128, u128, usize) = match pillar {
            Pillar::LookupHit | Pillar::LookupMiss => {
                let m = mt.as_ref().unwrap();
                let e = exp.as_ref().unwrap();
                // Both sides fetch the stored value and fold it, so neither
                // value read can be elided and the two folds must agree.
                let t0 = Instant::now();
                let mut sink = 0u64;
                let mut ops_done = 0u64;
                for p in &w.probes {
                    sink ^= m.get(ti, *p).unwrap_or(0);
                    ops_done += 1;
                    if ops_done.is_multiple_of(QUIESCE_EVERY) {
                        ti.quiesce();
                    }
                }
                let mt_t = t0.elapsed().as_nanos();
                black_box(sink);
                let mt_sink = sink;

                let t1 = Instant::now();
                let mut sink = 0u64;
                for p in &w.probes {
                    sink ^= e.get(*p).unwrap_or(0);
                }
                let exp_t = t1.elapsed().as_nanos();
                black_box(sink);
                if mt_sink != sink {
                    void(&format!("round {round}: Masstree and Expanse sinks differ"));
                }
                (mt_t, exp_t, w.probes.len())
            }
            Pillar::Insert => {
                let t0 = Instant::now();
                let m = Masstree::new(ti, table);
                for (i, k) in w.population.iter().enumerate() {
                    black_box(m.insert(ti, *k, value_of(i)));
                    if (i as u64 + 1).is_multiple_of(QUIESCE_EVERY) {
                        ti.quiesce();
                    }
                }
                let mt_t = t0.elapsed().as_nanos();
                let built = m.len(ti);
                std::mem::forget(m);

                let t1 = Instant::now();
                let mut e = ExpanseMap::new();
                for (i, k) in w.population.iter().enumerate() {
                    black_box(e.insert(*k, value_of(i)));
                }
                let exp_t = t1.elapsed().as_nanos();
                let built_e = e.len() as usize;
                std::mem::forget(e);

                if built != pop || built_e != pop {
                    void(&format!(
                        "insert round {round}: Masstree {built}, Expanse {built_e}, intended {pop}"
                    ));
                }
                (mt_t, exp_t, pop)
            }
            Pillar::Scan => {
                // Starts are drawn from the probe stream, so both sides walk
                // from identical positions.
                let starts: Vec<u64> = w.probes.iter().copied().take(1_000).collect();
                let m = mt.as_ref().unwrap();
                let e = exp.as_ref().unwrap();
                let t0 = Instant::now();
                let mut visited = 0usize;
                let mut mt_sink = 0u64;
                for (i, s) in starts.iter().enumerate() {
                    let (c, sink) = m.scan(ti, *s, scan_k);
                    visited += c;
                    mt_sink ^= sink;
                    if (i as u64 + 1).is_multiple_of(QUIESCE_EVERY) {
                        ti.quiesce();
                    }
                }
                let mt_t = t0.elapsed().as_nanos();
                black_box(mt_sink);

                let t1 = Instant::now();
                let mut visited_e = 0usize;
                let mut exp_sink = 0u64;
                for s in &starts {
                    let mut c = 0usize;
                    for (_, v) in e.range(*s..=u64::MAX) {
                        exp_sink ^= v;
                        c += 1;
                        if c == scan_k {
                            break;
                        }
                    }
                    visited_e += c;
                }
                let exp_t = t1.elapsed().as_nanos();
                black_box(exp_sink);
                if visited != visited_e || mt_sink != exp_sink {
                    void(&format!(
                        "scan round {round}: Masstree visited {visited}, Expanse {visited_e}; sinks equal: {}",
                        mt_sink == exp_sink
                    ));
                }
                (mt_t, exp_t, visited.max(1))
            }
        };

        println!(
            "{{\"workload_id\":\"masstree_map_64bit\",\"pillar\":\"{}\",\"arm\":\"map\",\"dist\":\"{}\",\
             \"order\":\"{}\",\"table\":\"{}\",\"keyspace_bits\":64,\"population\":{},\"lambda\":{:.4},\"scan_k\":{},\
             \"round\":{},\"ops\":{},\"masstree_ns_per_op\":{:.4},\"expanse_ns_per_op\":{:.4}}}",
            pillar.name(),
            dist.name(),
            order.name(),
            table.name(),
            pop,
            w.lambda(),
            if pillar == Pillar::Scan { scan_k } else { 0 },
            round,
            ops,
            ns_per_op(mt_ns, ops),
            ns_per_op(exp_ns, ops),
        );
    }
    ti.exit();
}
