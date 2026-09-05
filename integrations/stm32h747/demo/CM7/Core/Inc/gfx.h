/* Direct framebuffer drawing for the demo: one ARGB8888 800x480 buffer in
 * SDRAM (non-cacheable), written by the main loop only. Text is composited
 * from 2-bit anti-aliased glyph atlases (fonts.h) over a background colour
 * the caller names, so a glyph is one store per pixel and the framebuffer is
 * never read. Every text run carries a width budget: an overflow is clipped
 * and reported through gfx_overflow(), which the firmware routes to the VCP
 * as a CHECK line, so a collision cannot ship unseen (#605 review). */
#ifndef GFX_H
#define GFX_H
#include <stdint.h>
#include "fonts.h"

#define LCD_W 800U
#define LCD_H 480U

#define RGB(r, g, b) (0xFF000000U | ((uint32_t)(r) << 16) | ((uint32_t)(g) << 8) | (uint32_t)(b))
#define C_BG      RGB(0x0b, 0x12, 0x20)
#define C_PANEL   RGB(0x11, 0x1a, 0x2e)
#define C_GRID    RGB(0x1e, 0x2a, 0x44)
#define C_TEXT    RGB(0xe6, 0xea, 0xf2)
#define C_MUTED   RGB(0x9a, 0xa6, 0xbf)
#define C_BLUE    RGB(0x4c, 0x8d, 0xff)
#define C_AMBER   RGB(0xf5, 0xa5, 0x24)
#define C_GREEN   RGB(0x2f, 0xd2, 0x75)
#define C_RED     RGB(0xf0, 0x4e, 0x4e)
#define C_CYAN    RGB(0x22, 0xd3, 0xee)
#define C_MAGENTA RGB(0xe8, 0x79, 0xf9)

enum { ALIGN_LEFT = 0, ALIGN_RIGHT = 1 };

void gfx_init(uint32_t framebuffer_addr);
void fill_rect(int x, int y, int w, int h, uint32_t c);
void rect(int x, int y, int w, int h, uint32_t c);
/* Width of `s` in `f`, in pixels. */
int  text_w(const font_t *f, const char *s);
/* Draw `s` at (x, y) — y is the top of the glyph cell — composited over `bg`.
 * With ALIGN_RIGHT, x is the right edge. A run wider than `max_w` is clipped
 * at the budget and reported once through gfx_overflow(). Returns the width. */
int  text_f(const font_t *f, int x, int y, const char *s, uint32_t fg, uint32_t bg, int max_w, int align);
/* Hook the firmware provides: a text run exceeded its width budget. */
void gfx_overflow(const char *s, int width, int max_w);

#endif
