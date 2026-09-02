/* The two lanes of the demo (#605): the same tracker workload on two data
 * structures, each with its own 1 kHz timer interrupt reading its table.
 *
 *   Expanse lane: by-id index = sync32 map (read by the interrupt with a
 *   single-attempt reader, never blocked); by-time index = plain map, swept
 *   with remove_range whose callback removes each expired id from sync32.
 *   Hash lane: one open-addressing table (alts.c) id -> slot, read by its
 *   interrupt with a plain get; the main loop masks that interrupt around
 *   every write and around the full-scan sweep — the honest twin.
 *
 * Records live in a slab (index = value stored in the tables). A handful of
 * "tracked" records never expire; the main loop moves them and the
 * interrupt reads their slot through the table to draw the dots. */
#ifndef LANES_H
#define LANES_H
#include <stdbool.h>
#include <stdint.h>
#include "alts.h"
#include "expanse.h"

#define N_TRACKED 12
#define TTL_MS    60000u

typedef struct { uint32_t id; uint32_t born_ms; int16_t x, y; uint8_t used; } rec_t;

typedef struct lane {
    const char *name; int x0; uint32_t accent; bool is_expanse;
    /* data */
    rec_t *slab; uint32_t cap; uint32_t *free_stack; uint32_t free_top;
    expanse_sync32_map_t *sync; expanse_sync32_map_writer_t *w; expanse_sync32_map_reader_t *r;
    expanse_map_t *by_time; void *hash;
    uint32_t tracked_idx[N_TRACKED]; uint32_t tracked_id[N_TRACKED];
    /* workload state */
    uint32_t count, seq, rng, ingest_credit_ms, last_sweep_ms;
    /* instruments */
    volatile uint32_t isr_served, isr_ok, isr_nf, isr_busy, isr_bad;
    volatile uint32_t lat_max_ticks, lat_last_ticks;     /* entry latency, 50 ns ticks: whole missed ms x 20,000 + timer CNT */
    volatile uint32_t last_isr_ms;
    volatile uint8_t served_ms[4096];                    /* ring by HAL ms: interrupt ran in that ms */
    uint32_t sweep_us, sweep_removed, sweep_max_us, refused;
    int16_t dot_x[N_TRACKED], dot_y[N_TRACKED];          /* last drawn dot positions (interrupt-owned) */
    volatile uint32_t irq_masked;                        /* nesting count of the hash lane's masking */
    volatile uint32_t masked_ticks_max;                  /* longest masked stretch, ticks of the HAL ms clock */
} lane_t;

extern lane_t lane_a, lane_b;

/* set-up */
bool lanes_init(uint32_t target_records);
/* main-loop side: ingest at `rate_per_s`, move the tracked dots, sweep when due */
void lane_ingest(lane_t *l, uint32_t now_ms, uint32_t rate_per_s);
void lane_move_tracked(lane_t *l, uint32_t now_ms);
void lane_sweep(lane_t *l, uint32_t now_ms);
/* interrupt side: read the tracked positions through the table and draw */
void lane_isr(lane_t *l, uint32_t entry_ticks, uint32_t now_ms);

/* the hash lane's twin: mask its own reader interrupt (and only that) */
void lane_mask(lane_t *l);
void lane_unmask(lane_t *l);

#endif
