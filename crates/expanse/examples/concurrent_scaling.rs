//! Concurrent read scaling per `docs/BENCHMARKING.md`: reader throughput
//! of [`SyncExpanseMap`] at 1..N threads, with and without a concurrent
//! writer churning — the measurement that decides whether the per-node
//! version refinement of the Phase 7 OCC protocol (tree-level seqlock
//! today) earns its complexity.
//!
//! Timing-based: follow the BENCHMARKING.md load-hygiene protocol (quiet
//! host, load snapshots) before reading anything into the numbers.
//!
//! Run: `cargo run --release -p expanse-trie --example concurrent_scaling`

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
const WINDOW: Duration = Duration::from_millis(500);

fn populate() -> Arc<SyncExpanseMap> {
    let m = SyncExpanseMap::new();
    let mut rng = XorShift(0x5CA1_AB1E);
    for _ in 0..POP {
        let k = rng.next();
        m.insert(k, !k);
    }
    Arc::new(m)
}

/// Total reader ops across `readers` threads over the window, with an
/// optional writer thread churning inserts/removes.
fn run(m: &Arc<SyncExpanseMap>, readers: usize, with_writer: bool) -> (u64, u64) {
    let stop = Arc::new(AtomicBool::new(false));
    let total = Arc::new(AtomicU64::new(0));

    let writer = with_writer.then(|| {
        let m = Arc::clone(m);
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            let mut rng = XorShift(0xD00D_BEEF);
            let mut ops = 0u64;
            while !stop.load(Ordering::Relaxed) {
                let k = rng.next();
                if k & 1 == 0 {
                    m.insert(k, !k);
                } else {
                    m.remove(k);
                }
                ops += 1;
            }
            ops
        })
    });

    let handles: Vec<_> = (0..readers)
        .map(|i| {
            let m = Arc::clone(m);
            let stop = Arc::clone(&stop);
            let total = Arc::clone(&total);
            std::thread::spawn(move || {
                let rd = m.reader();
                let mut rng = XorShift(0x1000 + i as u64);
                let mut ops = 0u64;
                let mut sink = 0u64;
                while !stop.load(Ordering::Relaxed) {
                    sink ^= rd.get(rng.next()).unwrap_or(0);
                    ops += 1;
                }
                std::hint::black_box(sink);
                total.fetch_add(ops, Ordering::Relaxed);
            })
        })
        .collect();

    std::thread::sleep(WINDOW);
    stop.store(true, Ordering::Relaxed);
    for h in handles {
        h.join().expect("reader");
    }
    let wops = writer.map_or(0, |w| w.join().expect("writer"));
    (total.load(Ordering::Relaxed), wops)
}

fn main() {
    let m = populate();
    let cores = std::thread::available_parallelism().map_or(4, usize::from);
    println!(
        "SyncExpanseMap read scaling — {POP} random keys, {}ms windows, {} cores",
        WINDOW.as_millis(),
        cores
    );
    println!(
        "{:>8} {:>16} {:>10} {:>16} {:>12}",
        "readers", "reads/s (idle)", "scale", "reads/s (churn)", "writer op/s"
    );
    let mut base = 0f64;
    for readers in [1usize, 2, 4].into_iter().chain((cores >= 8).then_some(8)) {
        let (idle, _) = run(&m, readers, false);
        let (churn, wops) = run(&m, readers, true);
        let idle_s = idle as f64 / WINDOW.as_secs_f64();
        if readers == 1 {
            base = idle_s;
        }
        println!(
            "{:>8} {:>16.0} {:>9.2}x {:>16.0} {:>12.0}",
            readers,
            idle_s,
            idle_s / base,
            churn as f64 / WINDOW.as_secs_f64(),
            wops as f64 / WINDOW.as_secs_f64(),
        );
    }
    println!("\n(retry pressure: a churn column far below idle at equal reader");
    println!(" counts = tree-level seqlock retries; the per-node refinement's");
    println!(" go/no-go signal per ARCHITECTURE.md §6)");
}
