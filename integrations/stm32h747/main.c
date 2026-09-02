/* STM32H747I-DISCO Cortex-M7 harness for libexpanse (narrow surface).
 *
 * Runs the embedded_memtable fixtures through the C ABI — and through three
 * alternatives a firmware engineer would reach for (alts.c) — reporting DWT
 * cycle counts over USART1 (the ST-LINK V3 virtual COM port, PA9/PA10) at
 * three core clocks (64 MHz HSI, 160 MHz PLL1/VOS3, 400 MHz PLL1/VOS1) with
 * the D-cache off and on, then the sync32 ISR-reader arm against a
 * critical-section twin. The board is wired for direct SMPS supply, which
 * caps the part at VOS1 / 400 MHz (VOS0 / 480 MHz needs the LDO path).
 * Every number is in core cycles; capture.py times the TICK/TOCK spin so
 * the harvest can convert to ns with a host-verified clock.
 *
 * Output lines are machine-readable: `RESULT k=v k=v ...`. */
#include <errno.h>
#include <malloc.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include "alts.h"
#include "expanse.h"

#if EXPANSE_WIDE_SURFACE
#error "expects the 32-bit (narrow) surface"
#endif

#define REG(a) (*(volatile uint32_t *)(a))
#define RCC_CR        REG(0x58024400u)
#define RCC_CFGR      REG(0x58024410u)
#define RCC_D1CFGR    REG(0x58024418u)
#define RCC_D2CFGR    REG(0x5802441Cu)
#define RCC_D3CFGR    REG(0x58024420u)
#define RCC_PLLCKSELR REG(0x58024428u)
#define RCC_PLLCFGR   REG(0x5802442Cu)
#define RCC_PLL1DIVR  REG(0x58024430u)
#define RCC_D2CCIP2R  REG(0x58024454u)
#define RCC_AHB4ENR   REG(0x580244E0u)
#define RCC_APB2ENR   REG(0x580244F0u)
#define PWR_CSR1      REG(0x58024804u)
#define PWR_CR3       REG(0x5802480Cu)
#define PWR_D3CR      REG(0x58024818u)
#define DBGMCU_IDCODE REG(0x5C001000u)
#define GPIOA_MODER   REG(0x58020000u)
#define GPIOA_OSPEEDR REG(0x58020008u)
#define GPIOA_AFRH    REG(0x58020024u)
#define USART1_CR1    REG(0x40011000u)
#define USART1_BRR    REG(0x4001100Cu)
#define USART1_ISR    REG(0x4001101Cu)
#define USART1_TDR    REG(0x40011028u)
#define DEMCR         REG(0xE000EDFCu)
#define DWT_CTRL      REG(0xE0001000u)
#define DWT_CYCCNT    REG(0xE0001004u)
#define DWT_LAR       REG(0xE0001FB0u)
#define SCB_CPUID     REG(0xE000ED00u)
#define SCB_CCR       REG(0xE000ED14u)
#define SCB_CCSIDR    REG(0xE000ED80u)
#define SCB_CSSELR    REG(0xE000ED84u)
#define SCB_ICIALLU   REG(0xE000EF50u)
#define SCB_DCISW     REG(0xE000EF60u)
#define SCB_DCCISW    REG(0xE000EF74u)
#define SYST_CSR      REG(0xE000E010u)
#define SYST_RVR      REG(0xE000E014u)
#define SYST_CVR      REG(0xE000E018u)

#define BAUD   115200u
#define PASSES 5u

static uint32_t sysclk_hz = 64000000u;
static inline uint32_t cyc(void) { return DWT_CYCCNT; }

/* ---- UART (kernel clock pinned to hsi_ker_ck so the baud survives PLL switches) */
static void uart_init(void) {
    RCC_AHB4ENR |= 1u;         (void)RCC_AHB4ENR;
    RCC_APB2ENR |= (1u << 4);  (void)RCC_APB2ENR;
    GPIOA_MODER = (GPIOA_MODER & ~((3u << 18) | (3u << 20))) | (2u << 18) | (2u << 20);
    GPIOA_OSPEEDR |= (3u << 18) | (3u << 20);
    GPIOA_AFRH = (GPIOA_AFRH & ~((0xFu << 4) | (0xFu << 8))) | (7u << 4) | (7u << 8);
    RCC_D2CCIP2R = (RCC_D2CCIP2R & ~(7u << 3)) | (3u << 3);
    USART1_CR1 = 0;
    USART1_BRR = 64000000u / BAUD;
    USART1_CR1 = (1u << 3) | (1u << 2) | 1u;
}
static void uart_putc(char c) { while (!(USART1_ISR & (1u << 7))) {} USART1_TDR = (uint32_t)c; }
void uart_puts(const char *s) { while (*s) uart_putc(*s++); }
static void uart_u32(uint32_t v) {
    char b[11]; int i = 10; b[i] = 0;
    do { b[--i] = (char)('0' + v % 10); v /= 10; } while (v);
    uart_puts(&b[i]);
}
void uart_hex(uint32_t v) {
    static const char h[] = "0123456789abcdef";
    uart_puts("0x");
    for (int i = 28; i >= 0; i -= 4) uart_putc(h[(v >> i) & 0xF]);
}
static void kv(const char *k, uint32_t v) { uart_putc(' '); uart_puts(k); uart_putc('='); uart_u32(v); }
static void ks(const char *k, const char *v) { uart_putc(' '); uart_puts(k); uart_putc('='); uart_puts(v); }
static void result(const char *name, const char *impl, uint32_t dcache, uint32_t pass, uint32_t cycles, uint32_t ops) {
    uart_puts("RESULT name="); uart_puts(name); ks("impl", impl);
    kv("sysclk", sysclk_hz); kv("dcache", dcache); kv("pass", pass); kv("cycles", cycles); kv("ops", ops);
    uart_puts("\r\n");
}

/* ---- heap over AXI SRAM, host hooks for the staticlib ------------------ */
extern uint8_t _heap_start, _heap_end;
static uint8_t *brk = &_heap_start;
static uint8_t *brk_hwm = &_heap_start;
void *_sbrk(ptrdiff_t incr) {
    if (brk + incr > &_heap_end) { errno = ENOMEM; return (void *)-1; }
    void *p = brk; brk += incr;
    if (brk > brk_hwm) brk_hwm = brk;
    return p;
}
void *expanse_host_malloc(size_t n) { return acct_malloc(n); }
void expanse_host_free(void *p) { acct_free(p); }
static size_t heap_in_use(void) { return (size_t)mallinfo().uordblks; }

/* ---- caches, cycle counter, clocks ---------------------------------- */
static void dsb_isb(void) { __asm volatile("dsb\n isb" ::: "memory"); }
static void icache_enable(void) { dsb_isb(); SCB_ICIALLU = 0; dsb_isb(); SCB_CCR |= (1u << 17); dsb_isb(); }
static void dcache_set_way(volatile uint32_t *op) {
    SCB_CSSELR = 0; __asm volatile("dsb" ::: "memory");
    uint32_t ccsidr = SCB_CCSIDR;
    uint32_t sets = (ccsidr >> 13) & 0x7FFF, ways = (ccsidr >> 3) & 0x3FF;
    for (uint32_t s = 0; s <= sets; s++)
        for (uint32_t w = 0; w <= ways; w++) *op = (s << 5) | (w << 30);
    dsb_isb();
}
static void dcache_enable(void) { dcache_set_way(&SCB_DCISW); SCB_CCR |= (1u << 16); dsb_isb(); }
static void dcache_disable(void) { SCB_CCR &= ~(1u << 16); __asm volatile("dsb" ::: "memory"); dcache_set_way(&SCB_DCCISW); }
static void dwt_init(void) { DEMCR |= (1u << 24); DWT_LAR = 0xC5ACCE55u; DWT_CYCCNT = 0; DWT_CTRL |= 1u; }
static void calibrate(void) { /* host times TICK..TOCK */
    uart_puts("TICK\r\n");
    uint32_t t0 = cyc(); while (cyc() - t0 < 320000000u) {}
    uart_puts("TOCK cycles=320000000\r\n");
}
/* Back to HSI and PLL1 off, so the PLL can be reprogrammed. */
static void clock_hsi(void) {
    RCC_CFGR &= ~7u; while (RCC_CFGR & (7u << 3)) {}
    RCC_CR &= ~(1u << 24); while (RCC_CR & (1u << 25)) {}
}
/* PLL1: HSI 64 / M=4 -> 16 MHz ref, N -> VCO 16*N MHz, P=2. hpre/ppre are
 * the register codes (0 = /1, 0x8 / 0x4 = /2). FLASH_ACR stays at its reset
 * maximum (7 WS): correct at every clock here, and the I-cache hides it. */
static void clock_pll(uint32_t n, uint32_t hpre, uint32_t ppre) {
    clock_hsi();
    RCC_D1CFGR = (RCC_D1CFGR & ~(0xFu | (7u << 4))) | hpre | (ppre << 4);
    RCC_D2CFGR = (RCC_D2CFGR & ~((7u << 4) | (7u << 8))) | (ppre << 4) | (ppre << 8);
    RCC_D3CFGR = (RCC_D3CFGR & ~(7u << 4)) | (ppre << 4);
    RCC_PLLCKSELR = (RCC_PLLCKSELR & ~((0x3Fu << 4) | 3u)) | (4u << 4);
    RCC_PLLCFGR = (RCC_PLLCFGR & ~0xFu) | (3u << 2) | (1u << 16);
    RCC_PLL1DIVR = (n - 1u) | (1u << 9) | (1u << 16) | (1u << 24);
    RCC_CR |= (1u << 24); while (!(RCC_CR & (1u << 25))) {}
    RCC_CFGR = (RCC_CFGR & ~7u) | 3u; while ((RCC_CFGR & (7u << 3)) != (3u << 3)) {}
}
/* Direct SMPS supply (what the DISCO is wired for; PWR_CR3 already shows
 * SDEN=1/LDOEN=0 at reset on this board) then VOS1, the ceiling for SMPS. */
static void power_vos1(void) {
    PWR_CR3 = (PWR_CR3 & ~0x3Fu) | (1u << 2);
    while (!(PWR_CSR1 & (1u << 13))) {}
    PWR_D3CR = (PWR_D3CR & ~(3u << 14)) | (3u << 14);
    while (!(PWR_D3CR & (1u << 13))) {}
}
static volatile uint32_t sink;

/* ---- fixtures (embedded_memtable.rs shapes), one code path per vtable -- */
#define N_INGEST 2000u
#define N_CAN    500u
#define N_BLE    2000u
static const alt_ops *const impls[] = { &alt_expanse, &alt_sorted_array, &alt_open_hash, &alt_tsearch };
#define N_IMPLS (sizeof impls / sizeof impls[0])

static uint32_t fx_ingest(const alt_ops *ops, size_t *heap_delta, size_t *req_delta) {
    size_t h0 = heap_in_use(), r0 = acct_live();
    void *m = ops->create(N_INGEST);
    uint32_t t0 = cyc();
    for (uint32_t i = 0; i < N_INGEST; i++) ops->insert(m, 1700000000u + i, 42);
    uint32_t t = cyc() - t0;
    if (heap_delta) { *heap_delta = heap_in_use() - h0; *req_delta = acct_live() - r0; }
    uint32_t v = 0;
    if (!ops->get(m, 1700000000u + N_INGEST - 1, &v) || v != 42 || !ops->get(m, 1700000000u, &v)) uart_puts("CHECK ingest\r\n");
    ops->destroy(m);
    return t;
}

static uint32_t can_keys[N_CAN];
static uint32_t fx_can(const alt_ops *ops, void *m) {
    uint32_t sum = 0, v;
    uint32_t t0 = cyc();
    for (uint32_t i = 0; i < N_CAN; i++) if (ops->get(m, can_keys[i], &v)) sum += v;
    uint32_t t = cyc() - t0;
    sink = sum;
    if (sum != (N_CAN * (N_CAN - 1)) / 2) uart_puts("CHECK can sum\r\n");
    return t;
}

static uint32_t fnv1a(const uint8_t *d, size_t n) {
    uint32_t h = 2166136261u;
    for (size_t i = 0; i < n; i++) { h ^= d[i]; h *= 16777619u; }
    return h;
}
static uint8_t macs[N_BLE][6];
static void mac_of(uint32_t i, uint8_t *m) {
    m[0] = 0x00; m[1] = 0x1A; m[2] = 0x2B;
    m[3] = (uint8_t)(i >> 16); m[4] = (uint8_t)(i >> 8); m[5] = (uint8_t)i;
}
static uint32_t seen_bulk(uint32_t i) { return i * 10u; }
static uint32_t seen_steady(uint32_t i) { return i < 25 ? i * 10u : 10000u + i; }
typedef uint32_t (*seen_fn)(uint32_t);

/* Ordered structures keep the dual index (by_mac + by_time); an unordered
 * one keeps the single mac-keyed table and evicts by scanning it, exactly
 * like the host suite's HashMap arm. */
static void ble_build(const alt_ops *ops, void **by_mac, void **by_time, seen_fn seen) {
    *by_mac = ops->create(N_BLE);
    *by_time = ops->ordered ? ops->create(N_BLE) : 0;
    for (uint32_t i = 0; i < N_BLE; i++) {
        ops->insert(*by_mac, fnv1a(macs[i], 6), i);
        if (*by_time) ops->insert(*by_time, ((seen(i) / 1000u) << 13) | (i & 0x1FFF), i);
    }
}
typedef struct { const alt_ops *ops; void *by_mac; seen_fn seen; } evict_ctx;
static void evict_cb(uint32_t tk, uint32_t idx, void *p) {
    (void)tk; evict_ctx *c = p; c->ops->remove(c->by_mac, fnv1a(macs[idx], 6));
}
static bool expired_cb(uint32_t k, uint32_t idx, void *p) { (void)k; return ((evict_ctx *)p)->seen(idx) < 6000u; }

enum { EV_LOOP, EV_RANGE, EV_SCAN };
static uint32_t fx_evict(const alt_ops *ops, seen_fn seen, int mode, uint32_t *evicted_out) {
    void *by_mac, *by_time; ble_build(ops, &by_mac, &by_time, seen);
    const uint32_t max_tk = (5u << 13) | 0x1FFF;
    evict_ctx c = { ops, by_mac, seen };
    uint32_t evicted = 0, tk, idx;
    uint32_t t0 = cyc();
    if (mode == EV_SCAN) {
        evicted = (uint32_t)ops->evict_scan(by_mac, expired_cb, &c);
    } else if (mode == EV_RANGE) {
        evicted = (uint32_t)ops->remove_range(by_time, 0, max_tk, evict_cb, &c);
    } else {
        while (ops->first(by_time, &tk, &idx)) {
            if (tk > max_tk) break;
            ops->remove(by_time, tk);
            ops->remove(by_mac, fnv1a(macs[idx], 6));
            evicted++;
        }
    }
    uint32_t t = cyc() - t0;
    *evicted_out = evicted;
    uint32_t v;
    if (ops->get(by_mac, fnv1a(macs[N_BLE - 1], 6), &v) != true || v != N_BLE - 1) uart_puts("CHECK evict survivor\r\n");
    if (ops->get(by_mac, fnv1a(macs[0], 6), &v)) uart_puts("CHECK evict expired still present\r\n");
    ops->destroy(by_mac); if (by_time) ops->destroy(by_time);
    return t;
}

static void run_fixtures(uint32_t dcache, bool report_bytes) {
    for (size_t k = 0; k < N_IMPLS; k++) {
        const alt_ops *ops = impls[k];
        size_t hd = 0, rd = 0;
        for (uint32_t p = 0; p < PASSES; p++) result("ingest", ops->name, dcache, p, fx_ingest(ops, p ? 0 : &hd, p ? 0 : &rd), N_INGEST);
        if (report_bytes) {
            uart_puts("RESULT name=bytes"); ks("impl", ops->name); ks("shape", "ingest"); kv("keys", N_INGEST);
            kv("heap_bytes", hd); kv("req_bytes", rd); uart_puts("\r\n");
        }
        void *can = ops->create(N_CAN);
        for (uint32_t i = 0; i < N_CAN; i++) ops->insert(can, can_keys[i], i);
        for (uint32_t p = 0; p < PASSES; p++) result("can_dispatch", ops->name, dcache, p, fx_can(ops, can), N_CAN);
        ops->destroy(can);
        if (report_bytes) {
            size_t h0 = heap_in_use(), r0 = acct_live(); void *a, *b; ble_build(ops, &a, &b, seen_bulk);
            uart_puts("RESULT name=bytes"); ks("impl", ops->name); ks("shape", "ble_index"); kv("keys", N_BLE);
            kv("heap_bytes", heap_in_use() - h0); kv("req_bytes", acct_live() - r0); uart_puts("\r\n");
            ops->destroy(a); if (b) ops->destroy(b);
        }
        uint32_t ev;
        if (ops->ordered) {
            for (uint32_t p = 0; p < PASSES; p++) { uint32_t t = fx_evict(ops, seen_bulk, EV_LOOP, &ev); result("evict_bulk_loop", ops->name, dcache, p, t, ev); }
            for (uint32_t p = 0; p < PASSES; p++) { uint32_t t = fx_evict(ops, seen_bulk, EV_RANGE, &ev); result("evict_bulk_range", ops->name, dcache, p, t, ev); }
            for (uint32_t p = 0; p < PASSES; p++) { uint32_t t = fx_evict(ops, seen_steady, EV_LOOP, &ev); result("evict_steady_loop", ops->name, dcache, p, t, ev); }
            for (uint32_t p = 0; p < PASSES; p++) { uint32_t t = fx_evict(ops, seen_steady, EV_RANGE, &ev); result("evict_steady_range", ops->name, dcache, p, t, ev); }
        } else {
            for (uint32_t p = 0; p < PASSES; p++) { uint32_t t = fx_evict(ops, seen_bulk, EV_SCAN, &ev); result("evict_bulk_scan", ops->name, dcache, p, t, ev); }
            for (uint32_t p = 0; p < PASSES; p++) { uint32_t t = fx_evict(ops, seen_steady, EV_SCAN, &ev); result("evict_steady_scan", ops->name, dcache, p, t, ev); }
        }
    }
}

/* ---- sync32 ISR-reader arm vs critical-section twin ------------------ */
#define KEYS       4096u
#define KEYS_FIXED 2048u
#define ISR_PERIOD 20000u      /* core cycles between SysTick interrupts */
#define DUTY_BUDGET 200000000u /* cycles per (arm, duty) */
#define VAL(k) ((k) ^ 0xABCDu)
/* Writer mutation rates (per second at the current clock): full duty, then
 * paced with gaps jittered uniformly over [P/2, 3P/2) so the writer never
 * aliases with the fixed SysTick period (commensurate periods made the ISR
 * land only in the pacing spin: 0 BUSY, measured, not real). */
static const uint32_t rates[] = { 0, 40000, 10000, 1000 };
#define N_RATES (sizeof rates / sizeof rates[0])
static uint32_t wrng = 0x2545F491u;
static inline uint32_t gap_of(uint32_t period) {
    wrng ^= wrng << 13; wrng ^= wrng >> 17; wrng ^= wrng << 5;
    return period / 2u + (wrng % period);
}

enum { ISR_OFF = 0, ISR_SYNC32 = 1, ISR_CS = 2 };
static volatile uint32_t isr_mode;
static expanse_sync32_map_reader_t *g_reader;
static expanse_map_t *g_plain;
static volatile uint32_t isr_n, isr_ok, isr_nf, isr_busy, isr_bad;
static volatile uint32_t lat_max, lat_sum, dur_max, dur_sum;
static uint32_t rng = 0x9E3779B9u;

void SysTick_Handler(void) {
    uint32_t latency = (ISR_PERIOD - 1u) - SYST_CVR;
    uint32_t t0 = cyc();
    uint32_t mode = isr_mode;
    if (mode == ISR_OFF) return;
    rng ^= rng << 13; rng ^= rng >> 17; rng ^= rng << 5;
    uint32_t k = rng & (KEYS - 1u);
    expanse_word_t v;
    if (mode == ISR_SYNC32) {
        expanse_sync32_status_t st = expanse_sync32_map_reader_try_get(g_reader, k, &v);
        if (st == EXPANSE_SYNC32_OK) { if (v == VAL(k)) isr_ok++; else isr_bad++; }
        else if (st == EXPANSE_SYNC32_NOT_FOUND) isr_nf++;
        else if (st == EXPANSE_SYNC32_BUSY) isr_busy++;
        else isr_bad++;
    } else {
        if (expanse_map_get(g_plain, k, &v)) { if (v == VAL(k)) isr_ok++; else isr_bad++; }
        else isr_nf++;
    }
    uint32_t d = cyc() - t0;
    isr_n++;
    if (latency > lat_max) lat_max = latency;
    lat_sum += latency;
    if (d > dur_max) dur_max = d;
    dur_sum += d;
}
static void isr_reset(void) { isr_n = isr_ok = isr_nf = isr_busy = isr_bad = 0; lat_max = lat_sum = dur_max = dur_sum = 0; }
static void systick_start(void) { SYST_RVR = ISR_PERIOD - 1u; SYST_CVR = 0; SYST_CSR = 7u; }
static void systick_stop(void) { SYST_CSR = 0; isr_mode = ISR_OFF; }
static void isr_report(const char *arm, uint32_t period, uint32_t mutations, uint32_t writer_cycles, uint32_t refused, uint32_t arena_full) {
    uart_puts("RESULT name="); uart_puts(arm); kv("sysclk", sysclk_hz); kv("dcache", 1); kv("period", period);
    kv("writer_cycles", writer_cycles); kv("mutations", mutations);
    kv("isr_n", isr_n); kv("isr_ok", isr_ok); kv("isr_nf", isr_nf); kv("isr_busy", isr_busy); kv("isr_bad", isr_bad);
    kv("lat_max", lat_max); kv("lat_sum", lat_sum); kv("dur_max", dur_max); kv("dur_sum", dur_sum);
    kv("refused", refused); kv("arena_full", arena_full);
    uart_puts("\r\n");
}
static uint32_t period_of(uint32_t rate) { return rate ? sysclk_hz / rate : 0; }

static void arm_sync32(void) {
    expanse_sync32_map_t *map = expanse_sync32_map_new(KEYS, 1);
    if (!map) { uart_puts("CHECK sync32 new\r\n"); return; }
    expanse_sync32_map_writer_t *w = expanse_sync32_map_writer(map);
    g_reader = expanse_sync32_map_reader(map, 0);
    bool rep; expanse_word_t old;
    for (uint32_t k = 0; k < KEYS_FIXED; k++)
        if (expanse_sync32_map_writer_try_insert(w, k, VAL(k), &rep, &old) != EXPANSE_SYNC32_OK) uart_puts("CHECK sync32 prefill\r\n");
    for (size_t d = 0; d < N_RATES; d++) {
        uint32_t period = period_of(rates[d]);
        isr_reset(); isr_mode = ISR_SYNC32; systick_start();
        uint32_t refused = 0, arena_full = 0, i = 0;
        uint32_t t0 = cyc(), next = t0;
        while (cyc() - t0 < DUTY_BUDGET) {
            if (period) { while ((int32_t)(cyc() - next) < 0) {} next += gap_of(period); }
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
            i++;
        }
        uint32_t t = cyc() - t0;
        systick_stop();
        isr_report("isr_sync32", period, i, t, refused, arena_full);
        for (uint32_t k = KEYS_FIXED; k < KEYS; k++) expanse_sync32_map_writer_try_remove(w, k, &old);
        expanse_sync32_map_writer_try_reclaim(w);
    }
    expanse_sync32_stats_t s; expanse_sync32_map_writer_stats(w, &s, sizeof s);
    uart_puts("INFO sync32 len="); uart_u32((uint32_t)s.len); kv("mem_used", s.mem_used); kv("pending", s.pending_len); kv("free_slots", s.free_slots); uart_puts("\r\n");
    expanse_sync32_map_free(map);
}

static void arm_critical_section(void) {
    g_plain = expanse_map_new();
    expanse_word_t old;
    for (uint32_t k = 0; k < KEYS_FIXED; k++) expanse_map_insert(g_plain, k, VAL(k), &old);
    for (size_t d = 0; d < N_RATES; d++) {
        uint32_t period = period_of(rates[d]);
        isr_reset(); isr_mode = ISR_CS; systick_start();
        uint32_t i = 0, t0 = cyc(), next = t0;
        while (cyc() - t0 < DUTY_BUDGET) {
            if (period) { while ((int32_t)(cyc() - next) < 0) {} next += gap_of(period); }
            uint32_t k = KEYS_FIXED + ((i >> 1) & (KEYS_FIXED - 1u));
            __asm volatile("cpsid i" ::: "memory");
            if (i & 1u) expanse_map_remove(g_plain, k, &old); else expanse_map_insert(g_plain, k, VAL(k), &old);
            __asm volatile("cpsie i" ::: "memory");
            i++;
        }
        uint32_t t = cyc() - t0;
        systick_stop();
        isr_report("isr_critical_section", period, i, t, 0, 0);
        for (uint32_t k = KEYS_FIXED; k < KEYS; k++) expanse_map_remove(g_plain, k, &old);
    }
    expanse_map_free(g_plain); g_plain = 0;
}

int main(void) {
    uart_init();
    dwt_init();
    icache_enable();
    uart_puts("\r\nEXPANSE stm32h747 m7 harness\r\n");
    uart_puts("INFO cpuid="); uart_hex(SCB_CPUID); uart_puts(" idcode="); uart_hex(DBGMCU_IDCODE);
    uart_puts(" pwr_cr3="); uart_hex(PWR_CR3);
    SCB_CSSELR = 0; __asm volatile("dsb" ::: "memory");
    uint32_t ccsidr = SCB_CCSIDR;
    kv("dcache_line_bytes", 1u << ((ccsidr & 7u) + 4u));
    kv("dcache_ways", ((ccsidr >> 3) & 0x3FF) + 1u); kv("dcache_sets", ((ccsidr >> 13) & 0x7FFF) + 1u);
    kv("sync32_headroom", (uint32_t)expanse_sync32_mutation_headroom());
    uart_puts("\r\n");
    for (uint32_t i = 0; i < N_CAN; i++) can_keys[i] = (i * 100007u) & 0x1FFFFFFFu;
    for (uint32_t i = 0; i < N_BLE; i++) mac_of(i, macs[i]);

    calibrate();
    run_fixtures(0, true);
    dcache_enable();
    run_fixtures(1, false);

    clock_pll(20, 0x8u, 0u); sysclk_hz = 160000000u;          /* VOS3: 160 MHz, HCLK 80 */
    uart_puts("INFO sysclk=160000000 hclk=80000000 vos=3\r\n");
    calibrate();
    run_fixtures(1, false);
    dcache_disable();
    run_fixtures(0, false);

    power_vos1();
    clock_pll(50, 0x8u, 0x4u); sysclk_hz = 400000000u;        /* VOS1: 400 MHz, HCLK 200, APB 100 */
    uart_puts("INFO sysclk=400000000 hclk=200000000 vos=1\r\n");
    calibrate();
    run_fixtures(0, false);
    dcache_enable();
    run_fixtures(1, false);

    arm_sync32();
    arm_critical_section();
    uart_puts("INFO heap_hwm="); uart_u32((uint32_t)(brk_hwm - &_heap_start)); uart_puts("\r\n");
    uart_puts("DONE\r\n");
    for (;;) { sink++; }  /* spin, not WFI: keeps SWD attachable */
}
