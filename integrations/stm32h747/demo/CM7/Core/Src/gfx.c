#include <string.h>
#include "gfx.h"

static volatile uint32_t *fb;

void gfx_init(uint32_t framebuffer_addr) { fb = (volatile uint32_t *)framebuffer_addr; }

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

static inline const glyph_t *glyph(const font_t *f, char ch) {
    unsigned code = (unsigned char)ch;
    if (code < 32 || code > 126) code = '?';
    return &f->g[code - 32];
}
int text_w(const font_t *f, const char *s) {
    int w = 0;
    for (; *s; s++) w += glyph(f, *s)->adv;
    return w;
}
/* fg over bg at one of four coverage levels, per channel, no framebuffer read */
static inline uint32_t mix(uint32_t fg, uint32_t bg, unsigned a) {
    if (a == 3) return fg;
    if (a == 0) return bg;
    uint32_t r = (((fg >> 16) & 0xFF) * a + ((bg >> 16) & 0xFF) * (3 - a)) / 3;
    uint32_t g = (((fg >> 8) & 0xFF) * a + ((bg >> 8) & 0xFF) * (3 - a)) / 3;
    uint32_t b = ((fg & 0xFF) * a + (bg & 0xFF) * (3 - a)) / 3;
    return 0xFF000000u | (r << 16) | (g << 8) | b;
}
int text_f(const font_t *f, int x, int y, const char *s, uint32_t fg, uint32_t bg, int max_w, int align) {
    int w = text_w(f, s);
    if (w > max_w) gfx_overflow(s, w, max_w);
    int x0 = align == ALIGN_RIGHT ? x - (w < max_w ? w : max_w) : x;
    int limit = x0 + max_w;
    for (int pen = x0; *s; s++) {
        const glyph_t *g = glyph(f, *s);
        int gx = pen + g->dx, rowbytes = (g->w + 3) / 4;
        for (unsigned row = 0; row < f->h; row++) {
            int py = y + (int)row;
            if (py < 0 || py >= (int)LCD_H) continue;
            const uint8_t *bits = f->bits + g->off + row * rowbytes;
            volatile uint32_t *dst = fb + (unsigned)py * LCD_W;
            for (int col = 0; col < g->w; col++) {
                unsigned a = (bits[col >> 2] >> (6 - 2 * (col & 3))) & 3u;
                int px = gx + col;
                if (a && px >= x0 && px < limit && px < (int)LCD_W) dst[px] = mix(fg, bg, a);
            }
        }
        pen += g->adv;
        if (pen >= limit) break;
    }
    return w;
}
