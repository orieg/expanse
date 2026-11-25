/* Placement of the hot JudyL paths in the Linux shared library.
 *
 * Applied only when the build sets EXPANSE_LINK_HOT_LAYOUT=1 (build.rs).
 * No source file names a section: each line below selects a function by the
 * input section the compiler gives it, `.text.<symbol>`. An exported C
 * function is selected by its name. A Rust function is selected by the tail
 * of its v0 mangled name, `<len><module><len><function>` after the crate
 * name, behind a `_R*` wildcard that covers the crate hash; a generic
 * function's pattern ends in `K*` and takes every instantiation.
 *
 * `. = ALIGN(. - X, 4096) + X;` moves to the next address whose offset within
 * a 4 KiB page is X. A function placed after such a line therefore keeps its
 * within-page offset whatever the size of the code before it, in this section
 * or outside it. A function listed without such a line follows its
 * predecessor and moves when the predecessor changes size. A pattern takes
 * every copy and instantiation of its function, in link order; the offset is
 * that of the first.
 *
 * The offsets are those the functions had in the library built from main at
 * 59110748f with 64-byte function alignment, and were chosen by measurement
 * (docs/benchmarks/stressng_comparison/README.md).
 *
 * A pattern that matches nothing places nothing and the link still succeeds.
 * tests/test_hot_layout_sync.rs checks every pattern against the function
 * definitions in the source.
 */
SECTIONS
{
  .text.expanse_hot : ALIGN(4096)
  {
    . = ALIGN(. - 0x6c0, 4096) + 0x6c0;
    *(.text._R*12expanse_trie3get18locate_slot_popcnt)
    . = ALIGN(. - 0x9c0, 4096) + 0x9c0;
    *(.text.JudyLGet)
    . = ALIGN(. - 0xf40, 4096) + 0xf40;
    *(.text._R*12expanse_trie4leaf6search)
    *(.text._R*12expanse_trie3get11locate_slot)
    . = ALIGN(. - 0xe80, 4096) + 0xe80;
    *(.text._R*12expanse_trie4leaf6locate)
    . = ALIGN(. - 0xc40, 4096) + 0xc40;
    *(.text._R*12expanse_trie4bits11find_byte_8)
    . = ALIGN(. - 0xb80, 4096) + 0xb80;
    *(.text.JudyLIns)
    . = ALIGN(. - 0xcc0, 4096) + 0xcc0;
    *(.text._R*12expanse_trie10mutate_map25map_insert_with_path_flatK*)
    . = ALIGN(. - 0xe80, 4096) + 0xe80;
    *(.text._R*12expanse_trie10mutate_map15write_map_immedK*)
    . = ALIGN(. - 0x280, 4096) + 0x280;
    *(.text._R*12expanse_trie6mutate14upgrade_b_to_uK*)
    . = ALIGN(. - 0x580, 4096) + 0x580;
    *(.text.JudyLDel)
    . = ALIGN(. - 0x0c0, 4096) + 0x0c0;
    *(.text._R*12expanse_trie10mutate_map10map_removeK*)
    . = ALIGN(. - 0xd00, 4096) + 0xd00;
    *(.text._R*12expanse_trie4leaf18map_realloc_remove)
    . = ALIGN(. - 0x340, 4096) + 0x340;
    *(.text._R*12expanse_trie6mutate16downgrade_u_to_bK*)
    . = ALIGN(. - 0x300, 4096) + 0x300;
    *(.text._R*12expanse_trie6mutate18linear_remove_slot)
    . = ALIGN(4096);
  }
}
INSERT BEFORE .text;
