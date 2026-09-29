//! Layout census: a diagnostic walk that decomposes a tree's `mem_used()`
//! into the allocation shapes behind it (Refs #1257, #1256, #1255).
//!
//! Compiled only under the diagnostic `layout-census` feature. It records
//! allocation *shapes* rather than bytes wherever a size is a function of a
//! count — a linear leaf by (slot level, key bytes, population), a bitmap
//! branch's edge subarray by its entry count — so a cost model outside the
//! crate (`scripts/leaf_layout_model.py`) can re-price the same tree under
//! a different size-class ladder and must reproduce `mem_used()` exactly
//! under the current one. [`LayoutCensus::bytes`] is the walk's own sum;
//! the tests pin it equal to `mem_used()`.
//!
//! With a [`MergeRule`], the walk also reports every **topmost** branch that
//! the rule would have kept as one leaf: a branch at form level 2..=7 whose
//! subtree holds at most `max_keys` keys and whose decoded byte takes at most
//! `max_digits` values. Each such group is reported with its key count, the
//! number of distinct values at every byte position of its remainders, and
//! the bytes its current subtree occupies, which is what a leaf form that
//! replaces the subtree has to beat. The rule is evaluated on the tree the
//! current engine built: it projects the canonical insert-only shape of an
//! engine that applied the rule, and says nothing about hysteresis.
//!
//! Not a stability surface: the field set follows the model's needs.

use crate::alloc::accounted_size;
use crate::mutate::{
    branch_form_level, decode_value, immed_keys, immed_map_keys, leaf_keys, sub_edges_size,
    sub_vals_size,
};
use crate::mutate_map::{map_immed_val_size, read_map_leaf};
use crate::node::{BranchB, BranchL3, BranchL7, BranchU, Edge, LeafBitmap1, LeafBitmapL};
use crate::types::{EdgeTag, EdgeType, RAW_ALIGN};
use core::mem::size_of;
use core_alloc::collections::BTreeMap;
use core_alloc::vec;
use core_alloc::vec::Vec;

/// Which topmost branches a projected leaf form would have absorbed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MergeRule {
    /// Largest subtree population the projected leaf may hold.
    pub max_keys: usize,
    /// Largest number of distinct values of the branch's decoded byte.
    pub max_digits: usize,
}

/// One class of merge candidates: every field but the count is a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MergeGroup {
    /// The branch's form level: the width of the remainders its keys differ in.
    pub key_bytes: u8,
    /// The level of the slot that holds the branch. Above `key_bytes` only
    /// behind a narrow pointer; a leaf that grows in that slot instead of
    /// cascading keeps this width.
    pub slot_level: u8,
    /// Keys in the subtree.
    pub keys: usize,
    /// Distinct values at each byte of the `key_bytes`-byte remainder, most
    /// significant first; entry 0 is the branch's own digit count. Entries
    /// at and past `key_bytes` are 0.
    pub byte_cards: [u16; 7],
    /// Heap bytes of the subtree the projected leaf would replace: the
    /// branch, its subarrays and every descendant allocation.
    pub bytes: usize,
}

/// Allocation shapes of one tree, or of every sub-map of a string map.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LayoutCensus {
    /// Keys counted by the walk.
    pub keys: u64,
    /// Heap bytes the walk attributes; equal to `mem_used()`.
    pub bytes: usize,
    /// Root leaves by population.
    pub root_leaves: BTreeMap<usize, usize>,
    /// Linear leaves by (slot level, key bytes, population). Key bytes are
    /// below the slot level only behind a narrow pointer.
    pub linear_leaves: BTreeMap<(u8, u8, usize), usize>,
    /// Immediate edges by (level, key count). They cost heap bytes only in
    /// the map flavor with two or more keys (a class-sized value array).
    pub immediates: BTreeMap<(u8, usize), usize>,
    /// Bitmap leaves by slot level (level 1 unless behind a narrow pointer).
    pub bitmap_leaves: BTreeMap<u8, usize>,
    /// Map-flavor bitmap-leaf value subarrays by entry count.
    pub bitmap_leaf_value_subarrays: BTreeMap<usize, usize>,
    /// Linear branches of up to 3 children.
    pub branch_l3: usize,
    /// Linear branches of up to 7 children.
    pub branch_l7: usize,
    /// Bitmap branches.
    pub branch_b: usize,
    /// Bitmap-branch edge subarrays by entry count.
    pub branch_b_subarrays: BTreeMap<usize, usize>,
    /// Uncompressed branches.
    pub branch_u: usize,
    /// Branches by (form level, child count), every form together.
    pub branches_by_level_fanout: BTreeMap<(u8, usize), usize>,
    /// String-map node shells (0 for the integer maps).
    pub str_nodes: usize,
    /// Bytes of those shells.
    pub str_node_shell_bytes: usize,
    /// String-map suffix leaves by suffix length.
    pub suffix_leaves: BTreeMap<usize, usize>,
    /// Bytes of those suffix leaves, whether inside or outside the allocator.
    pub suffix_bytes: usize,
    /// Topmost branches the [`MergeRule`] would absorb, when one was given.
    pub merge_groups: BTreeMap<MergeGroup, usize>,
}

impl LayoutCensus {
    /// Folds another census into this one.
    pub fn absorb(&mut self, o: &Self) {
        fn add<K: Ord + Copy>(a: &mut BTreeMap<K, usize>, b: &BTreeMap<K, usize>) {
            for (k, v) in b {
                *a.entry(*k).or_insert(0) += v;
            }
        }
        self.keys += o.keys;
        self.bytes += o.bytes;
        add(&mut self.root_leaves, &o.root_leaves);
        add(&mut self.linear_leaves, &o.linear_leaves);
        add(&mut self.immediates, &o.immediates);
        add(&mut self.bitmap_leaves, &o.bitmap_leaves);
        add(
            &mut self.bitmap_leaf_value_subarrays,
            &o.bitmap_leaf_value_subarrays,
        );
        self.branch_l3 += o.branch_l3;
        self.branch_l7 += o.branch_l7;
        self.branch_b += o.branch_b;
        add(&mut self.branch_b_subarrays, &o.branch_b_subarrays);
        self.branch_u += o.branch_u;
        add(
            &mut self.branches_by_level_fanout,
            &o.branches_by_level_fanout,
        );
        self.str_nodes += o.str_nodes;
        self.str_node_shell_bytes += o.str_node_shell_bytes;
        add(&mut self.suffix_leaves, &o.suffix_leaves);
        self.suffix_bytes += o.suffix_bytes;
        add(&mut self.merge_groups, &o.merge_groups);
    }
}

const fn raw(bytes: usize) -> usize {
    accounted_size(bytes, RAW_ALIGN)
}

/// What one subtree reports to its parent.
struct Sub {
    pop: usize,
    bytes: usize,
    /// The subtree's key remainders at its slot level, when it is small
    /// enough for a merge rule to consider; `None` otherwise.
    keys: Option<Vec<u64>>,
    /// Topmost qualifying groups inside the subtree.
    groups: Vec<MergeGroup>,
}

/// Records a root leaf (`bytes` is its allocation, already class-sized).
pub(crate) fn root_leaf(c: &mut LayoutCensus, pop: usize, bytes: usize) {
    c.keys += pop as u64;
    c.bytes += raw(bytes);
    *c.root_leaves.entry(pop).or_insert(0) += 1;
}

/// Walks the tree under the level-8 `top` edge into `c`.
///
/// # Safety
///
/// `top` must be the live top edge of a tree of flavor `MAP` with no
/// concurrent writer.
pub(crate) unsafe fn tree<const MAP: bool>(
    c: &mut LayoutCensus,
    top: &Edge,
    rule: Option<MergeRule>,
) {
    // SAFETY: forwarded contract.
    let sub = unsafe { walk::<MAP>(c, top, 8, rule) };
    c.keys += sub.pop as u64;
    c.bytes += sub.bytes;
    for g in sub.groups {
        *c.merge_groups.entry(g).or_insert(0) += 1;
    }
}

fn wants_keys(rule: Option<MergeRule>, pop: usize) -> bool {
    rule.is_some_and(|r| pop <= r.max_keys)
}

/// # Safety
///
/// `edge` is a live edge at slot `level` of a quiescent tree of flavor `MAP`.
unsafe fn walk<const MAP: bool>(
    c: &mut LayoutCensus,
    edge: &Edge,
    level: u8,
    rule: Option<MergeRule>,
) -> Sub {
    let empty = |pop, bytes| Sub {
        pop,
        bytes,
        keys: None,
        groups: Vec::new(),
    };
    let Some(tag) = edge.tag() else {
        return empty(0, 0);
    };
    match tag {
        // An empty slot holds no keys, and so does not stop its parent from
        // forming a group.
        EdgeTag::Structural(EdgeType::Null) => Sub {
            pop: 0,
            bytes: 0,
            keys: rule.map(|_| Vec::new()),
            groups: Vec::new(),
        },
        // A full expanse costs no heap bytes, so no leaf form can beat the
        // subtree it sits in: reporting no keys keeps its parent out of every
        // group on purpose.
        EdgeTag::Structural(EdgeType::FullExpanse) => {
            debug_assert!(level < 8, "a full-expanse edge below the root");
            let pop = 1usize << (8 * u32::from(level));
            empty(pop, 0)
        }
        EdgeTag::Immed(im) => {
            let n = im.key_count() as usize;
            *c.immediates.entry((level, n)).or_insert(0) += 1;
            let bytes = if MAP && n >= 2 {
                raw(map_immed_val_size(n))
            } else {
                0
            };
            let keys = wants_keys(rule, n).then(|| {
                if MAP {
                    immed_map_keys(edge, im).to_vec()
                } else {
                    immed_keys(edge, im).to_vec()
                }
            });
            Sub {
                pop: n,
                bytes,
                keys,
                groups: Vec::new(),
            }
        }
        EdgeTag::Structural(
            t @ (EdgeType::Leaf1
            | EdgeType::Leaf2
            | EdgeType::Leaf3
            | EdgeType::Leaf4
            | EdgeType::Leaf5
            | EdgeType::Leaf6
            | EdgeType::Leaf7),
        ) => {
            let kb = t.leaf_key_bytes().expect("linear leaf tag");
            let pop = (edge.pop0(kb) + 1) as usize;
            *c.linear_leaves.entry((level, kb, pop)).or_insert(0) += 1;
            let bytes = raw(if MAP {
                crate::leaf::size_map(kb, pop)
            } else {
                crate::leaf::size_set(kb, pop)
            });
            let keys = wants_keys(rule, pop).then(|| {
                let prefix = if kb < level {
                    decode_value(edge, kb, level) << (8 * u32::from(kb))
                } else {
                    0
                };
                // SAFETY: live leaf of `pop` keys per contract.
                let low: Vec<u64> = unsafe {
                    if MAP {
                        read_map_leaf(edge, kb, pop)
                            .into_iter()
                            .map(|(k, _)| k)
                            .collect()
                    } else {
                        leaf_keys(edge, kb, pop)
                    }
                };
                low.into_iter().map(|k| k | prefix).collect()
            });
            Sub {
                pop,
                bytes,
                keys,
                groups: Vec::new(),
            }
        }
        EdgeTag::Structural(EdgeType::LeafB1) => {
            *c.bitmap_leaves.entry(level).or_insert(0) += 1;
            let ptr = edge.node_ptr();
            let (bitmap, mut bytes) = if MAP {
                // SAFETY: live map-flavor bitmap leaf.
                let node = unsafe { &*ptr.cast::<LeafBitmapL>() };
                (node.bitmap, size_of::<LeafBitmapL>())
            } else {
                // SAFETY: live set-flavor bitmap leaf.
                let node = unsafe { &*ptr.cast::<LeafBitmap1>() };
                (node.bitmap, size_of::<LeafBitmap1>())
            };
            if MAP {
                for sub in 0..8u8 {
                    let n = (0..32u8).filter(|i| bitmap.test(sub * 32 + i)).count();
                    if n > 0 {
                        *c.bitmap_leaf_value_subarrays.entry(n).or_insert(0) += 1;
                        bytes += raw(sub_vals_size(n));
                    }
                }
            }
            let pop = bitmap.count() as usize;
            let keys = wants_keys(rule, pop).then(|| {
                let prefix = if level > 1 {
                    decode_value(edge, 1, level) << 8
                } else {
                    0
                };
                (0..=255u8)
                    .filter(|&d| bitmap.test(d))
                    .map(|d| u64::from(d) | prefix)
                    .collect()
            });
            Sub {
                pop,
                bytes,
                keys,
                groups: Vec::new(),
            }
        }
        EdgeTag::Structural(
            t @ (EdgeType::BranchL3 | EdgeType::BranchL7 | EdgeType::BranchB | EdgeType::BranchU),
        ) => {
            // SAFETY: live branch of type `t`.
            let bl = unsafe { branch_form_level(edge, t, level) };
            let ptr = edge.node_ptr();
            let mut children: Vec<(u8, &Edge)> = Vec::new();
            let mut bytes = 0usize;
            match t {
                EdgeType::BranchL3 => {
                    c.branch_l3 += 1;
                    bytes += size_of::<BranchL3>();
                    // SAFETY: the edge's tag says `ptr` is a live BranchL3.
                    let b = unsafe { &*ptr.cast::<BranchL3>() };
                    for i in 0..b.hdr.num as usize {
                        children.push((b.hdr.digits[i], &b.edges[i]));
                    }
                }
                EdgeType::BranchL7 => {
                    c.branch_l7 += 1;
                    bytes += size_of::<BranchL7>();
                    // SAFETY: the edge's tag says `ptr` is a live BranchL7.
                    let b = unsafe { &*ptr.cast::<BranchL7>() };
                    for i in 0..b.hdr.num as usize {
                        children.push((b.hdr.digits[i], &b.edges[i]));
                    }
                }
                EdgeType::BranchB => {
                    c.branch_b += 1;
                    bytes += size_of::<BranchB>();
                    // SAFETY: the edge's tag says `ptr` is a live BranchB.
                    let b = unsafe { &*ptr.cast::<BranchB>() };
                    for sub in 0..8usize {
                        let n = b.pop_counts[sub] as usize;
                        if n == 0 {
                            continue;
                        }
                        *c.branch_b_subarrays.entry(n).or_insert(0) += 1;
                        bytes += raw(sub_edges_size(n));
                        let digits = (0..32u8)
                            .map(|i| (sub * 32) as u8 + i)
                            .filter(|&d| b.bitmap.test(d));
                        for (i, d) in digits.enumerate() {
                            // SAFETY: index below the subexpanse's count.
                            children.push((d, unsafe { &*b.subarrays[sub].add(i) }));
                        }
                    }
                }
                _ => {
                    c.branch_u += 1;
                    bytes += size_of::<BranchU>();
                    // SAFETY: the edge's tag says `ptr` is a live BranchU.
                    let b = unsafe { &*ptr.cast::<BranchU>() };
                    for (d, e) in b.edges.iter().enumerate() {
                        if !e.is_null() {
                            children.push((d as u8, e));
                        }
                    }
                }
            }
            *c.branches_by_level_fanout
                .entry((bl, children.len()))
                .or_insert(0) += 1;
            let mut pop = 0usize;
            let mut groups = Vec::new();
            let mut keys: Option<Vec<u64>> = rule.map(|_| Vec::new());
            for &(d, e) in &children {
                // SAFETY: live child at the level below the branch's form level.
                let sub = unsafe { walk::<MAP>(c, e, bl - 1, rule) };
                pop += sub.pop;
                bytes += sub.bytes;
                groups.extend(sub.groups);
                keys = match (keys, sub.keys) {
                    (Some(mut acc), Some(ks)) if wants_keys(rule, pop) => {
                        let shift = 8 * u32::from(bl - 1);
                        acc.extend(ks.into_iter().map(|k| (u64::from(d) << shift) | k));
                        Some(acc)
                    }
                    _ => None,
                };
            }
            // Would the rule keep this subtree as one leaf?
            if let (Some(r), Some(ks)) = (rule, keys.as_ref())
                && (2..=7).contains(&bl)
                && children.len() <= r.max_digits
                && pop <= r.max_keys
            {
                let mut byte_cards = [0u16; 7];
                for (i, card) in byte_cards.iter_mut().enumerate().take(bl as usize) {
                    let shift = 8 * (u32::from(bl) - 1 - i as u32);
                    let mut seen = [false; 256];
                    for &k in ks {
                        seen[((k >> shift) & 0xFF) as usize] = true;
                    }
                    *card = seen.iter().filter(|&&s| s).count() as u16;
                }
                groups = vec![MergeGroup {
                    key_bytes: bl,
                    slot_level: level,
                    keys: pop,
                    byte_cards,
                    bytes,
                }];
            }
            // Re-express the kept keys at the slot level.
            let keys = keys.map(|ks| {
                if bl < level {
                    let prefix = decode_value(edge, bl, level) << (8 * u32::from(bl));
                    ks.into_iter().map(|k| k | prefix).collect()
                } else {
                    ks
                }
            });
            Sub {
                pop,
                bytes,
                keys,
                groups,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MergeRule;
    use crate::map::ExpanseMap;
    use crate::set::ExpanseSet;
    use crate::strmap::{ExpanseStrMap, NulFreeStr};
    use core_alloc::format;
    use core_alloc::vec::Vec;

    fn xorshift(state: &mut u64) -> u64 {
        let mut x = *state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *state = x;
        x
    }

    /// Key sets spanning every node form: root leaves, immediates, narrow
    /// pointers, bitmap leaves, and all four branch forms.
    fn shapes() -> Vec<(&'static str, Vec<u64>)> {
        let mut out = Vec::new();
        for n in [5usize, 31, 32, 200, 5_000, 70_000] {
            let mut s = 0x0DDB_1A5E_5EED_0001u64;
            out.push(("random", (0..n).map(|_| xorshift(&mut s)).collect()));
            out.push(("sequential", (0..n as u64).collect()));
            out.push(("sparse", (0..n as u64).map(|i| i << 40).collect()));
            out.push((
                "decimal",
                (0..n as u64)
                    .map(|i| {
                        u64::from_be_bytes(*format!("{i:08}").as_bytes().first_chunk().unwrap())
                    })
                    .collect(),
            ));
        }
        out
    }

    /// The census is a decomposition of `mem_used()`, not an estimate beside
    /// it: its byte total and key count match the engine's, for both
    /// flavors, with and without a merge rule, and every merge group
    /// replaces bytes the walk counted.
    #[test]
    fn census_sums_to_mem_used() {
        let rules = [
            None,
            Some(MergeRule {
                max_keys: 256,
                max_digits: 16,
            }),
        ];
        for (name, keys) in shapes() {
            let mut m = ExpanseMap::new();
            let mut s = ExpanseSet::new();
            for &k in &keys {
                m.insert(k, k);
                s.insert(k);
            }
            // Twice: as built, and after removing every third key, so the
            // census also covers trees the remove walks have demoted.
            for round in 0..2 {
                if round == 1 {
                    for &k in keys.iter().step_by(3) {
                        m.remove(k);
                        s.remove(k);
                    }
                }
                let live = m.len();
                for rule in rules {
                    let cm = m.layout_census(rule);
                    assert_eq!(
                        cm.bytes,
                        m.mem_used(),
                        "map {name} n={} round {round}",
                        keys.len()
                    );
                    assert_eq!(cm.keys, live, "map {name} round {round}");
                    let cs = s.layout_census(rule);
                    assert_eq!(
                        cs.bytes,
                        s.mem_used(),
                        "set {name} n={} round {round}",
                        keys.len()
                    );
                    assert_eq!(cs.keys, live, "set {name} round {round}");
                    for c in [&cm, &cs] {
                        let merged: usize = c.merge_groups.iter().map(|(g, n)| g.bytes * n).sum();
                        assert!(
                            merged <= c.bytes,
                            "{name}: groups replace more than the tree"
                        );
                        if rule.is_none() {
                            assert!(c.merge_groups.is_empty());
                        }
                    }
                }
            }
        }
    }

    /// The joint (level, population) histogram (#1256): its row sums, with
    /// bitmap leaves, are `stats().leaf_depth_histogram`.
    #[test]
    fn joint_histogram_rows_sum_to_leaf_depth_histogram() {
        for (name, keys) in shapes() {
            let mut m = ExpanseMap::new();
            for &k in &keys {
                m.insert(k, k);
            }
            let st = m.stats();
            let c = m.layout_census(None);
            let mut rows = [0usize; 9];
            for (&(level, _, _), &n) in &c.linear_leaves {
                rows[level as usize] += n;
            }
            for (&level, &n) in &c.bitmap_leaves {
                rows[level as usize] += n;
            }
            rows[0] += c.root_leaves.values().sum::<usize>();
            assert_eq!(rows, st.leaf_depth_histogram, "{name} n={}", keys.len());
        }
    }

    /// The string map's census adds shells and suffix leaves to its word
    /// maps' shapes and still sums to `mem_used()` (#1255).
    #[test]
    fn strmap_census_sums_to_mem_used() {
        for n in [1usize, 40, 3_000, 30_000] {
            let mut m = ExpanseStrMap::new();
            for i in 0..n {
                let s = format!("t{:03}:orders:{i:010}", i % 7);
                m.insert(NulFreeStr::new(s.as_bytes()).unwrap(), i as u64);
                // Keys that end mid-chunk and keys that leave suffix leaves.
                let s = format!("k{i}-{}", "x".repeat(i % 23));
                m.insert(NulFreeStr::new(s.as_bytes()).unwrap(), i as u64);
            }
            let c = m.layout_census(Some(MergeRule {
                max_keys: 128,
                max_digits: 16,
            }));
            assert_eq!(c.bytes, m.mem_used(), "n={n}");
            assert_eq!(c.keys, m.len());
            assert!(c.str_nodes > 0);
        }
    }
    /// A removal-heavy phase (#1257 follow-up): load, remove a random half,
    /// census. The census still decomposes `mem_used()` exactly and counts
    /// the survivors, so what a removal phase leaves behind is measured by
    /// the same instrument as a fresh build, not assumed.
    #[test]
    fn census_after_random_half_removal() {
        let mut state = 0x0DDB_1A5E_5EED_0002u64;
        let n = 20_000u64;
        let keys: Vec<u64> = (0..n).map(|i| i * 7 + 3).collect();
        let doomed: Vec<bool> = (0..n).map(|_| xorshift(&mut state) & 1 == 1).collect();
        let mut m = ExpanseMap::new();
        let mut s = ExpanseSet::new();
        let mut sm = ExpanseStrMap::new();
        let names: Vec<_> = (0..n)
            .map(|i| format!("t{:03}:orders:{i:010}", i % 13))
            .collect();
        for (i, &k) in keys.iter().enumerate() {
            m.insert(k, k);
            s.insert(k);
            sm.insert(NulFreeStr::new(names[i].as_bytes()).unwrap(), k);
        }
        for (i, &k) in keys.iter().enumerate() {
            if doomed[i] {
                m.remove(k);
                s.remove(k);
                sm.remove(NulFreeStr::new(names[i].as_bytes()).unwrap());
            }
        }
        let live = doomed.iter().filter(|&&d| !d).count() as u64;
        assert!(
            live > n / 3 && live < 2 * n / 3,
            "a random half, not a degenerate one"
        );
        let rule = Some(MergeRule {
            max_keys: 128,
            max_digits: 16,
        });
        for rule in [None, rule] {
            let (cm, cs, csm) = (
                m.layout_census(rule),
                s.layout_census(rule),
                sm.layout_census(rule),
            );
            assert_eq!((cm.bytes, cm.keys), (m.mem_used(), live));
            assert_eq!((cs.bytes, cs.keys), (s.mem_used(), live));
            assert_eq!((csm.bytes, csm.keys), (sm.mem_used(), live));
        }
    }

    /// The census through the `Sync*` wrappers (#1257 follow-up): after
    /// inserts from several threads, taken with writers excluded, it sums to
    /// the wrapped tree's `mem_used()` and counts every key. The string
    /// map's count comes from the walk, not from a population field an
    /// optimistic writer may have left stale.
    #[test]
    #[cfg(feature = "std")]
    fn census_through_sync_wrappers() {
        use crate::sync::{SyncExpanseMap, SyncExpanseSet, SyncExpanseStrMap};
        let m = SyncExpanseMap::new();
        let s = SyncExpanseSet::new();
        let sm = SyncExpanseStrMap::new();
        let per = 5_000u64;
        std::thread::scope(|scope| {
            for t in 0..4u64 {
                let (m, s, sm) = (&m, &s, &sm);
                scope.spawn(move || {
                    for i in 0..per {
                        let k = (i << 8) | t;
                        m.insert(k, k);
                        s.insert(k);
                        let name = format!("t{t:03}:orders:{i:010}");
                        sm.insert(NulFreeStr::new(name.as_bytes()).unwrap(), k);
                    }
                });
            }
        });
        let rule = Some(MergeRule {
            max_keys: 128,
            max_digits: 16,
        });
        let cm = m.layout_census(rule);
        assert_eq!((cm.bytes, cm.keys), (m.mem_used(), 4 * per));
        let cs = s.layout_census(rule);
        assert_eq!(
            (cs.bytes, cs.keys),
            (s.with_locked(ExpanseSet::mem_used), 4 * per)
        );
        let csm = sm.layout_census(rule);
        assert_eq!(
            (csm.bytes, csm.keys),
            (sm.with_locked(ExpanseStrMap::mem_used), 4 * per)
        );
        assert!(
            !csm.merge_groups.is_empty(),
            "the decimal ranges form groups"
        );
    }
}
