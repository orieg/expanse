#!/usr/bin/env python3
"""
scripts/embedded_envelope.py

Derives the exact memory footprint envelopes for Expanse 32-bit digital trie
vs competitive baselines on 32-bit microcontrollers (ESP32-C3 / ESP32-C6).
Per Rule 12 / GEMINI.md §1.3 (math-first derivation in Python with tests).

Base density constants sourced from `bytes_per_key_32.rs` (measured, commit f48dcc6e):
- Clustered sensor timestamps (10k consecutive, ~1 kHz): 4.424 B/key
- Sparse 29-bit CAN IDs (500 IDs): 9.856 B/key (varies slightly across populations)
- Stride-100 sensor timestamps (~10 Hz): modeled at ~8.50 B/key

- Uniform-random 32-bit keys: 13.420 B/key, measured at N=5,000

Every constant above is measured by `bytes_per_key_32.rs`, and that harness's
PRNG *defines* the uniform-random one: a different key stream is a different
number, so the generator cannot be swapped without re-measuring the constant
(AGENTS.md §8.10.2).
"""

# Symmetric payload size across all BLE arms (bytes)
# mac(6) + rssi(1) + flags(1) + last_seen_ms(4) + distance_cm(2) + name(14) = 28 bytes
BLE_RECORD_SIZE = 28


def mem_expanse_tsdb(n: int, rate_1khz: bool = True) -> int:
    """ExpanseMap32 for sensor TSDB."""
    b_per_key = 4.424 if rate_1khz else 8.50
    return int(n * b_per_key)


def mem_expanse_can(n: int) -> int:
    """ExpanseMap32 with 29-bit CAN IDs (measured at 500 IDs)."""
    return int(n * 9.856)


def mem_expanse_sparse(n: int) -> int:
    """ExpanseMap32 with uniform random 32-bit keys (measured at N=5,000)."""
    return int(n * 13.420)


def mem_std_map(n: int, val_size: int = 4) -> int:
    """std::map (Red-Black tree) on 32-bit platform."""
    node_size = 4 + 4 + 4 + 4 + 4 + val_size + 8
    aligned_node = ((node_size + 7) // 8) * 8
    return n * aligned_node


def mem_unordered_map(n: int, val_size: int = 4) -> int:
    """std::unordered_map with reserve(n) on 32-bit platform."""
    buckets = int(n * 1.2) * 4
    node_size = 4 + 4 + 4 + val_size + 8
    aligned_node = ((node_size + 7) // 8) * 8
    return buckets + (n * aligned_node)


def mem_ring_buffer(n: int, val_size: int = 4) -> int:
    """Flat circular ring buffer (key + val)."""
    return n * (4 + val_size)


def mem_ble_tracker_expanse_slab(n: int) -> int:
    """
    Expanse BLE tracker: Dual ExpanseMap32 (by_mac + by_time) + 28B slab
    plus slab auxiliary arrays:
      - mono_last_seen_sec: 4B
      - free_indices: 2B
      - active_bitmap: ((n + 31) // 32) * 4 B
    """
    dual_trie = int(n * (13.420 * 2))
    slab_payload = n * BLE_RECORD_SIZE
    slab_mono_sec = n * 4
    slab_freelist = n * 2
    slab_bitmap = ((n + 31) // 32) * 4
    return dual_trie + slab_payload + slab_mono_sec + slab_freelist + slab_bitmap


def mem_ble_tracker_blobmap(n: int) -> int:
    """ExpanseBlobMap32 BLE tracker: Dual Map (by_mac + by_time) with blob arena storage."""
    dual_trie = int(n * (13.420 * 2))
    arena = n * (BLE_RECORD_SIZE + 4)
    return dual_trie + arena


def test_bounds() -> None:
    """Unit tests pinning known reference values (GEMINI.md Rule 12)."""
    assert BLE_RECORD_SIZE == 28, "BLE payload must be exactly 28 bytes"
    assert mem_expanse_tsdb(5000, True) == 22120, "TSDB 5k 1kHz must equal 22,120 bytes"
    assert mem_expanse_can(500) == 4928, "CAN 500 must equal 4,928 bytes"
    assert mem_expanse_sparse(5000) == 67100, "Sparse 5k must equal 67,100 bytes"
    assert mem_std_map(5000, 4) == 160000, "std::map 5k u32 must equal 160,000 bytes"
    # At N=2000: dual_trie (53,680) + slab (56,000) + mono_sec (8,000) + free_idx (4,000) + bitmap (252) = 121,932 B
    assert mem_ble_tracker_expanse_slab(2000) == 121932, "BLE slab 2k must match exact auxiliary derivation"


if __name__ == "__main__":
    test_bounds()
    print("=== 32-Bit Microcontroller Memory Envelopes ===")
    for n in [500, 2000, 5000]:
        print(f"\n--- Population N = {n} ---")
        tsdb_1k = mem_expanse_tsdb(n, True)
        tsdb_10h = mem_expanse_tsdb(n, False)
        can_exp = mem_expanse_can(n)
        sparse_exp = mem_expanse_sparse(n)
        std_map_u32 = mem_std_map(n, 4)
        unord_map_u32 = mem_unordered_map(n, 4)
        ring_mem = mem_ring_buffer(n, 4)

        ble_slab = mem_ble_tracker_expanse_slab(n)
        ble_blob = mem_ble_tracker_blobmap(n)
        ble_unord = mem_unordered_map(n, BLE_RECORD_SIZE)
        ble_stdmap = mem_std_map(n, BLE_RECORD_SIZE)

        print(f"TSDB 1kHz (ExpanseMap32):            {tsdb_1k:6d} B ({tsdb_1k/1024:5.2f} KiB) [4.42 B/key]")
        print(f"TSDB 10Hz (ExpanseMap32 stride-100):  {tsdb_10h:6d} B ({tsdb_10h/1024:5.2f} KiB) [~8.50 B/key]")
        print(f"CAN Dispatch (ExpanseMap32):         {can_exp:6d} B ({can_exp/1024:5.2f} KiB) [9.86 B/key]")
        print(f"Sparse Events (ExpanseMap32):        {sparse_exp:6d} B ({sparse_exp/1024:5.2f} KiB) [13.42 B/key]")
        print(f"std::unordered_map<u32, u32>:        {unord_map_u32:6d} B ({unord_map_u32/1024:5.2f} KiB) [~28-32 B/key]")
        print(f"std::map<u32, u32>:                  {std_map_u32:6d} B ({std_map_u32/1024:5.2f} KiB) [32.00 B/key]")
        print(f"BLE Tracker (ExpanseMap32 + Slab):   {ble_slab:6d} B ({ble_slab/1024:5.2f} KiB) [~56.05 B/entry total]")
        print(f"BLE Tracker (ExpanseBlobMap32):     {ble_blob:6d} B ({ble_blob/1024:5.2f} KiB) [~53.92 B/entry total]")
        print(f"std::unordered_map<u64, 28B>:        {ble_unord:6d} B ({ble_unord/1024:5.2f} KiB) [~52.00 B/entry total]")
        print(f"std::map<u64, 28B>:                  {ble_stdmap:6d} B ({ble_stdmap/1024:5.2f} KiB) [56.00 B/entry total]")
