/* Expanse "two lanes" LCD demo — STM32H747I-DISCO, Cortex-M7 (#605).
 *
 * Built to the reviewed mockup (docs/benchmarks/stm32h747/README.md, "Demo").
 * Each lane's 1 kHz interrupt reads its table and records the outcome into a
 * time ring; it draws nothing. The main loop draws everything from the rings
 * and the counters: a heartbeat strip per lane (one column per 10.6 ms of the
 * last 4 s, swept left to right; a blocked millisecond takes precedence in
 * its column), four numbers per lane in plain words (blocked ms, no-value %,
 * max stale ms, wrong reads), the sweep cost, and a memory/lookup ledger
 * measured on the demo's own objects. A program of steps advances on its own
 * (prologue, five masking/rate steps, a growth step, a summary frame); the
 * blue user button skips ahead. Every number on screen is measured in this
 * run; the RESULT lines on the VCP carry the same counters once a second.
 *
 * Board configuration (clock, MPU, UART, DMA2D, DSI/LTDC) is the prior
 * working set for this board (docs/NT35510_LCD_SETUP.md). Demo, not
 * benchmark: measured claims live in docs/benchmarks/stm32h747. */
#include "main.h"
#include <errno.h>
#include <stdio.h>
#include <string.h>
#include "stm32h747i_discovery.h"
#include "stm32h747i_discovery_lcd.h"
#include "stm32h747i_discovery_sdram.h"
#include "gfx.h"
#include "lanes.h"

#define FRAME_BUFFER_ADDR (SDRAM_DEVICE_ADDR)              /* 0xD0000000, normal non-cacheable */
#define HEAP_START        (SDRAM_DEVICE_ADDR + 0x1000000U) /* 0xD1000000, 16 MB, cacheable write-back */
#define HEAP_END          (SDRAM_DEVICE_ADDR + 0x2000000U)

#define TARGET_RECORDS 100000u
#define INGEST_PER_S   (TARGET_RECORDS / (TTL_MS / 1000u))   /* steady state: what expires is re-added */
#define GROWTH_PER_S   5000u

DMA2D_HandleTypeDef hdma2d;
UART_HandleTypeDef huart1;
extern LTDC_HandleTypeDef hlcd_ltdc;
typedef enum { LCD_CTRL_NT35510, LCD_CTRL_OTM8009A } LCD_Driver_t;
extern LCD_Driver_t Lcd_Driver_Type;

void SystemClock_Config(void);
void PeriphCommonClock_Config(void);
static void MPU_Config(void);
static void MX_GPIO_Init(void);
static void MX_USART1_UART_Init(void);
static void MX_DMA2D_Init(void);
static void timers_init(void);

/* ---- heap in SDRAM (both lanes, symmetric), host hooks for libexpanse ------ */
static uint8_t *brk = (uint8_t *)HEAP_START;
void *_sbrk(ptrdiff_t incr) {
    if (brk + incr > (uint8_t *)HEAP_END) { errno = ENOMEM; return (void *)-1; }
    void *p = brk; brk += incr; return p;
}
void *expanse_host_malloc(size_t n) { return acct_malloc(n); }
void expanse_host_free(void *p) { acct_free(p); }

/* ---- the program ----------------------------------------------------------------- */
typedef struct { const char *name; uint32_t hz, mode, secs; bool growth; const char *watch; } step_t;
static const step_t PROG[] = {
    { "PROLOGUE", 0,  HASH_MASK_WHOLE,     6,  false, "both reader interrupts run every millisecond; no sweep yet" },
    { "STEP 1",   1,  HASH_MASK_WHOLE,     20, false, "once a second the hash table sweeps; its reader is masked meanwhile" },
    { "STEP 2",   5,  HASH_MASK_WHOLE,     20, false, "five sweeps a second" },
    { "STEP 3",   10, HASH_MASK_WHOLE,     20, false, "ten sweeps a second: the hash reader is blocked half the time" },
    { "STEP 4",   10, HASH_MASK_PER_WRITE, 20, false, "the competent hash firmware masks only around each write: no gaps expected" },
    { "STEP 5",   10, HASH_UNMASKED,       20, false, "no mask at all: no gaps, but the hash reader can read a wrong value" },
    { "STEP 6",   10, HASH_MASK_PER_WRITE, 20, true,  "filling both tables from empty at 5,000/s; each hash-table doubling is one write" },
    { "SUMMARY",  0,  HASH_MASK_WHOLE,     0,  false, "" },
};
#define N_STEPS (sizeof PROG / sizeof PROG[0])
#define STEP_SUMMARY (N_STEPS - 1)
typedef struct { uint32_t a_blocked, a_stale, a_wrong, a_busy, b_blocked, b_stale, b_wrong; bool filled; } ledger_t;
static ledger_t ledger[N_STEPS];

/* ---- layout: the row budget shared with the mockup (px) ------------------------- */
enum { STRIP_Y = 0, STRIP_H = 44, HEAD_Y = 50, HEAD_H = 58, BEAT_Y = 112, BEAT_H = 196, NUMS_Y = 318, NUMS_H = 84,
       SWEEP_Y = 406, SWEEP_H = 24, SCORE_Y = 432, SCORE_H = 24, FOOT_Y = 460, FOOT_H = 20 };
static const int LANE_X[2] = { 6, 406 };
#define LANE_W 388
#define BAND_X(l)  (LANE_X[(l) == &lane_b] + 6)
#define BAND_W     376
#define BAND_Y     (BEAT_Y + 30)
#define BAND_H     (BEAT_H - 64)
#define WINDOW_MS  4000u

void gfx_overflow(const char *s, int width, int max_w) {
    printf("CHECK text overflow: %d px in %d: %s\r\n", width, max_w, s);
}
static void mode_line(const lane_t *l, const step_t *p, char *b, size_t n) {
    if (p->growth) {
        if (l->is_expanse) { snprintf(b, n, "grows node by node: no rehash"); return; }
        if (!l->rehash_n) { snprintf(b, n, "table doubles at 50%% load"); return; }
        int k = snprintf(b, n, "doublings cost");
        for (uint32_t i = 0; i < l->rehash_n && k < (int)n - 8; i++) k += snprintf(b + k, n - k, "%s%lu", i ? " \xB7 " : " ", (unsigned long)l->rehash_ms[i]);
        snprintf(b + k, n - k, " ms");
        return;
    }
    if (l->is_expanse) snprintf(b, n, "reader interrupt: never blocked");
    else snprintf(b, n, "reader interrupt: %s", l->mode == HASH_MASK_WHOLE ? "masked in the scan" : l->mode == HASH_MASK_PER_WRITE ? "masked per write" : "never masked");
}
/* the panel font has no middle dot: draw it as a plain dot */
static void fix_dots(char *s) { for (; *s; s++) if ((unsigned char)*s == 0xB7) *s = '.'; }

static void draw_strip(uint32_t step, uint32_t since_ms, uint32_t records) {
    const step_t *p = &PROG[step];
    char b[96];
    fill_rect(0, STRIP_Y, LCD_W, STRIP_H, C_PANEL);
    const char *mode = p->mode == HASH_MASK_WHOLE ? "hash reader masked in the scan" : p->mode == HASH_MASK_PER_WRITE ? "hash reader masked per write" : "hash reader not masked";
    if (step == 0) snprintf(b, sizeof b, "PROLOGUE . no sweeps yet");
    else if (step == STEP_SUMMARY) snprintf(b, sizeof b, "SUMMARY . what this run measured");
    else if (p->growth) snprintf(b, sizeof b, "%s / %u . growth . %s", p->name, (unsigned)(N_STEPS - 2), mode);
    else snprintf(b, sizeof b, "%s / %u . %lu sweep%s/s . %s", p->name, (unsigned)(N_STEPS - 2), (unsigned long)p->hz, p->hz > 1 ? "s" : "", mode);
    text_f(&cond24, 10, STRIP_Y + 3, b, C_TEXT, C_PANEL, p->growth ? 570 : 720, ALIGN_LEFT);   /* the growth readout on the right is wider */
    if (step != STEP_SUMMARY) {
        if (p->growth) snprintf(b, sizeof b, "%6lu records . %2lu s", (unsigned long)records, (unsigned long)(since_ms / 1000u));
        else snprintf(b, sizeof b, "%2lu s", (unsigned long)(since_ms / 1000u));
        text_f(&mono16, LCD_W - 10, STRIP_Y + 8, b, C_MUTED, C_PANEL, 220, ALIGN_RIGHT);
        fill_rect(0, STRIP_Y + STRIP_H - 3, LCD_W, 3, C_GRID);
        if (p->secs) fill_rect(0, STRIP_Y + STRIP_H - 3, (int)((uint64_t)LCD_W * (since_ms < p->secs * 1000u ? since_ms : p->secs * 1000u) / (p->secs * 1000u)), 3, C_MUTED);
        if (p->watch[0] && since_ms < 6000u) text_f(&mono16, 10, STRIP_Y + 26, p->watch, C_TEXT, C_PANEL, 780, ALIGN_LEFT);
    }
}
static void draw_head(lane_t *l, const step_t *p) {
    int x = LANE_X[l == &lane_b]; char b[64];
    fill_rect(x, HEAD_Y, LANE_W, HEAD_H, C_BG);
    text_f(&cond32, x + 6, HEAD_Y, l->name, l->is_expanse ? C_BLUE : C_AMBER, C_BG, 200, ALIGN_LEFT);
    mode_line(l, p, b, sizeof b); fix_dots(b);
    text_f(&mono16, x + 6, HEAD_Y + 36, b, C_MUTED, C_BG, LANE_W - 12, ALIGN_LEFT);
}
static void draw_beat_frame(lane_t *l, const step_t *p) {
    int x = LANE_X[l == &lane_b], bx = BAND_X(l); char b[64];
    fill_rect(x, BEAT_Y, LANE_W, BEAT_H, C_PANEL);
    text_f(&mono16, x + 6, BEAT_Y + 4, "did the reader interrupt run? last 4 s", C_MUTED, C_PANEL, LANE_W - 12, ALIGN_LEFT);
    fill_rect(bx, BAND_Y, BAND_W, BAND_H, C_BG);
    /* legend: only the outcomes this step can produce, or has */
    int lx = x + 6, ly = BEAT_Y + BEAT_H - 22;
    struct { uint32_t c; const char *s; bool on; } items[4] = {
        { l->is_expanse ? C_BLUE : C_AMBER, "value", true },
        { C_CYAN, "BUSY", l->is_expanse },
        { C_RED, "blocked", !l->is_expanse ? (p->mode != HASH_UNMASKED || l->blocked_ms) : l->blocked_ms != 0 },
        { C_MAGENTA, "wrong", (!l->is_expanse && p->mode == HASH_UNMASKED) || l->isr_bad || l->isr_nf },
    };
    for (int i = 0; i < 4; i++) {
        if (!items[i].on) continue;
        fill_rect(lx, ly + 4, 12, 12, items[i].c);
        snprintf(b, sizeof b, "%s", items[i].s);
        lx += 18 + text_f(&mono16, lx + 18, ly, b, C_MUTED, C_PANEL, 120, ALIGN_LEFT) + 22;
    }
}
/* one strip column per 10.64 ms slot of the 4 s window, swept left to right;
 * a held millisecond takes precedence, then wrong, then BUSY, then value */
static uint32_t band_next_ms[2];
static void draw_beat_columns(lane_t *l, uint32_t now_ms) {
    int li = l == &lane_b, bx = BAND_X(l);
    uint32_t accent = l->is_expanse ? C_BLUE : C_AMBER;
    if (band_next_ms[li] == 0) band_next_ms[li] = now_ms;
    for (uint32_t t = band_next_ms[li]; t + 1u < now_ms; ) {
        uint32_t col = (t % WINDOW_MS) * BAND_W / WINDOW_MS;
        uint32_t t_end = ((col + 1u) * WINDOW_MS + BAND_W - 1u) / BAND_W + (t / WINDOW_MS) * WINDOW_MS;   /* first ms of the next column */
        if (t_end > now_ms - 1u) break;                                                                   /* column not complete yet */
        bool held = false, wrong = false, busy = false, value = false;
        for (uint32_t s = t; s < t_end; s++) {
            uint8_t v = l->ring[s & (RING_LEN - 1u)];
            if (v == SLOT_HELD) held = true; else if (v == SLOT_WRONG) wrong = true; else if (v == SLOT_BUSY) busy = true; else if (v == SLOT_VALUE) value = true;
        }
        uint32_t c = held ? C_RED : wrong ? C_MAGENTA : busy ? C_CYAN : value ? accent : C_BG;
        fill_rect(bx + (int)col, BAND_Y, 1, BAND_H, c);
        fill_rect(bx + (int)((col + 1u) % BAND_W), BAND_Y, 1, BAND_H, C_GRID);                          /* faint cursor: a frame grab reads the live buffer and would smear a bright one */
        t = t_end;
        band_next_ms[li] = t;
    }
}
static void draw_nums(lane_t *l) {
    int x = LANE_X[l == &lane_b], cw = LANE_W / 4; char b[16];
    fill_rect(x, NUMS_Y, LANE_W, NUMS_H, C_PANEL);
    uint32_t ticks = l->isr_served + l->blocked_ms;
    uint32_t pct10 = ticks ? (uint32_t)((uint64_t)l->no_value_ms * 1000u / ticks) : 0;     /* tenths of a percent */
    uint32_t stale = l->stale_max ? l->stale_max + 1u : 1u;
    uint32_t wrong = l->isr_bad + l->isr_nf;
    const char *labels[4] = { "BLOCKED ms", "NO VALUE %", "STALE ms", "WRONG" };
    uint32_t cols[4] = { l->blocked_ms ? C_RED : C_GREEN, l->no_value_ms ? (l->is_expanse ? C_CYAN : C_RED) : C_GREEN, stale > 2 ? C_RED : C_GREEN, wrong ? C_MAGENTA : C_GREEN };
    for (int i = 0; i < 4; i++) {
        text_f(&mono16, x + 4 + i * cw, NUMS_Y + 6, labels[i], C_MUTED, C_PANEL, cw - 4, ALIGN_LEFT);
        if (i == 0) snprintf(b, sizeof b, "%lu", (unsigned long)l->blocked_ms);
        else if (i == 1) { if (pct10 >= 100) snprintf(b, sizeof b, "%lu", (unsigned long)(pct10 / 10u)); else snprintf(b, sizeof b, "%lu.%lu", (unsigned long)(pct10 / 10u), (unsigned long)(pct10 % 10u)); }
        else if (i == 2) snprintf(b, sizeof b, "%lu", (unsigned long)stale);
        else snprintf(b, sizeof b, "%lu", (unsigned long)wrong);
        text_f(&cond40, x + 4 + i * cw, NUMS_Y + 26, b, cols[i], C_PANEL, cw - 4, ALIGN_LEFT);
    }
}
static void draw_sweep(lane_t *l, const step_t *p) {
    int x = LANE_X[l == &lane_b]; char b[64];
    fill_rect(x, SWEEP_Y, LANE_W, SWEEP_H, C_BG);
    if (p->hz) snprintf(b, sizeof b, "sweep %lu.%lu ms %s . %lu%% core", (unsigned long)(l->sweep_us / 1000u), (unsigned long)((l->sweep_us / 100u) % 10u), l->is_expanse ? "remove_range" : "full scan", (unsigned long)(l->sweep_us * p->hz / 10000u));
    else snprintf(b, sizeof b, "sweep: none yet");
    text_f(&mono16, x + 8, SWEEP_Y + 3, b, C_TEXT, C_BG, LANE_W - 16, ALIGN_LEFT);
}
static void draw_score(lane_t *l) {
    int x = LANE_X[l == &lane_b]; char b[40];
    lane_t *o = l == &lane_a ? &lane_b : &lane_a;
    fill_rect(x, SCORE_Y, LANE_W, SCORE_H, C_BG);
    uint32_t bpr10 = l->count ? (uint32_t)((uint64_t)l->index_bytes * 10u / l->count) : 0;
    uint32_t obpr10 = o->count ? (uint32_t)((uint64_t)o->index_bytes * 10u / o->count) : 0;
    bool low_b = bpr10 && obpr10 && bpr10 < obpr10, low_n = l->lookup_ns && o->lookup_ns && l->lookup_ns < o->lookup_ns;
    snprintf(b, sizeof b, "%lu.%lu B/record%s", (unsigned long)(bpr10 / 10u), (unsigned long)(bpr10 % 10u), low_b ? " v" : "");
    text_f(&mono16, x + 8, SCORE_Y + 3, b, low_b ? C_GREEN : C_TEXT, C_BG, LANE_W / 2 - 12, ALIGN_LEFT);
    snprintf(b, sizeof b, "%lu ns/lookup%s", (unsigned long)l->lookup_ns, low_n ? " v" : "");
    text_f(&mono16, x + LANE_W - 8, SCORE_Y + 3, b, low_n ? C_GREEN : C_TEXT, C_BG, LANE_W / 2 - 12, ALIGN_RIGHT);
}
static void draw_foot(const step_t *p) {
    fill_rect(0, FOOT_Y, LCD_W, FOOT_H, C_BG);
    text_f(&mono16, 8, FOOT_Y + 1, p->growth ? "demo, not benchmark . each doubling timed by DWT . docs/benchmarks/stm32h747"
                                              : "demo, not benchmark . 100,000 records/lane . measured claims: docs/benchmarks/stm32h747", C_MUTED, C_BG, 784, ALIGN_LEFT);
}
static void draw_summary(void) {
    char b[48];
    fill_rect(0, HEAD_Y, LCD_W, FOOT_Y - HEAD_Y, C_BG);
    static const int cx[9] = { 10, 90, 170, 300, 385, 465, 560, 645, 725 };
    text_f(&cond24, cx[3], HEAD_Y + 2, "EXPANSE (ms)", C_BLUE, C_BG, 250, ALIGN_LEFT);
    text_f(&cond24, cx[6], HEAD_Y + 2, "HASH TABLE (ms)", C_AMBER, C_BG, 250, ALIGN_LEFT);
    static const char *hdr[9] = { "step", "sweeps/s", "hash mask", "blocked", "stale", "wrong", "blocked", "stale", "wrong" };
    for (int i = 0; i < 9; i++) text_f(&mono16, cx[i], HEAD_Y + 30, hdr[i], C_MUTED, C_BG, (i < 8 ? cx[i + 1] : (int)LCD_W - 10) - cx[i] - 6, ALIGN_LEFT);
    fill_rect(10, HEAD_Y + 50, LCD_W - 20, 1, C_GRID);
    int row = 0;
    for (uint32_t s = 1; s < STEP_SUMMARY; s++) {
        if (!ledger[s].filled) continue;
        const ledger_t *r = &ledger[s]; const step_t *p = &PROG[s];
        int yy = HEAD_Y + 60 + row++ * 32;
        const char *vals[9]; char cells[9][16];
        snprintf(cells[0], 16, "%s", p->name); snprintf(cells[1], 16, "%lu", (unsigned long)p->hz);
        snprintf(cells[2], 16, "%s", p->growth ? "growth" : p->mode == HASH_MASK_WHOLE ? "in scan" : p->mode == HASH_MASK_PER_WRITE ? "per write" : "none");
        snprintf(cells[3], 16, "%lu", (unsigned long)r->a_blocked); snprintf(cells[4], 16, "%lu", (unsigned long)r->a_stale); snprintf(cells[5], 16, "%lu", (unsigned long)r->a_wrong);
        snprintf(cells[6], 16, "%lu", (unsigned long)r->b_blocked); snprintf(cells[7], 16, "%lu", (unsigned long)r->b_stale); snprintf(cells[8], 16, "%lu", (unsigned long)r->b_wrong);
        uint32_t col[9] = { C_TEXT, C_TEXT, C_TEXT, r->a_blocked ? C_RED : C_GREEN, r->a_stale > 2 ? C_RED : C_GREEN, r->a_wrong ? C_MAGENTA : C_GREEN,
                            r->b_blocked ? C_RED : C_GREEN, r->b_stale > 2 ? C_RED : C_GREEN, r->b_wrong ? C_MAGENTA : C_GREEN };
        for (int i = 0; i < 9; i++) { vals[i] = cells[i]; text_f(&mono16, cx[i], yy, vals[i], col[i], C_BG, (i < 8 ? cx[i + 1] : (int)LCD_W - 10) - cx[i] - 6, ALIGN_LEFT); }
    }
    uint32_t a10 = lane_a.count ? (uint32_t)((uint64_t)lane_a.index_bytes * 10u / lane_a.count) : 0, b10 = lane_b.count ? (uint32_t)((uint64_t)lane_b.index_bytes * 10u / lane_b.count) : 0;
    char line[96];
    snprintf(line, sizeof line, "memory %lu.%lu vs %lu.%lu B/record (Expanse incl. by-time index) . lookup %lu vs %lu ns",
             (unsigned long)(a10 / 10), (unsigned long)(a10 % 10), (unsigned long)(b10 / 10), (unsigned long)(b10 % 10), (unsigned long)lane_a.lookup_ns, (unsigned long)lane_b.lookup_ns);
    text_f(&mono16, 10, SCORE_Y - 14, line, C_MUTED, C_BG, 780, ALIGN_LEFT);
    (void)b;
    text_f(&mono16, 10, SCORE_Y + 6, "every value on this screen was measured in this run", C_MUTED, C_BG, 780, ALIGN_LEFT);
}

static void begin_step(uint32_t step, uint32_t now_ms) {
    const step_t *p = &PROG[step];
    lane_b.mode = p->mode;
    lane_a.growable = lane_b.growable = p->growth;
    if (p->growth) {                                   /* both lanes rebuilt empty; the interrupts keep reading the tracked ids */
        NVIC_DisableIRQ(TIM6_DAC_IRQn); NVIC_DisableIRQ(TIM7_IRQn);
        if (!lane_fill(&lane_a, 0, now_ms) || !lane_fill(&lane_b, 0, now_ms)) printf("CHECK growth rebuild\r\n");
        NVIC_EnableIRQ(TIM6_DAC_IRQn); NVIC_EnableIRQ(TIM7_IRQn);
    }
    __disable_irq();                                     /* a few microseconds: no tick may land between the reset and the re-arm */
    lane_step_reset(&lane_a); lane_step_reset(&lane_b);
    /* the next tick's exact due time, from each lane timer's remaining count (same 20 MHz tick as TIM2) */
    lane_a.expected_cnt = TIM2->CNT + (TIM6->ARR - TIM6->CNT + 1u);
    lane_b.expected_cnt = TIM2->CNT + (TIM7->ARR - TIM7->CNT + 1u);
    __enable_irq();
    fill_rect(0, 0, LCD_W, LCD_H, C_BG);
    draw_strip(step, 0, lane_a.count);
    if (step == STEP_SUMMARY) { draw_summary(); }
    else for (lane_t *l = &lane_a; l; l = l == &lane_a ? &lane_b : 0) { draw_head(l, p); draw_beat_frame(l, p); draw_nums(l); draw_sweep(l, p); draw_score(l); }
    draw_foot(p);
    band_next_ms[0] = band_next_ms[1] = 0;
    printf("INFO step=%lu name=%s sweep_hz=%lu mode=%lu growth=%d\r\n", (unsigned long)step, p->name, (unsigned long)p->hz, (unsigned long)p->mode, p->growth);
}
static void end_step(uint32_t step) {
    if (step == 0 || step >= STEP_SUMMARY) return;
    ledger[step] = (ledger_t){ lane_a.blocked_ms, lane_a.stale_max ? lane_a.stale_max + 1 : 1, lane_a.isr_bad + lane_a.isr_nf, lane_a.isr_busy,
                               lane_b.blocked_ms, lane_b.stale_max ? lane_b.stale_max + 1 : 1, lane_b.isr_bad + lane_b.isr_nf, true };
}

int main(void) {
    MPU_Config();
    SCB_EnableICache();
    SCB_EnableDCache();
    HAL_Init();
    SystemClock_Config();
    PeriphCommonClock_Config();
    MX_GPIO_Init();
    MX_USART1_UART_Init();
    MX_DMA2D_Init();
    CoreDebug->DEMCR |= CoreDebug_DEMCR_TRCENA_Msk; DWT->LAR = 0xC5ACCE55; DWT->CYCCNT = 0; DWT->CTRL |= 1;
    if (BSP_SDRAM_Init(0) != BSP_ERROR_NONE) { for (;;) { __NOP(); } }
    printf("\r\nEXPANSE stm32h747 lcd demo (two lanes, to the reviewed mockup)\r\n");
    if (BSP_LCD_InitEx(0, LCD_ORIENTATION_LANDSCAPE, LCD_PIXEL_FORMAT_RGB888, LCD_W, LCD_H) != BSP_ERROR_NONE) { printf("[FAIL] LCD init\r\n"); Error_Handler(); }
    BSP_PB_Init(BUTTON_WAKEUP, BUTTON_MODE_GPIO);
    gfx_init(FRAME_BUFFER_ADDR);
    fill_rect(0, 0, LCD_W, LCD_H, C_BG);
    text_f(&cond24, 12, 10, "loading 100,000 records into both lanes...", C_TEXT, C_BG, 700, ALIGN_LEFT);
    BSP_LCD_SetLayerAddress(0, 0, FRAME_BUFFER_ADDR);
    BSP_LCD_DisplayOn(0);
    BSP_LCD_SetBrightness(0, 100);

    uint32_t t0 = HAL_GetTick();
    if (!lanes_init() || !lane_fill(&lane_a, TARGET_RECORDS, t0) || !lane_fill(&lane_b, TARGET_RECORDS, t0)) { printf("[FAIL] lanes init\r\n"); Error_Handler(); }
    printf("[OK] lanes: %lu records each, prefill %lu ms, heap %lu KB\r\n", (unsigned long)lane_a.count, (unsigned long)(HAL_GetTick() - t0), (unsigned long)((brk - (uint8_t *)HEAP_START) / 1024u));
    timers_init();
    printf("INFO nvic tim7(hash)=%lu tim6(expanse)=%lu systick=%lu ltdc=%lu\r\n", (unsigned long)NVIC_GetPriority(TIM7_IRQn), (unsigned long)NVIC_GetPriority(TIM6_DAC_IRQn),
           (unsigned long)NVIC_GetPriority(SysTick_IRQn), (unsigned long)NVIC_GetPriority(LTDC_IRQn));
    uint32_t step = 0, start_ms = HAL_GetTick(), step_start = start_ms, last_sweep = start_ms, last_stats = start_ms, last_log = start_ms + 500u, last_second = start_ms, btn_was = 0;
    lane_a.last_churn_ms = lane_b.last_churn_ms = start_ms;
    begin_step(0, start_ms);
    for (;;) {
        uint32_t now = HAL_GetTick(); const step_t *p = &PROG[step];
        uint32_t since = now - step_start;
        uint32_t btn = BSP_PB_GetState(BUTTON_WAKEUP);
        bool advance = (btn && !btn_was) || (p->secs && since >= p->secs * 1000u);
        btn_was = btn;
        if (advance && step < STEP_SUMMARY) { end_step(step); step++; step_start = now; begin_step(step, now); continue; }
        if (step == STEP_SUMMARY) { if (btn && !btn_was) {} continue; }
        if (p->growth) { lane_ingest(&lane_a, now, GROWTH_PER_S, TARGET_RECORDS); lane_ingest(&lane_b, now, GROWTH_PER_S, TARGET_RECORDS); }
        else if (step > 0) { lane_ingest(&lane_a, now, INGEST_PER_S, UINT32_MAX); lane_ingest(&lane_b, now, INGEST_PER_S, UINT32_MAX); }
        lane_churn(&lane_a, now); lane_churn(&lane_b, now);
        if (p->hz && now - last_sweep >= 1000u / p->hz) { last_sweep = now; lane_sweep(&lane_a, now); lane_sweep(&lane_b, now); draw_sweep(&lane_a, p); draw_sweep(&lane_b, p); }
        draw_beat_columns(&lane_a, now); draw_beat_columns(&lane_b, now);
        if (now - last_stats >= 200u) {
            last_stats = now;
            lane_measure_scoreboard(&lane_a); lane_measure_scoreboard(&lane_b);
            draw_nums(&lane_a); draw_nums(&lane_b); draw_score(&lane_a); draw_score(&lane_b);
            if (p->growth) { draw_head(&lane_b, p); }
        }
        if (now - last_second >= 1000u) { last_second = now; draw_strip(step, since, lane_a.count); if (since >= 6000u && since < 7000u) draw_beat_frame(&lane_a, p), draw_beat_frame(&lane_b, p), band_next_ms[0] = band_next_ms[1] = 0; }
        if (now - last_log >= 1000u) {
            last_log = now;
            printf("RESULT t=%lu step=%lu sweep_hz=%lu mode=%lu growth=%d "
                   "a_records=%lu a_sweep_us=%lu a_sweep_net_us=%lu a_lat_max_ns=%lu a_blocked_ms=%lu a_served=%lu a_ok=%lu a_busy=%lu a_nf=%lu a_bad=%lu a_no_value_ms=%lu a_stale_max_ms=%lu a_body_max_cyc=%lu a_drops=%lu a_bytes=%lu a_lookup_ns=%lu "
                   "b_records=%lu b_sweep_us=%lu b_sweep_net_us=%lu b_lat_max_ns=%lu b_blocked_ms=%lu b_served=%lu b_ok=%lu b_nf=%lu b_bad=%lu b_no_value_ms=%lu b_stale_max_ms=%lu b_body_max_cyc=%lu b_mask_max_us=%lu b_drops=%lu b_bytes=%lu b_lookup_ns=%lu b_rehashes=%lu\r\n",
                   (unsigned long)(now - start_ms), (unsigned long)step, (unsigned long)p->hz, (unsigned long)lane_b.mode, p->growth,
                   (unsigned long)lane_a.count, (unsigned long)lane_a.sweep_us, (unsigned long)lane_a.sweep_net_us, (unsigned long)(lane_a.lat_max_ticks * 50u), (unsigned long)lane_a.blocked_ms,
                   (unsigned long)lane_a.isr_served, (unsigned long)lane_a.isr_ok, (unsigned long)lane_a.isr_busy, (unsigned long)lane_a.isr_nf, (unsigned long)lane_a.isr_bad,
                   (unsigned long)lane_a.no_value_ms, (unsigned long)(lane_a.stale_max ? lane_a.stale_max + 1 : 1), (unsigned long)lane_a.body_max_cyc, (unsigned long)lane_a.drops, (unsigned long)lane_a.index_bytes, (unsigned long)lane_a.lookup_ns,
                   (unsigned long)lane_b.count, (unsigned long)lane_b.sweep_us, (unsigned long)lane_b.sweep_net_us, (unsigned long)(lane_b.lat_max_ticks * 50u), (unsigned long)lane_b.blocked_ms,
                   (unsigned long)lane_b.isr_served, (unsigned long)lane_b.isr_ok, (unsigned long)lane_b.isr_nf, (unsigned long)lane_b.isr_bad,
                   (unsigned long)lane_b.no_value_ms, (unsigned long)(lane_b.stale_max ? lane_b.stale_max + 1 : 1), (unsigned long)lane_b.body_max_cyc, (unsigned long)(lane_b.mask_max_cyc / 400u), (unsigned long)lane_b.drops,
                   (unsigned long)lane_b.index_bytes, (unsigned long)lane_b.lookup_ns, (unsigned long)lane_b.rehash_n);
        }
    }
}

/* ---- timers: TIM2 free-running 20 MHz reference; TIM6 -> lane A, TIM7 -> lane B,
 * 1 kHz each, half a period apart, equal NVIC priority ------------------------- */
static void timers_init(void) {
    __HAL_RCC_TIM2_CLK_ENABLE(); __HAL_RCC_TIM6_CLK_ENABLE(); __HAL_RCC_TIM7_CLK_ENABLE();
    /* APB1 timers run at 2 x PCLK1 = 200 MHz; prescale by 10 -> 20 MHz, 50 ns ticks */
    TIM2->PSC = 9; TIM2->ARR = 0xFFFFFFFFu; TIM2->EGR = TIM_EGR_UG; TIM2->CR1 = TIM_CR1_CEN;
    TIM6->PSC = 9; TIM6->ARR = TICKS_PER_MS - 1u; TIM6->EGR = TIM_EGR_UG; TIM6->SR = 0; TIM6->DIER = TIM_DIER_UIE;
    TIM7->PSC = 9; TIM7->ARR = TICKS_PER_MS - 1u; TIM7->EGR = TIM_EGR_UG; TIM7->SR = 0; TIM7->DIER = TIM_DIER_UIE;
    HAL_NVIC_SetPriority(TIM7_IRQn, 5, 0);
    HAL_NVIC_SetPriority(TIM6_DAC_IRQn, 5, 0);   /* equal: neither lane's body nests inside the other's wait */
    NVIC_EnableIRQ(TIM6_DAC_IRQn); NVIC_EnableIRQ(TIM7_IRQn);
    uint32_t ref = TIM2->CNT;
    TIM6->CNT = 0;                 lane_a.expected_cnt = ref + TICKS_PER_MS;
    TIM7->CNT = TICKS_PER_MS / 2;  lane_b.expected_cnt = ref + TICKS_PER_MS / 2;
    TIM6->CR1 = TIM_CR1_CEN; TIM7->CR1 = TIM_CR1_CEN;
}
void TIM6_DAC_IRQHandler(void) {
    uint32_t now = TIM2->CNT; TIM6->SR = 0;
    lane_isr(&lane_a, now, HAL_GetTick());
}
void TIM7_IRQHandler(void) {
    uint32_t now = TIM2->CNT; TIM7->SR = 0;
    lane_isr(&lane_b, now, HAL_GetTick());
}

/* ---- board configuration: prior working values, kept verbatim ------------ */
void SystemClock_Config(void)
{
  RCC_OscInitTypeDef RCC_OscInitStruct = {0};
  RCC_ClkInitTypeDef RCC_ClkInitStruct = {0};
  HAL_PWREx_ConfigSupply(PWR_DIRECT_SMPS_SUPPLY);
  __HAL_PWR_VOLTAGESCALING_CONFIG(PWR_REGULATOR_VOLTAGE_SCALE1);
  while (!__HAL_PWR_GET_FLAG(PWR_FLAG_VOSRDY)) {}
  RCC_OscInitStruct.OscillatorType = RCC_OSCILLATORTYPE_HSI | RCC_OSCILLATORTYPE_HSE;
  RCC_OscInitStruct.HSEState = RCC_HSE_ON;
  RCC_OscInitStruct.HSIState = RCC_HSI_DIV1;
  RCC_OscInitStruct.HSICalibrationValue = RCC_HSICALIBRATION_DEFAULT;
  RCC_OscInitStruct.PLL.PLLState = RCC_PLL_ON;
  RCC_OscInitStruct.PLL.PLLSource = RCC_PLLSOURCE_HSE;
  RCC_OscInitStruct.PLL.PLLM = 5;
  RCC_OscInitStruct.PLL.PLLN = 160;
  RCC_OscInitStruct.PLL.PLLP = 2;
  RCC_OscInitStruct.PLL.PLLQ = 5;
  RCC_OscInitStruct.PLL.PLLR = 2;
  RCC_OscInitStruct.PLL.PLLRGE = RCC_PLL1VCIRANGE_2;
  RCC_OscInitStruct.PLL.PLLVCOSEL = RCC_PLL1VCOWIDE;
  RCC_OscInitStruct.PLL.PLLFRACN = 0;
  if (HAL_RCC_OscConfig(&RCC_OscInitStruct) != HAL_OK) Error_Handler();
  RCC_ClkInitStruct.ClockType = RCC_CLOCKTYPE_HCLK | RCC_CLOCKTYPE_SYSCLK | RCC_CLOCKTYPE_PCLK1 | RCC_CLOCKTYPE_PCLK2 | RCC_CLOCKTYPE_D3PCLK1 | RCC_CLOCKTYPE_D1PCLK1;
  RCC_ClkInitStruct.SYSCLKSource = RCC_SYSCLKSOURCE_PLLCLK;
  RCC_ClkInitStruct.SYSCLKDivider = RCC_SYSCLK_DIV1;
  RCC_ClkInitStruct.AHBCLKDivider = RCC_HCLK_DIV2;
  RCC_ClkInitStruct.APB3CLKDivider = RCC_APB3_DIV2;
  RCC_ClkInitStruct.APB1CLKDivider = RCC_APB1_DIV2;
  RCC_ClkInitStruct.APB2CLKDivider = RCC_APB2_DIV2;
  RCC_ClkInitStruct.APB4CLKDivider = RCC_APB4_DIV2;
  if (HAL_RCC_ClockConfig(&RCC_ClkInitStruct, FLASH_LATENCY_4) != HAL_OK) Error_Handler();
  HAL_RCC_MCOConfig(RCC_MCO1, RCC_MCO1SOURCE_HSI, RCC_MCODIV_1);
}

void PeriphCommonClock_Config(void)
{
  RCC_PeriphCLKInitTypeDef PeriphClkInitStruct = {0};
  PeriphClkInitStruct.PeriphClockSelection = RCC_PERIPHCLK_ADC;
  PeriphClkInitStruct.PLL2.PLL2M = 2;
  PeriphClkInitStruct.PLL2.PLL2N = 12;
  PeriphClkInitStruct.PLL2.PLL2P = 2;
  PeriphClkInitStruct.PLL2.PLL2Q = 2;
  PeriphClkInitStruct.PLL2.PLL2R = 2;
  PeriphClkInitStruct.PLL2.PLL2RGE = RCC_PLL2VCIRANGE_3;
  PeriphClkInitStruct.PLL2.PLL2VCOSEL = RCC_PLL2VCOMEDIUM;
  PeriphClkInitStruct.PLL2.PLL2FRACN = 0;
  PeriphClkInitStruct.AdcClockSelection = RCC_ADCCLKSOURCE_PLL2;
  if (HAL_RCCEx_PeriphCLKConfig(&PeriphClkInitStruct) != HAL_OK) Error_Handler();
}

static void MX_DMA2D_Init(void)
{
  hdma2d.Instance = DMA2D;
  hdma2d.Init.Mode = DMA2D_M2M;
  hdma2d.Init.ColorMode = DMA2D_OUTPUT_ARGB8888;
  hdma2d.Init.OutputOffset = 0;
  hdma2d.LayerCfg[1].InputOffset = 0;
  hdma2d.LayerCfg[1].InputColorMode = DMA2D_INPUT_ARGB8888;
  hdma2d.LayerCfg[1].AlphaMode = DMA2D_NO_MODIF_ALPHA;
  hdma2d.LayerCfg[1].InputAlpha = 0;
  hdma2d.LayerCfg[1].AlphaInverted = DMA2D_REGULAR_ALPHA;
  hdma2d.LayerCfg[1].RedBlueSwap = DMA2D_RB_REGULAR;
  hdma2d.LayerCfg[1].ChromaSubSampling = DMA2D_NO_CSS;
  if (HAL_DMA2D_Init(&hdma2d) != HAL_OK) Error_Handler();
  if (HAL_DMA2D_ConfigLayer(&hdma2d, 1) != HAL_OK) Error_Handler();
}

static void MX_USART1_UART_Init(void)
{
  huart1.Instance = USART1;
  huart1.Init.BaudRate = 115200;
  huart1.Init.WordLength = UART_WORDLENGTH_8B;
  huart1.Init.StopBits = UART_STOPBITS_1;
  huart1.Init.Parity = UART_PARITY_NONE;
  huart1.Init.Mode = UART_MODE_TX_RX;
  huart1.Init.HwFlowCtl = UART_HWCONTROL_NONE;
  huart1.Init.OverSampling = UART_OVERSAMPLING_16;
  if (HAL_UART_Init(&huart1) != HAL_OK) Error_Handler();
}

static void MX_GPIO_Init(void)
{
  GPIO_InitTypeDef GPIO_InitStruct = {0};
  __HAL_RCC_GPIOC_CLK_ENABLE();
  __HAL_RCC_GPIOA_CLK_ENABLE();
  __HAL_RCC_GPIOH_CLK_ENABLE();
  __HAL_RCC_GPIOJ_CLK_ENABLE();
  GPIO_InitStruct.Pin = CEC_CK_MCO1_Pin;
  GPIO_InitStruct.Mode = GPIO_MODE_AF_PP;
  GPIO_InitStruct.Pull = GPIO_NOPULL;
  GPIO_InitStruct.Speed = GPIO_SPEED_FREQ_LOW;
  GPIO_InitStruct.Alternate = GPIO_AF0_MCO;
  HAL_GPIO_Init(CEC_CK_MCO1_GPIO_Port, &GPIO_InitStruct);
  GPIO_InitStruct.Pin = GPIO_PIN_2;
  GPIO_InitStruct.Mode = GPIO_MODE_AF_PP;
  GPIO_InitStruct.Pull = GPIO_NOPULL;
  GPIO_InitStruct.Speed = GPIO_SPEED_FREQ_LOW;
  GPIO_InitStruct.Alternate = GPIO_AF13_DSI;
  HAL_GPIO_Init(GPIOJ, &GPIO_InitStruct);
}

/* MPU: region 0 = default no-access map; region 1 = SDRAM 32 MB normal
 * non-cacheable (framebuffer, scanned by LTDC; stores go through the write
 * buffer, unlike the strongly-ordered TEX0 setting); region 2 = the top
 * 16 MB of SDRAM cacheable write-back — the heap both lanes share. */
void MPU_Config(void)
{
  MPU_Region_InitTypeDef MPU_InitStruct = {0};
  HAL_MPU_Disable();
  MPU_InitStruct.Enable = MPU_REGION_ENABLE;
  MPU_InitStruct.Number = MPU_REGION_NUMBER0;
  MPU_InitStruct.BaseAddress = 0x0;
  MPU_InitStruct.Size = MPU_REGION_SIZE_4GB;
  MPU_InitStruct.SubRegionDisable = 0x87;
  MPU_InitStruct.AccessPermission = MPU_REGION_NO_ACCESS;
  MPU_InitStruct.DisableExec = MPU_INSTRUCTION_ACCESS_DISABLE;
  MPU_InitStruct.IsShareable = MPU_ACCESS_SHAREABLE;
  MPU_InitStruct.IsCacheable = MPU_ACCESS_NOT_CACHEABLE;
  MPU_InitStruct.IsBufferable = MPU_ACCESS_NOT_BUFFERABLE;
  HAL_MPU_ConfigRegion(&MPU_InitStruct);
  MPU_InitStruct.Number = MPU_REGION_NUMBER1;
  MPU_InitStruct.BaseAddress = 0xD0000000;
  MPU_InitStruct.Size = MPU_REGION_SIZE_32MB;
  MPU_InitStruct.SubRegionDisable = 0x0;
  MPU_InitStruct.TypeExtField = MPU_TEX_LEVEL1;
  MPU_InitStruct.AccessPermission = MPU_REGION_FULL_ACCESS;
  MPU_InitStruct.DisableExec = MPU_INSTRUCTION_ACCESS_ENABLE;
  MPU_InitStruct.IsShareable = MPU_ACCESS_NOT_SHAREABLE;
  MPU_InitStruct.IsCacheable = MPU_ACCESS_NOT_CACHEABLE;
  MPU_InitStruct.IsBufferable = MPU_ACCESS_BUFFERABLE;   /* stores merge in the write buffer; no cache to keep coherent with LTDC */
  HAL_MPU_ConfigRegion(&MPU_InitStruct);
  MPU_InitStruct.Number = MPU_REGION_NUMBER2;
  MPU_InitStruct.BaseAddress = HEAP_START;
  MPU_InitStruct.Size = MPU_REGION_SIZE_16MB;
  MPU_InitStruct.TypeExtField = MPU_TEX_LEVEL1;
  MPU_InitStruct.IsCacheable = MPU_ACCESS_CACHEABLE;
  MPU_InitStruct.IsBufferable = MPU_ACCESS_BUFFERABLE;
  HAL_MPU_ConfigRegion(&MPU_InitStruct);
  HAL_MPU_Enable(MPU_PRIVILEGED_DEFAULT);
}

void HAL_LTDC_LineEventCallback(LTDC_HandleTypeDef *hltdc) { (void)hltdc; }

void Error_Handler(void)
{
  printf("[CRITICAL] Error Handler Triggered! Halting.\r\n");
  __disable_irq();
  while (1) {}
}

int __io_putchar(int ch)
{
  HAL_UART_Transmit(&huart1, (uint8_t *)&ch, 1, 0xFFFF);
  return ch;
}

#ifdef USE_FULL_ASSERT
void assert_failed(uint8_t *file, uint32_t line) { (void)file; (void)line; }
#endif
