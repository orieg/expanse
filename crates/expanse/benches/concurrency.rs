//! Concurrency scalability benchmark for SyncExpanseSet, SyncExpanseMap,
//! SyncExpanseBlobMap and SyncExpanseStrMap (issue #219: the blob arm
//! compares the OCC wrapper against a `Mutex<ExpanseBlobMap>` baseline, an
//! `RwLock<BTreeMap>` and `crossbeam_skiplist`; the string arm against
//! `Mutex<ExpanseStrMap>` and `DashMap` — `ExpanseStrMap`, like the other
//! single-threaded structures, is deliberately `!Sync`, so an
//! `RwLock<ExpanseStrMap>` cannot legally be shared).

use crossbeam_skiplist::SkipMap;
use dashmap::DashMap;
use expanse_trie::ExpanseBlobMap;
use expanse_trie::strmap::ExpanseStrMap;
use expanse_trie::sync::{SyncExpanseBlobMap, SyncExpanseMap, SyncExpanseSet, SyncExpanseStrMap};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
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
    run_window(readers, move |i, stop| {
        let rd = m.reader();
        let mut rng = XorShift(0x1000 + i as u64);
        let (mut read_ops, mut write_ops) = (0u64, 0u64);
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
        (read_ops, write_ops)
    })
}

fn bench_set(ratio_read: u32, _ratio_write: u32, readers: usize) -> (f64, f64) {
    let s = Arc::new(SyncExpanseSet::new());
    let mut rng = XorShift(0x5CA1_AB1E);
    for _ in 0..POP {
        s.insert(rng.next());
    }
    run_window(readers, move |i, stop| {
        let rd = s.reader();
        let mut rng = XorShift(0x1000 + i as u64);
        let (mut read_ops, mut write_ops) = (0u64, 0u64);
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
        (read_ops, write_ops)
    })
}

/// Blob arm: bounded keyspace so lookups actually hit (~50%) and dereference
/// payload bytes; 128-byte payloads land squarely in the arena regime.
const BLOB_POP: u64 = 200_000;
const BLOB_KEYSPACE: u64 = 2 * BLOB_POP;
const BLOB_LEN: usize = 128;

fn blob_payload(k: u64, buf: &mut [u8; BLOB_LEN]) {
    let mut x = k | 1;
    for chunk in buf.chunks_mut(8) {
        x = x.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        chunk.copy_from_slice(&x.to_le_bytes()[..chunk.len()]);
    }
}

/// Runs `threads` copies of `work(thread_idx, stop) -> (read_ops, write_ops)`
/// for one measurement window; returns (read ops/sec, write ops/sec).
fn run_window<F>(threads: usize, work: F) -> (f64, f64)
where
    F: Fn(usize, &AtomicBool) -> (u64, u64) + Send + Sync + 'static,
{
    let stop = Arc::new(AtomicBool::new(false));
    let work = Arc::new(work);
    let total_read = Arc::new(AtomicU64::new(0));
    let total_write = Arc::new(AtomicU64::new(0));
    let handles: Vec<_> = (0..threads)
        .map(|i| {
            let stop = Arc::clone(&stop);
            let work = Arc::clone(&work);
            let total_read = Arc::clone(&total_read);
            let total_write = Arc::clone(&total_write);
            std::thread::spawn(move || {
                let (r, w) = work(i, &stop);
                total_read.fetch_add(r, Ordering::Relaxed);
                total_write.fetch_add(w, Ordering::Relaxed);
            })
        })
        .collect();
    std::thread::sleep(WINDOW);
    stop.store(true, Ordering::Relaxed);
    for h in handles {
        h.join().expect("thread join");
    }
    let r_ops = total_read.load(Ordering::Relaxed) as f64 / WINDOW.as_secs_f64();
    let w_ops = total_write.load(Ordering::Relaxed) as f64 / WINDOW.as_secs_f64();
    (r_ops, w_ops)
}

fn bench_blob_sync(ratio_read: u32, readers: usize) -> (f64, f64) {
    let m = Arc::new(SyncExpanseBlobMap::new());
    let mut rng = XorShift(0x5CA1_AB1E);
    let mut buf = [0u8; BLOB_LEN];
    for _ in 0..BLOB_POP {
        let k = rng.next() % BLOB_KEYSPACE;
        blob_payload(k, &mut buf);
        m.insert(k, &buf, k as u32 & 0xFF_FFFF).expect("prefill");
    }
    run_window(readers, move |i, stop| {
        let mut rd = m.reader();
        let mut rng = XorShift(0x1000 + i as u64);
        let mut buf = [0u8; BLOB_LEN];
        let (mut read_ops, mut write_ops) = (0u64, 0u64);
        let mut sink = 0u64;
        while !stop.load(Ordering::Relaxed) {
            let r = (rng.next() % 100) as u32;
            let k = rng.next() % BLOB_KEYSPACE;
            if r < ratio_read {
                let guard = rd.pin();
                if let Some((view, meta)) = guard.get(k) {
                    sink ^= u64::from(view.as_bytes()[0]) ^ u64::from(meta);
                }
                read_ops += 1;
            } else {
                if k & 1 == 0 {
                    blob_payload(k, &mut buf);
                    let _ = m.insert(k, &buf, k as u32 & 0xFF_FFFF);
                } else {
                    m.remove(k);
                }
                write_ops += 1;
            }
        }
        std::hint::black_box(sink);
        (read_ops, write_ops)
    })
}

/// Locked baseline. `ExpanseBlobMap` is deliberately `!Sync` (its map's
/// insert-path cache is mutated through `&self` on some read APIs; shared
/// access is the `SyncExpanseBlobMap` wrapper's job), so an
/// `RwLock<ExpanseBlobMap>` cannot legally be shared — the honest std
/// baseline is a `Mutex`, with `RwLock<BTreeMap>` below capturing the
/// reader-counter scaling behaviour of an `RwLock` on a `Sync` structure.
fn bench_blob_mutex(ratio_read: u32, readers: usize) -> (f64, f64) {
    let m = Arc::new(std::sync::Mutex::new(ExpanseBlobMap::new()));
    let mut rng = XorShift(0x5CA1_AB1E);
    let mut buf = [0u8; BLOB_LEN];
    {
        let mut g = m.lock().expect("lock");
        for _ in 0..BLOB_POP {
            let k = rng.next() % BLOB_KEYSPACE;
            blob_payload(k, &mut buf);
            g.insert(k, &buf, k as u32 & 0xFF_FFFF).expect("prefill");
        }
    }
    run_window(readers, move |i, stop| {
        let mut rng = XorShift(0x1000 + i as u64);
        let mut buf = [0u8; BLOB_LEN];
        let (mut read_ops, mut write_ops) = (0u64, 0u64);
        let mut sink = 0u64;
        while !stop.load(Ordering::Relaxed) {
            let r = (rng.next() % 100) as u32;
            let k = rng.next() % BLOB_KEYSPACE;
            // Payload generation stays outside the critical section so the
            // lock covers map work only (comparable to the OCC arm, whose
            // internal writer mutex sees pre-built payloads).
            if r < ratio_read {
                let g = m.lock().expect("lock");
                if let Some((view, meta)) = g.get(k) {
                    sink ^= u64::from(view.as_bytes()[0]) ^ u64::from(meta);
                }
                read_ops += 1;
            } else {
                if k & 1 == 0 {
                    blob_payload(k, &mut buf);
                    let mut g = m.lock().expect("lock");
                    let _ = g.insert(k, &buf, k as u32 & 0xFF_FFFF);
                } else {
                    let mut g = m.lock().expect("lock");
                    g.remove(k);
                }
                write_ops += 1;
            }
        }
        std::hint::black_box(sink);
        (read_ops, write_ops)
    })
}

type BlobBTree = std::collections::BTreeMap<u64, (Vec<u8>, u32)>;

fn bench_blob_rwlock_btree(ratio_read: u32, readers: usize) -> (f64, f64) {
    let m: Arc<RwLock<BlobBTree>> = Arc::new(RwLock::new(BlobBTree::new()));
    let mut rng = XorShift(0x5CA1_AB1E);
    let mut buf = [0u8; BLOB_LEN];
    {
        let mut g = m.write().expect("lock");
        for _ in 0..BLOB_POP {
            let k = rng.next() % BLOB_KEYSPACE;
            blob_payload(k, &mut buf);
            g.insert(k, (buf.to_vec(), k as u32 & 0xFF_FFFF));
        }
    }
    run_window(readers, move |i, stop| {
        let mut rng = XorShift(0x1000 + i as u64);
        let mut buf = [0u8; BLOB_LEN];
        let (mut read_ops, mut write_ops) = (0u64, 0u64);
        let mut sink = 0u64;
        while !stop.load(Ordering::Relaxed) {
            let r = (rng.next() % 100) as u32;
            let k = rng.next() % BLOB_KEYSPACE;
            if r < ratio_read {
                let g = m.read().expect("lock");
                if let Some((bytes, meta)) = g.get(&k) {
                    sink ^= u64::from(bytes[0]) ^ u64::from(*meta);
                }
                read_ops += 1;
            } else {
                // Payload generation and Vec construction stay outside the
                // write lock (see the Mutex arm).
                if k & 1 == 0 {
                    blob_payload(k, &mut buf);
                    let v = buf.to_vec();
                    let mut g = m.write().expect("lock");
                    g.insert(k, (v, k as u32 & 0xFF_FFFF));
                } else {
                    let mut g = m.write().expect("lock");
                    g.remove(&k);
                }
                write_ops += 1;
            }
        }
        std::hint::black_box(sink);
        (read_ops, write_ops)
    })
}

fn bench_blob_skiplist(ratio_read: u32, readers: usize) -> (f64, f64) {
    let m: Arc<SkipMap<u64, (Vec<u8>, u32)>> = Arc::new(SkipMap::new());
    let mut rng = XorShift(0x5CA1_AB1E);
    let mut buf = [0u8; BLOB_LEN];
    for _ in 0..BLOB_POP {
        let k = rng.next() % BLOB_KEYSPACE;
        blob_payload(k, &mut buf);
        m.insert(k, (buf.to_vec(), k as u32 & 0xFF_FFFF));
    }
    run_window(readers, move |i, stop| {
        let mut rng = XorShift(0x1000 + i as u64);
        let mut buf = [0u8; BLOB_LEN];
        let (mut read_ops, mut write_ops) = (0u64, 0u64);
        let mut sink = 0u64;
        while !stop.load(Ordering::Relaxed) {
            let r = (rng.next() % 100) as u32;
            let k = rng.next() % BLOB_KEYSPACE;
            if r < ratio_read {
                if let Some(e) = m.get(&k) {
                    let (bytes, meta) = e.value();
                    sink ^= u64::from(bytes[0]) ^ u64::from(*meta);
                }
                read_ops += 1;
            } else {
                if k & 1 == 0 {
                    blob_payload(k, &mut buf);
                    m.insert(k, (buf.to_vec(), k as u32 & 0xFF_FFFF));
                } else {
                    m.remove(&k);
                }
                write_ops += 1;
            }
        }
        std::hint::black_box(sink);
        (read_ops, write_ops)
    })
}

/// String arm (issue #219 Phase 2): URL-route-shaped keys (~40 bytes, 5
/// sub-trie hops) matching the prefix-routing workload; the key universe is
/// pre-generated so per-op costs are map work, not string formatting.
const STR_POP: usize = 100_000;
const STR_KEYSPACE: usize = 2 * STR_POP;

fn str_keys() -> Arc<Vec<Vec<u8>>> {
    Arc::new(
        (0..STR_KEYSPACE)
            .map(|i| format!("/api/v2/tenants/{:06}/resources/{:04}", i / 16, i % 16).into_bytes())
            .collect(),
    )
}

fn bench_str_sync(ratio_read: u32, readers: usize) -> (f64, f64) {
    let keys = str_keys();
    let m = Arc::new(SyncExpanseStrMap::new());
    let mut rng = XorShift(0x5CA1_AB1E);
    for _ in 0..STR_POP {
        let k = &keys[(rng.next() as usize) % STR_KEYSPACE];
        m.insert(k, rng.next());
    }
    run_window(readers, move |i, stop| {
        let rd = m.reader();
        let mut rng = XorShift(0x1000 + i as u64);
        let (mut read_ops, mut write_ops) = (0u64, 0u64);
        let mut sink = 0u64;
        while !stop.load(Ordering::Relaxed) {
            let r = (rng.next() % 100) as u32;
            let k = &keys[(rng.next() as usize) % STR_KEYSPACE];
            if r < ratio_read {
                sink ^= rd.get(k).unwrap_or(0);
                read_ops += 1;
            } else {
                if rng.next() & 1 == 0 {
                    m.insert(k, rng.next());
                } else {
                    m.remove(k);
                }
                write_ops += 1;
            }
        }
        std::hint::black_box(sink);
        (read_ops, write_ops)
    })
}

fn bench_str_mutex(ratio_read: u32, readers: usize) -> (f64, f64) {
    let keys = str_keys();
    let m = Arc::new(std::sync::Mutex::new(ExpanseStrMap::new()));
    let mut rng = XorShift(0x5CA1_AB1E);
    {
        let mut g = m.lock().expect("lock");
        for _ in 0..STR_POP {
            let k = &keys[(rng.next() as usize) % STR_KEYSPACE];
            g.insert(k, rng.next());
        }
    }
    run_window(readers, move |i, stop| {
        let mut rng = XorShift(0x1000 + i as u64);
        let (mut read_ops, mut write_ops) = (0u64, 0u64);
        let mut sink = 0u64;
        while !stop.load(Ordering::Relaxed) {
            let r = (rng.next() % 100) as u32;
            let k = &keys[(rng.next() as usize) % STR_KEYSPACE];
            let mut g = m.lock().expect("lock");
            if r < ratio_read {
                sink ^= g.get(k).unwrap_or(0);
                read_ops += 1;
            } else {
                if rng.next() & 1 == 0 {
                    g.insert(k, rng.next());
                } else {
                    g.remove(k);
                }
                write_ops += 1;
            }
        }
        std::hint::black_box(sink);
        (read_ops, write_ops)
    })
}

fn bench_str_dashmap(ratio_read: u32, readers: usize) -> (f64, f64) {
    let keys = str_keys();
    let m: Arc<DashMap<Vec<u8>, u64>> = Arc::new(DashMap::new());
    let mut rng = XorShift(0x5CA1_AB1E);
    for _ in 0..STR_POP {
        let k = &keys[(rng.next() as usize) % STR_KEYSPACE];
        m.insert(k.clone(), rng.next());
    }
    run_window(readers, move |i, stop| {
        let mut rng = XorShift(0x1000 + i as u64);
        let (mut read_ops, mut write_ops) = (0u64, 0u64);
        let mut sink = 0u64;
        while !stop.load(Ordering::Relaxed) {
            let r = (rng.next() % 100) as u32;
            let k = &keys[(rng.next() as usize) % STR_KEYSPACE];
            if r < ratio_read {
                sink ^= m.get(k.as_slice()).map_or(0, |e| *e.value());
                read_ops += 1;
            } else {
                if rng.next() & 1 == 0 {
                    m.insert(k.clone(), rng.next());
                } else {
                    m.remove(k.as_slice());
                }
                write_ops += 1;
            }
        }
        std::hint::black_box(sink);
        (read_ops, write_ops)
    })
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

    // Blob arm (issue #219): OCC wrapper vs RwLock baseline vs SkipMap,
    // 128-byte arena payloads, ~50% hit rate, payload bytes dereferenced.
    type EngineBench = fn(u32, usize) -> (f64, f64);
    let blob_engines: [(&str, EngineBench); 4] = [
        ("SyncExpanseBlobMap", bench_blob_sync),
        ("Mutex<ExpanseBlobMap>", bench_blob_mutex),
        (
            "RwLock<BTreeMap<u64, (Vec<u8>, u32)>>",
            bench_blob_rwlock_btree,
        ),
        ("SkipMap<u64, Vec<u8>>", bench_blob_skiplist),
    ];
    let str_engines: [(&str, EngineBench); 3] = [
        ("SyncExpanseStrMap", bench_str_sync),
        ("Mutex<ExpanseStrMap>", bench_str_mutex),
        ("DashMap<Vec<u8>, u64>", bench_str_dashmap),
    ];
    for (name, bench) in blob_engines.into_iter().chain(str_engines) {
        for (rr, _rw, wname) in workloads {
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
                let (rops, wops) = bench(rr, t);
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
