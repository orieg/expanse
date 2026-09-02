/* Minimal Cortex-M startup for both STM32H747XI cores (no Cube, no CMSIS).
 * Built with -DCORE_M7 or -DCORE_M4. */
#include <stdint.h>

extern uint32_t _estack, _sidata, _sdata, _edata, _sbss, _ebss;
extern int main(void);
void Reset_Handler(void);
void Default_Handler(void);
void SysTick_Handler(void) __attribute__((weak, alias("Default_Handler")));
void HardFault_Handler(void);

#define REG(a) (*(volatile uint32_t *)(a))
#define SCB_VTOR   REG(0xE000ED08)
#define SCB_CPACR  REG(0xE000ED88)
#define SCB_CFSR   REG(0xE000ED28)
#define SCB_HFSR   REG(0xE000ED2C)

__attribute__((section(".isr_vector"), used))
void (*const vector_table[])(void) = {
    (void (*)(void))&_estack,
    Reset_Handler,
    Default_Handler,   /* NMI */
    HardFault_Handler,
    Default_Handler,   /* MemManage */
    Default_Handler,   /* BusFault */
    Default_Handler,   /* UsageFault */
    0, 0, 0, 0,
    Default_Handler,   /* SVCall */
    Default_Handler,   /* DebugMon */
    0,
    Default_Handler,   /* PendSV */
    SysTick_Handler,
    Default_Handler, Default_Handler, Default_Handler, Default_Handler,
    Default_Handler, Default_Handler, Default_Handler, Default_Handler,
    Default_Handler, Default_Handler, Default_Handler, Default_Handler,
    Default_Handler, Default_Handler, Default_Handler, Default_Handler,
};

void fault_report(const char *what, uint32_t hfsr, uint32_t cfsr);

void Default_Handler(void) {
    fault_report("default handler", 0, 0);
    for (;;) { __asm volatile("bkpt 0"); }
}

void HardFault_Handler(void) {
    fault_report("HARDFAULT", SCB_HFSR, SCB_CFSR);
    for (;;) { __asm volatile("bkpt 0"); }
}

static void start_c(void) {
    SCB_VTOR = (uint32_t)vector_table;
    SCB_CPACR |= (0xFu << 20); /* CP10/CP11 full access: FPU on */
    __asm volatile("dsb; isb");
    uint32_t *src = &_sidata, *dst = &_sdata;
    while (dst < &_edata) *dst++ = *src++;
    for (dst = &_sbss; dst < &_ebss;) *dst++ = 0;
    main();
    for (;;) { __asm volatile("wfi"); }
}

#ifdef CORE_M4
/* The M4's stack lives in D2 SRAM3 and it talks to D3 SRAM4 before the M7
 * has done anything, so enable those block clocks with registers only —
 * no stack may be touched before RCC_AHB2ENR.SRAM3EN is set. */
__attribute__((naked, noreturn)) void Reset_Handler(void) {
    __asm volatile(
        "ldr r0, =0x580244DC\n"   /* RCC_AHB2ENR: SRAM1EN|SRAM2EN|SRAM3EN */
        "ldr r1, [r0]\n"
        "orr r1, r1, #0xE0000000\n"
        "str r1, [r0]\n"
        "ldr r0, =0x580244E0\n"   /* RCC_AHB4ENR: BKPRAMEN|SRAM4EN */
        "ldr r1, [r0]\n"
        "orr r1, r1, #0x30000000\n"
        "str r1, [r0]\n"
        "dsb\n"
        "b %0\n" :: "i"(start_c));
}
#else
void Reset_Handler(void) { start_c(); }
#endif
