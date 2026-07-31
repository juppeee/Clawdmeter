#include "../../hal/touch_hal.h"
#include "board.h"
#include <Arduino.h>
#include <Wire.h>
#include <TouchDrvCSTXXX.hpp>

static TouchDrvCST92xx touch;

static volatile bool     touch_data_ready = false;
static volatile bool     touch_pressed = false;
static volatile uint16_t touch_x = 0;
static volatile uint16_t touch_y = 0;

static void IRAM_ATTR touch_isr(void) {
    touch_data_ready = true;
}

void touch_hal_init(void) {
    touch.setPins(TP_RST, TP_INT);
    if (!touch.begin(Wire, CST9220_ADDR, IIC_SDA, IIC_SCL)) {
        Serial.println("Touch init failed");
        return;
    }
    touch.setMaxCoordinates(LCD_WIDTH, LCD_HEIGHT);
    // C6 2.16 panel mapping. The original values (swap=true, mirrorX=true)
    // were calibrated to the old display orientation that force-wrote MADCTL
    // 0x30 (MV transpose + ML). The display now runs at the CO5300 class
    // default (MADCTL 0x00 — USB-port-on-the-side orientation), so the touch
    // mapping is re-derived to match. SensorLib applies swap then mirror
    // (TouchDrvInterface::updateXY); tap-tested on C6 hardware, the raw
    // CST9217 coordinates map straight through in this orientation — no swap,
    // no mirror.
    touch.setSwapXY(false);
    touch.setMirrorXY(false, false);
    pinMode(TP_INT, INPUT_PULLUP);
    attachInterrupt(TP_INT, touch_isr, FALLING);
    Serial.println("Touch init OK");
}

// The panel image is rotated on the CPU (see display.cpp), but the touch
// controller keeps reporting in the panel's own coordinate system. Apply the
// same rotation here or every tap lands somewhere else on screen.
//
// display.cpp maps a panel point (px,py) to the rotated image; this is the
// inverse, from a raw touch (rx,ry) to image coordinates.
static inline void apply_rotation(uint16_t* x, uint16_t* y) {
#if BOARD_FIXED_ROTATION == 1
    uint16_t rx = *x, ry = *y;          // image 90° CW
    *x = ry;
    *y = (uint16_t)(LCD_WIDTH - 1 - rx);
#elif BOARD_FIXED_ROTATION == 2
    *x = (uint16_t)(LCD_WIDTH  - 1 - *x);
    *y = (uint16_t)(LCD_HEIGHT - 1 - *y);
#elif BOARD_FIXED_ROTATION == 3
    uint16_t rx = *x, ry = *y;          // image 90° counter-clockwise
    *x = (uint16_t)(LCD_HEIGHT - 1 - ry);
    *y = rx;
#else
    (void)x; (void)y;
#endif
}

void touch_hal_read(uint16_t* x, uint16_t* y, bool* pressed) {
    if (touch_data_ready) {
        touch_data_ready = false;
        int16_t tx[5], ty[5];
        uint8_t n = touch.getPoint(tx, ty, touch.getSupportTouchPoint());
        if (n > 0) {
            touch_pressed = true;
            uint16_t px = (uint16_t)tx[0], py = (uint16_t)ty[0];
            apply_rotation(&px, &py);
            touch_x = px;
            touch_y = py;
        } else {
            touch_pressed = false;
        }
    }
    *x = touch_x;
    *y = touch_y;
    *pressed = touch_pressed;
}
