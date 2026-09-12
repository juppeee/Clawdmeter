#pragma once

// Optional audio output (a passive piezo buzzer driven by LEDC PWM). Used to
// chime when the Claude session limit resets. Boards without a buzzer — the
// AMOLED-1.8 and the C6, whose stock hardware has no speaker output — no-op on
// init/tick and ignore play requests.
//
// Playback is non-blocking: sound_hal_play_reset() only *queues* the chime and
// returns immediately; sound_hal_tick() (called every loop) advances the notes
// so the LVGL render loop never stalls.

void sound_hal_init(void);
void sound_hal_tick(void);
void sound_hal_play_reset(void);

// Feedback for the hold-to-pair gesture, which is otherwise blind: the user
// holds a button for three seconds with nothing to go by. armed() fires the
// moment releasing would pair, paired() once the bonds are actually cleared.
// Boards with no speaker ignore both and rely on the on-screen overlay —
// including the Knob-1.8, whose haptic driver was measured to be too weak to
// notice (see that board's sound.cpp).
void sound_hal_play_pair_armed(void);
void sound_hal_play_paired(void);
