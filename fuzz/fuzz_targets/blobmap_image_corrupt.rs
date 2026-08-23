#![no_main]

use libfuzzer_sys::fuzz_target;
use expanse_trie::blobmap::ExpanseBlobMap;

fuzz_target!(|data: &[u8]| {
    // Test feeding arbitrary mutated/corrupted byte streams to ExpanseBlobMap
    // It must either succeed on valid images or return Err cleanly without panics or UB.
    let _ = ExpanseBlobMap::from_bytes_slice(data);
});
