#include "../../hal/power_hal.h"
#include "board.h"
#include <Arduino.h>

// No PMU, no power latch and no PWR key on this board, and BATT_ADC reads the
// 5 V rail rather than the cell — so battery, charging and VBUS all report
// "unknown".
//
// The PWR key's two jobs are split up instead: short press (next animation /
// brightness) moves to the ring via input_hal_encoder_steps(); hold-to-pair
// is a touch gesture in normal use (BoardCaps.touch_keys) and, with the case
// open, also a BOOT hold. For the latter, main.cpp's pairing logic sees the
// long / release edges synthesized here, so it stays board-agnostic:
//   long     — fired once when a BOOT hold crosses PWR_LONG_MS
//   release  — fired on every BOOT release edge
// A short-press edge is never produced: BOOT's short press is already Space.
// Holding BOOT also holds Space on the host for the duration — harmless
// while re-pairing, since the host link is about to be dropped.

#define PWR_POLL_MS  50
#define PWR_LONG_MS  1500

static bool     long_flag       = false;
static bool     released_flag   = false;
static bool     last_state      = false;
static bool     long_fired      = false;
static uint32_t press_started_ms = 0;
static uint32_t last_poll_ms    = 0;

void power_hal_init(void) {
    pinMode(BTN_BACK_GPIO, INPUT_PULLUP);
}

void power_hal_tick(void) {
    uint32_t now = millis();
    if (now - last_poll_ms < PWR_POLL_MS) return;
    last_poll_ms = now;

    bool held = (digitalRead(BTN_BACK_GPIO) == LOW);
    if (held && !last_state) {
        press_started_ms = now;
        long_fired = false;
    } else if (held && !long_fired && now - press_started_ms >= PWR_LONG_MS) {
        long_flag  = true;
        long_fired = true;
    } else if (!held && last_state) {
        released_flag = true;
    }
    last_state = held;
}

int  power_hal_battery_pct(void) { return -1; }
bool power_hal_is_charging(void) { return false; }
bool power_hal_is_vbus_in(void)  { return false; }

bool power_hal_pwr_pressed(void) { return false; }

bool power_hal_pwr_long_pressed(void) {
    if (long_flag) { long_flag = false; return true; }
    return false;
}

bool power_hal_pwr_released(void) {
    if (released_flag) { released_flag = false; return true; }
    return false;
}
