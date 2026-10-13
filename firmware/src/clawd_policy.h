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
    uint32_t react_pending_ms;   // a notice / wave may wait this long for him to hurry home
    uint32_t blink_min_ms;       // resting face: gap between blinks/glances
    uint32_t blink_max_ms;
    uint32_t peek_cooldown_ms;   // a tap on the meters earns the face peek at most once per this long
    uint32_t peek_look_min_ms;   // the peeking face holds its look toward the tap (min..max),
                                 // then rests until tapped (peek_hold)
    uint32_t peek_look_max_ms;
    uint8_t  peek_double_pct;    // chance the peek's blink is a double
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

// ─── Shows ───────────────────────────────────────────────────────────────────
// Which mood group a show performs from. The trigger (sustained activity 3)
// would lift the live group to Heavy every time, so the show reads the
// usage-only base group instead, floored at Active: a show is always a busy
// stretch, but the laptop / dancing / skateboard / soccer set stays reachable
// and Heavy's set is kept for when usage really is burning.
int clawd_show_group(int base_group);

// ─── Attention: where it returns to ─────────────────────────────────────────
// "Attention can interrupt anything and returns to where it left off."
// A show it cut short ends gracefully home (it was a 20 s treat; replaying it
// after a 2-minute alert would be stale, and the show gate already counted
// it). An interrupted screensaver comes back if nobody showed up: the alert
// timed out or was cleared with no tap, button, prompt or tool activity in
// between. Any sign of you (or a tap: "show me the meters") ends on the meters.
enum AttnFrom { ATTN_FROM_HOME, ATTN_FROM_SHOW, ATTN_FROM_SCREENSAVER };
enum AttnEnd  { ATTN_END_TAP, ATTN_END_CLEAR, ATTN_END_TIMEOUT };
bool attention_resumes_screensaver(AttnFrom from, AttnEnd how, bool engaged_since_alert);

// ─── Reactions while he's out ────────────────────────────────────────────────
// A notice or wave that arrives while he's away from his seat (a lurk trip)
// is not dropped: he cuts the trip short, hurries back and then reacts. It
// goes stale after `react_pending_ms` (he never got home in time).
bool reaction_still_due(uint32_t queued_at, uint32_t now, const ClawdTuning* t);

// ─── Corner seat ─────────────────────────────────────────────────────────────
// Feet line for the corner seat: `feet_y` lowered just enough that an act
// `tallest_px` tall clears the top edge by `top_margin`, but never below
// `max_feet` (the first meter panel). If it can't fit, the result is clamped
// and the caller's tall acts must fall back to something shorter.
int corner_seat_feet(int feet_y, int tallest_px, int max_feet, int top_margin);

// ─── A tap on the meters (the aquarium) ──────────────────────────────────────
// Tapping the glass while he's home gets a reaction. The full peek (startled
// hop, he ducks out, the face comes up and looks where you tapped) is the
// face's eye contact, so it's rationed to one per `peek_cooldown_ms`; taps in
// between get a small corner reaction. Nothing ever queues: a tap that lands
// while he's mid-reaction (or walking home) is simply absorbed, and the
// cooldown only starts when a peek actually starts.
enum TapReact {
    TAP_NONE,    // absorbed: minimal mode, not home, or he's already reacting
    TAP_HURRY,   // he's out (lurk trip / walking): he hurries back to his seat
    TAP_SMALL,   // a small corner reaction (clawd_tap_act)
    TAP_PEEK,    // the full face peek
};
enum CornerState {
    CORNER_FREE,   // seated, or in an idle act of his own (interruptible)
    CORNER_BUSY,   // a commanded act or a walk back to the seat
    CORNER_OUT,    // away from his seat on his own (lurk trip)
};
struct TapInput {
    uint32_t    now;
    bool        home;          // the director is in its home beat (meters, nothing staged)
    bool        minimal;       // minimal mode: Clawd stays still
    CornerState corner;
    bool        meters_live;   // fresh data; else the face would hide the pairing / stale hint
};
struct PeekGate { bool peeked; uint32_t last_peek; };
void     peek_gate_clear(PeekGate* g);
void     peek_gate_mark(PeekGate* g, uint32_t now);   // a peek started
TapReact clawd_tap_react(const PeekGate* g, const TapInput* in, const ClawdTuning* t);

// The small corner reactions (official acts that open on a crouch, so the
// reaction shows on the next frame). Never the same twice running; the
// peek's opening hop counts as "jumping".
extern const char* const CLAWD_TAP_ACTS[];
extern const int         CLAWD_TAP_ACT_COUNT;
const char* clawd_tap_act(const char* last, ClawdRng* r);

// The peek's face stays up until you tap it: after the look and the blink it
// rests on the glass like the screensaver face (irregular blinks, some
// double, glances), which also keeps the flat orange field from burning in.
// While it's held, prompts, stops, usage moving, buttons, a fresh window and
// an earned show neither drop it nor get their usual corner reaction (he's
// out of his seat, up at the glass); they still count as engagement. The
// fresh-window joy keeps its usual 5 min wait for him to sit down. A tap
// drops it (the acknowledging blink first). Attention interrupts it like
// anything else and ends on the meters, never back on the face (the alert
// started from the meters: ATTN_FROM_HOME). Each tick of the hold asks:
enum PeekHold {
    PEEK_HOLD_KEEP,          // stay up, resting
    PEEK_HOLD_DROP,          // the meters stopped being live: drop to show the pairing / stale hint
    PEEK_HOLD_SCREENSAVER,   // nobody's been around for quiet_ms: it becomes the
                             // screensaver in place (same face, already up, he's
                             // already out), so calm dips take over the burn-in care
};
PeekHold peek_hold(const QuietInput* q, const ClawdTuning* t);

// Where the peeking face looks: a glance proportional to how far from the
// centre the tap landed, ±max_px at the edges (the far eye then travels 1.5x
// that, face_eye_offsets).
int peek_glance_px(int tap_x, int width, int max_px);

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
