#pragma once
#include <Arduino.h>

// Gesture types
enum gesture_t {
    GESTURE_NONE,
    GESTURE_SWIPE_UP,
    GESTURE_SWIPE_DOWN,
    GESTURE_SWIPE_LEFT,
    GESTURE_SWIPE_RIGHT,
    GESTURE_DOUBLE_TAP_LOGO,
    GESTURE_HOLD_START,
    GESTURE_HOLD_END,
};

void hid_init(void);
void hid_on_gesture(gesture_t gesture);
