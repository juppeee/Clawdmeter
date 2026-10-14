#pragma once

// Optional audio output: the ding (ding.h), played for an attention alert
// and when the session window resets. ES8311 boards stream it through the
// shared chime engine (chime.h); boards without sound no-op on init/tick and
// ignore play requests.
//
// Playback is non-blocking: sound_hal_chime() only *queues* the ding and
// returns immediately, so the LVGL render loop never stalls.

void sound_hal_init(void);
void sound_hal_tick(void);
void sound_hal_chime(void);
