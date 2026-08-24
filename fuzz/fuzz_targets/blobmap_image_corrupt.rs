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
        let len = map.len();

        // Differential save -> load roundtrip: any image that parses must
        // re-serialize and re-parse to an identical map (same entry count, and
        // the same payload + hot_meta for every key, generation included).
        let mut buf = Vec::new();
        if map.save_to_writer(&mut buf).is_ok() {
            let reloaded = ExpanseBlobMap::from_bytes_slice(&buf)
                .expect("re-parsing a freshly serialized image must succeed");
            assert_eq!(reloaded.len(), len, "len changed across save/load");
            for (key, _raw) in map.index().iter() {
                match (map.get(key), reloaded.get(key)) {
                    (Some((v0, m0)), Some((v1, m1))) => {
                        assert_eq!(v0.as_bytes(), v1.as_bytes(), "payload differs after roundtrip");
                        assert_eq!(m0, m1, "hot_meta differs after roundtrip");
                    }
                    (None, None) => {}
                    _ => panic!("entry presence differs across save/load"),
                }
            }
        }

        map.clear();
    }
});
