//! Per-thread reader caches for the shared (`Sync*`) maps other than `SyncExpanseMap`.
//!
//! The design, and the reasons for it, are those of `MAP_READERS` in `sync.rs` (#554): a
//! one-shot `get` registers and drops a throwaway reader, which takes the collector's registry
//! mutex twice per lookup, so each thread registers one detached reader per map and reuses it.
//! The `Weak` keeps the map's allocation, and so its address, from being recycled while an
//! entry exists; the detached reader holds no reference to the map, so the cache never keeps a
//! map alive.

/// Entries a thread may accumulate in one cache before dead ones are swept.
pub(crate) const READER_CACHE_SWEEP_AT: usize = 64;

/// Declares a thread-local detached-reader cache and the function that runs a closure against
/// this thread's reader for a map.
macro_rules! detached_reader_cache {
    ($vis:vis fn $with:ident, $tls:ident, $map:ty, $reader:ty) => {
        ::std::thread_local! {
            static $tls: ::std::cell::RefCell<
                ::std::collections::HashMap<usize, (::std::sync::Weak<$map>, $reader)>,
            > = ::std::cell::RefCell::new(::std::collections::HashMap::new());
        }

        /// Runs `f` against this thread's cached reader for `map`, registering on first use.
        $vis fn $with<R>(map: &::std::sync::Arc<$map>, f: impl FnOnce(&mut $reader) -> R) -> R {
            let key = ::std::sync::Arc::as_ptr(map) as usize;
            $tls.with(|c| {
                let mut cache = c.borrow_mut();
                if !cache.contains_key(&key)
                    && cache.len() >= $crate::reader_cache::READER_CACHE_SWEEP_AT
                {
                    cache.retain(|_, (weak, _)| weak.strong_count() > 0);
                }
                let entry = cache
                    .entry(key)
                    .or_insert_with(|| (::std::sync::Arc::downgrade(map), map.detached_reader()));
                // The `Weak` pins the allocation, so a live entry's address can only belong to
                // the map it was registered against.
                debug_assert!(
                    ::std::ptr::eq(entry.0.as_ptr(), ::std::sync::Arc::as_ptr(map)),
                    "reader cache key aliased a different map"
                );
                f(&mut entry.1)
            })
        }
    };
}

pub(crate) use detached_reader_cache;
