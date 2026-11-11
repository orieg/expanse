//! The forward-only walker behind `ExpanseSet::iter` and `ExpanseMap::iter`.
//!
//! It is the stack walk of [`crate::iter::RawIter`] with three more leaf
//! states: runs of single-key immediate siblings read in place from a
//! `BranchU`'s or `BranchB`'s slots (`ImmedRunU`, `ImmedRunB`), and a map
//! bitmap leaf walked in place (`BitmapMapAsc`). It has no seek and no
//! reverse walk. Ranges, cursors and the reverse iterators keep `RawIter`,
//! whose leaf-cursor enum this type does not widen (#1687).

use crate::bits::Bitmap256;
use crate::leaf;
use crate::mutate::decode_value;
use crate::node::{BranchB, BranchL3, BranchL7, BranchU, Edge, LeafBitmap1, LeafBitmapL};
use crate::types::{EdgeTag, EdgeType, Key};

/// State of an active branch node on the traversal stack.
#[derive(Clone, Copy)]
enum BranchKind {
    L3 {
        ptr: *const BranchL3,
        num: u8,
        digits: [u8; 8],
        idx: u8,
    },
    L7 {
        ptr: *const BranchL7,
        num: u8,
        digits: [u8; 8],
        idx: u8,
    },
    B {
        ptr: *const BranchB,
        current_digit: u16,
    },
    U {
        ptr: *const BranchU,
        current_digit: u16,
    },
}

#[derive(Clone, Copy)]
struct StackFrame {
    kind: BranchKind,
    level: u8,
    prefix: u64,
}

/// State of an active leaf node yielding elements directly.
#[derive(Clone, Copy)]
enum LeafCursor {
    Empty,
    RootLeaf {
        keys: *const u64,
        values: *const u64,
        pop: u32,
        idx: u32,
    },
    Linear {
        keys_ptr: *const u8,
        values_ptr: *const u64,
        kb: u8,
        pop: u16,
        idx: u16,
        prefix: u64,
    },
    BitmapSet {
        words: [u64; 4],
        word_idx: u8,
        prefix: u64,
    },
    /// The forward walk's map bitmap leaf, read in place: `bits` holds the
    /// digits of subexpanse `sub` not yet read, `value` the value slot of the
    /// lowest of them, and `base` the keys' common bits down to `sub`. Built
    /// only where `MAP`; a set's bitmap leaf is [`Self::BitmapSet`].
    BitmapMapAsc {
        node: *const LeafBitmapL,
        value: *const u64,
        base: u64,
        bits: u32,
        sub: u8,
    },
    ImmedSingle {
        key: u64,
        value: u64,
        done: bool,
    },
    ImmedMulti {
        keys: [u64; 15],
        values: [u64; 15],
        count: u8,
        idx: u8,
    },
    FullExpanse {
        next_key: u64,
        max_key: u64,
    },
    /// A forward walk over a run of single-key immediate children of the
    /// `BranchU` on top of the stack, read in place from its slots. `edge` is
    /// the next slot to read and `end` one past the last; `key` holds the
    /// bits of `edge`'s keys above their own bytes, `step` apart per slot.
    /// The frame's own `current_digit` is stale while this cursor is live and
    /// is written back when the run ends.
    ImmedRunU {
        edge: *const Edge,
        end: *const Edge,
        key: u64,
        step: u64,
        mask: u64,
        tag: u8,
    },
    /// [`Self::ImmedRunU`] over the children of a `BranchB`: `bits` holds the
    /// digits of subexpanse `sub` not yet read, `edge` the subarray slot of
    /// the lowest of them, and `base` the keys' common bits down to `sub`.
    /// While `inner_left` is non-zero the run is inside a linear-leaf child
    /// that skips no level, whose next key is at `inner_keys` (and value at
    /// `inner_values`) under the bits `inner_base`.
    ImmedRunB {
        ptr: *const BranchB,
        edge: *const Edge,
        base: u64,
        mask: u64,
        inner_keys: *const u8,
        inner_values: *const u64,
        inner_base: u64,
        bits: u32,
        inner_left: u16,
        sub: u8,
        shift: u8,
        tag: u8,
    },
}

/// Extracts the `(key_suffix, value)` from a 1-key immediate edge.
#[inline(always)]
fn unpack_immed_single<const MAP: bool>(edge: &Edge, im: crate::types::ImmedType) -> (u64, u64) {
    let kb = im.key_bytes() as usize;
    let mask = if kb >= 8 {
        u64::MAX
    } else {
        (1u64 << (8 * kb)) - 1
    };
    let low = if MAP {
        edge.aux_word() & mask
    } else {
        edge.word0() & mask
    };
    let value = if MAP { edge.word0() } else { 0 };
    (low, value)
}

/// Key bytes of `edge` when it is a single-key immediate, else 0 (the null
/// edge and every other tag). A single-key immediate's tag byte is `kb << 4`
/// with `kb` in 1..=7 (`ImmedType::as_u8` at `key_count == 1`), and no
/// structural tag has that shape; the const block below pins both halves
/// over every tag byte.
#[inline(always)]
const fn single_immed_kb(edge: &Edge) -> u8 {
    let raw = edge.tag_byte();
    if raw & 0x8F == 0 { raw >> 4 } else { 0 }
}

const _: () = {
    let mut raw = 0u16;
    while raw < 256 {
        let single = match EdgeTag::from_u8(raw as u8) {
            Some(EdgeTag::Immed(im)) if im.key_count() == 1 => im.key_bytes(),
            _ => 0,
        };
        let fast = if raw as u8 & 0x8F == 0 {
            raw as u8 >> 4
        } else {
            0
        };
        assert!(single == fast);
        raw += 1;
    }
};

/// A zero-allocation, stack-based ascending walk over a whole trie.
pub(crate) struct RunIter<const MAP: bool> {
    leaf: LeafCursor,
    stack: [StackFrame; 8],
    depth: usize,
}

impl<const MAP: bool> RunIter<MAP> {
    /// Creates an empty iterator.
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self {
            leaf: LeafCursor::Empty,
            stack: [StackFrame {
                kind: BranchKind::U {
                    ptr: core::ptr::null(),
                    current_digit: 256,
                },
                level: 0,
                prefix: 0,
            }; 8],
            depth: 0,
        }
    }

    /// Initializes iteration from a root leaf.
    pub(crate) fn from_root_leaf(keys: *const u64, values: *const u64, pop: usize) -> Self {
        let mut iter = Self::new();
        iter.leaf = LeafCursor::RootLeaf {
            keys,
            values,
            pop: pop as u32,
            idx: 0,
        };
        iter
    }

    /// Initializes iteration from a root edge.
    ///
    /// # Safety
    /// `top` must point to a live, well-formed trie edge.
    pub(crate) unsafe fn from_tree(top: &Edge) -> Self {
        let mut iter = Self::new();
        // SAFETY: top points to a live trie per contract.
        unsafe {
            iter.descend(top, 8, 0);
        }
        iter
    }

    /// Descends from `edge` into the leftmost leaf, pushing branches onto `stack`.
    ///
    /// # Safety
    /// `edge` must point to a live node / leaf at `level`.
    #[inline(always)]
    unsafe fn descend(&mut self, edge: &Edge, level: u8, prefix: u64) {
        let mut cur_edge = *edge;
        let mut cur_level = level;
        let mut cur_prefix = prefix;

        loop {
            if cur_edge.is_null() {
                return;
            }

            let tag = match cur_edge.tag() {
                Some(t) => t,
                None => return,
            };

            match tag {
                EdgeTag::Structural(EdgeType::Null) => return,

                EdgeTag::Immed(im) => {
                    let n = im.key_count();
                    // Single-key immediates dominate sparse-key tries (every
                    // leaf holds one key). Decode the one key/value directly,
                    // skipping the two 15-wide staging arrays and the wide
                    // cursor copy the multi-key path pays per element.
                    if n == 1 {
                        let (low, value) = unpack_immed_single::<MAP>(&cur_edge, im);
                        self.leaf = LeafCursor::ImmedSingle {
                            key: cur_prefix | low,
                            value,
                            done: false,
                        };
                        return;
                    }
                    let (keys_buf, vals_buf) = if MAP {
                        let k = crate::mutate::immed_map_keys(&cur_edge, im);
                        let mut v = [0u64; 15];
                        // `n >= 2` here: the single-key immediate returned above.
                        let v_ptr = cur_edge.node_ptr().cast::<u64>();
                        for (i, val) in v.iter_mut().enumerate().take(n as usize) {
                            // SAFETY: live value array for n entries.
                            *val = unsafe { *v_ptr.add(i) };
                        }
                        (k, v)
                    } else {
                        (crate::mutate::immed_keys(&cur_edge, im), [0u64; 15])
                    };
                    let mut keys = [0u64; 15];
                    for i in 0..n as usize {
                        keys[i] = cur_prefix | keys_buf[i];
                    }
                    self.leaf = LeafCursor::ImmedMulti {
                        keys,
                        values: vals_buf,
                        count: n,
                        idx: 0,
                    };
                    return;
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
                    let pop = cur_edge.pop0(kb) as u16 + 1;
                    let dv = if kb < cur_level {
                        decode_value(&cur_edge, kb, cur_level)
                    } else {
                        0
                    };
                    let leaf_prefix = cur_prefix | (dv << (8 * u32::from(kb)));
                    let base = cur_edge.node_ptr();
                    let (keys_ptr, values_ptr): (*const u8, *const u64) = if MAP {
                        // SAFETY: map leaf layout: values then class-sized keys.
                        let keys = unsafe { base.add(leaf::map_keys_offset(pop as usize)) };
                        (keys, base.cast::<u64>())
                    } else {
                        (base, core::ptr::null())
                    };
                    self.leaf = LeafCursor::Linear {
                        keys_ptr,
                        values_ptr,
                        kb,
                        pop,
                        idx: 0,
                        prefix: leaf_prefix,
                    };
                    return;
                }

                EdgeTag::Structural(EdgeType::LeafB1) => {
                    let dv = if cur_level > 1 {
                        decode_value(&cur_edge, 1, cur_level)
                    } else {
                        0
                    };
                    let leaf_prefix = cur_prefix | (dv << 8);
                    if MAP {
                        // SAFETY: live LeafBitmapL node pointer.
                        let b = unsafe { &*cur_edge.node_ptr().cast::<LeafBitmapL>() };
                        self.leaf = LeafCursor::BitmapMapAsc {
                            node: b,
                            value: b.values[0],
                            base: leaf_prefix,
                            bits: (b.bitmap.words[0] & 0xFFFF_FFFF) as u32,
                            sub: 0,
                        };
                    } else {
                        // SAFETY: live LeafBitmap1 node pointer.
                        let b = unsafe { &*cur_edge.node_ptr().cast::<LeafBitmap1>() };
                        self.leaf = LeafCursor::BitmapSet {
                            words: b.bitmap.words,
                            word_idx: 0,
                            prefix: leaf_prefix,
                        };
                    }
                    return;
                }

                EdgeTag::Structural(EdgeType::FullExpanse) => {
                    let max = if cur_level == 8 {
                        u64::MAX
                    } else {
                        (1u64 << (8 * u32::from(cur_level))) - 1
                    };
                    self.leaf = LeafCursor::FullExpanse {
                        next_key: cur_prefix,
                        max_key: cur_prefix | max,
                    };
                    return;
                }

                EdgeTag::Structural(t @ (EdgeType::BranchL3 | EdgeType::BranchL7)) => {
                    // SAFETY: live branch per contract.
                    let bl = unsafe { crate::mutate::branch_form_level(&cur_edge, t, cur_level) };
                    let dv = if bl < cur_level {
                        decode_value(&cur_edge, bl, cur_level)
                    } else {
                        0
                    };
                    let branch_prefix = if bl < cur_level {
                        cur_prefix | (dv << (8 * u32::from(bl)))
                    } else {
                        cur_prefix
                    };

                    if matches!(t, EdgeType::BranchL3) {
                        // SAFETY: live BranchL3 pointer.
                        let b = unsafe { &*cur_edge.node_ptr().cast::<BranchL3>() };
                        let num = b.hdr.num;
                        if num == 0 {
                            return;
                        }
                        let d = b.hdr.digits[0];
                        let child_prefix =
                            branch_prefix | (u64::from(d) << (8 * u32::from(bl - 1)));
                        // SAFETY: slot 0 is valid for non-empty branch.
                        let next_child = b.edges[0];

                        self.stack[self.depth] = StackFrame {
                            kind: BranchKind::L3 {
                                ptr: b,
                                num,
                                digits: b.hdr.digits,
                                idx: 1,
                            },
                            level: bl,
                            prefix: branch_prefix,
                        };
                        self.depth += 1;

                        cur_edge = next_child;
                        cur_level = bl - 1;
                        cur_prefix = child_prefix;
                    } else {
                        // SAFETY: live BranchL7 pointer.
                        let b = unsafe { &*cur_edge.node_ptr().cast::<BranchL7>() };
                        let num = b.hdr.num;
                        if num == 0 {
                            return;
                        }
                        let d = b.hdr.digits[0];
                        let child_prefix =
                            branch_prefix | (u64::from(d) << (8 * u32::from(bl - 1)));
                        // SAFETY: slot 0 is valid for non-empty branch.
                        let next_child = b.edges[0];

                        self.stack[self.depth] = StackFrame {
                            kind: BranchKind::L7 {
                                ptr: b,
                                num,
                                digits: b.hdr.digits,
                                idx: 1,
                            },
                            level: bl,
                            prefix: branch_prefix,
                        };
                        self.depth += 1;

                        cur_edge = next_child;
                        cur_level = bl - 1;
                        cur_prefix = child_prefix;
                    }
                }

                EdgeTag::Structural(EdgeType::BranchB) => {
                    // SAFETY: live BranchB pointer.
                    let b = unsafe { &*cur_edge.node_ptr().cast::<BranchB>() };
                    let bl = b.level;
                    let dv = if bl < cur_level {
                        decode_value(&cur_edge, bl, cur_level)
                    } else {
                        0
                    };
                    let branch_prefix = if bl < cur_level {
                        cur_prefix | (dv << (8 * u32::from(bl)))
                    } else {
                        cur_prefix
                    };

                    let Some(first_d) = b.bitmap.next_set(0) else {
                        return;
                    };
                    let slot = b.bitmap.subexpanse_rank(first_d) as usize;
                    let sub = (first_d >> 5) as usize;
                    // SAFETY: first_d is set in bitmap → subarrays[sub] is non-null.
                    let next_child = unsafe { *b.subarrays[sub].add(slot) };
                    let child_prefix =
                        branch_prefix | (u64::from(first_d) << (8 * u32::from(bl - 1)));

                    self.stack[self.depth] = StackFrame {
                        kind: BranchKind::B {
                            ptr: b,
                            current_digit: u16::from(first_d) + 1,
                        },
                        level: bl,
                        prefix: branch_prefix,
                    };
                    self.depth += 1;

                    cur_edge = next_child;
                    cur_level = bl - 1;
                    cur_prefix = child_prefix;
                }

                EdgeTag::Structural(EdgeType::BranchU) => {
                    // SAFETY: live BranchU pointer.
                    let b = unsafe { &*cur_edge.node_ptr().cast::<BranchU>() };
                    let bl = cur_level;
                    let mut d = 0u16;
                    while d < 256 && b.edges[d as usize].is_null() {
                        d += 1;
                    }
                    if d == 256 {
                        return;
                    }
                    let next_child = b.edges[d as usize];
                    let child_prefix = cur_prefix | (u64::from(d) << (8 * u32::from(bl - 1)));

                    self.stack[self.depth] = StackFrame {
                        kind: BranchKind::U {
                            ptr: b,
                            current_digit: d + 1,
                        },
                        level: bl,
                        prefix: cur_prefix,
                    };
                    self.depth += 1;

                    cur_edge = next_child;
                    cur_level = bl - 1;
                    cur_prefix = child_prefix;
                }
            }
        }
    }

    /// Advances the iterator to the next leaf in tree order.
    ///
    /// # Safety
    /// Stack must contain valid node pointers per tree invariants.
    #[inline(always)]
    unsafe fn advance_leaf(&mut self) -> bool {
        while self.depth > 0 {
            let top = &mut self.stack[self.depth - 1];
            let bl = top.level;
            let branch_prefix = top.prefix;

            match &mut top.kind {
                BranchKind::L3 {
                    ptr,
                    num,
                    digits,
                    idx,
                } => {
                    if *idx < *num {
                        let slot = *idx as usize;
                        let d = digits[slot];
                        *idx += 1;
                        // SAFETY: ptr is valid BranchL3 and slot < num.
                        let child = unsafe { (*(*ptr)).edges[slot] };
                        let child_prefix =
                            branch_prefix | (u64::from(d) << (8 * u32::from(bl - 1)));
                        // SAFETY: child is live edge at bl - 1.
                        unsafe {
                            self.descend(&child, bl - 1, child_prefix);
                        }
                        return true;
                    }
                }
                BranchKind::L7 {
                    ptr,
                    num,
                    digits,
                    idx,
                } => {
                    if *idx < *num {
                        let slot = *idx as usize;
                        let d = digits[slot];
                        *idx += 1;
                        // SAFETY: ptr is valid BranchL7 and slot < num.
                        let child = unsafe { (*(*ptr)).edges[slot] };
                        let child_prefix =
                            branch_prefix | (u64::from(d) << (8 * u32::from(bl - 1)));
                        // SAFETY: child is live edge at bl - 1.
                        unsafe {
                            self.descend(&child, bl - 1, child_prefix);
                        }
                        return true;
                    }
                }
                BranchKind::B { ptr, current_digit } => {
                    if *current_digit <= 255 {
                        // SAFETY: ptr is valid BranchB.
                        let b = unsafe { &**ptr };
                        if let Some(next_d) = b.bitmap.next_set(*current_digit as u8) {
                            *current_digit = u16::from(next_d) + 1;
                            let slot = b.bitmap.subexpanse_rank(next_d) as usize;
                            let sub = (next_d >> 5) as usize;
                            // SAFETY: next_d is set in bitmap → subarrays[sub] is non-null.
                            let child = unsafe { *b.subarrays[sub].add(slot) };
                            let kb = single_immed_kb(&child);
                            if kb != 0 {
                                // A random expanse holds one key under most
                                // children: stream them as for `BranchU`.
                                let shift = 8 * (bl - 1);
                                self.leaf = LeafCursor::ImmedRunB {
                                    ptr: *ptr,
                                    // SAFETY: `slot` is a valid index of subarray `sub`.
                                    edge: unsafe { b.subarrays[sub].add(slot) },
                                    base: branch_prefix | (u64::from(next_d & 0xE0) << shift),
                                    mask: (1u64 << (8 * u32::from(kb))) - 1,
                                    bits: Self::subexpanse_word(&b.bitmap, sub as u8)
                                        & (u32::MAX << (next_d & 31)),
                                    inner_keys: core::ptr::null(),
                                    inner_values: core::ptr::null(),
                                    inner_base: 0,
                                    inner_left: 0,
                                    sub: sub as u8,
                                    shift,
                                    tag: child.tag_byte(),
                                };
                                return true;
                            }
                            let child_prefix =
                                branch_prefix | (u64::from(next_d) << (8 * u32::from(bl - 1)));
                            // SAFETY: child is live edge at bl - 1.
                            unsafe {
                                self.descend(&child, bl - 1, child_prefix);
                            }
                            return true;
                        }
                    }
                }
                BranchKind::U { ptr, current_digit } => {
                    // SAFETY: ptr is valid BranchU.
                    let b = unsafe { &**ptr };
                    while *current_digit < 256 {
                        let d = *current_digit;
                        *current_digit += 1;
                        let child = b.edges[d as usize];
                        if !child.is_null() {
                            let shift = 8 * u32::from(bl - 1);
                            let kb = single_immed_kb(&child);
                            if kb != 0 {
                                // Sparse expanses put one key under each child
                                // (#270). Stream the run of single-key siblings
                                // from the slots, so each key costs one slot
                                // read in `next` rather than a round trip
                                // through this walk. An immediate's key bytes
                                // equal its level (`validate.rs`), so every
                                // single-key sibling carries this child's tag.
                                let slots = b.edges.as_ptr_range();
                                self.leaf = LeafCursor::ImmedRunU {
                                    // SAFETY: `d < 256` indexes the slot array.
                                    edge: unsafe { slots.start.add(d as usize) },
                                    end: slots.end,
                                    key: branch_prefix | (u64::from(d) << shift),
                                    step: 1 << shift,
                                    mask: (1u64 << (8 * u32::from(kb))) - 1,
                                    tag: child.tag_byte(),
                                };
                                return true;
                            }
                            let child_prefix = branch_prefix | (u64::from(d) << shift);
                            // SAFETY: child is live edge at bl - 1.
                            unsafe {
                                self.descend(&child, bl - 1, child_prefix);
                            }
                            return true;
                        }
                    }
                }
            }

            self.depth -= 1;
        }

        false
    }

    /// Yields the next element in ascending key order.
    #[inline(always)]
    pub(crate) fn next(&mut self) -> Option<(Key, u64)> {
        'walk: loop {
            match &mut self.leaf {
                LeafCursor::Empty => return None,

                LeafCursor::RootLeaf {
                    keys,
                    values,
                    pop,
                    idx,
                } => {
                    if *idx < *pop {
                        let i = *idx as usize;
                        *idx += 1;
                        // SAFETY: i < pop per check.
                        let k = unsafe { *keys.add(i) };
                        let v = if MAP {
                            // SAFETY: values array holds at least pop entries.
                            unsafe { *values.add(i) }
                        } else {
                            0
                        };
                        return Some((k, v));
                    }
                    self.leaf = LeafCursor::Empty;
                    return None;
                }

                LeafCursor::Linear {
                    keys_ptr,
                    values_ptr,
                    kb,
                    pop,
                    idx,
                    prefix,
                } => {
                    if *idx < *pop {
                        let i = *idx as usize;
                        *idx += 1;
                        // SAFETY: i < pop and keys_ptr is valid for pop packed keys.
                        let low = unsafe { crate::mutate::read_packed(*keys_ptr, i, *kb as usize) };
                        let v = if MAP {
                            // SAFETY: values_ptr holds at least pop values.
                            unsafe { *values_ptr.add(i) }
                        } else {
                            0
                        };
                        return Some((*prefix | low, v));
                    }
                }

                LeafCursor::BitmapSet {
                    words,
                    word_idx,
                    prefix,
                } => {
                    while *word_idx < 4 {
                        let w = words[*word_idx as usize];
                        if w != 0 {
                            let bit = w.trailing_zeros();
                            words[*word_idx as usize] = w & (w - 1);
                            let d = (u64::from(*word_idx) << 6) | u64::from(bit);
                            return Some((*prefix | d, 0));
                        }
                        *word_idx += 1;
                    }
                }

                LeafCursor::BitmapMapAsc {
                    node,
                    value,
                    base,
                    bits,
                    sub,
                } => loop {
                    if *bits != 0 {
                        let d = bits.trailing_zeros();
                        *bits &= *bits - 1;
                        // SAFETY: digit `d` is set, so `value` addresses its slot.
                        let v = unsafe { **value };
                        // SAFETY: at most one past the subexpanse's value array.
                        *value = unsafe { value.add(1) };
                        return Some((*base | u64::from(d), v));
                    }
                    *sub += 1;
                    if *sub == 8 {
                        break;
                    }
                    // SAFETY: `node` is the live leaf this cursor walks.
                    let b = unsafe { &**node };
                    *bits = Self::subexpanse_word(&b.bitmap, *sub);
                    *value = b.values[*sub as usize];
                    *base = (*base & !0xE0) | (u64::from(*sub) << 5);
                },

                LeafCursor::ImmedSingle { key, value, done } => {
                    if !*done {
                        *done = true;
                        return Some((*key, *value));
                    }
                }

                LeafCursor::ImmedMulti {
                    keys,
                    values,
                    count,
                    idx,
                } => {
                    if *idx < *count {
                        let i = *idx as usize;
                        *idx += 1;
                        return Some((keys[i], values[i]));
                    }
                }

                LeafCursor::ImmedRunU {
                    edge,
                    end,
                    key,
                    step,
                    mask,
                    tag,
                    ..
                } => {
                    while *edge != *end {
                        // SAFETY: `edge` is below `end`, so it addresses a slot
                        // of the live `BranchU` on top of the stack.
                        let e = unsafe { &**edge };
                        let t = e.tag_byte();
                        if t == *tag {
                            let k = *key;
                            // SAFETY: at most `end`, one past the last slot.
                            *edge = unsafe { edge.add(1) };
                            *key = k.wrapping_add(*step);
                            return Some(if MAP {
                                (k | (e.aux_word() & *mask), e.word0())
                            } else {
                                (k | (e.word0() & *mask), 0)
                            });
                        }
                        if t != EdgeType::Null as u8 {
                            break;
                        }
                        // SAFETY: as above.
                        *edge = unsafe { edge.add(1) };
                        *key = key.wrapping_add(*step);
                    }
                    // The run ended at a child of another form, or at the end
                    // of the branch: resume the stack walk from that slot.
                    // SAFETY: `edge` and `end` address the same slot array.
                    let d = 256 - unsafe { end.offset_from(*edge) } as u16;
                    if let BranchKind::U { current_digit, .. } =
                        &mut self.stack[self.depth - 1].kind
                    {
                        *current_digit = d;
                    }
                }

                LeafCursor::ImmedRunB {
                    ptr,
                    edge,
                    base,
                    mask,
                    inner_keys,
                    inner_values,
                    inner_base,
                    bits,
                    inner_left,
                    sub,
                    shift,
                    tag,
                } => {
                    if *inner_left != 0 {
                        *inner_left -= 1;
                        let kb = usize::from(*shift / 8);
                        // SAFETY: `inner_left` keys of `kb` bytes remain at
                        // `inner_keys`, and as many values at `inner_values`.
                        let low = unsafe { crate::mutate::read_packed(*inner_keys, 0, kb) };
                        // SAFETY: at most one past the leaf's last key.
                        *inner_keys = unsafe { inner_keys.add(kb) };
                        let v = if MAP {
                            // SAFETY: as above, for the value array.
                            let v = unsafe { **inner_values };
                            // SAFETY: at most one past the leaf's last value.
                            *inner_values = unsafe { inner_values.add(1) };
                            v
                        } else {
                            0
                        };
                        return Some((*inner_base | low, v));
                    }
                    let resume = loop {
                        if *bits != 0 {
                            let bit = bits.trailing_zeros();
                            // SAFETY: `bits` is non-zero, so `edge` addresses
                            // the subarray slot of its lowest digit.
                            let e = unsafe { &**edge };
                            *bits &= *bits - 1;
                            // SAFETY: at most one past the subarray's last slot.
                            *edge = unsafe { edge.add(1) };
                            let k = *base | (u64::from(bit) << *shift);
                            let t = e.tag_byte();
                            if t == *tag {
                                return Some(if MAP {
                                    (k | (e.aux_word() & *mask), e.word0())
                                } else {
                                    (k | (e.word0() & *mask), 0)
                                });
                            }
                            let level = *shift / 8;
                            if t == EdgeType::Leaf1 as u8 + level - 1 {
                                // A linear leaf whose key bytes equal its level
                                // skips no digit: read its keys from inside
                                // the run, which keeps the run's place, instead
                                // of a descent and a fresh run after it. On
                                // random keys these are the two- and three-key
                                // children (#1687).
                                let pop = e.pop0(level) as usize + 1;
                                let node = e.node_ptr();
                                *inner_base = k;
                                *inner_left = pop as u16;
                                if MAP {
                                    // SAFETY: map leaf layout: `pop` values, then
                                    // the class-sized key array.
                                    *inner_keys = unsafe { node.add(leaf::map_keys_offset(pop)) };
                                    *inner_values = node.cast::<u64>();
                                } else {
                                    *inner_keys = node;
                                }
                                continue 'walk;
                            }
                            // A child of another form: descend into it from
                            // here, past it in both this cursor and the frame,
                            // rather than have the stack walk find it again in
                            // the bitmap.
                            let child = *e;
                            let d = ((u32::from(*sub) << 5) | bit) as u16;
                            if let BranchKind::B { current_digit, .. } =
                                &mut self.stack[self.depth - 1].kind
                            {
                                *current_digit = d + 1;
                            }
                            // SAFETY: `child` is a live edge at `level`.
                            unsafe { self.descend(&child, level, k) };
                            continue 'walk;
                        }
                        *sub += 1;
                        if *sub == 8 {
                            break 256;
                        }
                        // SAFETY: `ptr` is the live `BranchB` on top of the stack.
                        let b = unsafe { &**ptr };
                        *bits = Self::subexpanse_word(&b.bitmap, *sub);
                        *edge = b.subarrays[*sub as usize];
                        *base = (*base & !(0xE0u64 << *shift)) | (u64::from(*sub) << (*shift + 5));
                    };
                    // As for `ImmedRunU`: resume the walk at that digit.
                    if let BranchKind::B { current_digit, .. } =
                        &mut self.stack[self.depth - 1].kind
                    {
                        *current_digit = resume;
                    }
                }

                LeafCursor::FullExpanse { next_key, max_key } => {
                    if *next_key <= *max_key {
                        let k = *next_key;
                        *next_key += 1;
                        return Some((k, 0));
                    }
                }
            }

            // Current leaf exhausted; ascend to next leaf in tree walk.
            // SAFETY: tree invariant holds.
            let has_more = unsafe { self.advance_leaf() };
            if !has_more {
                self.leaf = LeafCursor::Empty;
                return None;
            }
        }
    }

    /// Reads the little-endian `u32` subexpanse word `sub` (0..8) of a 256-bit
    /// bitmap — the 32-digit value/child group for bitmap map-leaves.
    #[inline(always)]
    fn subexpanse_word(bitmap: &Bitmap256, sub: u8) -> u32 {
        debug_assert!(sub < 8);
        // SAFETY: the 32-byte, 8-aligned `words` hold 8 `u32` subwords, read
        // in place as `Bitmap256::subexpanse_rank` reads them.
        unsafe { *bitmap.words.as_ptr().cast::<u32>().add(sub as usize) }
    }
}

#[cfg(test)]
mod tests {
    use crate::map::ExpanseMap;
    use crate::set::ExpanseSet;
    use crate::types::Key;
    use std::collections::BTreeMap;

    fn value_of(k: Key) -> u64 {
        k.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x5555
    }

    /// Builds both containers from `keys` and checks that a full `iter()`
    /// of each yields the reference order (and the map its values), and
    /// that stopping part-way leaves no gap or repeat in what was read.
    fn check(keys: &[Key]) -> (ExpanseSet, ExpanseMap) {
        let mut set = ExpanseSet::new();
        let mut map = ExpanseMap::new();
        let mut reference = BTreeMap::new();
        for &k in keys {
            set.insert(k);
            map.insert(k, value_of(k));
            reference.insert(k, value_of(k));
        }
        let want: Vec<(Key, u64)> = reference.into_iter().collect();
        let got_map: Vec<(Key, u64)> = map.iter().collect();
        assert_eq!(got_map, want, "map iter");
        let got_set: Vec<Key> = set.iter().collect();
        let want_keys: Vec<Key> = want.iter().map(|&(k, _)| k).collect();
        assert_eq!(got_set, want_keys, "set iter");
        for cut in [0, 1, want.len() / 3, want.len().saturating_sub(1)] {
            let head: Vec<Key> = set.iter().take(cut).collect();
            assert_eq!(
                head,
                want_keys[..cut.min(want_keys.len())],
                "set prefix {cut}"
            );
        }
        (set, map)
    }

    fn seeded(seed: u64) -> impl FnMut() -> u64 {
        let mut s = seed;
        move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        }
    }

    /// Keys under another top digit, enough to move any small shape out of
    /// the root leaf and into a trie.
    fn filler() -> impl Iterator<Item = Key> {
        (0..300u64).map(|i| (0x10 << 56) | (i << 20))
    }

    #[test]
    fn empty_and_root_leaf() {
        check(&[]);
        check(&[7]);
        check(&[3, 1, 2, u64::MAX, 0]);
    }

    #[test]
    fn immediate_runs_broken_by_other_children() {
        // Runs of single-key immediates under a dense branch (`BranchU`) and a
        // sparse one (`BranchB`), broken by children of other forms: two- and
        // three-key children that skip no level (which a `BranchB` run reads
        // in place) and children that skip levels (which it descends into).
        for (count, stride, dense) in [(220u64, 1u64, true), (40, 6, false), (9, 28, false)] {
            let mut keys = Vec::new();
            for i in 0..count {
                let d = (0x5Au64 << 56) | ((i * stride) << 48);
                keys.push(d | (0x12 << 40) | 0x1234);
                if i % 7 == 3 {
                    keys.extend([d | (0x34 << 40) | 0x5678, d | (0x56 << 40) | 0x9999]);
                }
                if i % 11 == 5 {
                    keys.push(d | (0x12 << 40) | 0x5678);
                }
            }
            keys.extend(filler());
            let (_, map) = check(&keys);
            let counts = map.stats().node_counts;
            assert!(
                if dense {
                    counts.branch_u > 0
                } else {
                    counts.branch_b > 0
                },
                "{counts:?}"
            );
        }
    }

    #[test]
    fn runs_reaching_the_end_of_a_branch() {
        // A run whose last single-key child is the branch's last digit, and
        // one that ends on a multi-key child there, at the top level and one
        // level down, so the write-back resumes past the branch's end.
        for last_multi in [false, true] {
            for top in [0u64, 0xFF] {
                let mut keys = Vec::new();
                for d in (0..256u64).step_by(3).chain([255]) {
                    keys.push((top << 56) | (d << 48) | 0x77);
                }
                if last_multi {
                    keys.push((top << 56) | (255 << 48) | (0x10 << 40) | 0x78);
                }
                keys.push(0x0123_4567_89AB_CDEF);
                check(&keys);
            }
        }
    }

    #[test]
    fn sibling_runs_under_a_sparse_branch_across_subexpanses() {
        // A `BranchB` whose single-key children fill several 32-digit
        // subexpanses, with an in-place linear child at a subexpanse's last
        // digit and at the branch's last digit.
        let mut keys = Vec::new();
        for d in [0u64, 5, 31, 32, 33, 63, 64, 100, 127, 200, 223, 224, 255] {
            keys.push((0x42 << 56) | (d << 48) | 0x0101);
        }
        for d in [31u64, 255] {
            keys.push((0x42 << 56) | (d << 48) | (0x20 << 40) | 0x0202);
            keys.push((0x42 << 56) | (d << 48) | (0x30 << 40) | 0x0303);
        }
        keys.extend(filler());
        let (_, map) = check(&keys);
        assert!(map.stats().node_counts.branch_b > 0);
    }

    #[test]
    fn distributions() {
        let mut rng = seeded(0xC0FF_EE11);
        let random: Vec<Key> = (0..10_000).map(|_| rng()).collect();
        check(&random);
        let sparse: Vec<Key> = (0..4_000u64).map(|i| i << 40).collect();
        check(&sparse);
        let sequential: Vec<Key> = (0..20_000u64).collect();
        check(&sequential);
        let clustered: Vec<Key> = (0..64u64)
            .flat_map(|c| (0..300u64).map(move |i| (c << 32) | (i * 3)))
            .collect();
        check(&clustered);
        // Map bitmap leaves with gaps in every subexpanse, and one with a
        // lone digit in the last subexpanse.
        let gappy: Vec<Key> = (0..4096u64)
            .filter(|i| i % 5 != 0)
            .chain([0x1_00FF])
            .collect();
        check(&gappy);
    }
}
