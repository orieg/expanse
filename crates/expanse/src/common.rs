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

/// Fails the build if any listed type implements `Sync`.
///
/// Stable Rust has no negative bound, so this is the ambiguity construction:
/// every type gets the `()` impl, a `Sync` type gets a second one, and the
/// inferred `_` then has two candidates, which is an error (E0283). The
/// `compile_fail` doctests on the public types guard the same property from
/// outside the crate; this guards it at the definition, in every build,
/// `no_std` included (it names nothing from `std`). Its users, `alloc` and
/// `occ`, exist on 64-bit targets only.
#[cfg(target_pointer_width = "64")]
macro_rules! assert_not_sync {
    ($($t:ty),+ $(,)?) => {
        const _: fn() = || {
            trait AmbiguousIfSync<A> {
                fn probe() {}
            }
            impl<T: ?Sized> AmbiguousIfSync<()> for T {}
            struct IsSync;
            impl<T: ?Sized + Sync> AmbiguousIfSync<IsSync> for T {}
            $(let _ = <$t as AmbiguousIfSync<_>>::probe;)+
        };
    };
}
#[cfg(target_pointer_width = "64")]
pub(crate) use assert_not_sync;
