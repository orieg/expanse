//! Phase 4: the read-only lookup engine.
//!
//! [`test_set`] (set-flavor presence, the future `ExpanseSet`; compat:
//! Judy1) and [`get_map`] (map-flavor retrieval, the future `ExpanseMap`;
//! compat: JudyL) walk a subtree from an edge, decoding one
//! key digit per branch level. The walk is an iterative tag-dispatched
//! state machine: zero allocation, zero locks, at most one or two cache
//! lines touched per node (the Phase 3 geometry guarantee).
//!
//! `level` semantics: a call `lookup(edge, key, level)` means `edge` covers
//! an expanse of `level` undecoded low key bytes; digits at levels above
//! `level` were consumed (and validated) by the caller. A branch-tagged edge
//! sits *at* `level` and consumes `digit(key, level)`; leaf and immediate
//! tags state their own remaining-byte count.
//!
//! Narrow pointers: a leaf child (linear or bitmap) may sit below its
//! parent with skipped digits, validated against the edge's decode bytes
//! (`decode[i]` = the key digit at level `child_level + 1 + i`); the
//! mutation engines synthesize these for last-byte-divergent key sets.
//! Branch children **do** skip levels: a branch stores its own level in
//! its header (`BranchHeader.level` / `BranchB.level`), which
//! `mutate::branch_form_level` reads on every branch step. The original
//! resolves the same problem with per-level tag variants instead, which
//! keeps the level in a byte the walk has already loaded — see
//! `docs/ARCHITECTURE.md` §6 for why that trade is still open here.
//!
//! Two standing restrictions remain:
//!
//! - **immediate edges never skip** (their key bytes occupy the decode
//!   region; an immediate's `key_bytes` *is* its level, as in the original
//!   design), and a full-expanse edge covers its whole current expanse;
//! - **`BranchU` never skips** — it has no header to carry a level, so a
//!   skipping `BranchB` is wrapped one level above its form before it can
//!   be upgraded.
//!
//! Linear leaves (`Leaf1..Leaf7`) are header-less packed allocations whose
//! population comes from the parent edge's `pop0` — see `leaf`.

use crate::leaf;
use crate::node::{BranchB, BranchL3, BranchL7, BranchU, Edge, LeafBitmap1, LeafBitmapL};
use crate::types::{EdgeTag, EdgeType, ImmedType, Key, digit};

/// Outcome of locating a key in a subtree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lookup {
    /// The key is not present.
    Absent,
    /// The key is present (set flavor; no value).
    Present,
    /// The key is present with this value (map flavor).
    Value(u64),
}

/// Validates the skipped digits of a narrow pointer: the edge's decode bytes
/// for a child at `child_level` must equal the key digits at levels
/// `child_level + 1 ..= level`.
#[inline]
pub(crate) fn decode_matches(edge: &Edge, key: Key, child_level: u8, level: u8) -> bool {
    debug_assert!(child_level <= level && level <= 7);
    let decode = edge.decode_bytes(child_level);
    let mut lv = child_level + 1;
    while lv <= level {
        if decode[(lv - child_level - 1) as usize] != digit(key, lv) {
            return false;
        }
        lv += 1;
    }
    true
}

/// Matches `key` against the packed keys of an immediate edge and returns the
/// slot of the match, if any. `payload` holds `im.key_count()` keys of
/// `im.key_bytes()` bytes each, sorted, each key little-endian.
#[inline(always)]
fn immed_find(im: ImmedType, payload: &[u8], key: Key) -> Option<usize> {
    // Width-monomorphized like `leaf::search_fixed`. At a runtime width
    // the slice comparison lowers to a `memcmp` **call** — through a PLT
    // stub, once per stored key — and this is the terminal form roughly
    // two thirds of random-key lookups land on, i.e. the hottest
    // comparison in the library (issue #1).
    match im.key_bytes() {
        1 => immed_find_fixed::<1>(im, payload, key),
        2 => immed_find_fixed::<2>(im, payload, key),
        3 => immed_find_fixed::<3>(im, payload, key),
        4 => immed_find_fixed::<4>(im, payload, key),
        5 => immed_find_fixed::<5>(im, payload, key),
        6 => immed_find_fixed::<6>(im, payload, key),
        _ => immed_find_fixed::<7>(im, payload, key),
    }
}

/// Scans an immediate's packed keys at a compile-time width.
///
/// The single-key case — the common terminal for sparse and random keys —
/// collapses to a masked integer compare with no loop at all.
#[inline(always)]
fn immed_find_fixed<const KB: usize>(im: ImmedType, payload: &[u8], key: Key) -> Option<usize> {
    let n = im.key_count() as usize;
    debug_assert!(payload.len() >= n * KB);
    let le = key.to_le_bytes();
    let mut needle = [0u8; KB];
    needle.copy_from_slice(&le[..KB]);
    // Same idiom as `leaf::search_fixed`, and for the same reason. The
    // `chunks_exact(KB).take(n)` version this replaces yielded `&[u8]`,
    // so `candidate == needle` was a **slice**-to-array comparison — a
    // length check plus slice-equality machinery, under two iterator
    // adapters. Casting to `[u8; KB]` first makes it an array-to-array
    // compare of statically known width, which lowers to a word compare
    // with no length test.
    //
    // SAFETY: `[u8; KB]` has alignment 1, and the edge's tag guarantees
    // `n * KB` payload bytes (debug-asserted above) — exactly `n` such
    // arrays.
    let packed = unsafe { core::slice::from_raw_parts(payload.as_ptr().cast::<[u8; KB]>(), n) };
    packed.iter().position(|candidate| *candidate == needle)
}

/// The shared descent; `MAP` selects map flavor (values) over set flavor.
///
/// # Safety
///
/// Every pointer-tagged edge reachable from `edge` must point to a live,
/// well-formed node of the type its tag names, with subarray/value pointers
/// consistent with their bitmaps and counts (the tree invariants that the
/// mutation engine maintains and `docs/TESTING.md`'s validator checks).
/// The shared descent body. `#[inline(always)]` is load-bearing twice
/// over: it fuses the walk into the public entries (portable path), and
/// it lets the `#[target_feature(enable = "popcnt")]` entry clones adopt
/// the feature for every bitmap rank inside (a featureless callee may
/// inline into a feature-enabled caller; the reverse is forbidden).
///
/// **Dispatch lives at the PUBLIC SIGNATURES, and both prior placements
/// were measured and regressed.** Per-operation clones of the bitmap
/// ranks: every arm up (map_get/clustered +9.4%) — each fused rank
/// became a real call. Walk-granularity clones returning the internal
/// `Lookup`: a ~30-instruction wrapper per lookup, because the caller's
/// `Lookup -> Option<u64>` remap after the call blocks the sibling-call
/// optimization. Cloning at the public signature puts the remap INSIDE
/// the clone, so the dispatcher is load + test + tail-jump.
///
#[inline(always)]
unsafe fn walk_impl<const MAP: bool>(edge: &Edge, key: Key, level: u8) -> Lookup {
    let (mut edge, mut level) = (edge, level);
    loop {
        debug_assert!((1..=8).contains(&level));
        let Some(tag) = edge.tag() else {
            debug_assert!(false, "invalid edge tag {:#04x}", edge.tag_byte());
            return Lookup::Absent;
        };
        match tag {
            EdgeTag::Structural(t) => match t {
                EdgeType::Null => return Lookup::Absent,

                EdgeType::BranchL3 => {
                    // SAFETY: pointer-tagged edge → live BranchL3.
                    let b = unsafe { &*edge.node_ptr().cast::<BranchL3>() };
                    let bl = b.hdr.level;
                    if bl < level && !decode_matches(edge, key, bl, level) {
                        return Lookup::Absent;
                    }
                    let d = digit(key, bl);
                    let Some(slot) = b.hdr.find(d) else {
                        return Lookup::Absent;
                    };
                    debug_assert!(slot < b.edges.len());
                    // SAFETY: `slot < 3` accesses a valid child edge pointer.
                    edge = unsafe { &*b.edges.as_ptr().add(slot) };
                    level = bl - 1;
                }

                EdgeType::BranchL7 => {
                    // SAFETY: pointer-tagged edge → live BranchL7.
                    let b = unsafe { &*edge.node_ptr().cast::<BranchL7>() };
                    let bl = b.hdr.level;
                    if bl < level && !decode_matches(edge, key, bl, level) {
                        return Lookup::Absent;
                    }
                    let d = digit(key, bl);
                    let Some(slot) = b.hdr.find(d) else {
                        return Lookup::Absent;
                    };
                    debug_assert!(slot < b.edges.len());
                    // SAFETY: `slot < 7` accesses a valid child edge pointer.
                    edge = unsafe { &*b.edges.as_ptr().add(slot) };
                    level = bl - 1;
                }

                EdgeType::BranchB => {
                    // SAFETY: pointer-tagged edge → live BranchB.
                    let b = unsafe { &*edge.node_ptr().cast::<BranchB>() };
                    let bl = b.level;
                    if bl < level && !decode_matches(edge, key, bl, level) {
                        return Lookup::Absent;
                    }
                    let d = digit(key, bl);
                    let Some(slot) = b.bitmap.test_and_subexpanse_rank(d) else {
                        return Lookup::Absent;
                    };
                    let sub = (d >> 5) as usize;
                    // SAFETY: `sub < 8` accesses a valid subarray pointer; the bit is set,
                    // so the subexpanse subarray is non-null and holds at least `subexpanse_rank + 1` edges
                    // (bitmap/subarray consistency invariant).
                    let sub_ptr = unsafe { *b.subarrays.as_ptr().add(sub) };
                    // SAFETY: slot is the verified rank inside the live subexpanse subarray.
                    edge = unsafe { &*sub_ptr.add(slot) };
                    level = bl - 1;
                }

                EdgeType::BranchU => {
                    let mut b_ptr = edge.node_ptr().cast::<BranchU>();
                    loop {
                        let d = digit(key, level);
                        // SAFETY: pointer-tagged edge → live BranchU with 256 edges.
                        let next_edge = unsafe { &*(*b_ptr).edges.as_ptr().add(d as usize) };
                        level -= 1;
                        let next_tag = next_edge.tag_byte();
                        if next_tag == EdgeType::BranchU as u8 {
                            b_ptr = next_edge.node_ptr().cast::<BranchU>();
                        } else if next_tag == EdgeType::LeafB1 as u8 {
                            // Fast path: direct terminal LeafB1 lookup from uncompressed branch.
                            if level > 1 && !decode_matches(next_edge, key, 1, level) {
                                return Lookup::Absent;
                            }
                            let d1 = digit(key, 1);
                            if MAP {
                                // SAFETY: pointer-tagged edge → live LeafBitmapL.
                                let l = unsafe { &*next_edge.node_ptr().cast::<LeafBitmapL>() };
                                let sub = (d1 >> 5) as usize;
                                let Some(slot) = l.bitmap.test_and_subexpanse_rank(d1) else {
                                    return Lookup::Absent;
                                };
                                // SAFETY: `sub < 8` accesses a valid values subarray pointer.
                                let vals = unsafe { *l.values.as_ptr().add(sub) };
                                // SAFETY: slot is verified rank in live subexpanse values.
                                return Lookup::Value(unsafe { *vals.add(slot) });
                            }
                            // SAFETY: pointer-tagged edge → live LeafBitmap1.
                            let l = unsafe { &*next_edge.node_ptr().cast::<LeafBitmap1>() };
                            return if l.bitmap.test(d1) {
                                Lookup::Present
                            } else {
                                Lookup::Absent
                            };
                        } else {
                            edge = next_edge;
                            break;
                        }
                    }
                }

                EdgeType::LeafB1 => {
                    if level > 1 && !decode_matches(edge, key, 1, level) {
                        return Lookup::Absent;
                    }
                    let d = digit(key, 1);
                    if MAP {
                        // SAFETY: pointer-tagged edge → live LeafBitmapL.
                        let l = unsafe { &*edge.node_ptr().cast::<LeafBitmapL>() };
                        let sub = (d >> 5) as usize;
                        let Some(slot) = l.bitmap.test_and_subexpanse_rank(d) else {
                            return Lookup::Absent;
                        };
                        // SAFETY: `sub < 8` accesses a valid values subarray pointer.
                        let vals = unsafe { *l.values.as_ptr().add(sub) };
                        // SAFETY: the bit is set, so the value subarray is
                        // non-null and holds at least `slot + 1` values.
                        return Lookup::Value(unsafe { *vals.add(slot) });
                    }
                    // SAFETY: pointer-tagged edge → live LeafBitmap1.
                    let l = unsafe { &*edge.node_ptr().cast::<LeafBitmap1>() };
                    return if l.bitmap.test(d) {
                        Lookup::Present
                    } else {
                        Lookup::Absent
                    };
                }

                EdgeType::Leaf1
                | EdgeType::Leaf2
                | EdgeType::Leaf3
                | EdgeType::Leaf4
                | EdgeType::Leaf5
                | EdgeType::Leaf6
                | EdgeType::Leaf7 => {
                    let lf = t.leaf_key_bytes().expect("linear-leaf tag");
                    if level > lf && !decode_matches(edge, key, lf, level) {
                        return Lookup::Absent;
                    }
                    let pop = edge.pop0(lf) as usize + 1;
                    let base = edge.node_ptr();
                    if MAP {
                        // SAFETY: map leaves are one live allocation of
                        // `pop` values followed by the packed keys
                        // (leaf::size_map), per the tree contract.
                        let keys = unsafe { base.add(leaf::map_keys_offset(pop)) };
                        // SAFETY: `keys` spans `pop * lf` readable bytes.
                        let Some(slot) = (unsafe { leaf::search(keys, pop, lf, key) }) else {
                            return Lookup::Absent;
                        };
                        // SAFETY: slot < pop values live at the base.
                        return Lookup::Value(unsafe { *base.cast::<u64>().add(slot) });
                    }
                    // SAFETY: set leaves are `pop * lf` readable key bytes.
                    let found = unsafe { leaf::search(base, pop, lf, key) };
                    return if found.is_some() {
                        Lookup::Present
                    } else {
                        Lookup::Absent
                    };
                }

                EdgeType::FullExpanse => {
                    debug_assert!(!MAP, "full-expanse edges are set-flavor only");
                    return Lookup::Present;
                }
            },

            EdgeTag::Immed(im) => {
                debug_assert_eq!(
                    im.key_bytes(),
                    level,
                    "an immediate's key size is its level"
                );
                if MAP {
                    let kb = im.key_bytes() as usize;
                    let n = im.key_count() as usize;
                    debug_assert!(kb * n <= 7, "map immediates keep keys in aux");
                    let Some(slot) = immed_find(im, edge.aux_bytes(), key) else {
                        return Lookup::Absent;
                    };
                    return if n == 1 {
                        Lookup::Value(u64::from_le_bytes(edge.imm_bytes()))
                    } else {
                        let vals = edge.node_ptr().cast::<u64>();
                        // SAFETY: multi-key map immediates store a pointer
                        // to a live array of `key_count` values in word 0.
                        Lookup::Value(unsafe { *vals.add(slot) })
                    };
                }
                let payload = edge.imm_payload();
                return if immed_find(im, &payload, key).is_some() {
                    Lookup::Present
                } else {
                    Lookup::Absent
                };
            }
        }
    }
}

/// Tests membership of `key` in a set-flavor subtree (`ExpanseSet`;
/// compat: Judy1) rooted at `edge`, which covers `level` undecoded key
/// bytes.
///
/// # Safety
///
/// The tree reachable from `edge` must satisfy the structural invariants
/// described on the module: every pointer-tagged edge references a live,
/// well-formed node of its tagged type.
#[inline]
#[must_use]
pub unsafe fn test_set(edge: &Edge, key: Key, level: u8) -> bool {
    #[cfg(all(target_arch = "x86_64", not(target_feature = "popcnt")))]
    // SAFETY: contracts forwarded; `available()` gates the popcnt clone.
    unsafe {
        // BOTH arms are outlined tail calls with identical signatures, so
        // the dispatcher owns no frame. The asymmetric form (portable
        // body inlined here, clone called) was checked in the emitted
        // asm and pushes seven registers BEFORE the dispatch test — the
        // popcnt path paid the portable path's prologue.
        if crate::bits::popcnt_rt::available() {
            test_set_popcnt(edge, key, level)
        } else {
            test_set_swar(edge, key, level)
        }
    }
    #[cfg(not(all(target_arch = "x86_64", not(target_feature = "popcnt"))))]
    {
        // No dispatch on this target: the walk fuses directly.
        // SAFETY: forwarded caller contract.
        let r = unsafe { walk_impl::<false>(edge, key, level) };
        matches!(r, Lookup::Present)
    }
}

/// The portable half of [`test_set`]'s dispatch pair. `inline(never)`
/// keeps it out of the dispatcher, preserving the frameless
/// load+test+jmp shape there.
///
/// # Safety
///
/// Same contract as [`test_set`].
#[cfg(all(target_arch = "x86_64", not(target_feature = "popcnt")))]
#[inline(never)]
unsafe fn test_set_swar(edge: &Edge, key: Key, level: u8) -> bool {
    // SAFETY: forwarded caller contract.
    let r = unsafe { walk_impl::<false>(edge, key, level) };
    matches!(r, Lookup::Present)
}

/// [`test_set`] compiled with `popcnt`, result-shaping included — see
/// `walk_impl` for why the boundary sits at the public signature.
///
/// # Safety
///
/// Same contract as [`test_set`]; the CPU must support `popcnt`.
#[cfg(all(target_arch = "x86_64", not(target_feature = "popcnt")))]
#[target_feature(enable = "popcnt")]
unsafe fn test_set_popcnt(edge: &Edge, key: Key, level: u8) -> bool {
    // SAFETY: contract forwarded; the `inline(always)` body adopts the
    // feature, bitmap ranks included.
    let r = unsafe { walk_impl::<false>(edge, key, level) };
    matches!(r, Lookup::Present)
}

/// Retrieves the value of `key` from a map-flavor subtree (`ExpanseMap`;
/// compat: JudyL) rooted at `edge`, which covers `level` undecoded key
/// bytes.
///
/// # Safety
///
/// Same contract as [`test_set`].
#[inline]
#[must_use]
pub unsafe fn get_map(edge: &Edge, key: Key, level: u8) -> Option<u64> {
    #[cfg(all(target_arch = "x86_64", not(target_feature = "popcnt")))]
    // SAFETY: contracts forwarded — see `test_set` for the shape.
    unsafe {
        if crate::bits::popcnt_rt::available() {
            get_map_popcnt(edge, key, level)
        } else {
            get_map_swar(edge, key, level)
        }
    }
    #[cfg(not(all(target_arch = "x86_64", not(target_feature = "popcnt"))))]
    {
        // SAFETY: forwarded caller contract.
        match unsafe { walk_impl::<true>(edge, key, level) } {
            Lookup::Value(v) => Some(v),
            _ => None,
        }
    }
}

/// The portable half of [`get_map`]'s dispatch pair — see
/// [`test_set_swar`].
///
/// # Safety
///
/// Same contract as [`get_map`].
#[cfg(all(target_arch = "x86_64", not(target_feature = "popcnt")))]
#[inline(never)]
unsafe fn get_map_swar(edge: &Edge, key: Key, level: u8) -> Option<u64> {
    // SAFETY: forwarded caller contract.
    match unsafe { walk_impl::<true>(edge, key, level) } {
        Lookup::Value(v) => Some(v),
        _ => None,
    }
}

/// [`get_map`] compiled with `popcnt` — see [`test_set_popcnt`].
///
/// # Safety
///
/// Same contract as [`get_map`]; the CPU must support `popcnt`.
#[cfg(all(target_arch = "x86_64", not(target_feature = "popcnt")))]
#[target_feature(enable = "popcnt")]
unsafe fn get_map_popcnt(edge: &Edge, key: Key, level: u8) -> Option<u64> {
    // SAFETY: contract forwarded.
    match unsafe { walk_impl::<true>(edge, key, level) } {
        Lookup::Value(v) => Some(v),
        _ => None,
    }
}

/// Locates the **writable value slot** of `key` in a map-flavor subtree —
/// the pointer the compat layer's `JudyLIns`/`JudyLGet`/`JudyLFirst`
/// family hands to C callers, valid until the next structural mutation
/// (the classic JudyL slot contract).
///
/// The walk descends through raw pointers (node pointers and stored
/// subarray pointers), never deriving the returned slot from a shared
/// reference, so writes through it are sound.
///
/// # Safety
///
/// Same tree contract as [`get_map`]; `edge` must point at the live root
/// edge of a map-flavor subtree owned by the caller.
pub(crate) unsafe fn locate_slot(
    edge: *mut Edge,
    key: Key,
    level: u8,
) -> Option<core::ptr::NonNull<u64>> {
    #[cfg(all(target_arch = "x86_64", not(target_feature = "popcnt")))]
    // SAFETY: contracts forwarded — see `test_set` for the shape.
    unsafe {
        if crate::bits::popcnt_rt::available() {
            locate_slot_popcnt(edge, key, level)
        } else {
            locate_slot_swar(edge, key, level)
        }
    }
    #[cfg(not(all(target_arch = "x86_64", not(target_feature = "popcnt"))))]
    // SAFETY: contract forwarded.
    unsafe {
        locate_slot_impl(edge, key, level)
    }
}

/// The portable half of [`locate_slot`]'s dispatch pair — see
/// [`test_set_swar`].
///
/// # Safety
///
/// Same contract as [`locate_slot`].
#[cfg(all(target_arch = "x86_64", not(target_feature = "popcnt")))]
#[inline(never)]
unsafe fn locate_slot_swar(
    edge: *mut Edge,
    key: Key,
    level: u8,
) -> Option<core::ptr::NonNull<u64>> {
    // SAFETY: contract forwarded.
    unsafe { locate_slot_impl(edge, key, level) }
}

/// `locate_slot` compiled with `popcnt` — same dispatch story as
/// [`walk_popcnt`], for the C-ABI walk.
///
/// # Safety
///
/// Same contract as [`locate_slot`]; the CPU must support `popcnt`.
#[cfg(all(target_arch = "x86_64", not(target_feature = "popcnt")))]
#[target_feature(enable = "popcnt")]
unsafe fn locate_slot_popcnt(
    edge: *mut Edge,
    key: Key,
    level: u8,
) -> Option<core::ptr::NonNull<u64>> {
    // SAFETY: contract forwarded; the `inline(always)` body adopts the
    // feature.
    unsafe { locate_slot_impl(edge, key, level) }
}

/// The portable body — see [`locate_slot`].
///
/// # Safety
///
/// Same contract as [`locate_slot`].
#[inline(always)]
unsafe fn locate_slot_impl(
    edge: *mut Edge,
    key: Key,
    level: u8,
) -> Option<core::ptr::NonNull<u64>> {
    let (mut edge, mut level) = (edge, level);
    loop {
        debug_assert!((1..=8).contains(&level));
        // SAFETY: live edge per contract (shared reborrow used for reads
        // only; the returned slot pointer never derives from it).
        let tag = unsafe { (*edge).tag() }.expect("valid edge tag");
        match tag {
            EdgeTag::Structural(EdgeType::Null) => return None,

            EdgeTag::Immed(im) => {
                debug_assert_eq!(im.key_bytes(), level);
                // Same in-place scan as `descend`'s map branch. This used
                // to materialize every stored key into a stack buffer
                // (`immed_map_keys`: up to 15 byte-copies + widenings) and
                // then binary-search the buffer — strictly more work than
                // scanning the packed bytes where they lie, on the
                // terminal form ~2/3 of random-key lookups reach. The two
                // descents had drifted; `descend` got the fast idiom and
                // the C-ABI path kept the slow one.
                // SAFETY: live map immediate per contract; keys live in
                // the aux bytes.
                let slot = immed_find(im, unsafe { (*edge).aux_bytes() }, key)?;
                return core::ptr::NonNull::new(if im.key_count() == 1 {
                    // Word 0 (offset 0 of the edge) is the value itself.
                    edge.cast::<u64>()
                } else {
                    // SAFETY: live value array of key_count values.
                    unsafe { (*edge).node_ptr().cast::<u64>().add(slot) }
                });
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
                let kb = t.leaf_key_bytes().expect("leaf tag");
                // SAFETY: reads of the live edge/leaf per contract.
                unsafe {
                    if level > kb && !decode_matches(&*edge, key, kb, level) {
                        return None;
                    }
                    let pop = (*edge).pop0(kb) as usize + 1;
                    let base = (*edge).node_ptr();
                    let keys = base.add(leaf::map_keys_offset(pop));
                    let slot = leaf::search(keys, pop, kb, key)?;
                    return core::ptr::NonNull::new(base.cast::<u64>().add(slot));
                }
            }

            EdgeTag::Structural(EdgeType::LeafB1) => {
                // SAFETY: reads of the live edge/leaf per contract; the
                // slot pointer derives from the stored subarray pointer.
                unsafe {
                    if level > 1 && !decode_matches(&*edge, key, 1, level) {
                        return None;
                    }
                    let d = digit(key, 1);
                    let sub = (d >> 5) as usize;
                    let node = (*edge).node_ptr().cast::<LeafBitmapL>();
                    let rank = (*node).bitmap.test_and_subexpanse_rank(d)?;
                    let vals = *(*node).values.as_ptr().add(sub);
                    return core::ptr::NonNull::new(vals.add(rank));
                }
            }

            EdgeTag::Structural(EdgeType::FullExpanse) => {
                unreachable!("full-expanse edges are set-flavor only")
            }

            EdgeTag::Structural(EdgeType::BranchL3) => {
                // SAFETY: live branch per contract; the child edge pointer
                // derives from the raw node pointer.
                unsafe {
                    let b = (*edge).node_ptr().cast::<BranchL3>();
                    let bl = (*b).hdr.level;
                    if bl < level && !decode_matches(&*edge, key, bl, level) {
                        return None;
                    }
                    let d = digit(key, bl);
                    let slot = (*b).hdr.find(d)?;
                    edge = (*b).edges.as_mut_ptr().add(slot);
                    level = bl - 1;
                }
            }

            EdgeTag::Structural(EdgeType::BranchL7) => {
                // SAFETY: live branch per contract; the child edge pointer
                // derives from the raw node pointer.
                unsafe {
                    let b = (*edge).node_ptr().cast::<BranchL7>();
                    let bl = (*b).hdr.level;
                    if bl < level && !decode_matches(&*edge, key, bl, level) {
                        return None;
                    }
                    let d = digit(key, bl);
                    let slot = (*b).hdr.find(d)?;
                    edge = (*b).edges.as_mut_ptr().add(slot);
                    level = bl - 1;
                }
            }

            EdgeTag::Structural(EdgeType::BranchB) => {
                // SAFETY: live BranchB per contract; child from the stored
                // subarray pointer.
                unsafe {
                    let b = (*edge).node_ptr().cast::<BranchB>();
                    let bl = (*b).level;
                    if bl < level && !decode_matches(&*edge, key, bl, level) {
                        return None;
                    }
                    let d = digit(key, bl);
                    let rank = (*b).bitmap.test_and_subexpanse_rank(d)?;
                    let sub = (d >> 5) as usize;
                    let sub_ptr = *(*b).subarrays.as_ptr().add(sub);
                    edge = sub_ptr.add(rank);
                    level = bl - 1;
                }
            }

            EdgeTag::Structural(EdgeType::BranchU) => {
                // SAFETY: pointer-tagged edge → live BranchU.
                let mut b_ptr = unsafe { (*edge).node_ptr().cast::<BranchU>() };
                loop {
                    let d = digit(key, level);
                    // SAFETY: live BranchU per contract; edges array has 256 edges.
                    let next_edge = unsafe { (*b_ptr).edges.as_mut_ptr().add(d as usize) };
                    level -= 1;
                    // SAFETY: read tag byte of live child edge.
                    let next_tag = unsafe { (*next_edge).tag_byte() };
                    if next_tag == EdgeType::BranchU as u8 {
                        // SAFETY: pointer-tagged edge → live BranchU.
                        b_ptr = unsafe { (*next_edge).node_ptr().cast::<BranchU>() };
                    } else if next_tag == EdgeType::LeafB1 as u8 {
                        // Fast path: direct terminal LeafB1 slot lookup from uncompressed branch.
                        // SAFETY: reads of live next_edge per contract.
                        unsafe {
                            if level > 1 && !decode_matches(&*next_edge, key, 1, level) {
                                return None;
                            }
                        }
                        let d1 = digit(key, 1);
                        let sub = (d1 >> 5) as usize;
                        // SAFETY: next_edge is pointer-tagged live LeafBitmapL.
                        let node = unsafe { (*next_edge).node_ptr().cast::<LeafBitmapL>() };
                        // SAFETY: node is valid live pointer.
                        let rank = unsafe { (*node).bitmap.test_and_subexpanse_rank(d1)? };
                        // SAFETY: sub < 8 accesses valid values subarray pointer.
                        let vals = unsafe { *(*node).values.as_ptr().add(sub) };
                        // SAFETY: rank is verified slot in live subexpanse values.
                        return core::ptr::NonNull::new(unsafe { vals.add(rank) });
                    } else {
                        edge = next_edge;
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{BRANCH_L3_CAP, BRANCH_L7_CAP};
    use std::collections::{BTreeMap, BTreeSet};

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

    /// Differentially probes a set-flavor tree against its BTreeSet model:
    /// every model key, systematic perturbations of each, and a seeded
    /// random sweep masked to the tree's expanse.
    fn probe_set(root: &Edge, level: u8, model: &BTreeSet<u64>, expanse_mask: u64) {
        let probes = build_probes(model.iter().copied(), expanse_mask);
        for key in probes {
            // SAFETY: tests build well-formed trees over live locals.
            let got = unsafe { test_set(root, key, level) };
            assert_eq!(got, model.contains(&key), "key={key:#018x}");
        }
    }

    /// Differential probe for map-flavor trees against a BTreeMap model.
    fn probe_map(root: &Edge, level: u8, model: &BTreeMap<u64, u64>, expanse_mask: u64) {
        let probes = build_probes(model.keys().copied(), expanse_mask);
        for key in probes {
            // SAFETY: tests build well-formed trees over live locals.
            let got = unsafe { get_map(root, key, level) };
            assert_eq!(got, model.get(&key).copied(), "key={key:#018x}");
        }
    }

    fn build_probes(present: impl Iterator<Item = u64>, expanse_mask: u64) -> BTreeSet<u64> {
        let mut probes = BTreeSet::new();
        for k in present {
            probes.insert(k);
            probes.insert(k.wrapping_add(1) & expanse_mask);
            probes.insert(k.wrapping_sub(1) & expanse_mask);
            for byte in 0..8 {
                probes.insert((k ^ (0xFF << (byte * 8))) & expanse_mask);
                probes.insert((k ^ (0x01 << (byte * 8))) & expanse_mask);
            }
        }
        let mut rng = XorShift(0xFEED_FACE_0123_4567);
        for _ in 0..500 {
            probes.insert(rng.next() & expanse_mask);
        }
        probes.insert(0);
        probes.insert(expanse_mask);
        probes
    }

    fn bitmap1_from(keys: impl Iterator<Item = u8>) -> LeafBitmap1 {
        let mut leaf = LeafBitmap1::new();
        for k in keys {
            leaf.bitmap.set(k);
        }
        leaf
    }

    #[test]
    fn null_and_leafb1_direct() {
        let model: BTreeSet<u64> = [0x00u64, 0x07, 0x1F, 0x20, 0x80, 0xFF].into();
        let mut leaf = bitmap1_from(model.iter().map(|&k| k as u8));
        let root = Edge::new_node((&raw mut leaf).cast(), EdgeType::LeafB1.as_u8());
        probe_set(&root, 1, &model, 0xFF);
        probe_set(&Edge::NULL, 1, &BTreeSet::new(), 0xFF);
    }

    #[test]
    fn immediates_set_flavor() {
        // 1-byte keys, the full 15-key capacity.
        let keys1: Vec<u64> = (0..15u64).map(|i| i * 17).collect();
        let mut edge = Edge::NULL;
        let im = ImmedType::new(1, 15).unwrap();
        edge.set_tag(im.as_u8());
        let mut payload = [0u8; 15];
        for (slot, &k) in keys1.iter().enumerate() {
            payload[slot] = k as u8;
        }
        edge.set_imm_bytes(payload[..8].try_into().unwrap());
        edge.set_aux_bytes(payload[8..].try_into().unwrap());
        let model: BTreeSet<u64> = keys1.iter().copied().collect();
        probe_set(&edge, 1, &model, 0xFF);

        // 3-byte keys, 5 of them (payload spans word 0 into aux).
        let keys3: Vec<u64> = vec![0x000001, 0x0000FF, 0x00AB12, 0x7F0000, 0xFFFFFF];
        let im = ImmedType::new(3, 5).unwrap();
        let mut edge = Edge::NULL;
        edge.set_tag(im.as_u8());
        let mut payload = [0u8; 15];
        for (slot, &k) in keys3.iter().enumerate() {
            payload[slot * 3..slot * 3 + 3].copy_from_slice(&k.to_le_bytes()[..3]);
        }
        edge.set_imm_bytes(payload[..8].try_into().unwrap());
        edge.set_aux_bytes(payload[8..].try_into().unwrap());
        let model: BTreeSet<u64> = keys3.iter().copied().collect();
        probe_set(&edge, 3, &model, 0xFF_FFFF);

        // 7-byte keys, 2 of them.
        let keys7: Vec<u64> = vec![0x00DE_AD00_BEEF_0001, 0x00FF_FFFF_FFFF_FFFF];
        let im = ImmedType::new(7, 2).unwrap();
        let mut edge = Edge::NULL;
        edge.set_tag(im.as_u8());
        let mut payload = [0u8; 15];
        for (slot, &k) in keys7.iter().enumerate() {
            payload[slot * 7..slot * 7 + 7].copy_from_slice(&k.to_le_bytes()[..7]);
        }
        edge.set_imm_bytes(payload[..8].try_into().unwrap());
        edge.set_aux_bytes(payload[8..].try_into().unwrap());
        let model: BTreeSet<u64> = keys7.iter().copied().collect();
        probe_set(&edge, 7, &model, 0x00FF_FFFF_FFFF_FFFF);
    }

    #[test]
    fn linear_branches_over_bitmap_leaves() {
        // Level-2 expanse (16-bit keys): BranchL3 and BranchL7 over
        // LeafBitmap1 children, exercising header digit search.
        let digits_hi: [u8; 7] = [0x00, 0x13, 0x42, 0x77, 0xA0, 0xC3, 0xFF];
        let lows: [&[u8]; 7] = [
            &[0x00],
            &[0x01, 0x02, 0x03],
            &[0xFF],
            &[0x10, 0x20, 0x30, 0x40],
            &[0x00, 0xFF],
            &[0x55],
            &[0xFE, 0xFF],
        ];
        let mut leaves: Vec<LeafBitmap1> = lows
            .iter()
            .map(|ls| bitmap1_from(ls.iter().copied()))
            .collect();
        let mut model = BTreeSet::new();
        for (i, &hi) in digits_hi.iter().enumerate() {
            for &lo in lows[i] {
                model.insert((u64::from(hi) << 8) | u64::from(lo));
            }
        }

        // One base raw pointer for every element: re-indexing `leaves[i]`
        // per wire-up would mint a fresh `&mut` of the Vec each time and
        // invalidate the previously derived raw pointers (Miri catches
        // this as a Stacked Borrows violation).
        let base = leaves.as_mut_ptr();

        // BranchL7 with all 7 children.
        let mut b7 = BranchL7::new(2);
        b7.hdr.num = 7;
        b7.hdr.digits[..7].copy_from_slice(&digits_hi);
        for i in 0..7 {
            // SAFETY: `base.add(i)` stays inside the 7-element allocation.
            let leaf = unsafe { base.add(i) };
            b7.edges[i] = Edge::new_node(leaf.cast(), EdgeType::LeafB1.as_u8());
        }
        let root = Edge::new_node((&raw mut b7).cast(), EdgeType::BranchL7.as_u8());
        assert_eq!(BRANCH_L7_CAP, 7);
        probe_set(&root, 2, &model, 0xFFFF);

        // BranchL3 with the first 3 children only.
        let mut b3 = BranchL3::new(2);
        b3.hdr.num = 3;
        b3.hdr.digits[..3].copy_from_slice(&digits_hi[..3]);
        let mut model3 = BTreeSet::new();
        for (i, &hi) in digits_hi.iter().take(3).enumerate() {
            // SAFETY: `base.add(i)` stays inside the 7-element allocation.
            let leaf = unsafe { base.add(i) };
            b3.edges[i] = Edge::new_node(leaf.cast(), EdgeType::LeafB1.as_u8());
            for &lo in lows[i] {
                model3.insert((u64::from(hi) << 8) | u64::from(lo));
            }
        }
        let root = Edge::new_node((&raw mut b3).cast(), EdgeType::BranchL3.as_u8());
        assert_eq!(BRANCH_L3_CAP, 3);
        probe_set(&root, 2, &model3, 0xFFFF);
    }

    #[test]
    fn bitmap_branch_over_leaves_and_immediates() {
        // Level-2 expanse: BranchB with populated digits spread across all
        // eight 32-digit subexpanses, children = bitmap leaves + immediates.
        let hi_digits: [u8; 10] = [0x00, 0x1F, 0x20, 0x3F, 0x55, 0x7A, 0x9C, 0xBB, 0xDD, 0xFF];
        let mut model = BTreeSet::new();
        let mut leaves: Vec<LeafBitmap1> = Vec::new();
        let mut child_jps: Vec<Edge> = Vec::new();
        for (i, &hi) in hi_digits.iter().enumerate() {
            if i % 2 == 0 {
                // Bitmap-leaf child with a few low bytes.
                let lows = [hi ^ 0x0F, hi ^ 0xF0, 0x00];
                leaves.push(bitmap1_from(lows.iter().copied()));
                for &lo in &lows {
                    model.insert((u64::from(hi) << 8) | u64::from(lo));
                }
            } else {
                // Immediate child: two 1-byte keys.
                let (a, b) = (hi ^ 0x01, hi ^ 0x80);
                let im = ImmedType::new(1, 2).unwrap();
                let mut edge = Edge::NULL;
                edge.set_tag(im.as_u8());
                let (lo, hi_b) = if a <= b { (a, b) } else { (b, a) };
                edge.set_imm_bytes([lo, hi_b, 0, 0, 0, 0, 0, 0]);
                child_jps.push(edge);
                model.insert((u64::from(hi) << 8) | u64::from(lo));
                model.insert((u64::from(hi) << 8) | u64::from(hi_b));
            }
        }
        // Wire leaf pointers now that `leaves` will no longer reallocate.
        let mut leaf_iter = leaves.iter_mut();
        let mut imm_iter = child_jps.iter();
        let mut b = BranchB::new(2);
        let mut per_sub: [Vec<Edge>; 8] = Default::default();
        for (i, &hi) in hi_digits.iter().enumerate() {
            b.bitmap.set(hi);
            let edge = if i % 2 == 0 {
                let leaf = leaf_iter.next().unwrap();
                Edge::new_node((&raw mut *leaf).cast(), EdgeType::LeafB1.as_u8())
            } else {
                *imm_iter.next().unwrap()
            };
            per_sub[(hi >> 5) as usize].push(edge);
        }
        for (sub, edges) in per_sub.iter_mut().enumerate() {
            if !edges.is_empty() {
                b.pop_counts[sub] = edges.len() as u16;
                b.subarrays[sub] = edges.as_mut_ptr();
            }
        }
        let root = Edge::new_node((&raw mut b).cast(), EdgeType::BranchB.as_u8());
        probe_set(&root, 2, &model, 0xFFFF);
    }

    #[test]
    fn uncompressed_branch_full_expanse_and_deep_chain() {
        // Level-3 expanse: BranchU at level 3 → children per top digit:
        // a full-expanse subexpanse, a BranchU level 2 → bitmap leaf, and
        // a narrow pointer skipping level 2 straight to a bitmap leaf.
        let mut model = BTreeSet::new();

        let mut leaf_a = bitmap1_from([0x01u8, 0x80].into_iter());
        model.insert(0x05_33_01);
        model.insert(0x05_33_80);
        let mut mid = BranchU::new();
        mid.edges[0x33] = Edge::new_node((&raw mut leaf_a).cast(), EdgeType::LeafB1.as_u8());

        // Narrow pointer: keys 0x77_C4_xx — LeafB1 child of the level-3
        // root with decode byte for level 2 = 0xC4.
        let mut leaf_b = bitmap1_from([0x00u8, 0x42, 0xFF].into_iter());
        let mut skip_jp = Edge::new_node((&raw mut leaf_b).cast(), EdgeType::LeafB1.as_u8());
        skip_jp.set_decode_bytes(1, &[0xC4]);
        for lo in [0x00u64, 0x42, 0xFF] {
            model.insert(0x77_C4_00 | lo);
        }

        let mut root_u = BranchU::new();
        root_u.edges[0x05] = Edge::new_node((&raw mut mid).cast(), EdgeType::BranchU.as_u8());
        root_u.edges[0x77] = skip_jp;
        // Full expanse under top digit 0xAA: all 65536 keys present.
        root_u.edges[0xAA] = {
            let mut edge = Edge::NULL;
            edge.set_tag(EdgeType::FullExpanse.as_u8());
            edge
        };
        for k in 0xAA_0000u64..=0xAA_FFFF {
            if k % 977 == 0 || k == 0xAA_0000 || k == 0xAA_FFFF {
                model.insert(k);
            }
        }

        let root = Edge::new_node((&raw mut root_u).cast(), EdgeType::BranchU.as_u8());
        let probes = build_probes(model.iter().copied(), 0xFF_FFFF);
        for key in probes {
            // SAFETY: well-formed hand-built tree over live locals.
            let got = unsafe { test_set(&root, key, 3) };
            let expected = model.contains(&key) || (key >> 16) == 0xAA;
            assert_eq!(got, expected, "key={key:#08x}");
        }
        // Narrow-pointer mismatch: right leaf digits, wrong skipped byte.
        // SAFETY: same tree.
        unsafe {
            assert!(test_set(&root, 0x77_C4_42, 3));
            assert!(!test_set(&root, 0x77_C5_42, 3));
            assert!(!test_set(&root, 0x77_00_42, 3));
        }
    }

    #[test]
    fn linear_leaves_via_allocator() {
        use crate::alloc::NodeAlloc;

        let a = NodeAlloc::new();

        // Set flavor, level-3 expanse: BranchL7 → Leaf2 under digit 0x21,
        // and Leaf1 under digit 0x9E with a one-byte narrow skip (0x44).
        let keys2: Vec<u64> = vec![0x0000, 0x00FF, 0x1234, 0xFFFE, 0xFFFF];
        let leaf2_size = leaf::size_set(2, keys2.len());
        let leaf2 = a.alloc_bytes(leaf2_size);
        for (slot, k) in keys2.iter().enumerate() {
            // SAFETY: in-bounds write into the fresh `leaf2_size` block.
            unsafe {
                leaf2
                    .as_ptr()
                    .add(slot * 2)
                    .copy_from_nonoverlapping(k.to_le_bytes().as_ptr(), 2);
            }
        }
        let mut e2 = Edge::new_node(leaf2.as_ptr(), EdgeType::Leaf2.as_u8());
        e2.set_pop0(2, keys2.len() as u64 - 1);

        let keys1: Vec<u64> = vec![0x00, 0x7B, 0xFF];
        let leaf1_size = leaf::size_set(1, keys1.len());
        let leaf1 = a.alloc_bytes(leaf1_size);
        for (slot, k) in keys1.iter().enumerate() {
            // SAFETY: in-bounds write into the fresh `leaf1_size` block.
            unsafe { *leaf1.as_ptr().add(slot) = *k as u8 };
        }
        let mut e1 = Edge::new_node(leaf1.as_ptr(), EdgeType::Leaf1.as_u8());
        e1.set_pop0(1, keys1.len() as u64 - 1);
        e1.set_decode_bytes(1, &[0x44]);

        let mut b7 = BranchL7::new(3);
        b7.hdr.num = 2;
        b7.hdr.digits[..2].copy_from_slice(&[0x21, 0x9E]);
        b7.edges[0] = e2;
        b7.edges[1] = e1;
        let root = Edge::new_node((&raw mut b7).cast(), EdgeType::BranchL7.as_u8());

        let mut set_model = BTreeSet::new();
        for &k in &keys2 {
            set_model.insert(0x21_0000 | k);
        }
        for &k in &keys1 {
            set_model.insert(0x9E_4400 | k);
        }
        probe_set(&root, 3, &set_model, 0xFF_FFFF);
        // Skip byte must be validated.
        // SAFETY: well-formed allocator-built tree.
        unsafe {
            assert!(test_set(&root, 0x9E_447B, 3));
            assert!(!test_set(&root, 0x9E_457B, 3));
        }

        // Map flavor: a standalone level-5 map leaf (values then keys).
        let mkeys: Vec<u64> = vec![0x00_0000_0001, 0x12_3456_789A, 0xFF_FFFF_FFFF];
        let msize = leaf::size_map(5, mkeys.len());
        let mleaf = a.alloc_bytes(msize);
        for (slot, k) in mkeys.iter().enumerate() {
            // SAFETY: in-bounds writes: values first, packed keys after.
            unsafe {
                *mleaf.as_ptr().cast::<u64>().add(slot) = 1000 + slot as u64;
                mleaf
                    .as_ptr()
                    .add(leaf::map_keys_offset(mkeys.len()) + slot * 5)
                    .copy_from_nonoverlapping(k.to_le_bytes().as_ptr(), 5);
            }
        }
        let mut me = Edge::new_node(mleaf.as_ptr(), EdgeType::Leaf5.as_u8());
        me.set_pop0(5, mkeys.len() as u64 - 1);
        let mut map_model = BTreeMap::new();
        for (slot, &k) in mkeys.iter().enumerate() {
            map_model.insert(k, 1000 + slot as u64);
        }
        probe_map(&me, 5, &map_model, 0xFF_FFFF_FFFF);

        // Everything freed: the accounting and Miri's leak check agree.
        // SAFETY: freeing exactly the three allocations made above.
        unsafe {
            a.free_bytes(leaf2, leaf2_size);
            a.free_bytes(leaf1, leaf1_size);
            a.free_bytes(mleaf, msize);
        }
        assert_eq!(a.bytes_in_use(), 0);
        assert_eq!(a.live_allocs(), 0);
    }

    #[test]
    fn map_flavor_leaves_and_immediates() {
        // Level-2 expanse map: BranchL3 → LeafBitmapL, single-key
        // immediate (value in word 0), multi-key immediate (value array).
        let mut model = BTreeMap::new();

        // LeafBitmapL under digit 0x10 with values spread over subexpanses.
        let mut leaf = LeafBitmapL::new();
        let mut per_sub_vals: [Vec<u64>; 8] = Default::default();
        for &(lo, val) in &[
            (0x00u8, 111u64),
            (0x1Fu8, 222),
            (0x20u8, 333),
            (0x9Au8, 444),
            (0xFFu8, 555),
        ] {
            leaf.bitmap.set(lo);
            per_sub_vals[(lo >> 5) as usize].push(val);
            model.insert(0x10_00u64 | u64::from(lo), val);
        }
        for (sub, vals) in per_sub_vals.iter_mut().enumerate() {
            if !vals.is_empty() {
                leaf.values[sub] = vals.as_mut_ptr();
            }
        }

        // Single-key map immediate under digit 0x80: key byte in aux,
        // value in word 0.
        let mut imm1 = Edge::NULL;
        imm1.set_tag(ImmedType::new(1, 1).unwrap().as_u8());
        imm1.set_aux_bytes([0x77, 0, 0, 0, 0, 0, 0]);
        imm1.set_imm_bytes(0xDEAD_BEEF_u64.to_le_bytes());
        model.insert(0x80_77, 0xDEAD_BEEF);

        // Three-key map immediate under digit 0xC0: keys in aux, values in
        // a pointed-to array.
        let mut vals3 = [10u64, 20, 30];
        let mut imm3 = Edge::new_node(
            vals3.as_mut_ptr().cast(),
            ImmedType::new(1, 3).unwrap().as_u8(),
        );
        imm3.set_aux_bytes([0x05, 0x50, 0xAA, 0, 0, 0, 0]);
        model.insert(0xC0_05, 10);
        model.insert(0xC0_50, 20);
        model.insert(0xC0_AA, 30);

        let mut b3 = BranchL3::new(2);
        b3.hdr.num = 3;
        b3.hdr.digits[..3].copy_from_slice(&[0x10, 0x80, 0xC0]);
        b3.edges[0] = Edge::new_node((&raw mut leaf).cast(), EdgeType::LeafB1.as_u8());
        b3.edges[1] = imm1;
        b3.edges[2] = imm3;
        let root = Edge::new_node((&raw mut b3).cast(), EdgeType::BranchL3.as_u8());
        probe_map(&root, 2, &model, 0xFFFF);
    }
}
