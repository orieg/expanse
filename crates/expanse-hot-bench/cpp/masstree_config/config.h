/* config.h for the Expanse Masstree comparison arm (#661).
 *
 * masstree-beta generates this file with autoconf; the arm compiles Masstree
 * through cc-rs instead and cannot run ./configure. Every macro below is what
 * configure.ac (at 1119842) defines on an x86-64 Linux glibc host with GCC:
 * the builtin probes, the same-type probes, the sizes, and the defaults for
 * the tunables (row type, max key length, superpages, assertions off).
 *
 * Deliberate choices, each disclosed in docs/benchmarks/masstree_comparison/METHODOLOGY.md:
 *   - no HAVE_JEMALLOC / HAVE_TCMALLOC / HAVE_FLOW_MALLOC: the arm allocates
 *     from glibc malloc, the allocator every other arm in the suite uses;
 *   - HAVE_SUPERPAGE on, as configure enables it by default on Linux;
 *   - MASSTREE_MAXKEYLEN 255, the shipped default;
 *   - assertions, preconditions and invariants off (NDEBUG), as the authors
 *     specify for performance measurement (README: --disable-assertions).
 */
#ifndef MASSTREE_CONFIG_H_INCLUDED
#define MASSTREE_CONFIG_H_INCLUDED 1

#define WORDS_BIGENDIAN_SET 1

#define HAVE_SYS_EPOLL_H 1
#define HAVE_TYPE_TRAITS 1
#define HAVE_TIME_H 1
#define HAVE_EXECINFO_H 1
#define HAVE_DECL_CLOCK_GETTIME 1
#define HAVE_CLOCK_GETTIME 1
#define HAVE_DECL_GETLINE 1

#define HAVE___BUILTIN_CLZ 1
#define HAVE___BUILTIN_CLZL 1
#define HAVE___BUILTIN_CLZLL 1
#define HAVE___BUILTIN_CTZ 1
#define HAVE___BUILTIN_CTZL 1
#define HAVE___BUILTIN_CTZLL 1
#define HAVE___SYNC_SYNCHRONIZE 1
#define HAVE___SYNC_FETCH_AND_ADD 1
#define HAVE___SYNC_ADD_AND_FETCH 1
#define HAVE___SYNC_FETCH_AND_ADD_8 1
#define HAVE___SYNC_ADD_AND_FETCH_8 1
#define HAVE___SYNC_FETCH_AND_OR 1
#define HAVE___SYNC_OR_AND_FETCH 1
#define HAVE___SYNC_FETCH_AND_OR_8 1
#define HAVE___SYNC_OR_AND_FETCH_8 1
#define HAVE___SYNC_BOOL_COMPARE_AND_SWAP 1
#define HAVE___SYNC_BOOL_COMPARE_AND_SWAP_8 1
#define HAVE___SYNC_VAL_COMPARE_AND_SWAP 1
#define HAVE___SYNC_VAL_COMPARE_AND_SWAP_8 1
#define HAVE___SYNC_LOCK_TEST_AND_SET 1
#define HAVE___SYNC_LOCK_TEST_AND_SET_VAL 1
#define HAVE___SYNC_LOCK_RELEASE_SET 1

#define HAVE_CXX_TEMPLATE_ALIAS 1
#define HAVE_STD_HASH 1
#define HAVE_STD_IS_TRIVIALLY_COPYABLE 1
#define HAVE_STD_IS_TRIVIALLY_DESTRUCTIBLE 1
#define HAVE_STD_IS_RVALUE_REFERENCE 1

#define HAVE_OFF_T_IS_LONG 1
#define HAVE_INT64_T_IS_LONG 1
#define HAVE_SIZE_T_IS_UNSIGNED_LONG 1
#define HAVE_LONG_LONG 1
#define SIZEOF_SHORT 2
#define SIZEOF_INT 4
#define SIZEOF_LONG 8
#define SIZEOF_LONG_LONG 8
#define SIZEOF_VOID_P 8

#define MASSTREE_ROW_TYPE_BAG 1
#define MASSTREE_MAXKEYLEN 255
#define HAVE_MADV_HUGEPAGE 1
#define HAVE_MAP_HUGETLB 1
#define HAVE_SUPERPAGE 1
#define CACHE_LINE_SIZE 64
#define HAVE_UNALIGNED_ACCESS 1

/* --- verbatim AH_BOTTOM block from configure.ac --- */
#if !FORCE_ENABLE_ASSERTIONS && !ENABLE_ASSERTIONS
# define NDEBUG 1
#endif

/** @brief Assert macro that always runs. */
extern void fail_always_assert(const char* file, int line, const char* assertion, const char* message = 0) __attribute__((noreturn));
#define always_assert(x, ...) do { if (!(x)) fail_always_assert(__FILE__, __LINE__, #x, ## __VA_ARGS__); } while (0)
#define mandatory_assert always_assert

extern void fail_masstree_invariant(const char* file, int line, const char* assertion, const char* message = 0) __attribute__((noreturn));
#if FORCE_ENABLE_ASSERTIONS || (!defined(ENABLE_INVARIANTS) && ENABLE_ASSERTIONS) || ENABLE_INVARIANTS
#define masstree_invariant(x, ...) do { if (!(x)) fail_masstree_invariant(__FILE__, __LINE__, #x, ## __VA_ARGS__); } while (0)
#else
#define masstree_invariant(x, ...) do { } while (0)
#endif

extern void fail_masstree_precondition(const char* file, int line, const char* assertion, const char* message = 0) __attribute__((noreturn));
#if FORCE_ENABLE_ASSERTIONS || (!defined(ENABLE_PRECONDITIONS) && ENABLE_ASSERTIONS) || ENABLE_PRECONDITIONS
#define masstree_precondition(x, ...) do { if (!(x)) fail_masstree_precondition(__FILE__, __LINE__, #x, ## __VA_ARGS__); } while (0)
#else
#define masstree_precondition(x, ...) do { } while (0)
#endif

#ifndef invariant
#define invariant masstree_invariant
#endif
#ifndef precondition
#define precondition masstree_precondition
#endif

#endif
