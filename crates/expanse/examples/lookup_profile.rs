//! Lookup-path attribution harness (`docs/BENCHMARKING.md`).
//!
//! A tight `get`-only loop over a prebuilt tree, for use under a sampling
//! profiler (`samply record ./target/release/examples/lookup_profile`, or
//! macOS `sample`). The point is **where** the lookup spends its time, not
//! how long it takes: the distribution of samples *within one process* is
//! far less sensitive to co-resident load than a cross-binary wall-clock
//! ratio, so attribution stays informative on a shared machine where the
//! vs-libjudy comparison would not be.
//!
//! Prints a checksum so the loop cannot be optimized away, and a rough
//! ns/op that is explicitly NOT a benchmark number — it exists to confirm
//! the loop ran, and to spot order-of-magnitude changes while iterating.
//!
//! Run: `cargo run --release -p expanse-trie --example lookup_profile [dist] [seconds]`

use expanse_trie::map::ExpanseMap;
use std::time::{Duration, Instant};

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

const POP: usize = 1_000_000;

fn keys(dist: &str) -> Vec<u64> {
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
    let mut out = Vec::with_capacity(POP);
    match dist {
        "sequential" => out.extend(0..POP as u64),
        "random" => out.extend((0..POP).map(|_| rng.next())),
        "clustered" => {
            let mut base = 0;
            for i in 0..POP as u64 {
                if i % 256 == 0 {
                    base = rng.next() & !0xFF;
                }
                out.push(base + (i % 256));
            }
        }
        other => panic!("unknown distribution {other}"),
    }
    out
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dist = args.next().unwrap_or_else(|| "random".into());
    let secs: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(10);

    let ks = keys(&dist);
    let mut map = ExpanseMap::new();
    for &k in &ks {
        map.insert(k, !k);
    }
    // Probe order is deliberately shuffled relative to build order: a
    // sequential probe walk would prefetch the whole tree and profile
    // the memory system rather than the lookup path.
    let mut probes: Vec<u64> = ks.clone();
    let mut rng = XorShift(0x9E37_79B9);
    for i in (1..probes.len()).rev() {
        probes.swap(i, (rng.next() % (i as u64 + 1)) as usize);
    }

    eprintln!(
        "profiling `get` on {dist} ({POP} keys, {} B/key) for {secs}s",
        map.mem_used() as f64 / POP as f64
    );

    let deadline = Duration::from_secs(secs);
    let start = Instant::now();
    let mut sink = 0u64;
    let mut ops = 0u64;
    while start.elapsed() < deadline {
        // Batch between clock reads: the Instant::now() call is itself a
        // syscall-ish cost and would otherwise dominate the profile.
        for chunk in probes.chunks(4096) {
            for &k in chunk {
                sink ^= map.get(k).unwrap_or(0);
                ops += 1;
            }
            if start.elapsed() >= deadline {
                break;
            }
        }
    }
    let elapsed = start.elapsed();
    println!("checksum {sink:#x}");
    println!(
        "{ops} lookups in {:.2}s (~{:.1} ns/op — indicative only, NOT a benchmark)",
        elapsed.as_secs_f64(),
        elapsed.as_nanos() as f64 / ops as f64
    );
}
