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
 * libexpanse LD_PRELOADed, and diffs the transcripts; "ok" is printed only
 * if no macro took its error path. It is also compiled against libexpanse's
 * own Judy.h, where the macros pass PJE0.
 *
 * The function-form section is the disambiguation recipe of Judy1_funcs(3),
 * with the record pre-filled so that a record left unwritten is visible.
 */
#include <Judy.h>
#include <stdio.h>
#include <string.h>

static void j1_function_form(Pcvoid_t set, Word_t a, Word_t b) {
    JError_t je;
    memset(&je, 0xA5, sizeof je);
    Word_t n = Judy1Count(set, a, b, &je);
    printf("j1 fn %lu..%lu = %lu none=%d\n", (unsigned long) a, (unsigned long) b,
           (unsigned long) n, n == 0 ? JU_ERRNO(&je) == JU_ERRNO_NONE : -1);
}

static void jl_function_form(Pcvoid_t map, Word_t a, Word_t b) {
    JError_t je;
    memset(&je, 0xA5, sizeof je);
    Word_t n = JudyLCount(map, a, b, &je);
    printf("jl fn %lu..%lu = %lu none=%d\n", (unsigned long) a, (unsigned long) b,
           (unsigned long) n, n == 0 ? JU_ERRNO(&je) == JU_ERRNO_NONE : -1);
}

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
    for (Word_t k = 10; k <= 30; k += 10) {
        J1S(rc, set, k);
    }
    J1C(count, set, 0, -1);
    printf("j1 all %lu\n", (unsigned long) count);
    J1C(count, set, 11, 19);
    printf("j1 gap %lu\n", (unsigned long) count);
    J1C(count, set, 30, 10);
    printf("j1 reversed %lu\n", (unsigned long) count);
    j1_function_form(NULL, 0, -1);
    j1_function_form(set, 11, 19);
    j1_function_form(set, 30, 10);
    j1_function_form(set, 10, 30);
    J1FA(bytes, set);

    /* ---- JudyL through the JLC macro ---- */
    JLC(count, map, 0, -1);
    printf("jl empty %lu\n", (unsigned long) count);
    for (Word_t k = 10; k <= 30; k += 10) {
        JLI(pv, map, k);
        *pv = k;
    }
    JLC(count, map, 0, -1);
    printf("jl all %lu\n", (unsigned long) count);
    JLC(count, map, 11, 19);
    printf("jl gap %lu\n", (unsigned long) count);
    JLC(count, map, 30, 10);
    printf("jl reversed %lu\n", (unsigned long) count);
    jl_function_form(NULL, 0, -1);
    jl_function_form(map, 11, 19);
    jl_function_form(map, 30, 10);
    jl_function_form(map, 10, 30);
    JLFA(bytes, map);

    (void) bytes;
    printf("ok\n");
    return 0;
}
