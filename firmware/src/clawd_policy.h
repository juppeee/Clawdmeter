#pragma once
#include <stdint.h>

// Clawd's decision rules, free of LVGL/Arduino so they run in the native
// tests (test/test_clawd_policy). The director (clawd_director.cpp) owns the
// rooms and sequencing; everything here answers "should he, and when?".
//
// Times are millis()-style uint32 and wrap-safe.

// ─── Tunables ────────────────────────────────────────────────────────────────
// One struct so the simulator (SIM_FAST=1) can compress minutes into seconds
// without touching the logic. Defaults are the design-system values.
struct ClawdTuning {
    uint32_t show_after_ms;      // sustained activity level 3 that earns a show
    uint32_t show_cooldown_ms;   // at most one show per this long
    uint32_t show_scene_ms;      // how long the show holds its scene loop
    uint32_t quiet_ms;           // no hook events / engagement → screensaver
    uint32_t calm_min_ms;        // face rests this long (min..max, jittered)
    uint32_t calm_max_ms;        //   before dipping into a calm stage scene
    uint32_t calm_scene_ms;      // loop hold of a calm scene
    uint32_t alert_timeout_ms;   // attention gives up after this long
    uint32_t long_turn_ms;       // a turn this long earns a wave on Stop
    uint32_t joy_pending_ms;     // a reset celebration may wait this long for him to get home
    uint32_t blink_min_ms;       // resting face: gap between blinks/glances
    uint32_t blink_max_ms;
};

ClawdTuning& clawd_tuning(void);           // mutable singleton, defaults below
void         clawd_tuning_defaults(ClawdTuning* t);
void         clawd_tuning_fast(ClawdTuning* t);   // sim walkthrough timings

// ─── Randomness ──────────────────────────────────────────────────────────────
// Small xorshift so behaviour is reproducible in tests (seeded) and cheap on
// device (seeded from millis at boot).
struct ClawdRng { uint32_t s; };
void     clawd_rng_seed(ClawdRng* r, uint32_t seed);
uint32_t clawd_rng_next(ClawdRng* r);
uint32_t clawd_rng_range(ClawdRng* r, uint32_t lo, uint32_t hi);  // inclusive
bool     clawd_rng_chance(ClawdRng* r, uint8_t pct);               // 0..100

// Secondary action: never the same thing twice in a row. Picks an index in
// [0, count) whose `names[i]` differs from `last` (string compare; NULL
// entries are skipped). Returns -1 if nothing is pickable. With one distinct
// candidate it is returned even if it equals `last`.
int clawd_pick_no_repeat(const char* const* names, int count, const char* last, ClawdRng* r);

// Jittered gap: base ± 40 %.
uint32_t clawd_jitter(uint32_t base_ms, ClawdRng* r);

// ─── Show gate ───────────────────────────────────────────────────────────────
// A show is earned by `show_after_ms` of continuous activity level 3 and
// limited to one per `show_cooldown_ms`. Activity < 3 (or unknown) restarts
// the clock.
struct ShowGate {
    bool     busy;          // activity has been 3 since `busy_since`
    uint32_t busy_since;
    bool     shown;         // a show has happened since boot
    uint32_t last_show;
};
void show_gate_clear(ShowGate* g);
void show_gate_update(ShowGate* g, uint32_t now, int activity);
bool show_gate_ready(const ShowGate* g, uint32_t now, const ClawdTuning* t);
void show_gate_mark_shown(ShowGate* g, uint32_t now);   // also restarts the busy clock

// ─── Screensaver gate ────────────────────────────────────────────────────────
// Quiet = nothing has engaged him (hook event, tap, button, alert, usage
// moving) for `quiet_ms`. The face is only for when he can tell you're away,
// so it needs either the hooks (fast clock) or a window that hasn't started
// (the /usage "not started" state the slow clock can see on its own).
struct QuietInput {
    uint32_t now;
    uint32_t last_engagement;   // ms timestamp of the latest engagement
    bool     hooks_seen;        // signals_hooks_seen()
    bool     window_not_started;
    bool     meters_live;       // fresh data on the meters (not pairing / stale)
};
bool screensaver_ready(const QuietInput* in, const ClawdTuning* t);

// ─── Turn tracker ────────────────────────────────────────────────────────────
// A small wave after Stop when the turn (prompt → stop) ran long. The device
// approximates the turn from the event timing it saw.
struct TurnTracker {
    bool     in_turn;
    uint32_t prompt_at;
};
void turn_clear(TurnTracker* t);
// Feed a fired hook event; returns true when this event deserves the wave.
bool turn_on_event(TurnTracker* t, char event, uint32_t now, const ClawdTuning* tune);

// ─── Resting face behaviour ──────────────────────────────────────────────────
// What the resting face does next, and when. Blinks are irregular and
// sometimes double; the rest are glances where the far eye travels further.
enum FaceAct { FACE_BLINK, FACE_DOUBLE_BLINK, FACE_GLANCE_LEFT, FACE_GLANCE_RIGHT };
struct FacePlan { FaceAct act; uint32_t gap_ms; };
FacePlan face_plan_next(ClawdRng* r, const ClawdTuning* t);

// Glance geometry: eye x offsets for a glance of `g` px (negative = left).
// The eye on the far side of the glance travels 1.5x (#198).
static inline void face_eye_offsets(int g, int* left_dx, int* right_dx) {
    *left_dx  = g < 0 ? (g * 3) / 2 : g;
    *right_dx = g > 0 ? (g * 3) / 2 : g;
}
