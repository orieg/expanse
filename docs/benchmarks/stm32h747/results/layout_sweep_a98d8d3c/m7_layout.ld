/* STM32H747XI Cortex-M7 core: flash bank 1, DTCM for data/stack, AXI SRAM heap. */
MEMORY
{
  FLASH (rx)  : ORIGIN = 0x08000000, LENGTH = 1024K
  DTCM  (rwx) : ORIGIN = 0x20000000, LENGTH = 128K
  AXI   (rwx) : ORIGIN = 0x24000000, LENGTH = 512K
}

ENTRY(Reset_Handler)

_estack = ORIGIN(DTCM) + LENGTH(DTCM);
_heap_start = ORIGIN(AXI);
_heap_end = ORIGIN(AXI) + LENGTH(AXI);

SECTIONS
{
  .isr_vector :
  {
    . = ALIGN(4);
    KEEP(*(.isr_vector))
    . = ALIGN(4);
  } > FLASH

  .text :
  {
    . = ALIGN(4);
    _layout_pre_start = .;
    . += DEFINED(_layout_pre) ? _layout_pre : 0;
    *libexpanse*.a:*(.text*)
    _layout_gap_start = .;
    . += DEFINED(_layout_gap) ? _layout_gap : 0;
    _layout_harness_start = .;
    *(.text*)
    *(.rodata*)
    *(.glue_7) *(.glue_7t)
    . = ALIGN(4);
  } > FLASH

  .ARM.extab : { *(.ARM.extab* .gnu.linkonce.armextab.*) } > FLASH
  .ARM.exidx : { __exidx_start = .; *(.ARM.exidx*) __exidx_end = .; } > FLASH

  _sidata = LOADADDR(.data);
  .data :
  {
    . = ALIGN(4);
    _sdata = .;
    *(.data*)
    . = ALIGN(4);
    _edata = .;
  } > DTCM AT> FLASH

  .bss (NOLOAD) :
  {
    . = ALIGN(4);
    _sbss = .;
    *(.bss*)
    *(COMMON)
    . = ALIGN(4);
    _ebss = .;
  } > DTCM

  /* Kept (not loaded) so readelf -A can assert the hard-float ABI tag. */
  .ARM.attributes 0 : { *(.ARM.attributes) }
}
