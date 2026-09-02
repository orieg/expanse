/* Bare-metal smoke for QEMU's mps2-an385 (Cortex-M3, soft float): the ARM
 * execution gate that needs no hardware (#598 step 4).
 *
 * Exercises the narrow C ABI (ordered map and set, iteration, remove_range)
 * and the sync32 single-writer / interrupt-reader protocol with a SysTick
 * reader against a churning main-loop writer, asserting that no read ever
 * returns a wrong value. It verifies the ordered core and the protocol
 * logic on an ARMv7-M core; it says nothing about caches (the M3 has none)
 * or timing (QEMU is not cycle-accurate), which stay hardware measurements
 * (integrations/stm32h747).
 *
 * Output and exit code go through ARM semihosting, so `run.sh` gets a real
 * process exit status: 0 on PASS, 1 on a failed check, 2 on a fault. */
#include <errno.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include "expanse.h"

#if EXPANSE_WIDE_SURFACE
#error "expects the 32-bit (narrow) surface"
#endif

/* ---- semihosting ------------------------------------------------------ */
static uint32_t semihost(uint32_t op, void *arg) {
    register uint32_t r0 __asm("r0") = op;
    register void *r1 __asm("r1") = arg;
    __asm volatile("bkpt 0xAB" : "+r"(r0) : "r"(r1) : "memory");
    return r0;
}
static void out(const char *s) { semihost(0x04, (void *)s); }
static void out_u32(uint32_t v) {
    char b[11]; int i = 10; b[i] = 0;
    do { b[--i] = (char)('0' + v % 10); v /= 10; } while (v);
    out(&b[i]);
}
static void kv(const char *k, uint32_t v) { out(" "); out(k); out("="); out_u32(v); }
__attribute__((noreturn)) static void exit_qemu(uint32_t code) {
    uint32_t block[2] = { 0x20026u /* ADP_Stopped_ApplicationExit */, code };
    semihost(0x20 /* SYS_EXIT_EXTENDED */, block);
    for (;;) { __asm volatile("wfi"); }
}
static void fail(const char *what) { out("FAIL "); out(what); out("\n"); exit_qemu(1); }
void fault_report(const char *what) { out("FAULT "); out(what); out("\n"); exit_qemu(2); }

/* ---- heap over the mps2 SRAM, host hooks for the staticlib ------------ */
extern uint8_t _heap_start, _heap_end;
static uint8_t *brk = &_heap_start;
void *_sbrk(ptrdiff_t incr) {
    if (brk + incr > &_heap_end) { errno = ENOMEM; return (void *)-1; }
    void *p = brk; brk += incr; return p;
}
void *expanse_host_malloc(size_t n) { return malloc(n); }
void expanse_host_free(void *p) { free(p); }

/* ---- ordered map and set ------------------------------------------------ */
#define N_MAP 20000u
static void check_map(void) {
    expanse_map_t *m = expanse_map_new();
    expanse_word_t old, k, v;
    for (uint32_t i = 0; i < N_MAP; i++) {
        if (!expanse_map_insert(m, i * 7919u, i, &old)) fail("map insert reported replace of a new key");
    }
    if (expanse_map_insert(m, 7919u, 99u, &old) || old != 1u) fail("map insert of a present key");
    if (!expanse_map_get(m, 7919u, &v) || v != 99u) fail("map insert did not replace the value");
    expanse_map_insert(m, 7919u, 1u, &old);
    if (expanse_map_len(m) != N_MAP) fail("map len after insert");
    for (uint32_t i = 0; i < N_MAP; i++)
        if (!expanse_map_get(m, i * 7919u, &v) || v != i) fail("map get");
    if (expanse_map_get(m, 1u, &v)) fail("map get of absent key");
    /* ordered walk: ascending, exactly N_MAP entries */
    uint32_t n = 0, prev = 0;
    bool have = expanse_map_first(m, &k, &v);
    while (have) {
        if (n && k <= prev) fail("map iteration not ascending");
        if (v * 7919u != k) fail("map iteration value");
        prev = k; n++;
        have = expanse_map_next_after(m, k, &k, &v);
    }
    if (n != N_MAP) fail("map iteration count");
    if (!expanse_map_last(m, &k, &v) || k != (N_MAP - 1) * 7919u) fail("map last");
    /* remove every other key, then a range */
    for (uint32_t i = 0; i < N_MAP; i += 2)
        if (!expanse_map_remove(m, i * 7919u, &old) || old != i) fail("map remove");
    if (expanse_map_len(m) != N_MAP / 2) fail("map len after remove");
    const expanse_word_t hi = 7919u * 1001u;
    uint32_t expect = 0;
    for (uint32_t i = 1; i < N_MAP; i += 2) if (i * 7919u <= hi) expect++;
    size_t got = expanse_map_remove_range(m, 0, hi, 0, 0);
    if (got != expect) fail("map remove_range count");
    if (expanse_map_len(m) != N_MAP / 2 - expect) fail("map len after remove_range");
    if (!expanse_map_first(m, &k, &v) || k <= hi) fail("map first after remove_range");
    expanse_map_free(m);
    out("ok map"); kv("keys", N_MAP); kv("range_removed", (uint32_t)got); out("\n");
}

#define N_SET 5000u
static void check_set(void) {
    expanse_set_t *s = expanse_set_new();
    expanse_word_t k;
    for (uint32_t i = 0; i < N_SET; i++) if (!expanse_set_insert(s, i * 104729u)) fail("set insert");
    for (uint32_t i = 0; i < N_SET; i++) if (!expanse_set_contains(s, i * 104729u)) fail("set contains");
    if (expanse_set_contains(s, 3u)) fail("set contains absent");
    if (!expanse_set_first(s, &k) || k != 0) fail("set first");
    if (!expanse_set_last(s, &k) || k != (N_SET - 1) * 104729u) fail("set last");
    if (!expanse_set_next_after(s, 0, &k) || k != 104729u) fail("set next_after");
    for (uint32_t i = 0; i < N_SET; i += 3) if (!expanse_set_remove(s, i * 104729u)) fail("set remove");
    if (expanse_set_contains(s, 0)) fail("set removed key still present");
    expanse_set_free(s);
    out("ok set"); kv("keys", N_SET); out("\n");
}

/* ---- sync32: SysTick reader against a churning writer ------------------- */
#define KEYS       4096u
#define KEYS_FIXED 2048u
#define VAL(k) ((k) ^ 0xABCDu)
#define MUTATIONS     200000u   /* at least this many writer mutations ... */
#define MIN_ISR       2000u     /* ... and at least this many reader interrupts */
#define MUTATIONS_CAP 20000000u /* hard cap, so a silent SysTick still terminates */
#define SYST_CSR (*(volatile uint32_t *)0xE000E010u)
#define SYST_RVR (*(volatile uint32_t *)0xE000E014u)
#define SYST_CVR (*(volatile uint32_t *)0xE000E018u)

static expanse_sync32_map_reader_t *g_reader;
static volatile uint32_t isr_n, isr_ok, isr_nf, isr_busy, isr_bad;
static uint32_t rng = 0x9E3779B9u;

void SysTick_Handler(void) {
    if (!g_reader) return;
    rng ^= rng << 13; rng ^= rng >> 17; rng ^= rng << 5;
    uint32_t k = rng & (KEYS - 1u);
    expanse_word_t v;
    expanse_sync32_status_t st = expanse_sync32_map_reader_try_get(g_reader, k, &v);
    if (st == EXPANSE_SYNC32_OK) { if (v == VAL(k)) isr_ok++; else isr_bad++; }
    else if (st == EXPANSE_SYNC32_NOT_FOUND) isr_nf++;
    else if (st == EXPANSE_SYNC32_BUSY) isr_busy++;
    else isr_bad++;
    isr_n++;
}

static void check_sync32(void) {
    expanse_sync32_map_t *map = expanse_sync32_map_new(KEYS, 1);
    if (!map) fail("sync32 new");
    expanse_sync32_map_writer_t *w = expanse_sync32_map_writer(map);
    bool rep; expanse_word_t old, v;
    for (uint32_t k = 0; k < KEYS_FIXED; k++)
        if (expanse_sync32_map_writer_try_insert(w, k, VAL(k), &rep, &old) != EXPANSE_SYNC32_OK) fail("sync32 prefill");
    g_reader = expanse_sync32_map_reader(map, 0);
    SYST_RVR = 3000u; SYST_CVR = 0; SYST_CSR = 7u;   /* processor clock, interrupt, enable */
    uint32_t refused = 0, arena_full = 0, i;
    /* QEMU's SysTick runs on virtual time while the churn runs at host
     * speed, so churn until the reader has fired enough times to mean
     * something, with a hard cap so a broken timer still terminates. */
    for (i = 0; i < MUTATIONS_CAP && (i < MUTATIONS || isr_n < MIN_ISR); i++) {
        uint32_t k = KEYS_FIXED + ((i >> 1) & (KEYS_FIXED - 1u));
        expanse_sync32_status_t st;
        for (;;) {
            st = (i & 1u) ? expanse_sync32_map_writer_try_remove(w, k, &old)
                          : expanse_sync32_map_writer_try_insert(w, k, VAL(k), &rep, &old);
            if (st == EXPANSE_SYNC32_RECLAIM_BACKLOG) { refused++; expanse_sync32_map_writer_try_reclaim(w); continue; }
            break;
        }
        if (st == EXPANSE_SYNC32_ARENA_FULL) arena_full++;
        if ((i & 31u) == 31u) expanse_sync32_map_writer_try_reclaim(w);
    }
    SYST_CSR = 0; g_reader = 0;
    for (uint32_t k = 0; k < KEYS_FIXED; k++)
        if (!expanse_sync32_map_writer_get(w, k, &v) || v != VAL(k)) fail("sync32 writer view corrupted");
    expanse_sync32_stats_t s; expanse_sync32_map_writer_stats(w, &s, sizeof s);
    out("ok sync32"); kv("mutations", i); kv("isr_n", isr_n); kv("isr_ok", isr_ok); kv("isr_nf", isr_nf);
    kv("isr_busy", isr_busy); kv("isr_bad", isr_bad); kv("refused", refused); kv("arena_full", arena_full);
    kv("len", (uint32_t)s.len); kv("pending", s.pending_len); out("\n");
    if (isr_bad) fail("sync32 reader saw a wrong value");
    if (isr_n < MIN_ISR) fail("SysTick fired too rarely");
    if (isr_ok < 10) fail("sync32 reader almost never succeeded");
    if (arena_full) fail("sync32 arena full");
    if (s.len != KEYS_FIXED) fail("sync32 len after churn");
    expanse_sync32_map_free(map);
}

int main(void) {
    out("EXPANSE qemu mps2-an385 cortex-m3 smoke\n");
    check_map();
    check_set();
    check_sync32();
    out("PASS\n");
    exit_qemu(0);
}
