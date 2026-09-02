/* Cortex-M4 idle image for flash bank 2 (0x08100000): parks the second core
 * in WFI so the factory demo does not run alongside the M7 harness. */
#include <stdint.h>
extern uint32_t _estack;
void Reset_Handler(void) { for (;;) { __asm volatile("wfi"); } }
void Spin(void) { for (;;) { __asm volatile("wfi"); } }
__attribute__((section(".isr_vector"), used))
void (*const vector_table[])(void) = {
    (void (*)(void))&_estack, Reset_Handler, Spin, Spin, Spin, Spin, Spin,
    0, 0, 0, 0, Spin, Spin, 0, Spin, Spin,
};
