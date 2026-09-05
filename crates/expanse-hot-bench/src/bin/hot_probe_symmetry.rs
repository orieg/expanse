//! Probe symmetry check for the latency pillars — run before they are written.
//!
//! The question is not "do both arms return the right answer" but **"do both
//! arms do the same work per probe, and is that work consumed"**. This repo has
//! already published figures from a harness whose read path never dereferenced
//! the payload, billing index traversal only (`docs/DATABASE.md` §7 records the
//! retraction), and every census defect in this integration so far was found by
//! running something rather than by reading it.
//!
//! Three properties are checked, all deterministic (§8.4 — no wall-clock here):
//!
//! 1. **Answer agreement.** For an identical probe stream, HOT and Expanse must
//!    return the same result on every probe. A disagreement voids the arm and
//!    would otherwise show up as a latency difference.
//! 2. **Value consumption.** On the map arm both sides must produce the stored
//!    value, not merely a presence bit, and both sinks must be equal. HOT
//!    reaches its value through a heap pointer and Expanse reads it from an
//!    inline `ValueSlot`; that asymmetry in *cost* is the architectural
//!    difference the arm exists to measure, and it is only fair if both arms
//!    are actually made to fetch it.
//! 3. **Miss shape.** Misses are rejection-sampled from the same generator as
//!    the population, never a transform of a present key (§8.6) — a transformed
//!    miss lands in a different expanse and terminates at a different depth,
//!    averaging two descents into one number.
//! 4. **Probe order.** The stream is shuffled. `pop` is sorted, so hits drawn by
//!    index would arrive in ascending key order and hand an ordered trie cache
//!    and prefetch behaviour no real workload provides. `art_comparison/` had to
//!    amend for this after its first reference-host run.

use expanse_hot_bench::{HotMap, HotSet, hot_can_inline};
use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;

struct XorShift(u64);
impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

fn fail(msg: &str) -> ! {
    eprintln!("FAIL: {msg}");
    std::process::exit(1);
}

/// Population plus a 50/50 hit/miss probe stream whose misses are drawn from the
/// same generator and rejected on membership (§8.6).
fn workload(n: usize, width: u32) -> (Vec<u64>, Vec<u64>) {
    let mask = if width >= 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    };
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
    let mut pop: Vec<u64> = (0..n).map(|_| rng.next() & mask).collect();
    pop.sort_unstable();
    pop.dedup();

    let mut probes = Vec::with_capacity(pop.len());
    let mut i = 0usize;
    while probes.len() < pop.len() {
        if probes.len() % 2 == 0 {
            probes.push(pop[i % pop.len()]);
            i += 1;
        } else {
            // Same generator, rejected on membership — not a transform.
            loop {
                let c = rng.next() & mask;
                if pop.binary_search(&c).is_err() {
                    probes.push(c);
                    break;
                }
            }
        }
    }

    // Shuffle. `pop` is sorted, so hits drawn by index arrive in ascending key
    // order, and an ordered trie walking its keyspace monotonically gets cache
    // and prefetch behaviour no real point-lookup workload would give it. The
    // ART suite had to amend for exactly this after its first reference-host
    // run; inheriting the amendment is cheaper than rediscovering it.
    // Fisher-Yates from the same PRNG, so the stream stays reproducible.
    for j in (1..probes.len()).rev() {
        let k = (rng.next() % (j as u64 + 1)) as usize;
        probes.swap(j, k);
    }
    (pop, probes)
}

fn main() {
    // --- Arm A: membership only on both sides, 63-bit domain -----------------
    let (pop, probes) = workload(200_000, 63);
    if !pop.iter().all(|k| hot_can_inline(*k)) {
        fail("Arm A population escaped the 63-bit domain");
    }

    let mut hs = HotSet::new();
    let mut es = ExpanseSet::new();
    for k in &pop {
        hs.insert(*k);
        es.insert(*k);
    }

    let mut disagree = 0usize;
    let mut hits = 0usize;
    let mut sink_h = 0u64;
    let mut sink_e = 0u64;
    for p in &probes {
        let h = hs.contains(*p);
        let e = es.contains(*p);
        if h != e {
            disagree += 1;
        }
        if h {
            hits += 1;
            sink_h ^= *p;
        }
        if e {
            sink_e ^= *p;
        }
    }
    if disagree != 0 {
        fail(&format!(
            "Arm A: {disagree} of {} probes disagreed",
            probes.len()
        ));
    }
    if sink_h != sink_e {
        fail("Arm A: sinks diverged despite matching answers");
    }
    let rate = 100.0 * hits as f64 / probes.len() as f64;
    if !(45.0..=55.0).contains(&rate) {
        fail(&format!(
            "Arm A: hit rate {rate:.1}% is not the declared ~50%"
        ));
    }
    println!(
        "ok  Arm A (set, 63-bit): {} probes, {hits} hits ({rate:.1}%), 0 disagreements, sinks equal",
        probes.len()
    );

    // --- Arm B: both sides must produce the VALUE, not a presence bit --------
    let (pop, probes) = workload(200_000, 64);
    let mut hm = HotMap::new();
    let mut em = ExpanseMap::new();
    for (i, k) in pop.iter().enumerate() {
        let v = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        hm.insert(*k, v);
        em.insert(*k, v);
    }

    let mut disagree = 0usize;
    let mut value_mismatch = 0usize;
    let mut hits = 0usize;
    let mut sink_h = 0u64;
    let mut sink_e = 0u64;
    for p in &probes {
        let h = hm.get(*p);
        let e = em.get(*p);
        if h.is_some() != e.is_some() {
            disagree += 1;
            continue;
        }
        if let (Some(hv), Some(ev)) = (h, e) {
            hits += 1;
            if hv != ev {
                value_mismatch += 1;
            }
            // Both sinks are fed from the fetched value, so neither arm's value
            // read can be eliminated.
            sink_h ^= hv;
            sink_e ^= ev;
        }
    }
    if disagree != 0 {
        fail(&format!(
            "Arm B: {disagree} of {} probes disagreed on presence",
            probes.len()
        ));
    }
    if value_mismatch != 0 {
        fail(&format!(
            "Arm B: {value_mismatch} probes returned different values"
        ));
    }
    if sink_h != sink_e || sink_h == 0 {
        fail(
            "Arm B: value sinks diverged or were never fed — the value fetch is not being consumed",
        );
    }
    let rate = 100.0 * hits as f64 / probes.len() as f64;
    if !(45.0..=55.0).contains(&rate) {
        fail(&format!(
            "Arm B: hit rate {rate:.1}% is not the declared ~50%"
        ));
    }
    println!(
        "ok  Arm B (map, 64-bit): {} probes, {hits} hits ({rate:.1}%), 0 disagreements, values identical, sinks fed",
        probes.len()
    );

    println!(
        "\nBoth arms agree probe-for-probe and consume the fetched value.\n\
         Arm B's asymmetry is architectural and intended: HOT reaches its value through a heap\n\
         pointer, Expanse reads it from an inline ValueSlot. Both are made to fetch it, so the\n\
         latency pillars will bill that difference rather than hide it."
    );
}
