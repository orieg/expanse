//! Memory-footprint measurement per `docs/BENCHMARKING.md`: bytes/key at
//! population checkpoints, per key-distribution class, from the engine's
//! own byte-exact allocation accounting (`mem_used`). Deterministic — no
//! timing involved, so it is immune to machine load.
//!
//! Run: `cargo run --release -p expanse-trie --example bytes_per_key`

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

fn keys(dist: &str, n: usize) -> Vec<u64> {
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
    let mut out = Vec::with_capacity(n);
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
    out
}

fn main() {
    println!("bytes/key by distribution and population (set flavor / map flavor)");
    println!("target from docs/ARCHITECTURE.md: < 9.5 B/key dense+clustered (set)\n");
    println!(
        "{:<12} {:>10} {:>14} {:>14}",
        "dist", "pop", "set B/key", "map B/key"
    );
    for dist in ["sequential", "random", "clustered", "sparse"] {
        for pop in [1_000usize, 100_000, 1_000_000] {
            let ks = keys(dist, pop);
            let mut set = ExpanseSet::new();
            let mut map = ExpanseMap::new();
            for &k in &ks {
                set.insert(k);
                map.insert(k, !k);
            }
            let (sl, ml) = (set.len().max(1), map.len().max(1));
            println!(
                "{:<12} {:>10} {:>14.2} {:>14.2}",
                dist,
                pop,
                set.mem_used() as f64 / sl as f64,
                map.mem_used() as f64 / ml as f64,
            );
        }
    }
    println!("\n(map B/key includes the 8-byte value per key)");
}
