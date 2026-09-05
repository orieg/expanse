//! Validation gate for the string-key arms (#693) — must pass before any string
//! cell is recorded. Every check is on a deterministic invariant (walk counts,
//! round-trip counts, exact byte accounting), never on wall-clock (§8.4).
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `hot_string_validate` |
//! | `group` | 5 |
//! | `population` | 100k per shape for fidelity; 50k for the allocation-count checks; 1k for the truncation finding |
//! | `probes_and_reuse` | 50/50 shuffled stream, one pass, every probe its own allocation |
//! | `hit_rate` | 50% |
//! | `miss_gen_method` | same-generator rejection sampling (§8.6) |
//! | `value_dereference` | both sides return the stored word and the two sinks are compared |
//! | `measured_region` | N/A — deterministic checks, no timing |
//! | `arm_symmetry` | identical strings and probe stream on both sides of every pairing |
//! | `statistics` | exact counts; no interval (§8.4) |
//! | `verdict` | gate — pass/fail, never a published figure |
//!
//! Re-checks every silent-failure class the integer arms found (METHODOLOGY
//! §10.8) on the string path, and measures the one that is new: HOT's 255-byte
//! key window (§10.4).

use expanse_hot_bench::strings::{self, KeyStr, StrDist};
use expanse_hot_bench::{
    Census, HOT_STRING_KEY_WINDOW, HotStr, HotStrMap, hot_can_key, hot_string_key_window,
    pool_allocations, validate_census,
};
use expanse_trie::bytesmap::ExpanseBytesMap;
use expanse_trie::strmap::ExpanseStrMap;

const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

fn fail(msg: &str) -> ! {
    eprintln!("FAIL: {msg}");
    std::process::exit(1);
}

/// A key of exactly `len` bytes whose last byte is `last`, the rest `a`.
fn key_of(len: usize, last: u8) -> KeyStr {
    let mut v = vec![b'a'; len];
    v[len - 1] = last;
    KeyStr::new(&v)
}

fn main() {
    let mut checks = 0usize;

    // 0. The predicate constant agrees with HOT's header.
    let window = hot_string_key_window();
    if window != HOT_STRING_KEY_WINDOW {
        fail(&format!(
            "HOT_STRING_KEY_WINDOW is {HOT_STRING_KEY_WINDOW} but HOT's MAX_STRING_KEY_LENGTH is {window}"
        ));
    }
    checks += 1;
    println!(
        "ok  HOT discriminates {window} key bytes (MAX_STRING_KEY_LENGTH, read from the header)"
    );

    // 1. The census, on a cold pool, sees both arms — run FIRST because HOT's
    //    node pool is process-global (§9.2) and every later check warms it.
    if pool_allocations() != 0 {
        fail("HOT's pool is warm before anything ran");
    }
    {
        let w = strings::build_population(StrDist::Short, 100_000);
        let n = w.population.len();
        let (hot_len, hot) = Census::measure(|| {
            let mut t = HotStr::new();
            for k in &w.population {
                t.insert(k);
            }
            let len = t.len();
            std::mem::forget(t);
            len
        });
        let (exp_len, exp) = Census::measure(|| {
            let mut t = ExpanseStrMap::new();
            for k in &w.population {
                t.insert(k.bytes(), k.word());
            }
            let len = t.len() as usize;
            std::mem::forget(t);
            len
        });
        if hot_len != n || exp_len != n {
            fail(&format!(
                "census pass lost keys: HOT {hot_len}, Expanse {exp_len}, expected {n}"
            ));
        }
        if hot.live <= 0 || exp.live <= 0 {
            fail("census saw no allocations for one arm — the interposition is not reaching it");
        }
        // Arm C stores the pointer inline: a per-entry heap object on this arm
        // means the shim stopped doing so (§10.8).
        if hot.allocs >= n as i64 {
            fail(&format!(
                "Arm C HOT made {} allocations for {n} keys — a per-entry heap object appeared on the identity arm",
                hot.allocs
            ));
        }
        checks += 3;
        println!(
            "ok  one instrument sees both string arms at N={n} (short): HOT {:.2} B/key ({} allocs, < N), \
             ExpanseStrMap {:.2} B/key ({} allocs); pool was cold",
            hot.live as f64 / n as f64,
            hot.allocs,
            exp.live as f64 / n as f64,
            exp.allocs
        );
    }

    // 2. Census controls: the 1 MiB control, and a string-table control that
    //    must count exactly N allocations and return to zero after free.
    let control = validate_census(1 << 20);
    if !control.is_valid() {
        fail(&format!(
            "census control invalid: requested {} B, rose {} B, residual {} B",
            control.requested, control.alloc_delta, control.residual
        ));
    }
    {
        let n = 10_000usize;
        // The table's backing store is reserved before arming so only the
        // strings themselves are counted; the keys are formatted before arming
        // for the same reason.
        let keys: Vec<Vec<u8>> = (0..n).map(|i| format!("s{i:010}").into_bytes()).collect();
        let mut table: Vec<KeyStr> = Vec::with_capacity(n);
        Census::reset();
        Census::arm(true);
        for k in &keys {
            table.push(KeyStr::new(k));
        }
        std::hint::black_box(&table);
        let table_census = Census::read();
        table.clear();
        Census::arm(false);
        let freed = Census::read();
        if table_census.allocs != n as i64 {
            fail(&format!(
                "string-table control: {n} strings produced {} counted allocations",
                table_census.allocs
            ));
        }
        if freed.live != 0 || freed.frees != n as i64 {
            fail(&format!(
                "string-table control: after freeing {n} strings live is {} B and frees is {}",
                freed.live, freed.frees
            ));
        }
        checks += 3;
        println!(
            "ok  census controls: +{} B on a {} B request, residual {} B; {n} strings = {} allocations, \
             {:.2} B/string as allocated, live 0 after free",
            control.alloc_delta,
            control.requested,
            control.residual,
            table_census.allocs,
            table_census.live as f64 / n as f64
        );
    }

    // 3. The key window, measured at its boundary rather than read. Two keys
    //    of length L differing only in their last byte: HOT holds two entries
    //    iff L is inside the window.
    for len in [
        HOT_STRING_KEY_WINDOW - 1,
        HOT_STRING_KEY_WINDOW,
        HOT_STRING_KEY_WINDOW + 1,
        300,
    ] {
        let a = key_of(len, b'x');
        let b = key_of(len, b'y');
        let mut t = HotStr::new();
        let ins_a = t.insert(&a);
        let ins_b = t.insert(&b);
        let walk = t.len();
        let found_b = t.lookup(&b);
        let predicted = hot_can_key(len);
        let observed = walk == 2;
        if predicted != observed {
            fail(&format!(
                "key window boundary: len {len} predicted {} but HOT walks {walk} entries (insert a={ins_a}, b={ins_b})",
                if predicted {
                    "discriminable"
                } else {
                    "not discriminable"
                }
            ));
        }
        let note = if observed {
            "two entries — discriminated".to_string()
        } else {
            format!(
                "ONE entry; insert(b) returned {ins_b}; lookup(b) returns {}",
                match found_b {
                    Some(p) if p == a.word() => "a's pointer (a false positive)",
                    Some(_) => "b's pointer",
                    None => "not found",
                }
            )
        };
        println!("ok  key window: len {len:>3} → {note}");
        checks += 1;
    }

    // 4. The capability finding on the shape designed to fail the predicate:
    //    what HOT does with a `beyond` population, recorded rather than assumed.
    {
        let w = strings::build_population(StrDist::Beyond, 1_000);
        let n = w.population.len();
        let mut t = HotStr::new();
        let inserted = w.population.iter().filter(|k| t.insert(k)).count();
        let walk = t.len();
        let mut found = 0usize;
        let mut wrong_pointer = 0usize;
        for k in &w.population {
            if let Some(p) = t.lookup(k) {
                found += 1;
                if p != k.word() {
                    wrong_pointer += 1;
                }
            }
        }
        let frac = w.hot_representable_fraction();
        if frac != 0.0 {
            fail(&format!(
                "beyond: representable fraction is {frac}, expected 0"
            ));
        }
        // §8: population by walk differing from the intended population is the
        // signature of this failure; it must be visible, not silent.
        if walk == n {
            fail(&format!(
                "beyond: HOT walks {walk} of {n} keys that all exceed its window — the predicate is wrong"
            ));
        }
        checks += 2;
        println!(
            "ok  capability finding (beyond, 272 B keys sharing a 256 B prefix, N={n}): insert() reported {inserted} new, \
             trie walks {walk}; lookup() found {found} of {n} and returned another key's pointer for {wrong_pointer}"
        );
        // The Expanse side is unrestricted and must hold the whole population.
        let mut e = ExpanseStrMap::new();
        for k in &w.population {
            e.insert(k.bytes(), k.word());
        }
        if e.len() as usize != n
            || w.population
                .iter()
                .any(|k| e.get(k.bytes()) != Some(k.word()))
        {
            fail("beyond: ExpanseStrMap lost keys the workload contains");
        }
        checks += 1;
        println!("ok  beyond: ExpanseStrMap holds all {n} keys and returns each one's own word");
    }

    // 5. Population fidelity, answer agreement and sink equality on every
    //    shape HOT can represent, for all three pairings, with a 50/50 stream.
    for dist in StrDist::ALL {
        let w = strings::build(dist, 100_000, 0.5);
        let n = w.population.len();
        let hot_ok = w.hot_not_representable() == 0;

        // Expanse sides.
        let mut es = ExpanseStrMap::new();
        let mut em = ExpanseStrMap::new();
        let mut eb = ExpanseBytesMap::new();
        for (i, k) in w.population.iter().enumerate() {
            es.insert(k.bytes(), k.word());
            em.insert(k.bytes(), (i as u64).wrapping_mul(GOLDEN));
            eb.insert(k.bytes(), k.word());
        }
        if es.len() as usize != n || em.len() as usize != n || eb.len() as usize != n {
            fail(&format!(
                "{}: Expanse populations differ from the stream",
                dist.name()
            ));
        }

        if !hot_ok {
            println!(
                "ok  {:<8} N={n}: HOT column withheld ({} keys exceed the {HOT_STRING_KEY_WINDOW} B window); \
                 Expanse arms hold {n}/{n}",
                dist.name(),
                w.hot_not_representable()
            );
            checks += 1;
            continue;
        }

        let mut hs = HotStr::new();
        let mut hm = HotStrMap::new();
        for (i, k) in w.population.iter().enumerate() {
            hs.insert(k);
            hm.insert(k, (i as u64).wrapping_mul(GOLDEN));
        }
        if hs.len() != n || hm.len() != n {
            fail(&format!(
                "{}: HOT walks {} (identity) / {} (pair) of {n}",
                dist.name(),
                hs.len(),
                hm.len()
            ));
        }
        // Every population pointer must fit HOT's inline payload (bit 63 clear).
        if w.population.iter().any(|k| k.word() >> 63 != 0) {
            fail("a key pointer has bit 63 set; HOT's inline payload cannot hold it");
        }

        let (mut hits, mut dis_c, mut dis_d, mut dis_e) = (0usize, 0usize, 0usize, 0usize);
        let (mut sh, mut se, mut sb, mut shm, mut sem) = (0u64, 0u64, 0u64, 0u64, 0u64);
        for p in &w.probes {
            let h = hs.lookup(p);
            let e = es.get(p.bytes());
            let b = eb.get(p.bytes());
            let hmv = hm.get(p);
            let emv = em.get(p.bytes());
            if h.is_some() {
                hits += 1;
            }
            if h != e {
                dis_c += 1;
            }
            if hmv != emv {
                dis_d += 1;
            }
            if h != b {
                dis_e += 1;
            }
            sh ^= h.unwrap_or(0);
            se ^= e.unwrap_or(0);
            sb ^= b.unwrap_or(0);
            shm ^= hmv.unwrap_or(0);
            sem ^= emv.unwrap_or(0);
        }
        if dis_c != 0 || dis_d != 0 || dis_e != 0 {
            fail(&format!(
                "{}: probe disagreements — Arm C {dis_c}, Arm D {dis_d}, Arm E {dis_e}",
                dist.name()
            ));
        }
        if sh != se || sh != sb || shm != sem {
            fail(&format!("{}: sinks differ between arms", dist.name()));
        }
        if hits != w.hits {
            fail(&format!(
                "{}: {hits} hits observed, {} intended",
                dist.name(),
                w.hits
            ));
        }
        // Scan agreement (Arm C): same starts, same k, same visited counts and
        // the same folded pointers.
        let mut scan_dis = 0usize;
        for start in w.probes.iter().take(100) {
            let (hn, hsink) = hs.scan(start, 100);
            let mut en = 0usize;
            let mut esink = 0u64;
            let mut cur = es.next_at_or_after(start.bytes());
            while let Some((key, slot)) = cur {
                // SAFETY: the slot is live until the next mutation; none occurs.
                esink ^= unsafe { *slot.as_ptr() };
                en += 1;
                if en == 100 {
                    break;
                }
                cur = es.next_after(&key);
            }
            if hn != en || hsink != esink {
                scan_dis += 1;
            }
        }
        if scan_dis != 0 {
            fail(&format!(
                "{}: {scan_dis} of 100 scans disagree between HOT and ExpanseStrMap",
                dist.name()
            ));
        }
        checks += 5;
        println!(
            "ok  {:<8} N={n} mean len {:.1}: populations agree, {} probes agree on all three arms \
             ({hits} hits, sinks equal), 100 scans of k=100 agree",
            dist.name(),
            w.mean_len(),
            w.probes.len()
        );
    }

    // 6. The census sees Arm D's `operator new` pairs (§9.7 on the string path).
    {
        let w = strings::build_population(StrDist::Short, 50_000);
        let n = w.population.len();
        let ((), c) = Census::measure(|| {
            let mut t = HotStrMap::new();
            for (i, k) in w.population.iter().enumerate() {
                t.insert(k, i as u64);
            }
            std::mem::forget(t);
        });
        if c.allocs < n as i64 {
            fail(&format!(
                "census missed `operator new`: {n} heap pairs produced {} counted allocations",
                c.allocs
            ));
        }
        checks += 1;
        println!(
            "ok  census sees Arm D's operator new: {} allocations for {n} heap pairs",
            c.allocs
        );
    }

    // 7. Expanse's independence from the string table (§10.3): build from copy
    //    A, free A, probe with a byte-identical copy B.
    {
        let a = strings::build_population(StrDist::Short, 20_000);
        let words: Vec<u64> = a.population.iter().map(KeyStr::word).collect();
        let b: Vec<KeyStr> = a
            .population
            .iter()
            .map(|k| KeyStr::new(k.bytes()))
            .collect();
        let mut es = ExpanseStrMap::new();
        let mut eb = ExpanseBytesMap::new();
        for (k, wd) in a.population.iter().zip(&words) {
            es.insert(k.bytes(), *wd);
            eb.insert(k.bytes(), *wd);
        }
        drop(a);
        let bad = b
            .iter()
            .zip(&words)
            .filter(|(k, wd)| es.get(k.bytes()) != Some(**wd) || eb.get(k.bytes()) != Some(**wd))
            .count();
        if bad != 0 {
            fail(&format!(
                "{bad} of {} probes failed after the source strings were freed",
                b.len()
            ));
        }
        checks += 1;
        println!(
            "ok  Expanse arms answer all {} probes after the strings they were built from were freed \
             (HOT cannot: its leaves point into them)",
            b.len()
        );
    }

    println!("\n{checks} deterministic checks passed. String-arm foundation is valid.");
    println!(
        "NOTE: the B/key figures above are a validation sample, not a suite artifact. \
         Published cells come from hot_string_memory / hot_string_latency on the reference host."
    );
}
