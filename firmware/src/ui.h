#pragma once
#include "data.h"
#include "ble.h"

enum screen_t {
    SCREEN_SPLASH,
    SCREEN_USAGE,
    SCREEN_COUNT,
};

void ui_init(void);
void ui_update(const UsageData* data);
void ui_tick_anim(void);
void ui_show_screen(screen_t screen);
void ui_toggle_splash(void);
screen_t ui_get_current_screen(void);
void ui_update_ble_status(ble_state_t state, const char* name, const char* mac);
void ui_update_battery(int percent, bool charging);

// What the touch gestures do on boards with BoardCaps.touch_keys (main.cpp
// supplies the key actions). Ignored elsewhere — there every touch is a tap.
struct UiTouchKeys {
    void (*double_tap)(void);
    void (*hold_start)(void);             // finger held past LVGL's long-press time
    void (*hold_end)(uint32_t held_ms);   // that finger lifted; held_ms from first contact
};
void ui_set_touch_keys(const UiTouchKeys* keys);
