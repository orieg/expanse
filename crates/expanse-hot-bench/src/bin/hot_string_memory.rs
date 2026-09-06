//! String-key memory pillar (#693): bytes per key for the index, the external
//! key storage, and ownership, for the three string pairings of METHODOLOGY
//! §10.2, under the key-ownership rule of §10.3.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `hot_string_memory` |
//! | `group` | 5 |
//! | `population` | swept by the runner (§10.5); one cell per invocation |
//! | `probes_and_reuse` | N/A (memory census) |
//! | `hit_rate` | N/A |
//! | `miss_gen_method` | N/A |
//! | `value_dereference` | N/A — live bytes held from the C allocator |
//! | `measured_region` | build only; census armed around each index build and, separately, around the string table; teardown excluded |
//! | `arm_symmetry` | one allocator interposition measures both indexes (§9.1); the harness-owned strings are counted on neither side and published as their own column (§10.3); both arms see the same insertion order, `sorted` unless the cell says `shuffled` (§12.2) |
//! | `statistics` | exact byte counts, deterministic — no interval (§8.4) |
//! | `verdict` | pending measurement |
//!
//! ## Three columns, never one
//!
//! HOT stores a pointer into a string the harness owns; Expanse copies the key
//! bytes. So every cell reports the index alone (both arms, one instrument),
//! the external key storage (exact `Σ (len+1)`, and as the allocator holds it),
//! and ownership — HOT's index plus the strings its leaves point at, Expanse's
//! index alone. Publishing one column would measure the harness's string table
//! on one side and not the other.
//!
//! ## One process per cell
//!
//! HOT's node pool is process-global (§9.2); the binary refuses a warm pool.

use std::env;

use expanse_hot_bench::strings::{self, KeyStr, StrDist};
use expanse_hot_bench::workload::{self, Order};
use expanse_hot_bench::{Census, HotStr, HotStrMap, require_cold_pool, validate_census};
use expanse_trie::bytesmap::ExpanseBytesMap;
use expanse_trie::strmap::ExpanseStrMap;

const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

#[derive(Copy, Clone, PartialEq, Eq)]
enum Arm {
    Ptr,
    Map,
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

fn value_for(arm: Arm, k: &KeyStr, i: usize) -> u64 {
    match arm {
        Arm::Ptr | Arm::Bytes => k.word(),
        Arm::Map => (i as u64).wrapping_mul(GOLDEN),
    }
}

fn void(msg: &str) -> ! {
    eprintln!("{msg}; cell is void");
    std::process::exit(1);
}

fn json_f64(v: Option<f64>) -> String {
    v.map_or("null".to_string(), |x| format!("{x:.4}"))
}

fn json_i64(v: Option<i64>) -> String {
    v.map_or("null".to_string(), |x| x.to_string())
}

fn main() {
    let mut args: Vec<String> = env::args().collect();
    // Trailing token: insertion order (§12.2). The allocator census moves with
    // build order; the engine's own node census must not.
    let mut order = Order::Sorted;
    while let Some(last) = args.last().map(String::as_str) {
        match Order::parse(last) {
            Some(o) => order = o,
            None => break,
        }
        args.pop();
    }
    if args.len() != 4 {
        eprintln!(
            "usage: hot_string_memory <ptr|map|bytes> <short|counter|prefixed|skewed|beyond> \
             <population> [sorted|shuffled]"
        );
        eprintln!("  insertion order defaults to `sorted` (§12.2)");
        eprintln!("  one cell per invocation — HOT's node pool is process-global (§9.2)");
        std::process::exit(2);
    }
    let arm = match args[1].as_str() {
        "ptr" => Arm::Ptr,
        "map" => Arm::Map,
        "bytes" => Arm::Bytes,
        other => {
            eprintln!("unknown arm {other:?}");
            std::process::exit(2);
        }
    };
    let dist = StrDist::parse(&args[2]).unwrap_or_else(|| {
        eprintln!("unknown shape {:?}", args[2]);
        std::process::exit(2);
    });
    let n: usize = args[3].parse().unwrap_or_else(|_| {
        eprintln!("population must be an integer");
        std::process::exit(2);
    });

    let control = validate_census(1 << 20);
    if !control.is_valid() {
        void(&format!(
            "census control invalid (+{} B on {} B, residual {})",
            control.alloc_delta, control.requested, control.residual
        ));
    }
    require_cold_pool("hot_string_memory");

    // Generation happens with the census disarmed. The table both indexes are
    // then built from is a fresh copy made under its own census, so the
    // "external key storage as allocated" column is exactly the allocations
    // HOT's leaves point at (§10.3) — the copy's backing vector is reserved
    // before arming, so only the strings themselves are counted.
    let mut generated = strings::build_population(dist, n);
    if order == Order::Shuffled {
        workload::shuffle_in_place(&mut generated.population);
    }
    let pop = generated.population.len();
    let not_rep = generated.hot_not_representable();
    let hot_side = not_rep == 0;
    let key_bytes = generated.key_bytes();

    let mut table: Vec<KeyStr> = Vec::with_capacity(pop);
    let ((), ext) = Census::measure(|| {
        for k in &generated.population {
            table.push(KeyStr::new(k.bytes()));
        }
    });
    if ext.allocs != pop as i64 {
        void(&format!(
            "string table: {pop} strings produced {} counted allocations",
            ext.allocs
        ));
    }
    drop(generated);

    if hot_side && table.iter().any(|k| k.word() >> 63 != 0) {
        void("a key pointer has bit 63 set; HOT's inline payload cannot hold it");
    }

    // HOT index, on a cold pool, only where the §10.4 predicate holds.
    let hot = if hot_side {
        let (hot_pop, c) = Census::measure(|| match arm {
            Arm::Ptr | Arm::Bytes => {
                let mut t = HotStr::new();
                for k in &table {
                    t.insert(k);
                }
                let p = t.len();
                std::mem::forget(t);
                p
            }
            Arm::Map => {
                let mut t = HotStrMap::new();
                for (i, k) in table.iter().enumerate() {
                    t.insert(k, value_for(arm, k, i));
                }
                let p = t.len();
                std::mem::forget(t);
                p
            }
        });
        if hot_pop != pop {
            void(&format!("HOT walks {hot_pop} of {pop} intended"));
        }
        // Per-arm allocation-count assertions (§10.8): the identity arms store
        // the pointer inline and must not allocate per entry; the pair arm
        // must, and the census must see it.
        match arm {
            Arm::Ptr | Arm::Bytes if c.allocs >= pop as i64 => void(&format!(
                "identity arm made {} allocations for {pop} keys — a per-entry heap object appeared",
                c.allocs
            )),
            Arm::Map if c.allocs < pop as i64 => void(&format!(
                "pair arm made {} allocations for {pop} keys — the census is not seeing operator new",
                c.allocs
            )),
            _ => {}
        }
        Some(c)
    } else {
        None
    };

    // Expanse index.
    let (exp_pop, exp) = Census::measure(|| match arm {
        Arm::Ptr | Arm::Map => {
            let mut t = ExpanseStrMap::new();
            for (i, k) in table.iter().enumerate() {
                t.insert(k.bytes(), value_for(arm, k, i));
            }
            let p = t.len() as usize;
            std::mem::forget(t);
            p
        }
        Arm::Bytes => {
            let mut t = ExpanseBytesMap::new();
            for (i, k) in table.iter().enumerate() {
                t.insert(k.bytes(), value_for(arm, k, i));
            }
            let p = t.len() as usize;
            std::mem::forget(t);
            p
        }
    });
    if exp_pop != pop {
        void(&format!("Expanse holds {exp_pop} of {pop} intended"));
    }
    if exp.live <= 0 {
        void("census saw no Expanse allocations");
    }

    // The engine's own accounting alongside the allocator figure (§9.3).
    let exp_mem_used = match arm {
        Arm::Ptr | Arm::Map => {
            let mut t = ExpanseStrMap::new();
            for (i, k) in table.iter().enumerate() {
                t.insert(k.bytes(), value_for(arm, k, i));
            }
            t.mem_used()
        }
        Arm::Bytes => {
            let mut t = ExpanseBytesMap::new();
            for (i, k) in table.iter().enumerate() {
                t.insert(k.bytes(), value_for(arm, k, i));
            }
            t.mem_used()
        }
    };

    let pf = pop as f64;
    let mean_len = table.iter().map(KeyStr::len).sum::<usize>() as f64 / pf;
    let ext_per_key = ext.live as f64 / pf;
    let hot_index = hot.map(|c| c.live as f64 / pf);
    let exp_index = exp.live as f64 / pf;
    println!(
        "{{\"workload_id\":\"{}\",\"arm\":\"{}\",\"dist\":\"{}\",\"order\":\"{}\",\"population\":{},\
         \"mean_key_len\":{:.2},\"hot_not_representable\":{},\
         \"key_bytes_per_key\":{:.4},\"external_alloc_bytes_per_key\":{:.4},\
         \"hot_index_bytes_per_key\":{},\"expanse_index_bytes_per_key\":{:.4},\
         \"hot_ownership_bytes_per_key\":{},\"expanse_ownership_bytes_per_key\":{:.4},\
         \"expanse_mem_used_bytes_per_key\":{:.4},\
         \"hot_allocs\":{},\"expanse_allocs\":{},\"external_allocs\":{}}}",
        arm.workload_id(),
        arm.label(),
        dist.name(),
        order.name(),
        pop,
        mean_len,
        not_rep,
        key_bytes as f64 / pf,
        ext_per_key,
        json_f64(hot_index),
        exp_index,
        json_f64(hot_index.map(|h| h + ext_per_key)),
        exp_index,
        exp_mem_used as f64 / pf,
        json_i64(hot.map(|c| c.allocs)),
        exp.allocs,
        ext.allocs,
    );
}
