/* Minimal Cortex-M7 startup for the STM32H747XI (no Cube, no CMSIS). */
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
    /* 16 external IRQ slots, all unused. */
    Default_Handler, Default_Handler, Default_Handler, Default_Handler,
    Default_Handler, Default_Handler, Default_Handler, Default_Handler,
    Default_Handler, Default_Handler, Default_Handler, Default_Handler,
    Default_Handler, Default_Handler, Default_Handler, Default_Handler,
};

void uart_puts(const char *s);
void uart_hex(uint32_t v);

void Default_Handler(void) {
    uart_puts("FAULT default handler\r\n");
    for (;;) { __asm volatile("bkpt 0"); }
}

void HardFault_Handler(void) {
    uart_puts("HARDFAULT HFSR=");
    uart_hex(SCB_HFSR);
    uart_puts(" CFSR=");
    uart_hex(SCB_CFSR);
    uart_puts("\r\n");
    for (;;) { __asm volatile("bkpt 0"); }
}

void Reset_Handler(void) {
    SCB_VTOR = (uint32_t)vector_table;
    SCB_CPACR |= (0xFu << 20); /* CP10/CP11 full access: FPU on */
    __asm volatile("dsb; isb");

    uint32_t *src = &_sidata, *dst = &_sdata;
    while (dst < &_edata) *dst++ = *src++;
    for (dst = &_sbss; dst < &_ebss;) *dst++ = 0;

    main();
    for (;;) { __asm volatile("wfi"); }
}
