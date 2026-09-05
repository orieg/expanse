#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "stm32h7xx_hal.h"
#include "lanes.h"

lane_t lane_a = { .name = "EXPANSE", .is_expanse = true };
lane_t lane_b = { .name = "HASH TABLE", .is_expanse = false, .mode = HASH_MASK_WHOLE };

static inline uint32_t xorshift(uint32_t *s) { *s ^= *s << 13; *s ^= *s >> 17; *s ^= *s << 5; return *s; }
static inline uint32_t tk_of(uint32_t born_ms, uint32_t seq) { return (born_ms << 8) | (seq & 0xFFu); }

/* ---- masking: the hash lane's critical section, measured ------------------- */
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
    write_begin(l);
    if (l->growable) {
        uint32_t grow = 0; size_t len, slots;
        alt_open_hash_insert_grow(l->hash, id, idx, &grow);
        if (grow) {
            alt_open_hash_shape(l->hash, &len, &slots);
            uint32_t ms = (grow + 200000u) / 400000u;          /* 400 MHz, rounded */
            if (ms >= 2 && l->rehash_n < REHASH_MAX) { l->rehash_ms[l->rehash_n] = ms; l->rehash_moved[l->rehash_n] = (uint32_t)len; l->rehash_n++; }   /* under 2 ms cannot cost a tick: logged, not listed */
            printf("INFO rehash lane=%s moved=%lu slots=%lu cycles=%lu ms=%lu\r\n", l->name, (unsigned long)len, (unsigned long)slots, (unsigned long)grow, (unsigned long)ms);
        }
    } else alt_open_hash.insert(l->hash, id, idx);
    write_end(l);
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
#define SLAB_CAP (100000u + 100000u / 8 + 1024u)
bool lanes_init(void) {
    for (lane_t *l = &lane_a; l; l = l == &lane_a ? &lane_b : 0) {
        l->cap = SLAB_CAP;
        l->slab = calloc(l->cap, sizeof(rec_t));
        l->free_stack = malloc(l->cap * sizeof(uint32_t));
        if (!l->slab || !l->free_stack) return false;
    }
    return true;
}
static void structures_free(lane_t *l) {
    if (l->is_expanse) {
        if (l->sync) expanse_sync32_map_free(l->sync);
        if (l->by_time) expanse_map_free(l->by_time);
        l->sync = 0; l->w = 0; l->r = 0; l->rb = 0; l->by_time = 0;
    } else if (l->hash) { alt_open_hash.destroy(l->hash); l->hash = 0; }
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
/* Rebuild the lane's structures and fill them with `records` (0 for the
 * growth step, which then grows from empty). The slab is reused. */
bool lane_fill(lane_t *l, uint32_t records, uint32_t now_ms) {
    structures_free(l);
    for (uint32_t i = 0; i < l->cap; i++) { l->free_stack[i] = l->cap - 1 - i; l->slab[i].used = 0; l->slab[i].id = 0; }
    l->free_top = l->cap; l->count = 0; l->seq = 0; l->retire_n = 0; l->ingest_acc = 0; l->ingest_last_ms = 0; l->drops = 0;
    l->rehash_n = 0;
    l->rng = l->is_expanse ? 0x9E3779B9u : 0x2545F491u;
    if (l->is_expanse) {
        for (uint32_t node_cap = 65536; node_cap >= 8192 && !l->sync; node_cap /= 2)
            l->sync = expanse_sync32_map_new(node_cap, 2);          /* reader 0: the interrupt; reader 1: the main loop's readout */
        if (!l->sync) return false;
        l->w = expanse_sync32_map_writer(l->sync);
        l->r = expanse_sync32_map_reader(l->sync, 0);
        l->rb = expanse_sync32_map_reader(l->sync, 1);
        l->by_time = expanse_map_new();
        if (!l->by_time) return false;
    } else {
        l->hash = alt_open_hash.create(records ? records : 8);       /* growth: 16 slots to start */
        if (!l->hash) return false;
    }
    for (int t = 0; t < N_TRACKED; t++) {
        uint32_t idx = slot_alloc(l);
        l->slab[idx] = (rec_t){ .id = 0xC0DE0000u + (uint32_t)t, .born_ms = 0xFFFF0000u, .used = 1 };
        l->tracked_idx[t] = idx; l->tracked_id[t] = l->slab[idx].id;
        if (!tbl_insert(l, l->slab[idx].id, idx)) return false;
    }
    uint32_t pretend_now = now_ms + TTL_MS + 1000u, seed = 0x1234567u;
    for (uint32_t i = 0; i < records; i++)
        if (!add_record(l, pretend_now - xorshift(&seed) % TTL_MS)) return false;
    return true;
}

/* ---- main-loop side ---------------------------------------------------------- */
void lane_ingest(lane_t *l, uint32_t now_ms, uint32_t rate_per_s, uint32_t cap_records) {
    if (l->ingest_last_ms == 0) l->ingest_last_ms = now_ms;
    l->ingest_acc += (now_ms - l->ingest_last_ms) * rate_per_s;
    l->ingest_last_ms = now_ms;
    for (int n = 0; n < 256 && l->ingest_acc >= 1000u && l->count < cap_records; n++) {
        if (!add_record(l, now_ms + TTL_MS + 1000u)) { l->drops++; }
        l->ingest_acc -= 1000u;
    }
    uint32_t cap = rate_per_s * 1000u;
    if (l->ingest_acc > cap) { if (l->count < cap_records) l->drops += (l->ingest_acc - cap) / 1000u; l->ingest_acc = cap; }
}
/* re-register one tracked device in a fresh slot: an update of its value
 * under the writer; the old slot is retired at the next sweep */
void lane_churn(lane_t *l, uint32_t now_ms) {
    if (now_ms - l->last_churn_ms < 50u) return;
    l->last_churn_ms = now_ms;
    int t = (int)(l->churn_cursor++ % N_TRACKED);
    uint32_t old_idx = l->tracked_idx[t], new_idx = slot_alloc(l);
    if (new_idx == UINT32_MAX || l->retire_n >= RETIRE_MAX) return;
    l->slab[new_idx] = l->slab[old_idx];
    tbl_insert(l, l->tracked_id[t], new_idx);           /* same key: the insert replaces the value in place; no remove, no gap */
    l->tracked_idx[t] = new_idx;
    l->retire[l->retire_n++] = old_idx;
}
typedef struct { lane_t *l; uint32_t cutoff; uint32_t removed; } sweep_ctx;
static void expanse_sweep_cb(expanse_word_t tk, expanse_word_t idx, void *p) {
    (void)tk; sweep_ctx *c = p;
    tbl_remove(c->l, c->l->slab[idx].id);
    slot_free(c->l, idx); c->removed++;
}
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
    uint32_t isr0 = lane_a.body_total_cyc + lane_b.body_total_cyc;
    uint32_t t0 = DWT->CYCCNT;
    if (l->is_expanse) {
        expanse_map_remove_range(l->by_time, 0, tk_of(pretend_now - TTL_MS, 0xFF), expanse_sweep_cb, &c);
    } else if (l->mode == HASH_MASK_WHOLE) {
        lane_mask(l);
        alt_open_hash.evict_scan(l->hash, hash_expired, &c);
        lane_unmask(l);
    } else {
        alt_open_hash.evict_scan(l->hash, hash_expired, &c);
        if (l->irq_masked) lane_unmask(l);
    }
    uint32_t cyc = DWT->CYCCNT - t0;
    uint32_t isr1 = lane_a.body_total_cyc + lane_b.body_total_cyc;
    l->sweep_us = cyc / 400u;
    l->sweep_net_us = (cyc > isr1 - isr0 ? cyc - (isr1 - isr0) : 0) / 400u;
    l->sweep_removed = c.removed;
    l->count -= c.removed;
    for (uint32_t i = 0; i < l->retire_n; i++) slot_free(l, l->retire[i]);
    l->retire_n = 0;
}
void lane_step_reset(lane_t *l) {
    l->blocked_ms = 0; l->lat_max_ticks = 0; l->isr_served = 0; l->isr_ok = l->isr_nf = l->isr_busy = l->isr_bad = 0;
    l->no_value_ms = 0; l->stale_run = 0; l->stale_max = 0; l->body_max_cyc = 0; l->mask_max_cyc = 0; l->drops = 0; l->rehash_n = 0;
}

/* ---- scoreboard: measured on the lane's own objects ------------------------- */
static size_t lane_index_bytes(lane_t *l) {
    if (l->is_expanse) {
        expanse_sync32_stats_t s;
        if (expanse_sync32_map_writer_stats(l->w, &s, sizeof s) != EXPANSE_SYNC32_OK) return 0;
        return s.mem_used + expanse_map_mem_used(l->by_time);
    }
    return alt_open_hash_bytes(l->hash);
}
/* 64 lookups of present ids through the SAME path the interrupt uses
 * (sync32 reader slot 1 / the hash table's get); min of 3 batches, in ns. */
static uint32_t lane_lookup_ns(lane_t *l) {
    uint32_t ids[64]; uint32_t k = 0, probe = l->rng ^ 0xA5A5A5A5u;
    for (uint32_t tries = 0; k < 64 && tries < 4096; tries++) {
        probe = probe * 1664525u + 1013904223u;
        const rec_t *r = &l->slab[probe % l->cap];
        if (r->used && r->id) ids[k++] = r->id;
    }
    if (k < 8) return 0;
    uint32_t best = UINT32_MAX; volatile uint32_t sink = 0; expanse_word_t v;
    for (int batch = 0; batch < 3; batch++) {
        uint32_t t0 = DWT->CYCCNT;
        for (uint32_t i = 0; i < k; i++) {
            if (l->is_expanse) { if (expanse_sync32_map_reader_try_get(l->rb, ids[i], &v) == EXPANSE_SYNC32_OK) sink += v; }
            else { if (alt_open_hash.get(l->hash, ids[i], &v)) sink += v; }
        }
        uint32_t cyc = DWT->CYCCNT - t0;
        if (cyc < best) best = cyc;
    }
    (void)sink;
    return best * 10u / (4u * k);   /* cycles/lookup at 400 MHz -> ns */
}
void lane_measure_scoreboard(lane_t *l) { l->index_bytes = (uint32_t)lane_index_bytes(l); l->lookup_ns = lane_lookup_ns(l); }

/* ---- interrupt side: read, record, draw nothing ------------------------------ */
void lane_isr(lane_t *l, uint32_t tim2_now, uint32_t now_ms) {
    uint32_t body0 = DWT->CYCCNT;
    uint32_t lat = tim2_now - l->expected_cnt;
    if ((int32_t)lat < 0) lat = 0;                       /* tick arrived before the re-armed reference: not late */
    uint32_t missed = lat / TICKS_PER_MS;
    l->expected_cnt += TICKS_PER_MS * (1u + missed);
    if (missed) {
        l->blocked_ms += missed;
        uint32_t fill = missed > 255u ? 255u : missed;
        for (uint32_t k = 1; k <= fill; k++) l->ring[(now_ms - k) & (RING_LEN - 1u)] = SLOT_HELD;
        l->no_value_ms += missed; l->stale_run += missed;
        if (l->stale_run > l->stale_max) l->stale_max = l->stale_run;   /* the held run counts even if this tick then reads a value */
    }
    if (lat > l->lat_max_ticks) l->lat_max_ticks = lat;
    l->isr_served++;
    uint8_t slot = SLOT_VALUE;
    for (int d = 0; d < N_TRACKED; d++) {
        expanse_word_t idx; bool ok;
        if (l->is_expanse) {
            expanse_sync32_status_t st = expanse_sync32_map_reader_try_get(l->r, l->tracked_id[d], &idx);
            if (st == EXPANSE_SYNC32_BUSY) { l->isr_busy++; if (slot == SLOT_VALUE) slot = SLOT_BUSY; continue; }
            ok = (st == EXPANSE_SYNC32_OK);
        } else {
            ok = alt_open_hash.get(l->hash, l->tracked_id[d], &idx);
        }
        if (!ok) { l->isr_nf++; slot = SLOT_WRONG; continue; }               /* a tracked id is always present: not-found is a wrong answer */
        if (idx >= l->cap || l->slab[idx].id != l->tracked_id[d]) { l->isr_bad++; slot = SLOT_WRONG; continue; }
        l->isr_ok++;
    }
    l->ring[now_ms & (RING_LEN - 1u)] = slot;
    if (slot == SLOT_VALUE) l->stale_run = 0; else { l->no_value_ms++; l->stale_run++; }
    if (l->stale_run > l->stale_max) l->stale_max = l->stale_run;
    uint32_t body = DWT->CYCCNT - body0;
    l->body_total_cyc += body;
    if (body > l->body_max_cyc) l->body_max_cyc = body;
}
