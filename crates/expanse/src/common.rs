//! Primitives the 64-bit engine (`leaf`) and the 32-bit engine (`trie32`)
//! share.
//!
//! `leaf` compiles on 64-bit targets only, and `trie32` compiles on every
//! target, so a definition both engines use lives here rather than in either.

/// The default slot-capacity ladder (`leaf::cap_class`): populations of 1 or
/// 2 stay exact, 3..=16 round to multiples of 4 slots, 17..=32 round to 24 or
/// 32, and larger populations round to multiples of 4.
///
/// The 32-bit engine uses this ladder under every feature set. The 64-bit
/// `leaf::cap_class` uses it unless `ablation-leaf-class-10` replaces it, and
/// that ablation does not reach the 32-bit engine.
#[inline]
#[must_use]
pub(crate) const fn cap_class(pop: usize) -> usize {
    if pop <= 2 {
        pop
    } else if pop <= 16 {
        (pop + 3) & !3
    } else if pop <= 24 {
        24
    } else if pop <= 32 {
        32
    } else {
        (pop + 3) & !3
    }
}
