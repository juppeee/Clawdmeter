#include "touch.h"
#include <Arduino.h>
#include "display_cfg.h"

// Thresholds (easy to tune)
#define SWIPE_MIN_PX       50   // minimum distance to count as swipe
#define SWIPE_MAX_CROSS_PX 30   // max perpendicular movement for a clean swipe
#define HOLD_TIME_MS       500  // time before touch becomes a hold
#define DOUBLE_TAP_MS      300  // max gap between taps for double-tap
#define DOUBLE_TAP_PX      20   // max distance between taps
#define LOGO_X_MAX         60   // logo hit region (upper-left)
#define LOGO_Y_MAX         60

extern LGFX lcd;  // from main.cpp

enum touch_state_t {
    TS_IDLE,
    TS_DOWN,      // finger just touched, waiting to classify
    TS_HOLDING,   // classified as hold, space key is pressed
    TS_SWIPING,   // movement exceeded threshold, will be swipe on release
};

static touch_state_t state = TS_IDLE;
static uint16_t start_x, start_y;
static uint16_t last_x, last_y;  // track last known position for swipe classification
static uint32_t down_time;

// Double-tap tracking
static uint32_t last_tap_time = 0;
static uint16_t last_tap_x, last_tap_y;

static gesture_t classify_swipe(int dx, int dy) {
    int ax = abs(dx);
    int ay = abs(dy);

    if (ax > ay && ax >= SWIPE_MIN_PX && ay <= SWIPE_MAX_CROSS_PX) {
        return dx > 0 ? GESTURE_SWIPE_RIGHT : GESTURE_SWIPE_LEFT;
    }
    if (ay > ax && ay >= SWIPE_MIN_PX && ax <= SWIPE_MAX_CROSS_PX) {
        return dy > 0 ? GESTURE_SWIPE_DOWN : GESTURE_SWIPE_UP;
    }
    return GESTURE_NONE;
}

static bool in_logo_region(uint16_t x, uint16_t y) {
    return x < LOGO_X_MAX && y < LOGO_Y_MAX;
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
            down_time = now;
            state = TS_DOWN;
        }
        break;

    case TS_DOWN:
        if (!touching) {
            // Released quickly - this is a tap (check for double-tap on logo)
            if (in_logo_region(start_x, start_y) &&
                (now - last_tap_time) < DOUBLE_TAP_MS &&
                abs((int)start_x - (int)last_tap_x) < DOUBLE_TAP_PX &&
                abs((int)start_y - (int)last_tap_y) < DOUBLE_TAP_PX) {
                hid_on_gesture(GESTURE_DOUBLE_TAP_LOGO);
                last_tap_time = 0;  // reset so triple-tap doesn't re-trigger
            } else {
                last_tap_time = now;
                last_tap_x = start_x;
                last_tap_y = start_y;
            }
            state = TS_IDLE;
        } else {
            last_x = x;
            last_y = y;
            int dx = (int)x - (int)start_x;
            int dy = (int)y - (int)start_y;

            // Check if movement qualifies as a swipe
            if (abs(dx) >= SWIPE_MIN_PX || abs(dy) >= SWIPE_MIN_PX) {
                state = TS_SWIPING;
            }
            // Check if held long enough for a hold gesture
            else if ((now - down_time) >= HOLD_TIME_MS) {
                hid_on_gesture(GESTURE_HOLD_START);
                state = TS_HOLDING;
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
            // Use last known position since getTouch returns garbage on release
            int dx = (int)last_x - (int)start_x;
            int dy = (int)last_y - (int)start_y;
            gesture_t g = classify_swipe(dx, dy);
            if (g != GESTURE_NONE) {
                hid_on_gesture(g);
            }
            state = TS_IDLE;
        }
        break;
    }
}
