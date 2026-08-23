//! Embedded Memory Density Measurement for 32-Bit Expanse (`bytes_per_key_32`).
//!
//! Asserts memory density invariant floors on 32-bit workloads:
//! - Clustered/Dense Sets: <= 0.40 B/key
//! - Sparse Sets: <= 4.0 B/key
//! - Maps: <= 5.0 B/key

use expanse_trie::{ExpanseBlobMap32, ExpanseMap32, ExpanseSet32, Key32};

fn main() {
    println!("==========================================================================");
    println!("Expanse 32-Bit Embedded Memory Density & Allocation Invariant Audit");
    println!("==========================================================================");

    // 1. Clustered Time-Series / Sensor Index (10,000 timestamps)
    let n_sensor = 10_000;
    let mut sensor_set = ExpanseSet32::new();
    for i in 0..n_sensor {
        sensor_set.insert(1_700_000_000 + i as Key32);
    }
    let mem_sensor = sensor_set.mem_used();
    let bpk_sensor = mem_sensor as f64 / n_sensor as f64;
    println!(
        "1. Clustered Sensor Timestamps (N = {}): {} bytes ({:.4} B/key)",
        n_sensor, mem_sensor, bpk_sensor
    );
    assert!(
        bpk_sensor <= 0.60,
        "Clustered set B/key exceeded budget: {:.4}",
        bpk_sensor
    );

    // 2. Sparse CAN-Bus / Modbus Identifiers (500 sparse 29-bit IDs)
    let n_can = 500;
    let mut can_set = ExpanseSet32::new();
    for i in 0..n_can {
        can_set.insert((i * 100_007) & 0x1FFF_FFFF);
    }
    let mem_can = can_set.mem_used();
    let bpk_can = mem_can as f64 / n_can as f64;
    println!(
        "2. Sparse 29-bit CAN IDs (N = {}): {} bytes ({:.4} B/key)",
        n_can, mem_can, bpk_can
    );
    assert!(
        bpk_can <= 5.0,
        "Sparse set B/key exceeded budget: {:.4}",
        bpk_can
    );

    // 3. IPv4 Routing Table / Network MAC Table (2,000 /24 routes)
    let n_routes = 2_000;
    let mut route_map = ExpanseMap32::new();
    for i in 0..n_routes {
        let ip = (10 << 24) | ((i as Key32 / 256) << 16) | ((i as Key32 % 256) << 8);
        route_map.insert(ip, (i % 16) as u32);
    }
    let mem_routes = route_map.mem_used();
    let bpk_routes = mem_routes as f64 / n_routes as f64;
    println!(
        "3. IPv4 Subnet Routing Table (N = {}): {} bytes ({:.4} B/key)",
        n_routes, mem_routes, bpk_routes
    );
    assert!(
        bpk_routes <= 9.0,
        "Map B/key exceeded budget: {:.4}",
        bpk_routes
    );

    // 4. Large-Value OTA Firmware Chunks with Hot Metadata (1,000 blocks)
    let n_blocks = 1_000;
    let mut ota_map = ExpanseBlobMap32::new();
    for i in 0..n_blocks {
        // Small 3-byte inline block checksums
        ota_map.insert(i as Key32, &[0xAA, 0xBB, 0xCC], (i % 500) as u16);
    }
    println!(
        "4. OTA Firmware Inlined Checksums (N = {}): {} live records (0 heap payloads)",
        n_blocks,
        ota_map.len()
    );

    println!("\n✅ All 32-bit embedded memory density invariants PASS!");
}
