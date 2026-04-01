#include "hid.h"
#include "USB.h"
#include "USBHIDKeyboard.h"

static USBHIDKeyboard keyboard;

// Gesture-to-key mapping table. Edit this to change controls.
struct gesture_mapping_t {
    gesture_t gesture;
    uint8_t key;       // HID_KEY_* constant (for pressRaw/releaseRaw)
    uint8_t modifier;  // KEY_LEFT_CTRL, KEY_LEFT_SHIFT, etc. (0 = none)
};

static const gesture_mapping_t mappings[] = {
    { GESTURE_SWIPE_UP,        HID_KEY_ARROW_UP,    0 },
    { GESTURE_SWIPE_DOWN,      HID_KEY_ARROW_DOWN,  0 },
    { GESTURE_SWIPE_LEFT,      HID_KEY_TAB,         KEY_LEFT_SHIFT },
    { GESTURE_SWIPE_RIGHT,     HID_KEY_ENTER,       0 },
    { GESTURE_DOUBLE_TAP_LOGO, HID_KEY_SPACE,       KEY_LEFT_CTRL },
};
#define MAPPING_COUNT (sizeof(mappings) / sizeof(mappings[0]))

// Hold gesture uses space key (press on start, release on end)
#define HOLD_KEY HID_KEY_SPACE

void hid_init(void) {
    USB.begin();
    keyboard.begin();
}

void hid_on_gesture(gesture_t gesture) {
    if (gesture == GESTURE_HOLD_START) {
        keyboard.pressRaw(HOLD_KEY);
        return;
    }
    if (gesture == GESTURE_HOLD_END) {
        keyboard.releaseRaw(HOLD_KEY);
        return;
    }

    for (int i = 0; i < MAPPING_COUNT; i++) {
        if (mappings[i].gesture == gesture) {
            if (mappings[i].modifier) keyboard.press(mappings[i].modifier);
            keyboard.pressRaw(mappings[i].key);
            delay(20);
            keyboard.releaseRaw(mappings[i].key);
            if (mappings[i].modifier) keyboard.release(mappings[i].modifier);
            return;
        }
    }
}
