//! The `BranchSplitKind::Remove` sub-split (Refs #1610): each optimistic
//! removal that falls back as `BranchSplitRemove` also bumps exactly one of
//! five sub-counters, and each sub-counter is reached by the removal it
//! names.
//!
//! `occ_stats` counters are process-global, so one test here reads them:
//! the harness runs each integration target in its own process, and a second
//! counter-reading test would race the deltas this one asserts. The other
//! test is a source census and reads no counter.
//!
//! **Behavioural cases.** Each builds a fresh tree, checks the node form the
//! case depends on with `stats()`, and asserts the exact counter delta of one
//! removal. They run on `SyncExpanseMap` (the map body `SyncExpanseStrMap`'s
//! sub-maps share) and on `SyncExpanseSet`, whose removal sites are separate
//! code. A churn over all three wrappers then asserts the partition identity
//! (sub-counters sum to `BranchSplitRemove`) on workloads that mix causes.
//!
//! **Census.** `sync.rs` has twelve `Remove` sites, six per walk. The cases
//! reach seven of them: on both walks the prefix mismatch at a narrow bitmap
//! leaf and at a narrow linear leaf, and a child emptied as a one-key
//! immediate; on the map walk a one-key linear leaf emptied. No fixture here
//! reaches the other five (a one-key bitmap leaf, which demotes before it gets
//! there; the set's one-key linear leaf, which demotes to an immediate first;
//! a narrow immediate on either walk, which the optimistic demotion never
//! builds). The census pins every site's category from the source instead.
//!
//! # Workload shape
//!
//! | field | value |
//! |---|---|
//! | `workload_id` | `olc_remove_split_attribution` |
//! | `group` | correctness |
//! | `population` | per case 45 to 85 integer keys; churn 20000 integer keys per wrapper, 20000 short strings |
//! | `insertion_order` | generator |
//! | `probes_and_reuse` | each case removes one key (or probes one absent key) per assertion; churn removes every key once |
//! | `hit_rate` | n/a — counter attribution, not lookup |
//! | `miss_gen_method` | prefix-mismatch probes change one digit inside a narrow pointer's skipped prefix |
//! | `value_dereference` | the removed value is compared |
//! | `measured_region` | n/a — counter invariants, nothing is timed |
//! | `arm_symmetry` | the same keys on the map and the set |
//! | `statistics` | none — deterministic integer counters |
//! | `verdict` | assertion on exact counter deltas, the partition identity and the source census |
// Kept as its own attribute, and in this exact form: the nightly Miri shard
// census (`scripts/check_miri_shards.py`) matches `^#!\[cfg\(not\(miri\)\)\]`
// literally.
#![cfg(not(miri))]
#![cfg(all(feature = "occ-stats", feature = "std", target_pointer_width = "64"))]

use expanse_trie::occ_stats::{self, NAMES, NUM_STATS, Stat};
use expanse_trie::strmap::NulFreeStr;
use expanse_trie::sync::{SyncExpanseMap, SyncExpanseSet, SyncExpanseStrMap};
use expanse_trie::validate::NodeCounts;

/// The five sub-counters of `BranchSplitRemove`.
const SPLIT: [Stat; 5] = [
    Stat::BranchSplitRemoveEmptiedLinear,
    Stat::BranchSplitRemoveEmptiedBitmap,
    Stat::BranchSplitRemoveEmptiedOther,
    Stat::BranchSplitRemovePrefix,
    Stat::BranchSplitRemoveOther,
];

type Delta = [u64; NUM_STATS];

fn delta(f: impl FnOnce()) -> Delta {
    let before = occ_stats::snapshot();
    f();
    let after = occ_stats::snapshot();
    let mut d = [0; NUM_STATS];
    // Wrapping: the gauges (retained garbage bytes) can fall across a
    // removal. Only event counters are read from the delta.
    for (i, slot) in d.iter_mut().enumerate() {
        *slot = after[i].wrapping_sub(before[i]);
    }
    d
}

/// Asserts that one removal fell back as `BranchSplitRemove` with sub-cause
/// `want`, and that no other sub-counter moved.
fn assert_one(case: &str, d: &Delta, want: Stat) {
    assert_eq!(
        d[Stat::BranchSplitRemove as usize],
        1,
        "{case}: expected one BranchSplitRemove"
    );
    assert_eq!(
        d[Stat::FallbackBranchSplit as usize],
        1,
        "{case}: expected one FallbackBranchSplit"
    );
    for s in SPLIT {
        let exp = u64::from(s == want);
        assert_eq!(
            d[s as usize], exp,
            "{case}: {} moved by {}, expected {exp}",
            NAMES[s as usize], d[s as usize]
        );
    }
}

/// Asserts that a removal took no `BranchSplitRemove` fallback at all.
fn assert_none(case: &str, d: &Delta) {
    assert_eq!(
        d[Stat::BranchSplitRemove as usize],
        0,
        "{case}: unexpected BranchSplitRemove"
    );
    for s in SPLIT {
        assert_eq!(d[s as usize], 0, "{case}: {} moved", NAMES[s as usize]);
    }
}

/// Six top digits: one key each under `1..=5`, and `0x42` holding 40 keys
/// that differ at level 2 — a narrow `BranchB` whose children are one-key
/// immediates. The top branch has six children: a `BranchL7`.
fn emptied_keys() -> Vec<u64> {
    let mut keys: Vec<u64> = (1..=5u64).map(|i| (i << 56) | 1).collect();
    keys.extend((0..40u64).map(|d| (0x42 << 56) | (d << 8) | 1));
    keys
}

/// Seven top digits: one key each under `1..=5`, `0x06` holding 40 keys
/// that differ only in the low byte, and `0x42` as in [`emptied_keys`]. At
/// 40 keys a linear leaf overflows and the `0x06` keys become one bitmap leaf
/// behind a narrow pointer at level 7, its decode bytes holding `0x07` at
/// level 4. The `0x42` keys keep the root a tree while the case drains it.
fn prefix_keys() -> Vec<u64> {
    let mut keys: Vec<u64> = (1..=5u64).map(|i| (i << 56) | 1).collect();
    keys.extend((0..40u64).map(narrow_key));
    keys.extend((0..40u64).map(|d| (0x42 << 56) | (d << 8) | 1));
    keys
}

fn narrow_key(low: u64) -> u64 {
    (6 << 56) | (7 << 24) | low
}

/// A key absent from [`prefix_keys`] whose skipped level-4 digit differs.
const PREFIX_PROBE: u64 = (6 << 56) | (8 << 24) | 3;

trait Tree {
    fn build(keys: &[u64]) -> Self;
    fn rm(&self, key: u64) -> bool;
    fn counts(&self) -> NodeCounts;
    fn check(&self);
}

impl Tree for SyncExpanseMap {
    fn build(keys: &[u64]) -> Self {
        let m = SyncExpanseMap::new();
        for &k in keys {
            m.insert(k, !k);
        }
        m
    }
    fn rm(&self, key: u64) -> bool {
        match self.remove(key) {
            Some(v) => {
                assert_eq!(v, !key, "map: wrong value removed for {key:#x}");
                true
            }
            None => false,
        }
    }
    fn counts(&self) -> NodeCounts {
        self.with_locked(|m| m.stats()).node_counts
    }
    fn check(&self) {
        self.with_locked(expanse_trie::map::ExpanseMap::validate);
    }
}

impl Tree for SyncExpanseSet {
    fn build(keys: &[u64]) -> Self {
        let s = SyncExpanseSet::new();
        for &k in keys {
            s.insert(k);
        }
        s
    }
    fn rm(&self, key: u64) -> bool {
        self.remove(key)
    }
    fn counts(&self) -> NodeCounts {
        self.with_locked(|s| s.stats()).node_counts
    }
    fn check(&self) {
        self.with_locked(expanse_trie::set::ExpanseSet::validate);
    }
}

/// Child emptied under a linear branch and under a bitmap branch.
fn emptied_cases<T: Tree>(name: &str) {
    let keys = emptied_keys();

    let t = T::build(&keys);
    let c = t.counts();
    assert_eq!(c.branch_l7, 1, "{name}: fixture needs a BranchL7: {c:?}");
    assert_eq!(c.branch_b, 1, "{name}: fixture needs a BranchB: {c:?}");
    assert_eq!(c.branch_u, 0, "{name}: no BranchU in the fixture: {c:?}");
    let d = delta(|| assert!(t.rm((1 << 56) | 1)));
    assert_one(
        &format!("{name}: one-key immediate under BranchL7"),
        &d,
        Stat::BranchSplitRemoveEmptiedLinear,
    );
    t.check();

    // Above the floor, the emptied child leaves the `BranchB` optimistically
    // (Refs #1610): no fallback, and the branch keeps its form.
    let t = T::build(&keys);
    let d = delta(|| assert!(t.rm((0x42 << 56) | (10 << 8) | 1)));
    assert_none(
        &format!("{name}: one-key immediate under a BranchB of 40 digits"),
        &d,
    );
    assert_eq!(t.counts().branch_b, 1, "{name}: the BranchB keeps its form");
    assert!(
        !t.rm((0x42 << 56) | (10 << 8) | 1),
        "{name}: the key is gone"
    );
    t.check();

    // At the floor it still falls back: a `BranchB` of eight digits takes one
    // optimistic emptying down to seven, and the next, which would leave six
    // (`BRANCHB_TO_L7_DOWN`), falls back for the exclusive B → L7 demotion.
    // Each child holds `FLOOR_CHILD_KEYS` keys; only the removal of a child's
    // last key empties it.
    let t = T::build(&floor_keys());
    assert_eq!(
        t.counts().branch_b,
        1,
        "{name}: fixture needs a BranchB: {:?}",
        t.counts()
    );
    for j in 1..=FLOOR_CHILD_KEYS {
        let d = delta(|| assert!(t.rm(floor_key(7, j))));
        assert_none(
            &format!("{name}: digit 7's key {j}, eight digits to seven"),
            &d,
        );
    }
    assert_eq!(
        t.counts().branch_b,
        1,
        "{name}: seven digits stay a BranchB"
    );
    t.check();
    for j in 1..FLOOR_CHILD_KEYS {
        let d = delta(|| assert!(t.rm(floor_key(6, j))));
        assert_none(&format!("{name}: digit 6's key {j}"), &d);
    }
    let d = delta(|| assert!(t.rm(floor_key(6, FLOOR_CHILD_KEYS))));
    assert_one(
        &format!("{name}: the last key under a BranchB at its floor"),
        &d,
        Stat::BranchSplitRemoveEmptiedBitmap,
    );
    assert_eq!(
        t.counts().branch_b,
        0,
        "{name}: the exclusive path demoted the BranchB"
    );
    t.check();
}

/// Keys per child in [`floor_keys`]: eight children of five keys are more
/// than one leaf holds, so the eight become a `BranchB`.
const FLOOR_CHILD_KEYS: u64 = 5;

fn floor_key(d: u64, j: u64) -> u64 {
    (0x42 << 56) | (d << 8) | j
}

/// The top digits of [`emptied_keys`] with eight children under `0x42`: a
/// `BranchB` one digit above the count a removal may leave optimistically.
/// The 40 `0x06` keys of [`prefix_keys`] keep the root a tree.
fn floor_keys() -> Vec<u64> {
    let mut keys: Vec<u64> = (1..=5u64).map(|i| (i << 56) | 1).collect();
    keys.extend((0..40u64).map(narrow_key));
    keys.extend((0..8u64).flat_map(|d| (1..=FLOOR_CHILD_KEYS).map(move |j| floor_key(d, j))));
    keys
}

/// Prefix mismatch at a narrow bitmap leaf and at the narrow linear leaf it
/// demotes to; then the drain's last removal empties the child under the
/// top `BranchL7`.
fn prefix_cases<T: Tree>(name: &str) {
    let t = T::build(&prefix_keys());
    let c = t.counts();
    assert_eq!(
        c.leaf_bitmap, 1,
        "{name}: fixture needs a bitmap leaf: {c:?}"
    );
    assert_eq!(c.leaf_linear, 0, "{name}: no linear leaf yet: {c:?}");
    assert_eq!(c.branch_l7, 1, "{name}: fixture needs a BranchL7: {c:?}");
    let d = delta(|| assert!(!t.rm(PREFIX_PROBE)));
    assert_one(
        &format!("{name}: prefix mismatch at a narrow bitmap leaf"),
        &d,
        Stat::BranchSplitRemovePrefix,
    );

    // Drain until the bitmap leaf demotes. Shrinking a leaf in place, or
    // demoting it, is not a `Remove` fallback.
    let mut left = 40u64;
    while t.counts().leaf_bitmap == 1 {
        left -= 1;
        let d = delta(|| assert!(t.rm(narrow_key(left))));
        assert_none(&format!("{name}: draining the bitmap leaf to {left}"), &d);
    }
    let c = t.counts();
    assert_eq!(c.leaf_linear, 1, "{name}: expected a linear leaf: {c:?}");
    let d = delta(|| assert!(!t.rm(PREFIX_PROBE)));
    assert_one(
        &format!("{name}: prefix mismatch at a narrow linear leaf ({left} keys)"),
        &d,
        Stat::BranchSplitRemovePrefix,
    );

    // Drain to one key, then remove it: the child under the top branch
    // empties. The map's narrow linear leaf stays a leaf down to one key;
    // the set's demotes to an immediate first.
    while left > 1 {
        left -= 1;
        let d = delta(|| assert!(t.rm(narrow_key(left))));
        assert_none(&format!("{name}: draining the leaf to {left}"), &d);
    }
    let d = delta(|| assert!(t.rm(narrow_key(0))));
    assert_one(
        &format!("{name}: last key under the top BranchL7"),
        &d,
        Stat::BranchSplitRemoveEmptiedLinear,
    );
    t.check();
}

fn xorshift(x: &mut u64) -> u64 {
    *x ^= *x << 13;
    *x ^= *x >> 7;
    *x ^= *x << 17;
    *x
}

/// Inserts `n` keys and removes all of them, returning the removals' delta.
fn churn_int<T: Tree>(seed: u64, n: usize) -> Delta {
    let mut x = seed;
    let mut keys: Vec<u64> = (0..n).map(|_| xorshift(&mut x)).collect();
    keys.sort_unstable();
    keys.dedup();
    let t = T::build(&keys);
    for i in (1..keys.len()).rev() {
        keys.swap(i, (xorshift(&mut x) % (i as u64 + 1)) as usize);
    }
    let d = delta(|| {
        for &k in &keys {
            assert!(t.rm(k));
        }
    });
    t.check();
    d
}

/// The string map's removals reach the same map body through its sub-maps.
fn churn_str(seed: u64, n: usize) -> Delta {
    const ALNUM: &[u8; 62] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut x = seed;
    let mut keys: Vec<Vec<u8>> = (0..n)
        .map(|_| {
            let len = 4 + (xorshift(&mut x) % 13) as usize;
            (0..len)
                .map(|_| ALNUM[(xorshift(&mut x) % 62) as usize])
                .collect()
        })
        .collect();
    keys.sort_unstable();
    keys.dedup();
    let m = SyncExpanseStrMap::new();
    for (i, k) in keys.iter().enumerate() {
        m.insert(NulFreeStr::new(k).expect("alnum"), i as u64);
    }
    for i in (1..keys.len()).rev() {
        keys.swap(i, (xorshift(&mut x) % (i as u64 + 1)) as usize);
    }
    let d = delta(|| {
        for k in &keys {
            assert!(m.remove(NulFreeStr::new(k).expect("alnum")).is_some());
        }
    });
    assert_eq!(m.len(), 0);
    d
}

#[test]
fn remove_split_attribution() {
    emptied_cases::<SyncExpanseMap>("map");
    emptied_cases::<SyncExpanseSet>("set");
    prefix_cases::<SyncExpanseMap>("map");
    prefix_cases::<SyncExpanseSet>("set");

    let churns = [
        (
            "map churn",
            churn_int::<SyncExpanseMap>(0x0DDB_1A5E_5EED_1610, 20_000),
        ),
        (
            "set churn",
            churn_int::<SyncExpanseSet>(0x0DDB_1A5E_5EED_1611, 20_000),
        ),
        ("strmap churn", churn_str(0x0DDB_1A5E_5EED_1612, 20_000)),
    ];
    for (case, d) in &churns {
        let remove = d[Stat::BranchSplitRemove as usize];
        assert!(
            remove > 0,
            "{case}: the churn must take BranchSplitRemove fallbacks for the identity to bind"
        );
        let sum: u64 = SPLIT.iter().map(|&s| d[s as usize]).sum();
        assert_eq!(
            sum, remove,
            "{case}: the sub-counters must sum to BranchSplitRemove"
        );
        assert_eq!(
            d[Stat::BranchSplitRemoveEmptiedOther as usize],
            0,
            "{case}: a child emptied under an unclassified parent form"
        );
        assert_eq!(
            d[Stat::BranchSplitRemoveOther as usize],
            0,
            "{case}: no removal site constructs Other"
        );
    }
}

/// Strips `//` line comments and `/* */` block comments, leaving string
/// literals intact, so the census counts code and not prose (AGENTS.md §5:
/// a structural scanner must turn red when the code it guards is commented
/// out). The same routine as `test_fallback_attribution.rs`, with the same
/// stated gaps: a character literal holding a quote or a slash, and raw
/// strings, are read as ordinary text; `sync.rs` holds neither around the
/// patterns counted here.
fn strip_comments(src: &str) -> String {
    let b = src.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0usize;
    let mut in_str = false;
    let mut in_line = false;
    let mut block = 0usize;
    while i < b.len() {
        let c = b[i];
        let next = b.get(i + 1).copied();
        if in_line {
            if c == b'\n' {
                in_line = false;
                out.push(c);
            }
            i += 1;
        } else if block > 0 {
            if c == b'*' && next == Some(b'/') {
                block -= 1;
                i += 2;
            } else if c == b'/' && next == Some(b'*') {
                block += 1;
                i += 2;
            } else {
                if c == b'\n' {
                    out.push(c);
                }
                i += 1;
            }
        } else if in_str {
            if c == b'\\' {
                out.push(c);
                if let Some(n) = next {
                    out.push(n);
                }
                i += 2;
                continue;
            }
            if c == b'"' {
                in_str = false;
            }
            out.push(c);
            i += 1;
        } else if c == b'/' && next == Some(b'/') {
            in_line = true;
            i += 2;
        } else if c == b'/' && next == Some(b'*') {
            block = 1;
            i += 2;
        } else {
            if c == b'"' {
                in_str = true;
            }
            out.push(c);
            i += 1;
        }
    }
    String::from_utf8(out).expect("stripping ASCII delimiters preserves UTF-8 boundaries")
}

/// Every `Remove` site in `sync.rs` names its category, and each
/// prefix-mismatch site sits under the `decode_matches` failure it reports.
///
/// Clauses, each with its own negative control (AGENTS.md §5):
/// (a) every `Remove` fallback is one of the two forms below — 12 in all;
/// (b) exactly six report `PrefixMismatch`;
/// (c) exactly six classify the emptied child by `parent.edge_type`;
/// (d) each `PrefixMismatch` site has a `decode_matches(` within the three
///     lines above it.
#[test]
fn remove_split_census() {
    const PREFIX: &str = "branch_split(BranchSplitKind::Remove(RemoveSplit::PrefixMismatch))";
    const EMPTIED: &str =
        "branch_split(BranchSplitKind::Remove(RemoveSplit::emptied(parent.edge_type)))";
    const ANY: &str = "branch_split(BranchSplitKind::Remove(";
    let code = strip_comments(include_str!("../src/sync.rs"));
    // Joined without whitespace and without the trailing commas rustfmt adds
    // when it wraps an argument list, so a wrapped site still matches.
    let flat: String = code
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .replace(",)", ")");
    let any = flat.matches(ANY).count();
    let prefix = flat.matches(PREFIX).count();
    let emptied = flat.matches(EMPTIED).count();
    assert_eq!(any, 12, "(a) expected 12 Remove sites, found {any}");
    assert_eq!(
        prefix, 6,
        "(b) expected 6 PrefixMismatch sites, found {prefix}"
    );
    assert_eq!(
        emptied, 6,
        "(c) expected 6 emptied(parent.edge_type) sites, found {emptied}"
    );

    let lines: Vec<&str> = code.lines().collect();
    let mut paired = 0;
    for (i, line) in lines.iter().enumerate() {
        if !line.contains("RemoveSplit::PrefixMismatch))") {
            continue;
        }
        let window = &lines[i.saturating_sub(3)..i];
        assert!(
            window.iter().any(|l| l.contains("decode_matches(")),
            "(d) PrefixMismatch at sync.rs line {} has no decode_matches( within 3 lines",
            i + 1
        );
        paired += 1;
    }
    assert_eq!(paired, 6, "(d) expected 6 single-line PrefixMismatch sites");
}
