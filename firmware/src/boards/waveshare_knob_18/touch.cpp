#include "../../hal/touch_hal.h"
#include "board.h"
#include <Arduino.h>
#include <Wire.h>

// Minimal CST816 reader — the same FocalTech-style register layout as the
// AMOLED-1.8 and LCD-1.54 ports (regs 0x02..0x06):
//   reg 0x02:        low nibble = active finger count
//   reg 0x03 / 0x04: X high (low nibble) + X low
//   reg 0x05 / 0x06: Y high (low nibble) + Y low
// Waveshare's demo reports panel-native coordinates at rotation 0 — no swap
// or mirror. With the panel turned 180° (LCD_ROTATION_180) both axes are
// mirrored here to match.

static volatile bool     touch_data_ready = false;
static volatile bool     touch_pressed = false;
static volatile uint16_t touch_x = 0;
static volatile uint16_t touch_y = 0;

static void IRAM_ATTR touch_isr(void) {
    touch_data_ready = true;
}

static void touch_read_into_shared_state(void) {
    Wire.beginTransmission(CST816_ADDR);
    Wire.write(0x02);
    if (Wire.endTransmission(false) != 0) { touch_pressed = false; return; }
    if (Wire.requestFrom((uint8_t)CST816_ADDR, (uint8_t)5) != 5) { touch_pressed = false; return; }
    uint8_t fingers = Wire.read() & 0x0F;
    uint8_t xH = Wire.read();
    uint8_t xL = Wire.read();
    uint8_t yH = Wire.read();
    uint8_t yL = Wire.read();
    if (fingers == 0 || fingers > 5) {
        touch_pressed = false;
        return;
    }
    uint16_t x = ((uint16_t)(xH & 0x0F) << 8) | xL;
    uint16_t y = ((uint16_t)(yH & 0x0F) << 8) | yL;
    if (x >= LCD_WIDTH)  x = LCD_WIDTH - 1;
    if (y >= LCD_HEIGHT) y = LCD_HEIGHT - 1;
#if LCD_ROTATION_180
    x = LCD_WIDTH  - 1 - x;
    y = LCD_HEIGHT - 1 - y;
#endif
    touch_x = x;
    touch_y = y;
    touch_pressed = true;
}

void touch_hal_init(void) {
    pinMode(TP_RST, OUTPUT);
    digitalWrite(TP_RST, LOW);
    delay(10);
    digitalWrite(TP_RST, HIGH);
    delay(100);

    // CST816 chip id lives at reg 0xA7.
    Wire.beginTransmission(CST816_ADDR);
    Wire.write(0xA7);
    if (Wire.endTransmission(false) == 0 &&
        Wire.requestFrom((uint8_t)CST816_ADDR, (uint8_t)1) == 1) {
        Serial.printf("Touch CST816 ID=0x%02X (addr 0x%02X)\n", Wire.read(), CST816_ADDR);
    } else {
        Serial.printf("Touch ID read failed (addr 0x%02X)\n", CST816_ADDR);
    }

    // Keep the controller out of auto-sleep — asleep it stops raising INT and
    // the first tap after idle would be swallowed. Reg 0xFE nonzero = stay on.
    Wire.beginTransmission(CST816_ADDR);
    Wire.write(0xFE);
    Wire.write(0x01);
    Wire.endTransmission();

    pinMode(TP_INT, INPUT_PULLUP);
    attachInterrupt(TP_INT, touch_isr, FALLING);
}

void touch_hal_read(uint16_t* x, uint16_t* y, bool* pressed) {
    if (touch_data_ready) {
        touch_data_ready = false;
        touch_read_into_shared_state();
    } else if (touch_pressed) {
        // The finger-up report can land between polls; keep re-reading while
        // pressed so a stuck "pressed" state clears.
        touch_read_into_shared_state();
    }
    *x = touch_x;
    *y = touch_y;
    *pressed = touch_pressed;
}
