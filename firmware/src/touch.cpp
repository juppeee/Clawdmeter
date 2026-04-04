#include "touch.h"
#include "ui.h"
#include "ble.h"
#include <Arduino.h>
#include "display_cfg.h"

// Thresholds
#define SWIPE_MIN_PX       50
#define SWIPE_MAX_CROSS_PX 30
#define HOLD_TIME_MS       500
#define LOGO_HOLD_TIME_MS  800  // longer hold to toggle screens
#define TAP_MAX_MOVE_PX    15
#define LOGO_X_MAX         60
#define LOGO_Y_MAX         60

extern LGFX lcd;

enum touch_state_t {
    TS_IDLE,
    TS_DOWN,
    TS_HOLDING,
    TS_SWIPING,
    TS_LOGO_HOLD,  // holding the logo to toggle screens
};

static touch_state_t state = TS_IDLE;
static uint16_t start_x, start_y;
static uint16_t last_x, last_y;
static uint32_t down_time;

static gesture_t classify_swipe(int dx, int dy) {
    int ax = abs(dx);
    int ay = abs(dy);
    if (ax > ay && ax >= SWIPE_MIN_PX && ay <= SWIPE_MAX_CROSS_PX)
        return dx > 0 ? GESTURE_SWIPE_RIGHT : GESTURE_SWIPE_LEFT;
    if (ay > ax && ay >= SWIPE_MIN_PX && ax <= SWIPE_MAX_CROSS_PX)
        return dy > 0 ? GESTURE_SWIPE_DOWN : GESTURE_SWIPE_UP;
    return GESTURE_NONE;
}

static bool in_logo(uint16_t x, uint16_t y) {
    return x < LOGO_X_MAX && y < LOGO_Y_MAX;
}

// Detect which controller zone was tapped
static gesture_t classify_zone_tap(uint16_t x, uint16_t y) {
    // Logo tap = ESC
    // Left column: ESC top (y 56-156), < bottom (y 166-266)
    if (x < 108) {
        if (y >= 56 && y < 156) return GESTURE_TAP_ESCAPE;
        if (y >= 166 && y < 266) return GESTURE_TAP_ARROW_LEFT;
    }
    // Right column: DEL top, > bottom
    if (x > 372) {
        if (y >= 56 && y < 156) return GESTURE_TAP_BACKSPACE;
        if (y >= 166 && y < 266) return GESTURE_TAP_ARROW_RIGHT;
    }
    return GESTURE_NONE;
}

// Check if point is in a tap-only zone (should not trigger hold-for-space)
static bool is_in_tap_zone(uint16_t x, uint16_t y) {
    return classify_zone_tap(x, y) != GESTURE_NONE;
}

// Check if tap is in the "Clear Bonds" zone on the Bluetooth screen
static bool in_ble_clear_zone(uint16_t x, uint16_t y) {
    return x >= 8 && x <= 472 && y >= 190 && y < 250;
}

void touch_init(void) {
    state = TS_IDLE;
}

void touch_tick(void) {
    uint16_t x, y;
    bool touching = lcd.getTouch(&x, &y);
    uint32_t now = millis();

    switch (state) {
    case TS_IDLE:
        if (touching) {
            start_x = x;
            start_y = y;
            last_x = x;
            last_y = y;
            down_time = now;
            state = TS_DOWN;
        }
        break;

    case TS_DOWN:
        if (!touching) {
            bool is_tap = abs((int)last_x - (int)start_x) < TAP_MAX_MOVE_PX &&
                          abs((int)last_y - (int)start_y) < TAP_MAX_MOVE_PX;

            if (is_tap && in_logo(start_x, start_y)) {
                // Logo tap cycles screens: Usage -> Controller -> Bluetooth -> ...
                ui_cycle_screen();
            } else if (is_tap && ui_get_current_screen() == SCREEN_CONTROLLER) {
                // Zone taps only on controller screen
                gesture_t zone = classify_zone_tap(start_x, start_y);
                if (zone != GESTURE_NONE) {
                    hid_on_gesture(zone);
                }
            } else if (is_tap && ui_get_current_screen() == SCREEN_BLUETOOTH) {
                // "Clear Bonds" tap zone on Bluetooth screen
                if (in_ble_clear_zone(start_x, start_y)) {
                    ble_clear_bonds();
                }
            }
            state = TS_IDLE;
        } else {
            last_x = x;
            last_y = y;
            int dx = (int)x - (int)start_x;
            int dy = (int)y - (int)start_y;

            // Logo hold = Ctrl+Space (controller screen only)
            if (in_logo(start_x, start_y) && ui_get_current_screen() == SCREEN_CONTROLLER && (now - down_time) >= LOGO_HOLD_TIME_MS) {
                hid_on_gesture(GESTURE_TAP_CTRL_SPACE);
                state = TS_LOGO_HOLD;
            }
            // Everything below only on controller screen
            else if (ui_get_current_screen() == SCREEN_CONTROLLER) {
                if (abs(dx) >= SWIPE_MIN_PX || abs(dy) >= SWIPE_MIN_PX) {
                    state = TS_SWIPING;
                } else if (!in_logo(start_x, start_y) && !is_in_tap_zone(start_x, start_y) && (now - down_time) >= HOLD_TIME_MS) {
                    hid_on_gesture(GESTURE_HOLD_START);
                    state = TS_HOLDING;
                }
            }
        }
        break;

    case TS_HOLDING:
        if (!touching) {
            hid_on_gesture(GESTURE_HOLD_END);
            state = TS_IDLE;
        }
        break;

    case TS_SWIPING:
        if (touching) {
            last_x = x;
            last_y = y;
        } else {
            int dx = (int)last_x - (int)start_x;
            int dy = (int)last_y - (int)start_y;
            gesture_t g = classify_swipe(dx, dy);
            if (g != GESTURE_NONE) {
                hid_on_gesture(g);
            }
            state = TS_IDLE;
        }
        break;

    case TS_LOGO_HOLD:
        // Wait for finger to lift after screen toggle
        if (!touching) {
            state = TS_IDLE;
        }
        break;
    }
}
