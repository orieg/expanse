/*
 * COMPAT.md gate G4: an unmodified libjudy consumer, compiled against the
 * STOCK Judy.h and linked against the stock library, must behave
 * identically when libexpanse is LD_PRELOADed over it.
 *
 * The CI job compiles this file with the system (classic) header, runs it
 * once against stock libjudy and once under LD_PRELOAD of libexpanse, and
 * diffs the transcripts. Only semantic outputs are printed — byte totals
 * (MemUsed / FreeArray returns) are implementation-defined (doc-gaps
 * D4 and friends) and are asserted nonzero rather than printed.
 */
#include <Judy.h>
#include <stdio.h>
#include <string.h>

int main(void) {
    /* ---- Judy1 ---- */
    Pvoid_t set = NULL;
    for (Word_t i = 0; i < 600; i++) {
        Judy1Set(&set, i * 37, PJE0);
    }
    Judy1Unset(&set, 37 * 41, PJE0);
    printf("j1 test %d %d\n", (int) Judy1Test(set, 37 * 40, PJE0),
           (int) Judy1Test(set, 37 * 41, PJE0));
    printf("j1 count %lu\n", (unsigned long) Judy1Count(set, 0, -1, PJE0));
    Word_t idx = 0;
    int rc = Judy1First(set, &idx, PJE0);
    unsigned long walked = 0, sum = 0;
    while (rc == 1) {
        walked++;
        sum ^= idx;
        rc = Judy1Next(set, &idx, PJE0);
    }
    printf("j1 walk %lu %lx\n", walked, sum);
    if (Judy1FreeArray(&set, PJE0) == 0 || set != NULL) {
        printf("j1 free BAD\n");
    }

    /* ---- JudyL ---- */
    Pvoid_t map = NULL;
    for (Word_t i = 0; i < 500; i++) {
        PWord_t pv = (PWord_t) JudyLIns(&map, i << 16, PJE0);
        *pv = ~i;
    }
    PWord_t pv = (PWord_t) JudyLGet(map, 123UL << 16, PJE0);
    printf("jl get %lx\n", pv ? (unsigned long) *pv : 0UL);
    JudyLDel(&map, 123UL << 16, PJE0);
    printf("jl del %p\n", JudyLGet(map, 123UL << 16, PJE0));
    printf("jl count %lu\n", (unsigned long) JudyLCount(map, 0, -1, PJE0));
    if (JudyLFreeArray(&map, PJE0) == 0 || map != NULL) {
        printf("jl free BAD\n");
    }

    /* ---- JudySL ---- */
    Pvoid_t smap = NULL;
    const char *words[] = {"expanse", "judy", "trie", "expanse2", "a", ""};
    for (int i = 0; i < 6; i++) {
        PWord_t sv = (PWord_t) JudySLIns(&smap, (const unsigned char *) words[i], PJE0);
        *sv = (Word_t) strlen(words[i]);
    }
    unsigned char buf[64] = "";
    PWord_t sv = (PWord_t) JudySLFirst(smap, buf, PJE0);
    while (sv != NULL) {
        printf("jsl %s=%lu\n", buf, (unsigned long) *sv);
        sv = (PWord_t) JudySLNext(smap, buf, PJE0);
    }
    if (JudySLFreeArray(&smap, PJE0) == 0 || smap != NULL) {
        printf("jsl free BAD\n");
    }

    /* ---- JudyHS ---- */
    Pvoid_t hmap = NULL;
    const char key1[] = "byte\0key";
    PWord_t hv = (PWord_t) JudyHSIns(&hmap, (void *) key1, sizeof key1, PJE0);
    *hv = 4242;
    hv = (PWord_t) JudyHSIns(&hmap, (void *) "other", 5, PJE0);
    *hv = 7;
    hv = (PWord_t) JudyHSGet(hmap, (void *) key1, sizeof key1);
    printf("jhs get %lu\n", hv ? (unsigned long) *hv : 0UL);
    printf("jhs prefix %p\n", JudyHSGet(hmap, (void *) key1, 4));
    printf("jhs del %d %d\n", (int) JudyHSDel(&hmap, (void *) "other", 5, PJE0),
           (int) JudyHSDel(&hmap, (void *) "other", 5, PJE0));
    if (JudyHSFreeArray(&hmap, PJE0) == 0 || hmap != NULL) {
        printf("jhs free BAD\n");
    }

    printf("ok\n");
    return 0;
}
