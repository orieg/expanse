/*
 * trim_census.c: an LD_PRELOAD census of the glibc heap's growth and trim
 * calls, attributed to the allocator call that made each one (README.md,
 * "Attribution: where main's heap trims come from", #1680).
 *
 * Untimed and deterministic: it counts, it does not time. Linux and glibc only.
 *
 * Every allocator entry point is wrapped. The wrapper reads the program break
 * (sbrk(0), which glibc answers from a cached value without a system call)
 * before and after the real call:
 *   - a free or realloc after which the break is lower is a TRIM event: the
 *     freed chunk's usable size, the pages given back, the top chunk left
 *     (mallinfo2().keepcost) and the calling object are recorded;
 *   - an allocation after which the break is higher is a GROW event: the
 *     request size and the pages taken are recorded.
 * The JudyL entry points stress-ng's `judy` stressor calls (JudyLIns,
 * JudyLGet, JudyLDel, JudyLFreeArray) are wrapped too, only to name the
 * phase an event falls in. A JudyLDel after which the array word is null is
 * the `teardown` phase: the call that empties the array, with every free the
 * library makes inside it.
 *
 * The real allocator is reached through glibc's exported __libc_* entry
 * points, so the wrapper never calls dlsym on an allocation path. The JudyL
 * entry points are reached through dlsym(RTLD_NEXT), resolved once.
 *
 * Output: one JSON object per process, written at exit (atexit and _exit
 * both, once) to $TRIM_CENSUS_DIR/trim_<pid>.json, without allocating. A process that made no
 * JudyL call writes nothing, so stress-ng's parent is not reported.
 *
 * Build: gcc -O2 -shared -fPIC -o trim_census.so trim_census.c -ldl
 */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <stdarg.h>
#include <malloc.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/syscall.h>
#include <unistd.h>

extern void *__libc_malloc(size_t);
extern void __libc_free(void *);
extern void *__libc_calloc(size_t, size_t);
extern void *__libc_realloc(void *, size_t);
extern void *__libc_memalign(size_t, size_t);

typedef void *Pvoid_t;
typedef unsigned long Word_t;
typedef void *PJError_t;

enum { PH_INIT, PH_INSERT, PH_GET, PH_DELETE, PH_TEARDOWN, PH_FREEARRAY, PH_N, PH_PEND = PH_N };
static const char *PH_NAME[PH_N] = {"init", "insert", "get", "delete", "teardown", "freearray"};
static int phase = PH_INIT;
static int judy_seen = 0;
static long page_size = 4096;

/* Per phase; PH_PEND holds one JudyLDel call's events until the call returns
 * and says whether it emptied the array (teardown) or not (delete). */
struct ctr { uint64_t frees, allocs, trims, pages_trimmed, grows, pages_grown; };
static struct ctr C[PH_N + 1];
static uint64_t n_ins, n_get, n_del, n_fa, n_teardown;

/* Sizes, exact up to a cap of distinct values per table. */
#define NSZ 512
struct szc { uint64_t size, count, pages; };
enum { T_TRIM, T_GROW, T_KEEP, T_FREE, T_N };
static const char *T_NAME[T_N] = {"trim_by_free_usable_size", "grow_by_request_size",
                                  "top_bytes_after_trim", "frees_by_usable_size"};
static struct szc TBL[PH_N + 1][T_N][NSZ];
static uint64_t sz_overflow;

static void szc_add(struct szc *t, uint64_t size, uint64_t count, uint64_t pages) {
    for (int i = 0; i < NSZ; i++) {
        if (t[i].count == 0) { t[i].size = size; t[i].count = count; t[i].pages = pages; return; }
        if (t[i].size == size) { t[i].count += count; t[i].pages += pages; return; }
    }
    sz_overflow++;
}

/* Calling object of a trim, by return address; resolved at dump time. */
#define NCALLER 256
struct callc { uintptr_t ra; uint64_t count; };
static struct callc trim_caller[NCALLER];
static void caller_add(uintptr_t ra) {
    for (int i = 0; i < NCALLER; i++) {
        if (trim_caller[i].count == 0) { trim_caller[i].ra = ra; trim_caller[i].count = 1; return; }
        if (trim_caller[i].ra == ra) { trim_caller[i].count++; return; }
    }
}

static inline uintptr_t brk_now(void) { return (uintptr_t)sbrk(0); }

static void after_free(uintptr_t b0, size_t usable, uintptr_t ra) {
    uintptr_t b1 = brk_now();
    C[phase].frees++;
    szc_add(TBL[phase][T_FREE], usable, 1, 0);
    if (b1 < b0) {
        uint64_t pages = (b0 - b1) / (uint64_t)page_size;
        C[phase].trims++;
        C[phase].pages_trimmed += pages;
        szc_add(TBL[phase][T_TRIM], usable, 1, pages);
        struct mallinfo2 mi = mallinfo2();
        szc_add(TBL[phase][T_KEEP], mi.keepcost, 1, 0);
        caller_add(ra);
    }
}

static void after_alloc(uintptr_t b0, size_t req) {
    uintptr_t b1 = brk_now();
    C[phase].allocs++;
    if (b1 > b0) {
        uint64_t pages = (b1 - b0) / (uint64_t)page_size;
        C[phase].grows++;
        C[phase].pages_grown += pages;
        szc_add(TBL[phase][T_GROW], req, 1, pages);
    }
}

void free(void *p) {
    if (!p) return;
    uintptr_t b0 = brk_now();
    size_t u = malloc_usable_size(p);
    __libc_free(p);
    after_free(b0, u, (uintptr_t)__builtin_return_address(0));
}
void *malloc(size_t n) {
    uintptr_t b0 = brk_now();
    void *r = __libc_malloc(n);
    after_alloc(b0, n);
    return r;
}
void *calloc(size_t a, size_t b) {
    uintptr_t b0 = brk_now();
    void *r = __libc_calloc(a, b);
    after_alloc(b0, a * b);
    return r;
}
void *realloc(void *p, size_t n) {
    uintptr_t b0 = brk_now();
    size_t u = p ? malloc_usable_size(p) : 0;
    void *r = __libc_realloc(p, n);
    if (brk_now() < b0) after_free(b0, u, (uintptr_t)__builtin_return_address(0));
    else after_alloc(b0, n);
    return r;
}
void *memalign(size_t al, size_t n) {
    uintptr_t b0 = brk_now();
    void *r = __libc_memalign(al, n);
    after_alloc(b0, n);
    return r;
}
void *aligned_alloc(size_t al, size_t n) { return memalign(al, n); }
int posix_memalign(void **out, size_t al, size_t n) {
    uintptr_t b0 = brk_now();
    void *r = __libc_memalign(al, n);
    after_alloc(b0, n);
    if (!r) return ENOMEM;
    *out = r;
    return 0;
}

/* JudyL phase markers. */
typedef Pvoid_t *(*ins_fn)(Pvoid_t *, Word_t, PJError_t);
typedef Pvoid_t *(*get_fn)(Pvoid_t, Word_t, PJError_t);
typedef int (*del_fn)(Pvoid_t *, Word_t, PJError_t);
typedef Word_t (*fa_fn)(Pvoid_t *, PJError_t);
static ins_fn r_ins;
static get_fn r_get;
static del_fn r_del;
static fa_fn r_fa;

static void resolve(void) {
    if (r_ins) return;
    r_ins = (ins_fn)dlsym(RTLD_NEXT, "JudyLIns");
    r_get = (get_fn)dlsym(RTLD_NEXT, "JudyLGet");
    r_del = (del_fn)dlsym(RTLD_NEXT, "JudyLDel");
    r_fa = (fa_fn)dlsym(RTLD_NEXT, "JudyLFreeArray");
    if (!r_ins || !r_get || !r_del || !r_fa) {
        fputs("trim_census: JudyL entry points not found\n", stderr);
        abort();
    }
}

Pvoid_t *JudyLIns(Pvoid_t *pp, Word_t i, PJError_t e) {
    resolve();
    judy_seen = 1;
    phase = PH_INSERT;
    n_ins++;
    return r_ins(pp, i, e);
}

Pvoid_t *JudyLGet(Pvoid_t p, Word_t i, PJError_t e) {
    resolve();
    judy_seen = 1;
    phase = PH_GET;
    n_get++;
    return r_get(p, i, e);
}

int JudyLDel(Pvoid_t *pp, Word_t i, PJError_t e) {
    resolve();
    judy_seen = 1;
    n_del++;
    phase = PH_PEND;
    int r = r_del(pp, i, e);
    int to = (pp && *pp == NULL) ? PH_TEARDOWN : PH_DELETE;
    if (to == PH_TEARDOWN) n_teardown++;
    struct ctr *s = &C[PH_PEND], *d = &C[to];
    if (s->frees | s->allocs) {
        d->frees += s->frees;
        d->allocs += s->allocs;
        d->trims += s->trims;
        d->pages_trimmed += s->pages_trimmed;
        d->grows += s->grows;
        d->pages_grown += s->pages_grown;
        memset(s, 0, sizeof *s);
        for (int t = 0; t < T_N; t++) {
            struct szc *src = TBL[PH_PEND][t];
            for (int k = 0; k < NSZ && src[k].count; k++) szc_add(TBL[to][t], src[k].size, src[k].count, src[k].pages);
            for (int k = 0; k < NSZ && src[k].count; k++) src[k].count = 0;
        }
    }
    phase = PH_DELETE;
    return r;
}

Word_t JudyLFreeArray(Pvoid_t *pp, PJError_t e) {
    resolve();
    judy_seen = 1;
    phase = PH_FREEARRAY;
    n_fa++;
    return r_fa(pp, e);
}

static int dumped = 0;

/* The report is built in a static buffer and written with write(2), so
 * writing it allocates nothing and moves no break after the counts close. */
static char OUT[1 << 20];
static size_t out_n;
static void outf(const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int k = vsnprintf(OUT + out_n, sizeof OUT - out_n, fmt, ap);
    va_end(ap);
    if (k > 0) out_n += ((size_t)k < sizeof OUT - out_n) ? (size_t)k : sizeof OUT - out_n - 1;
}

static void dump_szc(const char *name, const struct szc *t) {
    outf("\"%s\":[", name);
    for (int i = 0; i < NSZ && t[i].count; i++)
        outf("%s[%llu,%llu,%llu]", i ? "," : "", (unsigned long long)t[i].size,
                (unsigned long long)t[i].count, (unsigned long long)t[i].pages);
    outf("]");
}

static void dump(void) {
    if (dumped || !judy_seen) return;
    dumped = 1;
    phase = PH_INIT;
    const char *dir = getenv("TRIM_CENSUS_DIR");
    if (!dir) return;
    char path[512];
    snprintf(path, sizeof path, "%s/trim_%d.json", dir, (int)getpid());
    struct mallinfo2 mi = mallinfo2();
    outf("{\"pid\":%d,\"ops\":{\"ins\":%llu,\"get\":%llu,\"del\":%llu,\"freearray\":%llu,\"teardown\":%llu},",
            (int)getpid(), (unsigned long long)n_ins, (unsigned long long)n_get, (unsigned long long)n_del,
            (unsigned long long)n_fa, (unsigned long long)n_teardown);
    outf("\"phases\":{");
    for (int p = 0; p < PH_N; p++) {
        outf("%s\"%s\":{\"frees\":%llu,\"allocs\":%llu,\"trims\":%llu,\"pages_trimmed\":%llu,"
                   "\"grows\":%llu,\"pages_grown\":%llu,",
                p ? "," : "", PH_NAME[p], (unsigned long long)C[p].frees, (unsigned long long)C[p].allocs,
                (unsigned long long)C[p].trims, (unsigned long long)C[p].pages_trimmed,
                (unsigned long long)C[p].grows, (unsigned long long)C[p].pages_grown);
        for (int t = 0; t < T_N; t++) {
            if (t) outf(",");
            dump_szc(T_NAME[t], TBL[p][t]);
        }
        outf("}");
    }
    outf("},\"trim_callers\":[");
    for (int i = 0; i < NCALLER && trim_caller[i].count; i++) {
        Dl_info di;
        const char *obj = "?";
        if (dladdr((void *)trim_caller[i].ra, &di) && di.dli_fname) {
            obj = strrchr(di.dli_fname, '/');
            obj = obj ? obj + 1 : di.dli_fname;
        }
        outf("%s[\"%s\",%llu]", i ? "," : "", obj, (unsigned long long)trim_caller[i].count);
    }
    outf("],\"size_table_overflow\":%llu,", (unsigned long long)sz_overflow);
    outf("\"mallinfo2_at_exit\":{\"arena\":%zu,\"ordblks\":%zu,\"hblks\":%zu,\"hblkhd\":%zu,"
               "\"uordblks\":%zu,\"fordblks\":%zu,\"keepcost\":%zu}}\n",
            mi.arena, mi.ordblks, mi.hblks, mi.hblkhd, mi.uordblks, mi.fordblks, mi.keepcost);
    int fd = open(path, O_WRONLY | O_CREAT | O_TRUNC, 0644);
    if (fd < 0) return;
    for (size_t off = 0; off < out_n;) {
        ssize_t w = write(fd, OUT + off, out_n - off);
        if (w <= 0) break;
        off += (size_t)w;
    }
    close(fd);
}

__attribute__((constructor)) static void init(void) {
    page_size = sysconf(_SC_PAGESIZE);
    atexit(dump);
}

void _exit(int status) {
    dump();
    syscall(SYS_exit_group, status);
    __builtin_unreachable();
}
