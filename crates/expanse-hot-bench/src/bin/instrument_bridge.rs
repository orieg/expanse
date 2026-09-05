//! Reconciles the two memory instruments this suite has to live between.
//!
//! The repo's committed `bytes/key` table (`crates/expanse/examples/bytes_per_key.rs`,
//! workload `example_bytes_per_key`) reports **`mem_used()`** — the engine's own
//! byte-exact node accounting. The HOT suite cannot use it: HOT has no
//! equivalent, so §3.2 of the methodology locked **bytes held from the C
//! allocator**, which is the only definition both arms can satisfy.
//!
//! Those two numbers are not the same quantity, and the difference is not noise:
//! the allocator adds a chunk header and rounds to a size class. Publishing an
//! Expanse cell here that disagrees with the repo's own table, with no stated
//! bridge, would look like a contradiction between two committed artifacts.
//! This probe measures both on the same structure so the bridge is a measured
//! ratio rather than an assertion.
//!
//! It also separates the *other* reason the suite's cells differ from the
//! table's: the table's `random` row draws full 64-bit keys, and this suite is
//! locked to a 63-bit keyspace (HOT tags leaves in bit 0). Both are reported.
//!
//! Deterministic accounting only — no wall-clock, so no CI or statistics
//! apply (§8.4).

use expanse_hot_bench::Census;
use expanse_trie::ExpanseSet;
use expanse_trie::map::ExpanseMap;

/// The generator `bytes_per_key.rs` uses, reproduced exactly — same algorithm,
/// same seed — so a cell here is comparable to that table's cell rather than
/// merely similar.
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

fn keys(dist: &str, n: usize, truncate63: bool) -> Vec<u64> {
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
    let mut out: Vec<u64> = Vec::with_capacity(n);
    match dist {
        "sequential" => out.extend(0..n as u64),
        "random" => out.extend((0..n).map(|_| rng.next())),
        "clustered" => {
            let mut base = 0;
            for i in 0..n as u64 {
                if i % 256 == 0 {
                    base = rng.next() & !0xFF;
                }
                out.push(base + (i % 256));
            }
        }
        "sparse" => out.extend((0..n as u64).map(|i| i << 40)),
        _ => unreachable!(),
    }
    if truncate63 {
        // Reported as a contrast, never as the suite's domain: clearing the top
        // bit halves the keyspace, which for a structure partitioning by key
        // expanse is arithmetically the same as doubling the population.
        for k in &mut out {
            *k &= (1u64 << 63) - 1;
        }
    }
    out
}

fn main() {
    println!(
        "Instrument bridge: engine `mem_used()` vs bytes held from the C allocator.\n\
         Same structure, same key stream, both instruments read on the same build.\n"
    );
    println!(
        "{:<12} {:>9} {:>4} {:>12} {:>12} {:>8}  flavor",
        "dist", "N", "bits", "mem_used B/k", "alloc B/k", "ratio"
    );

    for dist in ["sequential", "clustered", "sparse", "random"] {
        for n in [10_000usize, 100_000, 1_000_000] {
            // 64-bit is what the committed table measured; 63-bit is what this
            // suite is locked to. Reported side by side so neither is mistaken
            // for the other.
            for truncate in [false, true] {
                // `random` is the only distribution the truncation can move;
                // the others never set bit 63 at these populations.
                if truncate && dist != "random" {
                    continue;
                }
                let ks = keys(dist, n, truncate);

                let (set_used, set_census) = Census::measure(|| {
                    let mut s = ExpanseSet::new();
                    for k in &ks {
                        s.insert(*k);
                    }
                    let used = s.mem_used();
                    std::mem::forget(s);
                    used
                });
                let (map_used, map_census) = Census::measure(|| {
                    let mut m = ExpanseMap::new();
                    for (i, k) in ks.iter().enumerate() {
                        m.insert(*k, i as u64);
                    }
                    let used = m.mem_used();
                    std::mem::forget(m);
                    used
                });

                let n_f = ks.len() as f64;
                for (flavor, used, census) in
                    [("set", set_used, set_census), ("map", map_used, map_census)]
                {
                    let used_bk = used as f64 / n_f;
                    let alloc_bk = census.live as f64 / n_f;
                    println!(
                        "{:<12} {:>9} {:>4} {:>12.2} {:>12.2} {:>8.3}  {}",
                        dist,
                        ks.len(),
                        if truncate { 63 } else { 64 },
                        used_bk,
                        alloc_bk,
                        alloc_bk / used_bk,
                        flavor
                    );
                }
            }
        }
    }

    println!(
        "\nratio = allocator-held / mem_used. Above 1.000 is allocator overhead the\n\
         engine's own accounting does not count (chunk headers, size-class rounding).\n\
         This suite publishes the allocator column, because it is the only one HOT can\n\
         also be measured under; the repo's bytes/key table publishes the other."
    );
}
