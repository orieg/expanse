//! High-performance stack-based in-order iterator for Expanse tries.
//!
//! Replaces per-element `next_at_or_after` re-descents with an amortized
//! stack walk, streaming contiguous leaf elements in O(1) time per key.

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
    BitmapMap {
        bitmap: Bitmap256,
        values: [*mut u64; 8],
        sub: u8,
        current_word: u32,
        rank: u16,
        prefix: u64,
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

/// A zero-allocation, stack-based in-order trie iterator.
pub struct RawIter<const MAP: bool> {
    leaf: LeafCursor,
    stack: [StackFrame; 8],
    depth: usize,
}

impl<const MAP: bool> Default for RawIter<MAP> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const MAP: bool> RawIter<MAP> {
    /// Creates an empty iterator.
    #[must_use]
    pub const fn new() -> Self {
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
    pub fn from_root_leaf(keys: *const u64, values: *const u64, pop: usize) -> Self {
        let mut iter = Self::new();
        iter.leaf = LeafCursor::RootLeaf {
            keys,
            values,
            pop: pop as u32,
            idx: 0,
        };
        iter
    }

    /// Initializes iteration from a root leaf starting at `start_key`.
    ///
    /// # Safety
    /// `keys` and `values` must point to live, initialized allocations of at least `pop` entries.
    pub unsafe fn from_root_leaf_range(
        keys: *const u64,
        values: *const u64,
        pop: usize,
        start_key: u64,
    ) -> Self {
        let mut iter = Self::new();
        // SAFETY: root leaf contains `pop` sorted u64 keys per contract.
        let slice = unsafe { core::slice::from_raw_parts(keys, pop) };
        let idx = slice.partition_point(|&k| k < start_key);
        iter.leaf = LeafCursor::RootLeaf {
            keys,
            values,
            pop: pop as u32,
            idx: idx as u32,
        };
        iter
    }

    /// Initializes iteration from a root edge.
    ///
    /// # Safety
    /// `top` must point to a live, well-formed trie edge.
    pub unsafe fn from_tree(top: &Edge) -> Self {
        let mut iter = Self::new();
        // SAFETY: top points to a live trie per contract.
        unsafe {
            iter.descend(top, 8, 0);
        }
        iter
    }

    /// Initializes iteration from a root edge starting at `start_key`.
    ///
    /// # Safety
    /// `top` must point to a live, well-formed trie edge.
    pub unsafe fn from_tree_range(top: &Edge, start_key: u64) -> Self {
        let mut iter = Self::new();
        // SAFETY: top points to a live trie per contract.
        unsafe {
            iter.descend_seek(top, 8, 0, start_key);
            if matches!(iter.leaf, LeafCursor::Empty) {
                iter.advance_leaf();
            }
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
                        if n == 1 {
                            v[0] = u64::from_le_bytes(cur_edge.imm_bytes());
                        } else {
                            let v_ptr = cur_edge.node_ptr().cast::<u64>();
                            for (i, val) in v.iter_mut().enumerate().take(n as usize) {
                                // SAFETY: live value array for n entries.
                                *val = unsafe { *v_ptr.add(i) };
                            }
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
                        let current_word = (b.bitmap.words[0] & 0xFFFF_FFFF) as u32;
                        self.leaf = LeafCursor::BitmapMap {
                            bitmap: b.bitmap,
                            values: b.values,
                            sub: 0,
                            current_word,
                            rank: 0,
                            prefix: leaf_prefix,
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

    /// Descends from `edge` targeting the smallest key `>= start`, pushing branches onto `stack`.
    ///
    /// # Safety
    /// `edge` must point to a live node / leaf at `level`.
    #[inline(always)]
    unsafe fn descend_seek(&mut self, edge: &Edge, level: u8, prefix: u64, start: u64) {
        let mut cur_edge = *edge;
        let mut cur_level = level;
        let mut cur_prefix = prefix;

        loop {
            if cur_edge.is_null() {
                self.leaf = LeafCursor::Empty;
                return;
            }

            let tag = match cur_edge.tag() {
                Some(t) => t,
                None => {
                    self.leaf = LeafCursor::Empty;
                    return;
                }
            };

            match tag {
                EdgeTag::Structural(EdgeType::Null) => {
                    self.leaf = LeafCursor::Empty;
                    return;
                }

                EdgeTag::Immed(im) => {
                    let n = im.key_count();
                    if n == 1 {
                        let kb = im.key_bytes() as usize;
                        let (low, value) = if MAP {
                            let aux = cur_edge.aux_bytes();
                            let mut kbuf = [0u8; 8];
                            kbuf[..kb].copy_from_slice(&aux[..kb]);
                            (
                                u64::from_le_bytes(kbuf),
                                u64::from_le_bytes(cur_edge.imm_bytes()),
                            )
                        } else {
                            let w0 = cur_edge.imm_bytes();
                            let mut kbuf = [0u8; 8];
                            kbuf[..kb].copy_from_slice(&w0[..kb]);
                            (u64::from_le_bytes(kbuf), 0)
                        };
                        let full_k = cur_prefix | low;
                        if full_k >= start {
                            self.leaf = LeafCursor::ImmedSingle {
                                key: full_k,
                                value,
                                done: false,
                            };
                        } else {
                            self.leaf = LeafCursor::Empty;
                        }
                        return;
                    }
                    let (keys_buf, vals_buf) = if MAP {
                        let k = crate::mutate::immed_map_keys(&cur_edge, im);
                        let mut v = [0u64; 15];
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
                    let mut first_idx = None;
                    for i in 0..n as usize {
                        let full_k = cur_prefix | keys_buf[i];
                        keys[i] = full_k;
                        if first_idx.is_none() && full_k >= start {
                            first_idx = Some(i as u8);
                        }
                    }
                    if let Some(idx) = first_idx {
                        self.leaf = LeafCursor::ImmedMulti {
                            keys,
                            values: vals_buf,
                            count: n,
                            idx,
                        };
                    } else {
                        self.leaf = LeafCursor::Empty;
                    }
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

                    let shift = 8 * u32::from(kb);
                    let start_high = if shift >= 64 { 0 } else { start >> shift };
                    let prefix_high = if shift >= 64 { 0 } else { leaf_prefix >> shift };

                    if start_high > prefix_high {
                        self.leaf = LeafCursor::Empty;
                        return;
                    }

                    let idx = if start_high < prefix_high {
                        0u16
                    } else {
                        let start_low = crate::mutate::key_low(start, kb);
                        // SAFETY: keys_ptr points to pop packed keys in live leaf.
                        let at = unsafe {
                            leaf::locate(keys_ptr, pop as usize, kb, start_low)
                                .unwrap_or_else(|insert_idx| insert_idx)
                        };
                        if at >= pop as usize {
                            self.leaf = LeafCursor::Empty;
                            return;
                        }
                        at as u16
                    };

                    self.leaf = LeafCursor::Linear {
                        keys_ptr,
                        values_ptr,
                        kb,
                        pop,
                        idx,
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

                    let start_high = start >> 8;
                    let prefix_high = leaf_prefix >> 8;

                    if start_high > prefix_high {
                        self.leaf = LeafCursor::Empty;
                        return;
                    }

                    let target_d = if start_high < prefix_high {
                        0u8
                    } else {
                        (start & 0xFF) as u8
                    };

                    if MAP {
                        // SAFETY: live LeafBitmapL node pointer.
                        let b = unsafe { &*cur_edge.node_ptr().cast::<LeafBitmapL>() };
                        if let Some(first_d) = b.bitmap.next_set(target_d) {
                            let sub = first_d >> 5;
                            let rank = b.bitmap.subexpanse_rank(first_d) as u16;
                            let sub_word = (b.bitmap.words[sub as usize / 2]
                                >> ((sub as usize % 2) * 32))
                                as u32;
                            let bit_in_sub = (first_d & 31) as u32;
                            let current_word = sub_word & (!0u32 << bit_in_sub);
                            self.leaf = LeafCursor::BitmapMap {
                                bitmap: b.bitmap,
                                values: b.values,
                                sub,
                                current_word,
                                rank,
                                prefix: leaf_prefix,
                            };
                        } else {
                            self.leaf = LeafCursor::Empty;
                        }
                    } else {
                        // SAFETY: live LeafBitmap1 node pointer.
                        let b = unsafe { &*cur_edge.node_ptr().cast::<LeafBitmap1>() };
                        if let Some(first_d) = b.bitmap.next_set(target_d) {
                            let word_idx = first_d >> 6;
                            let mut words = b.bitmap.words;
                            for w in words.iter_mut().take(word_idx as usize) {
                                *w = 0;
                            }
                            let bit_in_word = (first_d & 63) as u32;
                            words[word_idx as usize] &= !0u64 << bit_in_word;
                            self.leaf = LeafCursor::BitmapSet {
                                words,
                                word_idx,
                                prefix: leaf_prefix,
                            };
                        } else {
                            self.leaf = LeafCursor::Empty;
                        }
                    }
                    return;
                }

                EdgeTag::Structural(EdgeType::FullExpanse) => {
                    let max = if cur_level == 8 {
                        u64::MAX
                    } else {
                        (1u64 << (8 * u32::from(cur_level))) - 1
                    };
                    let max_key = cur_prefix | max;
                    let next_key = start.max(cur_prefix);
                    if next_key <= max_key {
                        self.leaf = LeafCursor::FullExpanse { next_key, max_key };
                    } else {
                        self.leaf = LeafCursor::Empty;
                    }
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

                    let shift = 8 * u32::from(bl);
                    let start_high = if shift >= 64 { 0 } else { start >> shift };
                    let prefix_high = if shift >= 64 {
                        0
                    } else {
                        branch_prefix >> shift
                    };

                    if start_high > prefix_high {
                        self.leaf = LeafCursor::Empty;
                        return;
                    }

                    let (num, digits, edges_ptr, ptr_raw) = if matches!(t, EdgeType::BranchL3) {
                        // SAFETY: cur_edge is a live BranchL3 pointer.
                        let b = unsafe { &*cur_edge.node_ptr().cast::<BranchL3>() };
                        (
                            b.hdr.num,
                            b.hdr.digits,
                            b.edges.as_ptr(),
                            b as *const BranchL3 as *const (),
                        )
                    } else {
                        // SAFETY: cur_edge is a live BranchL7 pointer.
                        let b = unsafe { &*cur_edge.node_ptr().cast::<BranchL7>() };
                        (
                            b.hdr.num,
                            b.hdr.digits,
                            b.edges.as_ptr(),
                            b as *const BranchL7 as *const (),
                        )
                    };

                    if num == 0 {
                        self.leaf = LeafCursor::Empty;
                        return;
                    }

                    if start_high < prefix_high {
                        let d = digits[0];
                        let child_prefix =
                            branch_prefix | (u64::from(d) << (8 * u32::from(bl - 1)));
                        // SAFETY: slot 0 is valid for non-empty branch.
                        let next_child = unsafe { *edges_ptr };

                        if matches!(t, EdgeType::BranchL3) {
                            self.stack[self.depth] = StackFrame {
                                kind: BranchKind::L3 {
                                    ptr: ptr_raw.cast(),
                                    num,
                                    digits,
                                    idx: 1,
                                },
                                level: bl,
                                prefix: branch_prefix,
                            };
                        } else {
                            self.stack[self.depth] = StackFrame {
                                kind: BranchKind::L7 {
                                    ptr: ptr_raw.cast(),
                                    num,
                                    digits,
                                    idx: 1,
                                },
                                level: bl,
                                prefix: branch_prefix,
                            };
                        }
                        self.depth += 1;

                        cur_edge = next_child;
                        cur_level = bl - 1;
                        cur_prefix = child_prefix;
                    } else {
                        let target_d = crate::types::digit(start, bl);
                        let mut match_slot = None;
                        for (s, &d) in digits.iter().enumerate().take(num as usize) {
                            if d >= target_d {
                                match_slot = Some(s);
                                break;
                            }
                        }

                        let Some(slot) = match_slot else {
                            self.leaf = LeafCursor::Empty;
                            return;
                        };

                        let d = digits[slot];
                        let child_prefix =
                            branch_prefix | (u64::from(d) << (8 * u32::from(bl - 1)));
                        // SAFETY: slot < num and edges_ptr has num entries.
                        let next_child = unsafe { *edges_ptr.add(slot) };

                        if matches!(t, EdgeType::BranchL3) {
                            self.stack[self.depth] = StackFrame {
                                kind: BranchKind::L3 {
                                    ptr: ptr_raw.cast(),
                                    num,
                                    digits,
                                    idx: (slot + 1) as u8,
                                },
                                level: bl,
                                prefix: branch_prefix,
                            };
                        } else {
                            self.stack[self.depth] = StackFrame {
                                kind: BranchKind::L7 {
                                    ptr: ptr_raw.cast(),
                                    num,
                                    digits,
                                    idx: (slot + 1) as u8,
                                },
                                level: bl,
                                prefix: branch_prefix,
                            };
                        }
                        self.depth += 1;

                        if d == target_d {
                            cur_edge = next_child;
                            cur_level = bl - 1;
                            cur_prefix = child_prefix;
                        } else {
                            // First child >= target_d is strictly greater than target_d; descend leftmost
                            // SAFETY: next_child is a live edge at bl - 1.
                            unsafe {
                                self.descend(&next_child, bl - 1, child_prefix);
                            }
                            return;
                        }
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

                    let shift = 8 * u32::from(bl);
                    let start_high = if shift >= 64 { 0 } else { start >> shift };
                    let prefix_high = if shift >= 64 {
                        0
                    } else {
                        branch_prefix >> shift
                    };

                    if start_high > prefix_high {
                        self.leaf = LeafCursor::Empty;
                        return;
                    }

                    let target_d = if start_high < prefix_high {
                        0u8
                    } else {
                        crate::types::digit(start, bl)
                    };

                    let Some(first_d) = b.bitmap.next_set(target_d) else {
                        self.leaf = LeafCursor::Empty;
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

                    if first_d == target_d && start_high == prefix_high {
                        cur_edge = next_child;
                        cur_level = bl - 1;
                        cur_prefix = child_prefix;
                    } else {
                        // Descend leftmost into next_child
                        // SAFETY: next_child is a live edge at bl - 1.
                        unsafe {
                            self.descend(&next_child, bl - 1, child_prefix);
                        }
                        return;
                    }
                }

                EdgeTag::Structural(EdgeType::BranchU) => {
                    // SAFETY: live BranchU pointer.
                    let b = unsafe { &*cur_edge.node_ptr().cast::<BranchU>() };
                    let bl = cur_level;
                    let shift = 8 * u32::from(bl);
                    let start_high = if shift >= 64 { 0 } else { start >> shift };
                    let prefix_high = if shift >= 64 { 0 } else { cur_prefix >> shift };

                    if start_high > prefix_high {
                        self.leaf = LeafCursor::Empty;
                        return;
                    }

                    let target_d = if start_high < prefix_high {
                        0u16
                    } else {
                        u16::from(crate::types::digit(start, bl))
                    };

                    let mut d = target_d;
                    while d < 256 && b.edges[d as usize].is_null() {
                        d += 1;
                    }
                    if d == 256 {
                        self.leaf = LeafCursor::Empty;
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

                    if d == target_d && start_high == prefix_high {
                        cur_edge = next_child;
                        cur_level = bl - 1;
                        cur_prefix = child_prefix;
                    } else {
                        // Descend leftmost into next_child
                        // SAFETY: next_child is a live edge at bl - 1.
                        unsafe {
                            self.descend(&next_child, bl - 1, child_prefix);
                        }
                        return;
                    }
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
                            let child_prefix =
                                branch_prefix | (u64::from(d) << (8 * u32::from(bl - 1)));
                            if let Some(EdgeTag::Immed(im)) = child.tag()
                                && im.key_count() == 1
                            {
                                let (low, value) = unpack_immed_single::<MAP>(&child, im);
                                self.leaf = LeafCursor::ImmedSingle {
                                    key: child_prefix | low,
                                    value,
                                    done: false,
                                };
                                return true;
                            }
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
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<(Key, u64)> {
        loop {
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

                LeafCursor::BitmapMap {
                    bitmap,
                    values,
                    sub,
                    current_word,
                    rank,
                    prefix,
                } => {
                    while *sub < 8 {
                        if *current_word != 0 {
                            let bit = current_word.trailing_zeros();
                            *current_word &= *current_word - 1;
                            let d = (u64::from(*sub) << 5) | u64::from(bit);
                            // SAFETY: bit is set in subexpanse → value array has entry at rank.
                            let v = unsafe { *values[*sub as usize].add(*rank as usize) };
                            *rank += 1;
                            return Some((*prefix | d, v));
                        }
                        *sub += 1;
                        if *sub < 8 {
                            *current_word = (bitmap.words[*sub as usize / 2]
                                >> ((*sub as usize % 2) * 32))
                                as u32;
                            *rank = 0;
                        }
                    }
                }

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

    // --- Reverse (descending) traversal ---------------------------------
    //
    // A `RawIter` instance is mono-directional: the reverse constructors
    // seed the leaf cursor at the *high* end and the stack frames record the
    // *previous* child to descend into (going down), so `next_back` streams
    // keys in descending order exactly as `next` streams them ascending. The
    // forward and reverse machinery never share one instance; the public
    // double-ended wrappers hold one cursor per direction (see map.rs/set.rs).

    /// Reads the little-endian `u32` subexpanse word `sub` (0..8) of a 256-bit
    /// bitmap — the 32-digit value/child group for bitmap map-leaves.
    #[inline(always)]
    fn subexpanse_word(bitmap: &Bitmap256, sub: u8) -> u32 {
        (bitmap.words[sub as usize / 2] >> ((sub as usize % 2) * 32)) as u32
    }

    /// Initializes reverse iteration from a root leaf (descending).
    pub fn from_root_leaf_rev(keys: *const u64, values: *const u64, pop: usize) -> Self {
        let mut iter = Self::new();
        iter.leaf = if pop == 0 {
            LeafCursor::Empty
        } else {
            LeafCursor::RootLeaf {
                keys,
                values,
                pop: pop as u32,
                idx: pop as u32,
            }
        };
        iter
    }

    /// Initializes reverse iteration from a root leaf yielding keys `<= end`.
    ///
    /// # Safety
    /// `keys` and `values` must point to live, initialized allocations of at least `pop` entries.
    pub unsafe fn from_root_leaf_range_rev(
        keys: *const u64,
        values: *const u64,
        pop: usize,
        end: u64,
    ) -> Self {
        let mut iter = Self::new();
        // SAFETY: root leaf contains `pop` sorted u64 keys per contract.
        let slice = unsafe { core::slice::from_raw_parts(keys, pop) };
        let idx = slice.partition_point(|&k| k <= end);
        iter.leaf = if idx == 0 {
            LeafCursor::Empty
        } else {
            LeafCursor::RootLeaf {
                keys,
                values,
                pop: pop as u32,
                idx: idx as u32,
            }
        };
        iter
    }

    /// Initializes reverse iteration from a root edge (descending).
    ///
    /// # Safety
    /// `top` must point to a live, well-formed trie edge.
    pub unsafe fn from_tree_rev(top: &Edge) -> Self {
        let mut iter = Self::new();
        // SAFETY: top points to a live trie per contract.
        unsafe {
            iter.descend_max(top, 8, 0);
        }
        iter
    }

    /// Initializes reverse iteration from a root edge yielding keys `<= end`.
    ///
    /// # Safety
    /// `top` must point to a live, well-formed trie edge.
    pub unsafe fn from_tree_range_rev(top: &Edge, end: u64) -> Self {
        let mut iter = Self::new();
        // SAFETY: top points to a live trie per contract.
        unsafe {
            iter.descend_seek_back(top, 8, 0, end);
            if matches!(iter.leaf, LeafCursor::Empty) {
                iter.retreat_leaf();
            }
        }
        iter
    }

    /// Descends from `edge` into the rightmost leaf, seeding reverse cursors.
    ///
    /// # Safety
    /// `edge` must point to a live node / leaf at `level`.
    #[inline(always)]
    unsafe fn descend_max(&mut self, edge: &Edge, level: u8, prefix: u64) {
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
                        idx: n,
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
                        idx: pop,
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
                        let sub = 7u8;
                        let current_word = Self::subexpanse_word(&b.bitmap, sub);
                        let rank = (current_word.count_ones() as u16).wrapping_sub(1);
                        self.leaf = LeafCursor::BitmapMap {
                            bitmap: b.bitmap,
                            values: b.values,
                            sub,
                            current_word,
                            rank,
                            prefix: leaf_prefix,
                        };
                    } else {
                        // SAFETY: live LeafBitmap1 node pointer.
                        let b = unsafe { &*cur_edge.node_ptr().cast::<LeafBitmap1>() };
                        self.leaf = LeafCursor::BitmapSet {
                            words: b.bitmap.words,
                            word_idx: 3,
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
                        let slot = (num - 1) as usize;
                        let d = b.hdr.digits[slot];
                        let child_prefix =
                            branch_prefix | (u64::from(d) << (8 * u32::from(bl - 1)));
                        let next_child = b.edges[slot];

                        self.stack[self.depth] = StackFrame {
                            kind: BranchKind::L3 {
                                ptr: b,
                                num,
                                digits: b.hdr.digits,
                                idx: slot as u8,
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
                        let slot = (num - 1) as usize;
                        let d = b.hdr.digits[slot];
                        let child_prefix =
                            branch_prefix | (u64::from(d) << (8 * u32::from(bl - 1)));
                        let next_child = b.edges[slot];

                        self.stack[self.depth] = StackFrame {
                            kind: BranchKind::L7 {
                                ptr: b,
                                num,
                                digits: b.hdr.digits,
                                idx: slot as u8,
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

                    let Some(last_d) = b.bitmap.prev_set(255) else {
                        return;
                    };
                    let slot = b.bitmap.subexpanse_rank(last_d) as usize;
                    let sub = (last_d >> 5) as usize;
                    // SAFETY: last_d is set in bitmap → subarrays[sub] is non-null.
                    let next_child = unsafe { *b.subarrays[sub].add(slot) };
                    let child_prefix =
                        branch_prefix | (u64::from(last_d) << (8 * u32::from(bl - 1)));

                    self.stack[self.depth] = StackFrame {
                        kind: BranchKind::B {
                            ptr: b,
                            current_digit: u16::from(last_d),
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
                    let mut d = 255i32;
                    while d >= 0 && b.edges[d as usize].is_null() {
                        d -= 1;
                    }
                    if d < 0 {
                        return;
                    }
                    let d = d as usize;
                    let next_child = b.edges[d];
                    let child_prefix = cur_prefix | (u64::from(d as u8) << (8 * u32::from(bl - 1)));

                    self.stack[self.depth] = StackFrame {
                        kind: BranchKind::U {
                            ptr: b,
                            current_digit: d as u16,
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

    /// Descends from `edge` targeting the largest key `<= end`, seeding reverse cursors.
    ///
    /// # Safety
    /// `edge` must point to a live node / leaf at `level`.
    #[inline(always)]
    unsafe fn descend_seek_back(&mut self, edge: &Edge, level: u8, prefix: u64, end: u64) {
        let mut cur_edge = *edge;
        let mut cur_level = level;
        let mut cur_prefix = prefix;

        loop {
            if cur_edge.is_null() {
                self.leaf = LeafCursor::Empty;
                return;
            }

            let tag = match cur_edge.tag() {
                Some(t) => t,
                None => {
                    self.leaf = LeafCursor::Empty;
                    return;
                }
            };

            match tag {
                EdgeTag::Structural(EdgeType::Null) => {
                    self.leaf = LeafCursor::Empty;
                    return;
                }

                EdgeTag::Immed(im) => {
                    let n = im.key_count();
                    if n == 1 {
                        let (low, value) = unpack_immed_single::<MAP>(&cur_edge, im);
                        let full_k = cur_prefix | low;
                        if full_k <= end {
                            self.leaf = LeafCursor::ImmedSingle {
                                key: full_k,
                                value,
                                done: false,
                            };
                        } else {
                            self.leaf = LeafCursor::Empty;
                        }
                        return;
                    }
                    let (keys_buf, vals_buf) = if MAP {
                        let k = crate::mutate::immed_map_keys(&cur_edge, im);
                        let mut v = [0u64; 15];
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
                    let mut cnt = 0u8;
                    for i in 0..n as usize {
                        let full_k = cur_prefix | keys_buf[i];
                        keys[i] = full_k;
                        if full_k <= end {
                            cnt = (i + 1) as u8;
                        }
                    }
                    if cnt > 0 {
                        self.leaf = LeafCursor::ImmedMulti {
                            keys,
                            values: vals_buf,
                            count: n,
                            idx: cnt,
                        };
                    } else {
                        self.leaf = LeafCursor::Empty;
                    }
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

                    let shift = 8 * u32::from(kb);
                    let end_high = if shift >= 64 { 0 } else { end >> shift };
                    let prefix_high = if shift >= 64 { 0 } else { leaf_prefix >> shift };

                    if end_high < prefix_high {
                        self.leaf = LeafCursor::Empty;
                        return;
                    }

                    let idx = if end_high > prefix_high {
                        pop
                    } else {
                        let end_low = crate::mutate::key_low(end, kb);
                        // SAFETY: keys_ptr points to pop packed keys in live leaf.
                        let cnt = unsafe {
                            match leaf::locate(keys_ptr, pop as usize, kb, end_low) {
                                Ok(i) => i + 1,
                                Err(i) => i,
                            }
                        };
                        if cnt == 0 {
                            self.leaf = LeafCursor::Empty;
                            return;
                        }
                        cnt as u16
                    };

                    self.leaf = LeafCursor::Linear {
                        keys_ptr,
                        values_ptr,
                        kb,
                        pop,
                        idx,
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

                    let end_high = end >> 8;
                    let prefix_high = leaf_prefix >> 8;

                    if end_high < prefix_high {
                        self.leaf = LeafCursor::Empty;
                        return;
                    }

                    let target_d = if end_high > prefix_high {
                        255u8
                    } else {
                        (end & 0xFF) as u8
                    };

                    if MAP {
                        // SAFETY: live LeafBitmapL node pointer.
                        let b = unsafe { &*cur_edge.node_ptr().cast::<LeafBitmapL>() };
                        if let Some(last_d) = b.bitmap.prev_set(target_d) {
                            let sub = last_d >> 5;
                            let sub_word = Self::subexpanse_word(&b.bitmap, sub);
                            let bit = u32::from(last_d & 31);
                            let keep = if bit == 31 {
                                u32::MAX
                            } else {
                                (1u32 << (bit + 1)) - 1
                            };
                            let current_word = sub_word & keep;
                            let rank = (current_word.count_ones() as u16).wrapping_sub(1);
                            self.leaf = LeafCursor::BitmapMap {
                                bitmap: b.bitmap,
                                values: b.values,
                                sub,
                                current_word,
                                rank,
                                prefix: leaf_prefix,
                            };
                        } else {
                            self.leaf = LeafCursor::Empty;
                        }
                    } else {
                        // SAFETY: live LeafBitmap1 node pointer.
                        let b = unsafe { &*cur_edge.node_ptr().cast::<LeafBitmap1>() };
                        if let Some(last_d) = b.bitmap.prev_set(target_d) {
                            let word_idx = last_d >> 6;
                            let mut words = b.bitmap.words;
                            for w in words.iter_mut().skip(word_idx as usize + 1) {
                                *w = 0;
                            }
                            let bit = u32::from(last_d & 63);
                            let keep = if bit == 63 {
                                u64::MAX
                            } else {
                                (1u64 << (bit + 1)) - 1
                            };
                            words[word_idx as usize] &= keep;
                            self.leaf = LeafCursor::BitmapSet {
                                words,
                                word_idx,
                                prefix: leaf_prefix,
                            };
                        } else {
                            self.leaf = LeafCursor::Empty;
                        }
                    }
                    return;
                }

                EdgeTag::Structural(EdgeType::FullExpanse) => {
                    let max = if cur_level == 8 {
                        u64::MAX
                    } else {
                        (1u64 << (8 * u32::from(cur_level))) - 1
                    };
                    let max_key = cur_prefix | max;
                    if end < cur_prefix {
                        self.leaf = LeafCursor::Empty;
                    } else {
                        self.leaf = LeafCursor::FullExpanse {
                            next_key: cur_prefix,
                            max_key: end.min(max_key),
                        };
                    }
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

                    let shift = 8 * u32::from(bl);
                    let end_high = if shift >= 64 { 0 } else { end >> shift };
                    let prefix_high = if shift >= 64 {
                        0
                    } else {
                        branch_prefix >> shift
                    };

                    if end_high < prefix_high {
                        self.leaf = LeafCursor::Empty;
                        return;
                    }
                    if end_high > prefix_high {
                        // SAFETY: cur_edge is a live branch at cur_level.
                        unsafe {
                            self.descend_max(&cur_edge, cur_level, cur_prefix);
                        }
                        return;
                    }

                    let (num, digits, edges_ptr, ptr_raw) = if matches!(t, EdgeType::BranchL3) {
                        // SAFETY: cur_edge is a live BranchL3 pointer.
                        let b = unsafe { &*cur_edge.node_ptr().cast::<BranchL3>() };
                        (
                            b.hdr.num,
                            b.hdr.digits,
                            b.edges.as_ptr(),
                            b as *const BranchL3 as *const (),
                        )
                    } else {
                        // SAFETY: cur_edge is a live BranchL7 pointer.
                        let b = unsafe { &*cur_edge.node_ptr().cast::<BranchL7>() };
                        (
                            b.hdr.num,
                            b.hdr.digits,
                            b.edges.as_ptr(),
                            b as *const BranchL7 as *const (),
                        )
                    };

                    if num == 0 {
                        self.leaf = LeafCursor::Empty;
                        return;
                    }

                    let target_d = crate::types::digit(end, bl);
                    let bound = digits[..num as usize].partition_point(|&bd| bd <= target_d);
                    if bound == 0 {
                        self.leaf = LeafCursor::Empty;
                        return;
                    }
                    let slot = bound - 1;
                    let d = digits[slot];
                    let child_prefix = branch_prefix | (u64::from(d) << (8 * u32::from(bl - 1)));
                    // SAFETY: slot < num and edges_ptr has num entries.
                    let next_child = unsafe { *edges_ptr.add(slot) };

                    if matches!(t, EdgeType::BranchL3) {
                        self.stack[self.depth] = StackFrame {
                            kind: BranchKind::L3 {
                                ptr: ptr_raw.cast(),
                                num,
                                digits,
                                idx: slot as u8,
                            },
                            level: bl,
                            prefix: branch_prefix,
                        };
                    } else {
                        self.stack[self.depth] = StackFrame {
                            kind: BranchKind::L7 {
                                ptr: ptr_raw.cast(),
                                num,
                                digits,
                                idx: slot as u8,
                            },
                            level: bl,
                            prefix: branch_prefix,
                        };
                    }
                    self.depth += 1;

                    if d == target_d {
                        cur_edge = next_child;
                        cur_level = bl - 1;
                        cur_prefix = child_prefix;
                    } else {
                        // First digit <= target_d is strictly less; descend rightmost.
                        // SAFETY: next_child is a live edge at bl - 1.
                        unsafe {
                            self.descend_max(&next_child, bl - 1, child_prefix);
                        }
                        return;
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

                    let shift = 8 * u32::from(bl);
                    let end_high = if shift >= 64 { 0 } else { end >> shift };
                    let prefix_high = if shift >= 64 {
                        0
                    } else {
                        branch_prefix >> shift
                    };

                    if end_high < prefix_high {
                        self.leaf = LeafCursor::Empty;
                        return;
                    }
                    if end_high > prefix_high {
                        // SAFETY: cur_edge is a live branch at cur_level.
                        unsafe {
                            self.descend_max(&cur_edge, cur_level, cur_prefix);
                        }
                        return;
                    }

                    let target_d = crate::types::digit(end, bl);
                    let Some(last_d) = b.bitmap.prev_set(target_d) else {
                        self.leaf = LeafCursor::Empty;
                        return;
                    };

                    let slot = b.bitmap.subexpanse_rank(last_d) as usize;
                    let sub = (last_d >> 5) as usize;
                    // SAFETY: last_d is set in bitmap → subarrays[sub] is non-null.
                    let next_child = unsafe { *b.subarrays[sub].add(slot) };
                    let child_prefix =
                        branch_prefix | (u64::from(last_d) << (8 * u32::from(bl - 1)));

                    self.stack[self.depth] = StackFrame {
                        kind: BranchKind::B {
                            ptr: b,
                            current_digit: u16::from(last_d),
                        },
                        level: bl,
                        prefix: branch_prefix,
                    };
                    self.depth += 1;

                    if last_d == target_d {
                        cur_edge = next_child;
                        cur_level = bl - 1;
                        cur_prefix = child_prefix;
                    } else {
                        // SAFETY: next_child is a live edge at bl - 1.
                        unsafe {
                            self.descend_max(&next_child, bl - 1, child_prefix);
                        }
                        return;
                    }
                }

                EdgeTag::Structural(EdgeType::BranchU) => {
                    // SAFETY: live BranchU pointer.
                    let b = unsafe { &*cur_edge.node_ptr().cast::<BranchU>() };
                    let bl = cur_level;
                    let shift = 8 * u32::from(bl);
                    let end_high = if shift >= 64 { 0 } else { end >> shift };
                    let prefix_high = if shift >= 64 { 0 } else { cur_prefix >> shift };

                    if end_high < prefix_high {
                        self.leaf = LeafCursor::Empty;
                        return;
                    }
                    if end_high > prefix_high {
                        // SAFETY: cur_edge is a live branch at cur_level.
                        unsafe {
                            self.descend_max(&cur_edge, cur_level, cur_prefix);
                        }
                        return;
                    }

                    let target_d = i32::from(crate::types::digit(end, bl));
                    let mut d = target_d;
                    while d >= 0 && b.edges[d as usize].is_null() {
                        d -= 1;
                    }
                    if d < 0 {
                        self.leaf = LeafCursor::Empty;
                        return;
                    }
                    let d = d as usize;
                    let next_child = b.edges[d];
                    let child_prefix = cur_prefix | (u64::from(d as u8) << (8 * u32::from(bl - 1)));

                    self.stack[self.depth] = StackFrame {
                        kind: BranchKind::U {
                            ptr: b,
                            current_digit: d as u16,
                        },
                        level: bl,
                        prefix: cur_prefix,
                    };
                    self.depth += 1;

                    if d as i32 == target_d {
                        cur_edge = next_child;
                        cur_level = bl - 1;
                        cur_prefix = child_prefix;
                    } else {
                        // SAFETY: next_child is a live edge at bl - 1.
                        unsafe {
                            self.descend_max(&next_child, bl - 1, child_prefix);
                        }
                        return;
                    }
                }
            }
        }
    }

    /// Retreats the iterator to the previous leaf in tree order.
    ///
    /// # Safety
    /// Stack must contain valid node pointers per tree invariants.
    #[inline(always)]
    unsafe fn retreat_leaf(&mut self) -> bool {
        while self.depth > 0 {
            let top = &mut self.stack[self.depth - 1];
            let bl = top.level;
            let branch_prefix = top.prefix;

            match &mut top.kind {
                BranchKind::L3 {
                    ptr,
                    num: _,
                    digits,
                    idx,
                } => {
                    if *idx > 0 {
                        *idx -= 1;
                        let slot = *idx as usize;
                        let d = digits[slot];
                        // SAFETY: ptr is valid BranchL3 and slot < num.
                        let child = unsafe { (*(*ptr)).edges[slot] };
                        let child_prefix =
                            branch_prefix | (u64::from(d) << (8 * u32::from(bl - 1)));
                        // SAFETY: child is live edge at bl - 1.
                        unsafe {
                            self.descend_max(&child, bl - 1, child_prefix);
                        }
                        return true;
                    }
                }
                BranchKind::L7 {
                    ptr,
                    num: _,
                    digits,
                    idx,
                } => {
                    if *idx > 0 {
                        *idx -= 1;
                        let slot = *idx as usize;
                        let d = digits[slot];
                        // SAFETY: ptr is valid BranchL7 and slot < num.
                        let child = unsafe { (*(*ptr)).edges[slot] };
                        let child_prefix =
                            branch_prefix | (u64::from(d) << (8 * u32::from(bl - 1)));
                        // SAFETY: child is live edge at bl - 1.
                        unsafe {
                            self.descend_max(&child, bl - 1, child_prefix);
                        }
                        return true;
                    }
                }
                BranchKind::B { ptr, current_digit } => {
                    if *current_digit > 0 {
                        // SAFETY: ptr is valid BranchB.
                        let b = unsafe { &**ptr };
                        if let Some(prev_d) = b.bitmap.prev_set((*current_digit - 1) as u8) {
                            *current_digit = u16::from(prev_d);
                            let slot = b.bitmap.subexpanse_rank(prev_d) as usize;
                            let sub = (prev_d >> 5) as usize;
                            // SAFETY: prev_d is set in bitmap → subarrays[sub] is non-null.
                            let child = unsafe { *b.subarrays[sub].add(slot) };
                            let child_prefix =
                                branch_prefix | (u64::from(prev_d) << (8 * u32::from(bl - 1)));
                            // SAFETY: child is live edge at bl - 1.
                            unsafe {
                                self.descend_max(&child, bl - 1, child_prefix);
                            }
                            return true;
                        }
                    }
                }
                BranchKind::U { ptr, current_digit } => {
                    // SAFETY: ptr is valid BranchU.
                    let b = unsafe { &**ptr };
                    while *current_digit > 0 {
                        *current_digit -= 1;
                        let d = *current_digit;
                        let child = b.edges[d as usize];
                        if !child.is_null() {
                            let child_prefix =
                                branch_prefix | (u64::from(d) << (8 * u32::from(bl - 1)));
                            if let Some(EdgeTag::Immed(im)) = child.tag()
                                && im.key_count() == 1
                            {
                                let (low, value) = unpack_immed_single::<MAP>(&child, im);
                                self.leaf = LeafCursor::ImmedSingle {
                                    key: child_prefix | low,
                                    value,
                                    done: false,
                                };
                                return true;
                            }
                            // SAFETY: child is live edge at bl - 1.
                            unsafe {
                                self.descend_max(&child, bl - 1, child_prefix);
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

    /// Yields the next element in descending key order.
    #[inline(always)]
    pub fn next_back(&mut self) -> Option<(Key, u64)> {
        loop {
            match &mut self.leaf {
                LeafCursor::Empty => return None,

                LeafCursor::RootLeaf {
                    keys,
                    values,
                    pop: _,
                    idx,
                } => {
                    if *idx > 0 {
                        *idx -= 1;
                        let i = *idx as usize;
                        // SAFETY: i < pop per idx invariant.
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
                    pop: _,
                    idx,
                    prefix,
                } => {
                    if *idx > 0 {
                        *idx -= 1;
                        let i = *idx as usize;
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
                } => loop {
                    let w = words[*word_idx as usize];
                    if w != 0 {
                        let bit = 63 - w.leading_zeros();
                        words[*word_idx as usize] = w & !(1u64 << bit);
                        let d = (u64::from(*word_idx) << 6) | u64::from(bit);
                        return Some((*prefix | d, 0));
                    }
                    if *word_idx == 0 {
                        break;
                    }
                    *word_idx -= 1;
                },

                LeafCursor::BitmapMap {
                    bitmap,
                    values,
                    sub,
                    current_word,
                    rank,
                    prefix,
                } => loop {
                    if *current_word != 0 {
                        let bit = 31 - current_word.leading_zeros();
                        *current_word &= !(1u32 << bit);
                        let d = (u64::from(*sub) << 5) | u64::from(bit);
                        // SAFETY: bit is set in subexpanse → value array has entry at rank.
                        let v = unsafe { *values[*sub as usize].add(*rank as usize) };
                        *rank = rank.wrapping_sub(1);
                        return Some((*prefix | d, v));
                    }
                    if *sub == 0 {
                        break;
                    }
                    *sub -= 1;
                    let sw = Self::subexpanse_word(bitmap, *sub);
                    *current_word = sw;
                    *rank = (sw.count_ones() as u16).wrapping_sub(1);
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
                    count: _,
                    idx,
                } => {
                    if *idx > 0 {
                        *idx -= 1;
                        let i = *idx as usize;
                        return Some((keys[i], values[i]));
                    }
                }

                LeafCursor::FullExpanse { next_key, max_key } => {
                    if *max_key >= *next_key {
                        let k = *max_key;
                        if *max_key == *next_key {
                            *next_key = 1;
                            *max_key = 0;
                        } else {
                            *max_key -= 1;
                        }
                        return Some((k, 0));
                    }
                }
            }

            // Current leaf exhausted; descend to previous leaf in tree walk.
            // SAFETY: tree invariant holds.
            let has_more = unsafe { self.retreat_leaf() };
            if !has_more {
                self.leaf = LeafCursor::Empty;
                return None;
            }
        }
    }

    // --- Stateful forward seek (advance_to cursor engine) ----------------
    //
    // A `RawIter` built as a *forward* cursor (`from_tree` / `from_tree_range`
    // / `from_root_leaf*`) keeps its whole descent path — the edge stack plus a
    // leaf position. `seek_forward` reuses that path to reposition to the next
    // key `>= target` without re-descending from the root when it can avoid it:
    // a target inside the current leaf is a leaf-local search, a target under a
    // near ancestor re-descends only the levels it crosses, and only a target
    // beyond the entire current path re-descends from the root. This is the
    // engine behind [`crate::cursor`]; see docs/ALGORITHMS.md §3.5.

    /// Repositions this **forward** cursor so the next [`next`](Self::next)
    /// yields the smallest key `>= target`, reusing the current descent path.
    ///
    /// A `target` inside the current leaf is a leaf-local SIMD/SWAR search; a
    /// `target` in a sibling or ancestor subtree re-descends only from the
    /// deepest stack ancestor whose expanse still covers it; only a `target`
    /// beyond the whole current path re-descends from `top`. Zero allocation.
    ///
    /// `top` is the trie root edge, or [`Edge::NULL`] for a flat root-leaf
    /// cursor (which is handled entirely leaf-locally).
    ///
    /// **Precondition:** `target` should be strictly greater than the cursor's
    /// current position (monotone skip-scan). A call that violates it —
    /// `target` at or below the current key — is a **no-op**: every leaf arm
    /// clamps to the current front rather than rewinding, so the next
    /// [`next`](Self::next) still yields a key `>= target` and never re-yields a
    /// consumed one.
    ///
    /// # Safety
    /// The iterator must be a live forward cursor over the trie rooted at
    /// `top` (or a root leaf) and positioned per the tree invariants.
    #[inline]
    pub unsafe fn seek_forward(&mut self, top: &Edge, target: u64) {
        // Flat root-leaf cursor: no trie stack, pure leaf-local reposition.
        if let LeafCursor::RootLeaf { keys, pop, idx, .. } = &mut self.leaf {
            // SAFETY: root leaf holds `pop` sorted u64 keys per contract.
            let slice = unsafe { core::slice::from_raw_parts(*keys, *pop as usize) };
            let at = slice.partition_point(|&k| k < target);
            if at >= *pop as usize {
                self.leaf = LeafCursor::Empty;
            } else if at as u32 > *idx {
                *idx = at as u32;
            }
            return;
        }

        // Drained / empty cursor: nothing remains at or after `target`.
        if matches!(self.leaf, LeafCursor::Empty) {
            return;
        }

        // 1. Leaf-local fast path: `target` inside the current leaf's expanse.
        // SAFETY: the current leaf cursor holds live pointers per invariants.
        if unsafe { self.leaf_seek_forward(target) } {
            return;
        }

        // 2. Ascend, re-descending from the deepest ancestor still covering it.
        loop {
            if self.depth == 0 {
                // Whole current path exhausted for `target`: re-descend root.
                self.leaf = LeafCursor::Empty;
                if !top.is_null() {
                    // SAFETY: `top` is a live trie root per contract.
                    unsafe {
                        self.descend_seek(top, 8, 0, target);
                        if matches!(self.leaf, LeafCursor::Empty) {
                            self.advance_leaf();
                        }
                    }
                }
                return;
            }
            let frame = self.stack[self.depth - 1];
            let shift = 8u32 * u32::from(frame.level);
            let covers = shift >= 64 || (target >> shift) == (frame.prefix >> shift);
            if !covers {
                self.depth -= 1;
                continue;
            }
            // SAFETY: the covering frame holds a live branch per invariants.
            if unsafe { self.reseek_branch(target) } {
                return;
            }
            // Branch has no child `>= target`'s digit here: ascend to parent.
            self.depth -= 1;
        }
    }

    /// Leaf-local forward seek: if `target` lies in the current leaf's expanse
    /// and a key `>= target` still remains in it, repositions inside the leaf
    /// and returns `true`. Returns `false` (caller must ascend) when `target`
    /// is beyond the leaf, leaving the cursor usable for the stack walk.
    ///
    /// # Safety
    /// The current leaf cursor must hold live pointers per the tree invariants.
    #[inline]
    unsafe fn leaf_seek_forward(&mut self, target: u64) -> bool {
        match &mut self.leaf {
            LeafCursor::Empty | LeafCursor::RootLeaf { .. } => false,

            LeafCursor::Linear {
                keys_ptr,
                kb,
                pop,
                idx,
                prefix,
                ..
            } => {
                let shift = 8 * u32::from(*kb);
                let t_high = target >> shift;
                let p_high = *prefix >> shift;
                if t_high < p_high {
                    // Below the leaf: the current front is already `>= target`.
                    return true;
                }
                if t_high > p_high {
                    return false;
                }
                let t_low = crate::mutate::key_low(target, *kb);
                // SAFETY: `keys_ptr` addresses `pop` packed keys of width `kb`.
                let at = unsafe {
                    match crate::leaf::locate(*keys_ptr, *pop as usize, *kb, t_low) {
                        Ok(i) | Err(i) => i,
                    }
                };
                if at >= *pop as usize {
                    return false;
                }
                if at as u16 > *idx {
                    *idx = at as u16;
                }
                true
            }

            LeafCursor::BitmapSet {
                words,
                word_idx,
                prefix,
            } => {
                let t_high = target >> 8;
                let p_high = *prefix >> 8;
                if t_high < p_high {
                    return true;
                }
                if t_high > p_high {
                    return false;
                }
                let td = (target & 0xFF) as u8;
                let new_wi = (td >> 6) as usize;
                for w in words.iter_mut().take(new_wi) {
                    *w = 0;
                }
                words[new_wi] &= u64::MAX << (td & 63);
                if words[new_wi..].iter().all(|&w| w == 0) {
                    return false;
                }
                *word_idx = new_wi as u8;
                true
            }

            LeafCursor::BitmapMap {
                bitmap,
                sub,
                current_word,
                rank,
                prefix,
                ..
            } => {
                let t_high = target >> 8;
                let p_high = *prefix >> 8;
                if t_high < p_high {
                    return true;
                }
                if t_high > p_high {
                    return false;
                }
                let td = (target & 0xFF) as u8;
                let Some(fd) = bitmap.next_set(td) else {
                    return false;
                };
                let new_sub = fd >> 5;
                // Defensive rewind guard. This arm re-derives the position from
                // the pristine `bitmap`, so a below-precondition `target` (≤ the
                // current front) would otherwise resolve `fd` at or before where
                // we already are and re-yield a consumed digit. The current
                // front is `(sub << 5) | current_word.trailing_zeros()` when
                // `current_word != 0`, else it lies in a higher sub. If `fd`
                // would land at or before it, no-op instead of rewinding. (The
                // other leaf arms are monotone by construction.)
                if new_sub < *sub
                    || (new_sub == *sub
                        && (*current_word == 0 || (fd & 31) < current_word.trailing_zeros() as u8))
                {
                    return true;
                }
                let sub_word = Self::subexpanse_word(bitmap, new_sub);
                *sub = new_sub;
                *current_word = sub_word & (u32::MAX << (u32::from(fd) & 31));
                *rank = bitmap.subexpanse_rank(fd) as u16;
                true
            }

            LeafCursor::ImmedSingle { key, done, .. } => !*done && *key >= target,

            LeafCursor::ImmedMulti {
                keys, count, idx, ..
            } => {
                let mut i = *idx as usize;
                while i < *count as usize && keys[i] < target {
                    i += 1;
                }
                if i >= *count as usize {
                    return false;
                }
                *idx = i as u8;
                true
            }

            LeafCursor::FullExpanse { next_key, max_key } => {
                if target > *max_key {
                    return false;
                }
                if target > *next_key {
                    *next_key = target;
                }
                true
            }
        }
    }

    /// Re-dispatches the top stack branch toward `target`, descending into the
    /// first child whose digit is `>= target`'s digit at this branch level and
    /// advancing the frame's forward cursor past it. Returns `false` when the
    /// branch holds no such child (caller ascends); otherwise repositions the
    /// leaf cursor — recovering the next in-order leaf when the chosen child
    /// itself holds nothing `>= target` — and returns `true`.
    ///
    /// # Safety
    /// The top stack frame must hold a live branch pointer at its level.
    #[inline]
    unsafe fn reseek_branch(&mut self, target: u64) -> bool {
        let depth = self.depth;
        let bl;
        let branch_prefix;
        let child;
        let child_d;
        {
            let frame = &mut self.stack[depth - 1];
            bl = frame.level;
            branch_prefix = frame.prefix;
            let target_d = crate::types::digit(target, bl);
            match &mut frame.kind {
                BranchKind::L3 {
                    ptr,
                    num,
                    digits,
                    idx,
                } => {
                    let n = *num as usize;
                    let bound = digits[..n].partition_point(|&d| d < target_d);
                    if bound >= n {
                        return false;
                    }
                    *idx = (bound + 1) as u8;
                    child_d = digits[bound];
                    // SAFETY: `ptr` is a live BranchL3; `bound < num`.
                    child = unsafe { (*(*ptr)).edges[bound] };
                }
                BranchKind::L7 {
                    ptr,
                    num,
                    digits,
                    idx,
                } => {
                    let n = *num as usize;
                    let bound = digits[..n].partition_point(|&d| d < target_d);
                    if bound >= n {
                        return false;
                    }
                    *idx = (bound + 1) as u8;
                    child_d = digits[bound];
                    // SAFETY: `ptr` is a live BranchL7; `bound < num`.
                    child = unsafe { (*(*ptr)).edges[bound] };
                }
                BranchKind::B { ptr, current_digit } => {
                    // SAFETY: `ptr` is a live BranchB.
                    let b = unsafe { &**ptr };
                    let Some(d) = b.bitmap.next_set(target_d) else {
                        return false;
                    };
                    *current_digit = u16::from(d) + 1;
                    let slot = b.bitmap.subexpanse_rank(d) as usize;
                    let sub = (d >> 5) as usize;
                    child_d = d;
                    // SAFETY: `d` is set → `subarrays[sub]` non-null, `slot` valid.
                    child = unsafe { *b.subarrays[sub].add(slot) };
                }
                BranchKind::U { ptr, current_digit } => {
                    // SAFETY: `ptr` is a live BranchU.
                    let b = unsafe { &**ptr };
                    let mut d = u16::from(target_d);
                    while d < 256 && b.edges[d as usize].is_null() {
                        d += 1;
                    }
                    if d == 256 {
                        return false;
                    }
                    *current_digit = d + 1;
                    child_d = d as u8;
                    child = b.edges[d as usize];
                }
            }
        }
        let child_prefix = branch_prefix | (u64::from(child_d) << (8 * u32::from(bl - 1)));
        // SAFETY: `child` is a live edge at level `bl - 1`.
        unsafe {
            self.descend_seek(&child, bl - 1, child_prefix, target);
            if matches!(self.leaf, LeafCursor::Empty) {
                self.advance_leaf();
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::ExpanseMap;
    use crate::set::ExpanseSet;
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn test_empty_iter() {
        let mut empty_set_iter = RawIter::<false>::new();
        assert_eq!(empty_set_iter.next(), None);

        let mut empty_map_iter = RawIter::<true>::new();
        assert_eq!(empty_map_iter.next(), None);
    }

    #[test]
    fn seek_forward_below_current_no_rewind_bitmap_map() {
        // Exercises the BitmapMap arm's defensive rewind guard directly: a
        // dense low-byte run under one prefix builds a bitmap map-leaf, and a
        // `seek_forward` whose target is *below* the current front (violating
        // the monotone precondition) must be a no-op, not a rewind.
        use crate::map::ExpanseMap;
        use crate::sync::RootSnapshot;

        let base = 0x7700u64;
        let mut map = ExpanseMap::new();
        for i in 0..200u64 {
            map.insert(base + i, i);
        }
        assert!(
            map.stats().node_counts.leaf_bitmap > 0,
            "test must exercise a bitmap map-leaf"
        );

        let (snap, _) = map.occ_root();
        let RootSnapshot::Tree { top, .. } = snap else {
            panic!("expected a tree root");
        };
        // SAFETY: `top` is the live tree root of `map`, held immutable here.
        let mut it = unsafe { RawIter::<true>::from_tree(&top) };

        // Drive into the middle of the bitmap leaf.
        let mut last = 0u64;
        for _ in 0..100 {
            last = it.next().expect("element").0;
        }
        assert_eq!(last, base + 99);

        // Below-current target in the SAME leaf expanse: must not rewind.
        // SAFETY: `it` is a live forward cursor over `top`.
        unsafe { it.seek_forward(&top, base + 5) };
        let n = it.next().expect("element after seek").0;
        assert!(
            n > last,
            "seek_forward with a below-current target rewound: {n:#x} <= {last:#x}"
        );
        assert_eq!(
            n,
            base + 100,
            "no-op seek must resume at the next in-order key"
        );
    }

    #[test]
    fn test_set_iter_dense_and_sparse() {
        let mut set = ExpanseSet::new();
        let mut model = BTreeSet::new();

        // 1. Immediate
        for k in [10, 20, 30] {
            set.insert(k);
            model.insert(k);
        }
        assert_eq!(
            set.iter().collect::<Vec<_>>(),
            model.iter().copied().collect::<Vec<_>>()
        );

        // 2. Linear leaf
        for k in 100..120 {
            set.insert(k);
            model.insert(k);
        }
        assert_eq!(
            set.iter().collect::<Vec<_>>(),
            model.iter().copied().collect::<Vec<_>>()
        );

        // 3. Bitmap leaf & multi-level branches
        for k in (1000..2000).step_by(3) {
            set.insert(k);
            model.insert(k);
        }
        for k in [100_000, 1_000_000, 100_000_000, 0x1234_5678_9ABC_DEF0] {
            set.insert(k);
            model.insert(k);
        }
        assert_eq!(
            set.iter().collect::<Vec<_>>(),
            model.iter().copied().collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_iter_sparse_single_key_immediates() {
        // `keys(i) = i << 40` (benches/compare.rs "sparse"): bytes 0..4 are
        // always zero, so every leaf is a single-key immediate. Exercises the
        // ImmedSingle fast path for both set and map flavors against the
        // ordered std baselines.
        let n = if cfg!(miri) { 100u64 } else { 20_000u64 };

        let mut set = ExpanseSet::new();
        let mut set_model = BTreeSet::new();
        for i in 0..n {
            let k = i << 40;
            set.insert(k);
            set_model.insert(k);
        }
        assert!(
            set.iter().eq(set_model.iter().copied()),
            "sparse set ordered iteration"
        );

        let mut map = ExpanseMap::new();
        let mut map_model = BTreeMap::new();
        for i in 0..n {
            let k = i << 40;
            let v = k.wrapping_mul(0x9E37_79B9_7F4A_7C15);
            map.insert(k, v);
            map_model.insert(k, v);
        }
        assert!(
            map.iter().eq(map_model.iter().map(|(&k, &v)| (k, v))),
            "sparse map ordered iteration"
        );
    }

    // ---- Reverse iteration differential tests -------------------------

    fn seeded(seed: u64) -> impl FnMut() -> u64 {
        let mut s = seed | 1;
        move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        }
    }

    fn build_pair(keys: &[u64]) -> (ExpanseSet, ExpanseMap, BTreeSet<u64>, BTreeMap<u64, u64>) {
        let mut set = ExpanseSet::new();
        let mut map = ExpanseMap::new();
        let mut bset = BTreeSet::new();
        let mut bmap = BTreeMap::new();
        for &k in keys {
            let v = k.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xABCD;
            set.insert(k);
            map.insert(k, v);
            bset.insert(k);
            bmap.insert(k, v);
        }
        (set, map, bset, bmap)
    }

    fn check_distribution(keys: &[u64]) {
        let (set, map, bset, bmap) = build_pair(keys);

        // Full reverse via iter_rev; the reverse iterator is double-ended, so
        // `.rev()` on it recovers ascending order.
        let desc_set: Vec<u64> = bset.iter().rev().copied().collect();
        let asc_set: Vec<u64> = bset.iter().copied().collect();
        assert_eq!(set.iter_rev().collect::<Vec<_>>(), desc_set, "set.iter_rev");
        assert_eq!(
            set.iter_rev().rev().collect::<Vec<_>>(),
            asc_set,
            "set.iter_rev().rev()"
        );
        let desc_map: Vec<(u64, u64)> = bmap.iter().rev().map(|(&k, &v)| (k, v)).collect();
        let asc_map: Vec<(u64, u64)> = bmap.iter().map(|(&k, &v)| (k, v)).collect();
        assert_eq!(map.iter_rev().collect::<Vec<_>>(), desc_map, "map.iter_rev");
        assert_eq!(
            map.iter_rev().rev().collect::<Vec<_>>(),
            asc_map,
            "map.iter_rev().rev()"
        );

        // Range reverse over a spread of bounds, including boundary keys.
        let mut bounds: Vec<u64> = keys.to_vec();
        bounds.sort_unstable();
        bounds.dedup();
        let probes: Vec<u64> = if bounds.is_empty() {
            vec![0, 1, u64::MAX]
        } else {
            let mut p = vec![0u64, u64::MAX];
            for i in [0, bounds.len() / 3, bounds.len() / 2, bounds.len() - 1] {
                let b = bounds[i];
                p.push(b.saturating_sub(1));
                p.push(b);
                p.push(b.saturating_add(1));
            }
            p
        };
        for &lo in &probes {
            for &hi in &probes {
                let (w_set, wm_desc, wm_asc) = if lo <= hi {
                    let ws: Vec<u64> = bset.range(lo..=hi).copied().collect();
                    let wmd: Vec<(u64, u64)> =
                        bmap.range(lo..=hi).rev().map(|(&k, &v)| (k, v)).collect();
                    let wma: Vec<(u64, u64)> = bmap.range(lo..=hi).map(|(&k, &v)| (k, v)).collect();
                    (ws, wmd, wma)
                } else {
                    (Vec::new(), Vec::new(), Vec::new())
                };

                let ds: Vec<u64> = set.range_rev(lo..=hi).collect();
                let mut exp_rev = w_set.clone();
                exp_rev.reverse();
                assert_eq!(ds, exp_rev, "set.range_rev({lo}..={hi})");
                assert_eq!(
                    set.range_rev(lo..=hi).rev().collect::<Vec<_>>(),
                    w_set,
                    "set.range_rev({lo}..={hi}).rev()"
                );

                assert_eq!(
                    map.range_rev(lo..=hi).collect::<Vec<_>>(),
                    wm_desc,
                    "map.range_rev({lo}..={hi})"
                );
                assert_eq!(
                    map.range_rev(lo..=hi).rev().collect::<Vec<_>>(),
                    wm_asc,
                    "map.range_rev({lo}..={hi}).rev()"
                );
            }
        }
    }

    #[test]
    fn rev_immediate_and_linear() {
        check_distribution(&[]);
        check_distribution(&[42]);
        check_distribution(&[10, 20, 30]);
        check_distribution(&(100..140).collect::<Vec<_>>());
    }

    #[test]
    fn rev_bitmap_and_branches() {
        let dense: Vec<u64> = (1000..2000).step_by(3).collect();
        check_distribution(&dense);
        let spread: Vec<u64> = vec![
            0,
            1,
            255,
            256,
            100_000,
            1_000_000,
            100_000_000,
            0x1234_5678_9ABC_DEF0,
            u64::MAX - 1,
            u64::MAX,
        ];
        check_distribution(&spread);
    }

    #[test]
    fn rev_sparse_single_key_immediates() {
        let count = if cfg!(miri) { 100u64 } else { 5000u64 };
        let keys: Vec<u64> = (0..count).map(|i| i << 40).collect();
        check_distribution(&keys);
    }

    #[test]
    fn rev_random_distributions() {
        let count = if cfg!(miri) { 50 } else { 3000 };
        for seed in [1u64, 7, 99, 12345] {
            let mut rng = seeded(seed);
            let keys: Vec<u64> = (0..count).map(|_| rng()).collect();
            check_distribution(&keys);
            let masked: Vec<u64> = (0..count).map(|_| rng() & 0xFFFF).collect();
            check_distribution(&masked);
        }
    }

    #[test]
    fn rev_interleaved_no_cross() {
        // DoubleEndedIterator contract on the reverse iterators: `next`
        // descends from the top, `next_back` ascends from the bottom, and
        // together they yield every element exactly once, ends never crossing.
        let count = if cfg!(miri) { 40 } else { 1500 };
        for seed in [3u64, 42, 2024] {
            let mut rng = seeded(seed);
            let keys: Vec<u64> = (0..count).map(|_| rng() & 0x3FFF).collect();
            let (set, map, bset, bmap) = build_pair(&keys);

            // Two-ended drain against an ascending model: `next` consumes the
            // high end (`top`), `next_back` the low end (`bot`).
            let model: Vec<u64> = bset.iter().copied().collect();
            let mut bot = 0usize;
            let mut top = model.len();
            let mut it = set.iter_rev();
            let mut take_primary = true;
            while bot < top {
                if take_primary {
                    top -= 1;
                    assert_eq!(it.next(), Some(model[top]), "primary (descending)");
                } else {
                    assert_eq!(it.next_back(), Some(model[bot]), "back (ascending)");
                    bot += 1;
                }
                take_primary = !take_primary;
            }
            assert_eq!(it.next(), None, "primary exhausted");
            assert_eq!(it.next_back(), None, "back exhausted");

            // Same for a bounded range on the map.
            let (lo, hi) = (0x0400u64, 0x3000u64);
            let model: Vec<(u64, u64)> = bmap.range(lo..=hi).map(|(&k, &v)| (k, v)).collect();
            let mut bot = 0usize;
            let mut top = model.len();
            let mut it = map.range_rev(lo..=hi);
            let mut take_primary = true;
            while bot < top {
                if take_primary {
                    top -= 1;
                    assert_eq!(it.next(), Some(model[top]));
                } else {
                    assert_eq!(it.next_back(), Some(model[bot]));
                    bot += 1;
                }
                take_primary = !take_primary;
            }
            assert_eq!(it.next(), None);
            assert_eq!(it.next_back(), None);
        }
    }

    #[test]
    fn test_map_iter_dense_and_sparse() {
        let count = if cfg!(miri) { 100u64 } else { 5000u64 };
        let mut map = ExpanseMap::new();
        let mut model = BTreeMap::new();

        for i in 0..count {
            let k = (i * 37 + 13) ^ (i << 12);
            let v = k.wrapping_mul(3);
            map.insert(k, v);
            model.insert(k, v);
        }

        let map_items: Vec<_> = map.iter().collect();
        let model_items: Vec<_> = model.iter().map(|(&k, &v)| (k, v)).collect();
        assert_eq!(map_items, model_items);
    }
}
