#include "power.h"
#include "display_cfg.h"
#include <Arduino.h>

#ifdef BOARD_AMOLED_18
#include "io_expander.h"
#endif

// Poll intervals
#define BATTERY_POLL_MS   2000
#define CHARGING_POLL_MS  500

static int      cached_pct      = -1;
static bool     cached_charging = false;
static bool     cached_vbus     = false;
static bool     pwr_pressed_flag = false;
static uint32_t last_battery_ms  = 0;
static uint32_t last_charging_ms = 0;
static uint32_t last_pwr_ms      = 0;
#define PWR_POLL_MS 50

#ifdef BOARD_AMOLED_18
static bool last_pwr_state = false;   // edge detection for XCA9554 EXIO4
#endif

void power_init(void) {
    if (!pmu.begin(Wire, AXP2101_ADDR, IIC_SDA, IIC_SCL)) {
        Serial.println("AXP2101 init failed");
        return;
    }
    Serial.println("AXP2101 init OK");

    pmu.enableBattDetection();
    pmu.enableBattVoltageMeasure();

#ifndef BOARD_AMOLED_18
    // AMOLED-2.16: PWR button events come from AXP2101 PKEY short-press IRQ.
    // AMOLED-1.8 routes the PWR button through XCA9554 EXIO4 instead — we
    // poll it in power_tick() rather than subscribing to the PMU IRQ.
    pmu.disableIRQ(XPOWERS_AXP2101_ALL_IRQ);
    pmu.clearIrqStatus();
    pmu.enableIRQ(XPOWERS_AXP2101_PKEY_SHORT_IRQ);
#endif

    cached_charging = pmu.isCharging();
    cached_vbus     = pmu.isVbusIn();
    cached_pct = pmu.getBatteryPercent();
}

void power_tick(void) {
    uint32_t now = millis();

    if (now - last_charging_ms >= CHARGING_POLL_MS) {
        last_charging_ms = now;
        cached_charging = pmu.isCharging();
        cached_vbus     = pmu.isVbusIn();
    }

    if (now - last_battery_ms >= BATTERY_POLL_MS) {
        last_battery_ms = now;
        cached_pct = pmu.getBatteryPercent();
    }

    // Poll PWR button
    if (now - last_pwr_ms >= PWR_POLL_MS) {
        last_pwr_ms = now;
#ifdef BOARD_AMOLED_18
        // XCA9554 EXIO4 — active HIGH, edge-trigger on press
        bool pwr_now = io_expander_get(IOX_PIN_PWR_BTN);
        if (pwr_now && !last_pwr_state) {
            pwr_pressed_flag = true;
        }
        last_pwr_state = pwr_now;
#else
        pmu.getIrqStatus();
        if (pmu.isPekeyShortPressIrq()) {
            pwr_pressed_flag = true;
        }
        pmu.clearIrqStatus();
#endif
    }
}

int power_battery_pct(void) {
    return cached_pct;
}

bool power_is_charging(void) {
    return cached_charging;
}

bool power_is_vbus_in(void) {
    return cached_vbus;
}

bool power_pwr_pressed(void) {
    if (pwr_pressed_flag) {
        pwr_pressed_flag = false;
        return true;
    }
    return false;
}
