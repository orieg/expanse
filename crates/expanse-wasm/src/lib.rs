mod blobmap;
mod bytesmap;
mod map;
mod set;
mod strmap;

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn init_panic_hook() {
    // Optional panic hook initialization
}

/// Standalone WebAssembly Memory64 smoke test entry point.
///
/// Runs core 64-bit ExpanseMap and ExpanseSet operations inside WebAssembly Memory64
/// and returns a verification checksum, confirming that 64-bit raw pointer operations,
/// 16-byte Edge descriptors, and 8-level digital trie descent function correctly.
#[unsafe(no_mangle)]
pub extern "C" fn expanse_wasm64_smoke_test() -> u64 {
    #[cfg(target_pointer_width = "64")]
    {
        let mut map = expanse_trie::ExpanseMap::new();
        let mut set = expanse_trie::ExpanseSet::new();

        // 1. Basic small keys
        map.insert(42, 1042);
        map.insert(100, 2100);
        set.insert(42);
        set.insert(100);

        // 2. Large 64-bit keys (> 4GB, exercising 64-bit key expanse)
        let large_key1 = 0x1_0000_0000u64 | 0xDEAD_BEEFu64;
        let large_key2 = 0x8000_0000_0000_0000u64 | 0xCAFE_BABEu64;
        map.insert(large_key1, 0x1122_3344_5566_7788u64);
        map.insert(large_key2, 0x99AA_BBCC_DDEE_FF00u64);
        set.insert(large_key1);
        set.insert(large_key2);

        // 3. Multi-key population to trigger trie node transitions (Immediate -> Linear -> Bitmap)
        for i in 1000u64..1500u64 {
            let k = (i << 32) | (i * 37);
            let v = i.wrapping_mul(0x517C_C1B7_2722_0A95);
            map.insert(k, v);
            set.insert(k);
        }

        // Verify map lookups
        let v1 = map.get(42).unwrap_or(0);
        let v2 = map.get(100).unwrap_or(0);
        let v3 = map.get(large_key1).unwrap_or(0);
        let v4 = map.get(large_key2).unwrap_or(0);

        // Verify set lookups
        let s1 = if set.contains(42) { 1u64 } else { 0 };
        let s2 = if set.contains(100) { 1u64 } else { 0 };
        let s3 = if set.contains(large_key1) { 1u64 } else { 0 };
        let s4 = if set.contains(large_key2) { 1u64 } else { 0 };
        let s_miss = if set.contains(999) { 100u64 } else { 0 };

        // Verify ordered iteration & navigation
        let (first_k, _) = map.first().unwrap_or((0, 0));
        let (last_k, _) = map.last().unwrap_or((0, 0));
        let next_k = map.next_after(42).map(|(k, _)| k).unwrap_or(0);
        let prev_k = map.prev_before(100).map(|(k, _)| k).unwrap_or(0);

        // Verify set range counting & rank
        let range_cnt = set.count_range(0..=0xFFFF_FFFF_FFFF_FFFF);
        let rank_42 = set.count_below(43);

        // Verify removal
        let r1 = map.remove(42).unwrap_or(0);
        let r2 = if set.remove(42) { 1u64 } else { 0 };

        // Verify remaining size
        let m_len = map.len();
        let s_len = set.len();

        let mut acc = v1
            ^ v2
            ^ v3
            ^ v4
            ^ s1
            ^ s2
            ^ s3
            ^ s4
            ^ s_miss
            ^ r1
            ^ r2
            ^ m_len
            ^ s_len
            ^ first_k
            ^ last_k
            ^ next_k
            ^ prev_k
            ^ range_cnt
            ^ rank_42;

        // Sample scan checksum
        for i in 1000u64..1500u64 {
            let k = (i << 32) | (i * 37);
            if let Some(val) = map.get(k) {
                acc ^= val;
            }
        }

        acc
    }
    #[cfg(not(target_pointer_width = "64"))]
    {
        0
    }
}
