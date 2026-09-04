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

use expanse_hot_bench::{
    Census, HotMap, HotSet, InlineInsert, hot_can_inline, pool_allocations, validate_census,
};
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

/// Full 64-bit key stream. The suite's domain is `u64`; no key is folded to fit
/// a competitor's payload encoding.
fn keys(n: usize, seed: u64, sequential: bool) -> Vec<u64> {
    if sequential {
        return (0..n as u64).collect();
    }
    let mut rng = XorShift64::new(seed);
    let mut v: Vec<u64> = (0..n).map(|_| rng.next()).collect();
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

    // 1. Per-arm capability, measured every run rather than assumed.
    //
    //    This probe is the suite's own falsifier and must stay reachable. An
    //    earlier revision folded every key into a 63-bit space, which made the
    //    most transferable finding about HOT -- that `insert` reports success on
    //    keys `lookup` will never find -- unobservable by construction.
    let spanning: [u64; 6] = [
        1,
        42,
        (1u64 << 62) + 7,
        1u64 << 63,
        (1u64 << 63) + 99,
        u64::MAX,
    ];
    let representable = spanning.iter().filter(|k| hot_can_inline(**k)).count();

    let mut hs = HotSet::new();
    let mut refused = 0usize;
    for k in &spanning {
        if hs.insert(*k) == InlineInsert::NotRepresentable {
            refused += 1;
        }
    }
    let set_found = spanning.iter().filter(|k| hs.contains(**k)).count();
    if set_found != representable || refused != spanning.len() - representable {
        fail(&format!(
            "Arm A capability drifted: {set_found} found and {refused} refused of {}, expected {representable} found and {} refused",
            spanning.len(),
            spanning.len() - representable
        ));
    }

    let mut hm = HotMap::new();
    for (i, k) in spanning.iter().enumerate() {
        hm.insert(*k, 7000 + i as u64);
    }
    let map_ok = spanning
        .iter()
        .enumerate()
        .filter(|(i, k)| hm.get(**k) == Some(7000 + *i as u64))
        .count();
    if map_ok != spanning.len() || hm.len() != spanning.len() {
        fail(&format!(
            "Arm B lost keys it should hold: {map_ok}/{} round-trip, walk sees {}",
            spanning.len(),
            hm.len()
        ));
    }
    checks += 2;
    println!(
        "ok  arm capability on keys spanning bit 63: Arm A holds {}/{} and refuses {} at the call; \
         Arm B holds {}/{} over the full 64-bit domain",
        set_found,
        spanning.len(),
        refused,
        map_ok,
        spanning.len()
    );

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

        // Arm A can only hold the representable part of the stream, and says
        // so; that is the arm's declared scope, not a silent truncation.
        let inlinable: Vec<u64> = ks.iter().copied().filter(|k| hot_can_inline(*k)).collect();
        let want_set = inlinable.len();
        let mut hs = HotSet::new();
        for k in &inlinable {
            hs.insert(*k);
        }
        if hs.len() != want_set {
            fail(&format!(
                "HotSet/{label}: built {want_set} representable keys of {want}, trie walks {} — insert() reported success on keys it cannot find",
                hs.len()
            ));
        }
        let misses = inlinable.iter().filter(|k| !hs.contains(**k)).count();
        if misses != 0 {
            fail(&format!(
                "HotSet/{label}: {misses} of {want_set} inserted keys not found"
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
            "ok  population fidelity {label}: HotSet {want_set}/{want_set} representable of {want}, HotMap {want}/{want} over the full 64-bit domain"
        );
    }

    // 4. The Expanse arms must agree on the same stream, so a later divergence
    //    is attributable to the engine rather than to the generator.
    let ks = keys(200_000, 0x1234_5678_9ABC, false);
    let mut es = ExpanseSet::new();
    let mut em = ExpanseMap::new();
    for (i, k) in ks.iter().enumerate() {
        es.insert(*k);
        em.insert(*k, i as u64);
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

    let inlinable: Vec<u64> = ks.iter().copied().filter(|k| hot_can_inline(*k)).collect();
    let n = inlinable.len();
    let (hot_len, hot_census) = Census::measure(|| {
        let mut t = HotSet::new();
        for k in &inlinable {
            t.insert(*k);
        }
        let len = t.len();
        std::mem::forget(t);
        len
    });
    let (exp_len, exp_census) = Census::measure(|| {
        let mut t = ExpanseSet::new();
        for k in &inlinable {
            t.insert(*k);
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
