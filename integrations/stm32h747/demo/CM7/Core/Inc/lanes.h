/* The two lanes of the demo (#605, as amended after the first build): the
 * same tracker workload on two data structures, each with its own 1 kHz
 * timer interrupt that reads its table and OWNS THE MOTION it draws.
 *
 *   Expanse lane: by-id index = sync32 map (single-attempt reader in the
 *   interrupt, never blocked: BUSY instead); by-time index = plain map swept
 *   with remove_range whose callback removes each expired id from sync32.
 *   Hash lane: one open-addressing table (alts.c) id -> slot; its reader
 *   interrupt is masked according to `mode`: around the whole scan and every
 *   write (naive), per write only (competent), or not at all (unmasked).
 *
 * Tracked devices ride straight rows. Each interrupt tick: look the device
 * up through the table; if the read succeeded, advance it to f(now) and
 * draw the ribbon segment — coloured by the interrupt's own measured entry
 * gap (accent on time, red for the catch-up after a longer gap), a yellow
 * speck on BUSY, a magenta speck on a phantom miss. The interrupt never
 * erases; the main loop clears a row when its device wraps. So "interrupt
 * did not run" and "dot did not move" are the same event, and the evidence
 * stays on screen.
 *
 * Tracked devices are re-registered under the writer continuously
 * (remove + insert into a fresh slot; the old slot is retired at the next
 * sweep), so an unmasked hash reader can miss the device mid-registration
 * — the correctness instrument can fail. */
#ifndef LANES_H
#define LANES_H
#include <stdbool.h>
#include <stdint.h>
#include "alts.h"
#include "expanse.h"

#define N_TRACKED 4
#define TTL_MS    60000u
#define TICKS_PER_MS 20000u           /* TIM2/6/7 tick: 50 ns */
#define RETIRE_MAX 256

enum { HASH_MASK_WHOLE = 0, HASH_MASK_PER_WRITE = 1, HASH_UNMASKED = 2, HASH_MODES = 3 };

typedef struct { uint32_t id; uint32_t born_ms; uint16_t speed; uint16_t phase; uint8_t row; uint8_t used; } rec_t;

typedef struct lane {
    const char *name; int x0; uint32_t accent; bool is_expanse; uint32_t mode;
    /* data */
    rec_t *slab; uint32_t cap; uint32_t *free_stack; uint32_t free_top;
    expanse_sync32_map_t *sync; expanse_sync32_map_writer_t *w; expanse_sync32_map_reader_t *r;
    expanse_map_t *by_time; void *hash;
    uint32_t tracked_idx[N_TRACKED]; uint32_t tracked_id[N_TRACKED];
    uint32_t retire[RETIRE_MAX]; uint32_t retire_n; uint32_t churn_cursor, last_churn_ms;
    /* workload state */
    uint32_t count, seq, rng, ingest_acc, ingest_last_ms, drops, last_sweep_ms;
    /* interrupt instruments (TIM2 reference, 50 ns ticks) */
    volatile uint32_t expected_cnt;                  /* TIM2 value the next tick is due at */
    volatile uint32_t isr_served, missed_ticks, lat_max_ticks, lat_last_ticks;
    volatile uint32_t isr_ok, isr_nf, isr_busy, isr_bad;
    volatile uint32_t body_max_cyc, body_sum_cyc, body_total_cyc;
    volatile uint8_t served_ms[4096];
    /* main-loop instruments */
    uint32_t sweep_us, sweep_net_us, sweep_removed, mask_max_cyc, mask_t0, irq_masked;
    /* drawing state (interrupt-owned) */
    int16_t last_x[N_TRACKED]; volatile uint8_t row_dirty[N_TRACKED];
} lane_t;

extern lane_t lane_a, lane_b;

bool lanes_init(uint32_t target_records);
void lane_ingest(lane_t *l, uint32_t now_ms, uint32_t rate_per_s);
void lane_churn(lane_t *l, uint32_t now_ms);          /* re-register one tracked device */
void lane_sweep(lane_t *l, uint32_t now_ms);
void lane_clear_dirty_rows(lane_t *l);
void lane_isr(lane_t *l, uint32_t tim2_now, uint32_t now_ms);
void lane_mask(lane_t *l);
void lane_unmask(lane_t *l);
uint32_t lane_isr_cycles(const lane_t *l);          /* running total of ISR body cycles */

/* field geometry shared with main.c */
#define FIELD_X(l) ((l)->x0 + 12)
#define FIELD_Y 106
#define FIELD_W 368
#define FIELD_H 200
#define ROW_H  (FIELD_H / N_TRACKED)

#endif
