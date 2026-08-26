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

        // Resolve every stored slot through both the point-lookup and the
        // range-scan paths. With the wide-offset arena, an index entry may carry
        // an `ArenaShort` OR an `ArenaLong` locator; a crafted image can hold an
        // `ArenaLong` slot with an out-of-range chunk id / offset. Every such
        // slot must resolve to `None` (or valid bytes) with no panic or UB —
        // this drives the `get_blob_slice_long` chunk/offset math on hostile
        // input.
        for (key, _raw) in map.index().iter() {
            if let Some((view, _meta)) = map.get(key) {
                // Force a read of the borrowed payload bytes.
                let _ = view.as_bytes().iter().fold(0u8, |a, &b| a ^ b);
            }
        }
        // Differential: with an always-true predicate, `scan_filtered` (which
        // resolves each match from the slot the range walk already holds, #355)
        // must visit exactly what the point-lookup `get` path returns, in
        // ascending key order — same keys, same payload bytes, same hot_meta.
        let mut scanned: Vec<(u64, u32, Vec<u8>)> = Vec::new();
        map.scan_filtered(
            0..=u64::MAX,
            |_key, _meta| true,
            |key, view, meta| {
                scanned.push((key, meta, view.as_bytes().to_vec()));
                true
            },
        );
        let via_get: Vec<(u64, u32, Vec<u8>)> = map
            .index()
            .iter()
            .filter_map(|(key, _raw)| {
                map.get(key)
                    .map(|(view, meta)| (key, meta, view.as_bytes().to_vec()))
            })
            .collect();
        assert_eq!(
            scanned, via_get,
            "scan_filtered output diverged from the get() re-descent path"
        );

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
