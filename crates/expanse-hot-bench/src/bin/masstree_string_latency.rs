//! Masstree arm, string latency pillars (#661, METHODOLOGY §5): point lookup
//! (hit and 50/50), insert, and ordered scan — Masstree with byte-string keys
//! against `ExpanseStrMap`, both `string → u64` (pairing M2).
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `masstree_string_latency` |
//! | `group` | 5 |
//! | `population` | selected per invocation; mean key length reported per cell |
//! | `probes_and_reuse` | shuffled stream, one pass, `population`-many probes, every probe its own allocation |
//! | `hit_rate` | 100% for `lookup_hit`, 50% for `lookup_miss`, n/a otherwise |
//! | `miss_gen_method` | same-generator rejection sampling (§8.6) |
//! | `value_dereference` | both sides fetch the stored value and fold it; the two sinks must agree |
//! | `measured_region` | probe loop only; string generation, build and teardown outside; Masstree's per-64-op `quiesce` inside its loop (§3.2) |
//! | `arm_symmetry` | identical strings and probe stream; both sides copy key bytes into their own nodes (§4); same ISA target; Masstree column withheld where the §3.4 predicate fails |
//! | `statistics` | per-round medians emitted raw; BCa 95% CIs computed by the harvester (§8.4) |
//! | `verdict` | pending measurement |
//!
//! ## The Masstree column is a predicate, not a precondition
//!
//! A population containing keys longer than `MASSTREE_MAXKEYLEN` is outside
//! Masstree's contract (§3.4). The cell still runs — the Expanse side is never
//! restricted — and emits `"masstree_ns_per_op": null` with the count of keys
//! Masstree cannot represent, so the runner publishes the finding in
//! Masstree's column rather than a number over a smaller population.
//!
//! ## Scan surface
//!
//! The Expanse scan drives the shipped `ExpanseStrMap` navigation surface —
//! `next_at_or_after` then `next_after` per element, each a root descent
//! returning a fresh key allocation — against Masstree's `scan` from a start
//! key with a visitor that stops after k (`hot_comparison` §10.6 applies).

use std::env;
use std::hint::black_box;
use std::time::Instant;

use expanse_hot_bench::masstree::{
    Masstree, MtThread, QUIESCE_EVERY, StrInsert, Table, masstree_can_key,
};
use expanse_hot_bench::strings::{self, KeyStr, StrDist};
use expanse_hot_bench::workload::{self, Order};
use expanse_trie::strmap::ExpanseStrMap;

const ROUNDS: usize = 15;
const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;
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
        "usage: masstree_string_latency <lookup_hit|lookup_miss|insert|scan> \
         <short|counter|prefixed|skewed|beyond> <population> [scan_k] [sorted|shuffled] [single|concurrent]"
    );
    eprintln!(
        "  insertion order defaults to `sorted` (§10.2); the table to `single`, the M2 twin (§10.3)"
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

fn build_mt(ti: MtThread, table: Table, pop: &[KeyStr]) -> Masstree {
    let t = Masstree::new(ti, table);
    for (i, k) in pop.iter().enumerate() {
        if t.str_insert(ti, k.bytes(), value_of(i)) == StrInsert::NotRepresentable {
            void("a key beyond the predicate reached the Masstree side (§9)");
        }
        if (i as u64 + 1).is_multiple_of(QUIESCE_EVERY) {
            ti.quiesce();
        }
    }
    t
}

fn build_exp(pop: &[KeyStr]) -> ExpanseStrMap {
    let mut e = ExpanseStrMap::new();
    for (i, k) in pop.iter().enumerate() {
        e.insert(k.bytes(), value_of(i));
    }
    e
}

fn main() {
    let mut a: Vec<String> = env::args().collect();
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
    let dist = StrDist::parse(&a[2]).unwrap_or_else(|| usage());
    let n: usize = a[3].parse().unwrap_or_else(|_| usage());
    let scan_k: usize = if a.len() == 5 {
        a[4].parse().unwrap_or_else(|_| usage())
    } else {
        100
    };

    let mut w = strings::build(dist, n, pillar.hit_rate());
    if order == Order::Shuffled {
        workload::shuffle_in_place(&mut w.population);
    }
    let pop = w.population.len();
    let not_rep = w.not_representable(masstree_can_key);
    // §3.4: evaluated against the workload, reported, never used to trim it.
    // A probe stream may also carry a long miss; the predicate covers it.
    let mt_side = not_rep == 0 && w.probes.iter().all(|p| masstree_can_key(p.len()));

    let ti = MtThread::slot(SLOT);
    ti.enter();

    let (mt, mut exp) = if pillar == Pillar::Insert {
        (None, None)
    } else {
        let m = if mt_side {
            let m = build_mt(ti, table, &w.population);
            if m.len(ti) != pop {
                void(&format!("Masstree walks {} of {pop} intended", m.len(ti)));
            }
            Some(m)
        } else {
            None
        };
        let e = build_exp(&w.population);
        if e.len() as usize != pop {
            void(&format!("Expanse holds {} of {pop} intended", e.len()));
        }
        (m, Some(e))
    };

    for round in 0..ROUNDS {
        let (mt_ns, exp_ns, ops): (Option<u128>, u128, usize) = match pillar {
            Pillar::LookupHit | Pillar::LookupMiss => {
                let e = exp.as_ref().unwrap();
                let mut mt_ns = None;
                let mut mt_sink = 0u64;
                if let Some(m) = mt.as_ref() {
                    let t0 = Instant::now();
                    let mut sink = 0u64;
                    let mut done = 0u64;
                    for p in &w.probes {
                        sink ^= m.str_get(ti, p.bytes()).unwrap_or(0);
                        done += 1;
                        if done.is_multiple_of(QUIESCE_EVERY) {
                            ti.quiesce();
                        }
                    }
                    mt_ns = Some(t0.elapsed().as_nanos());
                    black_box(sink);
                    mt_sink = sink;
                }

                let t1 = Instant::now();
                let mut sink = 0u64;
                for p in &w.probes {
                    sink ^= e.get(p.bytes()).unwrap_or(0);
                }
                let exp_t = t1.elapsed().as_nanos();
                black_box(sink);
                if mt.is_some() && mt_sink != sink {
                    void(&format!("round {round}: Masstree and Expanse sinks differ"));
                }
                (mt_ns, exp_t, w.probes.len())
            }
            Pillar::Insert => {
                let mut mt_ns = None;
                if mt_side {
                    let t0 = Instant::now();
                    let m = Masstree::new(ti, table);
                    for (i, k) in w.population.iter().enumerate() {
                        black_box(m.str_insert(ti, k.bytes(), value_of(i)));
                        if (i as u64 + 1).is_multiple_of(QUIESCE_EVERY) {
                            ti.quiesce();
                        }
                    }
                    mt_ns = Some(t0.elapsed().as_nanos());
                    let built = m.len(ti);
                    std::mem::forget(m);
                    if built != pop {
                        void(&format!(
                            "insert round {round}: Masstree walks {built} of {pop}"
                        ));
                    }
                }

                let t1 = Instant::now();
                let mut e = ExpanseStrMap::new();
                for (i, k) in w.population.iter().enumerate() {
                    e.insert(k.bytes(), value_of(i));
                }
                let exp_t = t1.elapsed().as_nanos();
                let built_e = e.len() as usize;
                std::mem::forget(e);
                if built_e != pop {
                    void(&format!(
                        "insert round {round}: Expanse holds {built_e} of {pop}"
                    ));
                }
                (mt_ns, exp_t, pop)
            }
            Pillar::Scan => {
                let starts: Vec<&KeyStr> = w.probes.iter().take(1_000).collect();
                let mut mt_ns = None;
                let mut mt_visited = 0usize;
                let mut mt_sink = 0u64;
                if let Some(m) = mt.as_ref() {
                    let t0 = Instant::now();
                    for (i, s) in starts.iter().enumerate() {
                        let (c, sink) = m.str_scan(ti, s.bytes(), scan_k);
                        mt_visited += c;
                        mt_sink ^= sink;
                        if (i as u64 + 1).is_multiple_of(QUIESCE_EVERY) {
                            ti.quiesce();
                        }
                    }
                    mt_ns = Some(t0.elapsed().as_nanos());
                    black_box(mt_sink);
                }

                let e = exp.as_mut().unwrap();
                let t1 = Instant::now();
                let mut visited_e = 0usize;
                let mut exp_sink = 0u64;
                for s in &starts {
                    let mut c = 0usize;
                    let mut cur = e.next_at_or_after(s.bytes());
                    while let Some((key, slot)) = cur {
                        // SAFETY: the slot pointer is valid until the next
                        // structural mutation, and none occurs during the scan.
                        exp_sink ^= unsafe { *slot.as_ptr() };
                        c += 1;
                        if c == scan_k {
                            break;
                        }
                        cur = e.next_after(&key);
                    }
                    visited_e += c;
                }
                let exp_t = t1.elapsed().as_nanos();
                black_box(exp_sink);
                if mt.is_some() && (mt_visited != visited_e || mt_sink != exp_sink) {
                    void(&format!(
                        "scan round {round}: Masstree visited {mt_visited}, Expanse {visited_e}; sinks equal: {}",
                        mt_sink == exp_sink
                    ));
                }
                (mt_ns, exp_t, visited_e.max(1))
            }
        };

        let mt_field = match mt_ns {
            Some(ns) => format!("{:.4}", ns_per_op(ns, ops)),
            None => "null".to_string(),
        };
        println!(
            "{{\"workload_id\":\"masstree_str_map\",\"pillar\":\"{}\",\"arm\":\"str\",\"dist\":\"{}\",\
             \"order\":\"{}\",\"table\":\"{}\",\"population\":{},\"mean_key_len\":{:.2},\"masstree_not_representable\":{},\
             \"masstree_representable_fraction\":{:.4},\"scan_k\":{},\"round\":{},\"ops\":{},\
             \"masstree_ns_per_op\":{},\"expanse_ns_per_op\":{:.4}}}",
            pillar.name(),
            dist.name(),
            order.name(),
            table.name(),
            pop,
            w.mean_len(),
            not_rep,
            w.representable_fraction(masstree_can_key),
            if pillar == Pillar::Scan { scan_k } else { 0 },
            round,
            ops,
            mt_field,
            ns_per_op(exp_ns, ops),
        );
    }
    ti.exit();
}
