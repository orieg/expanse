//! Concurrency scalability benchmark for SyncExpanseSet, SyncExpanseMap,
//! SyncExpanseBlobMap, SyncExpanseStrMap and SyncExpanseBytesMap (issue
//! #219: the blob arm compares the OCC wrapper against a
//! `Mutex<ExpanseBlobMap>` baseline, an `RwLock<BTreeMap>` and
//! `crossbeam_skiplist`; the string arm against `Mutex<ExpanseStrMap>` and
//! `DashMap` — `ExpanseStrMap`, like the other single-threaded structures,
//! is deliberately `!Sync`, so an `RwLock<ExpanseStrMap>` cannot legally be
//! shared). The bytes arm (issue #362) runs the *identical* workload as the
//! string and `DashMap` arms — same keys, same distribution — so the
//! unordered hash-keyed wrapper is directly comparable to `DashMap` and to
//! the ordered cascade it sidesteps.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `core_concurrency` |
//! | `group` | 2 |
//! | `population` | 1M (keyspace 2M) |
//! | `probes_and_reuse` | Continuous stream in 500ms window |
//! | `hit_rate` | ~50% |
//! | `miss_gen_method` | Bounded keyspace random stream |
//! | `value_dereference` | `black_box(sink)` |
//! | `measured_region` | Clean (`run_window`) |
//! | `arm_symmetry` | Symmetric across concurrency primitives |
//! | `statistics` | Throughput ops/sec |
//! | `verdict` | **PASS** `[verified: RUN (reference host, run 33030152085)]`: Corrected in #375 to bounded keyspace. |

use crossbeam_skiplist::SkipMap;
use dashmap::DashMap;
use expanse_trie::ExpanseBlobMap;
use expanse_trie::bytesmap::ExpanseBytesMap;
use expanse_trie::strmap::ExpanseStrMap;
use expanse_trie::sync::{
    SyncExpanseBlobMap, SyncExpanseBytesMap, SyncExpanseMap, SyncExpanseSet, SyncExpanseStrMap,
};
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
/// Map/set arms: prefill and probes draw from the same bounded keyspace
/// (the `BLOB_KEYSPACE` construction the blob arm below uses), so reads
/// actually hit ~50% of the time and exercise the full descent-to-value
/// path. Until #375 these arms probed unbounded `u64` keys against a
/// 1M-key prefill (~100% miss), so the read numbers measured early-exit
/// descent on absent keys only.
const KEYSPACE: u64 = 2 * POP;
const WINDOW: Duration = Duration::from_millis(500);

fn bench_map(ratio_read: u32, _ratio_write: u32, readers: usize) -> (f64, f64) {
    let m = Arc::new(SyncExpanseMap::new());
    let mut rng = XorShift(0x5CA1_AB1E);
    for _ in 0..POP {
        let k = rng.next() % KEYSPACE;
        m.insert(k, !k);
    }
    run_window(readers, move |i, stop| {
        let rd = m.reader();
        let mut rng = XorShift(0x1000 + i as u64);
        let (mut read_ops, mut write_ops) = (0u64, 0u64);
        let mut sink = 0u64;
        while !stop.load(Ordering::Relaxed) {
            let r = (rng.next() % 100) as u32;
            let k = rng.next() % KEYSPACE;
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
        s.insert(rng.next() % KEYSPACE);
    }
    run_window(readers, move |i, stop| {
        let rd = s.reader();
        let mut rng = XorShift(0x1000 + i as u64);
        let (mut read_ops, mut write_ops) = (0u64, 0u64);
        let mut sink = false;
        while !stop.load(Ordering::Relaxed) {
            let r = (rng.next() % 100) as u32;
            let k = rng.next() % KEYSPACE;
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
///
/// The spawn ramp (thread creation + each worker's startup before its op
/// loop) is inside the measurement window. Every engine pays it identically
/// — an equal handicap, not a per-engine bias — and at `WINDOW` = 500 ms it
/// is a small, uniform deflation of absolute ops/sec.
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

/// Bytes arm (issue #362): the same workload as the string and DashMap
/// arms — identical keys, distribution, and op mix — over the unordered
/// hash-keyed wrapper.
fn bench_bytes_sync(ratio_read: u32, readers: usize) -> (f64, f64) {
    let keys = str_keys();
    let m = Arc::new(SyncExpanseBytesMap::new());
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

fn bench_bytes_mutex(ratio_read: u32, readers: usize) -> (f64, f64) {
    let keys = str_keys();
    let m = Arc::new(std::sync::Mutex::new(ExpanseBytesMap::new()));
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

/// Parses a comma-separated list from the environment variable `name`,
/// falling back to `default` when unset. CI uses these knobs
/// (`EXPANSE_BENCH_THREADS`, `EXPANSE_BENCH_WORKLOADS`) to bound the sweep;
/// local runs with the variables unset keep the full default sweep.
fn env_csv<T>(name: &str, default: &[T]) -> Vec<T>
where
    T: Copy + std::str::FromStr,
{
    match std::env::var(name) {
        Ok(s) => {
            let parsed: Vec<T> = s
                .split(',')
                .map(|p| {
                    p.trim()
                        .parse::<T>()
                        .unwrap_or_else(|_| panic!("invalid {name} entry: {p:?}"))
                })
                .collect();
            assert!(!parsed.is_empty(), "{name} must not be empty");
            parsed
        }
        Err(_) => default.to_vec(),
    }
}

fn main() {
    let max_threads = std::thread::available_parallelism()
        .map_or(16, usize::from)
        .min(16);
    // Thread counts and workload read-percentages; overridable for CI via
    // EXPANSE_BENCH_THREADS="1,4,16" / EXPANSE_BENCH_WORKLOADS="100,50".
    let threads_list = env_csv("EXPANSE_BENCH_THREADS", &[1usize, 2, 4, 8, 16]);
    let read_pcts = env_csv("EXPANSE_BENCH_WORKLOADS", &[100u32, 95, 50]);
    let workloads: Vec<(u32, u32, String)> = read_pcts
        .iter()
        .map(|&rr| {
            assert!(
                rr <= 100,
                "EXPANSE_BENCH_WORKLOADS entries are read percentages (0-100)"
            );
            (rr, 100 - rr, format!("{rr}% Read / {}% Write", 100 - rr))
        })
        .collect();

    for (name, is_map) in [("SyncExpanseMap", true), ("SyncExpanseSet", false)] {
        for (rr, rw, wname) in &workloads {
            println!("\n=== {} ({}) ===", name, wname);
            println!(
                "{:>8} {:>16} {:>16} {:>10}",
                "threads", "read ops/sec", "write ops/sec", "scale"
            );

            let mut base: Option<f64> = None;
            for &t in &threads_list {
                if t > max_threads && t != 1 {
                    continue;
                }
                let (rops, wops) = if is_map {
                    bench_map(*rr, *rw, t)
                } else {
                    bench_set(*rr, *rw, t)
                };

                let total = rops + wops;
                let base = *base.get_or_insert(total);

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
    let str_engines: [(&str, EngineBench); 5] = [
        ("SyncExpanseStrMap", bench_str_sync),
        ("Mutex<ExpanseStrMap>", bench_str_mutex),
        ("SyncExpanseBytesMap", bench_bytes_sync),
        ("Mutex<ExpanseBytesMap>", bench_bytes_mutex),
        ("DashMap<Vec<u8>, u64>", bench_str_dashmap),
    ];
    for (name, bench) in blob_engines.into_iter().chain(str_engines) {
        for (rr, _rw, wname) in &workloads {
            println!("\n=== {} ({}) ===", name, wname);
            println!(
                "{:>8} {:>16} {:>16} {:>10}",
                "threads", "read ops/sec", "write ops/sec", "scale"
            );
            let mut base: Option<f64> = None;
            for &t in &threads_list {
                if t > max_threads && t != 1 {
                    continue;
                }
                let (rops, wops) = bench(*rr, t);
                let total = rops + wops;
                let base = *base.get_or_insert(total);
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
