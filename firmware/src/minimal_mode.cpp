#include "minimal_mode.h"
#include "hal/board_caps.h"
#include <Preferences.h>
#include <Arduino.h>

// NVS byte: 0xFF / absent = auto, else a MinimalOverride value.
#define PREF_NS  "clawdmeter"
#define PREF_KEY "min_ui"

static MinimalOverride cur = MINIMAL_AUTO;
static minimal_mode_cb_t listener = nullptr;

static bool effective(MinimalOverride o) {
    if (o == MINIMAL_ON)  return true;
    if (o == MINIMAL_OFF) return false;
    return board_caps().minimal_ui;
}

void minimal_mode_init(void) {
    Preferences prefs;
    prefs.begin(PREF_NS, true);
    uint8_t v = prefs.getUChar(PREF_KEY, 0xFF);
    prefs.end();
    cur = (v == MINIMAL_ON || v == MINIMAL_OFF) ? (MinimalOverride)v : MINIMAL_AUTO;
    Serial.printf("Minimal mode: %s (override=%u)\n", effective(cur) ? "on" : "off", (unsigned)cur);
}

bool minimal_mode(void) { return effective(cur); }
MinimalOverride minimal_mode_override(void) { return cur; }

void minimal_mode_set_override(MinimalOverride o) {
    const bool before = effective(cur);
    cur = o;
    Preferences prefs;
    prefs.begin(PREF_NS, false);
    prefs.putUChar(PREF_KEY, (uint8_t)o);
    prefs.end();
    const bool after = effective(cur);
    if (after != before && listener) listener(after);
}

void minimal_mode_on_change(minimal_mode_cb_t cb) { listener = cb; }
