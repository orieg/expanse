//! String-key latency pillars (#693): point lookup (hit and 50/50), insert,
//! and ordered scan, for the three string pairings of METHODOLOGY §10.2.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `hot_string_latency` |
//! | `group` | 5 |
//! | `population` | selected per invocation; mean key length reported per cell |
//! | `probes_and_reuse` | shuffled stream, one pass, `population`-many probes, every probe its own allocation |
//! | `hit_rate` | 100% for `lookup_hit`, 50% for `lookup_miss`, n/a otherwise |
//! | `miss_gen_method` | same-generator rejection sampling (§8.6) |
//! | `value_dereference` | both sides return the stored word and fold it; Arm C/E sinks must be equal (§10.2) |
//! | `measured_region` | probe loop only; string generation, build and teardown outside the timed window |
//! | `arm_symmetry` | identical strings and probe stream within a pairing; same ISA target (§3.3); HOT column withheld where the §10.4 predicate fails |
//! | `statistics` | per-round medians emitted raw; BCa 95% CIs computed by the harvester (§8.4) |
//! | `verdict` | pending measurement |
//!
//! ## One cell per invocation
//!
//! HOT's node pool is process-global (§9.2). The runner drives the sweep; each
//! cell is its own process.
//!
//! ## The HOT column is a predicate, not a precondition
//!
//! A population containing keys longer than HOT's 255-byte window cannot be
//! held by HOT with fidelity (§10.4). The cell still runs — the Expanse side is
//! never restricted — and emits `"hot_ns_per_op": null` with the count of keys
//! HOT cannot represent, so the runner publishes the finding in HOT's column
//! rather than a number over a silently smaller population.
//!
//! ## Scan surface
//!
//! The Expanse scan drives the shipped `ExpanseStrMap` navigation surface —
//! `next_at_or_after` then `next_after` per element, each a root descent
//! returning a fresh key allocation — against HOT's `lower_bound` plus
//! incremental iterator (§10.6). `ExpanseBytesMap` is unordered and has no scan
//! pillar.

use std::env;
use std::hint::black_box;
use std::time::Instant;

use expanse_hot_bench::strings::{self, KeyStr, StrDist};
use expanse_hot_bench::{HotStr, HotStrMap};
use expanse_trie::bytesmap::ExpanseBytesMap;
use expanse_trie::strmap::ExpanseStrMap;

/// Rounds per cell. The harvester bootstraps over these.
const ROUNDS: usize = 15;
const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

#[derive(Copy, Clone, PartialEq, Eq)]
enum Arm {
    /// Arm C: HOT identity extractor vs `ExpanseStrMap` storing the key pointer.
    Ptr,
    /// Arm D: HOT pair pointer vs `ExpanseStrMap` string → u64.
    Map,
    /// Arm E: HOT identity extractor vs `ExpanseBytesMap` storing the key pointer.
    Bytes,
}

impl Arm {
    fn workload_id(self) -> &'static str {
        match self {
            Arm::Ptr => "hot_str_ptr",
            Arm::Map => "hot_str_map",
            Arm::Bytes => "hot_bytes_ptr",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Arm::Ptr => "ptr",
            Arm::Map => "map",
            Arm::Bytes => "bytes",
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
            Pillar::LookupMiss => 0.5,
            _ => 1.0,
        }
    }
}

fn usage() -> ! {
    eprintln!(
        "usage: hot_string_latency <ptr|map|bytes> <lookup_hit|lookup_miss|insert|scan> \
         <short|counter|prefixed|skewed|beyond> <population> [scan_k]"
    );
    eprintln!("  one cell per invocation — HOT's node pool is process-global (§9.2)");
    eprintln!("  `bytes` has no scan pillar: ExpanseBytesMap is unordered");
    std::process::exit(2);
}

fn void(msg: &str) -> ! {
    eprintln!("{msg}; cell is void");
    std::process::exit(1);
}

/// Value the map arm stores for population index `i`.
fn map_value(i: usize) -> u64 {
    (i as u64).wrapping_mul(GOLDEN)
}

/// Either Expanse string structure, behind one interface for the pillars.
enum Exp {
    Str(ExpanseStrMap),
    Bytes(ExpanseBytesMap),
}

impl Exp {
    fn new(arm: Arm) -> Self {
        match arm {
            Arm::Ptr | Arm::Map => Exp::Str(ExpanseStrMap::new()),
            Arm::Bytes => Exp::Bytes(ExpanseBytesMap::new()),
        }
    }
    #[inline]
    fn insert(&mut self, k: &[u8], v: u64) {
        match self {
            Exp::Str(m) => {
                m.insert(k, v);
            }
            Exp::Bytes(m) => {
                m.insert(k, v);
            }
        }
    }
    #[inline]
    fn get(&self, k: &[u8]) -> Option<u64> {
        match self {
            Exp::Str(m) => m.get(k),
            Exp::Bytes(m) => m.get(k),
        }
    }
    fn len(&self) -> usize {
        match self {
            Exp::Str(m) => m.len() as usize,
            Exp::Bytes(m) => m.len() as usize,
        }
    }
}

/// Either HOT configuration, behind one interface for the pillars.
enum Hot<'a> {
    Str(HotStr<'a>),
    Map(HotStrMap<'a>),
}

impl<'a> Hot<'a> {
    fn new(arm: Arm) -> Self {
        match arm {
            Arm::Ptr | Arm::Bytes => Hot::Str(HotStr::new()),
            Arm::Map => Hot::Map(HotStrMap::new()),
        }
    }
    #[inline]
    fn insert(&mut self, k: &'a KeyStr, v: u64) -> bool {
        match self {
            Hot::Str(t) => t.insert(k),
            Hot::Map(t) => t.insert(k, v),
        }
    }
    #[inline]
    fn get(&self, k: &KeyStr) -> Option<u64> {
        match self {
            Hot::Str(t) => t.lookup(k),
            Hot::Map(t) => t.get(k),
        }
    }
    fn len(&self) -> usize {
        match self {
            Hot::Str(t) => t.len(),
            Hot::Map(t) => t.len(),
        }
    }
    fn scan(&self, lo: &KeyStr, k: usize) -> (usize, u64) {
        match self {
            Hot::Str(t) => t.scan(lo, k),
            Hot::Map(t) => t.scan(lo, k),
        }
    }
}

/// The value each side stores for population entry `i` on this arm: the key's
/// own pointer on the identity arms, a distinct word on the map arm.
#[inline]
fn value_for(arm: Arm, k: &KeyStr, i: usize) -> u64 {
    match arm {
        Arm::Ptr | Arm::Bytes => k.word(),
        Arm::Map => map_value(i),
    }
}

fn build_hot<'a>(arm: Arm, pop: &'a [KeyStr]) -> Hot<'a> {
    let mut h = Hot::new(arm);
    for (i, k) in pop.iter().enumerate() {
        h.insert(k, value_for(arm, k, i));
    }
    h
}

fn build_exp(arm: Arm, pop: &[KeyStr]) -> Exp {
    let mut e = Exp::new(arm);
    for (i, k) in pop.iter().enumerate() {
        e.insert(k.bytes(), value_for(arm, k, i));
    }
    e
}

fn ns_per_op(elapsed_ns: u128, ops: usize) -> f64 {
    elapsed_ns as f64 / ops as f64
}

fn main() {
    let a: Vec<String> = env::args().collect();
    if a.len() < 5 || a.len() > 6 {
        usage();
    }
    let arm = match a[1].as_str() {
        "ptr" => Arm::Ptr,
        "map" => Arm::Map,
        "bytes" => Arm::Bytes,
        _ => usage(),
    };
    let pillar = match a[2].as_str() {
        "lookup_hit" => Pillar::LookupHit,
        "lookup_miss" => Pillar::LookupMiss,
        "insert" => Pillar::Insert,
        "scan" => Pillar::Scan,
        _ => usage(),
    };
    let dist = StrDist::parse(&a[3]).unwrap_or_else(|| usage());
    let n: usize = a[4].parse().unwrap_or_else(|_| usage());
    let scan_k: usize = if a.len() == 6 {
        a[5].parse().unwrap_or_else(|_| usage())
    } else {
        100
    };
    if pillar == Pillar::Scan && arm == Arm::Bytes {
        usage();
    }

    let w = strings::build(dist, n, pillar.hit_rate());
    let pop = w.population.len();
    let not_rep = w.hot_not_representable();
    // §10.4: evaluated against the workload, reported, never used to trim it.
    let hot_side = not_rep == 0;
    if hot_side && w.population.iter().any(|k| k.word() >> 63 != 0) {
        void("a key pointer has bit 63 set; HOT's inline payload cannot hold it");
    }

    // Build once, outside the timed window, for every pillar but insert (§8.6);
    // structures are leaked deliberately so no round times a destructor.
    let (hot, mut exp) = if pillar == Pillar::Insert {
        (None, None)
    } else {
        let h = if hot_side {
            let h = build_hot(arm, &w.population);
            if h.len() != pop {
                void(&format!("HOT walks {} of {pop} intended", h.len()));
            }
            Some(h)
        } else {
            None
        };
        let e = build_exp(arm, &w.population);
        if e.len() != pop {
            void(&format!("Expanse holds {} of {pop} intended", e.len()));
        }
        (h, Some(e))
    };

    for round in 0..ROUNDS {
        let (hot_ns, exp_ns, ops): (Option<u128>, u128, usize) = match pillar {
            Pillar::LookupHit | Pillar::LookupMiss => {
                let e = exp.as_ref().unwrap();
                let mut hot_ns = None;
                let mut hot_sink = 0u64;
                if let Some(h) = hot.as_ref() {
                    let t0 = Instant::now();
                    let mut sink = 0u64;
                    for p in &w.probes {
                        sink ^= h.get(p).unwrap_or(0);
                    }
                    hot_ns = Some(t0.elapsed().as_nanos());
                    black_box(sink);
                    hot_sink = sink;
                }

                let t1 = Instant::now();
                let mut sink = 0u64;
                for p in &w.probes {
                    sink ^= e.get(p.bytes()).unwrap_or(0);
                }
                let exp_t = t1.elapsed().as_nanos();
                black_box(sink);

                // Both sides stored the same words, so the folded results must
                // agree (§10.2). A divergence is a correctness failure that
                // would otherwise surface as a latency difference.
                if hot.is_some() && hot_sink != sink {
                    void(&format!("round {round}: HOT and Expanse sinks differ"));
                }
                (hot_ns, exp_t, w.probes.len())
            }
            Pillar::Insert => {
                let mut hot_ns = None;
                if hot_side {
                    let t0 = Instant::now();
                    let mut h = Hot::new(arm);
                    for (i, k) in w.population.iter().enumerate() {
                        black_box(h.insert(k, value_for(arm, k, i)));
                    }
                    hot_ns = Some(t0.elapsed().as_nanos());
                    let built = h.len();
                    std::mem::forget(h);
                    if built != pop {
                        void(&format!("insert round {round}: HOT walks {built} of {pop}"));
                    }
                }

                let t1 = Instant::now();
                let mut e = Exp::new(arm);
                for (i, k) in w.population.iter().enumerate() {
                    e.insert(k.bytes(), value_for(arm, k, i));
                }
                let exp_t = t1.elapsed().as_nanos();
                let built_e = e.len();
                std::mem::forget(e);
                if built_e != pop {
                    void(&format!(
                        "insert round {round}: Expanse holds {built_e} of {pop}"
                    ));
                }
                (hot_ns, exp_t, pop)
            }
            Pillar::Scan => {
                // Starts are drawn from the probe stream, so both sides walk
                // from identical positions.
                let starts: Vec<&KeyStr> = w.probes.iter().take(1_000).collect();
                let mut hot_ns = None;
                let mut hot_visited = 0usize;
                let mut hot_sink = 0u64;
                if let Some(h) = hot.as_ref() {
                    let t0 = Instant::now();
                    for s in &starts {
                        let (c, sink) = h.scan(s, scan_k);
                        hot_visited += c;
                        hot_sink ^= sink;
                    }
                    hot_ns = Some(t0.elapsed().as_nanos());
                    black_box(hot_sink);
                }

                let Some(Exp::Str(e)) = exp.as_mut() else {
                    unreachable!("scan runs on ExpanseStrMap only")
                };
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

                if hot.is_some() && (hot_visited != visited_e || hot_sink != exp_sink) {
                    void(&format!(
                        "scan round {round}: HOT visited {hot_visited}, Expanse {visited_e}; sinks equal: {}",
                        hot_sink == exp_sink
                    ));
                }
                (hot_ns, exp_t, visited_e.max(1))
            }
        };

        let hot_field = match hot_ns {
            Some(ns) => format!("{:.4}", ns_per_op(ns, ops)),
            None => "null".to_string(),
        };
        println!(
            "{{\"workload_id\":\"{}\",\"pillar\":\"{}\",\"arm\":\"{}\",\"dist\":\"{}\",\
             \"population\":{},\"mean_key_len\":{:.2},\"hot_not_representable\":{},\
             \"hot_representable_fraction\":{:.4},\"scan_k\":{},\"round\":{},\"ops\":{},\
             \"hot_ns_per_op\":{},\"expanse_ns_per_op\":{:.4}}}",
            arm.workload_id(),
            pillar.name(),
            arm.label(),
            dist.name(),
            pop,
            w.mean_len(),
            not_rep,
            w.hot_representable_fraction(),
            if pillar == Pillar::Scan { scan_k } else { 0 },
            round,
            ops,
            hot_field,
            ns_per_op(exp_ns, ops),
        );
    }
}
