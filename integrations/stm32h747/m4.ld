MEMORY
{
  FLASH (rx)  : ORIGIN = 0x08100000, LENGTH = 1024K
  SRAM1 (rwx) : ORIGIN = 0x10000000, LENGTH = 128K
}
ENTRY(Reset_Handler)
_estack = ORIGIN(SRAM1) + LENGTH(SRAM1);
SECTIONS
{
  .isr_vector : { KEEP(*(.isr_vector)) } > FLASH
  .text : { *(.text*) *(.rodata*) } > FLASH
  /DISCARD/ : { *(.ARM.attributes) *(.ARM.exidx*) }
}
