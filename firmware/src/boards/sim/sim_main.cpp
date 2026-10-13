// Native entry point — stands in for the Arduino runtime. loop()'s own
// delay(5) paces the loop the same way it does on hardware.
#include <Arduino.h>
#include "sim_platform.h"
#include "../../clawd_policy.h"
#include "../../clawd_director.h"
#include <stdlib.h>
#include <Preferences.h>

int main(void) {
    printf(
        "Clawdmeter simulator\n"
        "  mouse          touch (on the meters he reacts; elsewhere: show me the meters)\n"
        "  space          play/pause scenario    left/right step    1-9 jump\n"
        "  d              toggle BLE link\n"
        "  b / n (hold)   BOOT / secondary button    p  PWR button\n"
        "  c              toggle charging            - / =  battery down/up\n"
        "  u              toggle no battery (USB power only)\n"
        "  e / t          hook event: prompt / stop  a  cycle activity 0-3\n"
        "  m              attention alert / clear    z  toggle minimal mode\n"
        "  s              screenshot                 esc  quit\n\n");
    // SIM_FAST=1: director timings in seconds instead of minutes (show after
    // 8 s of activity 3, screensaver after 15 s quiet) for walkthroughs.
    if (getenv("SIM_FAST")) {
        clawd_tuning_fast(&clawd_tuning());
        printf("[sim] SIM_FAST: compressed director timings\n");
    }
    // SIM_TRACE=1: print every director beat step with a timestamp (the
    // timings walkthrough screenshots are picked from). Beat changes always print.
    if (getenv("SIM_TRACE")) director_set_trace(true);
    // SIM_MINIMAL=1: boot with the minimal-mode override on (as if the user
    // had sent `minimal on`); `z` toggles it at runtime.
    if (getenv("SIM_MINIMAL")) {
        Preferences p;
        p.begin("clawdmeter", false);
        p.putUChar("min_ui", 1);
        p.end();
    }
    setup();
    while (!sim_should_quit()) {
        sim_pump();
        loop();
    }
    return 0;
}
