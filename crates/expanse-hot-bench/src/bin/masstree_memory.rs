//! Masstree arm, memory pillar (#661, METHODOLOGY §3.3): bytes per key on two
//! instruments — the shared allocator census and each engine's own node
//! accounting — for the integer map, the concurrent wrapper's build-only
//! census, and the string map.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `masstree_memory` |
//! | `group` | 5 |
//! | `population` | swept by the runner: λ targets on `random` u64 (§5), one N = 10⁶ cell per structured distribution, the string population sweep; one cell per invocation |
//! | `probes_and_reuse` | N/A (memory census) |
//! | `hit_rate` | N/A |
//! | `miss_gen_method` | N/A |
//! | `value_dereference` | N/A — live bytes held from the C allocator, and each engine's own node census |
//! | `measured_region` | build only; the census is armed before the thread slot and table are created and disarmed after the walk; teardown excluded |
//! | `arm_symmetry` | one allocator interposition measures both arms; Masstree's figure is slab-quantized and carries its measured slack and the `QUANTUM_DOMINATED` flag; Masstree `json_stats` beside Expanse `mem_used` (§3.3) |
//! | `statistics` | exact byte counts, deterministic — no interval (§8.4) |
//! | `verdict` | pending measurement |
//!
//! ## Two instruments, never mixed
//!
//! The allocator column is what the process holds; on Masstree that is a whole
//! number of 2 MiB slabs plus `malloc`ed suffix bags, so every cell also
//! reports the structural bytes Masstree's own node census implies and the
//! difference between the two. The runner reads the flag, not the reader.
//!
//! ## One process per cell, one fresh slot
//!
//! A reused slot reports a different figure (§2, §3.6). The Masstree side
//! builds on a slot no earlier code in this process has touched, created
//! inside the armed window so its per-thread constant is counted too.

use std::env;

use expanse_hot_bench::masstree::{
    Masstree, MtThread, QUIESCE_EVERY, StrInsert, Table, masstree_can_key, masstree_slab_bytes,
};
use expanse_hot_bench::strings::{self, StrDist};
use expanse_hot_bench::workload::{self, Dist, Order};
use expanse_hot_bench::{Census, validate_census};
use expanse_trie::map::ExpanseMap;
use expanse_trie::strmap::ExpanseStrMap;
use expanse_trie::sync::SyncExpanseMap;

const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;
/// A slot nothing else in this process uses (§3.6).
const FRESH_SLOT: u32 = 40;
/// `pool_max_nlines`: the most size classes a thread's pool can hold, and so
/// the most partially used slabs a census can be holding (§3.3 ceiling).
const SIZE_CLASSES: usize = 20;

#[derive(Copy, Clone, PartialEq, Eq)]
enum Arm {
    /// Masstree `u64 → u64` vs `ExpanseMap` (M1).
    Map,
    /// Masstree single writer vs `SyncExpanseMap`, the concurrent arm's M pillar.
    Sync,
    /// Masstree byte-string keys vs `ExpanseStrMap` (M2).
    Str,
}

impl Arm {
    fn workload_id(self) -> &'static str {
        match self {
            Arm::Map => "masstree_map_64bit",
            Arm::Sync => "masstree_conc_map_64bit",
            Arm::Str => "masstree_str_map",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Arm::Map => "map",
            Arm::Sync => "sync",
            Arm::Str => "str",
        }
    }
}

#[inline]
fn value_of(i: usize) -> u64 {
    (i as u64).wrapping_mul(GOLDEN)
}

fn void(msg: &str) -> ! {
    eprintln!("{msg}; cell is void");
    std::process::exit(1);
}

fn json_f64(v: Option<f64>) -> String {
    v.map_or("null".to_string(), |x| format!("{x:.4}"))
}

fn main() {
    let mut args: Vec<String> = env::args().collect();
    // Trailing tokens, in any order: insertion order (§10.2) and table
    // configuration (§10.3); the `sync` arm always uses the concurrent table.
    let (mut order, mut table_opt) = (Order::Sorted, None);
    while let Some(last) = args.last().map(String::as_str) {
        if let Some(o) = Order::parse(last) {
            order = o;
        } else if let Some(t) = Table::parse(last) {
            table_opt = Some(t);
        } else {
            break;
        }
        args.pop();
    }
    if args.len() != 4 {
        eprintln!(
            "usage: masstree_memory <map|sync> <sequential|clustered|sparse|random> <population> \
             [sorted|shuffled] [single|concurrent]"
        );
        eprintln!(
            "       masstree_memory str <short|counter|prefixed|skewed|beyond> <population> \
             [sorted|shuffled] [single|concurrent]"
        );
        eprintln!(
            "  one cell per invocation, on a fresh thread slot (§3.6); order defaults to sorted \
             (§10.2); the table to the pairing's twin (§10.3)"
        );
        std::process::exit(2);
    }
    let arm = match args[1].as_str() {
        "map" => Arm::Map,
        "sync" => Arm::Sync,
        "str" => Arm::Str,
        other => {
            eprintln!("unknown arm {other:?}");
            std::process::exit(2);
        }
    };
    let n: usize = args[3].parse().unwrap_or_else(|_| {
        eprintln!("population must be an integer");
        std::process::exit(2);
    });
    let table = table_opt.unwrap_or(match arm {
        Arm::Sync => Table::Concurrent,
        Arm::Map | Arm::Str => Table::Single,
    });

    let control = validate_census(1 << 20);
    if !control.is_valid() {
        void(&format!(
            "census control invalid (+{} B on {} B, residual {})",
            control.alloc_delta, control.requested, control.residual
        ));
    }

    // The workload, generated with the census disarmed.
    let (int_keys, dist_name, lambda, str_pop, mean_len, not_rep) = match arm {
        Arm::Map | Arm::Sync => {
            let dist = match args[2].as_str() {
                "sequential" => Dist::Sequential,
                "clustered" => Dist::Clustered,
                "sparse" => Dist::Sparse,
                "random" => Dist::Random,
                other => {
                    eprintln!("unknown distribution {other:?}");
                    std::process::exit(2);
                }
            };
            let mut w = workload::build(dist, n, 64, 0.0);
            if order == Order::Shuffled {
                workload::shuffle_in_place(&mut w.population);
            }
            let lam = (dist == Dist::Random).then(|| w.lambda());
            (Some(w.population), dist.name(), lam, None, None, 0)
        }
        Arm::Str => {
            let dist = StrDist::parse(&args[2]).unwrap_or_else(|| {
                eprintln!("unknown shape {:?}", args[2]);
                std::process::exit(2);
            });
            let mut w = strings::build_population(dist, n);
            if order == Order::Shuffled {
                workload::shuffle_in_place(&mut w.population);
            }
            let nr = w.not_representable(masstree_can_key);
            let ml = w.mean_len();
            (None, dist.name(), None, Some(w.population), Some(ml), nr)
        }
    };
    let pop = int_keys
        .as_ref()
        .map(Vec::len)
        .or_else(|| str_pop.as_ref().map(Vec::len))
        .unwrap();
    let mt_side = not_rep == 0;

    // Masstree, on a fresh slot created inside the window, only where the
    // §3.4 predicate holds. The table is returned, not dropped: its node
    // census is read after the counters are disarmed, then it is leaked.
    let masstree = if mt_side {
        let ((t, ti, walked, unsettled, settle_steps), c) = Census::measure(|| {
            let ti = MtThread::slot(FRESH_SLOT);
            ti.enter();
            let t = Masstree::new(ti, table);
            match (&int_keys, &str_pop) {
                (Some(ks), _) => {
                    for (i, k) in ks.iter().enumerate() {
                        t.insert(ti, *k, value_of(i));
                        if (i as u64 + 1).is_multiple_of(QUIESCE_EVERY) {
                            ti.quiesce();
                        }
                    }
                }
                (_, Some(ks)) => {
                    for (i, k) in ks.iter().enumerate() {
                        if t.str_insert(ti, k.bytes(), value_of(i)) == StrInsert::NotRepresentable {
                            void("a key beyond the predicate reached the Masstree side (§9)");
                        }
                        if (i as u64 + 1).is_multiple_of(QUIESCE_EVERY) {
                            ti.quiesce();
                        }
                    }
                }
                _ => unreachable!(),
            }
            let unsettled = Census::read().live;
            // §10.4: reclaim what the build deferred through RCU before reading
            // the figure. Each step frees at most 128 limbo entries; stop when
            // the census sees no further frees.
            let mut steps = 0u32;
            let mut frees = Census::read().frees;
            // The epoch must pass the recording epoch twice before an entry is
            // reclaimable, and a step may free nothing while later ones free plenty,
            // so stop only after three consecutive idle steps.
            let mut idle = 0u32;
            while idle < 3 && steps < 1_000_000 {
                ti.settle_step();
                steps += 1;
                let now = Census::read().frees;
                idle = if now == frees { idle + 1 } else { 0 };
                frees = now;
            }
            let walked = t.len(ti);
            (t, ti, walked, unsettled, steps)
        });
        if walked != pop {
            void(&format!("Masstree walks {walked} of {pop} intended"));
        }
        let st = t.stats(ti);
        if st.size as usize != pop {
            void(&format!("Masstree json_stats counts {} of {pop}", st.size));
        }
        ti.exit();
        std::mem::forget(t);
        if c.live < st.structural_bytes as i64 {
            void(&format!(
                "allocator census ({} B) below Masstree's structural bytes ({} B): the census is not seeing the arm (§3.3)",
                c.live, st.structural_bytes
            ));
        }
        let slack = c.live - st.structural_bytes as i64;
        let ceiling = (SIZE_CLASSES * masstree_slab_bytes()) as i64 + 16 * c.allocs;
        if slack > ceiling {
            void(&format!(
                "allocator census exceeds structural bytes by {slack} B, above the slab ceiling of {ceiling} B (§3.3)"
            ));
        }
        Some((c, st, slack, unsettled, settle_steps))
    } else {
        None
    };

    // Expanse, under the same instrument.
    let (exp_pop, exp) = Census::measure(|| match (arm, &int_keys, &str_pop) {
        (Arm::Map, Some(ks), _) => {
            let mut t = ExpanseMap::new();
            for (i, k) in ks.iter().enumerate() {
                t.insert(*k, value_of(i));
            }
            let p = t.len() as usize;
            std::mem::forget(t);
            p
        }
        (Arm::Sync, Some(ks), _) => {
            let t = SyncExpanseMap::new();
            for (i, k) in ks.iter().enumerate() {
                t.insert(*k, value_of(i));
            }
            let p = t.len() as usize;
            std::mem::forget(t);
            p
        }
        (Arm::Str, _, Some(ks)) => {
            let mut t = ExpanseStrMap::new();
            for (i, k) in ks.iter().enumerate() {
                t.insert(k.bytes(), value_of(i));
            }
            let p = t.len() as usize;
            std::mem::forget(t);
            p
        }
        _ => unreachable!(),
    });
    if exp_pop != pop {
        void(&format!("Expanse holds {exp_pop} of {pop} intended"));
    }
    if exp.live <= 0 {
        void("census saw no Expanse allocations");
    }

    // The engine's own accounting alongside the allocator figure.
    let exp_mem_used = match (arm, &int_keys, &str_pop) {
        (Arm::Map, Some(ks), _) => {
            let mut t = ExpanseMap::new();
            for (i, k) in ks.iter().enumerate() {
                t.insert(*k, value_of(i));
            }
            t.mem_used()
        }
        (Arm::Sync, Some(ks), _) => {
            let t = SyncExpanseMap::new();
            for (i, k) in ks.iter().enumerate() {
                t.insert(*k, value_of(i));
            }
            t.with_locked(ExpanseMap::mem_used)
        }
        (Arm::Str, _, Some(ks)) => {
            let mut t = ExpanseStrMap::new();
            for (i, k) in ks.iter().enumerate() {
                t.insert(k.bytes(), value_of(i));
            }
            t.mem_used()
        }
        _ => unreachable!(),
    };

    let pf = pop as f64;
    let (
        mt_alloc,
        mt_struct,
        mt_slack,
        mt_slabs,
        mt_allocs,
        quantum,
        leaves,
        internodes,
        layers,
        fill,
    ) = match &masstree {
        Some((c, st, slack, _, _)) => (
            Some(c.live as f64 / pf),
            Some(st.structural_bytes as f64 / pf),
            Some(*slack as f64 / pf),
            Some(c.memalign),
            Some(c.allocs),
            Some((*slack as u128 * 4) > st.structural_bytes.max(1) as u128),
            Some(st.leaves),
            Some(st.internodes),
            Some(st.layers),
            Some(pop as f64 / (st.leaves.max(1) as f64 * 15.0)),
        ),
        None => (None, None, None, None, None, None, None, None, None, None),
    };
    let (mt_unsettled, mt_settle_steps, mt_frees) = match &masstree {
        Some((c, _, _, unsettled, steps)) => (
            Some(*unsettled as f64 / pf),
            Some(i64::from(*steps)),
            Some(c.frees),
        ),
        None => (None, None, None),
    };
    let opt_i = |v: Option<i64>| v.map_or("null".to_string(), |x| x.to_string());
    let opt_u = |v: Option<u64>| v.map_or("null".to_string(), |x| x.to_string());
    println!(
        "{{\"workload_id\":\"{}\",\"arm\":\"{}\",\"dist\":\"{}\",\"order\":\"{}\",\"table\":\"{}\",\
         \"population\":{},\"lambda\":{},\"mean_key_len\":{},\"masstree_not_representable\":{},\
         \"masstree_alloc_bytes_per_key\":{},\"masstree_structural_bytes_per_key\":{},\
         \"masstree_slack_bytes_per_key\":{},\"masstree_unsettled_bytes_per_key\":{},\
         \"masstree_settle_steps\":{},\"masstree_frees\":{},\"masstree_slabs\":{},\"masstree_allocs\":{},\
         \"masstree_quantum_dominated\":{},\"masstree_leaves\":{},\"masstree_internodes\":{},\
         \"masstree_layers\":{},\"masstree_leaf_fill\":{},\
         \"expanse_alloc_bytes_per_key\":{:.4},\"expanse_mem_used_bytes_per_key\":{:.4},\
         \"expanse_allocs\":{},\"slab_bytes\":{}}}",
        arm.workload_id(),
        arm.label(),
        dist_name,
        order.name(),
        table.name(),
        pop,
        json_f64(lambda),
        json_f64(mean_len),
        not_rep,
        json_f64(mt_alloc),
        json_f64(mt_struct),
        json_f64(mt_slack),
        json_f64(mt_unsettled),
        opt_i(mt_settle_steps),
        opt_i(mt_frees),
        opt_i(mt_slabs),
        opt_i(mt_allocs),
        quantum.map_or("null".to_string(), |b| b.to_string()),
        opt_u(leaves),
        opt_u(internodes),
        opt_u(layers),
        json_f64(fill),
        exp.live as f64 / pf,
        exp_mem_used as f64 / pf,
        exp.allocs,
        masstree_slab_bytes(),
    );
}
