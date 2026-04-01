#pragma once
#include <Arduino.h>

enum gesture_t {
    GESTURE_NONE,
    GESTURE_SWIPE_UP,
    GESTURE_SWIPE_DOWN,
    GESTURE_SWIPE_LEFT,
    GESTURE_SWIPE_RIGHT,
    GESTURE_HOLD_START,
    GESTURE_HOLD_END,
    GESTURE_TAP_ESCAPE,
    GESTURE_TAP_BACKSPACE,
    GESTURE_TAP_CTRL_SPACE,
    GESTURE_TAP_ARROW_LEFT,
    GESTURE_TAP_ARROW_RIGHT,
    GESTURE_TOGGLE_SCREEN,
};

void hid_init(void);
void hid_on_gesture(gesture_t gesture);
