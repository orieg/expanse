#![no_main]

use libfuzzer_sys::fuzz_target;
use expanse_trie::blobmap::ExpanseBlobMap;

fuzz_target!(|data: &[u8]| {
    // Test feeding arbitrary mutated/corrupted byte streams to ExpanseBlobMap
    // It must either succeed on valid images or return Err cleanly without panics or UB.
    if let Ok(mut map) = ExpanseBlobMap::from_bytes_slice(data) {
        let _ = map.get(0);
        let _ = map.get(1);
        let _ = map.get(u64::MAX);
        let _ = map.contains_key(0);
        let _ = map.len();
        map.clear();
    }
});
