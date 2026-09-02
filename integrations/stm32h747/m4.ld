/* STM32H747XI Cortex-M4 core: flash bank 2; D2 SRAM3 for data/stack,
 * D2 SRAM1+SRAM2 (contiguous 256 KB) as the heap. */
MEMORY
{
  FLASH (rx)  : ORIGIN = 0x08100000, LENGTH = 1024K
  SRAM3 (rwx) : ORIGIN = 0x10040000, LENGTH = 32K
  HEAP  (rwx) : ORIGIN = 0x10000000, LENGTH = 256K
}

ENTRY(Reset_Handler)

_estack = ORIGIN(SRAM3) + LENGTH(SRAM3);
_heap_start = ORIGIN(HEAP);
_heap_end = ORIGIN(HEAP) + LENGTH(HEAP);

SECTIONS
{
  .isr_vector : { . = ALIGN(4); KEEP(*(.isr_vector)) . = ALIGN(4); } > FLASH
  .text : { . = ALIGN(4); *(.text*) *(.rodata*) *(.glue_7) *(.glue_7t) . = ALIGN(4); } > FLASH
  .ARM.extab : { *(.ARM.extab* .gnu.linkonce.armextab.*) } > FLASH
  .ARM.exidx : { __exidx_start = .; *(.ARM.exidx*) __exidx_end = .; } > FLASH
  _sidata = LOADADDR(.data);
  .data : { . = ALIGN(4); _sdata = .; *(.data*) . = ALIGN(4); _edata = .; } > SRAM3 AT> FLASH
  .bss (NOLOAD) : { . = ALIGN(4); _sbss = .; *(.bss*) *(COMMON) . = ALIGN(4); _ebss = .; } > SRAM3
  .ARM.attributes 0 : { *(.ARM.attributes) }
}
