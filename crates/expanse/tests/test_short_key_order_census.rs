//! Which byte order an 8-byte `ExpanseShortBytesMap` key is read in decides
//! how much memory its trie holds (#1681). This census builds the same key
//! set under both orders in a plain `ExpanseMap` and pins the direction and a
//! bound on each shape, the losing shape included, so the trade-off recorded
//! in the pre-registration cannot drift unnoticed.
//!
//! Deterministic: `mem_used` and `mem_held` are byte counts of the trie's own
//! allocator, fixed by the key set and its insertion order.
#![cfg(not(miri))]
#![cfg(all(feature = "std", target_pointer_width = "64"))]

use expanse_trie::ExpanseMap;

/// Bijective 32-bit mix, the generator of
/// `docs/benchmarks/zmap_comparison/scripts/cachehash_bench.c`.
fn mix32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

const PORTS: [u16; 4] = [80, 443, 8080, 8443];

/// zmap's dedup key: address and port in network order, status and padding 0.
fn zmap_key(ip: u32, port: u16) -> [u8; 8] {
    let i = ip.to_be_bytes();
    let p = port.to_be_bytes();
    [i[0], i[1], i[2], i[3], p[0], p[1], 0, 0]
}

/// Responders spread over the address space: the cachehash harness's keys,
/// one full 1,000,000-entry window of them.
fn zmap_sparse() -> Vec<[u8; 8]> {
    (0..1_000_000u32)
        .map(|i| {
            let ip = mix32(i.wrapping_add(4242u32.wrapping_mul(0x9e37_79b9)));
            zmap_key(ip, PORTS[(i & 3) as usize])
        })
        .collect()
}

/// Every target of one /15 on four ports, in a pseudo-random order: the
/// end-to-end rig's 524,288 keys.
fn zmap_dense() -> Vec<[u8; 8]> {
    (0..1u32 << 19)
        .map(|i| {
            let j = i.wrapping_mul(0x9E37_79B1) & 0x7_FFFF;
            zmap_key(0xC612_0000 | (j >> 2), PORTS[(j & 3) as usize])
        })
        .collect()
}

/// `(mem_used, mem_held)` of a trie holding `keys`, each read by `word`.
fn census(keys: &[[u8; 8]], word: fn([u8; 8]) -> u64) -> (usize, usize) {
    let mut m = ExpanseMap::new();
    for k in keys {
        m.insert(word(*k), 1);
    }
    assert_eq!(m.len(), keys.len() as u64, "the key set has no duplicate");
    (m.mem_used(), m.mem_held())
}

/// Held bytes under the little-endian read, as a percentage of the
/// big-endian read's, rounded down. Prints both censuses, so
/// `--nocapture` reproduces the byte counts the #1681 pre-registration cites.
fn held_percent(shape: &str, keys: &[[u8; 8]]) -> usize {
    let (be_used, be) = census(keys, u64::from_be_bytes);
    let (le_used, le) = census(keys, u64::from_le_bytes);
    println!(
        "{shape}: {} keys; big-endian used {be_used} held {be}; little-endian used {le_used} held {le}",
        keys.len()
    );
    le * 100 / be
}

/// Held bytes at the end of the cachehash harness's eviction workload: 4,194,304
/// keys through a 1,000,000-entry window, the oldest key removed before each
/// insert once the window is full.
fn window_held(word: fn([u8; 8]) -> u64) -> usize {
    let mut m = ExpanseMap::new();
    let mut window = std::collections::VecDeque::with_capacity(1_000_000);
    for i in 0..1u32 << 22 {
        let ip = mix32(i.wrapping_add(4242u32.wrapping_mul(0x9e37_79b9)));
        let w = word(zmap_key(ip, PORTS[(i & 3) as usize]));
        if window.len() == 1_000_000 {
            let oldest = window.pop_front().expect("a full window");
            assert_eq!(m.remove(oldest), Some(1));
        }
        m.insert(w, 1);
        window.push_back(w);
    }
    assert_eq!(m.len(), 1_000_000);
    m.mem_held()
}

#[test]
fn zmap_keys_hold_less_little_endian() {
    let sparse = held_percent("zmap sparse", &zmap_sparse());
    assert!(sparse <= 85, "sparse zmap keys: {sparse}% of big-endian");
    let dense = held_percent("zmap dense", &zmap_dense());
    assert!(dense <= 99, "dense zmap keys: {dense}% of big-endian");
}

#[test]
fn zmap_window_holds_less_little_endian_through_eviction() {
    let (be, le) = (
        window_held(u64::from_be_bytes),
        window_held(u64::from_le_bytes),
    );
    println!("zmap window after eviction: big-endian held {be}; little-endian held {le}");
    let pct = le * 100 / be;
    assert!(pct <= 90, "window after eviction: {pct}% of big-endian");
}

#[test]
fn native_integers_hold_less_little_endian() {
    let keys: Vec<[u8; 8]> = (0..1_000_000u64).map(u64::to_le_bytes).collect();
    let pct = held_percent("native integers", &keys);
    assert!(
        pct <= 35,
        "sequential native integers: {pct}% of big-endian"
    );
}

/// The expected loss: 8 bytes of ASCII decimal text hold more than twice as
/// much little-endian. Pinned so that a change which removes the loss, or
/// one that deepens it, is seen.
#[test]
fn ascii_text_holds_more_little_endian() {
    let keys: Vec<[u8; 8]> = (0..1_000_000u32)
        .map(|i| format!("{i:08}").as_bytes().try_into().unwrap())
        .collect();
    let pct = held_percent("ASCII decimal", &keys);
    assert!(
        (200..=280).contains(&pct),
        "ASCII decimal keys: {pct}% of big-endian"
    );
}
