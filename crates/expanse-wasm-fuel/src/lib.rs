//! Fuel-metered WebAssembly benchmark module: the deterministic instrument for
//! the wasm targets (#629).
//!
//! Callgrind cannot see a `.wasm`, and V8 wall clock on a shared runner cannot
//! gate anything. This crate is the wasm analogue of
//! `crates/expanse/benches/instructions.rs`: one module, built for
//! `wasm32-unknown-unknown` and `wasm64-unknown-unknown`, whose exports each
//! run one benchmark arm end to end. `scripts/wasm_fuel.py` instantiates it
//! under wasmtime with fuel metering and reads back the exact fuel each export
//! consumed. Fuel is charged per executed instruction and is deterministic for
//! a given module and runtime version, so the number is an integer with no
//! variance, and two identical runs must agree to the unit.
//!
//! **The same source measures both engines.** On a 32-bit target the public
//! aliases point at the 32-bit engine (`ExpanseMap` is `ExpanseMap32`, 8-byte
//! `Edge32`); on wasm64 they point at the 64-bit engine. Nothing in this file
//! is width-specific beyond the key type, so the wasm32 and wasm64 fuel counts
//! of one arm are the same fixture on the two engines under one runtime.
//!
//! **Measured region.** Every export takes a `phase`: `0` performs only the
//! setup (key generation, structure build, probe selection) and returns a
//! checksum of it; `1` performs the setup and then the arm's loop. The arm's
//! cost is `fuel(phase 1) - fuel(phase 0)`, both exact, so construction cancels
//! and only the loop remains. Every loop folds what it reads into the returned
//! checksum, so nothing can be elided.
//!
//! **Teardown is never metered.** Every export hands its tree to [`leak`]
//! instead of dropping it, in both phases. Dropping would not cancel: the two
//! phases end with trees of different sizes (`insert` has no tree in phase 0
//! and a full one in phase 1; `remove` has a full one in phase 0 and an empty
//! one in phase 1), so the delta would carry a full tree's teardown with a
//! sign that depends on the arm, and a change to `Drop` alone would move the
//! `insert` and `remove` arms. Leaking is safe here because the driver runs
//! every call on a fresh wasmtime instance and discards its linear memory
//! afterwards; teardown has no arm of its own in this module.
//!
//! **Exports.** `map_insert`, `map_get`, `map_iterate`, `map_range`,
//! `map_remove`, `set_insert`, `set_contains`, `set_iterate`, `set_range`,
//! `set_remove`, each `(pop: u32, dist: u32, phase: u32) -> u64`, plus
//! `map_mem_used` / `set_mem_used` `(pop, dist) -> u64` bytes, the engine's own
//! accounting. `dist` is `0` sequential, `1` clustered, `2` random.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `wasm_fuel` |
//! | `group` | `6` |
//! | population | 10,000 keys per arm (the driver's default); `sequential` is 0..N, `clustered` is runs of 8 keys 4,096 apart, `random` is XorShift64 seed `0x0DDB_1A5E_5EED_0001` truncated to the key width, duplicates dropped |
//! | insertion_order | generator draw order — the population is inserted as drawn, neither sorted nor shuffled; the sorted copy in this file computes the range windows and is not the build |
//! | probes_and_reuse | 10,000 probes per arm in a seeded Fisher-Yates order, each once; reuse 1.0; `iterate` walks every key once; `range` is 100 windows that together cover the key space once |
//! | hit_rate | `insert` and `remove`: 100%, every key exactly once; `get` and `contains`: 50% hit / 50% miss |
//! | miss_gen_method | misses are drawn from the same XorShift64 stream after the population and rejected on membership; never a transform of a present key |
//! | value_dereference | every value, key and count the loop produces is XORed or added into the `u64` the export returns |
//! | measured_region | fuel of phase 1 minus fuel of phase 0, exact integers; the build and the probe selection are in both phases and cancel; the tree is leaked, never dropped, in both phases, so no teardown is metered |
//! | arm_symmetry | identical key stream, probe order and windows for the map and set arms and for both targets; the module source is byte-identical across wasm32 and wasm64 |
//! | statistics | none needed: wasmtime fuel is deterministic per module and runtime version; the driver runs every export twice and refuses to publish unless both runs agree to the unit |
//! | verdict | Callgrind analogue for the wasm targets. Fewer fuel units is better; `scripts/wasm_fuel.py --check-baseline` gates a change the way `perf_report.py` gates instruction counts |

// `K` is `u32` on wasm32 and `u64` on wasm64; every `as u64` / `as K` below is
// a real widening on one width and an identity on the other. The lint cannot
// see both builds at once, so it is silenced for the crate rather than at
// twenty sites.
#![allow(clippy::unnecessary_cast)]

extern crate alloc;

use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use expanse_trie::{ExpanseMap, ExpanseSet};

#[cfg(target_pointer_width = "32")]
type K = u32;
#[cfg(target_pointer_width = "64")]
type K = u64;

/// The seed every bindings harness shares (`docs/BINDINGS_BENCHMARKS.md`).
const SEED: u64 = 0x0DDB_1A5E_5EED_0001;
/// A second, independent stream for the probe order so it is not the
/// generation order shifted.
const PROBE_SEED: u64 = 0x9E37_79B9_7F4A_7C15;
/// Number of `range` windows per arm.
const WINDOWS: usize = 100;

const DIST_SEQUENTIAL: u32 = 0;
const DIST_CLUSTERED: u32 = 1;

struct XorShift64(u64);

impl XorShift64 {
    #[inline]
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

#[inline]
fn value_of(k: K) -> K {
    ((k as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x55) as K
}

/// The population for a distribution. Random keys are the shared XorShift64
/// stream truncated to the key width; a duplicate (possible on the 32-bit
/// width) is skipped so every arm sees each key exactly once. The generator
/// is returned so miss selection can continue the same stream.
fn population(pop: u32, dist: u32) -> (Vec<K>, XorShift64) {
    let mut rng = XorShift64(SEED);
    let n = pop as usize;
    let keys: Vec<K> = match dist {
        DIST_SEQUENTIAL => (0..pop).map(|i| i as K).collect(),
        DIST_CLUSTERED => (0..pop)
            .map(|i| ((i / 8) as K) * 4096 + (i % 8) as K)
            .collect(),
        _ => {
            let mut seen = BTreeSet::new();
            let mut v = Vec::with_capacity(n);
            while v.len() < n {
                let k = rng.next() as K;
                if seen.insert(k) {
                    v.push(k);
                }
            }
            v
        }
    };
    (keys, rng)
}

/// Seeded Fisher-Yates over a copy of `keys`.
fn shuffled(keys: &[K]) -> Vec<K> {
    let mut v = keys.to_vec();
    let mut rng = XorShift64(PROBE_SEED);
    for i in (1..v.len()).rev() {
        let j = (rng.next() % (i as u64 + 1)) as usize;
        v.swap(i, j);
    }
    v
}

/// The 50% hit / 50% miss probe set for the point arms: half of the shuffled
/// population, and as many misses drawn from the continuation of the same
/// stream and rejected on membership, interleaved in a seeded order.
fn hit_miss_probes(keys: &[K], rng: &mut XorShift64, present: impl Fn(K) -> bool) -> Vec<K> {
    let n = keys.len();
    let hits = &shuffled(keys)[..n / 2];
    let mut misses = Vec::with_capacity(n - n / 2);
    let mut chosen = BTreeSet::new();
    while misses.len() < n - n / 2 {
        let k = rng.next() as K;
        if !present(k) && chosen.insert(k) {
            misses.push(k);
        }
    }
    let mut probes: Vec<K> = hits.iter().chain(misses.iter()).copied().collect();
    let mut order = XorShift64(PROBE_SEED ^ 0xA5A5);
    for i in (1..probes.len()).rev() {
        let j = (order.next() % (i as u64 + 1)) as usize;
        probes.swap(i, j);
    }
    probes
}

/// `WINDOWS` inclusive key windows that partition the sorted population.
fn windows(keys: &[K]) -> Vec<(K, K)> {
    let mut sorted = keys.to_vec();
    sorted.sort_unstable();
    let n = sorted.len();
    (0..WINDOWS)
        .map(|w| {
            let lo = sorted[w * n / WINDOWS];
            let hi = sorted[((w + 1) * n / WINDOWS)
                .saturating_sub(1)
                .max(w * n / WINDOWS)];
            (lo, hi)
        })
        .collect()
}

fn build_map(keys: &[K]) -> ExpanseMap {
    let mut m = ExpanseMap::new();
    for &k in keys {
        m.insert(k, value_of(k));
    }
    m
}

fn build_set(keys: &[K]) -> ExpanseSet {
    let mut s = ExpanseSet::new();
    for &k in keys {
        s.insert(k);
    }
    s
}

/// Ends a tree's life without running its `Drop`, so its teardown is metered
/// in neither phase. Every export passes the tree it built to this on every
/// return path; see the module documentation for why dropping does not cancel.
#[inline]
fn leak<T>(tree: T) {
    core::mem::forget(tree);
}

#[inline]
fn fold(acc: u64, x: u64) -> u64 {
    acc.rotate_left(5) ^ x
}

// ---------------------------------------------------------------- map arms

/// Insert every key once, in generation order. Phase 0 generates the keys and
/// the shuffled order the other arms use, so the insertion loop is the delta.
#[unsafe(no_mangle)]
pub extern "C" fn map_insert(pop: u32, dist: u32, phase: u32) -> u64 {
    let (keys, _) = population(pop, dist);
    let probes = shuffled(&keys);
    let mut acc = fold(keys.len() as u64, probes[0] as u64);
    if phase == 0 {
        return acc;
    }
    let mut m = ExpanseMap::new();
    for &k in &keys {
        if let Some(old) = m.insert(k, value_of(k)) {
            acc = fold(acc, old as u64);
        }
    }
    let acc = fold(acc, m.len() as u64);
    leak(m);
    acc
}

/// Point lookup at 50% hit; every returned value is folded into the checksum.
#[unsafe(no_mangle)]
pub extern "C" fn map_get(pop: u32, dist: u32, phase: u32) -> u64 {
    let (keys, mut rng) = population(pop, dist);
    let m = build_map(&keys);
    let probes = hit_miss_probes(&keys, &mut rng, |k| m.contains_key(k));
    let mut acc = fold(m.len() as u64, probes[0] as u64);
    if phase == 0 {
        leak(m);
        return acc;
    }
    for &k in &probes {
        if let Some(v) = m.get(k) {
            acc = fold(acc, v as u64);
        } else {
            acc = acc.wrapping_add(1);
        }
    }
    leak(m);
    acc
}

/// Full ordered walk.
#[unsafe(no_mangle)]
pub extern "C" fn map_iterate(pop: u32, dist: u32, phase: u32) -> u64 {
    let (keys, _) = population(pop, dist);
    let m = build_map(&keys);
    let mut acc = m.len() as u64;
    if phase == 0 {
        leak(m);
        return acc;
    }
    for (k, v) in m.iter() {
        acc = fold(acc, (k as u64) ^ (v as u64));
    }
    leak(m);
    acc
}

/// `WINDOWS` inclusive range scans that together cover the key space once.
#[unsafe(no_mangle)]
pub extern "C" fn map_range(pop: u32, dist: u32, phase: u32) -> u64 {
    let (keys, _) = population(pop, dist);
    let m = build_map(&keys);
    let ws = windows(&keys);
    let mut acc = fold(m.len() as u64, ws.len() as u64);
    if phase == 0 {
        leak(m);
        return acc;
    }
    for &(lo, hi) in &ws {
        for (k, v) in m.range(lo..=hi) {
            acc = fold(acc, (k as u64) ^ (v as u64));
        }
    }
    leak(m);
    acc
}

/// Remove every key once, in the seeded shuffled order (scattered removal).
#[unsafe(no_mangle)]
pub extern "C" fn map_remove(pop: u32, dist: u32, phase: u32) -> u64 {
    let (keys, _) = population(pop, dist);
    let mut m = build_map(&keys);
    let probes = shuffled(&keys);
    let mut acc = fold(m.len() as u64, probes[0] as u64);
    if phase == 0 {
        leak(m);
        return acc;
    }
    for &k in &probes {
        if let Some(v) = m.remove(k) {
            acc = fold(acc, v as u64);
        }
    }
    let acc = fold(acc, m.len() as u64);
    leak(m);
    acc
}

/// Bytes the map accounts for itself after the build (`mem_used`).
#[unsafe(no_mangle)]
pub extern "C" fn map_mem_used(pop: u32, dist: u32) -> u64 {
    let (keys, _) = population(pop, dist);
    build_map(&keys).mem_used() as u64
}

// ---------------------------------------------------------------- set arms

/// Insert every key once, in generation order.
#[unsafe(no_mangle)]
pub extern "C" fn set_insert(pop: u32, dist: u32, phase: u32) -> u64 {
    let (keys, _) = population(pop, dist);
    let probes = shuffled(&keys);
    let mut acc = fold(keys.len() as u64, probes[0] as u64);
    if phase == 0 {
        return acc;
    }
    let mut s = ExpanseSet::new();
    for &k in &keys {
        acc = acc.wrapping_add(u64::from(s.insert(k)));
    }
    let acc = fold(acc, s.len() as u64);
    leak(s);
    acc
}

/// Membership at 50% hit.
#[unsafe(no_mangle)]
pub extern "C" fn set_contains(pop: u32, dist: u32, phase: u32) -> u64 {
    let (keys, mut rng) = population(pop, dist);
    let s = build_set(&keys);
    let probes = hit_miss_probes(&keys, &mut rng, |k| s.contains(k));
    let mut acc = fold(s.len() as u64, probes[0] as u64);
    if phase == 0 {
        leak(s);
        return acc;
    }
    for &k in &probes {
        acc = fold(acc, u64::from(s.contains(k)));
    }
    leak(s);
    acc
}

/// Full ordered walk.
#[unsafe(no_mangle)]
pub extern "C" fn set_iterate(pop: u32, dist: u32, phase: u32) -> u64 {
    let (keys, _) = population(pop, dist);
    let s = build_set(&keys);
    let mut acc = s.len() as u64;
    if phase == 0 {
        leak(s);
        return acc;
    }
    for k in s.iter() {
        acc = fold(acc, k as u64);
    }
    leak(s);
    acc
}

/// `WINDOWS` inclusive range scans that together cover the key space once.
#[unsafe(no_mangle)]
pub extern "C" fn set_range(pop: u32, dist: u32, phase: u32) -> u64 {
    let (keys, _) = population(pop, dist);
    let s = build_set(&keys);
    let ws = windows(&keys);
    let mut acc = fold(s.len() as u64, ws.len() as u64);
    if phase == 0 {
        leak(s);
        return acc;
    }
    for &(lo, hi) in &ws {
        for k in s.range(lo..=hi) {
            acc = fold(acc, k as u64);
        }
    }
    leak(s);
    acc
}

/// Remove every key once, in the seeded shuffled order.
#[unsafe(no_mangle)]
pub extern "C" fn set_remove(pop: u32, dist: u32, phase: u32) -> u64 {
    let (keys, _) = population(pop, dist);
    let mut s = build_set(&keys);
    let probes = shuffled(&keys);
    let mut acc = fold(s.len() as u64, probes[0] as u64);
    if phase == 0 {
        leak(s);
        return acc;
    }
    for &k in &probes {
        acc = acc.wrapping_add(u64::from(s.remove(k)));
    }
    let acc = fold(acc, s.len() as u64);
    leak(s);
    acc
}

/// Bytes the set accounts for itself after the build (`mem_used`).
#[unsafe(no_mangle)]
pub extern "C" fn set_mem_used(pop: u32, dist: u32) -> u64 {
    let (keys, _) = population(pop, dist);
    build_set(&keys).mem_used() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;
    use std::alloc::{GlobalAlloc, Layout, System};

    std::thread_local! {
        /// Bytes allocated minus bytes freed on this thread. Thread-local so
        /// the other tests running in parallel do not move it.
        static NET: Cell<isize> = const { Cell::new(0) };
    }

    /// The system allocator plus a per-thread live-byte count.
    struct Counting;

    impl Counting {
        fn add(delta: isize) {
            // An allocation during thread teardown, after the slot is gone,
            // goes uncounted; no test reads the counter at that point.
            NET.try_with(|n| n.set(n.get() + delta)).unwrap_or(());
        }
    }

    // SAFETY: every method forwards to `System` with the caller's own
    // arguments, so `System`'s guarantees are this allocator's; the only
    // addition is a counter update, which neither allocates nor unwinds.
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            // SAFETY: the caller upholds `GlobalAlloc::alloc`'s contract,
            // which is `System::alloc`'s.
            let p = unsafe { System.alloc(layout) };
            if !p.is_null() {
                Self::add(layout.size() as isize);
            }
            p
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            Self::add(-(layout.size() as isize));
            // SAFETY: `ptr` was returned by `alloc` above, i.e. by `System`,
            // with this `layout`, as the caller guarantees.
            unsafe { System.dealloc(ptr, layout) }
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            // SAFETY: the caller upholds `GlobalAlloc::realloc`'s contract,
            // and `ptr` came from `System` through this allocator.
            let p = unsafe { System.realloc(ptr, layout, new_size) };
            if !p.is_null() {
                Self::add(new_size as isize - layout.size() as isize);
            }
            p
        }
    }

    #[global_allocator]
    static ALLOC: Counting = Counting;

    /// Live bytes an export leaves behind on this thread.
    fn net_bytes(f: extern "C" fn(u32, u32, u32) -> u64, pop: u32, dist: u32, phase: u32) -> isize {
        let before = NET.with(Cell::get);
        core::hint::black_box(f(pop, dist, phase));
        NET.with(Cell::get) - before
    }

    /// No export runs a tree's teardown inside a metered phase: every phase
    /// that ends holding a populated tree leaves at least that tree's bytes
    /// behind, and a phase that never built one leaves nothing. Dropping the
    /// tree instead (the pre-fix harness) returns every byte and fails the
    /// first assertion; the phase-1 `remove` tree is empty, so it is the one
    /// case this cannot see.
    #[test]
    fn no_phase_meters_tree_teardown() {
        const POP: u32 = 500;
        for dist in 0..3 {
            let (keys, _) = population(POP, dist);
            let map_bytes = build_map(&keys).mem_used() as isize;
            let set_bytes = build_set(&keys).mem_used() as isize;
            assert!(map_bytes > 0 && set_bytes > 0);
            type Arm = extern "C" fn(u32, u32, u32) -> u64;
            // (export, tree bytes, phases that end holding a populated tree)
            let cases: [(&str, Arm, isize, &[u32]); 10] = [
                ("map_insert", map_insert, map_bytes, &[1]),
                ("map_get", map_get, map_bytes, &[0, 1]),
                ("map_iterate", map_iterate, map_bytes, &[0, 1]),
                ("map_range", map_range, map_bytes, &[0, 1]),
                ("map_remove", map_remove, map_bytes, &[0]),
                ("set_insert", set_insert, set_bytes, &[1]),
                ("set_contains", set_contains, set_bytes, &[0, 1]),
                ("set_iterate", set_iterate, set_bytes, &[0, 1]),
                ("set_range", set_range, set_bytes, &[0, 1]),
                ("set_remove", set_remove, set_bytes, &[0]),
            ];
            for (name, f, bytes, holding) in cases {
                for &phase in holding {
                    let net = net_bytes(f, POP, dist, phase);
                    assert!(
                        net >= bytes,
                        "{name} dist {dist} phase {phase}: {net} live bytes left, tree needs {bytes}; its teardown ran inside the metered call"
                    );
                }
            }
            for f in [map_insert as Arm, set_insert] {
                assert_eq!(
                    net_bytes(f, POP, dist, 0),
                    0,
                    "phase 0 of an insert arm builds no tree and must leak nothing"
                );
            }
        }
    }

    /// The checksum of a full arm must differ from its setup-only phase (the
    /// loop did work) and must be identical across two calls (determinism).
    #[test]
    fn phases_differ_and_repeat() {
        for dist in 0..3 {
            for f in [map_insert, map_get, map_iterate, map_range, map_remove] {
                assert_ne!(f(500, dist, 0), f(500, dist, 1));
                assert_eq!(f(500, dist, 1), f(500, dist, 1));
            }
            for f in [set_insert, set_contains, set_iterate, set_range, set_remove] {
                assert_ne!(f(500, dist, 0), f(500, dist, 1));
                assert_eq!(f(500, dist, 1), f(500, dist, 1));
            }
        }
    }

    /// Every key exactly once, and the point probes are exactly half misses,
    /// none of which is present.
    #[test]
    fn population_and_probes_are_as_declared() {
        for dist in 0..3 {
            let (keys, mut rng) = population(2_000, dist);
            assert_eq!(keys.len(), 2_000);
            let uniq: BTreeSet<K> = keys.iter().copied().collect();
            assert_eq!(uniq.len(), 2_000, "duplicate key in dist {dist}");
            let m = build_map(&keys);
            let probes = hit_miss_probes(&keys, &mut rng, |k| m.contains_key(k));
            assert_eq!(probes.len(), 2_000);
            let hits = probes.iter().filter(|&&k| m.contains_key(k)).count();
            assert_eq!(hits, 1_000);
            assert_eq!(windows(&keys).len(), WINDOWS);
        }
    }
}
