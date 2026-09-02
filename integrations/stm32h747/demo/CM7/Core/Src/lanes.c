#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "stm32h7xx_hal.h"
#include "gfx.h"
#include "lanes.h"

lane_t lane_a = { .name = "EXPANSE", .x0 = 6, .accent = C_BLUE, .is_expanse = true };
lane_t lane_b = { .name = "HASH TABLE", .x0 = 402, .accent = C_ORANGE, .is_expanse = false, .mode = HASH_MASK_WHOLE };

#define C_YELLOW  RGB(0xfa, 0xcc, 0x15)
#define C_MAGENTA RGB(0xe8, 0x79, 0xf9)

static inline uint32_t xorshift(uint32_t *s) { *s ^= *s << 13; *s ^= *s >> 17; *s ^= *s << 5; return *s; }
static inline uint32_t tk_of(uint32_t born_ms, uint32_t seq) { return (born_ms << 8) | (seq & 0xFFu); }

/* ---- masking: the hash lane's critical section, measured -------------------- */
void lane_mask(lane_t *l) {
    if (l->irq_masked++ == 0) { NVIC_DisableIRQ(TIM7_IRQn); l->mask_t0 = DWT->CYCCNT; }
}
void lane_unmask(lane_t *l) {
    if (--l->irq_masked == 0) {
        uint32_t w = DWT->CYCCNT - l->mask_t0;
        if (w > l->mask_max_cyc) l->mask_max_cyc = w;
        NVIC_EnableIRQ(TIM7_IRQn);
    }
}
static inline void write_begin(lane_t *l) { if (!l->is_expanse && l->mode != HASH_UNMASKED) lane_mask(l); }
static inline void write_end(lane_t *l) { if (!l->is_expanse && l->mode != HASH_UNMASKED) lane_unmask(l); }

uint32_t lane_isr_cycles(const lane_t *l) { return l->body_total_cyc; }

/* ---- slab ---------------------------------------------------------------- */
static uint32_t slot_alloc(lane_t *l) { return l->free_top ? l->free_stack[--l->free_top] : UINT32_MAX; }
static void slot_free(lane_t *l, uint32_t idx) { l->slab[idx].used = 0; l->slab[idx].id = 0; l->free_stack[l->free_top++] = idx; }

/* ---- table operations, per lane ------------------------------------------ */
static bool tbl_insert(lane_t *l, uint32_t id, uint32_t idx) {
    if (l->is_expanse) {
        bool rep; expanse_word_t old; expanse_sync32_status_t st;
        for (;;) {
            st = expanse_sync32_map_writer_try_insert(l->w, id, idx, &rep, &old);
            if (st == EXPANSE_SYNC32_RECLAIM_BACKLOG) { expanse_sync32_map_writer_try_reclaim(l->w); continue; }
            break;
        }
        return st == EXPANSE_SYNC32_OK;
    }
    write_begin(l); alt_open_hash.insert(l->hash, id, idx); write_end(l);
    return true;
}
static void tbl_remove(lane_t *l, uint32_t id) {
    if (l->is_expanse) {
        expanse_word_t old;
        while (expanse_sync32_map_writer_try_remove(l->w, id, &old) == EXPANSE_SYNC32_RECLAIM_BACKLOG)
            expanse_sync32_map_writer_try_reclaim(l->w);
        return;
    }
    write_begin(l); alt_open_hash.remove(l->hash, id); write_end(l);
}

/* ---- set-up ---------------------------------------------------------------- */
static bool lane_setup(lane_t *l, uint32_t target) {
    l->cap = target + target / 8 + 1024;
    l->slab = calloc(l->cap, sizeof(rec_t));
    l->free_stack = malloc(l->cap * sizeof(uint32_t));
    if (!l->slab || !l->free_stack) return false;
    for (uint32_t i = 0; i < l->cap; i++) l->free_stack[i] = l->cap - 1 - i;
    l->free_top = l->cap;
    l->rng = l->is_expanse ? 0x9E3779B9u : 0x2545F491u;
    if (l->is_expanse) {
        for (uint32_t node_cap = 65536; node_cap >= 8192 && !l->sync; node_cap /= 2)
            l->sync = expanse_sync32_map_new(node_cap, 1);
        if (!l->sync) return false;
        l->w = expanse_sync32_map_writer(l->sync);
        l->r = expanse_sync32_map_reader(l->sync, 0);
        l->by_time = expanse_map_new();
        if (!l->by_time) return false;
    } else {
        l->hash = alt_open_hash.create(l->cap);
        if (!l->hash) return false;
    }
    /* tracked devices: same ids and trajectories in both lanes, never expire */
    static const uint16_t speeds[N_TRACKED] = { 300, 240, 360, 270 };   /* px per second */
    for (int t = 0; t < N_TRACKED; t++) {
        uint32_t idx = slot_alloc(l);
        l->slab[idx] = (rec_t){ .id = 0xC0DE0000u + (uint32_t)t, .born_ms = 0xFFFF0000u,
                                .speed = speeds[t], .phase = (uint16_t)(t * 90), .row = (uint8_t)t, .used = 1 };
        l->tracked_idx[t] = idx; l->tracked_id[t] = l->slab[idx].id;
        if (!tbl_insert(l, l->slab[idx].id, idx)) return false;
        l->last_x[t] = -1;
    }
    return true;
}

static bool add_record(lane_t *l, uint32_t born_ms) {
    uint32_t idx = slot_alloc(l);
    if (idx == UINT32_MAX) return false;
    uint32_t id;
    do { id = xorshift(&l->rng); } while ((id & 0xFFFF0000u) == 0xC0DE0000u || id == 0);
    l->slab[idx] = (rec_t){ .id = id, .born_ms = born_ms, .used = 1 };
    if (!tbl_insert(l, id, idx)) { slot_free(l, idx); return false; }
    if (l->is_expanse) { expanse_word_t old; expanse_map_insert(l->by_time, tk_of(born_ms, l->seq), idx, &old); }
    l->seq++; l->count++;
    return true;
}

bool lanes_init(uint32_t target_records) {
    if (!lane_setup(&lane_a, target_records) || !lane_setup(&lane_b, target_records)) return false;
    uint32_t now = HAL_GetTick() + TTL_MS + 1000u; /* pretend time so nothing is born in the future */
    uint32_t seed = 0x1234567u;
    for (uint32_t i = 0; i < target_records; i++) {
        uint32_t born = now - xorshift(&seed) % TTL_MS;
        if (!add_record(&lane_a, born) || !add_record(&lane_b, born)) return false;
    }
    return true;
}

/* ---- main-loop side ---------------------------------------------------------- */
/* accumulator in milli-records; at most 64 inserts per call, a backlog of
 * more than one second's worth is dropped and counted, never silent */
void lane_ingest(lane_t *l, uint32_t now_ms, uint32_t rate_per_s) {
    if (l->ingest_last_ms == 0) l->ingest_last_ms = now_ms;
    l->ingest_acc += (now_ms - l->ingest_last_ms) * rate_per_s;
    l->ingest_last_ms = now_ms;
    for (int n = 0; n < 64 && l->ingest_acc >= 1000u; n++) {
        if (!add_record(l, now_ms + TTL_MS + 1000u)) { l->drops++; }
        l->ingest_acc -= 1000u;
    }
    uint32_t cap = rate_per_s * 1000u;
    if (l->ingest_acc > cap) { l->drops += (l->ingest_acc - cap) / 1000u; l->ingest_acc = cap; }
}

/* re-register one tracked device: fresh slot, remove + insert under the
 * writer, old slot retired at the next sweep (an interrupt may still hold
 * its index for a few microseconds) */
void lane_churn(lane_t *l, uint32_t now_ms) {
    if (now_ms - l->last_churn_ms < 50u) return;
    l->last_churn_ms = now_ms;
    int t = (int)(l->churn_cursor++ % N_TRACKED);
    uint32_t old_idx = l->tracked_idx[t], new_idx = slot_alloc(l);
    if (new_idx == UINT32_MAX || l->retire_n >= RETIRE_MAX) return;
    l->slab[new_idx] = l->slab[old_idx];
    tbl_remove(l, l->tracked_id[t]);
    tbl_insert(l, l->tracked_id[t], new_idx);
    l->tracked_idx[t] = new_idx;
    l->retire[l->retire_n++] = old_idx;
}

typedef struct { lane_t *l; uint32_t cutoff; uint32_t removed; } sweep_ctx;
static void expanse_sweep_cb(expanse_word_t tk, expanse_word_t idx, void *p) {
    (void)tk; sweep_ctx *c = p;
    tbl_remove(c->l, c->l->slab[idx].id);
    slot_free(c->l, idx); c->removed++;
}
/* the scan predicate. evict_scan (alts.c) stores the tombstone right after
 * a `true`; the table is opaque here, so per-write masking brackets that
 * store from the `true` to the next predicate call (a few dozen occupied
 * slots later) — a window of a microsecond or so, released at scan end.
 * Unmasked mode never masks. */
static bool hash_expired(uint32_t id, uint32_t idx, void *p) {
    (void)id; sweep_ctx *c = p; lane_t *l = c->l;
    if (l->mode == HASH_MASK_PER_WRITE && l->irq_masked) lane_unmask(l);
    rec_t *rec = &l->slab[idx];
    if (rec->born_ms + TTL_MS > c->cutoff) return false;
    slot_free(l, idx); c->removed++;
    if (l->mode == HASH_MASK_PER_WRITE) lane_mask(l);
    return true;
}

void lane_sweep(lane_t *l, uint32_t now_ms) {
    uint32_t pretend_now = now_ms + TTL_MS + 1000u;
    sweep_ctx c = { l, pretend_now, 0 };
    uint32_t isr0 = lane_isr_cycles(&lane_a) + lane_isr_cycles(&lane_b);
    uint32_t t0 = DWT->CYCCNT;
    if (l->is_expanse) {
        expanse_map_remove_range(l->by_time, 0, tk_of(pretend_now - TTL_MS, 0xFF), expanse_sweep_cb, &c);
    } else if (l->mode == HASH_MASK_WHOLE) {
        lane_mask(l);
        alt_open_hash.evict_scan(l->hash, hash_expired, &c);
        lane_unmask(l);
    } else {
        alt_open_hash.evict_scan(l->hash, hash_expired, &c);
        if (l->irq_masked) lane_unmask(l);          /* the last per-write window */
    }
    uint32_t cyc = DWT->CYCCNT - t0;
    uint32_t isr1 = lane_isr_cycles(&lane_a) + lane_isr_cycles(&lane_b);
    l->sweep_us = cyc / 400u;
    l->sweep_net_us = (cyc > isr1 - isr0 ? cyc - (isr1 - isr0) : 0) / 400u;
    l->sweep_removed = c.removed;
    l->count -= c.removed;
    l->last_sweep_ms = now_ms;
    /* retire the slots of re-registered devices: no interrupt can still be inside a read from a sweep ago */
    for (uint32_t i = 0; i < l->retire_n; i++) slot_free(l, l->retire[i]);
    l->retire_n = 0;
}

void lane_clear_dirty_rows(lane_t *l) {
    for (int t = 0; t < N_TRACKED; t++) {
        if (!l->row_dirty[t]) continue;
        l->row_dirty[t] = 0;
        fill_rect(FIELD_X(l), FIELD_Y + t * ROW_H + 1, FIELD_W, ROW_H - 2, C_BG);
    }
}

/* ---- interrupt side ------------------------------------------------------------ */
#define DOT_R 10
/* half-width of each scanline of a radius-10 disc, drawn as spans so the
 * interrupt spends a few microseconds, not a hundred, in the framebuffer */
static const int8_t span[2 * DOT_R + 1] = { 0, 4, 6, 7, 8, 9, 9, 10, 10, 10, 10, 10, 10, 10, 9, 9, 8, 7, 6, 4, 0 };
static inline void hspan(int fx, int a, int b, int y, uint32_t c) {   /* clipped to the field */
    if (a < 0) a = 0;
    if (b > FIELD_W - 1) b = FIELD_W - 1;
    if (b >= a) fill_rect(fx + a, y, b - a + 1, 1, c);
}
/* move the disc from px0 to x (x >= px0, or px0 < 0 for a first draw): per
 * scanline only the pixels the disc left are erased and only the pixels it
 * entered are drawn, so a 1-px step costs ~40 stores into SDRAM, not ~900 */
static void dot_move(int fx, int px0, int x, int cy, uint32_t c) {
    for (int dy = -DOT_R; dy <= DOT_R; dy++) {
        int s = span[dy + DOT_R], y = cy + dy;
        if (px0 >= 0) {
            int e1 = px0 + s < x - s - 1 ? px0 + s : x - s - 1;
            hspan(fx, px0 - s, e1, y, C_BG);
            int d0 = x - s > px0 + s + 1 ? x - s : px0 + s + 1;
            hspan(fx, d0, x + s, y, c);
        } else {
            hspan(fx, x - s, x + s, y, c);
        }
    }
}
void lane_isr(lane_t *l, uint32_t tim2_now, uint32_t now_ms) {
    uint32_t body0 = DWT->CYCCNT;
    /* latency against the free-running reference: how late is this tick, and
     * how many whole ticks were lost while the interrupt was held off */
    uint32_t lat = tim2_now - l->expected_cnt;
    uint32_t missed = lat / TICKS_PER_MS;
    l->expected_cnt += TICKS_PER_MS * (1u + missed);
    l->missed_ticks += missed;
    l->lat_last_ticks = lat;
    if (lat > l->lat_max_ticks) l->lat_max_ticks = lat;
    l->isr_served++;
    l->served_ms[now_ms & 4095u] = 1;
    bool late = lat > TICKS_PER_MS;                  /* more than one period behind */
    int fx = FIELD_X(l);
    for (int d = 0; d < N_TRACKED; d++) {
        expanse_word_t idx; bool ok;
        int y = FIELD_Y + d * ROW_H + ROW_H / 2;
        if (l->is_expanse) {
            expanse_sync32_status_t st = expanse_sync32_map_reader_try_get(l->r, l->tracked_id[d], &idx);
            if (st == EXPANSE_SYNC32_BUSY) { l->isr_busy++; if (l->last_x[d] >= 0) fill_rect(fx + l->last_x[d], y + ROW_H / 2 - 4, 1, 3, C_YELLOW); continue; }
            ok = (st == EXPANSE_SYNC32_OK);
        } else {
            ok = alt_open_hash.get(l->hash, l->tracked_id[d], &idx);
        }
        if (!ok) { l->isr_nf++; if (l->last_x[d] >= 0) fill_rect(fx + l->last_x[d], y + 6, 3, 12, C_MAGENTA); continue; }
        if (idx >= l->cap || l->slab[idx].id != l->tracked_id[d]) { l->isr_bad++; continue; }
        l->isr_ok++;
        const rec_t *rec = &l->slab[idx];
        /* position is a function of time: the read decided whether we advance */
        int x = (int)(((uint64_t)now_ms * rec->speed / 1000u + rec->phase) % (uint32_t)FIELD_W);
        int px0 = l->last_x[d];
        if (px0 < 0 || x < px0) { l->row_dirty[d] = 1; px0 = -1; }  /* first draw, or wrapped: main loop clears the row */
        /* a held-off tick leaves a red mark under the path, from where the dot
         * stood to where it caught up — the field stays clean otherwise */
        if (late && px0 >= 0) fill_rect(fx + px0, y + 6, x - px0 + 1, 12, C_RED);
        dot_move(fx, px0, x, y - 8, l->accent);
        l->last_x[d] = (int16_t)x;
    }
    uint32_t body = DWT->CYCCNT - body0;
    l->body_total_cyc += body; l->body_sum_cyc += body;
    if (body > l->body_max_cyc) l->body_max_cyc = body;
}
