#include "../../hal/sound_hal.h"

// AMOLED-1.8 has no buzzer wired (its stock audio path is input/codec only),
// so sound output is a no-op. See boards/waveshare_amoled_216/sound.cpp for the
// real LEDC buzzer driver.

void sound_hal_init(void) {}
void sound_hal_tick(void) {}
void sound_hal_play_reset(void) {}
