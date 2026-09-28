#include "../../hal/sound_hal.h"
#include "io_expander.h"
#include <Arduino.h>

// Active buzzer on the expander's BEE_EN pin: HIGH sounds a fixed tone, so a
// "note" is just an on/off span. sound_hal_tick() walks the pattern without
// blocking. An "off" write that fails is retried every tick — a latched-on
// buzzer is a loud continuous tone until power cycle.

#define BEEP_ON_MS   90
#define BEEP_GAP_MS  110

static int      edges_left = 0;      // on/off transitions still to do
static bool     buzzing    = false;
static bool     off_pending = false;
static uint32_t next_ms    = 0;

void sound_hal_init(void) { io_expander_set_buzzer(false); }

void sound_hal_tick(void) {
    if (off_pending) {
        if (io_expander_set_buzzer(false)) off_pending = false;
        return;
    }
    if (edges_left == 0 || (int32_t)(millis() - next_ms) < 0) return;
    buzzing = !buzzing;
    edges_left--;
    if (!io_expander_set_buzzer(buzzing) && !buzzing) off_pending = true;
    next_ms = millis() + (buzzing ? BEEP_ON_MS : BEEP_GAP_MS);
}

void sound_hal_play_beeps(int count) {
    if (count <= 0 || edges_left > 0) return;   // don't cut a pattern short
    buzzing    = false;
    edges_left = count * 2;
    next_ms    = millis();
}

void sound_hal_play_reset(void) { sound_hal_play_beeps(3); }
