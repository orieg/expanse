#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "stm32h7xx_hal.h"
#include "gfx.h"
#include "lanes.h"

lane_t lane_a = { .name = "EXPANSE", .x0 = 6, .accent = C_BLUE, .is_expanse = true };
lane_t lane_b = { .name = "HASH TABLE", .x0 = 402, .accent = C_ORANGE, .is_expanse = false };

/* field geometry shared with main.c's layout */
#define FIELD_X(l) ((l)->x0 + 12)
#define FIELD_Y 110
#define FIELD_W 368
#define FIELD_H 200

static inline uint32_t xorshift(uint32_t *s) { *s ^= *s << 13; *s ^= *s >> 17; *s ^= *s << 5; return *s; }
static inline uint32_t tk_of(uint32_t born_ms, uint32_t seq) { return (born_ms << 8) | (seq & 0xFFu); }

/* ---- masking: the hash lane's critical section ------------------------- */
void lane_mask(lane_t *l) {
    if (l->irq_masked++ == 0) NVIC_DisableIRQ(TIM7_IRQn);
}
void lane_unmask(lane_t *l) {
    if (--l->irq_masked == 0) NVIC_EnableIRQ(TIM7_IRQn);
}

/* ---- slab ---------------------------------------------------------------- */
static uint32_t slot_alloc(lane_t *l) { return l->free_top ? l->free_stack[--l->free_top] : UINT32_MAX; }
static void slot_free(lane_t *l, uint32_t idx) { l->slab[idx].used = 0; l->free_stack[l->free_top++] = idx; }

/* ---- table operations, per lane ------------------------------------------ */
static bool tbl_insert(lane_t *l, uint32_t id, uint32_t idx) {
    if (l->is_expanse) {
        bool rep; expanse_word_t old;
        expanse_sync32_status_t st;
        for (;;) {
            st = expanse_sync32_map_writer_try_insert(l->w, id, idx, &rep, &old);
            if (st == EXPANSE_SYNC32_RECLAIM_BACKLOG) { l->refused++; expanse_sync32_map_writer_try_reclaim(l->w); continue; }
            break;
        }
        return st == EXPANSE_SYNC32_OK;
    }
    lane_mask(l);
    alt_open_hash.insert(l->hash, id, idx);
    lane_unmask(l);
    return true;
}
static void tbl_remove(lane_t *l, uint32_t id) {
    if (l->is_expanse) {
        expanse_word_t old;
        while (expanse_sync32_map_writer_try_remove(l->w, id, &old) == EXPANSE_SYNC32_RECLAIM_BACKLOG) {
            l->refused++; expanse_sync32_map_writer_try_reclaim(l->w);
        }
        return;
    }
    lane_mask(l);
    alt_open_hash.remove(l->hash, id);
    lane_unmask(l);
}

/* ---- set-up ---------------------------------------------------------------- */
static bool lane_setup(lane_t *l, uint32_t target) {
    l->cap = target + target / 8 + 64;
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
    /* tracked devices: same ids in both lanes, never expire (born far ahead) */
    for (int t = 0; t < N_TRACKED; t++) {
        uint32_t idx = slot_alloc(l);
        l->slab[idx] = (rec_t){ .id = 0xC0DE0000u + (uint32_t)t, .born_ms = 0xFFFF0000u, .x = 0, .y = 0, .used = 1 };
        l->tracked_idx[t] = idx; l->tracked_id[t] = l->slab[idx].id;
        if (!tbl_insert(l, l->slab[idx].id, idx)) return false;
        l->dot_x[t] = l->dot_y[t] = -1;
        for (int b = 0; b < N_BEADS; b++) l->bead_x[t][b] = l->bead_y[t][b] = -1;
    }
    return true;
}

/* one record: random id, born at `born_ms` */
static bool add_record(lane_t *l, uint32_t born_ms) {
    uint32_t idx = slot_alloc(l);
    if (idx == UINT32_MAX) return false;
    uint32_t id;
    do { id = xorshift(&l->rng); } while ((id & 0xFFFF0000u) == 0xC0DE0000u || id == 0);
    l->slab[idx] = (rec_t){ .id = id, .born_ms = born_ms, .x = 0, .y = 0, .used = 1 };
    if (!tbl_insert(l, id, idx)) { slot_free(l, idx); return false; }
    if (l->is_expanse) { expanse_word_t old; expanse_map_insert(l->by_time, tk_of(born_ms, l->seq), idx, &old); }
    l->seq++; l->count++;
    return true;
}

bool lanes_init(uint32_t target_records) {
    if (!lane_setup(&lane_a, target_records) || !lane_setup(&lane_b, target_records)) return false;
    /* prefill both lanes with the same ages spread over one TTL, so expiry
     * is steady from the first sweep on */
    uint32_t now = HAL_GetTick() + TTL_MS + 1000u; /* pretend time so nothing is "born in the future" */
    uint32_t seed = 0x1234567u;
    for (uint32_t i = 0; i < target_records; i++) {
        uint32_t age = xorshift(&seed) % TTL_MS;
        uint32_t born = now - age;
        if (!add_record(&lane_a, born) || !add_record(&lane_b, born)) return false;
    }
    return true;
}

/* ---- main-loop side ---------------------------------------------------------- */
void lane_ingest(lane_t *l, uint32_t now_ms, uint32_t rate_per_s) {
    /* credit-based: rate_per_s records per 1000 ms, whole records at a time */
    if (l->ingest_credit_ms == 0) l->ingest_credit_ms = now_ms;
    uint32_t due = (now_ms - l->ingest_credit_ms) * rate_per_s / 1000u;
    if (!due) return;
    for (uint32_t i = 0; i < due && i < 64; i++) add_record(l, now_ms + TTL_MS + 1000u);
    l->ingest_credit_ms += due * 1000u / rate_per_s;
}

void lane_move_tracked(lane_t *l, uint32_t now_ms) {
    float t = (float)now_ms * 0.001f;
    for (int d = 0; d < N_TRACKED; d++) {
        float ph = (float)d * 0.7f;
        /* cheap lissajous without libm: two triangle-ish waves */
        float a = t * 2.7f + ph, b = t * 3.9f + ph * 1.7f;
        a -= (float)(int)(a / 6.2831853f) * 6.2831853f; b -= (float)(int)(b / 6.2831853f) * 6.2831853f;
        float sa = (a < 3.1415927f) ? (a / 1.5707963f - 1.0f) : (3.0f - a / 1.5707963f);
        float sb = (b < 3.1415927f) ? (b / 1.5707963f - 1.0f) : (3.0f - b / 1.5707963f);
        rec_t *rec = &l->slab[l->tracked_idx[d]];
        rec->x = (int16_t)(FIELD_W / 2 + (FIELD_W / 2 - 14) * sa);
        rec->y = (int16_t)(FIELD_H / 2 + (FIELD_H / 2 - 14) * sb);
    }
}

typedef struct { lane_t *l; uint32_t cutoff; uint32_t removed; } sweep_ctx;
static void expanse_sweep_cb(expanse_word_t tk, expanse_word_t idx, void *p) {
    (void)tk; sweep_ctx *c = p;
    tbl_remove(c->l, c->l->slab[idx].id);
    slot_free(c->l, idx); c->removed++;
}
static bool hash_expired(uint32_t id, uint32_t idx, void *p) {
    (void)id; sweep_ctx *c = p;
    rec_t *rec = &c->l->slab[idx];
    if (rec->born_ms + TTL_MS < c->cutoff) { slot_free(c->l, idx); c->removed++; return true; }
    return false;
}

void lane_field_border(lane_t *l, uint32_t colour) {
    int fx = FIELD_X(l);
    fill_rect(fx - 2, FIELD_Y - 2, FIELD_W + 4, 2, colour); fill_rect(fx - 2, FIELD_Y + FIELD_H, FIELD_W + 4, 2, colour);
    fill_rect(fx - 2, FIELD_Y - 2, 2, FIELD_H + 4, colour); fill_rect(fx + FIELD_W, FIELD_Y - 2, 2, FIELD_H + 4, colour);
}

void lane_sweep(lane_t *l, uint32_t now_ms) {
    uint32_t pretend_now = now_ms + TTL_MS + 1000u;
    sweep_ctx c = { l, pretend_now, 0 };
    uint32_t t0 = DWT->CYCCNT;
    if (l->is_expanse) {
        uint32_t cutoff_tk = tk_of(pretend_now - TTL_MS, 0xFF);
        expanse_map_remove_range(l->by_time, 0, cutoff_tk, expanse_sweep_cb, &c);
    } else {
        lane_field_border(l, C_RED);      /* the reader is about to be masked */
        lane_mask(l);
        alt_open_hash.evict_scan(l->hash, hash_expired, &c);
        lane_unmask(l);
    }
    uint32_t cyc = DWT->CYCCNT - t0;
    l->sweep_us = cyc / 400u; /* 400 MHz core */
    if (l->sweep_us > l->sweep_max_us) l->sweep_max_us = l->sweep_us;
    l->sweep_removed = c.removed;
    l->count -= c.removed;
    l->last_sweep_ms = now_ms;
}

/* ---- interrupt side ------------------------------------------------------------ */
void lane_isr(lane_t *l, uint32_t entry_ticks, uint32_t now_ms) {
    l->isr_served++;
    /* the timer reloads every ms, so CNT alone cannot show a mask longer
     * than that: add the whole milliseconds this interrupt was held off */
    uint32_t gap = now_ms - l->last_isr_ms;
    l->last_isr_ms = now_ms;
    uint32_t lat = entry_ticks + (gap > 1u ? (gap - 1u) * 20000u : 0u);
    l->lat_last_ticks = lat;
    if (lat > l->lat_max_ticks) l->lat_max_ticks = lat;
    l->served_ms[now_ms & 4095u] = 1;
    int fx = FIELD_X(l);
    for (int d = 0; d < N_TRACKED; d++) {
        expanse_word_t idx;
        bool ok;
        if (l->is_expanse) {
            expanse_sync32_status_t st = expanse_sync32_map_reader_try_get(l->r, l->tracked_id[d], &idx);
            if (st == EXPANSE_SYNC32_BUSY) { l->isr_busy++; continue; }
            ok = (st == EXPANSE_SYNC32_OK);
        } else {
            ok = alt_open_hash.get(l->hash, l->tracked_id[d], &idx);
        }
        if (!ok) { l->isr_nf++; continue; }
        if (idx >= l->cap || l->slab[idx].id != l->tracked_id[d]) { l->isr_bad++; continue; }
        l->isr_ok++;
        int nx = fx + l->slab[idx].x, ny = FIELD_Y + l->slab[idx].y;
        bool bead_tick = (l->bead_tick % BEAD_EVERY) == 0;
        if (bead_tick) {
            /* drop the oldest bead, plant a new one where the dot is now: a
             * string of beads every 16 ms, so a masked window leaves a gap */
            uint8_t hd = l->bead_head[d];
            if (l->bead_x[d][hd] >= 0) fill_circle(l->bead_x[d][hd], l->bead_y[d][hd], 2, C_BG);
            l->bead_x[d][hd] = (int16_t)nx; l->bead_y[d][hd] = (int16_t)ny;
            l->bead_head[d] = (uint8_t)((hd + 1u) % N_BEADS);
        }
        if (nx == l->dot_x[d] && ny == l->dot_y[d]) continue;
        if (l->dot_x[d] >= 0) fill_circle(l->dot_x[d], l->dot_y[d], 5, C_BG);
        /* repaint the beads the head may have erased, then the head */
        for (int b = 0; b < N_BEADS; b++) if (l->bead_x[d][b] >= 0) fill_circle(l->bead_x[d][b], l->bead_y[d][b], 2, l->accent);
        fill_circle(nx, ny, 5, l->accent);
        l->dot_x[d] = (int16_t)nx; l->dot_y[d] = (int16_t)ny;
    }
    l->bead_tick++;
}
