#pragma once
#include <stdint.h>
#include "data.h"
#include "signals.h"

// The director: one Clawd, three rooms, driven by the situation.
//
//   meters  home and default; every path ends here. The corner Clawd sits in
//           his seat and does small acts (mascot.h).
//   stage   where he performs a scene (splash.h). A treat after a busy
//           stretch, or a calm dip during the screensaver.
//   face    the #198 face resting on the glass (face.h): the screensaver,
//           and the curtain rooms change behind.
//
// Rules of continuity: only ever one Clawd; he enters and leaves through an
// edge; the screen never hard-cuts (rooms change behind the face, or behind
// the orange streak on a tap). A tap means "show me the meters" everywhere.
//
// Beats (design system §09): notice, show, tap → meters, screensaver + wake,
// attention, fresh-window joy, long-turn wave. The lurk trip and idle acts
// are the corner's own secondary action. Minimal mode collapses all of it to
// one room with a still Clawd. See docs/clawd-director.md.

enum DirectorRoom { ROOM_METERS, ROOM_STAGE, ROOM_FACE };

void director_init(void);          // after ui_init()
void director_tick(void);

// Per parsed payload, after signals_ingest().
void director_on_payload(const UsageData* d, const SignalsUpdate* sig);
void director_alert(const char* message);   // attention hook fired
void director_alert_clear(void);            // the session moved on

void director_tap(void);           // any touch press (also wired internally to LVGL)
void director_button(void);        // a physical button press: you're here

DirectorRoom director_room(void);
const char*  director_beat_name(void);

// QA hook for the `clawd <beat>` serial command: start a beat now so it can
// be captured with `screenshot` on hardware. beat = show | screensaver |
// attention | joy | home. Returns false for an unknown name, or for show /
// screensaver while another beat is still running.
bool director_debug(const char* beat);
