//! `JudyHS` as a fixed-capacity LRU index: insert, evict the oldest key,
//! re-insert evicted keys.
//!
//! zmap's response dedup (`lib/cachehash.c`) keeps its window this way:
//! `JudyHSGet` tests a key, `JudyHSIns` adds one, and `JudyHSDel` removes the
//! key the LRU evicts, after which that key can arrive and be inserted again.
//! Its key is `target_t {ip, port, status}`, 8 bytes with zero bytes in it,
//! so on libexpanse it runs on `ExpanseShortBytesMap`'s ordered word tries
//! (keys of at most 8 bytes); longer keys take the hashed path. Each case
//! below drives `Judy.h`'s four `JudyHS*` entry points through that pattern
//! and checks every call against a model: an `ordered` map of the live keys
//! and their values, plus the insertion-order queue the LRU evicts from.
//! The same sequence runs against a `dlopen`'d stock libjudy in the
//! differential oracle (`judyhs_lru_evict_reinsert_matches_stock_libjudy`,
//! `crates/expanse-capi/src/lib.rs`).
#![cfg(target_pointer_width = "64")]

use core::ffi::c_void;
use core::ptr::null_mut;
use expanse::{JudyHSDel, JudyHSFreeArray, JudyHSGet, JudyHSIns, Word};
use std::collections::{BTreeMap, VecDeque};

struct XorShift(u64);
impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

/// zmap's `target_t`: a 4-byte IPv4 address, a 2-byte port and a 2-byte
/// status, laid out as the struct is in memory. Addresses come from a narrow
/// range and the status is mostly zero, so most keys carry zero bytes.
fn target_t(i: u64) -> Vec<u8> {
    let ip = 0x0A00_0000u32 + (i / 3) as u32; // 10.0.0.0/8, low bytes vary
    let port = [80u16, 443, 0][(i % 3) as usize];
    let status = u16::from(i.is_multiple_of(7));
    let mut k = Vec::with_capacity(8);
    k.extend_from_slice(&ip.to_ne_bytes());
    k.extend_from_slice(&port.to_ne_bytes());
    k.extend_from_slice(&status.to_ne_bytes());
    k
}

/// Keys of 0 to 8 bytes that differ only in trailing zero bytes: `[b]`,
/// `[b, 0]`, … `[b, 0, 0, 0, 0, 0, 0, 0]` are eight distinct keys for one
/// `b`, and key 0 is the empty key. A short-key path that pads to a word and
/// loses the length would merge them.
fn zero_padded(i: u64) -> Vec<u8> {
    let Some(j) = i.checked_sub(1) else {
        return Vec::new();
    };
    let mut k = vec![0u8; 1 + (j % 8) as usize];
    k[0] = (j / 8) as u8;
    k
}

/// `target_t` padded to 16 bytes with zeros: the hashed path.
fn target_t_16(i: u64) -> Vec<u8> {
    let mut k = target_t(i);
    k.resize(16, 0);
    k
}

/// 40-byte keys, zero except for the last 8: the hashed path.
fn wide_40(i: u64) -> Vec<u8> {
    let mut k = vec![0u8; 40];
    k[32..].copy_from_slice(&i.wrapping_mul(0x9E37_79B9_7F4A_7C15).to_be_bytes());
    k
}

/// The value behind a slot pointer `JudyHSGet` or `JudyHSIns` returned, or
/// `None` if it is null. Every read of a slot goes through here, so the null
/// check and the dereference sit together.
///
/// # Safety
/// `p` is null or a live value slot, with no change to its array since the
/// call that returned it (`docs/COMPAT.md` D10).
unsafe fn read(p: *mut c_void) -> Option<Word> {
    assert_ne!(p as usize, usize::MAX, "PJERR returned");
    // SAFETY: per the caller contract; `as_ref` maps null to `None`.
    unsafe { p.cast::<Word>().as_ref().copied() }
}

/// Stores `v` in a slot `JudyHSIns` returned; panics on a null slot.
///
/// # Safety
/// As for [`read`].
unsafe fn write(p: *mut c_void, v: Word) {
    assert_ne!(p as usize, usize::MAX, "PJERR returned");
    // SAFETY: per the caller contract; `as_mut` maps null to `None`.
    let slot = unsafe { p.cast::<Word>().as_mut() }.expect("JudyHSIns returned null");
    *slot = v;
}

/// One LRU run: keys drawn from a pool of `pool` keys (so evicted keys
/// recur), a window of `capacity`, `steps` arrivals. Returns the number of
/// re-inserts of a previously evicted key, so a case can assert the pattern
/// it exists for actually happened.
fn run(key: fn(u64) -> Vec<u8>, capacity: usize, pool: u64, steps: usize, seed: u64) -> usize {
    let keys: Vec<Vec<u8>> = (0..pool).map(key).collect();
    {
        let mut distinct: Vec<&Vec<u8>> = keys.iter().collect();
        distinct.sort();
        distinct.dedup();
        assert_eq!(distinct.len(), keys.len(), "key generator repeats a key");
    }
    let mut rng = XorShift(seed | 1);
    let mut arr: *mut c_void = null_mut();
    let mut model: BTreeMap<Vec<u8>, Word> = BTreeMap::new();
    let mut order: VecDeque<Vec<u8>> = VecDeque::new();
    let mut evicted: BTreeMap<Vec<u8>, ()> = BTreeMap::new();
    let mut reinserts = 0usize;
    let hs = |k: &[u8]| (k.as_ptr().cast::<c_void>(), k.len() as Word);

    // SAFETY: every call passes a live (pointer, length) byte string and
    // the address of the array word; a slot pointer is used only before the
    // next call that changes the array (docs/COMPAT.md D10).
    unsafe {
        for step in 0..steps {
            let k = &keys[(rng.next() % pool) as usize];
            let (kp, kl) = hs(k);
            let got = read(JudyHSGet(arr, kp, kl));
            match model.get(k) {
                Some(&v) => {
                    assert_eq!(got, Some(v), "step {step}: live key {k:02x?}");
                    continue; // a hit: the window is unchanged
                }
                None => assert_eq!(got, None, "step {step}: absent key {k:02x?} found"),
            }
            if order.len() == capacity {
                let old = order.pop_front().expect("window is full");
                let (op, ol) = hs(&old);
                assert_eq!(
                    JudyHSDel(&raw mut arr, op, ol, null_mut()),
                    1,
                    "evict {old:02x?}"
                );
                assert!(
                    JudyHSGet(arr, op, ol).is_null(),
                    "evicted {old:02x?} still found"
                );
                assert_eq!(
                    JudyHSDel(&raw mut arr, op, ol, null_mut()),
                    0,
                    "re-delete {old:02x?}"
                );
                model.remove(&old);
                evicted.insert(old, ());
            }
            if evicted.remove(k).is_some() {
                reinserts += 1;
            }
            let s = JudyHSIns(&raw mut arr, kp, kl, null_mut());
            let fresh = read(s);
            assert_eq!(
                fresh,
                Some(0),
                "step {step}: re-inserted {k:02x?} kept an old value"
            );
            let v = (step as Word) << 1 | 1;
            write(s, v);
            model.insert(k.clone(), v);
            order.push_back(k.clone());

            // Every 1/8 of the run, check the whole pool against the model.
            if step % (steps / 8).max(1) == 0 {
                for p in &keys {
                    let (pp, pl) = hs(p);
                    let g = read(JudyHSGet(arr, pp, pl));
                    assert_eq!(g, model.get(p).copied(), "sweep {p:02x?}");
                }
            }
        }

        // Drain the window oldest first; the emptied array word is null.
        while let Some(old) = order.pop_front() {
            let (op, ol) = hs(&old);
            assert!(
                !arr.is_null(),
                "array word null with {} keys live",
                order.len() + 1
            );
            assert_eq!(
                JudyHSDel(&raw mut arr, op, ol, null_mut()),
                1,
                "drain {old:02x?}"
            );
        }
        assert!(arr.is_null(), "emptied array word is not null");

        // Refill from the evicted keys, then free the array whole.
        let refill: Vec<&Vec<u8>> = keys.iter().take(capacity).collect();
        for k in &refill {
            let (kp, kl) = hs(k);
            let s = JudyHSIns(&raw mut arr, kp, kl, null_mut());
            assert_eq!(read(s), Some(0), "refill {k:02x?} kept an old value");
            write(s, 7);
        }
        for k in &refill {
            let (kp, kl) = hs(k);
            assert_eq!(read(JudyHSGet(arr, kp, kl)), Some(7), "refill {k:02x?}");
        }
        assert!(
            JudyHSFreeArray(&raw mut arr, null_mut()) > 0,
            "FreeArray of a live array"
        );
        assert!(arr.is_null(), "FreeArray left the array word set");
        assert_eq!(
            JudyHSFreeArray(&raw mut arr, null_mut()),
            0,
            "FreeArray of null"
        );
    }
    reinserts
}

/// Capacities 1, 37 and `pool / 3`: a pool three times the window keeps a
/// steady share of arrivals landing on evicted keys.
fn case(key: fn(u64) -> Vec<u8>, pool: u64) {
    let wide = (pool / 3) as usize;
    for (capacity, seed) in [(1usize, 0x1672u64), (37, 0x1673), (wide, 0x1674)] {
        let reinserts = run(key, capacity, pool, 20_000, seed);
        assert!(
            reinserts > 0,
            "capacity {capacity}: no evicted key was re-inserted"
        );
    }
}

#[test]
fn lru_evict_reinsert_8_byte_target_t() {
    case(target_t, 3000);
}

#[test]
fn lru_evict_reinsert_short_keys_differing_in_trailing_zeros() {
    // The empty key plus 8 lengths x 256 first bytes: 2,049 distinct keys.
    case(zero_padded, 1 + 8 * 256);
}

#[test]
fn lru_evict_reinsert_16_byte_hashed() {
    case(target_t_16, 3000);
}

#[test]
fn lru_evict_reinsert_40_byte_hashed() {
    case(wide_40, 3000);
}
