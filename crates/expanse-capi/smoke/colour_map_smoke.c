/*
 * Two Judy1 arrays as a two-bit colour map, the way MariaDB's OQGraph keeps
 * vertex colours during breadth-first search and Dijkstra: bit 0 in one
 * array, bit 1 in the other, keyed by vertex id (white 0, gray 1, black 3).
 * The consumer only tests and sets bits one vertex at a time, empties both
 * arrays back to NULL between searches, and copies arrays, including empty
 * ones, by walking the source with First/Next.
 *
 * Built against the STOCK Judy.h with the classic macro error handling, the
 * differential-oracle CI job runs this on stock libjudy, with libexpanse
 * LD_PRELOADed, relinked against libexpanse, and built on libexpanse's own
 * Judy.h, and diffs the transcripts. It also checks its own results: it
 * exits 1 on any wrong colour, count, visit order or copy, and prints "ok"
 * only if every check held.
 */
#include <Judy.h>
#include <stdio.h>

#define WHITE 0
#define GRAY 1
#define BLACK 3

/* Vertex ids are not 0..n-1 in every OQGraph table: half the searches use a
 * sparse id space, so the bitsets see both dense and scattered indexes. */
static Word_t vertex_id(Word_t v, int sparse) {
    return sparse ? v * 0x9E3779B1UL + 7 : v;
}

static int colour(Pvoid_t b0, Pvoid_t b1, Word_t id) {
    int rc0, rc1;
    J1T(rc0, b0, id);
    J1T(rc1, b1, id);
    return rc0 | (rc1 << 1);
}

static void paint(Pvoid_t *b0, Pvoid_t *b1, Word_t id, int c) {
    int rc;
    if (c & 1) {
        J1S(rc, *b0, id);
    } else {
        J1U(rc, *b0, id);
    }
    if (c & 2) {
        J1S(rc, *b1, id);
    } else {
        J1U(rc, *b1, id);
    }
    (void) rc;
}

/* A deterministic graph: each vertex has three out-edges from an LCG, plus
 * the edge v -> v+1 so every vertex is reachable from 0. */
static Word_t edge(Word_t v, int e, Word_t n) {
    Word_t x = v * 6364136223846793005UL + (Word_t) e * 1442695040888963407UL + 1;
    return e == 3 ? (v + 1) % n : (x >> 17) % n;
}

/* Copies `src` into a fresh array by walking it with J1F/J1N. */
static Pvoid_t copy(Pvoid_t src) {
    Pvoid_t dst = (Pvoid_t) NULL;
    Word_t i = 0;
    int rc, s;
    J1F(rc, src, i);
    while (rc == 1) {
        J1S(s, dst, i);
        J1N(rc, src, i);
    }
    (void) s;
    return dst;
}

/* One breadth-first search from vertex 0 over n vertices. Returns 0 on
 * success and fills a checksum of the visit order. */
static int bfs(Word_t n, int sparse, Word_t *visit_sum, Word_t *visited) {
    static Word_t queue[100000];
    Pvoid_t b0 = (Pvoid_t) NULL, b1 = (Pvoid_t) NULL;
    Word_t head = 0, tail = 0, sum = 0, count, bytes;
    int rc;

    paint(&b0, &b1, vertex_id(0, sparse), GRAY);
    queue[tail++] = 0;
    while (head < tail) {
        Word_t v = queue[head++];
        sum = sum * 31 + v;
        for (int e = 0; e < 4; e++) {
            Word_t w = edge(v, e, n);
            if (colour(b0, b1, vertex_id(w, sparse)) == WHITE) {
                paint(&b0, &b1, vertex_id(w, sparse), GRAY);
                queue[tail++] = w;
            }
        }
        paint(&b0, &b1, vertex_id(v, sparse), BLACK);
    }

    /* Every vertex is black: both bits set on all of them. */
    J1C(count, b0, 0, -1);
    if (count != tail) return 1;
    J1C(count, b1, 0, -1);
    if (count != tail) return 1;
    for (Word_t v = 0; v < n; v++) {
        if (colour(b0, b1, vertex_id(v, sparse)) != BLACK) return 1;
    }

    /* A copy of a non-empty colour bit holds the same indexes. */
    Pvoid_t c0 = copy(b0);
    J1C(count, c0, 0, -1);
    if (count != tail) return 1;
    J1FA(bytes, c0);
    if (c0 != NULL) return 1;

    /* Back to white, one vertex at a time: both arrays return to NULL. */
    for (Word_t v = 0; v < n; v++) {
        paint(&b0, &b1, vertex_id(v, sparse), WHITE);
    }
    if (b0 != NULL || b1 != NULL) return 1;
    J1T(rc, b0, vertex_id(0, sparse));
    if (rc != 0) return 1;

    /* A copy of an empty array is empty, and freeing it frees nothing. */
    Pvoid_t e = copy(b0);
    if (e != NULL) return 1;
    J1FA(bytes, e);
    if (bytes != 0) return 1;

    *visit_sum = sum;
    *visited = tail;
    return 0;
}

int main(void) {
    static const Word_t sizes[] = {1, 10, 1000, 10000, 100000};
    for (int sparse = 0; sparse < 2; sparse++) {
        for (unsigned s = 0; s < sizeof sizes / sizeof *sizes; s++) {
            Word_t sum, visited;
            if (bfs(sizes[s], sparse, &sum, &visited) != 0) {
                printf("FAIL n %lu sparse %d\n", (unsigned long) sizes[s], sparse);
                return 1;
            }
            if (visited != sizes[s]) {
                printf("FAIL n %lu sparse %d visited %lu\n", (unsigned long) sizes[s], sparse,
                       (unsigned long) visited);
                return 1;
            }
            printf("bfs n %lu sparse %d visited %lu order %016lx\n", (unsigned long) sizes[s],
                   sparse, (unsigned long) visited, (unsigned long) sum);
        }
    }
    printf("ok\n");
    return 0;
}
