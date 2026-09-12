#include "../../hal/sound_hal.h"

// The PCM5100A DAC only drives a line-out on the expansion connector (no amp
// or speaker on the board), and its soft-mute pin is owned by the second MCU.
// No chime here.

void sound_hal_init(void) {}
void sound_hal_tick(void) {}
void sound_hal_play_reset(void) {}

// The pairing gesture is blind without feedback, and this board is the worst
// case for it: no speaker, and the gesture runs on a touch hold rather than a
// key. So the cues go to the DRV2605 instead — defined in this board's
// input.cpp, which owns the haptic driver.
#include <stdint.h>
void knob_haptic_effects(uint8_t first, uint8_t second);

#define HAPTIC_STRONG_CLICK 1    // ROM library 1: Strong Click - 100%
#define HAPTIC_DOUBLE_CLICK 10   // ROM library 1: Double Click - 100%

void sound_hal_play_pair_armed(void) { knob_haptic_effects(HAPTIC_STRONG_CLICK, 0); }
void sound_hal_play_paired(void)     { knob_haptic_effects(HAPTIC_DOUBLE_CLICK, 0); }
