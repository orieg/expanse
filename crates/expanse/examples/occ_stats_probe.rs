//! OCC protocol **event** probe — the load-immune companion to
//! `benches/concurrency.rs`.
//!
//! It replays the concurrency bench's map/set arm (same population, same
//! bounded keyspace, same per-thread 50/50 read-write mix, same hoisted
//! reader handle) and reports *counted events* rather than throughput:
//! optimistic walk attempts per read, reads that exhausted the retry
//! budget and took the writer mutex, and epoch-advance success rate.
//!
//! Why counts: a wall-clock ratio measured on a contended developer
//! machine is not a publishable number (AGENTS.md §8), but "N% of reads
//! ended up taking the writer mutex" is a property of the protocol under
//! the workload, not of the host's spare cores. The ops/s column below is
//! printed only as an internal sanity signal and is explicitly labelled
//! non-publishable.
//!
//! Run:
//! ```text
//! cargo run --release -p expanse-trie --features occ-stats --example occ_stats_probe
//! ```
//! Env knobs: `PROBE_THREADS` (csv), `PROBE_READ_PCT` (csv), `PROBE_MS`.

use expanse_trie::occ_stats::{self, NAMES, NUM_STATS};
use expanse_trie::sync::SyncExpanseMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

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

const POP: u64 = 1_000_000;
const KEYSPACE: u64 = 2 * POP;

fn env_csv<T: std::str::FromStr + Copy>(name: &str, dflt: &[T]) -> Vec<T> {
    match std::env::var(name) {
        Ok(v) if !v.trim().is_empty() => v
            .split(',')
            .map(|s| {
                s.trim()
                    .parse::<T>()
                    .unwrap_or_else(|_| panic!("bad {name}"))
            })
            .collect(),
        _ => dflt.to_vec(),
    }
}

struct Row {
    threads: usize,
    read_pct: u32,
    read_ops: u64,
    write_ops: u64,
    secs: f64,
    stats: [u64; NUM_STATS],
}

fn run(threads: usize, read_pct: u32, window: Duration) -> Row {
    let m = Arc::new(SyncExpanseMap::new());
    let mut rng = XorShift(0x5CA1_AB1E);
    for _ in 0..POP {
        let k = rng.next() % KEYSPACE;
        m.insert(k, !k);
    }

    occ_stats::reset();
    let stop = Arc::new(AtomicBool::new(false));
    let total_read = Arc::new(AtomicU64::new(0));
    let total_write = Arc::new(AtomicU64::new(0));
    let handles: Vec<_> = (0..threads)
        .map(|i| {
            let m = Arc::clone(&m);
            let stop = Arc::clone(&stop);
            let tr = Arc::clone(&total_read);
            let tw = Arc::clone(&total_write);
            std::thread::spawn(move || {
                let rd = m.reader();
                let mut rng = XorShift(0x1000 + i as u64);
                let (mut r_ops, mut w_ops) = (0u64, 0u64);
                let mut sink = 0u64;
                while !stop.load(Ordering::Relaxed) {
                    let r = (rng.next() % 100) as u32;
                    let k = rng.next() % KEYSPACE;
                    if r < read_pct {
                        sink ^= rd.get(k).unwrap_or(0);
                        r_ops += 1;
                    } else {
                        if k & 1 == 0 {
                            m.insert(k, !k);
                        } else {
                            m.remove(k);
                        }
                        w_ops += 1;
                    }
                }
                std::hint::black_box(sink);
                tr.fetch_add(r_ops, Ordering::Relaxed);
                tw.fetch_add(w_ops, Ordering::Relaxed);
            })
        })
        .collect();
    let t0 = std::time::Instant::now();
    std::thread::sleep(window);
    stop.store(true, Ordering::Relaxed);
    for h in handles {
        h.join().expect("thread join");
    }
    let secs = t0.elapsed().as_secs_f64();
    Row {
        threads,
        read_pct,
        read_ops: total_read.load(Ordering::Relaxed),
        write_ops: total_write.load(Ordering::Relaxed),
        secs,
        stats: occ_stats::snapshot(),
    }
}

fn main() {
    assert!(
        occ_stats::enabled(),
        "build with --features occ-stats; without it every counter is compiled out"
    );
    let threads = env_csv("PROBE_THREADS", &[1usize, 2, 4, 8, 16]);
    let read_pcts = env_csv("PROBE_READ_PCT", &[100u32, 50]);
    let ms: Vec<u64> = env_csv("PROBE_MS", &[500u64]);
    let window = Duration::from_millis(ms[0]);

    println!("counters (load-immune): {}", NAMES.join(", "));
    println!(
        "\n{:>3} {:>5} {:>12} {:>12} {:>10} {:>10} {:>12} {:>10} {:>13}",
        "thr",
        "rd%",
        "read_ops",
        "write_ops",
        "att/read",
        "fallback%",
        "spins/read",
        "adv_ok%",
        "rd_ops/s(*)"
    );
    for &rp in &read_pcts {
        for &t in &threads {
            let row = run(t, rp, window);
            let s = row.stats;
            let (rops, atts, fbs, wops, adv_c, adv_ok, spins) =
                (s[0], s[1], s[2], s[3], s[4], s[5], s[6]);
            let per = |n: u64, d: u64| if d == 0 { 0.0 } else { n as f64 / d as f64 };
            println!(
                "{:>3} {:>5} {:>12} {:>12} {:>10.3} {:>9.2}% {:>12.3} {:>9.1}% {:>13.0}",
                row.threads,
                row.read_pct,
                rops,
                wops,
                per(atts, rops),
                100.0 * per(fbs, rops),
                per(spins, rops),
                100.0 * per(adv_ok, adv_c),
                row.read_ops as f64 / row.secs,
            );
            debug_assert_eq!(rops, row.read_ops);
            debug_assert_eq!(wops, row.write_ops);
        }
    }
    println!(
        "\n(*) rd_ops/s is an INTERNAL signal only — measured on a shared, loaded\n\
         developer machine. Not a publishable number (AGENTS.md §8). Read the\n\
         event ratios; they do not depend on how many cores were free."
    );
}
