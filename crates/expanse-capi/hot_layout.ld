/* Placement of the hot JudyL paths in the Linux shared library.
 *
 * Applied only when the build sets EXPANSE_LINK_HOT_LAYOUT=1 (build.rs).
 * Each function named here carries a `link_section` attribute of the form
 * `.text.expanse_hot.<group>.<name>`. Without this script those input
 * sections fall into `.text` with every other function, as before.
 *
 * `. = ALIGN(. - X, 4096) + X;` moves to the next address whose offset within
 * a 4 KiB page is X. A function placed after such a line therefore keeps its
 * within-page offset whatever the size of the code before it, in this section
 * or outside it, for as long as the gap before it is not overrun. A function
 * listed without such a line follows its predecessor and moves when the
 * predecessor changes size. An input section holds every instantiation and
 * copy of its function, in the compiler's order; the offset is that of the
 * first.
 *
 * The offsets are those the functions had in the library built from main at
 * 59110748f with 64-byte function alignment, and were chosen by measurement
 * (docs/benchmarks/stressng_comparison/README.md). The trailing wildcard of
 * each group takes a function whose attribute names the group and which this
 * file does not list; tests/test_hot_layout_sync.rs keeps the two lists equal.
 */
SECTIONS
{
  .text.expanse_hot : ALIGN(4096)
  {
    . = ALIGN(. - 0x6c0, 4096) + 0x6c0;
    *(.text.expanse_hot.get.locate_slot_popcnt)
    . = ALIGN(. - 0x9c0, 4096) + 0x9c0;
    *(.text.expanse_hot.get.JudyLGet)
    . = ALIGN(. - 0xf40, 4096) + 0xf40;
    *(.text.expanse_hot.get.leaf_search)
    *(.text.expanse_hot.get.locate_slot)
    . = ALIGN(. - 0xe80, 4096) + 0xe80;
    *(.text.expanse_hot.get.leaf_locate)
    . = ALIGN(. - 0xc40, 4096) + 0xc40;
    *(.text.expanse_hot.get.find_byte_8)
    *(.text.expanse_hot.get.*)
    . = ALIGN(. - 0xb80, 4096) + 0xb80;
    *(.text.expanse_hot.ins.JudyLIns)
    . = ALIGN(. - 0xcc0, 4096) + 0xcc0;
    *(.text.expanse_hot.ins.map_insert_with_path_flat)
    . = ALIGN(. - 0xe80, 4096) + 0xe80;
    *(.text.expanse_hot.ins.write_map_immed)
    . = ALIGN(. - 0x280, 4096) + 0x280;
    *(.text.expanse_hot.ins.upgrade_b_to_u)
    *(.text.expanse_hot.ins.*)
    . = ALIGN(. - 0x580, 4096) + 0x580;
    *(.text.expanse_hot.del.JudyLDel)
    . = ALIGN(. - 0x0c0, 4096) + 0x0c0;
    *(.text.expanse_hot.del.map_remove)
    . = ALIGN(. - 0xd00, 4096) + 0xd00;
    *(.text.expanse_hot.del.map_realloc_remove)
    . = ALIGN(. - 0x340, 4096) + 0x340;
    *(.text.expanse_hot.del.downgrade_u_to_b)
    . = ALIGN(. - 0x300, 4096) + 0x300;
    *(.text.expanse_hot.del.linear_remove_slot)
    *(.text.expanse_hot.del.*)
    . = ALIGN(4096);
  }
}
INSERT BEFORE .text;
