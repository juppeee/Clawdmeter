#include "../../hal/display_hal.h"
#include "board.h"
#include <Arduino.h>
#include <Arduino_GFX_Library.h>
#include <esp_heap_caps.h>

// C6 AMOLED-2.16 uses a CO5300 AMOLED panel (per the Waveshare
// ESP32-C6-Touch-AMOLED-2.16 spec) — the same controller as the S3
// AMOLED-2.16 sibling, so we drive it with Arduino_CO5300 and reuse that
// class's vendor-correct init rather than the SH8601 class + a hand-patched
// sequence. LCD reset is not wired to any MCU GPIO; the panel boots from its
// internal power-on reset (rst = GFX_NOT_DEFINED).
//
// Rotation: fixed, set by BOARD_FIXED_ROTATION in board.h, applied on the CPU
// exactly like the S3 2.16 port does. The CO5300's MADCTL can flip axes but
// cannot exchange rows and columns, so there is no hardware shortcut for 90°.
// Unlike the S3 there is no IMU cycle and no PSRAM here: the strip buffer
// comes out of internal SRAM and is sized to one LVGL partial flush
// (LCD_WIDTH × ROT_BUF_LINES × 2 bytes = 19 KB at 20 lines).

// Must match BUF_LINES in main.cpp for the PSRAM-free path.
#define ROT_BUF_LINES 20

static Arduino_DataBus* bus = nullptr;
static Arduino_CO5300*  gfx = nullptr;
static uint16_t*        rot_buf = nullptr;

void display_hal_init(void) {
    bus = new Arduino_ESP32QSPI(
        LCD_CS, LCD_SCLK, LCD_SDIO0, LCD_SDIO1, LCD_SDIO2, LCD_SDIO3);
    // CO5300 constructor: (bus, rst, rotation, w, h, col_off1..2, row_off1..2).
    // No reset GPIO on this board; the 480-wide panel is full-width so all
    // offsets are 0 — matches the S3 AMOLED-2.16 instantiation.
    gfx = new Arduino_CO5300(
        bus, GFX_NOT_DEFINED, 0 /* rotation disabled */,
        LCD_WIDTH, LCD_HEIGHT, 0, 0, 0, 0);
}

// Arduino_CO5300::begin() already issues SLPOUT, SPI-mode control, pixel
// format, brightness-control, DISPON and a default MADCTL. The ONLY thing it
// does not set is this panel's manufacturer page-0x20 driving-voltage
// registers (0x19/0x1C) — without them the panel stays black even with the
// rails up. Set just those; everything else the SH8601-era hack also wrote
// (0xC4/0x36/0x53/0x51/0x63/0x29) is now covered by the class init.
//
// Note: we deliberately do NOT restore the old MADCTL 0x30 (MV transpose).
// The CO5300 class default (rotation-0, MADCTL 0x00) orients the panel with
// the USB port on the side, which is the preferred desk orientation for this
// board.
static void send_panel_driving_init(Arduino_DataBus* b) {
    b->beginWrite();
    b->writeC8D8(0xFE, 0x20);    // enter manufacturer command page 0x20
    b->writeC8D8(0x19, 0x10);    // panel driving voltage
    b->writeC8D8(0x1C, 0xA0);    // panel driving voltage
    b->writeC8D8(0xFE, 0x00);    // back to user command page
    b->endWrite();
    delay(20);
}

void display_hal_begin(void) {
    gfx->begin();
    send_panel_driving_init(bus);   // panel-specific regs the class init omits
    gfx->fillScreen(0x0000);
    gfx->setBrightness(200);

#if BOARD_FIXED_ROTATION != 0
    // Strip buffer for the CPU rotation. Internal SRAM — this board has no
    // PSRAM. If it ever fails to allocate we fall back to drawing unrotated
    // rather than showing nothing: a sideways picture beats a black panel.
    const size_t rot_bytes = (size_t)LCD_WIDTH * ROT_BUF_LINES * 2;
    rot_buf = (uint16_t*)heap_caps_malloc(
        rot_bytes, MALLOC_CAP_INTERNAL | MALLOC_CAP_8BIT);
    Serial.printf("Display: rotation %d, strip %u bytes %s (free internal: %u)\n",
                  BOARD_FIXED_ROTATION, (unsigned)rot_bytes,
                  rot_buf ? "ok" : "FAILED -> drawing unrotated",
                  (unsigned)heap_caps_get_free_size(MALLOC_CAP_INTERNAL));
#endif
}

void display_hal_set_brightness(uint8_t level) {
    if (gfx) gfx->setBrightness(level);
}

void display_hal_fill_screen(uint16_t color) {
    if (gfx) gfx->fillScreen(color);
}

#if BOARD_FIXED_ROTATION != 0
// Rotate a w×h strip into rot_buf and compute where it lands on the 480×480
// panel. Same mapping as the S3 2.16 port; src is row-major over (sx, sy, w, h).
static void rotate_strip(const uint16_t* src, int32_t w, int32_t h,
                         int32_t sx, int32_t sy,
                         int32_t* dx, int32_t* dy, int32_t* dw, int32_t* dh) {
    const int32_t S = LCD_WIDTH;
#if BOARD_FIXED_ROTATION == 1
    // 90° CW: (x,y) -> (S-1-y, x)
    *dw = h; *dh = w;
    *dx = S - sy - h;
    *dy = sx;
    for (int32_t y = 0; y < h; y++)
        for (int32_t x = 0; x < w; x++)
            rot_buf[x * h + (h - 1 - y)] = src[y * w + x];
#elif BOARD_FIXED_ROTATION == 2
    // 180°: (x,y) -> (S-1-x, S-1-y)
    *dw = w; *dh = h;
    *dx = S - sx - w;
    *dy = S - sy - h;
    for (int32_t y = 0; y < h; y++)
        for (int32_t x = 0; x < w; x++)
            rot_buf[(h - 1 - y) * w + (w - 1 - x)] = src[y * w + x];
#else
    // 270° CW (= 90° counter-clockwise): (x,y) -> (y, S-1-x)
    *dw = h; *dh = w;
    *dx = sy;
    *dy = S - sx - w;
    for (int32_t y = 0; y < h; y++)
        for (int32_t x = 0; x < w; x++)
            rot_buf[(w - 1 - x) * h + y] = src[y * w + x];
#endif
}
#endif  // BOARD_FIXED_ROTATION != 0

void display_hal_draw_bitmap(int32_t x, int32_t y, int32_t w, int32_t h,
                             const uint16_t* pixels) {
    if (!gfx) return;
#if BOARD_FIXED_ROTATION != 0
    // Rotate in horizontal slices. Callers are NOT limited to one LVGL flush:
    // splash.cpp pushes bands of (up to 480 × scr_cell) straight to the panel,
    // and scr_cell is 24 here — bigger than one ROT_BUF_LINES strip. Bailing
    // out to the unrotated draw in that case left wide repaints sideways while
    // narrow ones came out upright, so slice instead of giving up.
    if (rot_buf && w > 0 && w <= LCD_WIDTH) {
        int32_t max_rows = (LCD_WIDTH * ROT_BUF_LINES) / w;
        max_rows &= ~1;                       // keep slices even (CO5300 alignment)
        if (max_rows >= 2) {
            for (int32_t y0 = 0; y0 < h; y0 += max_rows) {
                int32_t hh = h - y0 < max_rows ? h - y0 : max_rows;
                int32_t dx, dy, dw, dh;
                rotate_strip(pixels + (size_t)y0 * w, w, hh, x, y + y0,
                             &dx, &dy, &dw, &dh);
                gfx->draw16bitRGBBitmap(dx, dy, rot_buf, dw, dh);
            }
            return;
        }
    }
#endif
    gfx->draw16bitRGBBitmap(x, y, (uint16_t*)pixels, w, h);
}

void display_hal_tick(void) {
    // Rotation is fixed at boot - nothing to poll.
}

// CO5300 requires even-aligned flush regions.
void display_hal_round_area(int32_t* x1, int32_t* y1, int32_t* x2, int32_t* y2) {
    *x1 = *x1 & ~1;
    *y1 = *y1 & ~1;
    *x2 = *x2 | 1;
    *y2 = *y2 | 1;
}
