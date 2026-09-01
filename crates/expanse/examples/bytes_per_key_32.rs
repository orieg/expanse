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
//! | `population` | 10k |
//! | `probes_and_reuse` | N/A (Memory) |
//! | `hit_rate` | N/A |
//! | `miss_gen_method` | N/A |
//! | `value_dereference` | `mem_used()` accounting |
//! | `measured_region` | Clean |
//! | `arm_symmetry` | Pure 32-bit census |
//! | `statistics` | Exact byte count |
//! | `verdict` | **PASS** `[verified: RUN (27019b23)]`: Deterministic 32-bit memory census. |

use expanse_trie::{ExpanseBlobMap32, ExpanseMap32, ExpanseSet32, Key32};

fn report(label: &str, mem: usize, n: usize) -> f64 {
    let bpk = mem as f64 / n as f64;
    println!("{label} (N = {n}): {mem} bytes ({bpk:.4} B/key)");
    bpk
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
    let bpk_sensor = report(
        "1. Clustered sensor timestamps",
        sensor_set.mem_used(),
        n_sensor,
    );
    assert!(
        bpk_sensor <= 1.00,
        "regression: clustered set B/key {bpk_sensor:.4} exceeded guard 1.00"
    );

    // 2. Sparse CAN-bus / Modbus identifiers (sparse 29-bit IDs).
    let n_can = 500;
    let mut can_set = ExpanseSet32::new();
    for i in 0..n_can {
        can_set.insert((i * 100_007) & 0x1FFF_FFFF);
    }
    let bpk_can = report(
        "2. Sparse 29-bit CAN IDs",
        can_set.mem_used(),
        n_can as usize,
    );
    assert!(
        bpk_can <= 20.0,
        "regression: sparse set B/key {bpk_can:.4} exceeded guard 20.0"
    );

    // 3. IPv4 /24 subnet routing table (map: prefix -> next-hop).
    let n_routes = 2_000;
    let mut route_map = ExpanseMap32::new();
    for i in 0..n_routes {
        let ip = (10 << 24) | ((i as Key32 / 256) << 16) | ((i as Key32 % 256) << 8);
        route_map.insert(ip, (i % 16) as u32);
    }
    let bpk_routes = report(
        "3. IPv4 subnet routing map",
        route_map.mem_used(),
        n_routes as usize,
    );
    assert!(
        bpk_routes <= 24.0,
        "regression: map B/key {bpk_routes:.4} exceeded guard 24.0"
    );

    // 4. Dense map (consecutive keys) — the map density best case.
    let n_dense = 10_000;
    let mut dense_map = ExpanseMap32::new();
    for i in 0..n_dense {
        dense_map.insert(i as Key32, (i & 0xFF) as u32);
    }
    let bpk_dense = report("4. Dense consecutive map", dense_map.mem_used(), n_dense);
    assert!(
        bpk_dense <= 12.0,
        "regression: dense map B/key {bpk_dense:.4} exceeded guard 12.0"
    );

    // 5. OTA firmware chunk checksums (blob map, inline <=3 byte payloads).
    let n_blocks = 1_000;
    let mut ota_map = ExpanseBlobMap32::new();
    for i in 0..n_blocks {
        // Small 3-byte inline block checksums
        ota_map
            .insert(i as Key32, &[0xAA, 0xBB, 0xCC], (i % 500) as u16)
            .unwrap();
    }
    println!(
        "5. OTA firmware inline checksums (N = {}): {} live records",
        n_blocks,
        ota_map.len()
    );

    println!("\nAll 32-bit memory-density regression guards held.");
}
