//! Memory-footprint measurement per `docs/BENCHMARKING.md`: bytes/key at
//! population checkpoints, per key-distribution class, from the engine's
//! own byte-exact allocation accounting (`mem_used`). Deterministic — no
//! timing involved, so it is immune to machine load.
//!
//! Run: `cargo run --release -p expanse-trie --example bytes_per_key`
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `example_bytes_per_key` |
//! | `group` | 5 |
//! | `population` | 1k to 1M census; the `random` gate also samples 2M; 131,072 (2^17) keys per cell on the short-payload blob arms |
//! | `insertion_order` | generator draw order — the population is inserted as drawn, neither sorted nor shuffled |
//! | `probes_and_reuse` | N/A (Memory) |
//! | `hit_rate` | N/A |
//! | `miss_gen_method` | N/A |
//! | `value_dereference` | `mem_used()` accounting |
//! | `measured_region` | Clean |
//! | `arm_symmetry` | Pure 64-bit census; the short-payload blob arms differ only in `hot_meta` (0 against non-zero), with the same keys and payloads |
//! | `statistics` | Exact byte count |
//! | `verdict` | **PASS** `[verified: RUN (6c63826a)]`: Deterministic memory density census. |

use expanse_trie::blobmap::{ARENA_ALIGN, ExpanseBlobMap};
use expanse_trie::map::ExpanseMap;
use expanse_trie::set::ExpanseSet;
use expanse_trie::sync::SyncExpanseBlobMap;
use expanse_trie::types::LEAF_CAP;

#[path = "../benches/workload_common/mod.rs"]
mod workload_common;
use workload_common::XorShift;

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
        // Runs of 4096 span two key bytes: exercises branch-targeted
        // narrow pointers (divergence level 2), not just leaf-targeted.
        "clustered-wide" => {
            let mut base = 0;
            for i in 0..n as u64 {
                if i % 4096 == 0 {
                    base = rng.next() & !0xFFF;
                }
                out.push(base + (i % 4096));
            }
        }
        _ => unreachable!(),
    }
    out
}

/// Short-payload blob arms (#1548): the cost of keeping `hot_meta` for a
/// payload of 7 bytes or fewer. With `hot_meta == 0` the payload sits in the
/// value-slot word and uses no arena bytes; with `hot_meta != 0` it is an
/// `ArenaMeta` record, one 8-byte `BlobRecordHeader` plus the payload,
/// rounded up to `ARENA_ALIGN`. Every assertion is exact byte accounting.
const BLOB_N: usize = 1 << 17;

fn blob_payload(k: u64, len: usize) -> Vec<u8> {
    (0..len).map(|i| (k >> (8 * i)) as u8 | 0x80).collect()
}

/// Metadata in `1..=0xFFFF`: non-zero, inside the 24-bit field.
fn blob_meta(k: u64) -> u32 {
    1 + (k as u32 & 0xFFFF)
}

/// The arena figures the assertions are made on.
struct ArenaFigures {
    mem_used: usize,
    cursor_sum: usize,
    live_bytes: usize,
    index_mem: usize,
    total_mem: usize,
}

fn arena_figures(map: &ExpanseBlobMap) -> ArenaFigures {
    let arena = map.arena();
    ArenaFigures {
        mem_used: arena.mem_used(),
        cursor_sum: arena.chunks().iter().map(|c| c.cursor()).sum(),
        live_bytes: arena.live_bytes(),
        index_mem: map.index().mem_used(),
        total_mem: map.mem_used(),
    }
}

fn blob_short_payload_arms() {
    println!(
        "\nshort-payload blob maps, hot_meta == 0 against hot_meta != 0 (N = {BLOB_N} keys per cell)"
    );
    println!(
        "{:<8} {:>4} {:>13} {:>13} {:>12} {:>13}",
        "map", "len", "B/key hm=0", "B/key hm!=0", "delta B/key", "record B/key"
    );
    let mut failures = Vec::new();
    let chunk = ExpanseBlobMap::new().arena().chunk_size();
    for len in [0usize, 1, 3, 7] {
        // Record = 8-byte header + payload, rounded up to ARENA_ALIGN.
        let record = (8 + len).next_multiple_of(ARENA_ALIGN);
        let want_cursor = BLOB_N * record;
        let want_mem = want_cursor.div_ceil(chunk) * chunk;
        let want_live = BLOB_N * (8 + len);

        let mut plain = [ExpanseBlobMap::new(), ExpanseBlobMap::new()];
        let sync = [SyncExpanseBlobMap::new(), SyncExpanseBlobMap::new()];
        for k in 0..BLOB_N as u64 {
            let data = blob_payload(k, len);
            plain[0].insert(k, &data, 0).unwrap();
            plain[1].insert(k, &data, blob_meta(k)).unwrap();
            sync[0].insert(k, &data, 0).unwrap();
            sync[1].insert(k, &data, blob_meta(k)).unwrap();
        }
        // Reads: metadata is kept on the arm that passed it, and the payload
        // is the same on both.
        for k in [0u64, 1, BLOB_N as u64 / 2, BLOB_N as u64 - 1] {
            let data = blob_payload(k, len);
            let (p0, m0) = plain[0].get(k).unwrap();
            let (p1, m1) = plain[1].get(k).unwrap();
            assert_eq!((p0.as_ref(), m0), (&data[..], 0), "len {len} key {k}");
            assert_eq!(
                (p1.as_ref(), m1),
                (&data[..], blob_meta(k)),
                "len {len} key {k}"
            );
        }

        let a0 = arena_figures(&plain[0]);
        let a1 = arena_figures(&plain[1]);
        // Same keys and 8-byte slot words: the index costs the same, so the
        // whole difference between the arms is the arena.
        if a0.index_mem != a1.index_mem {
            failures.push(format!(
                "plain len {len}: index bytes differ {} vs {}",
                a0.index_mem, a1.index_mem
            ));
        }
        if (a0.mem_used, a0.cursor_sum, a0.live_bytes) != (0, 0, 0) {
            failures.push(format!(
                "plain len {len}: hot_meta == 0 arm used arena bytes {:?}",
                (a0.mem_used, a0.cursor_sum, a0.live_bytes)
            ));
        }
        if (a1.mem_used, a1.cursor_sum, a1.live_bytes) != (want_mem, want_cursor, want_live) {
            failures.push(format!(
                "plain len {len}: hot_meta != 0 arena (mem_used, cursor, live) {:?} != {:?}",
                (a1.mem_used, a1.cursor_sum, a1.live_bytes),
                (want_mem, want_cursor, want_live)
            ));
        }
        if a1.total_mem - a0.total_mem != a1.mem_used {
            failures.push(format!(
                "plain len {len}: mem_used delta {} != arena {}",
                a1.total_mem - a0.total_mem,
                a1.mem_used
            ));
        }
        let n = BLOB_N as f64;
        println!(
            "{:<8} {:>4} {:>13.2} {:>13.2} {:>12.2} {:>13.2}",
            "plain",
            len,
            a0.total_mem as f64 / n,
            a1.total_mem as f64 / n,
            (a1.total_mem - a0.total_mem) as f64 / n,
            a1.cursor_sum as f64 / n,
        );

        // The shared map folds its per-writer arenas under `with_locked`.
        // Its chunk cursors are not a byte count (a writer's private chunk is
        // granted whole, cursor = capacity), so the exact figure is
        // `live_bytes`. Chunk bytes are the exact multiple of `chunk` that
        // holds the records, plus at most one more chunk: the first
        // `ROOT_LEAF_CAP` keys take the shared allocation path and open a
        // chunk of their own beside the writer's private one.
        let s: Vec<ArenaFigures> = sync.iter().map(|m| m.with_locked(arena_figures)).collect();
        let sync_mem = [sync[0].mem_used(), sync[1].mem_used()];
        if (s[0].mem_used, s[0].live_bytes) != (0, 0) {
            failures.push(format!(
                "sync len {len}: hot_meta == 0 arm used arena bytes"
            ));
        }
        if s[1].live_bytes != want_live {
            failures.push(format!(
                "sync len {len}: hot_meta != 0 live bytes {} != {want_live}",
                s[1].live_bytes
            ));
        }
        if !s[1].mem_used.is_multiple_of(chunk)
            || !(want_mem..=want_mem + chunk).contains(&s[1].mem_used)
        {
            failures.push(format!(
                "sync len {len}: hot_meta != 0 arena chunk bytes {} not in {want_mem}..={}",
                s[1].mem_used,
                want_mem + chunk
            ));
        }
        if sync_mem[1] - sync_mem[0] != s[1].mem_used {
            failures.push(format!(
                "sync len {len}: mem_used delta {} != arena {}",
                sync_mem[1] - sync_mem[0],
                s[1].mem_used
            ));
        }
        println!(
            "{:<8} {:>4} {:>13.2} {:>13.2} {:>12.2} {:>13.2}",
            "sync",
            len,
            sync_mem[0] as f64 / n,
            sync_mem[1] as f64 / n,
            (sync_mem[1] - sync_mem[0]) as f64 / n,
            s[1].live_bytes as f64 / n,
        );
    }
    println!(
        "(record B/key: plain = ArenaMeta bytes written per key, 8-byte header + payload rounded \
         to ARENA_ALIGN = {ARENA_ALIGN}; sync = live bytes per key, 8-byte header + payload, \
         unrounded. delta B/key is the mem_used difference, counted in whole {chunk}-byte chunks)"
    );
    if !failures.is_empty() {
        for f in &failures {
            println!("MEMORY BUDGET EXCEEDED: blob short payload: {f}");
        }
        std::process::exit(1);
    }
    println!("blob short-payload accounting: exact");
}

fn main() {
    println!("bytes/key by distribution and population (set flavor / map flavor)");
    println!("target from docs/ARCHITECTURE.md: < 9.5 B/key dense+clustered (set)\n");
    println!(
        "{:<15} {:>10} {:>14} {:>14}",
        "dist", "pop", "set B/key", "map B/key"
    );
    for dist in [
        "sequential",
        "random",
        "clustered",
        "clustered-wide",
        "sparse",
    ] {
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
                "{:<15} {:>10} {:>14.2} {:>14.2}",
                dist,
                pop,
                set.mem_used() as f64 / sl as f64,
                map.mem_used() as f64 / ml as f64,
            );
        }
    }
    println!("\n(map B/key includes the 8-byte value per key)");

    // Regression gate (CI `memory-budget` job). These are deterministic
    // allocator-accounting numbers — exact regardless of machine load —
    // so unlike the timing tables they can gate a build. Ceilings sit a
    // little above the measured values in docs/BENCHMARKING.md: they
    // catch structural regressions (a compression path stopping firing),
    // not the last few percent. Raise one deliberately, with the
    // BENCHMARKING.md row updated in the same commit.
    //
    // The `random` ceiling is population-specific, and not in the way the
    // numbers suggest. Per-key cost on uniform keys is a sawtooth in expanse
    // occupancy λ = N / 2^16 — the mean population of a 2-byte-prefix
    // expanse — with its tooth at λ ≈ LEAF_CAP, where the linear leaf
    // cascades into a branch of single-key immediates (ARCHITECTURE.md §3.5).
    // N = 1_000_000 is λ = 15.26, 48% of LEAF_CAP, in the trough of that
    // curve: 8.21 B/key against a 9.00 ceiling reads as 10% of headroom in
    // bytes, but the real headroom is in N, and under 2× of it (8.75 B/key at
    // 1.6M, 10.72 at 1.8M). The same generator at N = 2_000_000 (λ = 30.5,
    // 95% of LEAF_CAP, 35% of expanses cascaded) measures 13.74 set / 19.78
    // map and breaches both ceilings with no code
    // change. Changing this row's N therefore means re-deriving its ceiling
    // from the curve, never raising the number until the gate is green. The
    // other four rows have occupancy fixed by their generators and do not
    // move with N. Keyspace width is the same knob: masking one top key bit
    // halves 2^16 and is exactly a doubling of N.
    //
    // The gate therefore samples `random` at TWO densities. N = 2_000_000
    // (λ = 30.5) sits in the cascade's mixture regime, where 35% of the
    // 2-byte expanses have overflowed LEAF_CAP and pay one 16-byte immediate
    // per key while the rest are still packed Leaf6 nodes. It is the
    // steepest part of the curve, which is the point: a regression in Leaf6
    // packing, in cap_class rounding or in the cost of the cascaded branch's
    // single-key children moves this cell and leaves the 1M cell alone. Its
    // ceilings follow the same policy as the others — a little above the
    // measured 13.74 set / 19.78 map, enough to catch a compression path
    // stopping firing (a fully cascaded expanse costs 17–21 B/key), not the
    // last few percent. The two cells are two points on one curve, not a
    // before/after; compare each only against its own history.
    let budget: &[(&str, usize, f64, f64)] = &[
        // dist, pop, set ceiling, map ceiling
        ("sequential", 1_000_000, 0.10, 9.00),
        ("clustered", 1_000_000, 0.50, 9.00),
        ("clustered-wide", 1_000_000, 0.30, 9.00),
        ("random", 1_000_000, 9.00, 18.00),
        ("random", 2_000_000, 15.00, 21.00),
        ("sparse", 1_000_000, 17.00, 17.00),
    ];
    let mut over = Vec::new();
    for &(dist, pop, set_max, map_max) in budget {
        if dist == "random" {
            // Print the density the ceiling was calibrated at, so a change to
            // `pop` is visible as a move along the curve in the job log.
            let lambda = pop as f64 / 65_536.0;
            println!(
                "random gate density: N = {pop}, λ = N / 2^16 = {lambda:.2} \
                 ({:.0}% of LEAF_CAP = {LEAF_CAP}); the ceiling is valid at this \
                 λ only — see docs/ARCHITECTURE.md §3.5",
                100.0 * lambda / LEAF_CAP as f64
            );
        }
        let ks = keys(dist, pop);
        let mut set = ExpanseSet::new();
        let mut map = ExpanseMap::new();
        for &k in &ks {
            set.insert(k);
            map.insert(k, !k);
        }
        let sb = set.mem_used() as f64 / set.len().max(1) as f64;
        let mb = map.mem_used() as f64 / map.len().max(1) as f64;
        if sb > set_max {
            over.push(format!("{dist} set {sb:.2} > {set_max:.2} B/key"));
        }
        if mb > map_max {
            over.push(format!("{dist} map {mb:.2} > {map_max:.2} B/key"));
        }
    }
    blob_short_payload_arms();
    if over.is_empty() {
        println!("memory budget: all distributions within ceilings");
    } else {
        for line in &over {
            println!("MEMORY BUDGET EXCEEDED: {line}");
        }
        std::process::exit(1);
    }
}
