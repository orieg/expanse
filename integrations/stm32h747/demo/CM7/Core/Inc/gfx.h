/* Direct framebuffer drawing for the demo: one ARGB8888 800x480 buffer in
 * SDRAM (non-cacheable), written by both the main loop and the lane
 * interrupts. Small and predictable on purpose: no library, no blits. */
#ifndef GFX_H
#define GFX_H
#include <stdint.h>

#define LCD_W 800U
#define LCD_H 480U

#define RGB(r, g, b) (0xFF000000U | ((uint32_t)(r) << 16) | ((uint32_t)(g) << 8) | (uint32_t)(b))
#define C_BG     RGB(0x0b, 0x12, 0x20)
#define C_PANEL  RGB(0x11, 0x1a, 0x2e)
#define C_GRID   RGB(0x1c, 0x27, 0x40)
#define C_TEXT   RGB(0xe5, 0xe7, 0xeb)
#define C_MUTED  RGB(0x94, 0xa3, 0xb8)
#define C_BLUE   RGB(0x3b, 0x82, 0xf6)
#define C_ORANGE RGB(0xf5, 0x9e, 0x0b)
#define C_GREEN  RGB(0x22, 0xc5, 0x5e)
#define C_RED    RGB(0xef, 0x44, 0x44)

void gfx_init(uint32_t framebuffer_addr);
void px(int x, int y, uint32_t c);
void fill_rect(int x, int y, int w, int h, uint32_t c);
void rect(int x, int y, int w, int h, uint32_t c);
void fill_circle(int cx, int cy, int r, uint32_t c);
int  text(int x, int y, const char *s, uint32_t c, int scale);
int  text_right(int xr, int y, const char *s, uint32_t c, int scale);

#endif
