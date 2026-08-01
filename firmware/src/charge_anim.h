#pragma once
#include <lvgl.h>

// Short full-screen animation when the USB cable goes in or comes out, in the
// spirit of a phone's charge splash: a battery outline that pops, fills, and
// flashes a bolt, then gets out of the way again.
//
// It is purely decorative and self-contained — no state to feed it, nothing to
// tick. Build it once next to the other screens, then call charge_anim_play()
// on the cable transition (main.cpp already detects that edge for the battery
// icon).

// Create the (hidden) overlay. Call last in ui_init() so it sits above
// everything else on the screen.
void charge_anim_init(lv_obj_t *parent);

// Play the plug-in (true) or unplug (false) sequence from the start. Calling
// it again mid-run restarts cleanly, so a bouncing cable can't stack playbacks.
void charge_anim_play(bool plugged_in);

// True while the overlay is on screen. The splash draws straight to the panel
// on some boards, so it consults this before its own redraws.
bool charge_anim_is_active(void);
