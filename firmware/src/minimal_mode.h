#pragma once

// Minimal mode (design system §10): one room (the meters), a still corner
// Clawd, no stage / face / streak / lurk. Boards opt in through
// BoardCaps.minimal_ui (e-ink, slow panels); any board can be switched by the
// user, persisted in NVS ("clawdmeter"/"min_ui") so it survives reboots.
//
// Shared code asks minimal_mode() — never BoardCaps.minimal_ui directly — so
// the user override always wins. Toggled today by the `minimal on|off|auto`
// serial command; the desktop app's Device tab will drive the same setter.

enum MinimalOverride {
    MINIMAL_AUTO = 0,   // follow BoardCaps.minimal_ui
    MINIMAL_ON   = 1,
    MINIMAL_OFF  = 2,
};

void minimal_mode_init(void);                 // load the saved override (call once in setup)
bool minimal_mode(void);                      // effective: override, else board default
MinimalOverride minimal_mode_override(void);
void minimal_mode_set_override(MinimalOverride o);   // persist + apply

// Optional change listener (the director re-plans its rooms). One slot;
// passing NULL clears it. Called after the value changes, not on init.
typedef void (*minimal_mode_cb_t)(bool minimal);
void minimal_mode_on_change(minimal_mode_cb_t cb);
