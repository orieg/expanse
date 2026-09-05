/* The two lanes of the demo (#605, to the reviewed mockup): the same tracker
 * workload on two data structures, each with its own 1 kHz timer interrupt
 * that reads its table and RECORDS the outcome — it draws nothing.
 *
 *   Expanse lane: by-id index = sync32 map (single-attempt reader in the
 *   interrupt, never blocked: BUSY instead); by-time index = plain map swept
 *   with remove_range whose callback removes each expired id from sync32.
 *   Hash lane: one open-addressing table (alts.c) id -> slot; its reader
 *   interrupt is masked according to `mode`: around the whole scan and every
 *   write (naive), per write only (competent), or not at all (unmasked).
 *
 * Each interrupt tick reads N_TRACKED ids through its table and writes one
 * byte into a time ring indexed by the millisecond: VALUE (every read
 * returned the right value), BUSY (a sync32 read was told to retry), WRONG
 * (a read returned a value that does not match its id), or HELD for every
 * millisecond the tick could not run (filled in by the tick that finally
 * ran, from its measured lateness). The main loop draws the heartbeat strip
 * from the ring; nothing an interrupt does lands in the framebuffer, so
 * nothing one lane draws can land in the other lane's entry wait.
 *
 * Tracked devices are re-registered under the writer continuously (remove +
 * insert into a fresh slot; the old slot is retired at the next sweep), so
 * an unmasked hash reader can genuinely read a stale slot. */
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
#define RING_LEN 8192u                /* 8 s of 1 ms slots */
#define REHASH_MAX 24

enum { SLOT_NONE = 0, SLOT_VALUE = 1, SLOT_BUSY = 2, SLOT_WRONG = 3, SLOT_HELD = 0xFF };
enum { HASH_MASK_WHOLE = 0, HASH_MASK_PER_WRITE = 1, HASH_UNMASKED = 2 };

typedef struct { uint32_t id; uint32_t born_ms; uint8_t used; } rec_t;

typedef struct lane {
    const char *name; bool is_expanse; uint32_t mode; bool growable;
    /* data */
    rec_t *slab; uint32_t cap; uint32_t *free_stack; uint32_t free_top;
    expanse_sync32_map_t *sync; expanse_sync32_map_writer_t *w;
    expanse_sync32_map_reader_t *r;       /* the interrupt's reader slot */
    expanse_sync32_map_reader_t *rb;      /* the main loop's reader slot, for the lookup readout */
    expanse_map_t *by_time; void *hash;
    uint32_t tracked_idx[N_TRACKED]; uint32_t tracked_id[N_TRACKED];
    uint32_t retire[RETIRE_MAX]; uint32_t retire_n; uint32_t churn_cursor, last_churn_ms;
    /* workload */
    uint32_t count, seq, rng, ingest_acc, ingest_last_ms, drops;
    /* interrupt instruments (TIM2 reference, 50 ns ticks) */
    volatile uint32_t expected_cnt;
    volatile uint32_t isr_served, blocked_ms, lat_max_ticks;
    volatile uint32_t isr_ok, isr_nf, isr_busy, isr_bad;
    volatile uint32_t no_value_ms, stale_run, stale_max;
    volatile uint32_t body_max_cyc, body_total_cyc;
    volatile uint8_t ring[RING_LEN];
    /* main-loop instruments */
    uint32_t sweep_us, sweep_net_us, sweep_removed, mask_max_cyc, mask_t0, irq_masked;
    uint32_t rehash_ms[REHASH_MAX]; uint32_t rehash_n; uint32_t rehash_moved[REHASH_MAX];
    uint32_t index_bytes, lookup_ns;
} lane_t;

extern lane_t lane_a, lane_b;

bool lanes_init(void);                                  /* allocate both lanes' slabs and structures, empty */
bool lane_fill(lane_t *l, uint32_t records, uint32_t now_ms); /* destroy + recreate the structures, prefill */
void lane_ingest(lane_t *l, uint32_t now_ms, uint32_t rate_per_s, uint32_t cap_records);
void lane_churn(lane_t *l, uint32_t now_ms);
void lane_sweep(lane_t *l, uint32_t now_ms);
void lane_isr(lane_t *l, uint32_t tim2_now, uint32_t now_ms);
void lane_mask(lane_t *l);
void lane_unmask(lane_t *l);
void lane_step_reset(lane_t *l);                        /* zero the per-step counters (the ring keeps history) */
void lane_measure_scoreboard(lane_t *l);                /* index_bytes and lookup_ns, min of 3 batches */

#endif
