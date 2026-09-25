//! Phase 6b: `ExpanseMap`, the public map-flavor tree (compat: JudyL).
//!
//! Root organization mirrors `ExpanseSet`: populations up to
//! [`crate::set::ROOT_LEAF_CAP`] live in a root leaf — here parallel
//! sorted-key and value arrays in one allocation — before a level-8 trie
//! exists. The tree condenses back into a root
//! leaf when its population falls one below the promotion boundary.

use crate::alloc::NodeAlloc;
#[cfg(feature = "std")]
use crate::bits::shared_word;
use crate::get;
use crate::mutate;
use crate::mutate_map;
use crate::node::Edge;
use crate::set::ROOT_LEAF_CAP;
#[cfg(all(target_pointer_width = "64", feature = "std"))]
use crate::sync::RootSnapshot;
use crate::types::Key;
use crate::validate::ExpanseStats;
use core::ptr::NonNull;
#[cfg(not(feature = "std"))]
use core_alloc::format;
#[cfg(not(feature = "std"))]
use core_alloc::string::String;

/// `repr(u64)` fixes the layout the concurrent string paths read through a
/// raw pointer (`MapCore::occ_snapshot_of`, #1086): a tag word, then the
/// variant's fields as a `repr(C)` struct. Same size and field offsets as
/// the default representation gave (24 bytes, fields at 8).
#[derive(Clone, Copy)]
#[repr(u64)]
enum Root {
    Empty = ROOT_EMPTY,
    /// One allocation: `pop` sorted keys, then `pop` values.
    Leaf {
        ptr: NonNull<u8>,
        pop: usize,
    } = ROOT_LEAF,
    /// A level-8 trie; the population lives beside the root in
    /// `MapCore::tree_pop` (the JPM role) so the enum stays a plain `Copy`
    /// word pair — an atomic inside it would make every read of the root
    /// reload through memory.
    Tree {
        top: Edge,
    } = ROOT_TREE,
}

const ROOT_EMPTY: u64 = 0;
const ROOT_LEAF: u64 = 1;
const ROOT_TREE: u64 = 2;

/// `Root::Leaf` and `Root::Tree` as the primitive representation lays them
/// out: the tag word, then the variant's fields in declaration order.
#[repr(C)]
struct RootLeafLayout {
    tag: u64,
    ptr: *mut u8,
    pop: usize,
}

#[repr(C)]
struct RootTreeLayout {
    tag: u64,
    top: Edge,
}

const _: () = {
    assert!(core::mem::size_of::<Root>() == 24);
    assert!(core::mem::size_of::<Root>() == core::mem::size_of::<RootTreeLayout>());
    assert!(core::mem::size_of::<Root>() == core::mem::size_of::<RootLeafLayout>());
    assert!(core::mem::offset_of!(RootLeafLayout, ptr) == 8);
    assert!(core::mem::offset_of!(RootLeafLayout, pop) == 16);
    assert!(core::mem::offset_of!(RootTreeLayout, top) == 8);
};

/// A string node's sub-map root, read by readers while the node's lock
/// holder stores it (#1086, class 1): the three words of `Root`, and the
/// population word beside it, as atomics. Word 1 carries a pointer in both
/// variants that use it (the leaf's allocation, the top edge's word 0) and
/// is always accessed as an `AtomicPtr`; the others as `AtomicU64`.
///
/// The tag is stored last with `Release` and loaded with `Acquire`, so a
/// reader that sees a variant's tag sees the words that variant wrote
/// before it: an empty root leaves words 1 and 2 uninitialised, and without
/// the pairing a relaxed reader could load them. Every value read here is
/// still discarded unless the node's cover word validates afterwards.
#[cfg(all(target_pointer_width = "64", feature = "std"))]
mod root_word {
    use super::{ROOT_EMPTY, ROOT_LEAF, ROOT_TREE, Root};
    use core::sync::atomic::{
        AtomicPtr, AtomicU64,
        Ordering::{Acquire, Relaxed, Release},
    };

    /// # Safety
    /// `r` points to a live `Root`, with write permission (a pointer from
    /// the node, never from `&Root`): every load forms an atomic reference.
    #[inline(always)]
    pub(super) unsafe fn tag(r: *mut Root) -> u64 {
        // SAFETY: caller contract; the tag is word 0.
        unsafe { AtomicU64::from_ptr(r.cast::<u64>()).load(Acquire) }
    }

    /// Word 1: the leaf's allocation or the top edge's word 0.
    ///
    /// # Safety
    /// As [`tag`], and the caller loaded a tag that is not `ROOT_EMPTY`.
    #[inline(always)]
    pub(super) unsafe fn ptr(r: *mut Root) -> *mut u8 {
        // SAFETY: caller contract; word 1, 8-aligned inside `Root`.
        unsafe { AtomicPtr::from_ptr(r.cast::<u64>().add(1).cast::<*mut u8>()).load(Relaxed) }
    }

    /// Word 2: the leaf's population or the top edge's aux word.
    ///
    /// # Safety
    /// As [`ptr`].
    #[inline(always)]
    pub(super) unsafe fn third(r: *mut Root) -> u64 {
        // SAFETY: caller contract; word 2.
        unsafe { AtomicU64::from_ptr(r.cast::<u64>().add(2)).load(Relaxed) }
    }

    /// Stores `root` at `r`, its tag last.
    ///
    /// # Safety
    /// As [`tag`], and the caller holds the lock that makes it the root's
    /// one writer.
    #[inline(always)]
    pub(super) unsafe fn store(r: *mut Root, root: Root) {
        let w = r.cast::<u64>();
        let (t, words) = match root {
            Root::Empty => (ROOT_EMPTY, None),
            Root::Leaf { ptr, pop } => (ROOT_LEAF, Some((ptr.as_ptr(), pop as u64))),
            Root::Tree { top } => (ROOT_TREE, Some((top.node_ptr(), top.aux_word()))),
        };
        // SAFETY: caller contract; words 1 and 2, then word 0.
        unsafe {
            if let Some((p, x)) = words {
                AtomicPtr::from_ptr(w.add(1).cast::<*mut u8>()).store(p, Relaxed);
                AtomicU64::from_ptr(w.add(2)).store(x, Relaxed);
            }
            AtomicU64::from_ptr(w).store(t, Release);
        }
    }

    /// # Safety
    /// `p` points to a live core's `tree_pop`, with write permission.
    #[inline(always)]
    pub(super) unsafe fn tree_pop(p: *const u64) -> u64 {
        // SAFETY: caller contract.
        unsafe { AtomicU64::from_ptr(p.cast_mut()).load(Relaxed) }
    }

    /// # Safety
    /// As [`tree_pop`], and the caller is the one writer.
    #[inline(always)]
    pub(super) unsafe fn store_tree_pop(p: *mut u64, v: u64) {
        // SAFETY: caller contract.
        unsafe { AtomicU64::from_ptr(p).store(v, Relaxed) }
    }
}

/// The map engine core: root organization plus every walk and mutation,
/// with **no owned allocator and no owned insert-path cache** — both are
/// passed in per call (issue #363 Step A).
///
/// [`ExpanseMap`] wraps one core with its own [`NodeAlloc`] and path
/// cache, forwarding through `#[inline(always)]` shims so the compiled
/// public paths are identical to the pre-split layout. `ExpanseStrMap`
/// embeds a bare core per sub-trie node and passes the **one allocator
/// shared across the whole string map**, which is what shrinks a
/// `StrNode` from ~700 bytes (embedded allocator + path cache) to the
/// size of this struct.
///
/// A core frees nothing on drop — it cannot, without its allocator — so
/// every owner must route teardown through [`Self::clear`] (or the
/// pathless twin) with the allocator that produced the core's nodes.
pub(crate) struct MapCore {
    root: Root,
    /// Total population while `root` is a `Tree` (the JPM role). A plain
    /// word, and on an unshared tree the population itself. A shared tree
    /// counts in its wrapper's sharded counter (`sync::ShardedTreePop`),
    /// which optimistic writers bump directly; a covered write folds the
    /// change it makes here back into that counter, so on a shared tree this
    /// field lags the population between covered writes.
    tree_pop: u64,
}

/// A sparse, dynamic map from `u64` keys to `u64` values (compat: JudyL).
///
/// Adaptive expanse-partitioned trie: memory stays near-proportional to
/// population across sequential, random, clustered, and sparse key
/// distributions, and lookups run in at most eight digit steps.
pub struct ExpanseMap {
    core: MapCore,
    alloc: NodeAlloc,
    path: core::cell::UnsafeCell<crate::mutate_map::InsertPathMap>,
}

// SAFETY: as for `ExpanseSet` — exclusive ownership of all reachable
// allocations; not `Sync`, shared access goes through `SyncExpanseMap`.
unsafe impl Send for ExpanseMap {}

// SAFETY: Scaffolding for Stage B multi-writer OLC (PR #815 engine prep).
// The OlcEngine contract specifies the protocol invariants that the PR 5 engine
// implementation fulfils; no multi-writer mutations are executed or bounded by
// this marker trait yet.
#[cfg(feature = "std")]
unsafe impl crate::occ::OlcEngine for ExpanseMap {}

/// Allocation size of a root leaf holding `pop` entries: a class-sized
/// key area followed by a class-sized value area. Class-sizing (as the
/// trie's linear leaves already do) means consecutive inserts and
/// deletes shift in place instead of reallocating on every operation —
/// without it, every map a C caller keeps under 32 entries paid a
/// malloc, a full copy and a free per insert (issue #1).
fn leaf_size(pop: usize) -> usize {
    16 * crate::leaf::cap_class(pop)
}

/// Offset of the value area inside a root leaf of `pop` entries. Keyed
/// to the capacity class, not the population, so it does not move when
/// the population changes within a class.
///
/// **This is the single definition of the root-leaf layout.** It is
/// `pub(crate)` because the concurrent read path in `sync` must use the
/// same rule: when this was duplicated there, the two copies drifted the
/// moment capacity classes arrived and readers returned a neighbouring
/// key's value (caught by `sync::tests::concurrent_readers_under_churn`
/// on aarch64). Anything that needs the value area asks here.
pub(crate) fn leaf_values_offset(pop: usize) -> usize {
    8 * crate::leaf::cap_class(pop)
}

/// Evaluates `$op` with `$this` bound to `$m`, and counts a root-state
/// change (diagnostic builds only). A macro rather than a method taking a
/// closure: the plain insert and remove bodies then reach their callers
/// through `#[inline(always)]` functions alone, and never through a closure
/// call whose inlining LLVM decides by heuristic, a decision that moves with
/// the size of unrelated code in the crate (Refs #1086).
macro_rules! noting_root_rewrite {
    ($this:expr, $m:ident => $op:expr) => {{
        let $m = $this;
        #[cfg(feature = "occ-stats")]
        let before = $m.root_fingerprint();
        let r = $op;
        #[cfg(feature = "occ-stats")]
        if $m.root_fingerprint() != before {
            crate::occ_stats::note_root_rewrite();
        }
        r
    }};
}

/// See `set::by_mode`: the three sharing modes an engine call is
/// monomorphized for, decided once per operation.
macro_rules! by_mode {
    ($alloc:expr, $obj:ident . $field:ident . $method:ident $args:tt) => {
        if $alloc.occ_enabled() {
            if $alloc.engine_covers_root() {
                $obj.$field.$method::<true, false> $args
            } else {
                $obj.$field.$method::<true, true> $args
            }
        } else {
            $obj.$field.$method::<false, false> $args
        }
    };
    ($alloc:expr, 1 $obj:ident . $field:ident . $method:ident $args:tt) => {
        if $alloc.occ_enabled() {
            $obj.$field.$method::<true> $args
        } else {
            $obj.$field.$method::<false> $args
        }
    };
    ($alloc:expr, $obj:ident . $method:ident $args:tt) => {
        if $alloc.occ_enabled() {
            if $alloc.engine_covers_root() {
                $obj.$method::<true, false> $args
            } else {
                $obj.$method::<true, true> $args
            }
        } else {
            $obj.$method::<false, false> $args
        }
    };
    ($alloc:expr, 1 $obj:ident . $method:ident $args:tt) => {
        if $alloc.occ_enabled() {
            $obj.$method::<true> $args
        } else {
            $obj.$method::<false> $args
        }
    };
    // With a leading const argument of the call's own (`KEEP`).
    ($alloc:expr, $call:ident::<$k:literal> $args:tt) => {
        if $alloc.occ_enabled() {
            if $alloc.engine_covers_root() {
                $call::<$k, true, false> $args
            } else {
                $call::<$k, true, true> $args
            }
        } else {
            $call::<$k, false, false> $args
        }
    };
    ($alloc:expr, $call:ident $args:tt) => {
        if $alloc.occ_enabled() {
            if $alloc.engine_covers_root() {
                $call::<true, false> $args
            } else {
                $call::<true, true> $args
            }
        } else {
            $call::<false, false> $args
        }
    };
}

/// The tree arm of insert, per sharing mode: the engine call and a plain
/// bump of `tree_pop` (on a shared tree the covered write folds it into the
/// wrapper's counter).
#[inline(always)]
fn tree_insert<const KEEP: bool, const OCC: bool, const NESTED: bool>(
    alloc: &NodeAlloc,
    tree_pop: &mut u64,
    path: &mut crate::mutate_map::InsertPathMap,
    top: &mut Edge,
    key: Key,
    val: u64,
) -> (Option<u64>, *mut u64) {
    // SAFETY: trie maintained/owned by this map's engine.
    let r = unsafe {
        mutate_map::map_insert_with_path::<KEEP, OCC, NESTED>(
            alloc,
            top,
            key,
            val,
            8,
            path,
            crate::occ::Cover::Tree,
        )
    };
    if r.0.is_none() {
        *tree_pop += 1;
    }
    r
}

/// The tree arm of remove, per sharing mode: `(removed value, population
/// after)`, `u64::MAX` when nothing was removed.
#[inline(always)]
fn tree_remove<const OCC: bool, const NESTED: bool>(
    alloc: &NodeAlloc,
    tree_pop: &mut u64,
    top: &mut Edge,
    key: Key,
) -> (Option<u64>, u64) {
    // SAFETY: trie maintained/owned by this map's engine.
    // A shared tree's removal is the raw-pointer copy (#1086), chosen here
    // rather than inside the plain body, which serves every plain tree.
    let old = unsafe {
        if OCC {
            mutate_map::map_remove_occ::<OCC, NESTED>(
                alloc,
                &raw mut *top,
                key,
                8,
                crate::occ::Cover::Tree,
            )
        } else {
            mutate_map::map_remove::<OCC, NESTED>(alloc, top, key, 8, crate::occ::Cover::Tree)
        }
    };
    let now = if old.is_none() {
        u64::MAX
    } else {
        *tree_pop -= 1;
        *tree_pop
    };
    (old, now)
}

/// [`MapCore::root_fingerprint`] of a root value.
#[cfg(feature = "occ-stats")]
fn root_fingerprint_of(root: &Root) -> (u8, u64, u64) {
    match root {
        Root::Empty => (0, 0, 0),
        Root::Leaf { ptr, .. } => (1, ptr.as_ptr() as u64, 0),
        Root::Tree { top, .. } => (2, top.word0(), top.aux_word()),
    }
}

/// What an insert into a root in leaf or empty state decided: the value it
/// replaced, and the root and tree population to store (`None`: unchanged).
struct LeafStateInsert {
    old: Option<u64>,
    root: Option<Root>,
    tree_pop: Option<u64>,
}

/// The insert of `key → val` into `root`, which is empty or a root leaf, as
/// a value: it allocates, frees and writes the leaf's heap areas, and returns
/// the root to store rather than storing it. One body for the owner's
/// `&mut MapCore` path and for a string node's cover holder, which stores
/// through a raw pointer because readers copy the core meanwhile (#1086).
#[inline(always)]
fn leaf_state_insert<const OCC: bool, const NESTED: bool>(
    root: Root,
    alloc: &NodeAlloc,
    key: Key,
    val: u64,
    path: &mut crate::mutate_map::InsertPathMap,
) -> LeafStateInsert {
    match root {
        Root::Empty => {
            let ptr = alloc.alloc_bytes_dispatch::<OCC>(leaf_size(1));
            // SAFETY: fresh allocation: key slot then value slot.
            unsafe {
                ptr.as_ptr().cast::<u64>().write(key);
                ptr.as_ptr()
                    .add(leaf_values_offset(1))
                    .cast::<u64>()
                    .write(val);
            }
            LeafStateInsert {
                old: None,
                root: Some(Root::Leaf { ptr, pop: 1 }),
                tree_pop: None,
            }
        }
        Root::Leaf { ptr, pop } => {
            let (keys, vals) = MapCore::leaf_parts(ptr, pop);
            let (hit, at) = if pop > 0 {
                let last = keys[pop - 1];
                if key > last {
                    (false, pop)
                } else if key == last {
                    (true, pop - 1)
                } else {
                    match keys.binary_search(&key) {
                        Ok(pos) => (true, pos),
                        Err(pos) => (false, pos),
                    }
                }
            } else {
                (false, 0)
            };
            if hit {
                // SAFETY: in-place value swap.
                unsafe {
                    let slot = vals.add(at);
                    let old = *slot;
                    slot.write(val);
                    return LeafStateInsert {
                        old: Some(old),
                        root: None,
                        tree_pop: None,
                    };
                }
            }
            if pop < ROOT_LEAF_CAP {
                if leaf_size(pop + 1) == leaf_size(pop) {
                    // Spare class capacity: shift both areas in
                    // place, no allocation and no copy of the
                    // whole leaf.
                    // SAFETY: same class, so the areas keep their
                    // offsets and the extra slot is in bounds.
                    unsafe {
                        let base = ptr.as_ptr().cast::<u64>();
                        core::ptr::copy(base.add(at), base.add(at + 1), pop - at);
                        base.add(at).write(key);
                        let v = ptr.as_ptr().add(leaf_values_offset(pop)).cast::<u64>();
                        core::ptr::copy(v.add(at), v.add(at + 1), pop - at);
                        v.add(at).write(val);
                    }
                    return LeafStateInsert {
                        old: None,
                        root: Some(Root::Leaf { ptr, pop: pop + 1 }),
                        tree_pop: None,
                    };
                }
                let new = alloc.alloc_bytes_dispatch::<OCC>(leaf_size(pop + 1));
                // SAFETY: copy keys and values around the insertion
                // point into the fresh (pop + 1)-entry leaf.
                unsafe {
                    let nk = new.as_ptr().cast::<u64>();
                    nk.copy_from_nonoverlapping(keys.as_ptr(), at);
                    nk.add(at).write(key);
                    nk.add(at + 1)
                        .copy_from_nonoverlapping(keys.as_ptr().add(at), pop - at);
                    let nv = new.as_ptr().add(leaf_values_offset(pop + 1)).cast::<u64>();
                    nv.copy_from_nonoverlapping(vals, at);
                    nv.add(at).write(val);
                    nv.add(at + 1)
                        .copy_from_nonoverlapping(vals.add(at), pop - at);
                    alloc.free_bytes_dispatch::<OCC>(ptr, leaf_size(pop));
                }
                LeafStateInsert {
                    old: None,
                    root: Some(Root::Leaf {
                        ptr: new,
                        pop: pop + 1,
                    }),
                    tree_pop: None,
                }
            } else {
                let top = promote_leaf::<OCC, NESTED>(alloc, keys, vals, key, val, path);
                // SAFETY: old root leaf no longer referenced.
                unsafe { alloc.free_bytes_dispatch::<OCC>(ptr, leaf_size(pop)) };
                LeafStateInsert {
                    old: None,
                    root: Some(Root::Tree { top }),
                    tree_pop: Some(pop as u64 + 1),
                }
            }
        }
        Root::Tree { .. } => unreachable!("leaf_state_insert on a tree root"),
    }
}

/// The removal of `key` from `root`, which is empty or a root leaf, as a
/// value: the value removed, and the root to store (`None`: unchanged). The
/// twin of [`leaf_state_insert`].
#[inline(always)]
fn leaf_state_remove<const OCC: bool>(
    root: Root,
    alloc: &NodeAlloc,
    key: Key,
) -> (Option<u64>, Option<Root>) {
    match root {
        Root::Empty => (None, None),
        Root::Leaf { ptr, pop } => {
            let (keys, vals) = MapCore::leaf_parts(ptr, pop);
            let Ok(at) = keys.binary_search(&key) else {
                return (None, None);
            };
            // SAFETY: in-bounds value read.
            let old = unsafe { *vals.add(at) };
            if pop == 1 {
                // SAFETY: last entry removed; free the leaf.
                unsafe { alloc.free_bytes_dispatch::<OCC>(ptr, leaf_size(1)) };
                (Some(old), Some(Root::Empty))
            } else if crate::leaf::cap_class(pop - 1) == crate::leaf::cap_class(pop) {
                // Fast path: capacity class unchanged — shift surviving entries in-place.
                // SAFETY: in-place shift inside class-sized buffer.
                unsafe {
                    let nk = ptr.as_ptr().cast::<u64>();
                    core::ptr::copy(nk.add(at + 1), nk.add(at), pop - 1 - at);
                    core::ptr::copy(vals.add(at + 1), vals.add(at), pop - 1 - at);
                }
                (Some(old), Some(Root::Leaf { ptr, pop: pop - 1 }))
            } else {
                let new = alloc.alloc_bytes_dispatch::<OCC>(leaf_size(pop - 1));
                // SAFETY: copy the surviving keys/values into the
                // smaller leaf.
                unsafe {
                    let nk = new.as_ptr().cast::<u64>();
                    nk.copy_from_nonoverlapping(keys.as_ptr(), at);
                    nk.add(at)
                        .copy_from_nonoverlapping(keys.as_ptr().add(at + 1), pop - 1 - at);
                    let nv = new.as_ptr().add(leaf_values_offset(pop - 1)).cast::<u64>();
                    nv.copy_from_nonoverlapping(vals, at);
                    nv.add(at)
                        .copy_from_nonoverlapping(vals.add(at + 1), pop - 1 - at);
                    alloc.free_bytes_dispatch::<OCC>(ptr, leaf_size(pop));
                }
                (
                    Some(old),
                    Some(Root::Leaf {
                        ptr: new,
                        pop: pop - 1,
                    }),
                )
            }
        }
        Root::Tree { .. } => unreachable!("leaf_state_remove on a tree root"),
    }
}

/// [`leaf_state_insert`] for a string node's cover holder, whose readers
/// load the root leaf while it stores (#1086): the stores into a live leaf
/// are atomic words (`bits::shared_word`). A copy rather than the generic
/// body's `OCC = true` instance, because `by_mode!` compiles that instance
/// into plain callers and changing it moved their code generation.
///
/// The insert of `key → val` into `root`, which is empty or a root leaf, as
/// a value: it allocates, frees and writes the leaf's heap areas, and returns
/// the root to store rather than storing it. One body for the owner's
/// `&mut MapCore` path and for a string node's cover holder, which stores
/// through a raw pointer because readers copy the core meanwhile (#1086).
#[cfg(all(target_pointer_width = "64", feature = "std"))]
#[inline(always)]
fn leaf_state_insert_shared(
    root: Root,
    alloc: &NodeAlloc,
    key: Key,
    val: u64,
    path: &mut crate::mutate_map::InsertPathMap,
) -> LeafStateInsert {
    match root {
        Root::Empty => {
            let ptr = alloc.alloc_bytes_dispatch::<true>(leaf_size(1));
            // SAFETY: fresh allocation: key slot then value slot.
            unsafe {
                ptr.as_ptr().cast::<u64>().write(key);
                ptr.as_ptr()
                    .add(leaf_values_offset(1))
                    .cast::<u64>()
                    .write(val);
            }
            LeafStateInsert {
                old: None,
                root: Some(Root::Leaf { ptr, pop: 1 }),
                tree_pop: None,
            }
        }
        Root::Leaf { ptr, pop } => {
            let (keys, vals) = MapCore::leaf_parts(ptr, pop);
            let (hit, at) = if pop > 0 {
                let last = keys[pop - 1];
                if key > last {
                    (false, pop)
                } else if key == last {
                    (true, pop - 1)
                } else {
                    match keys.binary_search(&key) {
                        Ok(pos) => (true, pos),
                        Err(pos) => (false, pos),
                    }
                }
            } else {
                (false, 0)
            };
            if hit {
                // SAFETY: in-place value swap.
                unsafe {
                    let slot = vals.add(at);
                    let old = *slot;
                    shared_word::store::<true>(slot, val);
                    return LeafStateInsert {
                        old: Some(old),
                        root: None,
                        tree_pop: None,
                    };
                }
            }
            if pop < ROOT_LEAF_CAP {
                if leaf_size(pop + 1) == leaf_size(pop) {
                    // Spare class capacity: shift both areas in
                    // place, no allocation and no copy of the
                    // whole leaf.
                    // SAFETY: same class, so the areas keep their
                    // offsets and the extra slot is in bounds.
                    unsafe {
                        let base = ptr.as_ptr().cast::<u64>();
                        shared_word::shift_up::<true>(base, at, pop - at);
                        shared_word::store::<true>(base.add(at), key);
                        let v = ptr.as_ptr().add(leaf_values_offset(pop)).cast::<u64>();
                        shared_word::shift_up::<true>(v, at, pop - at);
                        shared_word::store::<true>(v.add(at), val);
                    }
                    return LeafStateInsert {
                        old: None,
                        root: Some(Root::Leaf { ptr, pop: pop + 1 }),
                        tree_pop: None,
                    };
                }
                let new = alloc.alloc_bytes_dispatch::<true>(leaf_size(pop + 1));
                // SAFETY: copy keys and values around the insertion
                // point into the fresh (pop + 1)-entry leaf.
                unsafe {
                    let nk = new.as_ptr().cast::<u64>();
                    nk.copy_from_nonoverlapping(keys.as_ptr(), at);
                    nk.add(at).write(key);
                    nk.add(at + 1)
                        .copy_from_nonoverlapping(keys.as_ptr().add(at), pop - at);
                    let nv = new.as_ptr().add(leaf_values_offset(pop + 1)).cast::<u64>();
                    nv.copy_from_nonoverlapping(vals, at);
                    nv.add(at).write(val);
                    nv.add(at + 1)
                        .copy_from_nonoverlapping(vals.add(at), pop - at);
                    alloc.free_bytes_dispatch::<true>(ptr, leaf_size(pop));
                }
                LeafStateInsert {
                    old: None,
                    root: Some(Root::Leaf {
                        ptr: new,
                        pop: pop + 1,
                    }),
                    tree_pop: None,
                }
            } else {
                let top = promote_leaf::<true, true>(alloc, keys, vals, key, val, path);
                // SAFETY: old root leaf no longer referenced.
                unsafe { alloc.free_bytes_dispatch::<true>(ptr, leaf_size(pop)) };
                LeafStateInsert {
                    old: None,
                    root: Some(Root::Tree { top }),
                    tree_pop: Some(pop as u64 + 1),
                }
            }
        }
        Root::Tree { .. } => unreachable!("leaf_state_insert on a tree root"),
    }
}

/// [`leaf_state_remove`] for a string node's cover holder; as
/// [`leaf_state_insert_shared`].
///
/// The removal of `key` from `root`, which is empty or a root leaf, as a
/// value: the value removed, and the root to store (`None`: unchanged). The
/// twin of [`leaf_state_insert`].
#[cfg(all(target_pointer_width = "64", feature = "std"))]
#[inline(always)]
fn leaf_state_remove_shared(
    root: Root,
    alloc: &NodeAlloc,
    key: Key,
) -> (Option<u64>, Option<Root>) {
    match root {
        Root::Empty => (None, None),
        Root::Leaf { ptr, pop } => {
            let (keys, vals) = MapCore::leaf_parts(ptr, pop);
            let Ok(at) = keys.binary_search(&key) else {
                return (None, None);
            };
            // SAFETY: in-bounds value read.
            let old = unsafe { *vals.add(at) };
            if pop == 1 {
                // SAFETY: last entry removed; free the leaf.
                unsafe { alloc.free_bytes_dispatch::<true>(ptr, leaf_size(1)) };
                (Some(old), Some(Root::Empty))
            } else if crate::leaf::cap_class(pop - 1) == crate::leaf::cap_class(pop) {
                // Fast path: capacity class unchanged — shift surviving entries in-place.
                // SAFETY: in-place shift inside class-sized buffer.
                unsafe {
                    let nk = ptr.as_ptr().cast::<u64>();
                    shared_word::shift_down::<true>(nk, at, pop - 1 - at);
                    shared_word::shift_down::<true>(vals, at, pop - 1 - at);
                }
                (Some(old), Some(Root::Leaf { ptr, pop: pop - 1 }))
            } else {
                let new = alloc.alloc_bytes_dispatch::<true>(leaf_size(pop - 1));
                // SAFETY: copy the surviving keys/values into the
                // smaller leaf.
                unsafe {
                    let nk = new.as_ptr().cast::<u64>();
                    nk.copy_from_nonoverlapping(keys.as_ptr(), at);
                    nk.add(at)
                        .copy_from_nonoverlapping(keys.as_ptr().add(at + 1), pop - 1 - at);
                    let nv = new.as_ptr().add(leaf_values_offset(pop - 1)).cast::<u64>();
                    nv.copy_from_nonoverlapping(vals, at);
                    nv.add(at)
                        .copy_from_nonoverlapping(vals.add(at + 1), pop - 1 - at);
                    alloc.free_bytes_dispatch::<true>(ptr, leaf_size(pop));
                }
                (
                    Some(old),
                    Some(Root::Leaf {
                        ptr: new,
                        pop: pop - 1,
                    }),
                )
            }
        }
        Root::Tree { .. } => unreachable!("leaf_state_remove on a tree root"),
    }
}

/// The root-leaf promotion, per sharing mode (see `set::promote_leaf`).
#[inline(always)]
fn promote_leaf<const OCC: bool, const NESTED: bool>(
    alloc: &NodeAlloc,
    keys: &[u64],
    vals: *const u64,
    key: Key,
    val: u64,
    path: &mut crate::mutate_map::InsertPathMap,
) -> Edge {
    let mut top = Edge::NULL;
    let mut scratch = 0u32;
    let cover = if OCC {
        crate::occ::Cover::Node(&raw mut scratch)
    } else {
        crate::occ::Cover::Tree
    };
    for (at, &k) in keys.iter().enumerate() {
        // SAFETY: trie built and owned by `alloc`; values read in-bounds.
        let prev = unsafe {
            mutate_map::map_insert::<false, OCC, NESTED>(
                alloc,
                &mut top,
                k,
                *vals.add(at),
                8,
                cover,
            )
        };
        debug_assert!(prev.0.is_none());
    }
    // SAFETY: same trie; populate path for subsequent sequential/clustered inserts.
    let prev = unsafe {
        mutate_map::map_insert_with_path::<false, OCC, NESTED>(
            alloc, &mut top, key, val, 8, path, cover,
        )
    };
    debug_assert!(prev.0.is_none());
    top
}

impl MapCore {
    /// An empty core.
    pub(crate) const fn new() -> Self {
        Self {
            root: Root::Empty,
            tree_pop: 0,
        }
    }

    /// Number of keys in the map.
    #[inline(always)]
    #[must_use]
    pub(crate) fn len(&self) -> u64 {
        match &self.root {
            Root::Empty => 0,
            Root::Leaf { pop, .. } => *pop as u64,
            Root::Tree { .. } => self.tree_pop,
        }
    }

    /// True when no keys are present.
    #[inline(always)]
    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn leaf_parts(ptr: NonNull<u8>, pop: usize) -> (&'static [u64], *mut u64) {
        // SAFETY: the root leaf holds `pop` keys in a class-sized area,
        // then `pop` values in a second class-sized area, both 8-aligned
        // (allocations are cache-line aligned). The lifetime is scoped by
        // callers to the borrow of self.
        unsafe {
            (
                core::slice::from_raw_parts(ptr.as_ptr().cast::<u64>(), pop),
                ptr.as_ptr().add(leaf_values_offset(pop)).cast::<u64>(),
            )
        }
    }

    /// Returns the value stored for `key`.
    #[inline(always)]
    #[must_use]
    pub(crate) fn get(&self, key: Key) -> Option<u64> {
        match &self.root {
            Root::Empty => None,
            Root::Leaf { ptr, pop } => {
                let pop = *pop;
                let kptr = ptr.as_ptr().cast::<u64>();
                let at = if pop <= 4 {
                    // SAFETY: root leaf holds `pop` keys.
                    unsafe {
                        if pop >= 1 && *kptr == key {
                            0
                        } else if pop >= 2 && *kptr.add(1) == key {
                            1
                        } else if pop >= 3 && *kptr.add(2) == key {
                            2
                        } else if pop >= 4 && *kptr.add(3) == key {
                            3
                        } else {
                            return None;
                        }
                    }
                } else if pop <= 8 {
                    // SAFETY: root leaf holds `pop` keys.
                    unsafe {
                        if *kptr == key {
                            0
                        } else if *kptr.add(1) == key {
                            1
                        } else if *kptr.add(2) == key {
                            2
                        } else if *kptr.add(3) == key {
                            3
                        } else if pop >= 5 && *kptr.add(4) == key {
                            4
                        } else if pop >= 6 && *kptr.add(5) == key {
                            5
                        } else if pop >= 7 && *kptr.add(6) == key {
                            6
                        } else if pop >= 8 && *kptr.add(7) == key {
                            7
                        } else {
                            return None;
                        }
                    }
                } else {
                    // SAFETY: root leaf holds `pop` keys.
                    let keys = unsafe { core::slice::from_raw_parts(kptr, pop) };
                    keys.binary_search(&key).ok()?
                };
                // SAFETY: root leaf values live at values offset.
                let vptr = unsafe { ptr.as_ptr().add(leaf_values_offset(pop)).cast::<u64>() };
                // SAFETY: `at < pop` values live behind the keys.
                Some(unsafe { *vptr.add(at) })
            }
            // SAFETY: the trie is maintained by the map mutation engine
            // and satisfies the lookup contract.
            Root::Tree { top, .. } => unsafe { get::get_map(top, key, 8) },
        }
    }

    /// Look up a batch of `keys` simultaneously, writing values into `out`.
    #[inline]
    pub(crate) fn get_batch(&self, keys: &[Key], out: &mut [Option<u64>]) {
        self.get_batch_width::<{ get::BATCH_WIDTH }>(keys, out);
    }

    /// [`MapCore::get_batch`] at an explicit interleave width.
    #[inline]
    pub(crate) fn get_batch_width<const W: usize>(&self, keys: &[Key], out: &mut [Option<u64>]) {
        assert_eq!(
            keys.len(),
            out.len(),
            "keys and out slices must have equal length"
        );
        if keys.is_empty() {
            return;
        }
        match &self.root {
            Root::Empty => {
                out.fill(None);
            }
            Root::Leaf { .. } => {
                for (k, o) in keys.iter().zip(out.iter_mut()) {
                    *o = self.get(*k);
                }
            }
            Root::Tree { top, .. } => {
                // SAFETY: tree satisfies lookup invariants.
                unsafe {
                    get::get_map_batch_w::<W>(top, keys, out, 8);
                }
            }
        }
    }

    /// Look up a batch of `keys`, writing found values into `out_values` and presence flags
    /// into `out_found` (when `Some`). Returns the count of found keys.
    #[inline]
    pub(crate) fn get_batch_into(
        &self,
        keys: &[Key],
        out_values: &mut [u64],
        mut out_found: Option<&mut [bool]>,
    ) -> usize {
        assert_eq!(
            keys.len(),
            out_values.len(),
            "keys and out_values must have equal length"
        );
        if let Some(ref found) = out_found {
            assert_eq!(
                keys.len(),
                found.len(),
                "keys and out_found must have equal length"
            );
        }
        if keys.is_empty() {
            return 0;
        }

        let mut found_count = 0;
        // Scratch chunk, not the interleave width: the driver refills a
        // retired lane from the rest of the chunk, so a chunk boundary is
        // where the width has to drain. Sized well above `get::BATCH_WIDTH`
        // so those drains are a small fraction of the work.
        const SCRATCH: usize = 64;
        let mut tmp_opts = [None; SCRATCH];
        let mut offset = 0;

        for (k_chunk, v_chunk) in keys.chunks(SCRATCH).zip(out_values.chunks_mut(SCRATCH)) {
            let chunk_len = k_chunk.len();
            let opt_sub = &mut tmp_opts[..chunk_len];
            self.get_batch(k_chunk, opt_sub);
            for (idx, opt) in opt_sub.iter().enumerate() {
                let is_hit = opt.is_some();
                if let Some(val) = *opt {
                    v_chunk[idx] = val;
                    found_count += 1;
                }
                if let Some(ref mut found_slice) = out_found {
                    found_slice[offset + idx] = is_hit;
                }
            }
            offset += chunk_len;
        }

        found_count
    }

    /// Returns a pointer to `key`'s value slot in the leaf or root leaf, or `None`
    /// if absent.
    #[inline(always)]
    #[must_use]
    pub(crate) fn get_slot_ptr(&self, key: Key) -> Option<core::ptr::NonNull<u64>> {
        match &self.root {
            Root::Empty => None,
            Root::Leaf { ptr, pop } => {
                let pop = *pop;
                let kptr = ptr.as_ptr().cast::<u64>();
                let at = if pop <= 4 {
                    // SAFETY: root leaf holds `pop` keys.
                    unsafe {
                        if pop >= 1 && *kptr == key {
                            0
                        } else if pop >= 2 && *kptr.add(1) == key {
                            1
                        } else if pop >= 3 && *kptr.add(2) == key {
                            2
                        } else if pop >= 4 && *kptr.add(3) == key {
                            3
                        } else {
                            return None;
                        }
                    }
                } else if pop <= 8 {
                    // SAFETY: root leaf holds `pop` keys.
                    unsafe {
                        if *kptr == key {
                            0
                        } else if *kptr.add(1) == key {
                            1
                        } else if *kptr.add(2) == key {
                            2
                        } else if *kptr.add(3) == key {
                            3
                        } else if pop >= 5 && *kptr.add(4) == key {
                            4
                        } else if pop >= 6 && *kptr.add(5) == key {
                            5
                        } else if pop >= 7 && *kptr.add(6) == key {
                            6
                        } else if pop >= 8 && *kptr.add(7) == key {
                            7
                        } else {
                            return None;
                        }
                    }
                } else {
                    // SAFETY: root leaf holds `pop` keys.
                    let keys = unsafe { core::slice::from_raw_parts(kptr, pop) };
                    keys.binary_search(&key).ok()?
                };
                // SAFETY: root leaf values live at values offset.
                let vptr = unsafe { ptr.as_ptr().add(leaf_values_offset(pop)).cast::<u64>() };
                // SAFETY: `at < pop` values live behind the keys.
                core::ptr::NonNull::new(unsafe { vptr.add(at) })
            }
            Root::Tree { top, .. } => {
                // SAFETY: top is live valid tree root pointer.
                unsafe { crate::get::locate_slot((&raw const *top).cast_mut(), key, 8) }
            }
        }
    }

    /// Returns a **writable pointer to `key`'s value slot**, or `None` if
    /// the key is absent — the compat layer's `JudyLGet`/`JudyLIns` return
    /// convention. The pointer stays valid until the next structural
    /// mutation of the map (the classic JudyL contract); reading or
    /// writing through it after an `insert`/`remove`/`clear` is undefined.
    #[inline(always)]
    #[must_use]
    pub(crate) fn get_value_slot(
        &mut self,
        key: Key,
        path: &mut crate::mutate_map::InsertPathMap,
    ) -> Option<core::ptr::NonNull<u64>> {
        match &mut self.root {
            Root::Empty => None,
            Root::Leaf { ptr, pop } => {
                let pop = *pop;
                let kptr = ptr.as_ptr().cast::<u64>();
                let at = if pop <= 4 {
                    // SAFETY: root leaf holds `pop` keys.
                    unsafe {
                        if pop >= 1 && *kptr == key {
                            0
                        } else if pop >= 2 && *kptr.add(1) == key {
                            1
                        } else if pop >= 3 && *kptr.add(2) == key {
                            2
                        } else if pop >= 4 && *kptr.add(3) == key {
                            3
                        } else {
                            return None;
                        }
                    }
                } else if pop <= 8 {
                    // SAFETY: root leaf holds `pop` keys.
                    unsafe {
                        if *kptr == key {
                            0
                        } else if *kptr.add(1) == key {
                            1
                        } else if *kptr.add(2) == key {
                            2
                        } else if *kptr.add(3) == key {
                            3
                        } else if pop >= 5 && *kptr.add(4) == key {
                            4
                        } else if pop >= 6 && *kptr.add(5) == key {
                            5
                        } else if pop >= 7 && *kptr.add(6) == key {
                            6
                        } else if pop >= 8 && *kptr.add(7) == key {
                            7
                        } else {
                            return None;
                        }
                    }
                } else {
                    // SAFETY: root leaf holds `pop` keys.
                    let keys = unsafe { core::slice::from_raw_parts(kptr, pop) };
                    keys.binary_search(&key).ok()?
                };
                // SAFETY: root leaf values live at values offset.
                let vptr = unsafe { ptr.as_ptr().add(leaf_values_offset(pop)).cast::<u64>() };
                // SAFETY: `at < pop` values live behind the keys.
                core::ptr::NonNull::new(unsafe { vptr.add(at) })
            }
            // SAFETY: trie maintained/owned by this map's engine; the
            // raw walk derives the slot from node pointers only.
            Root::Tree { top, .. } => {
                let prefix = key >> 8;
                if path.prefix == prefix {
                    if let Some(leaf) = core::ptr::NonNull::new(path.leaf) {
                        let d = (key & 0xFF) as u8;
                        // SAFETY: path holds valid live LeafBitmapL pointer.
                        let node = unsafe { leaf.as_ref() };
                        let sub = (d >> 5) as usize;
                        if let Some(rank) = node.bitmap.test_and_subexpanse_rank(d) {
                            // SAFETY: sub < 8 accesses valid subarray; rank is in bounds.
                            let slot = unsafe { (*node.values.as_ptr().add(sub)).add(rank) };
                            return core::ptr::NonNull::new(slot);
                        }
                        return None;
                    } else if let Some(leaf1) = core::ptr::NonNull::new(path.leaf1) {
                        let d = (key & 0xFF) as u8;
                        let cur_pop = path.terminal_pop as usize;
                        let base = leaf1.as_ptr();
                        // SAFETY: base points to a live Leaf1 allocation; map_keys_offset is in-bounds.
                        let keys_ptr = unsafe { base.add(crate::leaf::map_keys_offset(cur_pop)) };
                        // SAFETY: keys_ptr holds cur_pop 1-byte keys.
                        let slot =
                            unsafe { crate::leaf::search_fixed::<1>(keys_ptr, cur_pop, d as u64) };
                        if let Some(slot) = slot {
                            // SAFETY: slot < cur_pop values live behind keys.
                            return core::ptr::NonNull::new(unsafe {
                                base.cast::<u64>().add(slot)
                            });
                        }
                        return None;
                    }
                }
                // SAFETY: top is live valid tree root pointer.
                unsafe { crate::get::locate_slot(&raw mut *top, key, 8) }
            }
        }
    }

    /// Inserts `key` with value 0 if absent — the existing value is kept
    /// untouched — and returns a **writable pointer to its value slot**:
    /// the compat `JudyLIns` contract, in one tree walk. The pointer stays
    /// valid until the next structural mutation.
    #[inline(always)]
    pub(crate) fn ins_slot(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
        path: &mut crate::mutate_map::InsertPathMap,
    ) -> core::ptr::NonNull<u64> {
        match &mut self.root {
            Root::Tree { top } => {
                let prefix = key >> 8;
                if path.prefix == prefix {
                    alloc.assert_bracketed();
                    if let Some(mut leaf) = core::ptr::NonNull::new(path.leaf) {
                        let d = (key & 0xFF) as u8;
                        // SAFETY: path holds valid live LeafBitmapL pointer.
                        let node = unsafe { leaf.as_mut() };
                        let sub = (d >> 5) as usize;
                        if let Some(rank) = node.bitmap.test_and_subexpanse_rank(d) {
                            // SAFETY: value subarray holds subexpanse_count values.
                            let slot = unsafe { node.values[sub].add(rank) };
                            return core::ptr::NonNull::new(slot).expect("slot");
                        }
                        let rank = node.bitmap.subexpanse_rank(d) as usize;
                        let old_n = node.bitmap.subexpanse_count(sub) as usize;
                        if old_n > 0
                            && crate::leaf::cap_class(old_n + 1) == crate::leaf::cap_class(old_n)
                        {
                            // Fast path: spare class capacity — shift in place.
                            // SAFETY: the subarray holds cap_class(old_n) slots.
                            unsafe {
                                let arr = node.values[sub];
                                core::ptr::copy(arr.add(rank), arr.add(rank + 1), old_n - rank);
                                arr.add(rank).write(0);
                            }
                        } else {
                            let new = alloc
                                .alloc_bytes_plain(crate::mutate::sub_vals_size(old_n + 1))
                                .cast::<u64>();
                            // SAFETY: copy old_n values around the inserted rank.
                            unsafe {
                                if old_n > 0 {
                                    let old = node.values[sub];
                                    new.as_ptr().copy_from_nonoverlapping(old, rank);
                                    new.as_ptr()
                                        .add(rank + 1)
                                        .copy_from_nonoverlapping(old.add(rank), old_n - rank);
                                    alloc.free_bytes_plain(
                                        core::ptr::NonNull::new(old.cast()).expect("values"),
                                        crate::mutate::sub_vals_size(old_n),
                                    );
                                }
                                new.as_ptr().add(rank).write(0);
                            }
                            node.values[sub] = new.as_ptr();
                        }
                        node.bitmap.set(d);
                        path.pending_pop += 1;
                        path.terminal_pop += 1;
                        self.tree_pop += 1;
                        debug_assert!(!path.edges[0].is_null());
                        // SAFETY: keep terminal edge pop0 up to date. The warm path is
                        // armed only where `edges[0]` is set beside `prefix`,
                        // `leaf`/`leaf1` and `depth` (mutate_map.rs:742, 821, 860, 922);
                        // `clear()` resets `prefix` to `u64::MAX`, which matches no
                        // `key >> 8`, so a cleared path never reaches here.
                        unsafe {
                            core::ptr::NonNull::new_unchecked(path.edges[0])
                                .as_mut()
                                .set_pop0(1, (path.terminal_pop - 1) as u64);
                        }
                        // SAFETY: freshly inserted slot.
                        let slot = unsafe { node.values[sub].add(rank) };
                        return core::ptr::NonNull::new(slot).expect("slot");
                    } else if let Some(leaf1) = core::ptr::NonNull::new(path.leaf1) {
                        let d = (key & 0xFF) as u8;
                        let cur_pop = path.terminal_pop as usize;
                        let base = leaf1.as_ptr();
                        // SAFETY: base points to a live Leaf1 allocation; map_keys_offset is in-bounds.
                        let keys_ptr = unsafe { base.add(crate::leaf::map_keys_offset(cur_pop)) };
                        // SAFETY: cur_pop >= 1 when leaf1 is active, so cur_pop - 1 is in bounds.
                        let last = unsafe { *keys_ptr.add(cur_pop - 1) };
                        if d > last {
                            if cur_pop < crate::mutate::LEAF1_CAP
                                && crate::leaf::cap_class(cur_pop + 1)
                                    == crate::leaf::cap_class(cur_pop)
                            {
                                // SAFETY: spare class capacity in the live Leaf1 allocation.
                                unsafe {
                                    *keys_ptr.add(cur_pop) = d;
                                    let vals = base.cast::<u64>();
                                    vals.add(cur_pop).write(0);
                                    debug_assert!(!path.edges[0].is_null());
                                    // SAFETY: the warm path is armed only where `edges[0]` is set beside
                                    // `prefix`, `leaf`/`leaf1` and `depth` (mutate_map.rs:742, 821, 860,
                                    // 922); `clear()` resets `prefix` to `u64::MAX`, which matches no
                                    // `key >> 8`, so a cleared path never reaches here.
                                    core::ptr::NonNull::new_unchecked(path.edges[0])
                                        .as_mut()
                                        .set_pop0(1, cur_pop as u64);
                                }
                                path.terminal_pop += 1;
                                path.pending_pop += 1;
                                self.tree_pop += 1;
                                // SAFETY: freshly written slot in live value area.
                                let slot = unsafe { base.cast::<u64>().add(cur_pop) };
                                return core::ptr::NonNull::new(slot).expect("slot");
                            }
                        } else if d == last {
                            // SAFETY: cur_pop - 1 is the existing slot for `last`.
                            let slot = unsafe { base.cast::<u64>().add(cur_pop - 1) };
                            return core::ptr::NonNull::new(slot).expect("slot");
                        }
                    }
                }
                path.clear();
                // SAFETY: trie maintained/owned by this map's engine.
                // The slot API is not on the shared wrapper; a deferred
                // tree still dispatches by flag so its stores are bracketed.
                let (_prev, slot) = by_mode!(
                    alloc,
                    tree_insert::<true>(alloc, &mut self.tree_pop, path, top, key, 0)
                );
                // SAFETY: map_insert always returns a valid, non-null slot pointer.
                unsafe { core::ptr::NonNull::new_unchecked(slot) }
            }
            Root::Empty => {
                let ptr = alloc.alloc_bytes(leaf_size(1));
                // SAFETY: fresh allocation: key slot then value slot.
                unsafe {
                    ptr.as_ptr().cast::<u64>().write(key);
                    let vptr = ptr.as_ptr().add(leaf_values_offset(1)).cast::<u64>();
                    vptr.write(0);
                    self.root = Root::Leaf { ptr, pop: 1 };
                    core::ptr::NonNull::new(vptr).expect("slot")
                }
            }
            Root::Leaf { ptr, pop } => {
                let (ptr_val, pop_val) = (*ptr, *pop);
                let (keys, vals) = Self::leaf_parts(ptr_val, pop_val);
                let (hit, at) = if pop_val > 0 {
                    let last = keys[pop_val - 1];
                    if key > last {
                        (false, pop_val)
                    } else if key == last {
                        (true, pop_val - 1)
                    } else if pop_val <= 4 {
                        let k0 = keys[0];
                        if key < k0 {
                            (false, 0)
                        } else if key == k0 {
                            (true, 0)
                        } else if pop_val == 2 {
                            (false, 1)
                        } else {
                            let k1 = keys[1];
                            if key < k1 {
                                (false, 1)
                            } else if key == k1 {
                                (true, 1)
                            } else if pop_val == 3 {
                                (false, 2)
                            } else {
                                let k2 = keys[2];
                                if key < k2 {
                                    (false, 2)
                                } else if key == k2 {
                                    (true, 2)
                                } else {
                                    (false, 3)
                                }
                            }
                        }
                    } else {
                        match keys.binary_search(&key) {
                            Ok(pos) => (true, pos),
                            Err(pos) => (false, pos),
                        }
                    }
                } else {
                    (false, 0)
                };
                if hit {
                    // SAFETY: vals points to live values array in the root leaf.
                    let slot = unsafe { vals.add(at) };
                    return core::ptr::NonNull::new(slot).expect("slot");
                }
                if pop_val < ROOT_LEAF_CAP {
                    if leaf_size(pop_val + 1) == leaf_size(pop_val) {
                        // Spare class capacity: shift in place, no realloc.
                        // SAFETY: same class, areas keep offsets, slot is in bounds.
                        unsafe {
                            let base = ptr_val.as_ptr().cast::<u64>();
                            core::ptr::copy(base.add(at), base.add(at + 1), pop_val - at);
                            base.add(at).write(key);
                            let v = ptr_val
                                .as_ptr()
                                .add(leaf_values_offset(pop_val))
                                .cast::<u64>();
                            core::ptr::copy(v.add(at), v.add(at + 1), pop_val - at);
                            let slot = v.add(at);
                            slot.write(0);
                            self.root = Root::Leaf {
                                ptr: ptr_val,
                                pop: pop_val + 1,
                            };
                            core::ptr::NonNull::new(slot).expect("slot")
                        }
                    } else {
                        let new = alloc.alloc_bytes(leaf_size(pop_val + 1));
                        // SAFETY: copy keys and values around insertion point into new leaf.
                        unsafe {
                            let nk = new.as_ptr().cast::<u64>();
                            nk.copy_from_nonoverlapping(keys.as_ptr(), at);
                            nk.add(at).write(key);
                            nk.add(at + 1)
                                .copy_from_nonoverlapping(keys.as_ptr().add(at), pop_val - at);
                            let nv = new
                                .as_ptr()
                                .add(leaf_values_offset(pop_val + 1))
                                .cast::<u64>();
                            nv.copy_from_nonoverlapping(vals, at);
                            let slot = nv.add(at);
                            slot.write(0);
                            nv.add(at + 1)
                                .copy_from_nonoverlapping(vals.add(at), pop_val - at);
                            alloc.free_bytes(ptr_val, leaf_size(pop_val));
                            self.root = Root::Leaf {
                                ptr: new,
                                pop: pop_val + 1,
                            };
                            core::ptr::NonNull::new(slot).expect("slot")
                        }
                    }
                } else {
                    self.insert(alloc, key, 0, path);
                    self.get_value_slot(key, path).expect("just-ensured key")
                }
            }
        }
    }

    /// [`Self::ins_slot`] on a tree whose readers run concurrently (#1086):
    /// a copy whose stores into a live root leaf are atomic words, so the
    /// plain body's code generation is not touched by the shared one.
    #[cfg(feature = "std")]
    #[inline(always)]
    pub(crate) fn ins_slot_shared(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
        path: &mut crate::mutate_map::InsertPathMap,
    ) -> core::ptr::NonNull<u64> {
        match &mut self.root {
            Root::Tree { top } => {
                let prefix = key >> 8;
                if path.prefix == prefix {
                    alloc.assert_bracketed();
                    if let Some(mut leaf) = core::ptr::NonNull::new(path.leaf) {
                        let d = (key & 0xFF) as u8;
                        // SAFETY: path holds valid live LeafBitmapL pointer.
                        let node = unsafe { leaf.as_mut() };
                        let sub = (d >> 5) as usize;
                        if let Some(rank) = node.bitmap.test_and_subexpanse_rank(d) {
                            // SAFETY: value subarray holds subexpanse_count values.
                            let slot = unsafe { node.values[sub].add(rank) };
                            return core::ptr::NonNull::new(slot).expect("slot");
                        }
                        let rank = node.bitmap.subexpanse_rank(d) as usize;
                        let old_n = node.bitmap.subexpanse_count(sub) as usize;
                        if old_n > 0
                            && crate::leaf::cap_class(old_n + 1) == crate::leaf::cap_class(old_n)
                        {
                            // Fast path: spare class capacity — shift in place.
                            // SAFETY: the subarray holds cap_class(old_n) slots.
                            unsafe {
                                let arr = node.values[sub];
                                core::ptr::copy(arr.add(rank), arr.add(rank + 1), old_n - rank);
                                arr.add(rank).write(0);
                            }
                        } else {
                            let new = alloc
                                .alloc_bytes_plain(crate::mutate::sub_vals_size(old_n + 1))
                                .cast::<u64>();
                            // SAFETY: copy old_n values around the inserted rank.
                            unsafe {
                                if old_n > 0 {
                                    let old = node.values[sub];
                                    new.as_ptr().copy_from_nonoverlapping(old, rank);
                                    new.as_ptr()
                                        .add(rank + 1)
                                        .copy_from_nonoverlapping(old.add(rank), old_n - rank);
                                    alloc.free_bytes_plain(
                                        core::ptr::NonNull::new(old.cast()).expect("values"),
                                        crate::mutate::sub_vals_size(old_n),
                                    );
                                }
                                new.as_ptr().add(rank).write(0);
                            }
                            node.values[sub] = new.as_ptr();
                        }
                        node.bitmap.set(d);
                        path.pending_pop += 1;
                        path.terminal_pop += 1;
                        self.tree_pop += 1;
                        debug_assert!(!path.edges[0].is_null());
                        // SAFETY: keep terminal edge pop0 up to date. The warm path is
                        // armed only where `edges[0]` is set beside `prefix`,
                        // `leaf`/`leaf1` and `depth` (mutate_map.rs:742, 821, 860, 922);
                        // `clear()` resets `prefix` to `u64::MAX`, which matches no
                        // `key >> 8`, so a cleared path never reaches here.
                        unsafe {
                            core::ptr::NonNull::new_unchecked(path.edges[0])
                                .as_mut()
                                .set_pop0(1, (path.terminal_pop - 1) as u64);
                        }
                        // SAFETY: freshly inserted slot.
                        let slot = unsafe { node.values[sub].add(rank) };
                        return core::ptr::NonNull::new(slot).expect("slot");
                    } else if let Some(leaf1) = core::ptr::NonNull::new(path.leaf1) {
                        let d = (key & 0xFF) as u8;
                        let cur_pop = path.terminal_pop as usize;
                        let base = leaf1.as_ptr();
                        // SAFETY: base points to a live Leaf1 allocation; map_keys_offset is in-bounds.
                        let keys_ptr = unsafe { base.add(crate::leaf::map_keys_offset(cur_pop)) };
                        // SAFETY: cur_pop >= 1 when leaf1 is active, so cur_pop - 1 is in bounds.
                        let last = unsafe { *keys_ptr.add(cur_pop - 1) };
                        if d > last {
                            if cur_pop < crate::mutate::LEAF1_CAP
                                && crate::leaf::cap_class(cur_pop + 1)
                                    == crate::leaf::cap_class(cur_pop)
                            {
                                // SAFETY: spare class capacity in the live Leaf1 allocation.
                                unsafe {
                                    *keys_ptr.add(cur_pop) = d;
                                    let vals = base.cast::<u64>();
                                    vals.add(cur_pop).write(0);
                                    debug_assert!(!path.edges[0].is_null());
                                    // SAFETY: the warm path is armed only where `edges[0]` is set beside
                                    // `prefix`, `leaf`/`leaf1` and `depth` (mutate_map.rs:742, 821, 860,
                                    // 922); `clear()` resets `prefix` to `u64::MAX`, which matches no
                                    // `key >> 8`, so a cleared path never reaches here.
                                    core::ptr::NonNull::new_unchecked(path.edges[0])
                                        .as_mut()
                                        .set_pop0(1, cur_pop as u64);
                                }
                                path.terminal_pop += 1;
                                path.pending_pop += 1;
                                self.tree_pop += 1;
                                // SAFETY: freshly written slot in live value area.
                                let slot = unsafe { base.cast::<u64>().add(cur_pop) };
                                return core::ptr::NonNull::new(slot).expect("slot");
                            }
                        } else if d == last {
                            // SAFETY: cur_pop - 1 is the existing slot for `last`.
                            let slot = unsafe { base.cast::<u64>().add(cur_pop - 1) };
                            return core::ptr::NonNull::new(slot).expect("slot");
                        }
                    }
                }
                path.clear();
                // SAFETY: trie maintained/owned by this map's engine.
                // The slot API is not on the shared wrapper; a deferred
                // tree still dispatches by flag so its stores are bracketed.
                let (_prev, slot) = by_mode!(
                    alloc,
                    tree_insert::<true>(alloc, &mut self.tree_pop, path, top, key, 0)
                );
                // SAFETY: map_insert always returns a valid, non-null slot pointer.
                unsafe { core::ptr::NonNull::new_unchecked(slot) }
            }
            Root::Empty => {
                let ptr = alloc.alloc_bytes(leaf_size(1));
                // SAFETY: fresh allocation: key slot then value slot.
                unsafe {
                    ptr.as_ptr().cast::<u64>().write(key);
                    let vptr = ptr.as_ptr().add(leaf_values_offset(1)).cast::<u64>();
                    vptr.write(0);
                    self.root = Root::Leaf { ptr, pop: 1 };
                    core::ptr::NonNull::new(vptr).expect("slot")
                }
            }
            Root::Leaf { ptr, pop } => {
                let (ptr_val, pop_val) = (*ptr, *pop);
                let (keys, vals) = Self::leaf_parts(ptr_val, pop_val);
                let (hit, at) = if pop_val > 0 {
                    let last = keys[pop_val - 1];
                    if key > last {
                        (false, pop_val)
                    } else if key == last {
                        (true, pop_val - 1)
                    } else if pop_val <= 4 {
                        let k0 = keys[0];
                        if key < k0 {
                            (false, 0)
                        } else if key == k0 {
                            (true, 0)
                        } else if pop_val == 2 {
                            (false, 1)
                        } else {
                            let k1 = keys[1];
                            if key < k1 {
                                (false, 1)
                            } else if key == k1 {
                                (true, 1)
                            } else if pop_val == 3 {
                                (false, 2)
                            } else {
                                let k2 = keys[2];
                                if key < k2 {
                                    (false, 2)
                                } else if key == k2 {
                                    (true, 2)
                                } else {
                                    (false, 3)
                                }
                            }
                        }
                    } else {
                        match keys.binary_search(&key) {
                            Ok(pos) => (true, pos),
                            Err(pos) => (false, pos),
                        }
                    }
                } else {
                    (false, 0)
                };
                if hit {
                    // SAFETY: vals points to live values array in the root leaf.
                    let slot = unsafe { vals.add(at) };
                    return core::ptr::NonNull::new(slot).expect("slot");
                }
                if pop_val < ROOT_LEAF_CAP {
                    if leaf_size(pop_val + 1) == leaf_size(pop_val) {
                        // Spare class capacity: shift in place, no realloc.
                        // SAFETY: same class, areas keep offsets, slot is in bounds.
                        unsafe {
                            let base = ptr_val.as_ptr().cast::<u64>();
                            shared_word::shift_up::<true>(base, at, pop_val - at);
                            shared_word::store::<true>(base.add(at), key);
                            let v = ptr_val
                                .as_ptr()
                                .add(leaf_values_offset(pop_val))
                                .cast::<u64>();
                            shared_word::shift_up::<true>(v, at, pop_val - at);
                            let slot = v.add(at);
                            shared_word::store::<true>(slot, 0);
                            self.root = Root::Leaf {
                                ptr: ptr_val,
                                pop: pop_val + 1,
                            };
                            core::ptr::NonNull::new(slot).expect("slot")
                        }
                    } else {
                        let new = alloc.alloc_bytes(leaf_size(pop_val + 1));
                        // SAFETY: copy keys and values around insertion point into new leaf.
                        unsafe {
                            let nk = new.as_ptr().cast::<u64>();
                            nk.copy_from_nonoverlapping(keys.as_ptr(), at);
                            nk.add(at).write(key);
                            nk.add(at + 1)
                                .copy_from_nonoverlapping(keys.as_ptr().add(at), pop_val - at);
                            let nv = new
                                .as_ptr()
                                .add(leaf_values_offset(pop_val + 1))
                                .cast::<u64>();
                            nv.copy_from_nonoverlapping(vals, at);
                            let slot = nv.add(at);
                            slot.write(0);
                            nv.add(at + 1)
                                .copy_from_nonoverlapping(vals.add(at), pop_val - at);
                            alloc.free_bytes(ptr_val, leaf_size(pop_val));
                            self.root = Root::Leaf {
                                ptr: new,
                                pop: pop_val + 1,
                            };
                            core::ptr::NonNull::new(slot).expect("slot")
                        }
                    }
                } else {
                    self.insert_shared(alloc, key, 0, path);
                    self.get_value_slot(key, path).expect("just-ensured key")
                }
            }
        }
    }

    /// Single-threaded insert-if-absent returning slot pointer, bypassing OCC checks.
    #[inline(always)]
    pub(crate) fn ins_slot_plain(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
        path: &mut crate::mutate_map::InsertPathMap,
    ) -> core::ptr::NonNull<u64> {
        match &mut self.root {
            Root::Tree { top } => {
                let prefix = key >> 8;
                if path.prefix == prefix {
                    alloc.assert_bracketed();
                    if let Some(mut leaf) = core::ptr::NonNull::new(path.leaf) {
                        let d = (key & 0xFF) as u8;
                        // SAFETY: path holds valid live LeafBitmapL pointer.
                        let node = unsafe { leaf.as_mut() };
                        let sub = (d >> 5) as usize;
                        if let Some(rank) = node.bitmap.test_and_subexpanse_rank(d) {
                            // SAFETY: value subarray holds subexpanse_count values.
                            let slot = unsafe { node.values[sub].add(rank) };
                            return core::ptr::NonNull::new(slot).expect("slot");
                        }
                        let rank = node.bitmap.subexpanse_rank(d) as usize;
                        let old_n = node.bitmap.subexpanse_count(sub) as usize;
                        if old_n > 0
                            && crate::leaf::cap_class(old_n + 1) == crate::leaf::cap_class(old_n)
                        {
                            // Fast path: spare class capacity — shift in place.
                            // SAFETY: the subarray holds cap_class(old_n) slots.
                            unsafe {
                                let arr = node.values[sub];
                                core::ptr::copy(arr.add(rank), arr.add(rank + 1), old_n - rank);
                                arr.add(rank).write(0);
                            }
                        } else {
                            let new = alloc
                                .alloc_bytes_plain(crate::mutate::sub_vals_size(old_n + 1))
                                .cast::<u64>();
                            // SAFETY: copy old_n values around the inserted rank.
                            unsafe {
                                if old_n > 0 {
                                    let old = node.values[sub];
                                    new.as_ptr().copy_from_nonoverlapping(old, rank);
                                    new.as_ptr()
                                        .add(rank + 1)
                                        .copy_from_nonoverlapping(old.add(rank), old_n - rank);
                                    alloc.free_bytes_plain(
                                        core::ptr::NonNull::new(old.cast()).expect("values"),
                                        crate::mutate::sub_vals_size(old_n),
                                    );
                                }
                                new.as_ptr().add(rank).write(0);
                            }
                            node.values[sub] = new.as_ptr();
                        }
                        node.bitmap.set(d);
                        path.pending_pop += 1;
                        path.terminal_pop += 1;
                        self.tree_pop += 1;
                        debug_assert!(!path.edges[0].is_null());
                        // SAFETY: keep terminal edge pop0 up to date. The warm path is
                        // armed only where `edges[0]` is set beside `prefix`,
                        // `leaf`/`leaf1` and `depth` (mutate_map.rs:742, 821, 860, 922);
                        // `clear()` resets `prefix` to `u64::MAX`, which matches no
                        // `key >> 8`, so a cleared path never reaches here.
                        unsafe {
                            core::ptr::NonNull::new_unchecked(path.edges[0])
                                .as_mut()
                                .set_pop0(1, (path.terminal_pop - 1) as u64);
                        }
                        // SAFETY: freshly inserted slot.
                        let slot = unsafe { node.values[sub].add(rank) };
                        return core::ptr::NonNull::new(slot).expect("slot");
                    } else if let Some(leaf1) = core::ptr::NonNull::new(path.leaf1) {
                        let d = (key & 0xFF) as u8;
                        let cur_pop = path.terminal_pop as usize;
                        let base = leaf1.as_ptr();
                        // SAFETY: base points to a live Leaf1 allocation; map_keys_offset is in-bounds.
                        let keys_ptr = unsafe { base.add(crate::leaf::map_keys_offset(cur_pop)) };
                        // SAFETY: cur_pop >= 1 when leaf1 is active, so cur_pop - 1 is in bounds.
                        let last = unsafe { *keys_ptr.add(cur_pop - 1) };
                        if d > last {
                            if cur_pop < crate::mutate::LEAF1_CAP
                                && crate::leaf::cap_class(cur_pop + 1)
                                    == crate::leaf::cap_class(cur_pop)
                            {
                                // SAFETY: spare class capacity in the live Leaf1 allocation.
                                unsafe {
                                    *keys_ptr.add(cur_pop) = d;
                                    let vals = base.cast::<u64>();
                                    vals.add(cur_pop).write(0);
                                    debug_assert!(!path.edges[0].is_null());
                                    // SAFETY: the warm path is armed only where `edges[0]` is set beside
                                    // `prefix`, `leaf`/`leaf1` and `depth` (mutate_map.rs:742, 821, 860,
                                    // 922); `clear()` resets `prefix` to `u64::MAX`, which matches no
                                    // `key >> 8`, so a cleared path never reaches here.
                                    core::ptr::NonNull::new_unchecked(path.edges[0])
                                        .as_mut()
                                        .set_pop0(1, cur_pop as u64);
                                }
                                path.terminal_pop += 1;
                                path.pending_pop += 1;
                                self.tree_pop += 1;
                                // SAFETY: freshly written slot in live value area.
                                let slot = unsafe { base.cast::<u64>().add(cur_pop) };
                                return core::ptr::NonNull::new(slot).expect("slot");
                            }
                        } else if d == last {
                            // SAFETY: cur_pop - 1 is the existing slot for `last`.
                            let slot = unsafe { base.cast::<u64>().add(cur_pop - 1) };
                            return core::ptr::NonNull::new(slot).expect("slot");
                        }
                    }
                }
                path.clear();
                let (_prev, slot) =
                    tree_insert::<true, false, false>(alloc, &mut self.tree_pop, path, top, key, 0);
                // SAFETY: map_insert always returns a valid, non-null slot pointer.
                unsafe { core::ptr::NonNull::new_unchecked(slot) }
            }
            Root::Empty => {
                let ptr = alloc.alloc_bytes_plain(leaf_size(1));
                // SAFETY: fresh allocation: key slot then value slot.
                unsafe {
                    ptr.as_ptr().cast::<u64>().write(key);
                    let vptr = ptr.as_ptr().add(leaf_values_offset(1)).cast::<u64>();
                    vptr.write(0);
                    self.root = Root::Leaf { ptr, pop: 1 };
                    core::ptr::NonNull::new(vptr).expect("slot")
                }
            }
            Root::Leaf { ptr, pop } => {
                let (ptr_val, pop_val) = (*ptr, *pop);
                let (keys, vals) = Self::leaf_parts(ptr_val, pop_val);
                let (hit, at) = if pop_val > 0 {
                    let last = keys[pop_val - 1];
                    if key > last {
                        (false, pop_val)
                    } else if key == last {
                        (true, pop_val - 1)
                    } else if pop_val <= 4 {
                        let k0 = keys[0];
                        if key < k0 {
                            (false, 0)
                        } else if key == k0 {
                            (true, 0)
                        } else if pop_val == 2 {
                            (false, 1)
                        } else {
                            let k1 = keys[1];
                            if key < k1 {
                                (false, 1)
                            } else if key == k1 {
                                (true, 1)
                            } else if pop_val == 3 {
                                (false, 2)
                            } else {
                                let k2 = keys[2];
                                if key < k2 {
                                    (false, 2)
                                } else if key == k2 {
                                    (true, 2)
                                } else {
                                    (false, 3)
                                }
                            }
                        }
                    } else {
                        match keys.binary_search(&key) {
                            Ok(pos) => (true, pos),
                            Err(pos) => (false, pos),
                        }
                    }
                } else {
                    (false, 0)
                };
                if hit {
                    // SAFETY: vals points to live values array in the root leaf.
                    let slot = unsafe { vals.add(at) };
                    return core::ptr::NonNull::new(slot).expect("slot");
                }
                if pop_val < ROOT_LEAF_CAP {
                    if leaf_size(pop_val + 1) == leaf_size(pop_val) {
                        // Spare class capacity: shift in place, no realloc.
                        // SAFETY: same class, areas keep offsets, slot is in bounds.
                        unsafe {
                            let base = ptr_val.as_ptr().cast::<u64>();
                            core::ptr::copy(base.add(at), base.add(at + 1), pop_val - at);
                            base.add(at).write(key);
                            let v = ptr_val
                                .as_ptr()
                                .add(leaf_values_offset(pop_val))
                                .cast::<u64>();
                            core::ptr::copy(v.add(at), v.add(at + 1), pop_val - at);
                            let slot = v.add(at);
                            slot.write(0);
                            self.root = Root::Leaf {
                                ptr: ptr_val,
                                pop: pop_val + 1,
                            };
                            core::ptr::NonNull::new(slot).expect("slot")
                        }
                    } else {
                        let new = alloc.alloc_bytes_plain(leaf_size(pop_val + 1));
                        // SAFETY: copy keys and values around insertion point into new leaf.
                        unsafe {
                            let nk = new.as_ptr().cast::<u64>();
                            nk.copy_from_nonoverlapping(keys.as_ptr(), at);
                            nk.add(at).write(key);
                            nk.add(at + 1)
                                .copy_from_nonoverlapping(keys.as_ptr().add(at), pop_val - at);
                            let nv = new
                                .as_ptr()
                                .add(leaf_values_offset(pop_val + 1))
                                .cast::<u64>();
                            nv.copy_from_nonoverlapping(vals, at);
                            let slot = nv.add(at);
                            slot.write(0);
                            nv.add(at + 1)
                                .copy_from_nonoverlapping(vals.add(at), pop_val - at);
                            alloc.free_bytes_plain(ptr_val, leaf_size(pop_val));
                            self.root = Root::Leaf {
                                ptr: new,
                                pop: pop_val + 1,
                            };
                            core::ptr::NonNull::new(slot).expect("slot")
                        }
                    }
                } else {
                    self.insert_plain(alloc, key, 0, path);
                    self.get_value_slot(key, path).expect("just-ensured key")
                }
            }
        }
    }

    #[inline(always)]
    pub(crate) fn ins_slot_dispatch<const OCC: bool, const NESTED: bool>(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
        path: &mut crate::mutate_map::InsertPathMap,
    ) -> core::ptr::NonNull<u64> {
        match &mut self.root {
            Root::Tree { top } => {
                path.clear();
                let (_prev, slot) =
                    tree_insert::<true, OCC, NESTED>(alloc, &mut self.tree_pop, path, top, key, 0);
                // SAFETY: map_insert always returns a valid, non-null slot pointer.
                unsafe { core::ptr::NonNull::new_unchecked(slot) }
            }
            Root::Empty => {
                let ptr = alloc.alloc_bytes_dispatch::<OCC>(leaf_size(1));
                // SAFETY: fresh allocation: key slot then value slot.
                unsafe {
                    ptr.as_ptr().cast::<u64>().write(key);
                    let vptr = ptr.as_ptr().add(leaf_values_offset(1)).cast::<u64>();
                    vptr.write(0);
                    self.root = Root::Leaf { ptr, pop: 1 };
                    core::ptr::NonNull::new(vptr).expect("slot")
                }
            }
            Root::Leaf { ptr, pop } => {
                let (ptr_val, pop_val) = (*ptr, *pop);
                let (keys, vals) = Self::leaf_parts(ptr_val, pop_val);
                let (hit, at) = if pop_val > 0 {
                    let last = keys[pop_val - 1];
                    if key > last {
                        (false, pop_val)
                    } else if key == last {
                        (true, pop_val - 1)
                    } else if pop_val <= 4 {
                        let k0 = keys[0];
                        if key < k0 {
                            (false, 0)
                        } else if key == k0 {
                            (true, 0)
                        } else if pop_val == 2 {
                            (false, 1)
                        } else {
                            let k1 = keys[1];
                            if key < k1 {
                                (false, 1)
                            } else if key == k1 {
                                (true, 1)
                            } else if pop_val == 3 {
                                (false, 2)
                            } else {
                                let k2 = keys[2];
                                if key < k2 {
                                    (false, 2)
                                } else if key == k2 {
                                    (true, 2)
                                } else {
                                    (false, 3)
                                }
                            }
                        }
                    } else {
                        match keys.binary_search(&key) {
                            Ok(pos) => (true, pos),
                            Err(pos) => (false, pos),
                        }
                    }
                } else {
                    (false, 0)
                };
                if hit {
                    // SAFETY: vals points to live values array in the root leaf.
                    let slot = unsafe { vals.add(at) };
                    return core::ptr::NonNull::new(slot).expect("slot");
                }
                if pop_val < ROOT_LEAF_CAP {
                    if leaf_size(pop_val + 1) == leaf_size(pop_val) {
                        // SAFETY: same class, areas keep offsets, slot is in bounds.
                        unsafe {
                            let base = ptr_val.as_ptr().cast::<u64>();
                            core::ptr::copy(base.add(at), base.add(at + 1), pop_val - at);
                            base.add(at).write(key);
                            let v = ptr_val
                                .as_ptr()
                                .add(leaf_values_offset(pop_val))
                                .cast::<u64>();
                            core::ptr::copy(v.add(at), v.add(at + 1), pop_val - at);
                            let slot = v.add(at);
                            slot.write(0);
                            self.root = Root::Leaf {
                                ptr: ptr_val,
                                pop: pop_val + 1,
                            };
                            core::ptr::NonNull::new(slot).expect("slot")
                        }
                    } else {
                        let new = alloc.alloc_bytes_dispatch::<OCC>(leaf_size(pop_val + 1));
                        // SAFETY: copy keys and values around insertion point into new leaf.
                        unsafe {
                            let nk = new.as_ptr().cast::<u64>();
                            nk.copy_from_nonoverlapping(keys.as_ptr(), at);
                            nk.add(at).write(key);
                            nk.add(at + 1)
                                .copy_from_nonoverlapping(keys.as_ptr().add(at), pop_val - at);
                            let nv = new
                                .as_ptr()
                                .add(leaf_values_offset(pop_val + 1))
                                .cast::<u64>();
                            nv.copy_from_nonoverlapping(vals, at);
                            let slot = nv.add(at);
                            slot.write(0);
                            nv.add(at + 1)
                                .copy_from_nonoverlapping(vals.add(at), pop_val - at);
                            alloc.free_bytes_dispatch::<OCC>(ptr_val, leaf_size(pop_val));
                            self.root = Root::Leaf {
                                ptr: new,
                                pop: pop_val + 1,
                            };
                            core::ptr::NonNull::new(slot).expect("slot")
                        }
                    }
                } else {
                    self.insert_dispatch::<OCC, NESTED>(alloc, key, 0, path);
                    self.get_value_slot(key, path).expect("just-ensured key")
                }
            }
        }
    }

    /// Whether the root is a level-8 trie (#568 PR 3): on a shared tree the
    /// wrapper brackets a root-leaf-state operation with the tree word
    /// itself, since every store then is a root-state write, and leaves a
    /// tree-state operation to the engine's per-node brackets. `StrCursor`
    /// also reads it to step a root-leaf level positionally rather than build
    /// a sub-map cursor for it.
    #[inline(always)]
    pub(crate) fn root_is_tree(&self) -> bool {
        matches!(self.root, Root::Tree { .. })
    }

    #[inline(always)]
    #[cfg(feature = "std")]
    pub(crate) unsafe fn root_top_ptr(&self) -> *mut Edge {
        match &self.root {
            Root::Tree { top } => (top as *const Edge).cast_mut(),
            _ => core::ptr::null_mut(),
        }
    }

    /// Phase 7 (occ): by-value root snapshot for the validated concurrent
    /// read walk (see `ExpanseSet::occ_root`).
    #[inline(always)]
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) fn occ_snapshot(&self) -> RootSnapshot {
        // A plain read of the root state that races a covered writer's
        // stores. The reader validates the tree word before acting on the
        // value, which makes it safe to discard, not the read race-free
        // (#1086, class 1).
        match &self.root {
            Root::Empty => RootSnapshot::Empty,
            Root::Leaf { ptr, pop } => RootSnapshot::Leaf {
                ptr: ptr.as_ptr(),
                pop: *pop,
            },
            Root::Tree { top } => RootSnapshot::Tree { top: *top },
        }
    }

    /// [`Self::occ_snapshot`] through a raw pointer, forming no reference to
    /// the core (#1086): a string node's core is read this way while the
    /// writer holding the node's cover may be storing to it. The tag and the
    /// variant's words are read one word at a time, so a torn read yields a
    /// snapshot the caller's validation discards, never an invalid `Root`.
    ///
    /// # Safety
    ///
    /// `this` points to a live core; the caller validates the result.
    #[inline(always)]
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) unsafe fn occ_snapshot_of(this: *const Self) -> RootSnapshot {
        // SAFETY: caller contract; every read stays inside the 24-byte
        // `Root` at the offsets the layout asserts above pin.
        unsafe {
            let r = (&raw const (*this).root).cast_mut();
            match root_word::tag(r) {
                ROOT_EMPTY => RootSnapshot::Empty,
                ROOT_LEAF => RootSnapshot::Leaf {
                    ptr: root_word::ptr(r).cast_const(),
                    pop: root_word::third(r) as usize,
                },
                _ => RootSnapshot::Tree {
                    top: Edge::from_words(root_word::ptr(r), root_word::third(r)),
                },
            }
        }
    }

    /// [`Self::insert_pathless`] for the holder of a string node's cover
    /// lock, through a raw pointer to the node's core (#1086): readers copy
    /// the core while the holder stores to it, so the holder must not hold
    /// `&mut MapCore`. The root must be empty or a root leaf (a tree's
    /// stores are the engine's per-node brackets, not the cover's).
    ///
    /// # Safety
    ///
    /// `this` points to a live core of a deferred, shared tree whose root is
    /// not a tree, and the caller holds the lock that makes it the core's
    /// only writer.
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    #[inline(always)]
    pub(crate) unsafe fn insert_leaf_state_at(
        this: *mut Self,
        alloc: &NodeAlloc,
        key: Key,
        val: u64,
    ) -> Option<u64> {
        debug_assert!(alloc.occ_enabled() && !alloc.engine_covers_root());
        // SAFETY: caller contract; the root is copied and stored by value
        // through field projections, never borrowed.
        unsafe {
            let rp = &raw mut (*this).root;
            let o = leaf_state_insert_shared(
                rp.read(),
                alloc,
                key,
                val,
                &mut crate::mutate_map::InsertPathMap::empty(),
            );
            if let Some(pop) = o.tree_pop {
                root_word::store_tree_pop(&raw mut (*this).tree_pop, pop);
            }
            if let Some(root) = o.root {
                #[cfg(feature = "occ-stats")]
                if root_fingerprint_of(&rp.read()) != root_fingerprint_of(&root) {
                    crate::occ_stats::note_root_rewrite();
                }
                root_word::store(rp, root);
            }
            o.old
        }
    }

    /// [`Self::remove_pathless`] for the holder of a string node's cover
    /// lock; the twin of [`Self::insert_leaf_state_at`].
    ///
    /// # Safety
    ///
    /// As [`Self::insert_leaf_state_at`].
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    #[inline(always)]
    pub(crate) unsafe fn remove_leaf_state_at(
        this: *mut Self,
        alloc: &NodeAlloc,
        key: Key,
    ) -> Option<u64> {
        debug_assert!(alloc.occ_enabled() && !alloc.engine_covers_root());
        // SAFETY: as in `insert_leaf_state_at`.
        unsafe {
            let rp = &raw mut (*this).root;
            let (old, root) = leaf_state_remove_shared(rp.read(), alloc, key);
            if let Some(root) = root {
                #[cfg(feature = "occ-stats")]
                if root_fingerprint_of(&rp.read()) != root_fingerprint_of(&root) {
                    crate::occ_stats::note_root_rewrite();
                }
                root_word::store(rp, root);
            }
            old
        }
    }

    /// Whether the root is a tree, through a raw pointer (see
    /// [`Self::occ_snapshot_of`]).
    ///
    /// # Safety
    ///
    /// As [`Self::occ_snapshot_of`].
    #[inline(always)]
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) unsafe fn root_is_tree_of(this: *const Self) -> bool {
        // SAFETY: caller contract; the tag word heads `Root`.
        unsafe { root_word::tag((&raw const (*this).root).cast_mut()) == ROOT_TREE }
    }

    /// The entry count, through a raw pointer (see
    /// [`Self::occ_snapshot_of`]).
    ///
    /// # Safety
    ///
    /// As [`Self::occ_snapshot_of`].
    #[inline(always)]
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) unsafe fn len_of(this: *const Self) -> u64 {
        // SAFETY: caller contract; offsets as in `occ_snapshot_of`.
        unsafe {
            let r = (&raw const (*this).root).cast_mut();
            match root_word::tag(r) {
                ROOT_EMPTY => 0,
                ROOT_LEAF => root_word::third(r),
                _ => root_word::tree_pop(&raw const (*this).tree_pop),
            }
        }
    }

    /// [`Self::root_top_ptr`] through a raw pointer: the top edge, with the
    /// provenance of `this`, or `None` when the root is not a tree (no null
    /// constant flows towards a dereference; see `ExpanseStrMap::root_raw`).
    ///
    /// # Safety
    ///
    /// As [`Self::occ_snapshot_of`].
    #[inline(always)]
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) unsafe fn root_top_ptr_of(this: *mut Self) -> Option<NonNull<Edge>> {
        // SAFETY: caller contract; the top edge sits at the offset the
        // layout asserts pin.
        unsafe {
            let r = &raw mut (*this).root;
            if root_word::tag(r) == ROOT_TREE {
                NonNull::new(
                    r.cast::<u8>()
                        .add(core::mem::offset_of!(RootTreeLayout, top))
                        .cast::<Edge>(),
                )
            } else {
                None
            }
        }
    }

    /// Insert for the holder of a string node's cover on the exclusive path,
    /// in any root state, through a raw pointer to the node's core (#1086):
    /// readers copy the core and walk its tree meanwhile, so the holder forms
    /// no `&mut` to the core or to its top edge. A root leaf goes through
    /// [`Self::insert_leaf_state_at`]; a tree through the engine's OCC walk
    /// from the top edge in place.
    ///
    /// # Safety
    ///
    /// As [`Self::insert_leaf_state_at`], except that the root may be in any
    /// state, and every other writer of the core is excluded.
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) unsafe fn insert_covered_at(
        this: *mut Self,
        alloc: &NodeAlloc,
        key: Key,
        val: u64,
    ) -> Option<u64> {
        // SAFETY: caller contract.
        unsafe {
            let Some(top) = Self::root_top_ptr_of(this) else {
                return Self::insert_leaf_state_at(this, alloc, key, val);
            };
            Self::tree_insert_at::<false>(this, alloc, top.as_ptr(), key, val).0
        }
    }

    /// [`Self::insert_covered_at`] as insert-if-absent, returning the value
    /// slot: the `ins_slot` of a string node's sub-map.
    ///
    /// # Safety
    ///
    /// As [`Self::insert_covered_at`].
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) unsafe fn ins_slot_covered_at(
        this: *mut Self,
        alloc: &NodeAlloc,
        key: Key,
    ) -> core::ptr::NonNull<u64> {
        // SAFETY: caller contract.
        unsafe {
            if !Self::root_is_tree_of(this) {
                if let Some(slot) = Self::leaf_slot_of(this, key) {
                    return slot;
                }
                Self::insert_leaf_state_at(this, alloc, key, 0);
                if let Some(slot) = Self::leaf_slot_of(this, key) {
                    return slot;
                }
                // The insert promoted the root leaf to a tree: find the
                // slot the way a tree insert-if-absent does.
            }
            let top = Self::root_top_ptr_of(this).expect("a root that is not a leaf is a tree");
            let (_, slot) = Self::tree_insert_at::<true>(this, alloc, top.as_ptr(), key, 0);
            core::ptr::NonNull::new(slot).expect("insert returns a slot")
        }
    }

    /// Stores the tree population through a raw pointer (see
    /// [`Self::occ_snapshot_of`]).
    ///
    /// # Safety
    ///
    /// As [`Self::insert_covered_at`].
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    #[inline(always)]
    pub(crate) unsafe fn set_tree_pop_at(this: *mut Self, pop: u64) {
        // SAFETY: caller contract.
        unsafe { root_word::store_tree_pop(&raw mut (*this).tree_pop, pop) };
    }

    /// The tree arm of [`Self::insert_covered_at`]: the engine's OCC walk
    /// from `top`, and the population's store.
    ///
    /// # Safety
    ///
    /// As [`Self::insert_covered_at`], and `top` is the core's top edge.
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    #[inline(always)]
    unsafe fn tree_insert_at<const KEEP: bool>(
        this: *mut Self,
        alloc: &NodeAlloc,
        top: *mut Edge,
        key: Key,
        val: u64,
    ) -> (Option<u64>, *mut u64) {
        // SAFETY: caller contract.
        unsafe {
            let r = mutate_map::map_insert_with_path_occ::<KEEP, true, true>(
                alloc,
                top,
                key,
                val,
                8,
                &mut crate::mutate_map::InsertPathMap::empty(),
                crate::occ::Cover::Tree,
            );
            if r.0.is_none() {
                let p = &raw mut (*this).tree_pop;
                root_word::store_tree_pop(p, root_word::tree_pop(p) + 1);
            }
            r
        }
    }

    /// The value slot of `key` in a root leaf, through a raw pointer; `None`
    /// when the root is not a leaf or holds no such key.
    ///
    /// # Safety
    ///
    /// As [`Self::occ_snapshot_of`], and no other thread stores to the core.
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    unsafe fn leaf_slot_of(this: *mut Self, key: Key) -> Option<core::ptr::NonNull<u64>> {
        // SAFETY: caller contract; the leaf is live while the root names it.
        unsafe {
            let RootSnapshot::Leaf { ptr, pop } = Self::occ_snapshot_of(this) else {
                return None;
            };
            let ptr = core::ptr::NonNull::new(ptr.cast_mut())?;
            let (keys, vals) = Self::leaf_parts(ptr, pop);
            let at = keys.binary_search(&key).ok()?;
            core::ptr::NonNull::new(vals.add(at))
        }
    }

    /// [`Self::insert_covered_at`]'s removal twin, including the root-state
    /// changes a removal makes: a tree emptied to nothing, and a tree
    /// condensed back to a root leaf.
    ///
    /// # Safety
    ///
    /// As [`Self::insert_covered_at`].
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) unsafe fn remove_covered_at(
        this: *mut Self,
        alloc: &NodeAlloc,
        key: Key,
    ) -> Option<u64> {
        // SAFETY: caller contract.
        unsafe {
            let Some(top) = Self::root_top_ptr_of(this) else {
                return Self::remove_leaf_state_at(this, alloc, key);
            };
            let top = top.as_ptr();
            let old = mutate_map::map_remove_occ::<true, true>(
                alloc,
                top,
                key,
                8,
                crate::occ::Cover::Tree,
            )?;
            let p = &raw mut (*this).tree_pop;
            let now = root_word::tree_pop(p) - 1;
            root_word::store_tree_pop(p, now);
            let rp = &raw mut (*this).root;
            if now == 0 {
                debug_assert!(Edge::load_at::<true>(top).is_null());
                root_word::store(rp, Root::Empty);
            } else if now < ROOT_LEAF_CAP as u64 {
                let mut local = Edge::load_at::<true>(top);
                let leaf = Self::condensed_leaf(alloc, &local, now as usize);
                root_word::store(rp, leaf);
                mutate::free_subtree::<true, true>(alloc, &mut local);
            }
            Some(old)
        }
    }

    /// Empties the core through a raw pointer: the removal twin of a whole
    /// string node's sub-map, for a node being disposed of while pinned
    /// readers may still read it.
    ///
    /// # Safety
    ///
    /// As [`Self::insert_covered_at`].
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) unsafe fn clear_at(this: *mut Self, alloc: &NodeAlloc) {
        // SAFETY: caller contract; the root is stored before what it named
        // is retired.
        unsafe {
            let rp = &raw mut (*this).root;
            match Self::occ_snapshot_of(this) {
                RootSnapshot::Empty => {}
                RootSnapshot::Leaf { ptr, pop } => {
                    root_word::store(rp, Root::Empty);
                    alloc.free_bytes_dispatch::<true>(
                        core::ptr::NonNull::new(ptr.cast_mut()).expect("leaf"),
                        leaf_size(pop),
                    );
                }
                RootSnapshot::Tree { top } => {
                    let mut local = top;
                    root_word::store(rp, Root::Empty);
                    mutate::free_subtree::<true, true>(alloc, &mut local);
                }
            }
        }
    }

    /// The root leaf holding the `n` entries of the tree under `top`, built
    /// privately: the condense step of a removal.
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    fn condensed_leaf(alloc: &NodeAlloc, top: &Edge, n: usize) -> Root {
        let new = alloc.alloc_bytes_dispatch::<true>(leaf_size(n));
        let mut written = 0usize;
        let mut from = Some(0u64);
        // SAFETY: engine-maintained trie; the caller excludes its writers.
        while let Some((k, v)) = from.and_then(|f| unsafe { crate::nav::next::<true>(top, f, 8) }) {
            debug_assert!(written < n);
            // SAFETY: in-bounds writes to a private allocation.
            unsafe {
                new.as_ptr().cast::<u64>().add(written).write(k);
                new.as_ptr()
                    .add(leaf_values_offset(n))
                    .cast::<u64>()
                    .add(written)
                    .write(v);
            }
            written += 1;
            from = k.checked_add(1);
        }
        debug_assert_eq!(written, n);
        Root::Leaf { ptr: new, pop: n }
    }

    /// Membership test.
    #[inline(always)]
    #[must_use]
    pub(crate) fn contains_key(&self, key: Key) -> bool {
        self.get(key).is_some()
    }

    /// The root state that a tree-level bracket must cover, as a value:
    /// variant, allocation address and the top edge's two words. Population
    /// is deliberately excluded — a `pop` bump alone is not a root rewrite.
    #[cfg(feature = "occ-stats")]
    fn root_fingerprint(&self) -> (u8, u64, u64) {
        root_fingerprint_of(&self.root)
    }

    /// Inserts `key → val`; returns the replaced value if the key was
    /// already present.
    #[inline(always)]
    pub(crate) fn insert(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
        val: u64,
        path: &mut crate::mutate_map::InsertPathMap,
    ) -> Option<u64> {
        noting_root_rewrite!(self, m => m.insert_inner(alloc, key, val, path))
    }

    /// [`Self::insert`] on a tree whose readers run concurrently (#1086):
    /// [`Self::insert_inner_shared`].
    #[cfg(feature = "std")]
    #[inline(always)]
    pub(crate) fn insert_shared(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
        val: u64,
        path: &mut crate::mutate_map::InsertPathMap,
    ) -> Option<u64> {
        noting_root_rewrite!(self, m => m.insert_inner_shared(alloc, key, val, path))
    }

    /// Single-threaded insert, bypassing OCC checks.
    #[inline(always)]
    pub(crate) fn insert_plain(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
        val: u64,
        path: &mut crate::mutate_map::InsertPathMap,
    ) -> Option<u64> {
        noting_root_rewrite!(self, m => m.insert_inner_plain(alloc, key, val, path))
    }

    #[inline(always)]
    pub(crate) fn insert_dispatch<const OCC: bool, const NESTED: bool>(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
        val: u64,
        path: &mut crate::mutate_map::InsertPathMap,
    ) -> Option<u64> {
        noting_root_rewrite!(self, m => m.insert_inner_dispatch::<OCC, NESTED>(alloc, key, val, path))
    }

    #[inline(always)]
    fn insert_inner(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
        val: u64,
        path: &mut crate::mutate_map::InsertPathMap,
    ) -> Option<u64> {
        match &mut self.root {
            Root::Empty => {
                let ptr = alloc.alloc_bytes(leaf_size(1));
                // SAFETY: fresh allocation: key slot then value slot.
                unsafe {
                    ptr.as_ptr().cast::<u64>().write(key);
                    ptr.as_ptr()
                        .add(leaf_values_offset(1))
                        .cast::<u64>()
                        .write(val);
                }
                self.root = Root::Leaf { ptr, pop: 1 };
                None
            }
            Root::Leaf { ptr, pop } => {
                let (ptr, pop) = (*ptr, *pop);
                let (keys, vals) = Self::leaf_parts(ptr, pop);
                let (hit, at) = if pop > 0 {
                    let last = keys[pop - 1];
                    if key > last {
                        (false, pop)
                    } else if key == last {
                        (true, pop - 1)
                    } else {
                        match keys.binary_search(&key) {
                            Ok(pos) => (true, pos),
                            Err(pos) => (false, pos),
                        }
                    }
                } else {
                    (false, 0)
                };
                if hit {
                    // SAFETY: in-place value swap.
                    unsafe {
                        let slot = vals.add(at);
                        let old = *slot;
                        slot.write(val);
                        return Some(old);
                    }
                }
                if pop < ROOT_LEAF_CAP {
                    if leaf_size(pop + 1) == leaf_size(pop) {
                        // Spare class capacity: shift both areas in
                        // place, no allocation and no copy of the
                        // whole leaf.
                        // SAFETY: same class, so the areas keep their
                        // offsets and the extra slot is in bounds.
                        unsafe {
                            let base = ptr.as_ptr().cast::<u64>();
                            core::ptr::copy(base.add(at), base.add(at + 1), pop - at);
                            base.add(at).write(key);
                            let v = ptr.as_ptr().add(leaf_values_offset(pop)).cast::<u64>();
                            core::ptr::copy(v.add(at), v.add(at + 1), pop - at);
                            v.add(at).write(val);
                        }
                        self.root = Root::Leaf { ptr, pop: pop + 1 };
                        return None;
                    }
                    let new = alloc.alloc_bytes(leaf_size(pop + 1));
                    // SAFETY: copy keys and values around the insertion
                    // point into the fresh (pop + 1)-entry leaf.
                    unsafe {
                        let nk = new.as_ptr().cast::<u64>();
                        nk.copy_from_nonoverlapping(keys.as_ptr(), at);
                        nk.add(at).write(key);
                        nk.add(at + 1)
                            .copy_from_nonoverlapping(keys.as_ptr().add(at), pop - at);
                        let nv = new.as_ptr().add(leaf_values_offset(pop + 1)).cast::<u64>();
                        nv.copy_from_nonoverlapping(vals, at);
                        nv.add(at).write(val);
                        nv.add(at + 1)
                            .copy_from_nonoverlapping(vals.add(at), pop - at);
                        alloc.free_bytes(ptr, leaf_size(pop));
                    }
                    self.root = Root::Leaf {
                        ptr: new,
                        pop: pop + 1,
                    };
                    None
                } else {
                    // Root leaf overflow: build the level-8 trie. The build
                    // is private until `self.root` is set; on a shared tree
                    // the wrapper holds the tree bracket for this root-state
                    // change (`sync::Shared::write_root_covered`) and the
                    // shared engine builds under a scratch word — never the
                    // tree word (a nested `begin` would make it even
                    // mid-write), never a node's own (an upgrade inside the
                    // build marks that node obsolete, which needs it even).
                    let top = by_mode!(alloc, promote_leaf(alloc, keys, vals, key, val, path));
                    // SAFETY: old root leaf no longer referenced.
                    unsafe { alloc.free_bytes(ptr, leaf_size(pop)) };
                    self.tree_pop = pop as u64 + 1;
                    self.root = Root::Tree { top };
                    None
                }
            }
            Root::Tree { top } => {
                let prefix = key >> 8;
                // A warm path never exists on a shared tree: the shared engine
                // clears the cache on entry and records nothing, so this bypass
                // is structurally cold there (AGENTS.md §2.1.5); the assert is
                // the tripwire, not the guard.
                if path.prefix == prefix {
                    alloc.assert_bracketed();
                    if let Some(mut leaf) = core::ptr::NonNull::new(path.leaf) {
                        let d = (key & 0xFF) as u8;
                        // SAFETY: path holds valid live LeafBitmapL pointer.
                        let node = unsafe { leaf.as_mut() };
                        let sub = (d >> 5) as usize;
                        if let Some(rank) = node.bitmap.test_and_subexpanse_rank(d) {
                            // SAFETY: value subarray holds subexpanse_count values; in-place swap.
                            unsafe {
                                let slot = node.values[sub].add(rank);
                                let old = *slot;
                                slot.write(val);
                                return Some(old);
                            }
                        }
                        let rank = node.bitmap.subexpanse_rank(d) as usize;
                        let old_n = node.bitmap.subexpanse_count(sub) as usize;
                        if old_n > 0
                            && crate::leaf::cap_class(old_n + 1) == crate::leaf::cap_class(old_n)
                        {
                            // Fast path: spare class capacity — shift in place.
                            // SAFETY: the subarray holds cap_class(old_n) slots.
                            unsafe {
                                let arr = node.values[sub];
                                core::ptr::copy(arr.add(rank), arr.add(rank + 1), old_n - rank);
                                arr.add(rank).write(val);
                            }
                        } else {
                            let new = alloc
                                .alloc_bytes_plain(crate::mutate::sub_vals_size(old_n + 1))
                                .cast::<u64>();
                            // SAFETY: copy old_n values around the inserted rank.
                            unsafe {
                                if old_n > 0 {
                                    let old = node.values[sub];
                                    new.as_ptr().copy_from_nonoverlapping(old, rank);
                                    new.as_ptr()
                                        .add(rank + 1)
                                        .copy_from_nonoverlapping(old.add(rank), old_n - rank);
                                    alloc.free_bytes_plain(
                                        core::ptr::NonNull::new(old.cast()).expect("values"),
                                        crate::mutate::sub_vals_size(old_n),
                                    );
                                }
                                new.as_ptr().add(rank).write(val);
                            }
                            node.values[sub] = new.as_ptr();
                        }
                        node.bitmap.set(d);
                        path.pending_pop += 1;
                        path.terminal_pop += 1;
                        self.tree_pop += 1;
                        debug_assert!(!path.edges[0].is_null());
                        // SAFETY: keep terminal edge pop0 up to date. The warm path is
                        // armed only where `edges[0]` is set beside `prefix`,
                        // `leaf`/`leaf1` and `depth` (mutate_map.rs:742, 821, 860, 922);
                        // `clear()` resets `prefix` to `u64::MAX`, which matches no
                        // `key >> 8`, so a cleared path never reaches here.
                        unsafe {
                            core::ptr::NonNull::new_unchecked(path.edges[0])
                                .as_mut()
                                .set_pop0(1, (path.terminal_pop - 1) as u64);
                        }
                        return None;
                    } else if let Some(leaf1) = core::ptr::NonNull::new(path.leaf1) {
                        let d = (key & 0xFF) as u8;
                        let cur_pop = path.terminal_pop as usize;
                        let base = leaf1.as_ptr();
                        // SAFETY: base points to a live Leaf1 allocation; map_keys_offset is in-bounds.
                        let keys_ptr = unsafe { base.add(crate::leaf::map_keys_offset(cur_pop)) };
                        // SAFETY: cur_pop >= 1 when leaf1 is active, so cur_pop - 1 is in bounds.
                        let last = unsafe { *keys_ptr.add(cur_pop - 1) };
                        if d > last {
                            if cur_pop < crate::mutate::LEAF1_CAP
                                && crate::leaf::cap_class(cur_pop + 1)
                                    == crate::leaf::cap_class(cur_pop)
                            {
                                // SAFETY: spare class capacity in the live Leaf1 allocation.
                                unsafe {
                                    *keys_ptr.add(cur_pop) = d;
                                    let vals = base.cast::<u64>();
                                    vals.add(cur_pop).write(val);
                                    debug_assert!(!path.edges[0].is_null());
                                    // SAFETY: the warm path is armed only where `edges[0]` is set beside
                                    // `prefix`, `leaf`/`leaf1` and `depth` (mutate_map.rs:742, 821, 860,
                                    // 922); `clear()` resets `prefix` to `u64::MAX`, which matches no
                                    // `key >> 8`, so a cleared path never reaches here.
                                    core::ptr::NonNull::new_unchecked(path.edges[0])
                                        .as_mut()
                                        .set_pop0(1, cur_pop as u64);
                                }
                                path.terminal_pop += 1;
                                path.pending_pop += 1;
                                self.tree_pop += 1;
                                return None;
                            }
                        } else if d == last {
                            // SAFETY: cur_pop - 1 is the existing slot for `last`.
                            unsafe {
                                let vals = base.cast::<u64>();
                                let slot = vals.add(cur_pop - 1);
                                let old = *slot;
                                slot.write(val);
                                return Some(old);
                            }
                        }
                    }
                }
                path.clear();
                // One OCC check per operation, where the runtime dispatch
                // always sat; the shared monomorph brackets every store by the
                // node that holds it and bumps the population atomically.
                by_mode!(
                    alloc,
                    tree_insert::<false>(alloc, &mut self.tree_pop, path, top, key, val)
                )
                .0
            }
        }
    }

    /// [`Self::insert_inner`] for a shared tree: the stores into a live root
    /// leaf are atomic words (`bits::shared_word`). A copy of the
    /// plain body, not a generic one: making the plain body generic over the
    /// access mode changed the plain callers' register allocation (#1086).
    #[cfg(feature = "std")]
    #[inline(always)]
    fn insert_inner_shared(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
        val: u64,
        path: &mut crate::mutate_map::InsertPathMap,
    ) -> Option<u64> {
        match &mut self.root {
            Root::Empty => {
                let ptr = alloc.alloc_bytes(leaf_size(1));
                // SAFETY: fresh allocation: key slot then value slot.
                unsafe {
                    ptr.as_ptr().cast::<u64>().write(key);
                    ptr.as_ptr()
                        .add(leaf_values_offset(1))
                        .cast::<u64>()
                        .write(val);
                }
                self.root = Root::Leaf { ptr, pop: 1 };
                None
            }
            Root::Leaf { ptr, pop } => {
                let (ptr, pop) = (*ptr, *pop);
                let (keys, vals) = Self::leaf_parts(ptr, pop);
                let (hit, at) = if pop > 0 {
                    let last = keys[pop - 1];
                    if key > last {
                        (false, pop)
                    } else if key == last {
                        (true, pop - 1)
                    } else {
                        match keys.binary_search(&key) {
                            Ok(pos) => (true, pos),
                            Err(pos) => (false, pos),
                        }
                    }
                } else {
                    (false, 0)
                };
                if hit {
                    // SAFETY: in-place value swap.
                    unsafe {
                        let slot = vals.add(at);
                        let old = *slot;
                        shared_word::store::<true>(slot, val);
                        return Some(old);
                    }
                }
                if pop < ROOT_LEAF_CAP {
                    if leaf_size(pop + 1) == leaf_size(pop) {
                        // Spare class capacity: shift both areas in
                        // place, no allocation and no copy of the
                        // whole leaf.
                        // SAFETY: same class, so the areas keep their
                        // offsets and the extra slot is in bounds.
                        unsafe {
                            let base = ptr.as_ptr().cast::<u64>();
                            shared_word::shift_up::<true>(base, at, pop - at);
                            shared_word::store::<true>(base.add(at), key);
                            let v = ptr.as_ptr().add(leaf_values_offset(pop)).cast::<u64>();
                            shared_word::shift_up::<true>(v, at, pop - at);
                            shared_word::store::<true>(v.add(at), val);
                        }
                        self.root = Root::Leaf { ptr, pop: pop + 1 };
                        return None;
                    }
                    let new = alloc.alloc_bytes(leaf_size(pop + 1));
                    // SAFETY: copy keys and values around the insertion
                    // point into the fresh (pop + 1)-entry leaf.
                    unsafe {
                        let nk = new.as_ptr().cast::<u64>();
                        nk.copy_from_nonoverlapping(keys.as_ptr(), at);
                        nk.add(at).write(key);
                        nk.add(at + 1)
                            .copy_from_nonoverlapping(keys.as_ptr().add(at), pop - at);
                        let nv = new.as_ptr().add(leaf_values_offset(pop + 1)).cast::<u64>();
                        nv.copy_from_nonoverlapping(vals, at);
                        nv.add(at).write(val);
                        nv.add(at + 1)
                            .copy_from_nonoverlapping(vals.add(at), pop - at);
                        alloc.free_bytes(ptr, leaf_size(pop));
                    }
                    self.root = Root::Leaf {
                        ptr: new,
                        pop: pop + 1,
                    };
                    None
                } else {
                    // Root leaf overflow: build the level-8 trie. The build
                    // is private until `self.root` is set; on a shared tree
                    // the wrapper holds the tree bracket for this root-state
                    // change (`sync::Shared::write_root_covered`) and the
                    // shared engine builds under a scratch word — never the
                    // tree word (a nested `begin` would make it even
                    // mid-write), never a node's own (an upgrade inside the
                    // build marks that node obsolete, which needs it even).
                    let top = by_mode!(alloc, promote_leaf(alloc, keys, vals, key, val, path));
                    // SAFETY: old root leaf no longer referenced.
                    unsafe { alloc.free_bytes(ptr, leaf_size(pop)) };
                    self.tree_pop = pop as u64 + 1;
                    self.root = Root::Tree { top };
                    None
                }
            }
            Root::Tree { top } => {
                let prefix = key >> 8;
                // A warm path never exists on a shared tree: the shared engine
                // clears the cache on entry and records nothing, so this bypass
                // is structurally cold there (AGENTS.md §2.1.5); the assert is
                // the tripwire, not the guard.
                if path.prefix == prefix {
                    alloc.assert_bracketed();
                    if let Some(mut leaf) = core::ptr::NonNull::new(path.leaf) {
                        let d = (key & 0xFF) as u8;
                        // SAFETY: path holds valid live LeafBitmapL pointer.
                        let node = unsafe { leaf.as_mut() };
                        let sub = (d >> 5) as usize;
                        if let Some(rank) = node.bitmap.test_and_subexpanse_rank(d) {
                            // SAFETY: value subarray holds subexpanse_count values; in-place swap.
                            unsafe {
                                let slot = node.values[sub].add(rank);
                                let old = *slot;
                                slot.write(val);
                                return Some(old);
                            }
                        }
                        let rank = node.bitmap.subexpanse_rank(d) as usize;
                        let old_n = node.bitmap.subexpanse_count(sub) as usize;
                        if old_n > 0
                            && crate::leaf::cap_class(old_n + 1) == crate::leaf::cap_class(old_n)
                        {
                            // Fast path: spare class capacity — shift in place.
                            // SAFETY: the subarray holds cap_class(old_n) slots.
                            unsafe {
                                let arr = node.values[sub];
                                core::ptr::copy(arr.add(rank), arr.add(rank + 1), old_n - rank);
                                arr.add(rank).write(val);
                            }
                        } else {
                            let new = alloc
                                .alloc_bytes_plain(crate::mutate::sub_vals_size(old_n + 1))
                                .cast::<u64>();
                            // SAFETY: copy old_n values around the inserted rank.
                            unsafe {
                                if old_n > 0 {
                                    let old = node.values[sub];
                                    new.as_ptr().copy_from_nonoverlapping(old, rank);
                                    new.as_ptr()
                                        .add(rank + 1)
                                        .copy_from_nonoverlapping(old.add(rank), old_n - rank);
                                    alloc.free_bytes_plain(
                                        core::ptr::NonNull::new(old.cast()).expect("values"),
                                        crate::mutate::sub_vals_size(old_n),
                                    );
                                }
                                new.as_ptr().add(rank).write(val);
                            }
                            node.values[sub] = new.as_ptr();
                        }
                        node.bitmap.set(d);
                        path.pending_pop += 1;
                        path.terminal_pop += 1;
                        self.tree_pop += 1;
                        debug_assert!(!path.edges[0].is_null());
                        // SAFETY: keep terminal edge pop0 up to date. The warm path is
                        // armed only where `edges[0]` is set beside `prefix`,
                        // `leaf`/`leaf1` and `depth` (mutate_map.rs:742, 821, 860, 922);
                        // `clear()` resets `prefix` to `u64::MAX`, which matches no
                        // `key >> 8`, so a cleared path never reaches here.
                        unsafe {
                            core::ptr::NonNull::new_unchecked(path.edges[0])
                                .as_mut()
                                .set_pop0(1, (path.terminal_pop - 1) as u64);
                        }
                        return None;
                    } else if let Some(leaf1) = core::ptr::NonNull::new(path.leaf1) {
                        let d = (key & 0xFF) as u8;
                        let cur_pop = path.terminal_pop as usize;
                        let base = leaf1.as_ptr();
                        // SAFETY: base points to a live Leaf1 allocation; map_keys_offset is in-bounds.
                        let keys_ptr = unsafe { base.add(crate::leaf::map_keys_offset(cur_pop)) };
                        // SAFETY: cur_pop >= 1 when leaf1 is active, so cur_pop - 1 is in bounds.
                        let last = unsafe { *keys_ptr.add(cur_pop - 1) };
                        if d > last {
                            if cur_pop < crate::mutate::LEAF1_CAP
                                && crate::leaf::cap_class(cur_pop + 1)
                                    == crate::leaf::cap_class(cur_pop)
                            {
                                // SAFETY: spare class capacity in the live Leaf1 allocation.
                                unsafe {
                                    *keys_ptr.add(cur_pop) = d;
                                    let vals = base.cast::<u64>();
                                    vals.add(cur_pop).write(val);
                                    debug_assert!(!path.edges[0].is_null());
                                    // SAFETY: the warm path is armed only where `edges[0]` is set beside
                                    // `prefix`, `leaf`/`leaf1` and `depth` (mutate_map.rs:742, 821, 860,
                                    // 922); `clear()` resets `prefix` to `u64::MAX`, which matches no
                                    // `key >> 8`, so a cleared path never reaches here.
                                    core::ptr::NonNull::new_unchecked(path.edges[0])
                                        .as_mut()
                                        .set_pop0(1, cur_pop as u64);
                                }
                                path.terminal_pop += 1;
                                path.pending_pop += 1;
                                self.tree_pop += 1;
                                return None;
                            }
                        } else if d == last {
                            // SAFETY: cur_pop - 1 is the existing slot for `last`.
                            unsafe {
                                let vals = base.cast::<u64>();
                                let slot = vals.add(cur_pop - 1);
                                let old = *slot;
                                slot.write(val);
                                return Some(old);
                            }
                        }
                    }
                }
                path.clear();
                // One OCC check per operation, where the runtime dispatch
                // always sat; the shared monomorph brackets every store by the
                // node that holds it and bumps the population atomically.
                by_mode!(
                    alloc,
                    tree_insert::<false>(alloc, &mut self.tree_pop, path, top, key, val)
                )
                .0
            }
        }
    }

    #[inline(always)]
    fn insert_inner_plain(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
        val: u64,
        path: &mut crate::mutate_map::InsertPathMap,
    ) -> Option<u64> {
        match &mut self.root {
            Root::Empty => {
                let ptr = alloc.alloc_bytes_plain(leaf_size(1));
                // SAFETY: fresh allocation: key slot then value slot.
                unsafe {
                    ptr.as_ptr().cast::<u64>().write(key);
                    ptr.as_ptr()
                        .add(leaf_values_offset(1))
                        .cast::<u64>()
                        .write(val);
                }
                self.root = Root::Leaf { ptr, pop: 1 };
                None
            }
            Root::Leaf { ptr, pop } => {
                let (ptr, pop) = (*ptr, *pop);
                let (keys, vals) = Self::leaf_parts(ptr, pop);
                let (hit, at) = if pop > 0 {
                    let last = keys[pop - 1];
                    if key > last {
                        (false, pop)
                    } else if key == last {
                        (true, pop - 1)
                    } else {
                        match keys.binary_search(&key) {
                            Ok(pos) => (true, pos),
                            Err(pos) => (false, pos),
                        }
                    }
                } else {
                    (false, 0)
                };
                if hit {
                    // SAFETY: in-place value swap.
                    unsafe {
                        let slot = vals.add(at);
                        let old = *slot;
                        slot.write(val);
                        return Some(old);
                    }
                }
                if pop < ROOT_LEAF_CAP {
                    if leaf_size(pop + 1) == leaf_size(pop) {
                        // Spare class capacity: shift both areas in
                        // place, no allocation and no copy of the
                        // whole leaf.
                        // SAFETY: same class, so the areas keep their
                        // offsets and the extra slot is in bounds.
                        unsafe {
                            let base = ptr.as_ptr().cast::<u64>();
                            core::ptr::copy(base.add(at), base.add(at + 1), pop - at);
                            base.add(at).write(key);
                            let v = ptr.as_ptr().add(leaf_values_offset(pop)).cast::<u64>();
                            core::ptr::copy(v.add(at), v.add(at + 1), pop - at);
                            v.add(at).write(val);
                        }
                        self.root = Root::Leaf { ptr, pop: pop + 1 };
                        return None;
                    }
                    let new = alloc.alloc_bytes_plain(leaf_size(pop + 1));
                    // SAFETY: copy keys and values around the insertion
                    // point into the fresh (pop + 1)-entry leaf.
                    unsafe {
                        let nk = new.as_ptr().cast::<u64>();
                        nk.copy_from_nonoverlapping(keys.as_ptr(), at);
                        nk.add(at).write(key);
                        nk.add(at + 1)
                            .copy_from_nonoverlapping(keys.as_ptr().add(at), pop - at);
                        let nv = new.as_ptr().add(leaf_values_offset(pop + 1)).cast::<u64>();
                        nv.copy_from_nonoverlapping(vals, at);
                        nv.add(at).write(val);
                        nv.add(at + 1)
                            .copy_from_nonoverlapping(vals.add(at), pop - at);
                        alloc.free_bytes_plain(ptr, leaf_size(pop));
                    }
                    self.root = Root::Leaf {
                        ptr: new,
                        pop: pop + 1,
                    };
                    None
                } else {
                    let top = promote_leaf::<false, false>(alloc, keys, vals, key, val, path);
                    // SAFETY: old root leaf no longer referenced.
                    unsafe { alloc.free_bytes_plain(ptr, leaf_size(pop)) };
                    self.tree_pop = pop as u64 + 1;
                    self.root = Root::Tree { top };
                    None
                }
            }
            Root::Tree { top } => {
                let prefix = key >> 8;
                if path.prefix == prefix {
                    alloc.assert_bracketed();
                    if let Some(mut leaf) = core::ptr::NonNull::new(path.leaf) {
                        let d = (key & 0xFF) as u8;
                        // SAFETY: path holds valid live LeafBitmapL pointer.
                        let node = unsafe { leaf.as_mut() };
                        let sub = (d >> 5) as usize;
                        if let Some(rank) = node.bitmap.test_and_subexpanse_rank(d) {
                            // SAFETY: value subarray holds subexpanse_count values; in-place swap.
                            unsafe {
                                let slot = node.values[sub].add(rank);
                                let old = *slot;
                                slot.write(val);
                                return Some(old);
                            }
                        }
                        let rank = node.bitmap.subexpanse_rank(d) as usize;
                        let old_n = node.bitmap.subexpanse_count(sub) as usize;
                        if old_n > 0
                            && crate::leaf::cap_class(old_n + 1) == crate::leaf::cap_class(old_n)
                        {
                            // Fast path: spare class capacity — shift in place.
                            // SAFETY: the subarray holds cap_class(old_n) slots.
                            unsafe {
                                let arr = node.values[sub];
                                core::ptr::copy(arr.add(rank), arr.add(rank + 1), old_n - rank);
                                arr.add(rank).write(val);
                            }
                        } else {
                            let new = alloc
                                .alloc_bytes_plain(crate::mutate::sub_vals_size(old_n + 1))
                                .cast::<u64>();
                            // SAFETY: copy old_n values around the inserted rank.
                            unsafe {
                                if old_n > 0 {
                                    let old = node.values[sub];
                                    new.as_ptr().copy_from_nonoverlapping(old, rank);
                                    new.as_ptr()
                                        .add(rank + 1)
                                        .copy_from_nonoverlapping(old.add(rank), old_n - rank);
                                    alloc.free_bytes_plain(
                                        core::ptr::NonNull::new(old.cast()).expect("values"),
                                        crate::mutate::sub_vals_size(old_n),
                                    );
                                }
                                new.as_ptr().add(rank).write(val);
                            }
                            node.values[sub] = new.as_ptr();
                        }
                        node.bitmap.set(d);
                        path.pending_pop += 1;
                        path.terminal_pop += 1;
                        self.tree_pop += 1;
                        debug_assert!(!path.edges[0].is_null());
                        // SAFETY: keep terminal edge pop0 up to date. The warm path is
                        // armed only where `edges[0]` is set beside `prefix`,
                        // `leaf`/`leaf1` and `depth` (mutate_map.rs:742, 821, 860, 922);
                        // `clear()` resets `prefix` to `u64::MAX`, which matches no
                        // `key >> 8`, so a cleared path never reaches here.
                        unsafe {
                            core::ptr::NonNull::new_unchecked(path.edges[0])
                                .as_mut()
                                .set_pop0(1, (path.terminal_pop - 1) as u64);
                        }
                        return None;
                    } else if let Some(leaf1) = core::ptr::NonNull::new(path.leaf1) {
                        let d = (key & 0xFF) as u8;
                        let cur_pop = path.terminal_pop as usize;
                        let base = leaf1.as_ptr();
                        // SAFETY: base points to a live Leaf1 allocation; map_keys_offset is in-bounds.
                        let keys_ptr = unsafe { base.add(crate::leaf::map_keys_offset(cur_pop)) };
                        // SAFETY: cur_pop >= 1 when leaf1 is active, so cur_pop - 1 is in bounds.
                        let last = unsafe { *keys_ptr.add(cur_pop - 1) };
                        if d > last {
                            if cur_pop < crate::mutate::LEAF1_CAP
                                && crate::leaf::cap_class(cur_pop + 1)
                                    == crate::leaf::cap_class(cur_pop)
                            {
                                // SAFETY: spare class capacity in the live Leaf1 allocation.
                                unsafe {
                                    *keys_ptr.add(cur_pop) = d;
                                    let vals = base.cast::<u64>();
                                    vals.add(cur_pop).write(val);
                                    debug_assert!(!path.edges[0].is_null());
                                    // SAFETY: the warm path is armed only where `edges[0]` is set beside
                                    // `prefix`, `leaf`/`leaf1` and `depth` (mutate_map.rs:742, 821, 860,
                                    // 922); `clear()` resets `prefix` to `u64::MAX`, which matches no
                                    // `key >> 8`, so a cleared path never reaches here.
                                    core::ptr::NonNull::new_unchecked(path.edges[0])
                                        .as_mut()
                                        .set_pop0(1, cur_pop as u64);
                                }
                                path.terminal_pop += 1;
                                path.pending_pop += 1;
                                self.tree_pop += 1;
                                return None;
                            }
                        } else if d == last {
                            // SAFETY: cur_pop - 1 is the existing slot for `last`.
                            unsafe {
                                let vals = base.cast::<u64>();
                                let slot = vals.add(cur_pop - 1);
                                let old = *slot;
                                slot.write(val);
                                return Some(old);
                            }
                        }
                    }
                }
                path.clear();
                tree_insert::<false, false, false>(alloc, &mut self.tree_pop, path, top, key, val).0
            }
        }
    }

    #[inline(always)]
    fn insert_inner_dispatch<const OCC: bool, const NESTED: bool>(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
        val: u64,
        path: &mut crate::mutate_map::InsertPathMap,
    ) -> Option<u64> {
        if let Root::Tree { top } = &mut self.root {
            path.clear();
            return tree_insert::<false, OCC, NESTED>(
                alloc,
                &mut self.tree_pop,
                path,
                top,
                key,
                val,
            )
            .0;
        }
        let o = leaf_state_insert::<OCC, NESTED>(self.root, alloc, key, val, path);
        if let Some(pop) = o.tree_pop {
            self.tree_pop = pop;
        }
        if let Some(root) = o.root {
            self.root = root;
        }
        o.old
    }

    /// Removes `key`; returns its value if it was present.
    #[inline(always)]
    pub(crate) fn remove(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
        path: &mut crate::mutate_map::InsertPathMap,
    ) -> Option<u64> {
        noting_root_rewrite!(self, m => m.remove_inner(alloc, key, path))
    }

    /// [`Self::remove`] on a tree whose readers run concurrently; as
    /// [`Self::insert_shared`].
    #[cfg(feature = "std")]
    #[inline(always)]
    pub(crate) fn remove_shared(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
        path: &mut crate::mutate_map::InsertPathMap,
    ) -> Option<u64> {
        noting_root_rewrite!(self, m => m.remove_inner_shared(alloc, key, path))
    }

    /// Single-threaded remove, bypassing OCC checks.
    #[inline(always)]
    pub(crate) fn remove_plain(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
        path: &mut crate::mutate_map::InsertPathMap,
    ) -> Option<u64> {
        noting_root_rewrite!(self, m => m.remove_inner_dispatch::<false, false>(alloc, key, path))
    }

    #[inline(always)]
    pub(crate) fn remove_dispatch<const OCC: bool, const NESTED: bool>(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
        path: &mut crate::mutate_map::InsertPathMap,
    ) -> Option<u64> {
        noting_root_rewrite!(self, m => m.remove_inner_dispatch::<OCC, NESTED>(alloc, key, path))
    }

    #[inline(always)]
    fn remove_inner(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
        path: &mut crate::mutate_map::InsertPathMap,
    ) -> Option<u64> {
        path.clear();
        match &mut self.root {
            Root::Empty => None,
            Root::Leaf { ptr, pop } => {
                let (ptr, pop) = (*ptr, *pop);
                let (keys, vals) = Self::leaf_parts(ptr, pop);
                let at = keys.binary_search(&key).ok()?;
                // SAFETY: in-bounds value read.
                let old = unsafe { *vals.add(at) };
                if pop == 1 {
                    // SAFETY: last entry removed; free the leaf.
                    unsafe { alloc.free_bytes(ptr, leaf_size(1)) };
                    self.root = Root::Empty;
                } else if crate::leaf::cap_class(pop - 1) == crate::leaf::cap_class(pop) {
                    // Fast path: capacity class unchanged — shift surviving entries in-place.
                    // SAFETY: in-place shift inside class-sized buffer.
                    unsafe {
                        let nk = ptr.as_ptr().cast::<u64>();
                        core::ptr::copy(nk.add(at + 1), nk.add(at), pop - 1 - at);
                        core::ptr::copy(vals.add(at + 1), vals.add(at), pop - 1 - at);
                    }
                    self.root = Root::Leaf { ptr, pop: pop - 1 };
                } else {
                    let new = alloc.alloc_bytes(leaf_size(pop - 1));
                    // SAFETY: copy the surviving keys/values into the
                    // smaller leaf.
                    unsafe {
                        let nk = new.as_ptr().cast::<u64>();
                        nk.copy_from_nonoverlapping(keys.as_ptr(), at);
                        nk.add(at)
                            .copy_from_nonoverlapping(keys.as_ptr().add(at + 1), pop - 1 - at);
                        let nv = new.as_ptr().add(leaf_values_offset(pop - 1)).cast::<u64>();
                        nv.copy_from_nonoverlapping(vals, at);
                        nv.add(at)
                            .copy_from_nonoverlapping(vals.add(at + 1), pop - 1 - at);
                        alloc.free_bytes(ptr, leaf_size(pop));
                    }
                    self.root = Root::Leaf {
                        ptr: new,
                        pop: pop - 1,
                    };
                }
                Some(old)
            }
            Root::Tree { top } => {
                // One OCC check per operation, where the runtime dispatch
                // always sat (see `insert_inner`).
                let (old, now) = by_mode!(alloc, tree_remove(alloc, &mut self.tree_pop, top, key));
                if old.is_some() {
                    if now == 0 {
                        debug_assert!(top.is_null());
                        // A root-state change: on a shared tree whose engine
                        // covers the root, the tree word brackets it (a no-op
                        // elsewhere; one load on this rare path).
                        crate::occ::tree_begin_if::<true>(alloc);
                        self.root = Root::Empty;
                        crate::occ::tree_end_if::<true>(alloc);
                    } else if now < ROOT_LEAF_CAP as u64 {
                        // Hysteresis twin of the root-leaf promotion; a
                        // root-state change, as above.
                        crate::occ::tree_begin_if::<true>(alloc);
                        by_mode!(alloc, 1 self.condense_to_root_leaf(alloc, path));
                        crate::occ::tree_end_if::<true>(alloc);
                    }
                }
                old
            }
        }
    }

    /// [`Self::remove_inner`] for a shared tree; as
    /// [`Self::insert_inner_shared`].
    #[cfg(feature = "std")]
    #[inline(always)]
    fn remove_inner_shared(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
        path: &mut crate::mutate_map::InsertPathMap,
    ) -> Option<u64> {
        path.clear();
        match &mut self.root {
            Root::Empty => None,
            Root::Leaf { ptr, pop } => {
                let (ptr, pop) = (*ptr, *pop);
                let (keys, vals) = Self::leaf_parts(ptr, pop);
                let at = keys.binary_search(&key).ok()?;
                // SAFETY: in-bounds value read.
                let old = unsafe { *vals.add(at) };
                if pop == 1 {
                    // SAFETY: last entry removed; free the leaf.
                    unsafe { alloc.free_bytes(ptr, leaf_size(1)) };
                    self.root = Root::Empty;
                } else if crate::leaf::cap_class(pop - 1) == crate::leaf::cap_class(pop) {
                    // Fast path: capacity class unchanged — shift surviving entries in-place.
                    // SAFETY: in-place shift inside class-sized buffer.
                    unsafe {
                        let nk = ptr.as_ptr().cast::<u64>();
                        shared_word::shift_down::<true>(nk, at, pop - 1 - at);
                        shared_word::shift_down::<true>(vals, at, pop - 1 - at);
                    }
                    self.root = Root::Leaf { ptr, pop: pop - 1 };
                } else {
                    let new = alloc.alloc_bytes(leaf_size(pop - 1));
                    // SAFETY: copy the surviving keys/values into the
                    // smaller leaf.
                    unsafe {
                        let nk = new.as_ptr().cast::<u64>();
                        nk.copy_from_nonoverlapping(keys.as_ptr(), at);
                        nk.add(at)
                            .copy_from_nonoverlapping(keys.as_ptr().add(at + 1), pop - 1 - at);
                        let nv = new.as_ptr().add(leaf_values_offset(pop - 1)).cast::<u64>();
                        nv.copy_from_nonoverlapping(vals, at);
                        nv.add(at)
                            .copy_from_nonoverlapping(vals.add(at + 1), pop - 1 - at);
                        alloc.free_bytes(ptr, leaf_size(pop));
                    }
                    self.root = Root::Leaf {
                        ptr: new,
                        pop: pop - 1,
                    };
                }
                Some(old)
            }
            Root::Tree { top } => {
                // One OCC check per operation, where the runtime dispatch
                // always sat (see `insert_inner`).
                let (old, now) = by_mode!(alloc, tree_remove(alloc, &mut self.tree_pop, top, key));
                if old.is_some() {
                    if now == 0 {
                        debug_assert!(top.is_null());
                        // A root-state change: on a shared tree whose engine
                        // covers the root, the tree word brackets it (a no-op
                        // elsewhere; one load on this rare path).
                        crate::occ::tree_begin_if::<true>(alloc);
                        self.root = Root::Empty;
                        crate::occ::tree_end_if::<true>(alloc);
                    } else if now < ROOT_LEAF_CAP as u64 {
                        // Hysteresis twin of the root-leaf promotion; a
                        // root-state change, as above.
                        crate::occ::tree_begin_if::<true>(alloc);
                        by_mode!(alloc, 1 self.condense_to_root_leaf(alloc, path));
                        crate::occ::tree_end_if::<true>(alloc);
                    }
                }
                old
            }
        }
    }

    #[inline(always)]
    fn remove_inner_dispatch<const OCC: bool, const NESTED: bool>(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
        path: &mut crate::mutate_map::InsertPathMap,
    ) -> Option<u64> {
        path.clear();
        match &mut self.root {
            Root::Empty | Root::Leaf { .. } => {
                let (old, root) = leaf_state_remove::<OCC>(self.root, alloc, key);
                if let Some(root) = root {
                    self.root = root;
                }
                old
            }
            Root::Tree { top } => {
                // One OCC check per operation, where the runtime dispatch
                // always sat (see `insert_inner`).
                let (old, now) = tree_remove::<OCC, NESTED>(alloc, &mut self.tree_pop, top, key);
                if old.is_some() {
                    if now == 0 {
                        debug_assert!(top.is_null());
                        // A root-state change: on a shared tree whose engine
                        // covers the root, the tree word brackets it (a no-op
                        // elsewhere; one load on this rare path).
                        crate::occ::tree_begin_if::<OCC>(alloc);
                        self.root = Root::Empty;
                        crate::occ::tree_end_if::<OCC>(alloc);
                    } else if now < ROOT_LEAF_CAP as u64 {
                        // Hysteresis twin of the root-leaf promotion; a
                        // root-state change, as above.
                        crate::occ::tree_begin_if::<OCC>(alloc);
                        self.condense_to_root_leaf::<OCC>(alloc, path);
                        crate::occ::tree_end_if::<OCC>(alloc);
                    }
                }
                old
            }
        }
    }

    /// Removes every entry (frees through `alloc`, which must be the
    /// allocator that produced this core's nodes).
    ///
    /// The caller's accounting invariant (`bytes_in_use == 0` after a
    /// lone map clears) lives with the allocator's owner — a shared
    /// allocator still carries its other cores' bytes here.
    #[inline(always)]
    pub(crate) fn clear<const OCC: bool>(
        &mut self,
        alloc: &NodeAlloc,
        path: &mut crate::mutate_map::InsertPathMap,
    ) {
        path.clear();
        match &mut self.root {
            Root::Empty => {}
            Root::Leaf { ptr, pop } => {
                // SAFETY: freeing the root leaf exactly once.
                unsafe { alloc.free_bytes_dispatch::<OCC>(*ptr, leaf_size(*pop)) };
            }
            Root::Tree { top, .. } => {
                // SAFETY: freeing the whole owned trie exactly once.
                unsafe { mutate::free_subtree::<OCC, true>(alloc, top) };
            }
        }
        self.root = Root::Empty;
    }

    /// Defensive trie structure validator that does not panic.
    ///
    /// The owner must flush its insert-path cache first (pending
    /// population deltas would otherwise disagree with the tree).
    ///
    /// Returns `Ok(())` if the trie invariants are fully met, or `Err(reason)`
    /// indicating what structural corruption was detected.
    pub(crate) fn validate_defensive(&self) -> Result<(), String> {
        match &self.root {
            Root::Empty => Ok(()),
            Root::Leaf { ptr, pop } => {
                if *pop < 1 || *pop > ROOT_LEAF_CAP {
                    return Err(format!("root leaf pop {pop} out of range"));
                }
                let (keys, _) = Self::leaf_parts(*ptr, *pop);
                if !keys.windows(2).all(|w| w[0] < w[1]) {
                    return Err("root leaf keys unsorted".into());
                }
                Ok(())
            }
            Root::Tree { top } => {
                if top.is_null() {
                    return Err("tree root with null top".into());
                }
                let mut stats = ExpanseStats::default();
                let counted =
                    crate::validate::expanse_validate_and_stats::<true>(top, 8, &mut stats, 0)?;
                let pop = self.tree_pop;
                if counted != pop {
                    return Err(format!(
                        "total population {pop} disagrees with tree {counted}"
                    ));
                }
                Ok(())
            }
        }
    }

    /// Gathers structural statistics of the trie (owner flushes first).
    #[must_use]
    pub(crate) fn stats(&self) -> ExpanseStats {
        let mut stats = ExpanseStats::default();
        match &self.root {
            Root::Empty => {}
            Root::Leaf { pop, .. } => {
                stats.depth_histogram[0] = 1;
                stats.leaf_pop_histogram[*pop] = 1;
                stats.node_counts.leaf_linear = 1;
                stats.leaf_depth_histogram[0] = 1;
                stats.node_bytes.leaf_linear =
                    crate::alloc::accounted_size(leaf_size(*pop), crate::types::RAW_ALIGN);
            }
            Root::Tree { top, .. } => {
                let _ = crate::validate::expanse_validate_and_stats::<true>(top, 8, &mut stats, 0);
            }
        }
        stats
    }
}

impl MapCore {
    fn leaf_entry(&self, at: usize) -> (u64, u64) {
        let Root::Leaf { ptr, pop } = &self.root else {
            unreachable!("leaf_entry outside root-leaf state")
        };
        let (keys, vals) = Self::leaf_parts(*ptr, *pop);
        // SAFETY: at < pop values live behind the keys.
        (keys[at], unsafe { *vals.add(at) })
    }

    /// Smallest entry in the map.
    #[inline(always)]
    #[must_use]
    pub(crate) fn first(&self) -> Option<(u64, u64)> {
        self.next_at_or_after(0)
    }

    /// Largest entry in the map.
    #[inline(always)]
    #[must_use]
    pub(crate) fn last(&self) -> Option<(u64, u64)> {
        self.prev_at_or_before(u64::MAX)
    }

    /// Smallest entry with key `>= key` (compat: `JudyLFirst`).
    #[inline(always)]
    #[must_use]
    pub(crate) fn next_at_or_after(&self, key: Key) -> Option<(u64, u64)> {
        match &self.root {
            Root::Empty => None,
            Root::Leaf { ptr, pop } => {
                let (keys, _) = Self::leaf_parts(*ptr, *pop);
                let at = keys.partition_point(|&k| k < key);
                (at < *pop).then(|| self.leaf_entry(at))
            }
            // SAFETY: trie maintained/owned by this map's engine.
            Root::Tree { top, .. } => unsafe { crate::nav::next::<true>(top, key, 8) },
        }
    }

    /// Smallest entry with key `> key` (compat: `JudyLNext`).
    #[inline(always)]
    #[must_use]
    pub(crate) fn next_after(&self, key: Key) -> Option<(u64, u64)> {
        self.next_at_or_after(key.checked_add(1)?)
    }

    /// Largest entry with key `<= key` (compat: `JudyLLast`).
    #[inline(always)]
    #[must_use]
    pub(crate) fn prev_at_or_before(&self, key: Key) -> Option<(u64, u64)> {
        match &self.root {
            Root::Empty => None,
            Root::Leaf { ptr, pop } => {
                let (keys, _) = Self::leaf_parts(*ptr, *pop);
                let at = keys.partition_point(|&k| k <= key).checked_sub(1)?;
                Some(self.leaf_entry(at))
            }
            // SAFETY: trie maintained/owned by this map's engine.
            Root::Tree { top, .. } => unsafe { crate::nav::prev::<true>(top, key, 8) },
        }
    }

    /// Largest entry with key `< key` (compat: `JudyLPrev`).
    #[inline(always)]
    #[must_use]
    pub(crate) fn prev_before(&self, key: Key) -> Option<(u64, u64)> {
        self.prev_at_or_before(key.checked_sub(1)?)
    }

    /// Number of keys strictly below `key` (rank; owner flushes first).
    #[inline(always)]
    #[must_use]
    pub(crate) fn count_below(&self, key: Key) -> u64 {
        match &self.root {
            Root::Empty => 0,
            Root::Leaf { ptr, pop } => {
                let (keys, _) = Self::leaf_parts(*ptr, *pop);
                keys.partition_point(|&k| k < key) as u64
            }
            // SAFETY: trie maintained/owned by this map's engine.
            Root::Tree { top, .. } => unsafe { crate::nav::count_below::<true>(top, key, 8) },
        }
    }

    /// Number of keys in the inclusive range (compat: `JudyLCount`;
    /// owner flushes first).
    #[inline(always)]
    #[must_use]
    pub(crate) fn count_range(&self, range: core::ops::RangeInclusive<u64>) -> u64 {
        let (a, b) = (*range.start(), *range.end());
        if a > b {
            return 0;
        }
        self.count_below(b) + u64::from(self.contains_key(b)) - self.count_below(a)
    }

    /// The entry with `n` keys below it — 0-based select (compat:
    /// `JudyLByCount`, which is 1-based; owner flushes first).
    #[inline(always)]
    #[must_use]
    pub(crate) fn by_count(&self, n: u64) -> Option<(u64, u64)> {
        if n >= self.len() {
            return None;
        }
        match &self.root {
            Root::Empty => None,
            Root::Leaf { .. } => Some(self.leaf_entry(n as usize)),
            // SAFETY: trie maintained/owned by this map's engine; n is
            // below the population.
            Root::Tree { top, .. } => Some(unsafe { crate::nav::by_count::<true>(top, n, 8) }),
        }
    }

    /// Builds a forward (ascending) raw cursor over all entries.
    fn iter_fwd_raw(&self) -> crate::iter::RawIter<true> {
        match &self.root {
            Root::Empty => crate::iter::RawIter::new(),
            Root::Leaf { ptr, pop } => {
                let keys_ptr = ptr.as_ptr().cast::<u64>();
                // SAFETY: root leaf holds pop values starting at leaf_values_offset(pop).
                let vals_ptr = unsafe { keys_ptr.add(leaf_values_offset(*pop) / 8) };
                crate::iter::RawIter::from_root_leaf(keys_ptr, vals_ptr, *pop)
            }
            // SAFETY: tree maintained by map engine per invariants.
            Root::Tree { top, .. } => unsafe { crate::iter::RawIter::from_tree(top) },
        }
    }

    /// Builds a forward (ascending) raw cursor over entries with key `>= start`.
    fn range_fwd_raw(&self, start: Key) -> crate::iter::RawIter<true> {
        match &self.root {
            Root::Empty => crate::iter::RawIter::new(),
            Root::Leaf { ptr, pop } => {
                let (keys, values) = Self::leaf_parts(*ptr, *pop);
                // SAFETY: root leaf contains valid keys and values arrays of length pop.
                unsafe {
                    crate::iter::RawIter::from_root_leaf_range(keys.as_ptr(), values, *pop, start)
                }
            }
            // SAFETY: tree maintained by map engine per invariants.
            Root::Tree { top, .. } => unsafe { crate::iter::RawIter::from_tree_range(top, start) },
        }
    }

    /// The trie root edge for cursor seeks, or `Edge::NULL` when the root is a
    /// flat leaf / empty (cursor seeks then stay leaf-local).
    #[inline]
    fn cursor_top(&self) -> Edge {
        match &self.root {
            Root::Tree { top, .. } => *top,
            _ => Edge::NULL,
        }
    }

    /// Creates a stateful forward [`MapCursor`](crate::cursor::MapCursor) for
    /// monotone skip-scans, positioned before the first entry.
    ///
    /// Unlike the stateless [`next_at_or_after`](Self::next_at_or_after), which
    /// re-descends from the root on every call, the cursor keeps its descent
    /// path and re-descends only from the deepest ancestor whose expanse still
    /// covers the next target (issue #340; docs/ALGORITHMS.md §3.5).
    #[inline(always)]
    #[must_use]
    pub(crate) fn cursor(&self) -> crate::cursor::MapCursor<'_> {
        crate::cursor::MapCursor::new(self.iter_fwd_raw(), self.cursor_top())
    }

    /// Creates a [`MapCursor`](crate::cursor::MapCursor) positioned at the
    /// smallest key `>= start`.
    #[inline(always)]
    #[must_use]
    pub(crate) fn cursor_from(&self, start: Key) -> crate::cursor::MapCursor<'_> {
        crate::cursor::MapCursor::new(self.range_fwd_raw(start), self.cursor_top())
    }

    /// Re-seeds `cursor` in place at the smallest key `>= start` — the state
    /// [`cursor_from`](Self::cursor_from) builds, without constructing and
    /// moving a cursor (#1096). It works on the engine cursor rather than
    /// `MapCursor`, whose `PhantomData<&ExpanseMap>` would make a long-lived
    /// owner such as `StrCursor` `!RefUnwindSafe`, a change to its public API.
    #[inline(always)]
    pub(crate) fn reset_raw_cursor(&self, cursor: &mut crate::cursor::RawCursor<true>, start: Key) {
        let top = self.cursor_top();
        match &self.root {
            Root::Empty => cursor.reset(top, |raw| raw.reset_empty()),
            Root::Leaf { ptr, pop } => {
                let (keys, values) = Self::leaf_parts(*ptr, *pop);
                // SAFETY: root leaf contains valid keys and values arrays of
                // length pop, as for `range_fwd_raw`.
                cursor.reset(top, |raw| unsafe {
                    raw.reset_root_leaf_range(keys.as_ptr(), values, *pop, start);
                });
            }
            // SAFETY: tree maintained by map engine per invariants, as for
            // `range_fwd_raw`.
            Root::Tree { top: edge, .. } => cursor.reset(top, |raw| unsafe {
                raw.reset_tree_range(edge, start);
            }),
        }
    }

    /// Ascending iterator over `(key, value)` entries.
    #[inline(always)]
    #[must_use]
    pub(crate) fn iter(&self) -> MapIter<'_> {
        MapIter {
            _map: core::marker::PhantomData,
            raw: self.iter_fwd_raw(),
        }
    }

    /// Returns an iterator over entries in the inclusive range `[start, end]`.
    #[inline(always)]
    #[must_use]
    pub(crate) fn range(&self, range: core::ops::RangeInclusive<Key>) -> MapRange<'_> {
        let (start, end) = (*range.start(), *range.end());
        let raw = if start > end {
            crate::iter::RawIter::new()
        } else {
            self.range_fwd_raw(start)
        };
        MapRange {
            _map: core::marker::PhantomData,
            raw,
            end,
        }
    }

    /// Builds a reverse (descending) raw cursor over all entries.
    fn iter_rev_raw(&self) -> crate::iter::RawIter<true> {
        match &self.root {
            Root::Empty => crate::iter::RawIter::new(),
            Root::Leaf { ptr, pop } => {
                let keys_ptr = ptr.as_ptr().cast::<u64>();
                // SAFETY: root leaf holds pop values starting at leaf_values_offset(pop).
                let vals_ptr = unsafe { keys_ptr.add(leaf_values_offset(*pop) / 8) };
                crate::iter::RawIter::from_root_leaf_rev(keys_ptr, vals_ptr, *pop)
            }
            // SAFETY: tree maintained by map engine per invariants.
            Root::Tree { top, .. } => unsafe { crate::iter::RawIter::from_tree_rev(top) },
        }
    }

    /// Builds a reverse (descending) raw cursor over entries with key `<= end`.
    fn range_rev_raw(&self, end: Key) -> crate::iter::RawIter<true> {
        match &self.root {
            Root::Empty => crate::iter::RawIter::new(),
            Root::Leaf { ptr, pop } => {
                let (keys, values) = Self::leaf_parts(*ptr, *pop);
                // SAFETY: root leaf contains valid keys and values arrays of length pop.
                unsafe {
                    crate::iter::RawIter::from_root_leaf_range_rev(keys.as_ptr(), values, *pop, end)
                }
            }
            // SAFETY: tree maintained by map engine per invariants.
            Root::Tree { top, .. } => unsafe {
                crate::iter::RawIter::from_tree_range_rev(top, end)
            },
        }
    }

    /// Descending (double-ended) iterator over `(key, value)` entries.
    #[inline(always)]
    #[must_use]
    pub(crate) fn iter_rev(&self) -> MapIterRev<'_> {
        MapIterRev {
            map: self,
            raw: self.iter_rev_raw(),
            front: None,
            lo: 0,
            hi: Key::MAX,
            done: self.is_empty(),
        }
    }

    /// Returns a descending (double-ended) iterator over entries in the
    /// inclusive range `[start, end]`.
    #[inline(always)]
    #[must_use]
    pub(crate) fn range_rev(&self, range: core::ops::RangeInclusive<Key>) -> MapRangeRev<'_> {
        let (start, end) = (*range.start(), *range.end());
        let (raw, done) = if start > end {
            (crate::iter::RawIter::new(), true)
        } else {
            (self.range_rev_raw(end), false)
        };
        MapRangeRev {
            map: self,
            raw,
            front: None,
            lo: start,
            hi: end,
            done,
        }
    }

    /// Returns an iterator over entries in `range` where the hot metadata word
    /// (bits 63:40 of the 64-bit value slot) satisfies the predicate.
    #[inline(always)]
    pub(crate) fn range_filtered<'a, P>(
        &'a self,
        range: core::ops::RangeInclusive<Key>,
        mut predicate: P,
    ) -> impl Iterator<Item = (Key, u64)> + 'a
    where
        P: FnMut(Key, u32) -> bool + 'a,
    {
        self.range(range).filter(move |&(k, v)| {
            let meta = ((v >> 40) & crate::slot::ValueSlot::ARENA_META_MASK) as u32;
            predicate(k, meta)
        })
    }

    /// Scans entries in `range`, evaluating `predicate(key, hot_meta)` directly on
    /// the raw value slot before invoking `callback(key, raw_val)`.
    #[inline(always)]
    pub(crate) fn scan_filtered<P, F>(
        &self,
        range: core::ops::RangeInclusive<Key>,
        mut predicate: P,
        mut callback: F,
    ) where
        P: FnMut(Key, u32) -> bool,
        F: FnMut(Key, u64) -> bool,
    {
        for (k, v) in self.range(range) {
            let meta = ((v >> 40) & crate::slot::ValueSlot::ARENA_META_MASK) as u32;
            if predicate(k, meta) && !callback(k, v) {
                break;
            }
        }
    }
}

/// Ascending entry iterator over an [`ExpanseMap`].
///
/// Forward-only by design: the deterministic-Callgrind zero-regression gate
/// (`AGENTS.md` §5) forbids adding any per-element work to this hot path, so
/// reverse iteration lives on the dedicated [`MapIterRev`] / [`MapRangeRev`]
/// types (`iter_rev` / `range_rev`), which are themselves double-ended.
pub struct MapIter<'a> {
    _map: core::marker::PhantomData<&'a MapCore>,
    raw: crate::iter::RawIter<true>,
}

impl Iterator for MapIter<'_> {
    type Item = (u64, u64);

    #[inline(always)]
    fn next(&mut self) -> Option<(u64, u64)> {
        self.raw.next()
    }
}

/// Ascending entry iterator over a key range in an [`ExpanseMap`].
pub struct MapRange<'a> {
    _map: core::marker::PhantomData<&'a MapCore>,
    raw: crate::iter::RawIter<true>,
    end: Key,
}

impl Iterator for MapRange<'_> {
    type Item = (Key, u64);

    #[inline(always)]
    fn next(&mut self) -> Option<(Key, u64)> {
        let (k, v) = self.raw.next()?;
        if k > self.end {
            return None;
        }
        Some((k, v))
    }
}

/// Double-ended entry iterator over an [`ExpanseMap`], descending by default.
///
/// `next` streams descending from the largest key; `next_back` streams
/// ascending from the smallest. The two ends share an inclusive `[lo, hi]`
/// window so interleaved calls never cross: `next` lowers `hi`, `next_back`
/// raises `lo`, and each stops once the window closes. The ascending cursor is
/// built lazily, so a pure-descending walk never pays for it.
pub struct MapIterRev<'a> {
    map: &'a MapCore,
    raw: crate::iter::RawIter<true>,
    front: Option<crate::iter::RawIter<true>>,
    lo: Key,
    hi: Key,
    done: bool,
}

impl Iterator for MapIterRev<'_> {
    type Item = (Key, u64);

    #[inline]
    fn next(&mut self) -> Option<(Key, u64)> {
        if self.done {
            return None;
        }
        let (k, v) = self.raw.next_back()?;
        if k < self.lo {
            self.done = true;
            return None;
        }
        if k == self.lo {
            self.done = true;
        } else {
            self.hi = k - 1;
        }
        Some((k, v))
    }
}

impl DoubleEndedIterator for MapIterRev<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<(Key, u64)> {
        if self.done {
            return None;
        }
        if self.front.is_none() {
            self.front = Some(self.map.iter_fwd_raw());
        }
        let (k, v) = self.front.as_mut().unwrap().next()?;
        if k > self.hi {
            self.done = true;
            return None;
        }
        if k == self.hi {
            self.done = true;
        } else {
            self.lo = k + 1;
        }
        Some((k, v))
    }
}

/// Double-ended entry iterator over a key range in an [`ExpanseMap`],
/// descending by default. See [`MapIterRev`] for the shared-window discipline.
pub struct MapRangeRev<'a> {
    map: &'a MapCore,
    raw: crate::iter::RawIter<true>,
    front: Option<crate::iter::RawIter<true>>,
    lo: Key,
    hi: Key,
    done: bool,
}

impl Iterator for MapRangeRev<'_> {
    type Item = (Key, u64);

    #[inline]
    fn next(&mut self) -> Option<(Key, u64)> {
        if self.done {
            return None;
        }
        let (k, v) = self.raw.next_back()?;
        if k < self.lo {
            self.done = true;
            return None;
        }
        if k == self.lo {
            self.done = true;
        } else {
            self.hi = k - 1;
        }
        Some((k, v))
    }
}

impl DoubleEndedIterator for MapRangeRev<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<(Key, u64)> {
        if self.done {
            return None;
        }
        if self.front.is_none() {
            // At first `next_back`, `lo` still equals the original range start.
            self.front = Some(self.map.range_fwd_raw(self.lo));
        }
        let (k, v) = self.front.as_mut().unwrap().next()?;
        if k > self.hi {
            self.done = true;
            return None;
        }
        if k == self.hi {
            self.done = true;
        } else {
            self.lo = k + 1;
        }
        Some((k, v))
    }
}

impl<'a> IntoIterator for &'a ExpanseMap {
    type Item = (u64, u64);
    type IntoIter = MapIter<'a>;

    fn into_iter(self) -> MapIter<'a> {
        self.iter()
    }
}

/// A deep copy: a new map holding the same entries and sharing no node with
/// this one, so later writes to either are invisible to the other. This is
/// the supported way to keep a point-in-time snapshot of a map.
///
/// Built by ordered iteration into a fresh map, so it costs O(n) time and one
/// full tree of memory, and it runs on the sequential-run insert bypass. The
/// copy's node census depends only on the key set (`mem_used` is
/// order-invariant, `tests/test_mem_used_order_invariant.rs`), so it equals
/// that of a map built by inserting the same keys; the copy takes nothing from
/// the original's freelists.
///
/// Copying a root edge out of a map's internals is *not* a snapshot: the
/// engine mutates nodes in place on every insert and remove, including the
/// population counts in every ancestor edge (`docs/ARCHITECTURE.md`,
/// "Snapshots"). Structurally shared snapshots are tracked in
/// [#1103](https://github.com/orieg/expanse/issues/1103).
impl Clone for ExpanseMap {
    fn clone(&self) -> Self {
        self.iter().collect()
    }
}

impl FromIterator<(Key, u64)> for ExpanseMap {
    fn from_iter<I: IntoIterator<Item = (Key, u64)>>(iter: I) -> Self {
        let mut map = Self::new();
        map.extend(iter);
        map
    }
}

impl Extend<(Key, u64)> for ExpanseMap {
    fn extend<I: IntoIterator<Item = (Key, u64)>>(&mut self, iter: I) {
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}

impl MapCore {
    /// Rebuilds the flat root leaf (parallel key/value arrays) from a
    /// small tree — the shrink twin of the promotion.
    #[inline(always)]
    fn condense_to_root_leaf<const OCC: bool>(
        &mut self,
        alloc: &NodeAlloc,
        path: &mut crate::mutate_map::InsertPathMap,
    ) {
        let Root::Tree { top } = &mut self.root else {
            unreachable!("condense outside tree state")
        };
        let n = self.tree_pop as usize;
        debug_assert!((1..ROOT_LEAF_CAP).contains(&n));
        let new = alloc.alloc_bytes_dispatch::<OCC>(leaf_size(n));
        let mut written = 0usize;
        let mut from = Some(0u64);
        // SAFETY: engine-maintained trie per this type's invariants.
        while let Some((k, v)) = from.and_then(|f| unsafe { crate::nav::next::<true>(top, f, 8) }) {
            debug_assert!(written < n);
            // SAFETY: in-bounds writes: keys then values.
            unsafe {
                new.as_ptr().cast::<u64>().add(written).write(k);
                new.as_ptr()
                    .add(leaf_values_offset(n))
                    .cast::<u64>()
                    .add(written)
                    .write(v);
            }
            written += 1;
            from = k.checked_add(1);
        }
        debug_assert_eq!(written, n);
        path.clear();
        // SAFETY: whole trie owned by this map; freed exactly once.
        unsafe { mutate::free_subtree::<OCC, true>(alloc, top) };
        self.root = Root::Leaf { ptr: new, pop: n };
    }
}

/// Pathless twins for owners without a persistent insert-path cache
/// (`ExpanseStrMap`'s sub-tries, issue #363 Step A). Each hands the
/// engine a fresh empty path: the sequential-bypass check can never hit
/// (`prefix == u64::MAX` matches no `key >> 8`), so after inlining the
/// bypass bookkeeping is dead and folds away.
impl MapCore {
    #[inline(always)]
    pub(crate) fn insert_pathless_dispatch<const OCC: bool, const NESTED: bool>(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
        val: u64,
    ) -> Option<u64> {
        self.insert_dispatch::<OCC, NESTED>(
            alloc,
            key,
            val,
            &mut crate::mutate_map::InsertPathMap::empty(),
        )
    }

    #[inline(always)]
    #[allow(dead_code)]
    pub(crate) fn insert_pathless(&mut self, alloc: &NodeAlloc, key: Key, val: u64) -> Option<u64> {
        by_mode!(alloc, self.insert_pathless_dispatch(alloc, key, val))
    }

    #[inline(always)]
    pub(crate) fn remove_pathless_dispatch<const OCC: bool, const NESTED: bool>(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
    ) -> Option<u64> {
        self.remove_dispatch::<OCC, NESTED>(
            alloc,
            key,
            &mut crate::mutate_map::InsertPathMap::empty(),
        )
    }

    #[inline(always)]
    #[allow(dead_code)]
    pub(crate) fn remove_pathless(&mut self, alloc: &NodeAlloc, key: Key) -> Option<u64> {
        by_mode!(alloc, self.remove_pathless_dispatch(alloc, key))
    }

    #[inline(always)]
    pub(crate) fn ins_slot_pathless_dispatch<const OCC: bool, const NESTED: bool>(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
    ) -> core::ptr::NonNull<u64> {
        self.ins_slot_dispatch::<OCC, NESTED>(
            alloc,
            key,
            &mut crate::mutate_map::InsertPathMap::empty(),
        )
    }

    #[inline(always)]
    #[allow(dead_code)]
    pub(crate) fn ins_slot_pathless(
        &mut self,
        alloc: &NodeAlloc,
        key: Key,
    ) -> core::ptr::NonNull<u64> {
        by_mode!(alloc, self.ins_slot_pathless_dispatch(alloc, key))
    }

    #[inline(always)]
    pub(crate) fn value_slot_pathless(&mut self, key: Key) -> Option<core::ptr::NonNull<u64>> {
        self.get_value_slot(key, &mut crate::mutate_map::InsertPathMap::empty())
    }

    #[inline(always)]
    pub(crate) fn clear_pathless_dispatch<const OCC: bool>(&mut self, alloc: &NodeAlloc) {
        self.clear::<OCC>(alloc, &mut crate::mutate_map::InsertPathMap::empty());
    }

    #[inline(always)]
    #[allow(dead_code)]
    pub(crate) fn clear_pathless(&mut self, alloc: &NodeAlloc) {
        by_mode!(alloc, 1 self.clear_pathless_dispatch(alloc));
    }
}

/// The public surface: forwards to `MapCore` with this map's own
/// allocator and insert-path cache. The core's hot entry points are
/// `#[inline(always)]`, so each forwarder compiles to exactly the
/// pre-split method body (issue #363 Step A's zero-regression
/// requirement on the single-threaded `JudyL*` paths).
impl ExpanseMap {
    /// Creates an empty map.
    #[must_use]
    pub fn new() -> Self {
        Self {
            core: MapCore::new(),
            alloc: NodeAlloc::new(),
            path: core::cell::UnsafeCell::new(crate::mutate_map::InsertPathMap::empty()),
        }
    }

    #[inline(always)]
    fn flush_path(&self) {
        // SAFETY: path is an internal cursor whose state is flushed through UnsafeCell.
        unsafe {
            (*self.path.get()).flush();
        }
    }

    #[inline(always)]
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) fn clear_path(&self) {
        // SAFETY: path is an internal cursor whose state is reset through UnsafeCell.
        unsafe {
            (*self.path.get()).clear();
        }
    }

    /// Number of keys in the map.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.core.len()
    }

    /// True when no keys are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.core.is_empty()
    }

    /// The allocator, reached from a raw pointer to the map without forming
    /// a reference to the whole map (#1086). A wrapper's optimistic writers
    /// take it while other optimistic writers run; a `&` to the whole map
    /// would also cover the root state, which the covered writer owns.
    ///
    /// # Safety
    ///
    /// `this` points to a live map for `'a`, and no `&mut` to it exists
    /// meanwhile (the wrapper's optimistic writers are quiesced before any
    /// covered writer takes one).
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    #[inline(always)]
    pub(crate) unsafe fn alloc_of<'a>(this: *const Self) -> &'a NodeAlloc {
        // SAFETY: caller contract; the place is projected through the raw
        // pointer, so no reference to the whole map is formed.
        unsafe { &*core::ptr::addr_of!((*this).alloc) }
    }

    #[inline(always)]
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) fn alloc(&self) -> &NodeAlloc {
        &self.alloc
    }

    /// Heap bytes currently used by the map's nodes and leaves.
    #[must_use]
    pub fn mem_used(&self) -> usize {
        self.alloc.bytes_in_use()
    }

    /// Heap bytes the map holds from the system allocator: [`Self::mem_used`]
    /// plus the freed blocks its allocator keeps on per-tree freelists for
    /// reuse and the unused part of the slab pages small nodes are carved
    /// from. Those blocks go back to the system only when the map is
    /// dropped or cleared, or on [`Self::shrink_to_fit`], so `malloc_trim`
    /// cannot recover them before then; this is the figure to
    /// compare with resident memory. The system allocator's own
    /// per-allocation overhead (chunk headers, size-class rounding) is
    /// allocator-specific and not included; the `allocator_overhead`
    /// example measures it.
    ///
    /// Computed on demand by walking the allocator's slab pages and
    /// freelists (O(pages + free blocks)); no counter is kept on the
    /// allocation path.
    #[must_use]
    pub fn mem_held(&self) -> usize {
        self.alloc.bytes_held()
    }

    /// Returns memory the map holds but does not use to the system
    /// allocator: freed blocks of the larger size classes, and slab pages
    /// with no live node on them. Returns the bytes released; afterwards
    /// [`Self::mem_held`] is lower by exactly that much and
    /// [`Self::mem_used`] is unchanged. Nothing moves, so no key, value or
    /// value pointer is affected.
    ///
    /// The map keeps freed blocks for reuse, so this pays off after a
    /// build or a burst of removals that leaves many blocks idle; it costs a
    /// walk of the allocator's pages and freelists. A no-op on a map
    /// shared through a concurrent wrapper, whose freed blocks go to its
    /// epoch collector: [`crate::sync::SyncExpanseMap::shrink_to_fit`] returns those.
    pub fn shrink_to_fit(&mut self) -> usize {
        self.alloc.release_free()
    }

    /// Cumulative node/leaf allocations made by this container since it
    /// was created (diagnostics; see `tests/no_heap_churn.rs`, which
    /// subtracts these from the process-wide count to isolate
    /// incidental scratch allocation).
    #[must_use]
    pub fn total_node_allocs(&self) -> usize {
        self.alloc.total_allocs()
    }

    /// Returns the value stored for `key`.
    #[inline(always)]
    #[must_use]
    pub fn get(&self, key: Key) -> Option<u64> {
        self.core.get(key)
    }

    /// See `MapCore::root_is_tree`.
    #[inline(always)]
    #[cfg(feature = "std")]
    pub(crate) fn root_is_tree(&self) -> bool {
        self.core.root_is_tree()
    }

    #[inline(always)]
    #[cfg(feature = "std")]
    pub(crate) unsafe fn root_top_ptr(&self) -> *mut Edge {
        // SAFETY: forwarded contract from MapCore::root_top_ptr.
        unsafe { self.core.root_top_ptr() }
    }

    /// Look up a batch of `keys` simultaneously, writing values into `out`.
    ///
    /// Results are identical to calling [`ExpanseMap::get`] on each key. When
    /// the root is a multi-level digital trie the descents are advanced one
    /// level at a time, [`get::BATCH_WIDTH`] of them interleaved, so their
    /// dependent misses can be outstanding together instead of serialising —
    /// see the batched-descent notes on `get`. Nothing in that path is shared
    /// with the single-key walk.
    #[inline]
    pub fn get_batch(&self, keys: &[Key], out: &mut [Option<u64>]) {
        self.core.get_batch(keys, out);
    }

    /// [`ExpanseMap::get_batch`] at an explicit interleave width.
    ///
    /// Which width overlaps the most misses without exceeding the core's
    /// outstanding-miss budget is a wall-clock question, so this entry exists
    /// for `benches/batch_lookup.rs` to sweep `W` against one build. Not a
    /// stable API: use [`ExpanseMap::get_batch`].
    #[doc(hidden)]
    #[inline]
    pub fn get_batch_width<const W: usize>(&self, keys: &[Key], out: &mut [Option<u64>]) {
        self.core.get_batch_width::<W>(keys, out);
    }

    /// Look up a batch of `keys`, writing found values into `out_values` and presence flags
    /// into `out_found` (when `Some`). Returns the count of found keys.
    #[inline]
    pub fn get_batch_into(
        &self,
        keys: &[Key],
        out_values: &mut [u64],
        out_found: Option<&mut [bool]>,
    ) -> usize {
        self.core.get_batch_into(keys, out_values, out_found)
    }

    /// Returns a pointer to `key`'s value slot in the leaf or root leaf, or `None`
    /// if absent.
    #[inline(always)]
    #[must_use]
    pub fn get_slot_ptr(&self, key: Key) -> Option<core::ptr::NonNull<u64>> {
        self.core.get_slot_ptr(key)
    }

    /// Returns a **writable pointer to `key`'s value slot**, or `None` if
    /// the key is absent — the compat layer's `JudyLGet`/`JudyLIns` return
    /// convention. The pointer stays valid until the next structural
    /// mutation of the map (the classic JudyL contract); reading or
    /// writing through it after an `insert`/`remove`/`clear` is undefined.
    #[inline(always)]
    #[must_use]
    pub fn get_value_slot(&mut self, key: Key) -> Option<core::ptr::NonNull<u64>> {
        self.core.get_value_slot(key, self.path.get_mut())
    }

    /// Inserts `key` with value 0 if absent — the existing value is kept
    /// untouched — and returns a **writable pointer to its value slot**:
    /// the compat `JudyLIns` contract, in one tree walk. The pointer stays
    /// valid until the next structural mutation.
    #[inline(always)]
    pub fn ins_slot(&mut self, key: Key) -> core::ptr::NonNull<u64> {
        self.core.ins_slot(&self.alloc, key, self.path.get_mut())
    }

    /// [`Self::ins_slot`] for the concurrent wrappers, whose readers load
    /// the root leaf while this stores to it (`MapCore::ins_slot_shared`).
    #[cfg(feature = "std")]
    #[inline(always)]
    pub(crate) fn ins_slot_shared(&mut self, key: Key) -> core::ptr::NonNull<u64> {
        self.core
            .ins_slot_shared(&self.alloc, key, self.path.get_mut())
    }

    /// Single-threaded insert-if-absent returning slot pointer, bypassing OCC checks.
    #[doc(hidden)]
    #[inline(always)]
    pub fn ins_slot_plain(&mut self, key: Key) -> core::ptr::NonNull<u64> {
        self.core
            .ins_slot_plain(&self.alloc, key, self.path.get_mut())
    }

    /// Phase 7 (occ): by-value root snapshot + allocation handle for
    /// the validated concurrent read walk (see `ExpanseSet::occ_root`).
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) fn occ_root(&self) -> (RootSnapshot, &NodeAlloc) {
        (self.core.occ_snapshot(), &self.alloc)
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn path_mut(&mut self) -> &mut crate::mutate_map::InsertPathMap {
        self.path.get_mut()
    }

    /// Membership test.
    #[must_use]
    pub fn contains_key(&self, key: Key) -> bool {
        self.core.contains_key(key)
    }

    /// Inserts `key → val`; returns the replaced value if the key was
    /// already present.
    #[inline(always)]
    pub fn insert(&mut self, key: Key, val: u64) -> Option<u64> {
        self.core.insert(&self.alloc, key, val, self.path.get_mut())
    }

    /// [`Self::insert`] for the concurrent wrapper, whose readers load the
    /// root leaf while this stores to it (`MapCore::insert_shared`).
    #[cfg(feature = "std")]
    #[inline(always)]
    pub(crate) fn insert_shared(&mut self, key: Key, val: u64) -> Option<u64> {
        self.core
            .insert_shared(&self.alloc, key, val, self.path.get_mut())
    }

    /// Single-threaded insert, bypassing OCC checks.
    #[doc(hidden)]
    #[inline(always)]
    pub fn insert_plain(&mut self, key: Key, val: u64) -> Option<u64> {
        self.core
            .insert_plain(&self.alloc, key, val, self.path.get_mut())
    }

    /// Removes `key`; returns its value if it was present.
    ///
    /// The blocks a removal frees stay with the map for reuse, including
    /// after the last key is removed; call [`Self::shrink_to_fit`] (or
    /// [`Self::clear`]) to return them to the system allocator after a
    /// drain.
    #[inline(always)]
    pub fn remove(&mut self, key: Key) -> Option<u64> {
        self.core.remove(&self.alloc, key, self.path.get_mut())
    }

    /// [`Self::remove`] for the concurrent wrapper; as [`Self::insert_shared`].
    #[cfg(feature = "std")]
    #[inline(always)]
    pub(crate) fn remove_shared(&mut self, key: Key) -> Option<u64> {
        self.core
            .remove_shared(&self.alloc, key, self.path.get_mut())
    }

    /// Single-threaded remove, bypassing OCC checks.
    #[doc(hidden)]
    #[inline(always)]
    pub fn remove_plain(&mut self, key: Key) -> Option<u64> {
        self.core
            .remove_plain(&self.alloc, key, self.path.get_mut())
    }

    /// Removes every entry, and returns the blocks the map's allocator
    /// kept for reuse to the system allocator, so the map holds what a new
    /// one does ([`Self::mem_held`] is 0). Unlike `Vec::clear` and
    /// `HashMap::clear`, it keeps no capacity: a map refilled after `clear`
    /// starts cold. Not on a map shared through a concurrent wrapper, whose
    /// blocks belong to its collector.
    pub fn clear(&mut self) {
        self.clear_entries();
        self.alloc.release_free();
    }

    /// [`Self::clear`] without the release: frees every entry and leaves the
    /// allocator's freed blocks in place. For `Drop` and for owners about to
    /// drop the map, where a release would only walk the blocks the
    /// allocator's own `Drop` frees.
    pub(crate) fn clear_entries(&mut self) {
        by_mode!(
            self.alloc,
            1 self.core.clear(&self.alloc, self.path.get_mut())
        );
        debug_assert_eq!(self.alloc.bytes_in_use(), 0);
    }

    /// Single-threaded clear, bypassing OCC checks. Unlike [`Self::clear`]
    /// it keeps the allocator's freed blocks: the C ABI's `*FreeArray`
    /// drops the tree straight after, which returns them anyway.
    #[doc(hidden)]
    #[inline(always)]
    pub fn clear_plain(&mut self) {
        self.core.clear::<false>(&self.alloc, self.path.get_mut());
        debug_assert_eq!(self.alloc.bytes_in_use(), 0);
    }

    #[inline(always)]
    #[cfg(all(target_pointer_width = "64", feature = "std"))]
    pub(crate) fn set_tree_pop(&mut self, pop: u64) {
        self.core.tree_pop = pop;
    }

    /// Walks the whole structure, panicking on any violated invariant
    /// (`docs/TESTING.md`, "Structural invariant validator").
    pub fn validate(&self) {
        if let Err(err) = self.validate_defensive() {
            panic!("{err}");
        }
    }

    /// Defensive trie structure validator that does not panic.
    ///
    /// Returns `Ok(())` if the trie invariants are fully met, or `Err(reason)`
    /// indicating what structural corruption was detected.
    pub fn validate_defensive(&self) -> Result<(), String> {
        self.flush_path();
        self.core.validate_defensive()
    }

    /// Gathers structural statistics of the trie.
    #[must_use]
    pub fn stats(&self) -> ExpanseStats {
        self.flush_path();
        self.core.stats()
    }

    /// Smallest entry in the map.
    #[must_use]
    pub fn first(&self) -> Option<(u64, u64)> {
        self.core.first()
    }

    /// Largest entry in the map.
    #[must_use]
    pub fn last(&self) -> Option<(u64, u64)> {
        self.core.last()
    }

    /// Smallest entry with key `>= key` (compat: `JudyLFirst`).
    #[must_use]
    pub fn next_at_or_after(&self, key: Key) -> Option<(u64, u64)> {
        self.core.next_at_or_after(key)
    }

    /// Smallest entry with key `> key` (compat: `JudyLNext`).
    #[must_use]
    pub fn next_after(&self, key: Key) -> Option<(u64, u64)> {
        self.core.next_after(key)
    }

    /// Largest entry with key `<= key` (compat: `JudyLLast`).
    #[must_use]
    pub fn prev_at_or_before(&self, key: Key) -> Option<(u64, u64)> {
        self.core.prev_at_or_before(key)
    }

    /// Largest entry with key `< key` (compat: `JudyLPrev`).
    #[must_use]
    pub fn prev_before(&self, key: Key) -> Option<(u64, u64)> {
        self.core.prev_before(key)
    }

    /// Number of keys strictly below `key` (rank).
    #[must_use]
    pub fn count_below(&self, key: Key) -> u64 {
        self.flush_path();
        self.core.count_below(key)
    }

    /// Number of keys in the inclusive range (compat: `JudyLCount`).
    #[must_use]
    pub fn count_range(&self, range: core::ops::RangeInclusive<u64>) -> u64 {
        self.flush_path();
        self.core.count_range(range)
    }

    /// The entry with `n` keys below it — 0-based select (compat:
    /// `JudyLByCount`, which is 1-based).
    #[must_use]
    pub fn by_count(&self, n: u64) -> Option<(u64, u64)> {
        self.flush_path();
        self.core.by_count(n)
    }

    /// Creates a stateful forward [`MapCursor`](crate::cursor::MapCursor) for
    /// monotone skip-scans, positioned before the first entry.
    ///
    /// Unlike the stateless [`next_at_or_after`](Self::next_at_or_after), which
    /// re-descends from the root on every call, the cursor keeps its descent
    /// path and re-descends only from the deepest ancestor whose expanse still
    /// covers the next target (issue #340; docs/ALGORITHMS.md §3.5).
    #[must_use]
    pub fn cursor(&self) -> crate::cursor::MapCursor<'_> {
        self.core.cursor()
    }

    /// Creates a [`MapCursor`](crate::cursor::MapCursor) positioned at the
    /// smallest key `>= start`.
    #[must_use]
    pub fn cursor_from(&self, start: Key) -> crate::cursor::MapCursor<'_> {
        self.core.cursor_from(start)
    }

    /// Ascending iterator over `(key, value)` entries.
    #[must_use]
    pub fn iter(&self) -> MapIter<'_> {
        self.core.iter()
    }

    /// Returns an iterator over entries in the inclusive range `[start, end]`.
    #[must_use]
    pub fn range(&self, range: core::ops::RangeInclusive<Key>) -> MapRange<'_> {
        self.core.range(range)
    }

    /// Descending (double-ended) iterator over `(key, value)` entries.
    #[must_use]
    pub fn iter_rev(&self) -> MapIterRev<'_> {
        self.core.iter_rev()
    }

    /// Returns a descending (double-ended) iterator over entries in the
    /// inclusive range `[start, end]`.
    #[must_use]
    pub fn range_rev(&self, range: core::ops::RangeInclusive<Key>) -> MapRangeRev<'_> {
        self.core.range_rev(range)
    }

    /// Returns an iterator over entries in `range` where the hot metadata word
    /// (bits 63:40 of the 64-bit value slot) satisfies the predicate.
    pub fn range_filtered<'a, P>(
        &'a self,
        range: core::ops::RangeInclusive<Key>,
        predicate: P,
    ) -> impl Iterator<Item = (Key, u64)> + 'a
    where
        P: FnMut(Key, u32) -> bool + 'a,
    {
        self.core.range_filtered(range, predicate)
    }

    /// Scans entries in `range`, evaluating `predicate(key, hot_meta)` directly on
    /// the raw value slot before invoking `callback(key, raw_val)`.
    pub fn scan_filtered<P, F>(
        &self,
        range: core::ops::RangeInclusive<Key>,
        predicate: P,
        callback: F,
    ) where
        P: FnMut(Key, u32) -> bool,
        F: FnMut(Key, u64) -> bool,
    {
        self.core.scan_filtered(range, predicate, callback);
    }
}

impl Default for ExpanseMap {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ExpanseMap {
    fn drop(&mut self) {
        self.clear_entries();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// Pins the hazard `docs/ARCHITECTURE.md` "Snapshots" documents (#1103):
    /// a copy of the root edge is not a snapshot. The engine mutates nodes in
    /// place, so a value overwrite and an insert below the root are both
    /// visible through a root edge copied before them. If
    /// this ever fails, writes have become out of place and that section,
    /// and `Clone`'s documentation, are out of date.
    #[test]
    fn a_copied_root_edge_sees_later_writes() {
        let mut map = ExpanseMap::new();
        // Past the root leaf, so the root is a level-8 tree.
        for k in 0..(4 * ROOT_LEAF_CAP as u64) {
            map.insert(k, k);
        }
        let copied = match map.core.root {
            Root::Tree { top } => top,
            _ => panic!("the root must be a tree"),
        };
        // SAFETY: `copied` is the live root edge's value and every node it
        // reaches is live; the map is not mutated during the read.
        let read = |k: u64| unsafe { crate::get::get_map(&copied, k, 8) };
        assert_eq!(read(7), Some(7));
        assert_eq!(read(4 * ROOT_LEAF_CAP as u64), None);

        map.insert(7, 700);
        map.insert(4 * ROOT_LEAF_CAP as u64, 1);
        assert_eq!(
            read(7),
            Some(700),
            "an overwrite is visible through the copied root edge"
        );
        assert_eq!(
            read(4 * ROOT_LEAF_CAP as u64),
            Some(1),
            "an insert below the root is visible through the copied root edge"
        );

        // A clone is a snapshot.
        let snapshot = map.clone();
        map.insert(7, 7_000);
        assert_eq!(snapshot.get(7), Some(700));
    }

    /// The shared-tree engine (`OCC = true`) on one thread, so Miri can see
    /// its raw-pointer discipline (#568 PR 3): a map deferred to a collector
    /// in the wrapper's mode is driven through every node form — dense keys
    /// through the bitmap leaves, one wide level through `BranchB` into
    /// `BranchU`, scattered keys through cascades and narrow pointers — and
    /// back down to empty. Sized for the Tier-1 Miri lane (docs/CI.md §5).
    #[test]
    fn occ_engine_single_thread_under_miri() {
        // Both sharing modes: the engine covering the root (brief per-node
        // brackets) and the wrapper holding the tree word (nested brackets).
        occ_engine_drive(true);
        occ_engine_drive(false);
    }

    fn occ_engine_drive(engine_covers_root: bool) {
        // The wrapper would own the tree word beside the tree; the drive
        // declares it first so it outlives the tree.
        let word = crate::occ::SeqVersion::new();
        let mut m = ExpanseMap::new();
        let collector = std::sync::Arc::new(crate::occ::Collector::new());
        m.occ_root().1.defer_to(collector);
        // SAFETY: `word` is declared before the tree, so it drops after it.
        unsafe { m.occ_root().1.bind_tree_word(core::ptr::from_ref(&word)) };
        if engine_covers_root {
            m.occ_root().1.cover_root();
        } else {
            // The wrapper would hold the tree word; here the drive does.
            #[cfg(debug_assertions)]
            m.occ_root().1.bracket_enter_any();
        }
        let mut model = BTreeMap::new();
        let mut keys: Vec<u64> = Vec::new();
        // Dense: root leaf, promotion, linear leaves into a bitmap leaf.
        keys.extend(0..600u64);
        // One wide level: 200 distinct digits at level 2 (BranchB, then U).
        keys.extend((0..200u64).map(|i| (i + 1) << 8));
        // Scattered: cascades at every level and narrow pointers.
        let mut x = 0x9E37_79B9_7F4A_7C15u64;
        for _ in 0..40 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            keys.push(x);
        }
        for (i, &k) in keys.iter().enumerate() {
            let v = i as u64 * 3 + 1;
            assert_eq!(m.insert(k, v), model.insert(k, v), "insert {k:#x}");
        }
        assert_eq!(m.len(), model.len() as u64);
        m.validate();
        for &k in &keys {
            assert_eq!(m.get(k), model.get(&k).copied(), "get {k:#x}");
        }
        // Overwrite in place, then remove everything in an order that
        // crosses every hysteresis floor.
        for &k in keys.iter().step_by(7) {
            assert_eq!(m.insert(k, 0), model.insert(k, 0));
        }
        for &k in keys.iter().rev() {
            assert_eq!(m.remove(k), model.remove(&k), "remove {k:#x}");
        }
        assert!(m.is_empty());
        m.validate();
        if !engine_covers_root {
            #[cfg(debug_assertions)]
            m.occ_root().1.bracket_leave_any();
        }
        #[cfg(debug_assertions)]
        assert!(crate::alloc::bracket_stack::open().is_empty());
    }

    /// The insert-path cache on the slot entry points behind `JudyLGet` and
    /// `JudyLIns`: `get_value_slot` and `ins_slot` on a tree root while
    /// `insert` has left the cache on a level-1 terminal. A key of the
    /// cached block (`key >> 8`) is served from that terminal without a
    /// descent, so both terminal forms are driven — a bitmap leaf
    /// (`path.leaf`) and a 1-byte linear leaf (`path.leaf1`) — with present
    /// and absent keys, and a key of another block is probed while the
    /// cache is warm, which must not be served from it. `pending_pop` moves
    /// only on a warm insert, which pins the branch each `ins_slot` took.
    /// Sized for Miri, which checks the cache's raw dereferences.
    #[test]
    fn slot_calls_on_a_warm_insert_path() {
        fn slot_value(m: &mut ExpanseMap, key: u64) -> Option<u64> {
            // SAFETY: the slot is read before the next mutation of `m`.
            m.get_value_slot(key).map(|p| unsafe { *p.as_ptr() })
        }
        fn ins_slot_expect(m: &mut ExpanseMap, key: u64, expect: u64, val: u64) {
            let slot = m.ins_slot(key);
            // SAFETY: the slot is used before the next mutation of `m`.
            unsafe {
                assert_eq!(*slot.as_ptr(), expect, "ins_slot({key:#x}) value");
                slot.as_ptr().write(val);
            }
        }
        let val = |k: u64| k.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let mut m = ExpanseMap::new();
        let mut model = BTreeMap::new();
        // Blocks 0 and 1 in full: past ROOT_LEAF_CAP, so the root is a tree,
        // and block 1 is a populated neighbour to probe.
        for k in 0..0x200u64 {
            m.insert(k, val(k));
            model.insert(k, val(k));
        }
        assert!(
            matches!(m.core.root, Root::Tree { .. }),
            "root must be a tree"
        );

        // Bitmap terminal: 32 even digits of block 2, past LEAF1_CAP.
        let b = 0x200u64;
        for d in (0..0x40u64).step_by(2) {
            m.insert(b | d, val(b | d));
            model.insert(b | d, val(b | d));
        }
        let p = m.path_mut();
        assert_eq!(p.prefix, b >> 8, "insert must leave the cache on block 2");
        assert!(!p.leaf.is_null(), "block 2 must be cached as a bitmap leaf");
        let pending = p.pending_pop;

        assert_eq!(slot_value(&mut m, b | 0x10), Some(val(b | 0x10)));
        assert_eq!(slot_value(&mut m, b | 0x11), None);
        assert_eq!(
            slot_value(&mut m, 0x110),
            Some(val(0x110)),
            "get_value_slot served block 1 from the block-2 cache"
        );
        // A present key keeps its value and adds nothing.
        ins_slot_expect(&mut m, b | 0x3E, val(b | 0x3E), val(b | 0x3E));
        assert_eq!(m.path_mut().prefix, b >> 8);
        assert_eq!(m.path_mut().pending_pop, pending);
        // Absent keys: 0x01 grows subexpanse 0 from 16 to 17 values (a new
        // capacity class, so the subarray is reallocated), 0x03 from 17 to
        // 18 (spare capacity, shifted in place), 0x80 fills the empty
        // subexpanse 4 (a fresh subarray).
        for (i, d) in [0x01u64, 0x03, 0x80].into_iter().enumerate() {
            ins_slot_expect(&mut m, b | d, 0, val(b | d));
            model.insert(b | d, val(b | d));
            assert_eq!(
                m.path_mut().pending_pop,
                pending + i + 1,
                "ins_slot({:#x}) left the warm branch",
                b | d
            );
        }
        assert_eq!(slot_value(&mut m, b | 0x80), Some(val(b | 0x80)));
        ins_slot_expect(&mut m, 0x110, val(0x110), val(0x110));
        assert_eq!(m.len(), model.len() as u64);
        m.validate();

        // Linear terminal: ten digits of block 3, within LEAF1_CAP.
        let c = 0x300u64;
        for d in (0x10..=0xA0u64).step_by(0x10) {
            m.insert(c | d, val(c | d));
            model.insert(c | d, val(c | d));
        }
        let p = m.path_mut();
        assert_eq!(p.prefix, c >> 8, "insert must leave the cache on block 3");
        assert!(
            p.leaf.is_null() && !p.leaf1.is_null(),
            "block 3 must be cached as a linear leaf"
        );
        assert_eq!(p.terminal_pop, 10);
        let pending = p.pending_pop;

        assert_eq!(slot_value(&mut m, c | 0x30), Some(val(c | 0x30)));
        assert_eq!(slot_value(&mut m, c | 0x35), None);
        assert_eq!(
            slot_value(&mut m, 0x230),
            Some(val(0x230)),
            "get_value_slot served block 2 from the block-3 cache"
        );
        // The last key: its existing slot.
        ins_slot_expect(&mut m, c | 0xA0, val(c | 0xA0), val(c | 0xA0));
        assert_eq!(m.path_mut().pending_pop, pending);
        // Past the last key with spare class capacity: appended in place,
        // 10 -> 11 -> 12 keys, all in the 12-slot class.
        for (i, d) in [0xB0u64, 0xC0].into_iter().enumerate() {
            ins_slot_expect(&mut m, c | d, 0, val(c | d));
            model.insert(c | d, val(c | d));
            assert_eq!(
                m.path_mut().pending_pop,
                pending + i + 1,
                "ins_slot({:#x}) left the warm branch",
                c | d
            );
        }
        assert_eq!(m.path_mut().terminal_pop, 12);
        assert_eq!(slot_value(&mut m, c | 0xC0), Some(val(c | 0xC0)));
        // Before a descent reads the terminal edge's population back.
        m.validate();
        // The fallbacks to a descent: 0xD0 needs the next class (12 -> 16),
        // 0x05 and 0x20 sort below the last key.
        for d in [0xD0u64, 0x05, 0x20] {
            let expect = model.get(&(c | d)).copied().unwrap_or(0);
            ins_slot_expect(&mut m, c | d, expect, val(c | d));
            model.insert(c | d, val(c | d));
            assert_eq!(
                m.path_mut().pending_pop,
                0,
                "ins_slot({:#x}) must descend",
                c | d
            );
        }
        // A removal clears the cache; the slot calls descend again.
        m.insert(c | 0xE0, val(c | 0xE0));
        model.insert(c | 0xE0, val(c | 0xE0));
        assert_eq!(m.path_mut().prefix, c >> 8);
        assert_eq!(m.remove(c | 0x30), model.remove(&(c | 0x30)));
        assert_eq!(m.path_mut().prefix, u64::MAX, "remove must clear the cache");
        assert_eq!(slot_value(&mut m, c | 0x30), None);
        assert_eq!(slot_value(&mut m, c | 0x40), Some(val(c | 0x40)));

        assert_eq!(m.len(), model.len() as u64);
        m.validate();
        for (&k, &v) in &model {
            assert_eq!(m.get(k), Some(v), "get {k:#x}");
        }
    }

    /// Map twin of `set::tests::warm_insert_path_across_a_move`: the cache's
    /// root-slot entry lives in the map, not in a trie node, so a move leaves
    /// it naming memory the map has left, and it is never dereferenced. The
    /// cache is warmed in a helper whose frame then ends, and the map is moved
    /// into a `Box` and later out of it, which frees the box. Flushes then run
    /// with the stale entry recorded — select after the first move, the cache
    /// clear before a cold insert after the second — and population, select
    /// (keys and values) and the validator are checked against a model. Sized
    /// for Miri.
    #[test]
    fn warm_insert_path_across_a_move() {
        use crate::node::Edge;
        fn val(k: u64) -> u64 {
            k.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1
        }
        /// Address of the root edge slot while the root is a tree.
        fn root_slot(m: &ExpanseMap) -> *const Edge {
            match &m.core.root {
                Root::Tree { top } => core::ptr::from_ref(top),
                _ => panic!("root must be a tree"),
            }
        }
        /// The cache's one level-8 entry; every other ancestor must sit at a
        /// level in `2..=7`.
        fn root_entry(p: &crate::mutate_map::InsertPathMap) -> *const Edge {
            assert!(
                p.depth >= 2,
                "the cache must hold a terminal and its ancestors"
            );
            let (edges, levels) = (&p.edges[1..p.depth], &p.levels[1..p.depth]);
            assert!(
                levels.iter().all(|l| (2..=8).contains(l)),
                "ancestor levels {levels:?}"
            );
            let mut at_8 = edges.iter().zip(levels).filter(|&(_, &l)| l == 8);
            let (&root, _) = at_8
                .next()
                .expect("the root slot must be recorded at level 8");
            assert!(at_8.next().is_none(), "only the root slot sits at level 8");
            root.cast_const()
        }
        /// Population, select over every rank, and the validator. Select reads
        /// the ancestors' `pop0`, so its first call flushes the cache.
        fn check(m: &ExpanseMap, model: &BTreeMap<u64, u64>) {
            for (n, (&k, &v)) in model.iter().enumerate() {
                assert_eq!(m.by_count(n as u64), Some((k, v)), "by_count({n})");
            }
            assert_eq!(m.by_count(model.len() as u64), None);
            assert_eq!(m.len(), model.len() as u64);
            m.validate();
        }
        /// Inserts `keys`, each of which must take the bypass: it is the only
        /// path that adds to `pending_pop` without flushing it.
        fn insert_warm(
            m: &mut ExpanseMap,
            model: &mut BTreeMap<u64, u64>,
            keys: impl Iterator<Item = u64>,
        ) {
            for k in keys {
                let pending = m.path_mut().pending_pop;
                assert_eq!(m.insert(k, val(k)), None);
                model.insert(k, val(k));
                assert_eq!(
                    m.path_mut().pending_pop,
                    pending + 1,
                    "insert({k:#x}) left the warm path"
                );
            }
        }
        fn warmed() -> (ExpanseMap, BTreeMap<u64, u64>, *const Edge) {
            let mut m = ExpanseMap::new();
            let mut model = BTreeMap::new();
            // Blocks 0 and 1 in full, past ROOT_LEAF_CAP, so the root is a
            // tree; then 32 even digits of block 2, past LEAF1_CAP, which
            // leave the cache on block 2's bitmap leaf.
            for k in (0..0x200u64).chain((0x200..0x240).step_by(2)) {
                assert_eq!(m.insert(k, val(k)), None);
                model.insert(k, val(k));
            }
            let root = root_slot(&m);
            let p = m.path_mut();
            assert_eq!(p.prefix, 2, "insert must leave the cache on block 2");
            assert!(!p.leaf.is_null(), "block 2 must be cached as a bitmap leaf");
            assert!(
                core::ptr::eq(root_entry(p), root),
                "the level-8 entry is the root slot"
            );
            (m, model, root)
        }

        // The frame that recorded the root slot has ended, and the box moves
        // the map again.
        let (m, mut model, recorded) = warmed();
        let mut m = Box::new(m);
        assert!(!core::ptr::eq(root_slot(&m), recorded));
        assert!(core::ptr::eq(root_entry(m.path_mut()), recorded));
        insert_warm(&mut m, &mut model, (0x201..0x240).step_by(2));
        check(&m, &model);
        assert_eq!(m.path_mut().pending_pop, 0, "select must flush the cache");
        assert!(core::ptr::eq(root_entry(m.path_mut()), recorded));

        // A cold descent records the root slot inside the box; moving the map
        // out frees the box with the entry still naming it.
        for k in (0x300..0x340u64).step_by(2) {
            assert_eq!(m.insert(k, val(k)), None);
            model.insert(k, val(k));
        }
        let recorded = root_slot(&m);
        assert!(core::ptr::eq(root_entry(m.path_mut()), recorded));
        let mut m = {
            let boxed = m;
            *boxed
        };
        assert!(!core::ptr::eq(root_slot(&m), recorded));
        assert!(core::ptr::eq(root_entry(m.path_mut()), recorded));
        insert_warm(&mut m, &mut model, (0x301..0x340).step_by(2));
        // A key of another block descends cold: the cache is flushed, then
        // cleared, first.
        assert_eq!(m.insert(0x400, val(0x400)), None);
        model.insert(0x400, val(0x400));
        assert_eq!(
            m.path_mut().prefix,
            u64::MAX,
            "a cold insert must clear the cache"
        );
        check(&m, &model);
    }

    /// Regression for the fuzz crash `crash-7048e639` (ASan overflow):
    /// a 1-byte-remainder linear leaf with pop 9..=12 has a
    /// cap_class-derived key area of only 12 bytes, which the 16-byte
    /// vectorized search kernel must not be gated into. Exercises every
    /// pop in the formerly-misgated range through get/insert probes.
    #[test]
    fn kb1_leaf_pop_9_to_12_lookups() {
        for pop in 9usize..=12 {
            let mut m = ExpanseMap::new();
            for i in 0..pop as u64 {
                m.insert(i * 3, i + 100);
            }
            for i in 0..pop as u64 {
                assert_eq!(m.get(i * 3), Some(i + 100), "pop={pop} key={}", i * 3);
                assert_eq!(m.get(i * 3 + 1), None, "pop={pop} miss={}", i * 3 + 1);
            }
            assert_eq!(m.get(u64::MAX), None);
        }
    }

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

    #[cfg(miri)]
    const OPS: usize = 250;
    #[cfg(not(miri))]
    const OPS: usize = 6000;

    fn model_run(seed: u64, gen_key: impl Fn(&mut XorShift) -> u64) {
        let mut rng = XorShift(seed);
        let mut map = ExpanseMap::new();
        let mut model = BTreeMap::new();
        for op in 0..OPS {
            let key = gen_key(&mut rng);
            match rng.next() % 4 {
                0 | 3 => {
                    let val = rng.next();
                    assert_eq!(map.insert(key, val), model.insert(key, val), "ins {key:#x}");
                }
                1 => assert_eq!(map.remove(key), model.remove(&key), "rem {key:#x}"),
                _ => assert_eq!(map.get(key), model.get(&key).copied(), "get {key:#x}"),
            }
            assert_eq!(map.len(), model.len() as u64);
            if op % 64 == 0 {
                map.validate();
            }
        }
        map.validate();
        for (&k, &v) in &model {
            assert_eq!(map.get(k), Some(v), "model entry {k:#x}");
        }
        let keys: Vec<u64> = model.keys().copied().collect();
        for k in keys {
            assert_eq!(map.remove(k), model.remove(&k));
        }
        map.validate();
        assert!(map.is_empty());
        assert_eq!(map.mem_used(), 0);
    }

    #[test]
    fn model_sequential() {
        model_run(0x1A, |rng| rng.next() % 4096);
    }

    #[test]
    fn model_random_full_width() {
        model_run(0x2B, |rng| rng.next());
    }

    #[test]
    fn model_clustered() {
        let bases = [
            0u64,
            0xDEAD_BEEF_0000,
            0xFFFF_FFFF_FFFF_FF00,
            0x1234_5678_9ABC_0000,
        ];
        model_run(0x3C, move |rng| {
            let base = bases[(rng.next() % 4) as usize];
            base.wrapping_add(rng.next() % 512)
        });
    }

    #[test]
    fn model_sparse_high_bytes() {
        model_run(0x4D, |rng| (rng.next() % 4096) << 48);
    }

    #[test]
    fn ladder_full_climb_and_descend() {
        // 0..=255 with distinct values: immed → linear map leaf → bitmap
        // map leaf on the way up (no full expanse for maps), every
        // hysteresis step down, values intact throughout.
        let stride = if cfg!(miri) { 32 } else { 1 };
        let mut map = ExpanseMap::new();
        for k in 0u64..=255 {
            assert_eq!(map.insert(k, k * 7 + 1), None);
            if k % stride == 0 {
                map.validate();
            }
        }
        assert_eq!(map.len(), 256);
        for k in 0u64..=255 {
            assert_eq!(map.get(k), Some(k * 7 + 1), "value of {k}");
        }
        // Replacement must preserve structure and return the old value.
        assert_eq!(map.insert(100, 9999), Some(701));
        assert_eq!(map.get(100), Some(9999));
        map.validate();
        for k in (0u64..=255).rev() {
            let expected = if k == 100 { 9999 } else { k * 7 + 1 };
            assert_eq!(map.remove(k), Some(expected), "remove {k}");
            if k % stride == 0 {
                map.validate();
            }
        }
        assert!(map.is_empty());
        assert_eq!(map.mem_used(), 0);
    }

    #[test]
    fn wide_keys_drain_leaf_to_null() {
        // 7-byte remainders have map_immed_max == 1: leaves must drain to
        // null, never to an empty immediate.
        let mut map = ExpanseMap::new();
        let keys: Vec<u64> = (0..40u64)
            .map(|i| i.wrapping_mul(0x0000_FF12_3456_789B) & ((1 << 56) - 1))
            .collect();
        for (i, &k) in keys.iter().enumerate() {
            assert_eq!(map.insert(k, i as u64), None);
        }
        map.validate();
        for (i, &k) in keys.iter().enumerate() {
            assert_eq!(map.remove(k), Some(i as u64));
        }
        map.validate();
        assert!(map.is_empty());
        assert_eq!(map.mem_used(), 0);
    }

    #[test]
    fn wide_fanout_reaches_uncompressed_branch() {
        let mut map = ExpanseMap::new();
        for hi in 0u64..256 {
            for lo in 0u64..16 {
                let k = (hi << 8) | lo;
                assert_eq!(map.insert(k, !k), None);
            }
        }
        map.validate();
        assert_eq!(map.len(), 4096);
        for hi in 8u64..256 {
            for lo in 0u64..16 {
                let k = (hi << 8) | lo;
                assert_eq!(map.remove(k), Some(!k));
            }
            if hi % 64 == 0 {
                map.validate();
            }
        }
        map.validate();
        assert_eq!(map.len(), 8 * 16);
        map.clear();
        assert_eq!(map.mem_used(), 0);
    }

    fn nav_differential(map: &ExpanseMap, model: &BTreeMap<u64, u64>, probes: &[u64]) {
        let pair = |o: Option<(&u64, &u64)>| o.map(|(k, v)| (*k, *v));
        assert_eq!(map.first(), pair(model.first_key_value()));
        assert_eq!(map.last(), pair(model.last_key_value()));
        assert!(
            map.iter().eq(model.iter().map(|(k, v)| (*k, *v))),
            "iterator order/values"
        );
        for &k in probes {
            assert_eq!(
                map.next_at_or_after(k),
                pair(model.range(k..).next()),
                "next>={k:#x}"
            );
            assert_eq!(
                map.prev_at_or_before(k),
                pair(model.range(..=k).next_back()),
                "prev<={k:#x}"
            );
            if k < u64::MAX {
                assert_eq!(map.next_after(k), pair(model.range(k + 1..).next()));
            }
            if k > 0 {
                assert_eq!(map.prev_before(k), pair(model.range(..k).next_back()));
            }
            assert_eq!(map.count_below(k), model.range(..k).count() as u64);
        }
        for n in 0..model.len().min(64) as u64 {
            assert_eq!(map.by_count(n), pair(model.iter().nth(n as usize)));
        }
        assert_eq!(map.by_count(model.len() as u64), None);
        for pr in probes.chunks(2) {
            if let [a, b] = pr {
                let (a, b) = (*a.min(b), *a.max(b));
                assert_eq!(map.count_range(a..=b), model.range(a..=b).count() as u64);
            }
        }
    }

    #[test]
    fn navigation_matches_model() {
        let n_rand = if cfg!(miri) { 30 } else { 1500 };
        let mut rng = XorShift(0x5EED_BA5E_D00D_F00D);
        let mut map = ExpanseMap::new();
        let mut model = BTreeMap::new();
        for k in 0u64..=255 {
            map.insert(k, !k);
            model.insert(k, !k);
        }
        for _ in 0..n_rand {
            let k = match rng.next() % 3 {
                0 => rng.next(),
                1 => 0x77_0000_0000 + (rng.next() % 300),
                _ => (rng.next() % 2048) << 48,
            };
            let v = rng.next();
            map.insert(k, v);
            model.insert(k, v);
        }
        for k in [0u64, u64::MAX] {
            map.insert(k, k ^ 0x5A5A);
            model.insert(k, k ^ 0x5A5A);
        }
        map.validate();
        let mut probes: Vec<u64> = if cfg!(miri) {
            model.keys().copied().step_by(16).collect()
        } else {
            model.keys().copied().collect()
        };
        for &k in model.keys().take(if cfg!(miri) { 10 } else { 200 }) {
            probes.push(k.wrapping_add(1));
            probes.push(k.wrapping_sub(1));
        }
        for _ in 0..if cfg!(miri) { 10 } else { 400 } {
            probes.push(rng.next());
        }
        nav_differential(&map, &model, &probes);
        map.clear();
    }

    #[test]
    fn navigation_root_leaf_and_empty() {
        let map = ExpanseMap::new();
        assert_eq!(map.first(), None);
        assert_eq!(map.by_count(0), None);
        assert_eq!(map.count_range(0..=u64::MAX), 0);
        assert!(map.iter().next().is_none());

        let mut map = ExpanseMap::new();
        let mut model = BTreeMap::new();
        for (i, k) in [5u64, 100, 7, 0, u64::MAX, 1 << 40].into_iter().enumerate() {
            map.insert(k, i as u64 * 11);
            model.insert(k, i as u64 * 11);
        }
        let probes: Vec<u64> = (0..64u64).map(|i| i * 0x0404_0404_0404).collect();
        nav_differential(&map, &model, &probes);
    }

    #[test]
    fn value_slots_are_writable_and_stable_between_mutations() {
        // Covers get::locate_slot under Miri: slots over every terminal
        // form (immediates single/multi, linear leaves, bitmap leaves,
        // root leaf), written through and read back via the public API.
        let mut map = ExpanseMap::new();
        // Root-leaf state.
        map.insert(3, 30);
        map.insert(9, 90);
        let slot = map.get_value_slot(9).unwrap();
        // SAFETY: slot valid until the next mutation; none intervenes.
        unsafe { slot.as_ptr().write(91) };
        assert_eq!(map.get(9), Some(91));
        assert!(map.get_value_slot(4).is_none());

        // Tree state across the ladder: dense byte run (bitmap leaf),
        // clustered (leaves), sparse-high (immediates down deep chains).
        // Miri interprets every walk; keep the corpus small there.
        let (dense, clustered, sparse) = if cfg!(miri) {
            (64, 64, 16)
        } else {
            (256, 640, 64)
        };
        let mut keys = Vec::new();
        for k in 0u64..dense {
            map.insert(k, k + 1);
            keys.push(k);
        }
        for k in (0u64..clustered).map(|i| 0x30_0000 + i * 3) {
            map.insert(k, k + 1);
            keys.push(k);
        }
        for k in (1u64..sparse).map(|i| i << 52) {
            map.insert(k, k + 1);
            keys.push(k);
        }
        for &k in &keys {
            let slot = map.get_value_slot(k).expect("present key has a slot");
            // SAFETY: valid slot; read then write before any mutation.
            unsafe {
                assert_eq!(*slot.as_ptr(), k + 1, "slot reads the value {k:#x}");
                slot.as_ptr().write(!k);
            }
        }
        for &k in &keys {
            assert_eq!(map.get(k), Some(!k), "written value visible {k:#x}");
        }
        map.validate();
        map.clear();
        assert_eq!(map.mem_used(), 0);
    }

    #[test]
    fn model_prefix_runs() {
        model_run(0x6F, |rng| {
            let base = rng.next() & !0xFF;
            base | (rng.next() % 256)
        });
    }

    #[test]
    fn narrow_pointer_lifecycle_with_values() {
        let mut map = ExpanseMap::new();
        let mut model = BTreeMap::new();
        let base = 0x1122_3344_5566_0000u64;
        for i in 0..256u64 {
            assert_eq!(map.insert(base | i, i * 3 + 1), None);
            model.insert(base | i, i * 3 + 1);
        }
        map.validate();
        assert!(
            map.mem_used() <= 2600,
            "cluster should collapse to one skip edge + values, used {}",
            map.mem_used()
        );
        for i in 0..256u64 {
            assert_eq!(map.get(base | i), Some(i * 3 + 1), "value {i}");
        }
        assert_eq!(map.get(base ^ (1 << 32)), None);
        assert_eq!(map.count_range(base..=base | 0xFF), 256);
        assert_eq!(map.by_count(7), Some((base | 7, 22)));

        // Value slots must locate through the narrow pointer too.
        let slot = map.get_value_slot(base | 9).unwrap();
        // SAFETY: slot valid until the next mutation.
        unsafe { slot.as_ptr().write(9999) };
        model.insert(base | 9, 9999);
        assert_eq!(map.get(base | 9), Some(9999));

        // Divergence splits, then drain through the conversions.
        for div in [1u64 << 16, 1 << 32, 1 << 48] {
            let k = base ^ div;
            assert_eq!(map.insert(k, !k), None, "diverging insert {k:#x}");
            model.insert(k, !k);
            map.validate();
        }
        assert!(map.iter().eq(model.iter().map(|(k, v)| (*k, *v))));
        for i in (1..256u64).rev() {
            assert_eq!(map.remove(base | i), model.remove(&(base | i)), "rm {i}");
            if i % 32 == 0 {
                map.validate();
            }
        }
        assert!(map.iter().eq(model.iter().map(|(k, v)| (*k, *v))));
        map.clear();
        assert_eq!(map.mem_used(), 0);
    }

    #[test]
    fn branch_skip_clusters() {
        // Map mirror of the set-flavor cluster test: divergence-level
        // branch placement, navigation, slots, and drain through the
        // downgrade ladder — all across a skipping branch.
        let mut map = ExpanseMap::new();
        let mut model = BTreeMap::new();
        let bases = [0x1122_3344_5566_0000u64, 0x99AA_BBCC_DDEE_0000u64];
        let count = if cfg!(miri) { 64u64 } else { 512u64 };
        for &base in &bases {
            for i in 0..count {
                assert_eq!(map.insert(base | i, !(base | i)), None);
                model.insert(base | i, !(base | i));
            }
            map.validate();
        }
        // Values dominate map memory (8 bytes/key); the skip keeps the
        // structural overhead to one branch + two bitmap leaves per
        // cluster instead of a per-level chain.
        let per_key = map.mem_used() as f64 / model.len() as f64;
        assert!(
            per_key <= 14.0,
            "structural overhead should collapse, {per_key:.2} B/key"
        );
        assert!(map.iter().eq(model.iter().map(|(k, v)| (*k, *v))));
        assert_eq!(
            map.next_at_or_after(bases[0] | count),
            Some((bases[1], !bases[1]))
        );
        assert_eq!(
            map.prev_before(bases[1]),
            Some((bases[0] | (count - 1), !(bases[0] | (count - 1))))
        );
        assert_eq!(map.count_range(bases[0]..=bases[0] | (count - 1)), count);
        // Value slots resolve through the skipping branch.
        let slot = map.get_value_slot(bases[0] | (count / 2)).unwrap();
        // SAFETY: slot valid until next mutation.
        unsafe { slot.as_ptr().write(42) };
        assert_eq!(map.get(bases[0] | (count / 2)), Some(42));
        model.insert(bases[0] | (count / 2), 42);
        // Diverge inside the skipped span, then drain one cluster.
        let split = bases[0] ^ (0x31 << 24);
        assert_eq!(map.insert(split, 7), None);
        model.insert(split, 7);
        map.validate();
        for i in (0..count).rev() {
            assert_eq!(map.remove(bases[0] | i), model.remove(&(bases[0] | i)));
            if i % 16 == 0 {
                map.validate();
                assert!(map.iter().eq(model.iter().map(|(k, v)| (*k, *v))));
            }
        }
        map.clear();
        assert_eq!(map.mem_used(), 0);
    }

    #[test]
    fn root_condenses_and_regrows() {
        let mut map = ExpanseMap::new();
        let mut model = BTreeMap::new();
        for k in 0u64..120 {
            map.insert(k << 24, !k);
            model.insert(k << 24, !k);
        }
        let stride = if cfg!(miri) { 10 } else { 1 };
        for k in (20u64..120).rev() {
            assert_eq!(map.remove(k << 24), model.remove(&(k << 24)), "rm {k}");
            if k % stride == 0 {
                map.validate();
                assert!(map.iter().eq(model.iter().map(|(k, v)| (*k, *v))), "at {k}");
            }
        }
        // Values survive condensation; slots still work.
        let slot = map.get_value_slot(5 << 24).unwrap();
        // SAFETY: slot valid until next mutation.
        unsafe { slot.as_ptr().write(777) };
        assert_eq!(map.get(5 << 24), Some(777));
        for k in 300u64..360 {
            map.insert(k << 40, k);
            model.insert(k << 40, k);
        }
        model.insert(5 << 24, 777);
        map.validate();
        assert!(map.iter().eq(model.iter().map(|(k, v)| (*k, *v))));
        map.clear();
        assert_eq!(map.mem_used(), 0);
    }

    #[test]
    #[should_panic(expected = "branch pop0 disagrees with subtree")]
    fn negative_control_validator_must_fire() {
        let mut map = ExpanseMap::new();
        for k in 0u64..200 {
            map.insert(k * 977, k);
        }
        let Root::Tree { top, .. } = &mut map.core.root else {
            panic!("expected a tree root");
        };
        // SAFETY: the top is a live BranchL3 (one distinct top digit in
        // this key set); we only rewrite the first child edge's aux bytes.
        unsafe {
            let b = &mut *top.node_ptr().cast::<crate::node::BranchL3>();
            let child = &mut b.edges[0];
            let pop0 = child.pop0(7);
            child.set_pop0(7, pop0 + 1);
        }
        map.validate();
    }

    #[test]
    fn test_deferred_ancestor_pop_clustered_and_boundary_flush() {
        let mut map = ExpanseMap::new();
        for cluster in 0..10u64 {
            let prefix = (cluster + 1) << 16;
            for i in 0..200u64 {
                assert_eq!(map.insert(prefix | i, i * 10), None);
            }
        }
        assert_eq!(map.len(), 2000);
        for cluster in 0..10u64 {
            let prefix = (cluster + 1) << 16;
            for i in 0..200u64 {
                assert_eq!(map.get(prefix | i), Some(i * 10));
            }
        }
        map.validate();
        assert_eq!(map.len(), 2000);
    }

    #[test]
    fn test_sequential_linear_leaf_cursor_bypass_and_upgrade() {
        let mut map = ExpanseMap::new();
        for i in 0..1000u64 {
            assert_eq!(map.insert(i, i * 3), None);
            assert_eq!(map.get(i), Some(i * 3));
            assert_eq!(map.len(), i + 1);
        }
        map.validate();
        for i in 0..1000u64 {
            assert_eq!(map.get(i), Some(i * 3));
        }
        for i in 0..1000u64 {
            assert_eq!(map.remove(i), Some(i * 3));
            assert_eq!(map.get(i), None);
        }
        map.validate();
        assert!(map.is_empty());
        assert_eq!(map.mem_used(), 0);
    }

    #[test]
    fn test_from_iterator_and_extend() {
        let entries: Vec<(u64, u64)> = (0..500u64).map(|i| (i * 7, i * 13)).collect();
        let map: ExpanseMap = entries.iter().copied().collect();
        assert_eq!(map.len(), 500);
        map.validate();
        for &(k, v) in &entries {
            assert_eq!(map.get(k), Some(v));
        }

        let mut extended = ExpanseMap::new();
        extended.extend(entries.iter().copied());
        assert_eq!(extended.len(), 500);
        extended.validate();
        for &(k, v) in &entries {
            assert_eq!(extended.get(k), Some(v));
        }
    }

    #[test]
    #[allow(clippy::reversed_empty_ranges)]
    fn test_map_range_cursor_parity_with_btreemap() {
        use std::collections::BTreeMap;
        let mut expanse = ExpanseMap::new();
        let mut btree = BTreeMap::new();

        // 1. Root leaf population (small)
        for i in [10u64, 25, 30, 42, 50, 75, 99, 120] {
            expanse.insert(i, i * 2);
            btree.insert(i, i * 2);
        }

        let queries = [
            0..=0,
            0..=10,
            10..=10,
            11..=24,
            25..=50,
            50..=120,
            100..=200,
            150..=200,
            0..=u64::MAX,
            50..=10, // inverted range
        ];

        for r in &queries {
            let exp_res: Vec<_> = expanse.range(r.clone()).collect();
            let bt_res: Vec<_> = if r.start() <= r.end() {
                btree.range(r.clone()).map(|(&k, &v)| (k, v)).collect()
            } else {
                vec![]
            };
            assert_eq!(exp_res, bt_res, "Root leaf range mismatch for {:?}", r);
        }

        // 2. Large multi-level trie population across multiple key distributions
        let mut large_exp = ExpanseMap::new();
        let mut large_bt = BTreeMap::new();

        // Dense cluster 1
        for i in 1000..2000u64 {
            large_exp.insert(i, i ^ 0xAA);
            large_bt.insert(i, i ^ 0xAA);
        }
        // Sparse cluster 2
        for i in (100_000..200_000u64).step_by(128) {
            large_exp.insert(i, i ^ 0xBB);
            large_bt.insert(i, i ^ 0xBB);
        }
        // Wide 64-bit keys
        for i in 0..500u64 {
            let k = (i + 1).wrapping_mul(0x0102_0304_0506_0708);
            large_exp.insert(k, i);
            large_bt.insert(k, i);
        }

        let trie_queries = [
            0..=500,
            1000..=1050,
            1500..=2500,
            99_000..=150_000,
            150_000..=250_000,
            0x0102_0304_0506_0708..=0x0502_0304_0506_0708,
            0..=u64::MAX,
            u64::MAX..=u64::MAX,
            2000..=1000,
        ];

        for r in &trie_queries {
            let exp_res: Vec<_> = large_exp.range(r.clone()).collect();
            let bt_res: Vec<_> = if r.start() <= r.end() {
                large_bt.range(r.clone()).map(|(&k, &v)| (k, v)).collect()
            } else {
                vec![]
            };
            assert_eq!(exp_res, bt_res, "Large trie range mismatch for {:?}", r);
        }
    }

    #[test]
    fn test_get_batch_against_single_get() {
        let mut map = ExpanseMap::new();
        let mut all_keys = Vec::new();

        // 1. Empty map batch test
        let mut out = [Some(999); 8];
        map.get_batch(&[1, 2, 3, 4, 5, 6, 7, 8], &mut out);
        assert_eq!(out, [None; 8]);

        // 2. Populate diverse keys: sequential, sparse, wide 64-bit
        for i in 0..10_000u64 {
            let k = if i % 3 == 0 {
                i
            } else if i % 3 == 1 {
                i * 1000
            } else {
                i.wrapping_mul(0x1122_3344_5566_7788)
            };
            let v = k ^ 0xDEAD_BEEF;
            map.insert(k, v);
            all_keys.push(k);
        }

        // Test various batch sizes: 0, 1, 7, 8, 9, 16, 100, full
        let test_batch_sizes = [0, 1, 2, 7, 8, 9, 15, 16, 64, 100, 1000];
        for &size in &test_batch_sizes {
            if size > all_keys.len() {
                continue;
            }
            let query_keys: Vec<u64> = all_keys[..size]
                .iter()
                .enumerate()
                .map(|(idx, &k)| if idx % 4 == 0 { k + 1 } else { k })
                .collect();

            let mut batch_out = vec![None; size];
            map.get_batch(&query_keys, &mut batch_out);

            let mut values_out = vec![0u64; size];
            let mut found_out = vec![false; size];
            let found_count =
                map.get_batch_into(&query_keys, &mut values_out, Some(&mut found_out));

            let mut expected_found = 0;
            for (idx, &k) in query_keys.iter().enumerate() {
                let single = map.get(k);
                assert_eq!(batch_out[idx], single, "Mismatch for key {}", k);
                assert_eq!(found_out[idx], single.is_some());
                if let Some(val) = single {
                    assert_eq!(values_out[idx], val);
                    expected_found += 1;
                }
            }
            assert_eq!(found_count, expected_found);
        }
    }

    /// Deterministic xorshift64, so a profile is reproducible across hosts.
    fn xs64(state: &mut u64) -> u64 {
        let mut x = *state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *state = x;
        x
    }

    /// `get_batch` must agree with `get` at every interleave width, not only
    /// the shipping one: the width is a tuning knob and a knob that can
    /// change an answer is not a tuning knob.
    #[test]
    fn batch_width_parity_with_single_get() {
        let mut st = 0x243F_6A88_85A3_08D3u64;
        let mut map = ExpanseMap::new();
        let mut present = Vec::new();
        for _ in 0..40_000 {
            let k = xs64(&mut st);
            map.insert(k, k ^ 0x5DEE_CE66);
            present.push(k);
        }
        // Half hits, half misses, interleaved so no width sees a uniform run.
        let mut probes = Vec::new();
        for (i, &k) in present.iter().take(4_000).enumerate() {
            probes.push(k);
            probes.push(k.wrapping_add(1).wrapping_mul(0x9E37_79B9_7F4A_7C15) | (i as u64 & 1));
        }
        let expect: Vec<Option<u64>> = probes.iter().map(|&k| map.get(k)).collect();

        macro_rules! check {
            ($($w:literal),*) => {$({
                let mut out = vec![Some(0); probes.len()];
                map.get_batch_width::<$w>(&probes, &mut out);
                assert_eq!(out, expect, "map get_batch_width::<{}> disagrees with get", $w);
                // Short slices exercise the priming and drain paths, where the
                // driver runs below its nominal width.
                for len in [0usize, 1, 2, 3, $w - 1, $w, $w + 1, 2 * $w + 3] {
                    let len = len.min(probes.len());
                    let mut short = vec![Some(0); len];
                    map.get_batch_width::<$w>(&probes[..len], &mut short);
                    assert_eq!(short[..], expect[..len], "len {} at width {}", len, $w);
                }
            })*};
        }
        check!(1, 2, 3, 4, 5, 8, 12, 16, 24, 32, 64);
    }

    /// The same parity requirement for the set flavor.
    #[test]
    fn batch_width_parity_with_single_contains() {
        use crate::set::ExpanseSet;
        let mut st = 0x13198A2E_03707344u64;
        let mut set = ExpanseSet::new();
        let mut present = Vec::new();
        for _ in 0..40_000 {
            let k = xs64(&mut st);
            set.insert(k);
            present.push(k);
        }
        let mut probes = Vec::new();
        for (i, &k) in present.iter().take(4_000).enumerate() {
            probes.push(k);
            probes.push(k.wrapping_add(1).wrapping_mul(0x9E37_79B9_7F4A_7C15) | (i as u64 & 1));
        }
        let expect: Vec<bool> = probes.iter().map(|&k| set.contains(k)).collect();
        let expect_count = expect.iter().filter(|&&b| b).count();

        macro_rules! check {
            ($($w:literal),*) => {$({
                let mut out = vec![false; probes.len()];
                let n = set.contains_batch_width::<$w>(&probes, &mut out);
                assert_eq!(out, expect, "set contains_batch_width::<{}> disagrees", $w);
                assert_eq!(n, expect_count);
            })*};
        }
        check!(1, 2, 3, 4, 5, 8, 12, 16, 24, 32, 64);
    }

    /// Measures the quantity the batched path exists to raise: how many
    /// independent descents are in flight at once.
    ///
    /// This is arithmetic over the trie's own chain lengths — deterministic,
    /// machine-independent, and free of any timing — so it can be checked
    /// here rather than on a quiet host. What it does **not** establish is
    /// that lanes in flight convert into wall clock: a lane step is a
    /// dependent node load, but only the ones that miss to DRAM cost the
    /// latency this is meant to overlap, and the conversion is bounded above
    /// by the core's outstanding-miss budget. That measurement is
    /// `benches/batch_lookup.rs` on the reference host.
    ///
    /// The assertion is the ordering the driver was changed for: refilling a
    /// retired lane from the key stream holds the width, where running fixed
    /// chunks to completion lets it decay to the chunk's deepest lane.
    #[test]
    fn batch_lane_occupancy_profile() {
        lane_occupancy_profile(100_000, 10_000);
    }

    /// The same instrument at the population `benches/compare.rs`'s
    /// cold-DRAM arm uses, where the trie is deep enough and the working set
    /// far enough past the LLC for the chain length to be the one the
    /// batched path is trying to overlap. Ignored by default: building 4M
    /// keys is not a unit test.
    ///
    /// `cargo test -p expanse-trie --release --lib lane_occupancy_cold_dram -- --ignored --nocapture`
    #[test]
    #[ignore = "4M-key population; run explicitly in release"]
    fn batch_lane_occupancy_profile_cold_dram() {
        lane_occupancy_profile(4_000_000, 100_000);
    }

    fn lane_occupancy_profile(pop: usize, probe_pairs: usize) {
        let mut st = 0xA409_3822_299F_31D0u64;
        let mut map = ExpanseMap::new();
        let mut present = Vec::new();
        for _ in 0..pop {
            let k = xs64(&mut st);
            map.insert(k, !k);
            present.push(k);
        }
        // 50% hit / 50% miss, the realistic read mix AGENTS.md §8.6 asks for.
        let mut probes = Vec::new();
        for i in 0..probe_pairs {
            probes.push(present[i * 7 % present.len()]);
            probes.push(xs64(&mut st));
        }

        let Root::Tree { top, .. } = &map.core.root else {
            panic!("100k random keys must build a tree root");
        };
        let depths: Vec<u32> = probes
            .iter()
            // SAFETY: `top` roots a live tree covering 8 undecoded key bytes.
            .map(|&k| unsafe { crate::get::descent_steps_map(top, k, 8) })
            .collect();
        let total_steps: u64 = depths.iter().map(|&d| d as u64).sum();

        // The #294 policy: fixed groups, each run to completion.
        fn chunked(depths: &[u32], w: usize) -> f64 {
            let mut sweeps = 0u64;
            let mut steps = 0u64;
            for c in depths.chunks(w) {
                sweeps += u64::from(*c.iter().max().unwrap());
                steps += c.iter().map(|&d| u64::from(d)).sum::<u64>();
            }
            steps as f64 / sweeps as f64
        }

        // This driver: a retired lane takes the next key immediately.
        fn streaming(depths: &[u32], w: usize) -> f64 {
            let mut lanes: Vec<u32> = Vec::with_capacity(w);
            let mut next = 0usize;
            while lanes.len() < w && next < depths.len() {
                lanes.push(depths[next]);
                next += 1;
            }
            let (mut sweeps, mut steps) = (0u64, 0u64);
            while !lanes.is_empty() {
                sweeps += 1;
                let mut i = 0;
                while i < lanes.len() {
                    lanes[i] -= 1;
                    steps += 1;
                    if lanes[i] == 0 {
                        if next < depths.len() {
                            lanes[i] = depths[next];
                            next += 1;
                            i += 1;
                        } else {
                            let last = lanes.pop().expect("non-empty");
                            if i < lanes.len() {
                                lanes[i] = last;
                            }
                        }
                    } else {
                        i += 1;
                    }
                }
            }
            steps as f64 / sweeps as f64
        }

        let mean_depth = total_steps as f64 / depths.len() as f64;
        let mut hist = [0usize; 16];
        for &d in &depths {
            hist[(d as usize).min(15)] += 1;
        }
        std::println!(
            "batch lane occupancy: pop {}, {} probes, mean chain length {:.2} levels",
            present.len(),
            depths.len(),
            mean_depth
        );
        std::println!(
            "  chain-length histogram: {:?}",
            hist.iter()
                .enumerate()
                .filter(|&(_, &c)| c > 0)
                .map(|(d, &c)| (d, c))
                .collect::<Vec<_>>()
        );
        std::println!("  W  chunked(#294)  streaming(this)  of W");
        for &w in &[1usize, 2, 4, 8, 12, 16, 24, 32] {
            let c = chunked(&depths, w);
            let s = streaming(&depths, w);
            std::println!(
                "{:3}  {:12.2}  {:15.2}  {:5.1}%",
                w,
                c,
                s,
                100.0 * s / w as f64
            );
            assert!(s >= c - 1e-9, "width {w}: streaming {s} below chunked {c}");
            assert!(s <= w as f64 + 1e-9, "width {w}: streaming {s} above W");
            if w > 1 {
                // A width that does not actually hold its lanes is a width in
                // name only; this is the property the refill exists for.
                assert!(s > 0.98 * w as f64, "width {w}: streaming held only {s}");
            }
        }
    }

    #[test]
    fn test_map_range_and_scan_filtered_extracts_correct_metadata() {
        use crate::slot::ValueSlot;

        let mut map = ExpanseMap::new();
        // Insert values formatted with ValueSlot::new_arena_meta(meta, locator).
        // meta is 24-bit (bits 63:40), locator is 32-bit (bits 39:8), tag is 0x10.
        // If locator has bits set in bits 39:32, an incorrect (v >> 32) decoder would pollute
        // the lower 8 bits of the extracted metadata with the high byte of locator.
        let meta_target = 0x123456;
        let locator_with_high_bits = 0xFF00_1234; // bits 39:32 in slot will be 0xFF
        let slot1 = ValueSlot::new_arena_meta(meta_target, locator_with_high_bits)
            .expect("valid arena meta");
        let slot2 = ValueSlot::new_arena_meta(0x654321, 0x0000_5678).expect("valid arena meta");

        map.insert(100, slot1.to_raw());
        map.insert(200, slot2.to_raw());

        // Test range_filtered
        let filtered: Vec<(u64, u64)> = map
            .range_filtered(0..=300, |_k, m| m == meta_target)
            .collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].0, 100);
        assert_eq!(filtered[0].1, slot1.to_raw());

        // Test scan_filtered
        let mut scanned = Vec::new();
        map.scan_filtered(
            0..=300,
            |_k, m| m == meta_target,
            |k, v| {
                scanned.push((k, v));
                true
            },
        );
        assert_eq!(scanned.len(), 1);
        assert_eq!(scanned[0].0, 100);
        assert_eq!(scanned[0].1, slot1.to_raw());
    }
}
