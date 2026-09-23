//! Patricia / radix tries vs Expanse: live-heap census.
//!
//! Two instruments on one GlobalAlloc hook, for every arm:
//! - **requested** bytes (`Layout::size`), what each structure asks for;
//! - **usable** bytes (`malloc_usable_size` on Linux, `malloc_size` on macOS),
//!   what the allocator actually hands out. Small nodes are rounded up far more
//!   than large ones, so the two can rank arms differently; both are recorded.
//!
//! Every arm is built in both orders and the counts recorded per order: the
//! allocator census is order-sensitive for Expanse (§8.12.4), and a Patricia
//! tree's node set is fixed by its key set only for keys of at most 255 bytes.
//! `u64` keys over the five distributions; shared-prefix string keys at four
//! prefix lengths (`ExpanseStrMap` vs the twins), in generator and sorted
//! order. A twin that panics or loses keys is recorded as invalid.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `patricia_memory` |
//! | `group` | 4 |
//! | `population` | 1k to 1M |
//! | `insertion_order` | both — `u64`: generator order and a Fisher–Yates permutation under `PROBE_SHUFFLE_SEED`; strings: generator (random ids) and sorted; per-order counts on every row |
//! | `probes_and_reuse` | N/A (memory) |
//! | `hit_rate` | N/A |
//! | `miss_gen_method` | None |
//! | `value_dereference` | None; counters read from the allocator hook |
//! | `measured_region` | Build loop only; each structure dropped before the next is built |
//! | `arm_symmetry` | Identical key sets and values, one allocator hook for all arms; every arm owns its key bytes |
//! | `statistics` | Exact deterministic byte counts (requested and usable) |
//! | `verdict` | **PENDING** `[unmeasured]`: no committed run yet. |

#[path = "art_common/mod.rs"]
mod art_common;
#[path = "patricia_common/mod.rs"]
mod patricia_common;

use art_common::ExpanseMap;
use expanse_trie::strmap::ExpanseStrMap;
use patricia_common::{
    DISTS, PREFIX_LENS, PatriciaMap, QpTrie, RadixMap, STRING_SEED, Twin, as_nulfree, build_twin,
    cli, emit, gen_paths, path_val, pkey, shuffled, u64_dist, val,
};
use serde_json::{Map, Value, json};
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

struct TrackingAlloc;
static REQUESTED: AtomicUsize = AtomicUsize::new(0);
static USABLE: AtomicUsize = AtomicUsize::new(0);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);

/// Name of the usable-size instrument on this target, recorded per artifact.
#[cfg(target_os = "linux")]
const USABLE_INSTRUMENT: &str = "malloc_usable_size";
#[cfg(target_os = "macos")]
const USABLE_INSTRUMENT: &str = "malloc_size";
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
const USABLE_INSTRUMENT: &str = "unavailable: requested size recorded";

/// Bytes the system allocator reserved for a live block from `System`.
fn usable(ptr: *mut u8, layout: Layout) -> usize {
    #[cfg(target_os = "linux")]
    {
        let _ = layout;
        // SAFETY: `ptr` is a live block returned by `System` (glibc malloc
        // family on Linux), which is what malloc_usable_size accepts.
        unsafe { libc::malloc_usable_size(ptr.cast()) }
    }
    #[cfg(target_os = "macos")]
    {
        let _ = layout;
        // SAFETY: `ptr` is a live block returned by `System` (libmalloc on
        // macOS), which is what malloc_size accepts.
        unsafe { libc::malloc_size(ptr.cast_const().cast()) }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = ptr;
        layout.size()
    }
}

// SAFETY: forwards every call to the System allocator unchanged; the counters
// are bookkeeping only and never affect the returned pointer or layout.
unsafe impl GlobalAlloc for TrackingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: caller upholds GlobalAlloc::alloc's contract; delegated as is.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            REQUESTED.fetch_add(layout.size(), Ordering::Relaxed);
            USABLE.fetch_add(usable(ptr, layout), Ordering::Relaxed);
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        REQUESTED.fetch_sub(layout.size(), Ordering::Relaxed);
        USABLE.fetch_sub(usable(ptr, layout), Ordering::Relaxed);
        ALLOCS.fetch_sub(1, Ordering::Relaxed);
        // SAFETY: `ptr` came from `alloc` above with this `layout`.
        unsafe { System.dealloc(ptr, layout) };
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let old_usable = usable(ptr, layout);
        // SAFETY: caller upholds GlobalAlloc::realloc's contract; delegated as is.
        let new = unsafe { System.realloc(ptr, layout, new_size) };
        if !new.is_null() {
            // SAFETY: `new` has `layout.align()` and `new_size`, per realloc.
            let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
            REQUESTED.fetch_sub(layout.size(), Ordering::Relaxed);
            REQUESTED.fetch_add(new_size, Ordering::Relaxed);
            USABLE.fetch_sub(old_usable, Ordering::Relaxed);
            USABLE.fetch_add(usable(new, new_layout), Ordering::Relaxed);
        }
        new
    }
}

#[global_allocator]
static GLOBAL: TrackingAlloc = TrackingAlloc;

fn snap() -> [usize; 3] {
    [
        REQUESTED.load(Ordering::SeqCst),
        USABLE.load(Ordering::SeqCst),
        ALLOCS.load(Ordering::SeqCst),
    ]
}

/// Builds with `f`, records the three counters' growth under `<arm>_<order>_*`,
/// then drops the structure. `Err` is recorded as invalid.
fn census<T>(
    row: &mut Map<String, Value>,
    arm: &str,
    order: &str,
    n: usize,
    f: impl FnOnce() -> Result<T, String>,
) -> Option<usize> {
    let base = snap();
    let built = f();
    let now = snap();
    let p = format!("{arm}_{order}");
    match built {
        Ok(t) => {
            let d: Vec<usize> = (0..3).map(|i| now[i].saturating_sub(base[i])).collect();
            row.insert(format!("{p}_requested_bytes"), json!(d[0]));
            row.insert(format!("{p}_usable_bytes"), json!(d[1]));
            row.insert(format!("{p}_allocs"), json!(d[2]));
            row.insert(
                format!("{p}_requested_bytes_per_key"),
                json!(d[0] as f64 / n as f64),
            );
            row.insert(
                format!("{p}_usable_bytes_per_key"),
                json!(d[1] as f64 / n as f64),
            );
            black_box(&t);
            drop(t);
            Some(d[0])
        }
        Err(e) => {
            row.insert(format!("{p}_status"), json!("invalid"));
            row.insert(format!("{p}_invalid_reason"), json!(e));
            None
        }
    }
}

fn twins<K: AsRef<[u8]>>(
    row: &mut Map<String, Value>,
    order: &str,
    keys: &[K],
    vals: &[u64],
) -> Option<usize>
where
    PatriciaMap<u64>: Twin<K>,
    RadixMap<u64>: Twin<K>,
    QpTrie<K, u64>: Twin<K>,
{
    let n = keys.len();
    let pt = census(row, "patricia_tree", order, n, || {
        build_twin::<K, PatriciaMap<u64>>(keys, vals)
    });
    census(row, "fast_radix_trie", order, n, || {
        build_twin::<K, RadixMap<u64>>(keys, vals)
    });
    census(row, "qp_trie", order, n, || {
        build_twin::<K, QpTrie<K, u64>>(keys, vals)
    });
    pt
}

fn u64_row(dist: &str, n: usize) -> Value {
    let keys = u64_dist(dist, n);
    let mut row = Map::new();
    let mut pt = Vec::new();
    for (order, ks) in [("generator", keys.clone()), ("shuffled", shuffled(&keys))] {
        let mut mem_used = 0;
        census(&mut row, "expanse", order, ks.len(), || {
            let mut m = ExpanseMap::new();
            for &k in &ks {
                m.insert(k, val(k));
            }
            mem_used = m.mem_used();
            Ok::<_, String>(m)
        });
        row.insert(format!("expanse_{order}_mem_used"), json!(mem_used));
        let bytes: Vec<[u8; 8]> = ks.iter().map(|&k| pkey(k)).collect();
        let vals: Vec<u64> = ks.iter().map(|&k| val(k)).collect();
        pt.push(twins(&mut row, order, &bytes, &vals));
    }
    row.insert(
        "patricia_tree_order_invariant".into(),
        json!(pt[0].is_some() && pt[0] == pt[1]),
    );
    row.insert("key_type".into(), json!("u64"));
    row.insert("distribution".into(), json!(dist));
    row.insert("population".into(), json!(keys.len()));
    row.insert("raw_draws".into(), json!(n));
    Value::Object(row)
}

fn path_row(n: usize, prefix_len: usize) -> Value {
    let keys = gen_paths(n, prefix_len, STRING_SEED);
    let mut sorted = keys.clone();
    sorted.sort();
    let mut row = Map::new();
    let mut pt = Vec::new();
    for (order, ks) in [("generator", &keys), ("sorted", &sorted)] {
        let nf = as_nulfree(ks);
        let vals: Vec<u64> = ks.iter().map(|k| path_val(k)).collect();
        let mut mem_used = 0;
        census(&mut row, "expanse", order, ks.len(), || {
            let mut m = ExpanseStrMap::new();
            for (k, &v) in nf.iter().zip(&vals) {
                m.insert(k, v);
            }
            mem_used = m.mem_used();
            Ok::<_, String>(m)
        });
        row.insert(format!("expanse_{order}_mem_used"), json!(mem_used));
        pt.push(twins(&mut row, order, ks, &vals));
    }
    row.insert(
        "patricia_tree_order_invariant".into(),
        json!(pt[0].is_some() && pt[0] == pt[1]),
    );
    row.insert("key_type".into(), json!("prefixed_path"));
    row.insert("distribution".into(), json!("prefixed_path"));
    row.insert("prefix_len".into(), json!(prefix_len));
    row.insert("key_len".into(), json!(prefix_len + 12));
    row.insert("population".into(), json!(n));
    Value::Object(row)
}

fn main() {
    let cli = cli(&[1_000, 10_000, 100_000, 1_000_000]);
    let mut rows = Vec::new();
    for &n in &cli.pops {
        for dist in DISTS {
            rows.push(u64_row(dist, n));
        }
        for pl in PREFIX_LENS {
            rows.push(path_row(n, pl));
        }
    }
    for r in rows.iter_mut() {
        r["usable_instrument"] = json!(USABLE_INSTRUMENT);
    }
    emit("patricia_memory", "patricia_memory", &cli, rows);
}
