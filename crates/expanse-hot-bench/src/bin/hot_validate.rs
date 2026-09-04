//! Validation gate for the HOT FFI foundation — the check that must pass before
//! any figure from this suite is recorded.
//!
//! Every assertion here is on a deterministic invariant (population counts,
//! round-trip error counts, exact byte accounting), never on wall-clock, per
//! AGENTS.md §8.4. It exists because the #660 Step 0 gate found two silent
//! failures that a timing harness would have reported as success:
//!
//! - `insert()` returning `true` for keys the trie then cannot find, so a build
//!   over uniform random 64-bit keys quietly held half its population.
//! - a census whose free path never subtracted, so bytes held only ever rose.
//!
//! Both are checked directly below.

use expanse_hot_bench::{Census, HotMap, HotSet, Key63, pool_allocations, validate_census};
use expanse_trie::{ExpanseMap, ExpanseSet};

/// XorShift64, matching the generator the rest of the repo's comparative suites
/// use, so key streams are reproducible across arms and runs (§8.3).
struct XorShift64(u64);

impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

fn keys(n: usize, seed: u64, sequential: bool) -> Vec<Key63> {
    if sequential {
        return (0..n as u64).map(Key63::truncate).collect();
    }
    let mut rng = XorShift64::new(seed);
    let mut v: Vec<Key63> = (0..n).map(|_| Key63::truncate(rng.next())).collect();
    v.sort_unstable();
    v.dedup();
    v
}

fn fail(msg: &str) -> ! {
    eprintln!("FAIL: {msg}");
    std::process::exit(1);
}

fn main() {
    let mut checks = 0usize;
    // HOT's node pool is a function-local `static` inside
    // `HOTSingleThreadedNodeBase::getMemoryPool()`, so it is process-global and
    // outlives every trie instance. `EXP_CENSUS_FIRST=1` runs the census before
    // anything else has built a HOT trie; the difference between the two orders
    // is the hazard `census_isolation` documents.
    let census_first = std::env::var("EXP_CENSUS_FIRST").is_ok();
    if census_first {
        census_pass();
        println!("(census ran first; remaining checks follow)");
    }

    // 1. The keyspace guard is real, not documentation.
    if Key63::new(1u64 << 63).is_some() {
        fail("Key63 accepted a key with bit 63 set");
    }
    if Key63::new((1u64 << 63) - 1).is_none() {
        fail("Key63 rejected the largest representable key");
    }
    if Key63::truncate(u64::MAX).get() != (1u64 << 63) - 1 {
        fail("Key63::truncate did not fold onto the 63-bit space");
    }
    checks += 3;
    println!("ok  keyspace guard rejects bit 63, truncate folds onto 2^63");

    // 2. The census instrument itself. This is the check the Step 0 gate program
    //    failed: it saw the free happen and did not subtract the bytes.
    let control = validate_census(1 << 20);
    if !control.is_valid() {
        fail(&format!(
            "census control invalid: requested {} B, counter rose {} B, residual after free {} B \
             (residual must be 0 — this is the Step 0 defect)",
            control.requested, control.alloc_delta, control.residual
        ));
    }
    checks += 1;
    println!(
        "ok  census control: +{} B on a {} B request, residual {} B after free",
        control.alloc_delta, control.requested, control.residual
    );

    // 3. Population fidelity on both arms, over both a structured and an
    //    unstructured key stream. A build that loses keys silently is the exact
    //    failure this suite was nearly published with.
    for (label, sequential) in [("sequential", true), ("random", false)] {
        let ks = keys(200_000, 0x1234_5678_9ABC, sequential);
        let want = ks.len();

        let mut hs = HotSet::new();
        for k in &ks {
            hs.insert(*k);
        }
        if hs.len() != want {
            fail(&format!(
                "HotSet/{label}: built {want} keys, trie walks {} — insert() reported success on keys it cannot find",
                hs.len()
            ));
        }
        let misses = ks.iter().filter(|k| !hs.contains(**k)).count();
        if misses != 0 {
            fail(&format!(
                "HotSet/{label}: {misses} of {want} inserted keys not found"
            ));
        }

        let mut hm = HotMap::new();
        for (i, k) in ks.iter().enumerate() {
            hm.insert(*k, i as u64);
        }
        if hm.len() != want {
            fail(&format!(
                "HotMap/{label}: built {want} keys, trie walks {}",
                hm.len()
            ));
        }
        let bad = ks
            .iter()
            .enumerate()
            .filter(|(i, k)| hm.get(**k) != Some(*i as u64))
            .count();
        if bad != 0 {
            fail(&format!(
                "HotMap/{label}: {bad} of {want} entries did not round-trip"
            ));
        }

        checks += 4;
        println!(
            "ok  population fidelity {label}: HotSet {want}/{want}, HotMap {want}/{want} round-trip"
        );
    }

    // 4. The Expanse arms must agree on the same stream, so a later divergence
    //    is attributable to the engine rather than to the generator.
    let ks = keys(200_000, 0x1234_5678_9ABC, false);
    let mut es = ExpanseSet::new();
    let mut em = ExpanseMap::new();
    for (i, k) in ks.iter().enumerate() {
        es.insert(k.get());
        em.insert(k.get(), i as u64);
    }
    if es.len() as usize != ks.len() || em.len() as usize != ks.len() {
        fail(&format!(
            "Expanse arms disagree with the stream: set {} map {} vs {} keys",
            es.len(),
            em.len(),
            ks.len()
        ));
    }
    checks += 1;
    println!(
        "ok  Expanse arms hold the same {} keys from the shared stream",
        ks.len()
    );

    // 5. The census sees BOTH arms.
    if !census_first {
        census_pass();
    }
    checks += 2;

    println!("\n{checks} deterministic checks passed. Foundation is valid.");
    println!(
        "NOTE: the B/key figures above are a validation sample, not a suite artifact. \
         Published cells come from the registered harnesses on the reference host."
    );
}

/// One census pass over both arms under the shared instrument.
fn census_pass() {
    let pool_before = pool_allocations();
    let ks = keys(100_000, 0xABCD_EF01_2345, false);
    let n = ks.len();

    let (hot_len, hot_census) = Census::measure(|| {
        let mut t = HotSet::new();
        for k in &ks {
            t.insert(*k);
        }
        let len = t.len();
        std::mem::forget(t);
        len
    });
    let (exp_len, exp_census) = Census::measure(|| {
        let mut t = ExpanseSet::new();
        for k in &ks {
            t.insert(k.get());
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
    if hot_census.live <= 0 || exp_census.live <= 0 {
        fail(&format!(
            "census saw no allocations for an arm: HOT {} B, Expanse {} B — the interposition is not reaching one of them",
            hot_census.live, exp_census.live
        ));
    }
    println!(
        "ok  one instrument sees both arms at N={n}: HOT {:.2} B/key ({} allocs), ExpanseSet {:.2} B/key ({} allocs)",
        hot_census.live as f64 / n as f64,
        hot_census.allocs,
        exp_census.live as f64 / n as f64,
        exp_census.allocs,
    );
    if pool_before != 0 {
        println!(
            "WARN census ran on a warm HOT pool ({pool_before} prior allocations): the HOT figure \
             above undercounts and is not publishable. Re-run with EXP_CENSUS_FIRST=1."
        );
    } else {
        println!("ok  HOT pool was cold at census entry (0 prior allocations)");
    }
}
