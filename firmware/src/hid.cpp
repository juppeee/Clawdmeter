#include "hid.h"
#include "ble.h"

// HID usage codes (same for USB and BLE HID)
#define HID_KEY_ARROW_UP      0x52
#define HID_KEY_ARROW_DOWN    0x51
#define HID_KEY_ARROW_LEFT    0x50
#define HID_KEY_ARROW_RIGHT   0x4F
#define HID_KEY_ENTER         0x28
#define HID_KEY_ESCAPE        0x29
#define HID_KEY_BACKSPACE     0x2A
#define HID_KEY_TAB           0x2B
#define HID_KEY_SPACE         0x2C

// Modifier bits
#define MOD_LEFT_CTRL   0x01
#define MOD_LEFT_SHIFT  0x02

struct gesture_mapping_t {
    gesture_t gesture;
    uint8_t key;
    uint8_t modifier;
};

static const gesture_mapping_t mappings[] = {
    { GESTURE_SWIPE_UP,        HID_KEY_ARROW_UP,    0 },
    { GESTURE_SWIPE_DOWN,      HID_KEY_ARROW_DOWN,  0 },
    { GESTURE_SWIPE_LEFT,      HID_KEY_TAB,         MOD_LEFT_SHIFT },
    { GESTURE_SWIPE_RIGHT,     HID_KEY_ENTER,       0 },
    { GESTURE_TAP_ESCAPE,      HID_KEY_ESCAPE,      0 },
    { GESTURE_TAP_BACKSPACE,   HID_KEY_BACKSPACE,   0 },
    { GESTURE_TAP_CTRL_SPACE,  HID_KEY_SPACE,       MOD_LEFT_CTRL },
    { GESTURE_TAP_ARROW_LEFT,  HID_KEY_ARROW_LEFT,  0 },
    { GESTURE_TAP_ARROW_RIGHT, HID_KEY_ARROW_RIGHT, 0 },
};
#define MAPPING_COUNT (sizeof(mappings) / sizeof(mappings[0]))

void hid_init(void) {
    // BLE HID is initialized in ble_init()
}

void hid_on_gesture(gesture_t gesture) {
    if (!ble_is_connected()) return;

    if (gesture == GESTURE_HOLD_START) {
        ble_keyboard_press(HID_KEY_SPACE, 0);
        return;
    }
    if (gesture == GESTURE_HOLD_END) {
        ble_keyboard_release();
        return;
    }
    if (gesture == GESTURE_TOGGLE_SCREEN) {
        return;
    }

    for (int i = 0; i < (int)MAPPING_COUNT; i++) {
        if (mappings[i].gesture == gesture) {
            ble_keyboard_press(mappings[i].key, mappings[i].modifier);
            delay(20);
            ble_keyboard_release();
            return;
        }
    }
}
