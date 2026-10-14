#include "../../hal/sound_hal.h"
#include <stdio.h>

void sound_hal_init(void) {}
void sound_hal_tick(void) {}
void sound_hal_chime(void) { printf("[sim] chime! (session reset)\n"); }
