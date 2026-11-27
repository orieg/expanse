//! The engine agrees with `scripts/dns_name_bounds.py` (#1800, AGENTS.md §8.8
//! commit 1).
//!
//! That script derives, for a set of DNS names, the string map's node count,
//! suffix-leaf count and bytes, and a lower bound on `mem_used()`. A bound that
//! only a Python model agrees with is not a bound on the engine, so this file
//! rebuilds the script's synthetic names and both of its key encodings, loads
//! them into the real containers and asserts the script's pinned constants
//! against `stats()`. A change to the chunking, the suffix layout or the root
//! leaf's size classes fails here and in the script's self-test together.
//!
//! Deterministic integer accounting, so these are exact equalities (AGENTS.md
//! §8.4 limits hard assertions to exactly this).
//!
//! Not built under Miri: it is byte accounting over safe calls that the
//! string map's own Miri tests already walk, and the 2,000-name cells would
//! only lengthen a nightly shard.
#![cfg(not(miri))]
#![cfg(target_pointer_width = "64")]

use expanse_trie::ExpanseOrderedBytesMap;
use expanse_trie::strmap::{ExpanseStrMap, NulFreeStr};

/// `scripts/dns_name_bounds.py::NAME_SEED`.
const NAME_SEED: u64 = 0x0D05_5EED_0000_0001;
const TLDS: [&[u8]; 6] = [b"com", b"net", b"org", b"io", b"de", b"uk"];
const HOSTS: [&[u8]; 3] = [b"www", b"mail", b"ns1"];
const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";

/// `scripts/patricia_envelope.py::xorshift`.
struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// `scripts/dns_name_bounds.py::gen_names`: names as labels in wire order.
fn gen_names(n: usize) -> Vec<Vec<Vec<u8>>> {
    let mut rng = XorShift(NAME_SEED);
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<Vec<Vec<u8>>> = Vec::new();
    while out.len() < n {
        let r = rng.next();
        let tld = TLDS[(r % TLDS.len() as u64) as usize].to_vec();
        let length = 3 + (r >> 8) % 14;
        let hosts = ((r >> 16) % 4) as usize;
        let sld: Vec<u8> = (0..length)
            .map(|_| ALPHABET[(rng.next() % ALPHABET.len() as u64) as usize])
            .collect();
        if !seen.insert((sld.clone(), tld.clone())) {
            continue;
        }
        out.push(vec![sld.clone(), tld.clone()]);
        for h in &HOSTS[..hosts] {
            out.push(vec![h.to_vec(), sld.clone(), tld.clone()]);
        }
    }
    out.truncate(n);
    out
}

/// `key_nul`: reversed lowercased labels, each 0x00-terminated, label octets
/// 0x00 and 0x01 escaped.
fn key_nul(labels: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    for label in labels.iter().rev() {
        for &b in label {
            match b.to_ascii_lowercase() {
                0 => out.extend_from_slice(&[1, 1]),
                1 => out.extend_from_slice(&[1, 2]),
                b => out.push(b),
            }
        }
        out.push(0);
    }
    out
}

/// `key_direct`: the NUL-free form, 0x01-terminated labels, label octets
/// 0x00..=0x02 escaped.
fn key_direct(labels: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    for label in labels.iter().rev() {
        for &b in label {
            match b.to_ascii_lowercase() {
                b @ 0..=2 => out.extend_from_slice(&[2, b + 2]),
                b => out.push(b),
            }
        }
        out.push(1);
    }
    out
}

/// One row of `scripts/dns_name_bounds.py::PINS`.
struct Pin {
    n: usize,
    shells: usize,
    suffixes: usize,
    suffix_bytes: usize,
    leaf_submap_bytes: usize,
    over_submaps: usize,
}

/// `SHELL_ACCOUNTED` in the script.
const SHELL_ACCOUNTED: usize = 48;

const DIRECT: [Pin; 2] = [
    Pin {
        n: 24,
        shells: 7,
        suffixes: 11,
        suffix_bytes: 336,
        leaf_submap_bytes: 496,
        over_submaps: 0,
    },
    Pin {
        n: 2000,
        shells: 829,
        suffixes: 785,
        suffix_bytes: 21840,
        leaf_submap_bytes: 34432,
        over_submaps: 1,
    },
];

const ORDERED_BYTES_STORED: [Pin; 2] = [
    Pin {
        n: 24,
        shells: 10,
        suffixes: 7,
        suffix_bytes: 208,
        leaf_submap_bytes: 576,
        over_submaps: 0,
    },
    Pin {
        n: 2000,
        shells: 934,
        suffixes: 1046,
        suffix_bytes: 30720,
        leaf_submap_bytes: 37088,
        over_submaps: 1,
    },
];

fn check(map: &ExpanseStrMap, pin: &Pin, what: &str) {
    let stats = map.stats();
    let n = pin.n;
    assert_eq!(map.len(), n as u64, "{what} n={n}: population");
    assert_eq!(stats.str_nodes, pin.shells, "{what} n={n}: shells");
    assert_eq!(stats.suffix_leaves, pin.suffixes, "{what} n={n}: suffixes");
    assert_eq!(
        stats.node_bytes.node_shells,
        pin.shells * SHELL_ACCOUNTED,
        "{what} n={n}: shell bytes"
    );
    assert_eq!(
        stats.node_bytes.suffixes, pin.suffix_bytes,
        "{what} n={n}: suffix bytes"
    );
    let lower = pin.shells * SHELL_ACCOUNTED + pin.suffix_bytes + pin.leaf_submap_bytes;
    let sub_maps = stats.node_bytes.sub_maps.total();
    if pin.over_submaps == 0 {
        // Every sub-map is a root leaf: the bound is the whole of `mem_used`.
        assert_eq!(
            sub_maps, pin.leaf_submap_bytes,
            "{what} n={n}: sub-map bytes"
        );
        assert_eq!(map.mem_used(), lower, "{what} n={n}: mem_used");
    } else {
        // The sub-maps above ROOT_LEAF_CAP are the part the script does not
        // derive; they can only add bytes.
        assert!(
            sub_maps > pin.leaf_submap_bytes,
            "{what} n={n}: sub-map bytes {sub_maps} do not exceed the root-leaf part {}",
            pin.leaf_submap_bytes
        );
        assert!(
            map.mem_used() > lower,
            "{what} n={n}: mem_used {} is not above the lower bound {lower}",
            map.mem_used()
        );
    }
}

#[test]
fn direct_encoding_census_matches_the_script() {
    for pin in &DIRECT {
        let mut map = ExpanseStrMap::new();
        for (i, name) in gen_names(pin.n).iter().enumerate() {
            let key = key_direct(name);
            let key = NulFreeStr::new(&key).expect("key_direct is NUL-free");
            assert_eq!(
                map.insert(key, i as u64),
                None,
                "generated names are distinct"
            );
        }
        check(&map, pin, "direct");
    }
}

#[test]
fn ordered_bytes_map_census_matches_the_script() {
    for pin in &ORDERED_BYTES_STORED {
        let mut map = ExpanseOrderedBytesMap::new();
        for (i, name) in gen_names(pin.n).iter().enumerate() {
            assert_eq!(map.insert(&key_nul(name), i as u64), None);
        }
        check(map.as_inner(), pin, "ordered_bytes_stored");
    }
}

/// The census is a function of the key set: reversed insertion order gives the
/// same structure and the same bytes.
#[test]
fn census_does_not_depend_on_insertion_order() {
    let names = gen_names(2000);
    let mut forward = ExpanseStrMap::new();
    let mut backward = ExpanseStrMap::new();
    for (i, name) in names.iter().enumerate() {
        let key = key_direct(name);
        forward.insert(NulFreeStr::new(&key).expect("NUL-free"), i as u64);
    }
    for (i, name) in names.iter().enumerate().rev() {
        let key = key_direct(name);
        backward.insert(NulFreeStr::new(&key).expect("NUL-free"), i as u64);
    }
    assert_eq!(forward.stats().node_bytes, backward.stats().node_bytes);
    assert_eq!(forward.mem_used(), backward.mem_used());
}

/// Both encodings order names as RFC 4034 §6.1 does: the ordered map returns
/// the RFC's own example in the order the RFC lists it.
#[test]
fn rfc4034_example_iterates_in_canonical_order() {
    let sorted: [&[&[u8]]; 9] = [
        &[b"example"],
        &[b"a", b"example"],
        &[b"yljkjljk", b"a", b"example"],
        &[b"Z", b"a", b"example"],
        &[b"zABC", b"a", b"EXAMPLE"],
        &[b"z", b"example"],
        &[b"\x01", b"z", b"example"],
        &[b"*", b"z", b"example"],
        &[b"\xc8", b"z", b"example"],
    ];
    let names: Vec<Vec<Vec<u8>>> = sorted
        .iter()
        .map(|n| n.iter().map(|l| l.to_vec()).collect())
        .collect();
    let mut map = ExpanseOrderedBytesMap::new();
    for (i, name) in names.iter().enumerate().rev() {
        map.insert(&key_nul(name), i as u64);
    }
    let order: Vec<u64> = map.iter().map(|(_, v)| v).collect();
    assert_eq!(order, (0..9).collect::<Vec<u64>>());
    // The predecessor of a name that is absent is the name before it.
    let absent = key_nul(&[b"b".to_vec(), b"example".to_vec()]);
    let (_, v) = map
        .prev_at_or_before_entry(&absent)
        .expect("a predecessor exists");
    assert_eq!(
        v, 4,
        "b.example sorts after zABC.a.EXAMPLE and before z.example"
    );
}
