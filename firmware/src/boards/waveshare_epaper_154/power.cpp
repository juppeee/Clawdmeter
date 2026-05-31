#include "../../hal/power_hal.h"
#include "board.h"
#include <Arduino.h>

// No PMU on this board; battery header is present but charge/measure
// circuitry is not populated, so we report battery_pct=-1. There is no
// battery either — the device runs only on USB — so vbus_in is hardcoded
// true. Reporting it false would let the idle state machine auto-sleep
// after 30 min, which gates display_hal_tick in main.cpp and freezes
// the e-paper indefinitely (LVGL keeps rendering into RAM, but the
// SSD1681 never receives the partial-refresh commands).
// PWR button is a direct GPIO; we edge-detect with debounce polling.

#define PWR_POLL_MS 50

static bool     pwr_pressed_flag = false;
static bool     last_pwr_state   = false;
static uint32_t last_pwr_ms      = 0;

void power_hal_init(void) {
    pinMode(BTN_PWR_GPIO, INPUT_PULLUP);
    last_pwr_state = (digitalRead(BTN_PWR_GPIO) == LOW);
}

void power_hal_tick(void) {
    uint32_t now = millis();
    if (now - last_pwr_ms < PWR_POLL_MS) return;
    last_pwr_ms = now;

    bool pwr_now = (digitalRead(BTN_PWR_GPIO) == LOW);
    if (pwr_now && !last_pwr_state) pwr_pressed_flag = true;
    last_pwr_state = pwr_now;
}

int  power_hal_battery_pct(void) { return -1; }
bool power_hal_is_charging(void) { return false; }
bool power_hal_is_vbus_in(void)  { return true; }   // see top-of-file note

bool power_hal_pwr_pressed(void) {
    if (pwr_pressed_flag) {
        pwr_pressed_flag = false;
        return true;
    }
    return false;
}
