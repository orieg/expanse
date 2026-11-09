/*
 * An unmodified `-lJudy` JudyL consumer, built against the installed package
 * layout by scripts/test_packaging.sh (docs/COMPAT.md, "Library names and
 * SONAME"). It uses only what a classic libjudy consumer uses: the JLI / JLG /
 * JLD / JLFA / JLMU macros from Judy.h.
 *
 * The key order is the stress-ng `judy` stressor's: gen_index(i) puts the
 * complement of i's low byte in bits 24..31, inserts and finds run in order of
 * i, and deletes run in reverse. Prints "ok" and exits 0 only if every insert,
 * find and delete behaves as documented and the emptied array is NULL.
 */
#include <Judy.h>
#include <stdio.h>

#define N 4096

static Word_t gen_index(Word_t i) {
    return ((~i & 0xff) << 24) | (i & 0x00ffffff);
}

int main(void) {
    Pvoid_t array = NULL;
    PWord_t pv;
    Word_t bytes;
    int rc;

    for (Word_t i = 0; i < N; i++) {
        JLI(pv, array, gen_index(i));
        if (pv == NULL || pv == PJERR) {
            printf("insert %lu failed\n", (unsigned long) i);
            return 1;
        }
        *pv = i;
    }
    for (Word_t i = 0; i < N; i++) {
        JLG(pv, array, gen_index(i));
        if (pv == NULL || *pv != i) {
            printf("find %lu failed\n", (unsigned long) i);
            return 1;
        }
    }
    JLMU(bytes, array);
    if (bytes == 0) {
        printf("JudyLMemUsed returned 0 for a populated array\n");
        return 1;
    }
    for (Word_t i = N; i-- > 0;) {
        JLD(rc, array, gen_index(i));
        if (rc != 1) {
            printf("delete %lu returned %d\n", (unsigned long) i, rc);
            return 1;
        }
    }
    if (array != NULL) {
        printf("array not NULL after deleting every key\n");
        return 1;
    }
    JLI(pv, array, 42);
    if (pv == NULL || pv == PJERR) {
        printf("re-insert failed\n");
        return 1;
    }
    JLFA(bytes, array);
    if (array != NULL) {
        printf("array not NULL after JudyLFreeArray\n");
        return 1;
    }
    printf("ok\n");
    return 0;
}
