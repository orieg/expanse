/*
 * `J1C` / `JLC` from an unmodified classic consumer (docs/COMPAT.md
 * doc-gap D7). Configure scripts that probe for libjudy (traildb, SaunaFS)
 * compile and run a program calling `J1C`, so a count that a stock-built
 * program cannot use fails their configure step, not only their runtime.
 *
 * Built with the classic macro error handling (no JUDYERROR_NOTEST): the
 * stock `J1C`/`JLC` read the error record when the count is 0 and hand
 * anything but JU_ERRNO_NONE to JUDYERROR, which exits. The CI job compiles
 * this file against the STOCK Judy.h and library, runs it on stock and with
 * libexpanse LD_PRELOADed, and diffs the transcripts. It also checks every
 * count and errno itself: it exits 1 on a wrong one, and prints "ok" only if
 * every check held and no macro took its error path. It is also compiled
 * against libexpanse's own Judy.h, where the macros pass PJE0.
 *
 * The function-form checks are the disambiguation recipe of Judy1_funcs(3).
 */
#include <Judy.h>
#include <stdio.h>
#include <string.h>

/*
 * The function form, with the record pre-filled so a record left unwritten
 * is visible. Returns 1 when the count is 0 and the record says
 * JU_ERRNO_NONE, 0 when it is 0 and says anything else, and -1 for a
 * nonzero count, whose record Judy1_funcs(3) does not ask the caller to read.
 */
static int j1_none(Pcvoid_t set, Word_t a, Word_t b, Word_t *n) {
    JError_t je;
    memset(&je, 0xA5, sizeof je);
    *n = Judy1Count(set, a, b, &je);
    return *n == 0 ? JU_ERRNO(&je) == JU_ERRNO_NONE : -1;
}

static int jl_none(Pcvoid_t map, Word_t a, Word_t b, Word_t *n) {
    JError_t je;
    memset(&je, 0xA5, sizeof je);
    *n = JudyLCount(map, a, b, &je);
    return *n == 0 ? JU_ERRNO(&je) == JU_ERRNO_NONE : -1;
}

/* Index ranges probed through the function form, and the counts expected on
 * {10, 20, 30}: a gap, a reversed range, and the populated span. */
static const Word_t FN_RANGE[3][2] = {{11, 19}, {30, 10}, {10, 30}};
static const Word_t FN_COUNT[3] = {0, 0, 3};

int main(void) {
    Pvoid_t set = (Pvoid_t) NULL;
    Pvoid_t map = (Pvoid_t) NULL;
    Word_t count;
    Word_t bytes;
    int rc;
    PWord_t pv;

    /* ---- Judy1 through the J1C macro ---- */
    J1C(count, set, 0, -1);
    printf("j1 empty %lu\n", (unsigned long) count);
    if (count != 0) {
        return 1;
    }
    for (Word_t k = 10; k <= 30; k += 10) {
        J1S(rc, set, k);
        if (rc != 1) {
            printf("j1 set %lu failed\n", (unsigned long) k);
            return 1;
        }
    }
    J1C(count, set, 0, -1);
    printf("j1 all %lu\n", (unsigned long) count);
    if (count != 3) {
        return 1;
    }
    J1C(count, set, 11, 19);
    printf("j1 gap %lu\n", (unsigned long) count);
    if (count != 0) {
        return 1;
    }
    J1C(count, set, 30, 10);
    printf("j1 reversed %lu\n", (unsigned long) count);
    if (count != 0) {
        return 1;
    }
    rc = j1_none(NULL, 0, -1, &count);
    printf("j1 fn empty = %lu none=%d\n", (unsigned long) count, rc);
    if (count != 0 || rc != 1) {
        return 1;
    }
    for (int i = 0; i < 3; i++) {
        rc = j1_none(set, FN_RANGE[i][0], FN_RANGE[i][1], &count);
        printf("j1 fn %lu..%lu = %lu none=%d\n", (unsigned long) FN_RANGE[i][0],
               (unsigned long) FN_RANGE[i][1], (unsigned long) count, rc);
        if (count != FN_COUNT[i] || rc != (FN_COUNT[i] == 0 ? 1 : -1)) {
            return 1;
        }
    }
    J1FA(bytes, set);
    if (bytes == 0 || set != NULL) {
        printf("j1 free failed\n");
        return 1;
    }

    /* ---- JudyL through the JLC macro ---- */
    JLC(count, map, 0, -1);
    printf("jl empty %lu\n", (unsigned long) count);
    if (count != 0) {
        return 1;
    }
    for (Word_t k = 10; k <= 30; k += 10) {
        JLI(pv, map, k);
        if (pv == NULL || pv == PJERR) {
            printf("jl insert %lu failed\n", (unsigned long) k);
            return 1;
        }
        *pv = k;
    }
    JLC(count, map, 0, -1);
    printf("jl all %lu\n", (unsigned long) count);
    if (count != 3) {
        return 1;
    }
    JLC(count, map, 11, 19);
    printf("jl gap %lu\n", (unsigned long) count);
    if (count != 0) {
        return 1;
    }
    JLC(count, map, 30, 10);
    printf("jl reversed %lu\n", (unsigned long) count);
    if (count != 0) {
        return 1;
    }
    rc = jl_none(NULL, 0, -1, &count);
    printf("jl fn empty = %lu none=%d\n", (unsigned long) count, rc);
    if (count != 0 || rc != 1) {
        return 1;
    }
    for (int i = 0; i < 3; i++) {
        rc = jl_none(map, FN_RANGE[i][0], FN_RANGE[i][1], &count);
        printf("jl fn %lu..%lu = %lu none=%d\n", (unsigned long) FN_RANGE[i][0],
               (unsigned long) FN_RANGE[i][1], (unsigned long) count, rc);
        if (count != FN_COUNT[i] || rc != (FN_COUNT[i] == 0 ? 1 : -1)) {
            return 1;
        }
    }
    JLFA(bytes, map);
    if (bytes == 0 || map != NULL) {
        printf("jl free failed\n");
        return 1;
    }

    printf("ok\n");
    return 0;
}
