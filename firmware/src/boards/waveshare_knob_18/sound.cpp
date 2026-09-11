#include "../../hal/sound_hal.h"

// The PCM5100A DAC only drives a line-out on the expansion connector (no amp
// or speaker on the board), and its soft-mute pin is owned by the second MCU.
// No chime here.

void sound_hal_init(void) {}
void sound_hal_tick(void) {}
void sound_hal_play_reset(void) {}
