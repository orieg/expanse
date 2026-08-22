//! Concurrency scalability benchmark for SyncExpanseSet and SyncExpanseMap

use expanse_trie::sync::{SyncExpanseMap, SyncExpanseSet};
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

fn bench_map(ratio_read: u32, _ratio_write: u32, readers: usize) -> (f64, f64) {
    let m = Arc::new(SyncExpanseMap::new());
    let mut rng = XorShift(0x5CA1_AB1E);
    for _ in 0..POP {
        let k = rng.next();
        m.insert(k, !k);
    }

    let stop = Arc::new(AtomicBool::new(false));
    let total_read_ops = Arc::new(AtomicU64::new(0));
    let total_write_ops = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..readers)
        .map(|i| {
            let m = Arc::clone(&m);
            let stop = Arc::clone(&stop);
            let total_read = Arc::clone(&total_read_ops);
            let total_write = Arc::clone(&total_write_ops);

            std::thread::spawn(move || {
                let rd = m.reader();
                let mut rng = XorShift(0x1000 + i as u64);
                let mut read_ops = 0u64;
                let mut write_ops = 0u64;
                let mut sink = 0u64;

                while !stop.load(Ordering::Relaxed) {
                    let r = (rng.next() % 100) as u32;
                    let k = rng.next();
                    if r < ratio_read {
                        sink ^= rd.get(k).unwrap_or(0);
                        read_ops += 1;
                    } else {
                        if k & 1 == 0 {
                            m.insert(k, !k);
                        } else {
                            m.remove(k);
                        }
                        write_ops += 1;
                    }
                }
                std::hint::black_box(sink);
                total_read.fetch_add(read_ops, Ordering::Relaxed);
                total_write.fetch_add(write_ops, Ordering::Relaxed);
            })
        })
        .collect();

    std::thread::sleep(WINDOW);
    stop.store(true, Ordering::Relaxed);
    for h in handles {
        h.join().expect("thread join");
    }

    let r_ops = total_read_ops.load(Ordering::Relaxed) as f64 / WINDOW.as_secs_f64();
    let w_ops = total_write_ops.load(Ordering::Relaxed) as f64 / WINDOW.as_secs_f64();
    (r_ops, w_ops)
}

fn bench_set(ratio_read: u32, _ratio_write: u32, readers: usize) -> (f64, f64) {
    let s = Arc::new(SyncExpanseSet::new());
    let mut rng = XorShift(0x5CA1_AB1E);
    for _ in 0..POP {
        s.insert(rng.next());
    }

    let stop = Arc::new(AtomicBool::new(false));
    let total_read_ops = Arc::new(AtomicU64::new(0));
    let total_write_ops = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..readers)
        .map(|i| {
            let s = Arc::clone(&s);
            let stop = Arc::clone(&stop);
            let total_read = Arc::clone(&total_read_ops);
            let total_write = Arc::clone(&total_write_ops);

            std::thread::spawn(move || {
                let rd = s.reader();
                let mut rng = XorShift(0x1000 + i as u64);
                let mut read_ops = 0u64;
                let mut write_ops = 0u64;
                let mut sink = false;

                while !stop.load(Ordering::Relaxed) {
                    let r = (rng.next() % 100) as u32;
                    let k = rng.next();
                    if r < ratio_read {
                        sink ^= rd.contains(k);
                        read_ops += 1;
                    } else {
                        if k & 1 == 0 {
                            s.insert(k);
                        } else {
                            s.remove(k);
                        }
                        write_ops += 1;
                    }
                }
                std::hint::black_box(sink);
                total_read.fetch_add(read_ops, Ordering::Relaxed);
                total_write.fetch_add(write_ops, Ordering::Relaxed);
            })
        })
        .collect();

    std::thread::sleep(WINDOW);
    stop.store(true, Ordering::Relaxed);
    for h in handles {
        h.join().expect("thread join");
    }

    let r_ops = total_read_ops.load(Ordering::Relaxed) as f64 / WINDOW.as_secs_f64();
    let w_ops = total_write_ops.load(Ordering::Relaxed) as f64 / WINDOW.as_secs_f64();
    (r_ops, w_ops)
}

fn main() {
    let max_threads = std::thread::available_parallelism()
        .map_or(16, usize::from)
        .min(16);
    let threads_list = [1, 2, 4, 8, 16];

    let workloads = [
        (100, 0, "100% Read / 0% Write"),
        (95, 5, "95% Read / 5% Write"),
        (50, 50, "50% Read / 50% Write"),
    ];

    for (name, is_map) in [("SyncExpanseMap", true), ("SyncExpanseSet", false)] {
        for (rr, rw, wname) in workloads {
            println!("\n=== {} ({}) ===", name, wname);
            println!(
                "{:>8} {:>16} {:>16} {:>10}",
                "threads", "read ops/sec", "write ops/sec", "scale"
            );

            let mut base = 0.0;
            for &t in &threads_list {
                if t > max_threads && t != 1 {
                    continue;
                }
                let (rops, wops) = if is_map {
                    bench_map(rr, rw, t)
                } else {
                    bench_set(rr, rw, t)
                };

                let total = rops + wops;
                if t == 1 {
                    base = total;
                }

                println!(
                    "{:>8} {:>16.0} {:>16.0} {:>9.2}x",
                    t,
                    rops,
                    wops,
                    total / base
                );
            }
        }
    }
}
