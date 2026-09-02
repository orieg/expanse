#include <string.h>
#include "gfx.h"
#include "font16.h"

static volatile uint32_t *fb;

void gfx_init(uint32_t framebuffer_addr) { fb = (volatile uint32_t *)framebuffer_addr; }

void px(int x, int y, uint32_t c) {
    if ((unsigned)x < LCD_W && (unsigned)y < LCD_H) fb[(unsigned)y * LCD_W + (unsigned)x] = c;
}
void fill_rect(int x, int y, int w, int h, uint32_t c) {
    if (x < 0) { w += x; x = 0; }
    if (y < 0) { h += y; y = 0; }
    if (x + w > (int)LCD_W) w = (int)LCD_W - x;
    if (y + h > (int)LCD_H) h = (int)LCD_H - y;
    for (int j = 0; j < h; j++) {
        volatile uint32_t *row = fb + (unsigned)(y + j) * LCD_W + (unsigned)x;
        for (int i = 0; i < w; i++) row[i] = c;
    }
}
void rect(int x, int y, int w, int h, uint32_t c) {
    fill_rect(x, y, w, 1, c); fill_rect(x, y + h - 1, w, 1, c);
    fill_rect(x, y, 1, h, c); fill_rect(x + w - 1, y, 1, h, c);
}
void fill_circle(int cx, int cy, int r, uint32_t c) {
    for (int dy = -r; dy <= r; dy++)
        for (int dx = -r; dx <= r; dx++)
            if (dx * dx + dy * dy <= r * r) px(cx + dx, cy + dy, c);
}
int text(int x, int y, const char *s, uint32_t c, int scale) {
    for (; *s; s++) {
        unsigned code = (unsigned char)*s;
        if (code < 32 || code > 126) code = '?';
        const uint16_t *g = font16[code - 32];
        for (unsigned row = 0; row < FONT_H; row++) {
            uint16_t m = g[row];
            if (!m) continue;
            for (unsigned col = 0; col < FONT_W; col++)
                if (m & (1u << (15 - col)))
                    fill_rect(x + (int)col * scale, y + (int)row * scale, scale, scale, c);
        }
        x += (int)FONT_W * scale;
    }
    return x;
}
int text_right(int xr, int y, const char *s, uint32_t c, int scale) {
    int w = (int)strlen(s) * (int)FONT_W * scale;
    return text(xr - w, y, s, c, scale);
}
