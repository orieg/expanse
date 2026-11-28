//! DNS-shaped keys: predecessor lookups from W concurrent readers.
//!
//! `docs/benchmarks/dns_names/METHODOLOGY.md` §5.
//! `SyncExpanseOrderedBytesMap`'s predecessor runs under the writer lock; only
//! its point `get` is optimistic. A server answering from zone snapshots would
//! read an immutable `ExpanseOrderedBytesMap` instead. That type is `Send` and
//! not `Sync` (its allocator handle has one owner), so it cannot be shared by
//! reference between threads: a snapshot reader owns a `Clone` of the map
//! (METHODOLOGY amendment A1). This harness sizes the difference between the
//! two, read-only, with no writer thread:
//!
//! - `clone_per_reader`: each reader thread owns a `Clone` of the map and
//!   calls `ExpanseOrderedBytesMap::prev_at_or_before_entry_decode_into` on it.
//! - `sync_locked`: `SyncExpanseOrderedBytesMap::prev_at_or_before_decode_into`.
//! - `sync_get`: `SyncExpanseOrderedBytesMap::get` on present names, the
//!   control that says whether the host and the pin allow scaling at all.
//!   The one-shot call registers a reader on every lookup.
//! - `sync_reader_get`: the same lookups through an `OrderedBytesReader` each
//!   thread registers once and keeps, an extra row with no prediction
//!   (METHODOLOGY amendment A1).
//!
//! One process times one round (`--round K`): every (arm, W) cell once, arm
//! order rotated by `K`. The suite runner starts one process per round. The
//! threads are not placed one by one: they run inside the affinity mask the
//! process inherits, which the row records, so every arm runs under one pin.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `dns_predecessor_concurrent` |
//! | `group` | 4 |
//! | `population` | 1M synthetic names (`dns_common::World`); 10k under `--quick` |
//! | `insertion_order` | sorted: ascending key order, the one build §5 pre-registers; the readers never write |
//! | `probes_and_reuse` | N/2 `miss_under_sld` names in a fixed permutation for the predecessor arms and N/2 present names for the two `get` arms; each reader cycles the stream from its own offset for a 1 s window (0.1 s under `--quick`) |
//! | `hit_rate` | Predecessor arms: 0% exact hits, every probe absent. `sync_get` and `sync_reader_get`: 100% |
//! | `miss_gen_method` | A generator-rule label in front of a uniformly drawn present two-label name, rejected on membership |
//! | `value_dereference` | Each reader folds value, key length and the key's last byte (the `get` arms: the value) into an accumulator that reaches `black_box` |
//! | `measured_region` | Each reader's loop between a start barrier and a stop flag read every 64 operations; builds, probe encoding, thread spawn and join are outside it; one discarded single-reader warm-up per arm |
//! | `arm_symmetry` | One name set, one build order and one probe stream; the same inherited affinity mask for every arm; the two predecessor arms decode into a per-thread 512-byte buffer. `clone_per_reader` holds W copies of the map, made before the cell and outside its window, where the `sync` arms hold one; `sync_get` and `sync_reader_get` are a different operation and each is compared only with itself across W |
//! | `statistics` | One round per process; `run_all.py` runs 8 processes and publishes S(W) = throughput(W) / throughput(1) per round with a BCa 95% interval over rounds |
//! | `verdict` | **PASS** `[verified: CODE READ]`: read-only cells under one recorded affinity mask, with a control arm. |

#[path = "dns_common/mod.rs"]
mod dns_common;

use dns_common::{World, cli, emit, key_nul, permutation};
use expanse_trie::ExpanseOrderedBytesMap;
use expanse_trie::sync::SyncExpanseOrderedBytesMap;
use serde_json::json;
use std::hint::black_box;
use std::sync::Barrier;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Decode buffer: the longest generated key is under 80 bytes.
const BUF: usize = 512;
/// Readers per cell (§5).
const READERS: [usize; 4] = [1, 2, 4, 8];
const ARMS: [&str; 4] = [
    "clone_per_reader",
    "sync_locked",
    "sync_get",
    "sync_reader_get",
];

/// The affinity mask the process, and so every reader thread, runs under.
fn cpus_allowed() -> String {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("Cpus_allowed_list:"))
                .map(|v| v.trim().to_string())
        })
        .unwrap_or_else(|| "unavailable (no /proc/self/status)".into())
}

/// One reader thread per element of `states`, each cycling `probes` through
/// `op` with its own state for `window`; returns operations per second summed
/// over the readers, each over its own elapsed time.
fn cell<S, F>(states: Vec<S>, window: Duration, probes: &[Vec<u8>], op: F) -> f64
where
    S: Send,
    F: Fn(&mut S, &[u8], &mut [u8; BUF]) -> u64 + Sync,
{
    let readers = states.len();
    let stop = AtomicBool::new(false);
    let barrier = Barrier::new(readers + 1);
    let (stop, barrier, op) = (&stop, &barrier, &op);
    std::thread::scope(|s| {
        let handles: Vec<_> = states
            .into_iter()
            .enumerate()
            .map(|(t, mut state)| {
                s.spawn(move || {
                    let mut buf = [0u8; BUF];
                    let mut i = t * probes.len() / readers;
                    let (mut ops, mut acc) = (0u64, 0u64);
                    barrier.wait();
                    let start = Instant::now();
                    while !stop.load(Ordering::Relaxed) {
                        for _ in 0..64 {
                            acc = acc.wrapping_add(op(&mut state, &probes[i], &mut buf));
                            i += 1;
                            if i == probes.len() {
                                i = 0;
                            }
                        }
                        ops += 64;
                    }
                    let elapsed = start.elapsed();
                    black_box(acc);
                    ops as f64 / elapsed.as_secs_f64()
                })
            })
            .collect();
        barrier.wait();
        std::thread::sleep(window);
        stop.store(true, Ordering::Relaxed);
        handles
            .into_iter()
            .map(|h| h.join().expect("reader thread"))
            .sum()
    })
}

#[inline(always)]
fn fold(value: u64, key: &[u8]) -> u64 {
    value
        .wrapping_add(key.len() as u64)
        .wrapping_add(u64::from(key[key.len() - 1]))
}

fn main() {
    let cli = cli();
    let n = match cli.pops.as_slice() {
        [n] => *n,
        _ if cli.quick => 10_000,
        _ => 1_000_000,
    };
    let window = Duration::from_millis(if cli.quick { 100 } else { 1_000 });
    let w = World::new(n);
    let build = w.build_order("sorted");

    let mut shared = ExpanseOrderedBytesMap::new();
    let sync = SyncExpanseOrderedBytesMap::new();
    for &i in &build {
        assert_eq!(shared.insert(&w.keys_nul[i], i as u64), None);
        assert_eq!(sync.insert(&w.keys_nul[i], i as u64), None);
    }
    assert_eq!(shared.len() as usize, n);
    assert_eq!(sync.len() as usize, n);

    let misses = w.miss_probes(&w.under_sld, key_nul);
    let hits: Vec<Vec<u8>> = permutation(w.hit_idx.len(), dns_common::PROBE_SEED)
        .into_iter()
        .map(|j| w.keys_nul[w.hit_idx[j]].clone())
        .collect();

    // The two predecessor arms must name the same entry for every probe, and
    // the control must find every present name.
    let (mut a, mut b) = ([0u8; BUF], [0u8; BUF]);
    for k in &misses {
        let x = shared
            .prev_at_or_before_entry_decode_into(k, &mut a)
            .expect("decode buffer");
        let y = sync
            .prev_at_or_before_decode_into(k, &mut b)
            .expect("decode buffer");
        assert_eq!(
            x.map(|(len, v)| (&a[..len], v)),
            y.map(|(len, v)| (&b[..len], v)),
            "the locked and the plain predecessor disagree"
        );
    }
    assert!(hits.iter().all(|k| sync.get(k).is_some()));

    // One copy of the map per reader, made once, outside every window.
    let max_readers = *READERS.iter().max().expect("reader counts");
    let mut clones: Vec<ExpanseOrderedBytesMap> =
        (0..max_readers).map(|_| shared.clone()).collect();
    for c in &clones {
        assert_eq!(c.len() as usize, n, "a clone lost entries");
    }
    drop(shared);
    let sync = &sync;
    let mut run = |arm: usize, readers: usize, window: Duration| -> f64 {
        match arm {
            0 => {
                let owned: Vec<&mut ExpanseOrderedBytesMap> =
                    clones.iter_mut().take(readers).collect();
                cell(owned, window, &misses, |m, k, buf| {
                    match m.prev_at_or_before_entry_decode_into(k, buf) {
                        Ok(Some((len, v))) => fold(v, &buf[..len]),
                        Ok(None) => 0,
                        Err(e) => panic!("decode buffer too small: {e:?}"),
                    }
                })
            }
            1 => cell(vec![(); readers], window, &misses, |(), k, buf| match sync
                .prev_at_or_before_decode_into(k, buf)
            {
                Ok(Some((len, v))) => fold(v, &buf[..len]),
                Ok(None) => 0,
                Err(e) => panic!("decode buffer too small: {e:?}"),
            }),
            2 => cell(vec![(); readers], window, &hits, |(), k, _| {
                sync.get(k).unwrap_or(0)
            }),
            _ => {
                // One registered reader per thread, kept for the whole cell.
                let handles: Vec<_> = (0..readers).map(|_| sync.reader()).collect();
                cell(handles, window, &hits, |r, k, _| r.get(k).unwrap_or(0))
            }
        }
    };

    for arm in 0..ARMS.len() {
        black_box(run(arm, 1, window / 5));
    }
    let allowed = cpus_allowed();
    let mut cells = Vec::new();
    for (wi, &readers) in READERS.iter().enumerate() {
        for i in 0..ARMS.len() {
            let arm = (cli.round + wi + i) % ARMS.len();
            let ops_per_s = run(arm, readers, window);
            assert!(ops_per_s > 0.0, "{}: no operation completed", ARMS[arm]);
            cells.push(json!({
                "population": n,
                "order": "sorted",
                "arm": ARMS[arm],
                "readers": readers,
                "probe_class": if arm >= 2 { "hit" } else { "miss_under_sld" },
                "rounds_raw": [{
                    "round": cli.round,
                    "arm": ARMS[arm],
                    "readers": readers,
                    "ops_per_s": ops_per_s,
                    "window_s": window.as_secs_f64(),
                    "cpus_allowed": allowed,
                }],
            }));
        }
    }
    cells.sort_by_key(|c| {
        (
            ARMS.iter().position(|a| c["arm"] == *a),
            c["readers"].as_u64(),
        )
    });
    emit("dns_predecessor_concurrent", &cli, "throughput", cells);
}
