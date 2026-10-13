#pragma once
#include <stdbool.h>
#include <stdint.h>

// The #198 face: Clawd up close, resting on the glass. Clawd-orange full
// field, two tall dark eyes; a blink is the solid-cell > < squint; on a glance
// the far eye travels 1.5x. Plain LVGL rectangles on lv_layer_top() — no pixel
// buffer — so it runs on every board, PSRAM or not.
//
// It is both a room (the screensaver) and the curtain the other rooms change
// behind: it rises from behind the bottom bezel (ease-out) and drops back
// below it (ease-in). Owned and sequenced by the director.

void face_init(void);
void face_tick(void);

void face_rise(void);          // up from the bottom bezel, ease-out
void face_drop(void);          // down behind the bezel, ease-in; hidden when gone
void face_hide_now(void);      // no animation (mode switch / reset only)

bool face_visible(void);       // any part of it on screen
bool face_up(void);            // fully up and not moving
bool face_moving(void);        // rising or dropping
bool face_busy(void);          // moving, blinking or glancing

void face_blink(bool twice);   // the noticing beat; also used as the curtain blink
void face_set_resting(bool on);// irregular blinks + glances while up
// Look toward screen x (a tap): the eyes glance proportionally (up to 1.5x
// the resting glance, far eye 1.5x on top), hold for hold_ms, then come back
// to you. face_busy() until they're back.
void face_look_at_x(int x, uint32_t hold_ms);
