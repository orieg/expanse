//! Real measured memory density for the 32-bit Expanse trie
//! (`bytes_per_key_32`).
//!
//! Every figure below is the *real* allocated node/leaf byte total reported
//! by `mem_used()` on the actual digital trie (not a formula). The
//! assertions are conservative **regression guards** sitting above the
//! currently-measured values on this build — they exist to catch a density
//! regression, not to define the published figure.
//!
//! These values are deterministic byte accounting: machine-independent and
//! load-immune, so the quiet-host rule in `docs/BENCHMARKING.md` does not
//! apply to them. They are published in `docs/visualizer_data.json` and
//! recomputed from the engine by `tests/test_visualizer_sync.rs`, so the
//! published figure cannot drift from the code.
//!
//! # Workload shape
//!
//! | Property | Value |
//! |---|---|
//! | `workload_id` | `example_bytes_per_key_32` |
//! | `group` | 5 |
//! | `population` | 10k; 5k on the uniform-random arm; 4,096 keys per cell on the short-payload blob arms |
//! | `insertion_order` | generator draw order — the population is inserted as drawn, neither sorted nor shuffled |
//! | `probes_and_reuse` | N/A (Memory) |
//! | `hit_rate` | N/A |
//! | `miss_gen_method` | N/A (no probes; the uniform-random arm rejects duplicates on insert) |
//! | `value_dereference` | `mem_used()` accounting; requested heap bytes from a counting allocator on the blob arms, which have no `mem_used()` |
//! | `measured_region` | Clean |
//! | `arm_symmetry` | Pure 32-bit census; the short-payload blob arms differ only in `hot_meta` (0 against non-zero), with the same keys and payloads |
//! | `statistics` | Exact byte count |
//! | `verdict` | **PASS** `[verified: RUN (4733dca5)]`: Deterministic 32-bit memory census. The uniform-random arm is the measured source of the sparse constant in `scripts/embedded_envelope.py`. |

use expanse_trie::blobmap32::BlobMap32Error;
use expanse_trie::{ExpanseBlobMap32, ExpanseMap32, ExpanseSet32, Key32};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Counts the heap bytes live at any instant, as requested (the `Layout`
/// size, before any allocator rounding). `ExpanseBlobMap32` has no
/// `mem_used()`; this is the byte-exact instrument for its arena arms. It
/// only counts, so it changes no other arm.
struct CountingAlloc;
static LIVE: AtomicUsize = AtomicUsize::new(0);

// SAFETY: forwards every call to `System` unchanged and only adjusts a counter.
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller's contract is `System`'s.
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            LIVE.fetch_add(layout.size(), Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        // SAFETY: the caller's contract is `System`'s.
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller's contract is `System`'s.
        let p = unsafe { System.realloc(ptr, layout, new_size) };
        if !p.is_null() {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
            LIVE.fetch_add(new_size, Ordering::Relaxed);
        }
        p
    }
}

#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc;

/// The house PRNG: same kernel and seed as `bytes_per_key.rs`, so the 32-bit
/// and 64-bit censuses agree on what "uniform random" means. This generator
/// *defines* the sparse-random density constant in
/// `scripts/embedded_envelope.py` -- swapping it silently would change the
/// constant without changing the code it claims to measure (AGENTS.md
/// §8.10.2), so it must not be replaced without re-measuring.
struct XorShift(u64);
impl XorShift {
    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x as u32
    }
}

/// Measures, prints and enforces one arm. The guard is printed beside the
/// figure so `scripts/perf_report.py` reads the ceiling this harness actually
/// enforces rather than keeping its own copy of it -- a mirrored table drifts,
/// and matching arms to ceilings by name drifts silently.
fn report(label: &str, mem: usize, n: usize, guard: f64) {
    let bpk = mem as f64 / n as f64;
    println!("{label} (N = {n}): {mem} bytes ({bpk:.4} B/key, guard {guard:.2})");
    assert!(
        bpk <= guard,
        "regression: {label} B/key {bpk:.4} exceeded guard {guard:.2}"
    );
}

/// Short-payload blob arms (#1548): the cost of keeping `hot_meta` for a
/// payload of 3 bytes or fewer. With `hot_meta == 0` the payload sits in the
/// 4-byte slot word; with `hot_meta != 0` it takes one arena entry (an
/// `Option<Vec<u8>>` slot in the arena `Vec`, plus the payload's own heap
/// block when it is not empty), against the 12-bit slab-offset cap.
fn blob_short_payload_arms() {
    const N: usize = 4096; // the arena holds exactly this many live entries
    println!(
        "\nshort-payload blob map, hot_meta == 0 against hot_meta != 0 (N = {N} keys per cell)"
    );
    println!(
        "{:<4} {:>13} {:>13} {:>12} {:>13}",
        "len", "B/key hm=0", "B/key hm!=0", "delta B/key", "arena entries"
    );
    for len in 0..=3usize {
        let data = &b"\xA1\xB2\xC3"[..len];
        // Live requested bytes of each map, measured as the growth of the
        // counter across building it; the other arm's map is already dropped,
        // so the two arms never overlap.
        let mut bytes = [0usize; 2];
        let mut arena_entries = [0usize; 2];
        for (arm, hot) in [0u16, 1].into_iter().enumerate() {
            let before = LIVE.load(Ordering::Relaxed);
            let mut map = ExpanseBlobMap32::new();
            for k in 0..N as Key32 {
                map.insert(k, data, if hot == 0 { 0 } else { 1 + (k as u16 & 0x0FFE) })
                    .unwrap();
            }
            bytes[arm] = LIVE.load(Ordering::Relaxed) - before;
            for k in 0..N as Key32 {
                let (view, meta) = map.get(k).unwrap();
                assert_eq!(view.as_bytes(), data, "len {len} key {k}");
                if hot == 0 {
                    assert!(view.is_inline() && meta == 0, "len {len} key {k}");
                } else {
                    assert!(
                        view.is_arena() && meta == 1 + (k as u16 & 0x0FFE),
                        "len {len} key {k}"
                    );
                    arena_entries[arm] += 1;
                }
            }
            // Capacity: the metadata arm holds the arena's 4,096 entries, so
            // the next metadata-carrying short entry is refused and leaves
            // the map unchanged. On the zero arm no arena slot is in use and
            // the same insert succeeds.
            let r = map.insert(N as Key32, data, 1);
            if hot == 0 {
                assert_eq!(r, Ok(()), "len {len}: no arena slot is in use");
                assert_eq!(map.len(), N + 1);
            } else {
                assert_eq!(r, Err(BlobMap32Error::OffsetOverflow), "len {len}");
                assert_eq!(map.len(), N, "a refused insert changes nothing");
                // An overwrite reuses its entry's slot, and a removal returns
                // one to the freelist for the next insert.
                assert_eq!(map.insert(0, data, 2), Ok(()));
                assert!(map.remove(1));
                assert_eq!(map.insert(N as Key32, data, 1), Ok(()));
                assert_eq!(map.len(), N);
            }
        }
        // The arena entries are exactly the keys with metadata.
        assert_eq!(arena_entries, [0, N], "len {len}");
        let n = N as f64;
        println!(
            "{:<4} {:>13.2} {:>13.2} {:>12.2} {:>13}",
            len,
            bytes[0] as f64 / n,
            bytes[1] as f64 / n,
            (bytes[1] - bytes[0]) as f64 / n,
            arena_entries[1],
        );
        // Floor on the delta (exact request sizes, no allocator rounding):
        // each entry costs at least its `Option<Vec<u8>>` slot in the arena
        // `Vec`, plus the payload's heap block when non-empty.
        let slot = core::mem::size_of::<Option<Vec<u8>>>();
        assert!(
            bytes[1] - bytes[0] >= N * (slot + len),
            "len {len}: arena delta {} below the {} B/entry floor",
            bytes[1] - bytes[0],
            slot + len
        );
    }
    println!(
        "(B/key = requested heap bytes of the whole map per key; arena entries = keys read back as arena records)"
    );
}

fn main() {
    println!("==========================================================================");
    println!("Expanse 32-Bit Trie — Real Measured Memory Density (mem_used)");
    println!("==========================================================================");

    // 1. Clustered time-series / sensor index (consecutive timestamps).
    let n_sensor = 10_000;
    let mut sensor_set = ExpanseSet32::new();
    for i in 0..n_sensor {
        sensor_set.insert(1_700_000_000 + i as Key32);
    }
    report(
        "1. Clustered sensor timestamps",
        sensor_set.mem_used(),
        n_sensor,
        1.00,
    );

    // 2. Sparse CAN-bus / Modbus identifiers (sparse 29-bit IDs).
    let n_can = 500;
    let mut can_set = ExpanseSet32::new();
    for i in 0..n_can {
        can_set.insert((i * 100_007) & 0x1FFF_FFFF);
    }
    report(
        "2. Sparse 29-bit CAN IDs",
        can_set.mem_used(),
        n_can as usize,
        20.0,
    );

    // 3. IPv4 /24 subnet routing table (map: prefix -> next-hop).
    let n_routes = 2_000;
    let mut route_map = ExpanseMap32::new();
    for i in 0..n_routes {
        let ip = (10 << 24) | ((i as Key32 / 256) << 16) | ((i as Key32 % 256) << 8);
        route_map.insert(ip, (i % 16) as u32);
    }
    report(
        "3. IPv4 subnet routing map",
        route_map.mem_used(),
        n_routes as usize,
        24.0,
    );

    // 4. Dense map (consecutive keys) — the map density best case.
    let n_dense = 10_000;
    let mut dense_map = ExpanseMap32::new();
    for i in 0..n_dense {
        dense_map.insert(i as Key32, (i & 0xFF) as u32);
    }
    report(
        "4. Dense consecutive map",
        dense_map.mem_used(),
        n_dense,
        12.0,
    );

    // 5. Uniform-random 32-bit keys — the sparse worst case, and the arm
    // `scripts/embedded_envelope.py` reads its sparse constant from. Measured
    // at N = 5,000 because that is the population the Sparse Events envelope
    // row uses; the BLE rows extrapolate from it, as they always have.
    let n_uniform = 5_000usize;
    let mut rng = XorShift(0x0DDB_1A5E_5EED_0001);
    let mut uniform_map = ExpanseMap32::new();
    while uniform_map.len() < n_uniform {
        let k = rng.next_u32();
        uniform_map.insert(k, k);
    }
    report(
        "5. Uniform-random 32-bit keys",
        uniform_map.mem_used(),
        n_uniform,
        20.0,
    );

    // 6. OTA firmware chunk checksums (blob map, <=3 byte payloads). Each is
    // inserted with non-zero metadata, so each is an arena entry (#1548).
    let n_blocks = 1_000;
    let mut ota_map = ExpanseBlobMap32::new();
    for i in 0..n_blocks {
        // Small 3-byte block checksums
        ota_map
            .insert(i as Key32, &[0xAA, 0xBB, 0xCC], (i % 500) as u16)
            .unwrap();
    }
    println!(
        "6. OTA firmware checksums (N = {}): {} live records",
        n_blocks,
        ota_map.len()
    );

    blob_short_payload_arms();

    println!("\nAll 32-bit memory-density regression guards held.");
}
