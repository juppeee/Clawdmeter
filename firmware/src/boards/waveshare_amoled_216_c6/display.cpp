#include "../../hal/display_hal.h"
#include "board.h"
#include <Arduino.h>
#include <Arduino_GFX_Library.h>

// C6 AMOLED-2.16 uses an SH8601 panel — same driver family as the
// AMOLED-1.8 port. LCD reset is not wired to any MCU GPIO; the SH8601
// boots from its internal POR. Rotation is disabled (no PSRAM headroom
// for the strip buffer).

static Arduino_DataBus* bus = nullptr;
static Arduino_SH8601*  gfx = nullptr;

void display_hal_init(void) {
    bus = new Arduino_ESP32QSPI(
        LCD_CS, LCD_SCLK, LCD_SDIO0, LCD_SDIO1, LCD_SDIO2, LCD_SDIO3);
    gfx = new Arduino_SH8601(
        bus, GFX_NOT_DEFINED, 0, LCD_WIDTH, LCD_HEIGHT);
}

// Vendor-specific init commands from the Waveshare C6-2.16 BSP
// (02_Example/Arduino-v3.3.3/09_LVGL_V9_Test/bsp_lvgl_port.cpp in the
// waveshareteam/ESP32-C6-Touch-AMOLED-2.16 repo). The stock Arduino_GFX
// SH8601 init does SLPOUT + NORON + INVOFF + PIXFMT + DISPON + brightness,
// which is enough for the AMOLED-1.8 panel but leaves this 2.16 panel
// dark. The page-switch sequence (0xFE 0x20 ... 0xFE 0x00) writes two
// panel-specific manufacturer registers (0x19 and 0x1C) that gate the
// driving voltages — without them the panel stays black even with the
// rails up and the reset pulse applied.
static void send_vendor_init(Arduino_DataBus* b) {
    b->beginWrite();
    b->writeC8D8(0xFE, 0x20);    // enter manufacturer command page 0x20
    b->writeC8D8(0x19, 0x10);    // panel driving
    b->writeC8D8(0x1C, 0xA0);    // panel driving
    b->writeC8D8(0xFE, 0x00);    // back to user command page
    b->writeC8D8(0xC4, 0x80);    // SPI mode control
    b->writeC8D8(0x36, 0x30);    // MADCTL (BSP value)
    b->writeC8D8(0x53, 0x20);    // CTRL display 1 (brightness control on)
    b->writeC8D8(0x51, 0xFF);    // brightness = max
    b->writeC8D8(0x63, 0xFF);    // HBM brightness = max
    b->writeCommand(0x29);       // DISPON (idempotent — stock init already did this)
    b->endWrite();
    delay(20);
}

void display_hal_begin(void) {
    gfx->begin();
    send_vendor_init(bus);       // patch up panel-specific regs the stock init misses
    gfx->fillScreen(0x0000);
    gfx->setBrightness(200);
}

void display_hal_set_brightness(uint8_t level) {
    if (gfx) gfx->setBrightness(level);
}

void display_hal_fill_screen(uint16_t color) {
    if (gfx) gfx->fillScreen(color);
}

void display_hal_draw_bitmap(int32_t x, int32_t y, int32_t w, int32_t h,
                             const uint16_t* pixels) {
    if (gfx) gfx->draw16bitRGBBitmap(x, y, (uint16_t*)pixels, w, h);
}

void display_hal_tick(void) {
    // No rotation cycle on this board.
}

// Mirrors the CO5300/SH8601 even-alignment pattern from the other ports.
// Harmless on SH8601, kept for consistency.
void display_hal_round_area(int32_t* x1, int32_t* y1, int32_t* x2, int32_t* y2) {
    *x1 = *x1 & ~1;
    *y1 = *y1 & ~1;
    *x2 = *x2 | 1;
    *y2 = *y2 | 1;
}
