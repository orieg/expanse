/* Shared-memory mailbox between the Cortex-M7 (writer, UART owner) and the
 * Cortex-M4 (fixture runner, then sync32 reader) in D3 SRAM4 (64 KB).
 *
 *   0x38000000  header (this struct), always non-cacheable on the M7
 *   0x38004000  48 KB text buffer: the M4's RESULT lines, dumped by the M7
 *               (the fixture, DWT-profile and ISR rows of one M4 turn are ~21 KB)
 *
 * The shared sync32 map itself lives in the M7's AXI SRAM heap; the M7's
 * MPU decides per cell whether that heap is cacheable.
 *
 * Protocol: the M7 writes `phase`, then increments `seq` (DMB between); the
 * M4 samples `seq` at boot and acts whenever it changes. SRAM4 keeps its
 * content across resets, so the M7 zeroes the header first and the M4 treats
 * PHASE_IDLE as "keep waiting". */
#ifndef DUAL_H
#define DUAL_H
#include <stdint.h>

#define SHM_BASE       0x38000000u
#define SHM_TEXT_BASE  (SHM_BASE + 0x4000u)
#define SHM_TEXT_SIZE  0xC000u

enum { PHASE_IDLE = 0, PHASE_M4_FIXTURES = 1, PHASE_READER = 2, PHASE_STOP = 3, PHASE_EXIT = 4 };
enum { READ_OPTIMISTIC = 0, READ_HSEM = 1 };
enum { M4_NONE = 0, M4_BOOTED = 1, M4_CALIB_START = 2, M4_CALIB_END = 3, M4_DONE = 4,
       M4_READING = 5, M4_STOPPED = 6 };

typedef struct {
    volatile uint32_t seq;
    volatile uint32_t phase;
    volatile uint32_t m4_state;
    void *volatile map;             /* expanse_sync32_map_t* in the M7 heap */
    volatile uint32_t mode;         /* READ_OPTIMISTIC or READ_HSEM for a PHASE_READER cell */
    volatile uint32_t wait_max, wait_sum_lo, wait_sum_hi; /* HSEM lock wait cycles on the M4 */
    volatile uint32_t ok, nf, busy, bad, reads, cyc_max, cyc_sum_lo, cyc_sum_hi;
    volatile uint32_t text_len, text_overflow;
    volatile uint32_t m4_cpuid;
} shm_hdr;

#define SHM ((shm_hdr *)SHM_BASE)
#define SHM_TEXT ((volatile char *)SHM_TEXT_BASE)

#endif
