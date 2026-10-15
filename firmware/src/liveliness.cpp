#include "liveliness.h"
#include "clawd_director.h"
#include "clawd_policy.h"
#include <Preferences.h>
#include <Arduino.h>

void liveliness_init(void) {
    Preferences prefs;
    prefs.begin("clawdmeter", true);
    const int saved = prefs.getUChar("lively", CLAWD_LIVELINESS_DEFAULT);
    prefs.end();
    director_set_liveliness(clawd_liveliness_clamp(saved));
    Serial.printf("Liveliness init: %d\n", clawd_liveliness());
}

void liveliness_set(int level) {
    level = clawd_liveliness_clamp(level);
    if (level == clawd_liveliness()) return;
    Preferences prefs;
    prefs.begin("clawdmeter", false);
    prefs.putUChar("lively", (uint8_t)level);
    prefs.end();
    director_set_liveliness(level);
    Serial.printf("Liveliness: %d\n", level);
}

int liveliness_get(void) { return clawd_liveliness(); }
