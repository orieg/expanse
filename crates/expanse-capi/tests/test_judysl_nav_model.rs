//! `JudySLFirst` / `JudySLNext` / `JudySLLast` / `JudySLPrev` against a
//! `BTreeMap<Vec<u8>, Word>` model.
//!
//! The four searches are byte-lexicographic over NUL-free strings: `First`
//! is the smallest key `>=` the index, `Next` the smallest `>`, `Last` the
//! largest `<=` and `Prev` the largest `<`. The key set is built to reach
//! every place the engine's chunked walk can decide a match: keys that are
//! prefixes of one another, the empty key, keys whose length is a multiple
//! of the 8-byte chunk and one either side of it, keys stored in suffix
//! leaves next to keys that continue below the same chunk, long keys, and
//! the extreme byte values `0x01` and `0xFF`. Every probe is checked on all
//! four searches: the returned slot, the value in it, and the string written
//! back into the index buffer.
#![cfg(target_pointer_width = "64")]

use core::ffi::c_void;
use core::ptr::{NonNull, null_mut};
use expanse::{
    JError, JudySLFirst, JudySLFreeArray, JudySLIns, JudySLLast, JudySLNext, JudySLPrev, Word,
};
use std::collections::BTreeMap;
use std::ops::Bound::{Excluded, Included, Unbounded};

type NavSl = unsafe extern "C" fn(*const c_void, *mut u8, *mut JError) -> *mut c_void;

/// Longest key the generator produces, plus room for a probe one byte longer
/// and its NUL.
const MAX_KEY: usize = 300;
const BUF: usize = MAX_KEY + 2;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// A key over a four-byte alphabet, so random keys share long prefixes.
fn small_alphabet_key(rng: &mut Rng, max_len: u64) -> Vec<u8> {
    const ALPHABET: [u8; 4] = [0x01, b'a', b'b', 0xFF];
    let len = rng.below(max_len + 1) as usize;
    (0..len).map(|_| ALPHABET[rng.below(4) as usize]).collect()
}

fn key_set() -> Vec<Vec<u8>> {
    let mut keys: Vec<Vec<u8>> = vec![Vec::new()];
    // Every prefix of one string across three chunks: each is a prefix of
    // the next, and lengths 7, 8, 9, 15, 16 and 17 straddle chunk ends.
    let base = b"abcdefghijklmnopqrstuvwxyz0123";
    for n in 1..=base.len() {
        keys.push(base[..n].to_vec());
    }
    // Siblings below shared chunks, including a sibling in the last byte of
    // a chunk and in the first byte of the next.
    for k in [
        &b"abcdefgX"[..],
        b"abcdefgh\x01",
        b"abcdefgh\xFF",
        b"abcdefghZ",
        b"abcdefghijklmnoX",
        b"abcdefghijklmnop\x01\x01",
        b"\x01",
        b"\x01\x01\x01\x01\x01\x01\x01\x01",
        b"\xFF",
        b"\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF",
        b"/k/0000000000000000",
        b"/k/0000000000000001",
        b"/k/00000000000000010",
        b"/k/000000000000000",
    ] {
        keys.push(k.to_vec());
    }
    // Long keys sharing a long prefix, diverging at several depths.
    let long: Vec<u8> = (0..MAX_KEY).map(|i| b'a' + (i % 26) as u8).collect();
    for cut in [40, 64, 65, 128, 199, 255, MAX_KEY] {
        keys.push(long[..cut].to_vec());
        let mut fork = long[..cut].to_vec();
        if let Some(b) = fork.last_mut() {
            *b = b'~';
        }
        keys.push(fork);
    }
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for _ in 0..600 {
        keys.push(small_alphabet_key(&mut rng, 20));
    }
    keys
}

/// Probes around every stored key and some that sort between them.
fn probes(keys: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let mut out: Vec<Vec<u8>> = vec![Vec::new(), vec![0x01], vec![0xFF; 40]];
    for k in keys {
        out.push(k.clone());
        if let Some(&last) = k.last() {
            let mut shorter = k.clone();
            shorter.pop();
            out.push(shorter);
            for step in [-1i16, 1] {
                let b = i16::from(last) + step;
                if (1..=255).contains(&b) {
                    let mut moved = k.clone();
                    *moved.last_mut().expect("non-empty") = b as u8;
                    out.push(moved);
                }
            }
        }
        if k.len() < MAX_KEY {
            for tail in [0x01, b'm', 0xFF] {
                let mut longer = k.clone();
                longer.push(tail);
                out.push(longer);
            }
        }
    }
    let mut rng = Rng(0xD1B5_4A32_D192_ED03);
    for _ in 0..400 {
        out.push(small_alphabet_key(&mut rng, 22));
    }
    out
}

/// Runs `f` with `probe` in the index buffer; `(key written back, value)`
/// when it found one.
fn search(arr: *const c_void, f: NavSl, probe: &[u8]) -> Option<(Vec<u8>, Word)> {
    let mut buf = [0u8; BUF];
    buf[..probe.len()].copy_from_slice(probe);
    // SAFETY: `arr` is a live JudySL array; `buf` holds a NUL-terminated
    // index and has room for the longest stored key and its NUL.
    let slot = unsafe { f(arr, buf.as_mut_ptr(), null_mut()) };
    let slot = NonNull::new(slot.cast::<Word>())?;
    // SAFETY: a non-null return is a live value slot of `arr`.
    let value = unsafe { *slot.as_ref() };
    let len = buf.iter().position(|&b| b == 0).expect("NUL-terminated");
    Some((buf[..len].to_vec(), value))
}

#[test]
fn judysl_nav_matches_btreemap() {
    let keys = key_set();
    let mut model: BTreeMap<Vec<u8>, Word> = BTreeMap::new();
    let mut arr: *mut c_void = null_mut();
    for (i, k) in keys.iter().enumerate() {
        assert!(k.len() <= MAX_KEY && !k.contains(&0));
        let mut cstr = k.clone();
        cstr.push(0);
        // SAFETY: `cstr` is NUL-terminated; `arr` is a JudySL array word.
        let slot = unsafe { JudySLIns(&raw mut arr, cstr.as_ptr(), null_mut()) };
        let mut slot = NonNull::new(slot.cast::<Word>()).expect("insert slot");
        let value = 1000 + i as Word;
        // SAFETY: a fresh slot of `arr`, not aliased.
        unsafe { *slot.as_mut() = value };
        model.insert(k.clone(), value);
    }

    let mut checked = 0usize;
    for p in probes(&keys) {
        let want = [
            (
                "JudySLFirst",
                model.range::<[u8], _>((Included(&p[..]), Unbounded)).next(),
            ),
            (
                "JudySLNext",
                model.range::<[u8], _>((Excluded(&p[..]), Unbounded)).next(),
            ),
            (
                "JudySLLast",
                model
                    .range::<[u8], _>((Unbounded, Included(&p[..])))
                    .next_back(),
            ),
            (
                "JudySLPrev",
                model
                    .range::<[u8], _>((Unbounded, Excluded(&p[..])))
                    .next_back(),
            ),
        ];
        let fs: [NavSl; 4] = [JudySLFirst, JudySLNext, JudySLLast, JudySLPrev];
        for ((name, want), f) in want.into_iter().zip(fs) {
            let got = search(arr, f, &p);
            let want = want.map(|(k, &v)| (k.clone(), v));
            assert_eq!(got, want, "{name} at {p:x?}");
            checked += 1;
        }
    }
    assert!(checked > 10_000, "only {checked} searches checked");

    // Whole traversals, both directions, against the model's order.
    let mut fwd = Vec::new();
    let mut cur = search(arr, JudySLFirst, b"");
    while let Some((k, v)) = cur {
        cur = search(arr, JudySLNext, &k);
        fwd.push((k, v));
    }
    let all: Vec<(Vec<u8>, Word)> = model.iter().map(|(k, &v)| (k.clone(), v)).collect();
    assert_eq!(fwd, all);
    let mut back = Vec::new();
    let mut cur = search(arr, JudySLLast, &[0xFF; MAX_KEY + 1]);
    while let Some((k, v)) = cur {
        cur = search(arr, JudySLPrev, &k);
        back.push((k, v));
    }
    back.reverse();
    assert_eq!(back, all);

    // SAFETY: `arr` is a live JudySL array, not used again.
    unsafe { JudySLFreeArray(&raw mut arr, null_mut()) };
    assert!(arr.is_null());
    // A null array finds nothing.
    for f in [JudySLFirst, JudySLNext, JudySLLast, JudySLPrev] {
        assert_eq!(search(null_mut(), f, b"abc"), None);
    }
}
