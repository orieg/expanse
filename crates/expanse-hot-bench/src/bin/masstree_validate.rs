//! Validation gate for the Masstree arm (#661) — must pass before any Masstree
//! cell is recorded. Every check is on a deterministic invariant (walk counts,
//! round-trip counts, exact byte accounting), never on wall-clock (§8.4).
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `masstree_validate` |
//! | `group` | 5 |
//! | `population` | 200k per integer distribution and 100k per string shape for fidelity; 1k for the key-length finding; 200k prefill + 200k fresh for the threaded checks; 100k for the census |
//! | `probes_and_reuse` | 50/50 shuffled stream, one pass, every string probe its own allocation |
//! | `hit_rate` | 50% |
//! | `miss_gen_method` | same-generator rejection sampling (§8.6) |
//! | `value_dereference` | both sides return the stored value and the two sinks are compared |
//! | `measured_region` | N/A — deterministic checks, no timing |
//! | `arm_symmetry` | identical keys and probe stream on both sides of every pairing |
//! | `statistics` | exact counts; no interval (§8.4) |
//! | `verdict` | gate — pass/fail, never a published figure |
//!
//! Re-checks on Masstree every silent-failure class the HOT arms found
//! (`hot_comparison` §10.8) and the ones the Masstree Step 0 gate added
//! (METHODOLOGY §2): the key-length contract, the slab-quantized census, the
//! reused-slot hazard, and the threading model driven from foreign threads.

use std::sync::Barrier;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use expanse_hot_bench::masstree::{
    MASSTREE_MAX_KEY_LEN, Masstree, MtThread, QUIESCE_EVERY, StrInsert, Table, masstree_can_key,
    masstree_max_key_len, masstree_node_bytes, masstree_slab_bytes,
};
use expanse_hot_bench::strings::{self, StrDist};
use expanse_hot_bench::workload::{self, Dist};
use expanse_hot_bench::{Census, probe_aligned_usable, validate_census};
use expanse_trie::map::ExpanseMap;
use expanse_trie::strmap::ExpanseStrMap;

const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

fn fail(msg: &str) -> ! {
    eprintln!("FAIL: {msg}");
    std::process::exit(1);
}

#[inline]
fn value_of(i: usize) -> u64 {
    (i as u64).wrapping_mul(GOLDEN)
}

fn main() {
    let mut checks = 0usize;

    // 0. The constants the envelope pins agree with the library.
    if masstree_max_key_len() != MASSTREE_MAX_KEY_LEN {
        fail(&format!(
            "MASSTREE_MAXKEYLEN is {} in the header, {MASSTREE_MAX_KEY_LEN} in the predicate",
            masstree_max_key_len()
        ));
    }
    let (leaf, inode) = masstree_node_bytes();
    if leaf != 320 || inode != 320 || masstree_slab_bytes() != (2 << 20) {
        fail(&format!(
            "layout drifted from scripts/masstree_envelope.py: leaf {leaf} B, internode {inode} B, slab {} B",
            masstree_slab_bytes()
        ));
    }
    checks += 2;
    println!(
        "ok  constants: MASSTREE_MAXKEYLEN {MASSTREE_MAX_KEY_LEN}, leaf/internode {leaf}/{inode} B pooled, slab {} MiB",
        masstree_slab_bytes() >> 20
    );

    // 1. The census instrument.
    let control = validate_census(1 << 20);
    if !control.is_valid() {
        fail(&format!(
            "census control invalid: requested {} B, counter rose {} B, residual {} B",
            control.requested, control.alloc_delta, control.residual
        ));
    }
    checks += 1;
    println!(
        "ok  census control: +{} B on a {} B request, residual {} B after free",
        control.alloc_delta, control.requested, control.residual
    );

    // 1b. The §10.1 mechanism, measured every run: what glibc reports as usable
    //     for a 2 MiB request at 2 MiB alignment (Masstree's pool slab) versus
    //     for a 64-byte-aligned node-sized request (HOT's and Expanse's shape).
    let slab = masstree_slab_bytes();
    let usable_slab = probe_aligned_usable(slab, slab);
    let usable_node = probe_aligned_usable(64, 320);
    if usable_slab < slab || usable_node < 320 {
        fail(&format!(
            "glibc probe returned less than requested: {usable_slab} for {slab}, {usable_node} for 320"
        ));
    }
    checks += 1;
    println!(
        "ok  glibc memalign probe: a {} MiB request at {} MiB alignment reports {} B usable ({:+} B of \
         padding, counted at the requested size by the side table); a 64-aligned 320 B request reports {} B",
        slab >> 20,
        slab >> 20,
        usable_slab,
        usable_slab as i64 - slab as i64,
        usable_node
    );

    // 2. The census, FIRST, on fresh slots (§3.6): allocator vs structural, in
    //    both table configurations (§10.3), settled (§10.4).
    for (table, slot) in [(Table::Single, 41u32), (Table::Concurrent, 44u32)] {
        let w = workload::build(Dist::Random, 100_000, 64, 0.0);
        let n = w.population.len();
        let ((t, ti, walked, unsettled, steps), c) = Census::measure(|| {
            let ti = MtThread::slot(slot);
            ti.enter();
            let t = Masstree::new(ti, table);
            for (i, k) in w.population.iter().enumerate() {
                t.insert(ti, *k, value_of(i));
                if (i as u64 + 1).is_multiple_of(QUIESCE_EVERY) {
                    ti.quiesce();
                }
            }
            let unsettled = Census::read().live;
            let (mut steps, mut frees) = (0u32, Census::read().frees);
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
        let st = t.stats(ti);
        ti.exit();
        std::mem::forget(t);
        if walked != n || st.size as usize != n {
            fail(&format!(
                "census build: walked {walked}, stats {}, intended {n}",
                st.size
            ));
        }
        if c.memalign < 1 {
            fail("census saw no posix_memalign slab from Masstree's pool");
        }
        if c.live < st.structural_bytes as i64 {
            fail(&format!(
                "allocator census {} B below structural {} B — the census is not seeing the arm",
                c.live, st.structural_bytes
            ));
        }
        let slack = c.live - st.structural_bytes as i64;
        if slack > (20 * masstree_slab_bytes()) as i64 + 16 * c.allocs {
            fail(&format!("slack {slack} B exceeds the slab ceiling"));
        }
        // Every slab is counted at exactly its requested size (§10.1).
        let slabs_bytes = c.memalign * masstree_slab_bytes() as i64;
        if c.live < slabs_bytes || c.live - slabs_bytes > 64 * 1024 + 16 * c.allocs {
            fail(&format!(
                "census {} B does not decompose into {} slabs of {} B plus small mallocs",
                c.live,
                c.memalign,
                masstree_slab_bytes()
            ));
        }
        checks += 4;
        println!(
            "ok  census ({}) at N={n} random u64: Masstree {:.2} B/key allocator ({} slabs, {} allocs, {} frees after \
             {steps} settle steps; {:.2} B/key before settling) vs {:.2} B/key structural ({} leaves, {} internodes, \
             slack {:.2} B/key, quantum-dominated: {})",
            table.name(),
            c.live as f64 / n as f64,
            c.memalign,
            c.allocs,
            c.frees,
            unsettled as f64 / n as f64,
            st.structural_bytes as f64 / n as f64,
            st.leaves,
            st.internodes,
            slack as f64 / n as f64,
            slack as u128 * 4 > st.structural_bytes as u128
        );
    }
    {
        let w = workload::build(Dist::Random, 100_000, 64, 0.0);
        let (_, e) = Census::measure(|| {
            let mut m = ExpanseMap::new();
            for (i, k) in w.population.iter().enumerate() {
                m.insert(*k, value_of(i));
            }
            std::mem::forget(m);
        });
        println!(
            "ok  census ExpanseMap at N={}: {:.2} B/key ({} allocs)",
            w.population.len(),
            e.live as f64 / w.population.len() as f64,
            e.allocs
        );
    }
    // 2b. RCU deferral on a suffix-heavy build (§10.4): settling must free
    //     the superseded suffix bags a `prefixed` build leaves in limbo.
    {
        let w = strings::build_population(StrDist::Prefixed, 100_000);
        let n = w.population.len();
        let ((t, ti, unsettled), c) = Census::measure(|| {
            let ti = MtThread::slot(45);
            ti.enter();
            let t = Masstree::new(ti, Table::Single);
            for (i, k) in w.population.iter().enumerate() {
                t.str_insert(ti, k.bytes(), value_of(i));
                if (i as u64 + 1).is_multiple_of(QUIESCE_EVERY) {
                    ti.quiesce();
                }
            }
            let unsettled = Census::read().live;
            let (mut steps, mut frees) = (0u32, Census::read().frees);
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
            (t, ti, unsettled)
        });
        let st = t.stats(ti);
        ti.exit();
        std::mem::forget(t);
        if c.live > unsettled {
            fail("settling increased the census");
        }
        checks += 1;
        println!(
            "ok  RCU settle on `prefixed` N={n}: {:.2} B/key before, {:.2} B/key after ({} frees reclaimed); structural {:.2} B/key, {} layers",
            unsettled as f64 / n as f64,
            c.live as f64 / n as f64,
            c.frees,
            st.structural_bytes as f64 / n as f64,
            st.layers
        );
    }

    // 3. Integer fidelity and answer agreement, structured and unstructured.
    let ti = MtThread::slot(0);
    ti.enter();
    for dist in [Dist::Sequential, Dist::Random] {
        let w = workload::build(dist, 200_000, 64, 0.5);
        let n = w.population.len();
        let t = Masstree::new(ti, Table::Single);
        let mut e = ExpanseMap::new();
        let mut newly = 0usize;
        for (i, k) in w.population.iter().enumerate() {
            newly += usize::from(t.insert(ti, *k, value_of(i)));
            e.insert(*k, value_of(i));
            if (i as u64 + 1).is_multiple_of(QUIESCE_EVERY) {
                ti.quiesce();
            }
        }
        if newly != n || t.len(ti) != n || e.len() as usize != n {
            fail(&format!(
                "{}: Masstree new {newly} walk {}, Expanse {}, intended {n}",
                dist.name(),
                t.len(ti),
                e.len()
            ));
        }
        let (mut mt_sink, mut ex_sink, mut disagree) = (0u64, 0u64, 0usize);
        for p in &w.probes {
            let a = t.get(ti, *p);
            let b = e.get(*p);
            disagree += usize::from(a != b);
            mt_sink ^= a.unwrap_or(0);
            ex_sink ^= b.unwrap_or(0);
        }
        if disagree != 0 || mt_sink != ex_sink {
            fail(&format!(
                "{}: {disagree} probes disagree; sinks equal: {}",
                dist.name(),
                mt_sink == ex_sink
            ));
        }
        // Scan agreement from 200 probe-drawn starts.
        for s in w.probes.iter().take(200) {
            let (c, sink) = t.scan(ti, *s, 100);
            let (mut ce, mut se) = (0usize, 0u64);
            for (_, v) in e.range(*s..=u64::MAX) {
                se ^= v;
                ce += 1;
                if ce == 100 {
                    break;
                }
            }
            if c != ce || sink != se {
                fail(&format!(
                    "{}: scan from {s} — Masstree {c}, Expanse {ce}, sinks equal {}",
                    dist.name(),
                    sink == se
                ));
            }
        }
        if t.iterate_xor(ti) != e.iter().fold(0u64, |acc, (_, v)| acc ^ v) {
            fail(&format!("{}: full-iteration sinks differ", dist.name()));
        }
        checks += 4;
        println!(
            "ok  integer fidelity {}: {n}/{n} on both sides, every probe agrees, 200 scans and the full walk agree",
            dist.name()
        );
    }

    // 4. Edge keys and values: the full 64-bit domain, value 0 included.
    {
        let t = Masstree::new(ti, Table::Single);
        let cases: [(u64, u64); 5] = [
            (0, 0),
            (1 << 63, 1 << 63),
            (u64::MAX, u64::MAX),
            (42, 0),
            ((1 << 62) + 7, 1),
        ];
        for (k, v) in cases {
            t.insert(ti, k, v);
        }
        let ok = cases
            .iter()
            .filter(|(k, v)| t.get(ti, *k) == Some(*v))
            .count();
        if ok != cases.len() || t.len(ti) != cases.len() {
            fail(&format!(
                "edge keys: {ok}/{} round-trip, walk {}",
                cases.len(),
                t.len(ti)
            ));
        }
        checks += 1;
        println!(
            "ok  edge keys: key 0 with value 0, bit 63, u64::MAX all round-trip — no payload predicate on integers"
        );
    }

    // 5. String fidelity, agreement and the §3.4 predicate.
    for dist in StrDist::ALL {
        let n = if dist == StrDist::Beyond {
            1_000
        } else {
            100_000
        };
        let w = strings::build(dist, n, 0.5);
        let pop = w.population.len();
        let not_rep = w.not_representable(masstree_can_key);
        if dist == StrDist::Beyond {
            if not_rep != pop {
                fail(&format!(
                    "beyond: predicate rejects {not_rep} of {pop}; expected all"
                ));
            }
            // What the shim does with a key beyond the contract: refuses it.
            let t = Masstree::new(ti, Table::Single);
            let refused = w
                .population
                .iter()
                .filter(|k| t.str_insert(ti, k.bytes(), 1) == StrInsert::NotRepresentable)
                .count();
            if refused != pop || t.len(ti) != 0 {
                fail(&format!(
                    "beyond: shim refused {refused} of {pop}, table holds {}",
                    t.len(ti)
                ));
            }
            checks += 2;
            println!(
                "ok  beyond ({} B keys): predicate rejects {pop}/{pop}; the shim refuses every one and stores nothing (§3.4)",
                w.mean_len() as usize
            );
            continue;
        }
        if not_rep != 0 {
            fail(&format!(
                "{}: {not_rep} keys beyond the predicate in a representable shape",
                dist.name()
            ));
        }
        let t = Masstree::new(ti, Table::Single);
        let mut e = ExpanseStrMap::new();
        for (i, k) in w.population.iter().enumerate() {
            if t.str_insert(ti, k.bytes(), value_of(i)) != StrInsert::Inserted {
                fail(&format!("{}: key {i} not newly inserted", dist.name()));
            }
            e.insert(k.bytes(), value_of(i));
            if (i as u64 + 1).is_multiple_of(QUIESCE_EVERY) {
                ti.quiesce();
            }
        }
        if t.len(ti) != pop || e.len() as usize != pop {
            fail(&format!(
                "{}: Masstree walks {}, Expanse {}, intended {pop}",
                dist.name(),
                t.len(ti),
                e.len()
            ));
        }
        let (mut a_sink, mut b_sink, mut disagree) = (0u64, 0u64, 0usize);
        for p in &w.probes {
            let a = t.str_get(ti, p.bytes());
            let b = e.get(p.bytes());
            disagree += usize::from(a != b);
            a_sink ^= a.unwrap_or(0);
            b_sink ^= b.unwrap_or(0);
        }
        if disagree != 0 || a_sink != b_sink {
            fail(&format!("{}: {disagree} probes disagree", dist.name()));
        }
        for s in w.probes.iter().take(200) {
            let (c, sink) = t.str_scan(ti, s.bytes(), 100);
            let (mut ce, mut se) = (0usize, 0u64);
            let mut cur = e.next_at_or_after(s.bytes());
            while let Some((key, slot)) = cur {
                // SAFETY: valid until the next structural mutation; none occurs.
                se ^= unsafe { *slot.as_ptr() };
                ce += 1;
                if ce == 100 {
                    break;
                }
                cur = e.next_after(&key);
            }
            if c != ce || sink != se {
                fail(&format!(
                    "{}: string scan — Masstree {c}, Expanse {ce}, sinks equal {}",
                    dist.name(),
                    sink == se
                ));
            }
        }
        let st = t.stats(ti);
        checks += 3;
        println!(
            "ok  string fidelity {}: {pop}/{pop} on both sides, probes and 200 scans agree; Masstree {} layers, {} leaves, {:.2} B/key structural",
            dist.name(),
            st.layers,
            st.leaves,
            st.structural_bytes as f64 / pop as f64
        );
    }
    ti.exit();

    // 6. Threading from foreign threads (§3.2): disjoint writers, then readers
    //    alongside writers, verified from a slot that never inserted.
    {
        let cw = workload::build_concurrent(200_000, 200_000, 64, 0.5);
        let t = Masstree::new(MtThread::slot(32), Table::Concurrent);
        let w = 8usize;
        let per = cw.base.population.len() / w;
        std::thread::scope(|sc| {
            for i in 0..w {
                let lo = i * per;
                let hi = if i + 1 == w {
                    cw.base.population.len()
                } else {
                    lo + per
                };
                let slice = &cw.base.population[lo..hi];
                let t = &t;
                sc.spawn(move || {
                    let c = MtThread::slot(i as u32);
                    c.enter();
                    for (j, k) in slice.iter().enumerate() {
                        t.insert(c, *k, k.wrapping_mul(GOLDEN));
                        if (j as u64 + 1).is_multiple_of(QUIESCE_EVERY) {
                            c.quiesce();
                        }
                    }
                    c.exit();
                });
            }
        });
        let v = MtThread::slot(40);
        v.enter();
        let found = cw
            .base
            .population
            .iter()
            .filter(|k| t.get(v, **k) == Some(k.wrapping_mul(GOLDEN)))
            .count();
        let walked = t.len(v);
        v.exit();
        if found != cw.base.population.len() || walked != cw.base.population.len() {
            fail(&format!(
                "threaded build: {found} found, {walked} walked of {}",
                cw.base.population.len()
            ));
        }
        // Readers alongside writers.
        let stop = AtomicBool::new(false);
        let errors = AtomicU64::new(0);
        let reads = AtomicU64::new(0);
        let barrier = Barrier::new(16);
        std::thread::scope(|sc| {
            let per = cw.new_keys.len() / 8;
            let mut wh = Vec::new();
            for i in 0..8usize {
                let lo = i * per;
                let hi = if i == 7 { cw.new_keys.len() } else { lo + per };
                let slice = &cw.new_keys[lo..hi];
                let (t, barrier) = (&t, &barrier);
                wh.push(sc.spawn(move || {
                    let c = MtThread::slot(i as u32);
                    c.enter();
                    barrier.wait();
                    for (j, k) in slice.iter().enumerate() {
                        t.insert(c, *k, k.wrapping_mul(GOLDEN));
                        if (j as u64 + 1).is_multiple_of(QUIESCE_EVERY) {
                            c.quiesce();
                        }
                    }
                    c.exit();
                }));
            }
            for r in 0..8usize {
                let (t, barrier, stop, errors, reads) = (&t, &barrier, &stop, &errors, &reads);
                let probes = &cw.base.population;
                sc.spawn(move || {
                    let c = MtThread::slot(16 + r as u32);
                    c.enter();
                    let mut i = r * probes.len() / 8;
                    let (mut n, mut err) = (0u64, 0u64);
                    barrier.wait();
                    while !stop.load(Ordering::Relaxed) {
                        let k = probes[i];
                        if t.get(c, k) != Some(k.wrapping_mul(GOLDEN)) {
                            err += 1;
                        }
                        n += 1;
                        if n.is_multiple_of(QUIESCE_EVERY) {
                            c.quiesce();
                        }
                        i += 1;
                        if i == probes.len() {
                            i = 0;
                        }
                    }
                    c.exit();
                    reads.fetch_add(n, Ordering::Relaxed);
                    errors.fetch_add(err, Ordering::Relaxed);
                });
            }
            // Writers finish their fixed work; readers run until told to stop.
            for h in wh {
                h.join().expect("writer thread panicked");
            }
            stop.store(true, Ordering::Relaxed);
        });
        let v = MtThread::slot(40);
        v.enter();
        let walked = t.len(v);
        v.exit();
        let expected = cw.base.population.len() + cw.new_keys.len();
        if walked != expected {
            fail(&format!(
                "readers-alongside-writers: walked {walked}, expected {expected}"
            ));
        }
        checks += 2;
        println!(
            "ok  threading: 8 writers on own slots, {}/{} verified from a non-inserting slot; 8 readers alongside 8 writers made {} reads with {} errors; {expected} walked",
            found,
            cw.base.population.len(),
            reads.load(Ordering::Relaxed),
            errors.load(Ordering::Relaxed)
        );
        if errors.load(Ordering::Relaxed) != 0 {
            fail("readers observed wrong values under concurrent writers");
        }
    }

    // 7. The reused-slot hazard (§3.6), demonstrated: drop a table, rebuild on
    //    the same slot, and the census figure changes.
    {
        let w = workload::build(Dist::Random, 100_000, 64, 0.0);
        let s = MtThread::slot(42);
        s.enter();
        let (t1, c1) = Census::measure(|| {
            let t = Masstree::new(s, Table::Single);
            for (i, k) in w.population.iter().enumerate() {
                t.insert(s, *k, value_of(i));
            }
            t
        });
        drop(t1);
        let (t2, c2) = Census::measure(|| {
            let t = Masstree::new(s, Table::Single);
            for (i, k) in w.population.iter().enumerate() {
                t.insert(s, *k, value_of(i));
            }
            t
        });
        s.exit();
        std::mem::forget(t2);
        checks += 1;
        println!(
            "ok  reused-slot hazard recorded: cold build {} B, rebuild on the same slot after destroy {} B — one process per cell, fresh slot (§3.6)",
            c1.live, c2.live
        );
    }

    println!("\n{checks} deterministic checks passed. The Masstree arm is valid.");
    println!("NOTE: the B/key figures above are a validation sample, not a suite artifact.");
}
