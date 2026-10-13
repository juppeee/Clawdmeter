// clawd_policy: the director's decision rules (shows, screensaver, turns,
// no-repeat picks, the resting face's rhythm).
//   pio test -d firmware -e native -f test_clawd_policy
#include <unity.h>
#include <string.h>
#include "clawd_policy.h"

static const uint32_t SEC = 1000UL;
static const uint32_t MIN = 60UL * SEC;
static ClawdTuning T;

void setUp(void) { clawd_tuning_defaults(&T); }
void tearDown(void) {}

static void test_show_needs_five_sustained_minutes_of_level_3(void) {
    ShowGate g; show_gate_clear(&g);
    show_gate_update(&g, 0, 3);
    show_gate_update(&g, 4 * MIN, 3);
    TEST_ASSERT_FALSE(show_gate_ready(&g, 4 * MIN, &T));
    show_gate_update(&g, 5 * MIN, 3);
    TEST_ASSERT_TRUE(show_gate_ready(&g, 5 * MIN, &T));
}

static void test_show_clock_restarts_when_activity_drops(void) {
    ShowGate g; show_gate_clear(&g);
    show_gate_update(&g, 0, 3);
    show_gate_update(&g, 4 * MIN, 2);              // a lull
    show_gate_update(&g, 4 * MIN + SEC, 3);
    TEST_ASSERT_FALSE(show_gate_ready(&g, 8 * MIN, &T));
    TEST_ASSERT_TRUE(show_gate_ready(&g, 9 * MIN + SEC, &T));
    show_gate_update(&g, 10 * MIN, -1);            // expired / unknown counts as a drop
    TEST_ASSERT_FALSE(show_gate_ready(&g, 20 * MIN, &T));
}

static void test_at_most_one_show_per_15_minutes(void) {
    ShowGate g; show_gate_clear(&g);
    show_gate_update(&g, 0, 3);
    TEST_ASSERT_TRUE(show_gate_ready(&g, 5 * MIN, &T));
    show_gate_mark_shown(&g, 5 * MIN);
    show_gate_update(&g, 11 * MIN, 3);             // busy the whole time
    TEST_ASSERT_FALSE(show_gate_ready(&g, 11 * MIN, &T));   // 6 min busy, but cooldown
    TEST_ASSERT_FALSE(show_gate_ready(&g, 19 * MIN, &T));
    TEST_ASSERT_TRUE(show_gate_ready(&g, 20 * MIN, &T));
}

static void test_screensaver_after_ten_quiet_minutes_with_hooks(void) {
    QuietInput q = { 10 * MIN - 1, 0, true, false, true };
    TEST_ASSERT_FALSE(screensaver_ready(&q, &T));
    q.now = 10 * MIN;
    TEST_ASSERT_TRUE(screensaver_ready(&q, &T));
    q.meters_live = false;                          // pairing / stale: never
    TEST_ASSERT_FALSE(screensaver_ready(&q, &T));
}

static void test_screensaver_without_hooks_only_for_unstarted_window(void) {
    QuietInput q = { 30 * MIN, 0, false, false, true };
    TEST_ASSERT_FALSE(screensaver_ready(&q, &T));  // old daemon: slow clock only
    q.window_not_started = true;
    TEST_ASSERT_TRUE(screensaver_ready(&q, &T));
}

static void test_engagement_wraps_safely(void) {
    QuietInput q = { 5 * MIN, 0xFFFFFFFFu - MIN, true, false, true };  // millis wrapped
    TEST_ASSERT_FALSE(screensaver_ready(&q, &T));                    // 6 min ago
    q.now = 9 * MIN + 1;
    TEST_ASSERT_TRUE(screensaver_ready(&q, &T));
}

static void test_long_turn_earns_a_wave(void) {
    TurnTracker t; turn_clear(&t);
    TEST_ASSERT_FALSE(turn_on_event(&t, 'p', 0, &T));
    TEST_ASSERT_FALSE(turn_on_event(&t, 'p', 60 * SEC, &T));    // follow-up keeps the start
    TEST_ASSERT_TRUE(turn_on_event(&t, 's', 2 * MIN, &T));
    TEST_ASSERT_FALSE(turn_on_event(&t, 's', 3 * MIN, &T));     // no open turn
    TEST_ASSERT_FALSE(turn_on_event(&t, 'p', 4 * MIN, &T));
    TEST_ASSERT_FALSE(turn_on_event(&t, 's', 5 * MIN, &T));     // a 1-minute turn
}

static void test_pick_never_repeats(void) {
    static const char* const acts[] = { "dancing", "waving", "dancing", "lurking" };
    ClawdRng r; clawd_rng_seed(&r, 42);
    const char* last = NULL;
    for (int i = 0; i < 200; i++) {
        int k = clawd_pick_no_repeat(acts, 4, last, &r);
        TEST_ASSERT_TRUE(k >= 0 && k < 4);
        if (last) TEST_ASSERT_TRUE(strcmp(acts[k], last) != 0);
        last = acts[k];
    }
    static const char* const one[] = { "pointing", NULL };
    TEST_ASSERT_EQUAL_INT(0, clawd_pick_no_repeat(one, 2, "pointing", &r));  // only choice
    static const char* const none[] = { NULL, NULL };
    TEST_ASSERT_EQUAL_INT(-1, clawd_pick_no_repeat(none, 2, NULL, &r));
}

static void test_jitter_stays_within_40_percent(void) {
    ClawdRng r; clawd_rng_seed(&r, 7);
    bool lo = false, hi = false;
    for (int i = 0; i < 500; i++) {
        uint32_t v = clawd_jitter(10000, &r);
        TEST_ASSERT_TRUE(v >= 6000 && v <= 14000);
        if (v < 8000) lo = true;
        if (v > 12000) hi = true;
    }
    TEST_ASSERT_TRUE(lo && hi);                    // actually varies
}

static void test_face_rhythm_is_irregular_with_some_double_blinks(void) {
    ClawdRng r; clawd_rng_seed(&r, 1234);
    int blinks = 0, doubles = 0, glances = 0;
    uint32_t gmin = 0xFFFFFFFF, gmax = 0;
    for (int i = 0; i < 1000; i++) {
        FacePlan p = face_plan_next(&r, &T);
        if (p.gap_ms < gmin) gmin = p.gap_ms;
        if (p.gap_ms > gmax) gmax = p.gap_ms;
        if (p.act == FACE_BLINK) blinks++;
        else if (p.act == FACE_DOUBLE_BLINK) doubles++;
        else glances++;
    }
    TEST_ASSERT_TRUE(gmin >= 2000 && gmax <= 6000);
    TEST_ASSERT_TRUE(gmax - gmin > 3000);
    TEST_ASSERT_TRUE(doubles > 50 && doubles < blinks);   // occasional, not the norm
    TEST_ASSERT_TRUE(glances > 200);
}

static void test_far_eye_travels_further(void) {
    int l, r;
    face_eye_offsets(20, &l, &r);
    TEST_ASSERT_EQUAL_INT(20, l);
    TEST_ASSERT_EQUAL_INT(30, r);
    face_eye_offsets(-20, &l, &r);
    TEST_ASSERT_EQUAL_INT(-30, l);
    TEST_ASSERT_EQUAL_INT(-20, r);
}

static void test_show_group_uses_base_rate_floored_at_active(void) {
    TEST_ASSERT_EQUAL_INT(2, clawd_show_group(0));   // idle usage, busy hooks: Active set
    TEST_ASSERT_EQUAL_INT(2, clawd_show_group(1));
    TEST_ASSERT_EQUAL_INT(2, clawd_show_group(2));
    TEST_ASSERT_EQUAL_INT(3, clawd_show_group(3));   // really burning: Heavy set
    TEST_ASSERT_EQUAL_INT(2, clawd_show_group(-1));  // no samples yet
    TEST_ASSERT_EQUAL_INT(3, clawd_show_group(7));
}

static void test_show_variety_is_reachable_and_never_repeats(void) {
    // The Active set a show now draws from with idle-to-active usage.
    static const char* const ACTIVE[] = { "laptop", "dancing", "skateboard", "soccer" };
    ClawdRng r; clawd_rng_seed(&r, 99);
    bool seen[4] = { false, false, false, false };
    const char* last = NULL;
    for (int i = 0; i < 200; i++) {
        const int k = clawd_pick_no_repeat(ACTIVE, 4, last, &r);
        TEST_ASSERT_TRUE(k >= 0 && k < 4);
        if (last) TEST_ASSERT_TRUE(strcmp(ACTIVE[k], last) != 0);
        seen[k] = true;
        last = ACTIVE[k];
    }
    for (int k = 0; k < 4; k++) TEST_ASSERT_TRUE(seen[k]);
}

static void test_attention_returns_to_an_unattended_screensaver(void) {
    TEST_ASSERT_TRUE(attention_resumes_screensaver(ATTN_FROM_SCREENSAVER, ATTN_END_TIMEOUT, false));
    TEST_ASSERT_TRUE(attention_resumes_screensaver(ATTN_FROM_SCREENSAVER, ATTN_END_CLEAR, false));
}

static void test_attention_ends_home_when_you_showed_up(void) {
    // A tap means "show me the meters", whatever was interrupted.
    TEST_ASSERT_FALSE(attention_resumes_screensaver(ATTN_FROM_SCREENSAVER, ATTN_END_TAP, false));
    // A prompt, button or tool activity during the alert: you're here.
    TEST_ASSERT_FALSE(attention_resumes_screensaver(ATTN_FROM_SCREENSAVER, ATTN_END_CLEAR, true));
    TEST_ASSERT_FALSE(attention_resumes_screensaver(ATTN_FROM_SCREENSAVER, ATTN_END_TIMEOUT, true));
    // An interrupted show ends gracefully home; home stays home.
    TEST_ASSERT_FALSE(attention_resumes_screensaver(ATTN_FROM_SHOW, ATTN_END_TIMEOUT, false));
    TEST_ASSERT_FALSE(attention_resumes_screensaver(ATTN_FROM_HOME, ATTN_END_CLEAR, false));
}

static void test_reaction_waits_for_him_to_hurry_home(void) {
    TEST_ASSERT_TRUE(reaction_still_due(1000, 1000, &T));
    TEST_ASSERT_TRUE(reaction_still_due(1000, 1000 + T.react_pending_ms, &T));
    TEST_ASSERT_FALSE(reaction_still_due(1000, 1001 + T.react_pending_ms, &T));
    TEST_ASSERT_TRUE(reaction_still_due(0xFFFFF000u, 0x00000100u, &T));   // wraps safely
    // Long enough for the lurk peek at speed plus the walk back at 2.5x.
    TEST_ASSERT_TRUE(T.react_pending_ms >= 10 * SEC);
}

static void test_corner_seat_leaves_headroom_for_the_jump(void) {
    // 480 panel: seat feet at 84, jump 30 cells x 3 px, first panel at 100.
    TEST_ASSERT_EQUAL_INT(92, corner_seat_feet(84, 90, 96, 2));
    // Already enough room: unchanged.
    TEST_ASSERT_EQUAL_INT(120, corner_seat_feet(120, 90, 200, 2));
    // 240 panel: 60 px jump can't fit above a panel at 44; clamped, never
    // pushed into the panel (the mascot swaps the act instead).
    TEST_ASSERT_EQUAL_INT(40, corner_seat_feet(38, 60, 40, 2));
    TEST_ASSERT_EQUAL_INT(38, corner_seat_feet(38, 60, 30, 2));   // never raised
}

int main(int, char**) {
    UNITY_BEGIN();
    RUN_TEST(test_show_needs_five_sustained_minutes_of_level_3);
    RUN_TEST(test_show_clock_restarts_when_activity_drops);
    RUN_TEST(test_at_most_one_show_per_15_minutes);
    RUN_TEST(test_screensaver_after_ten_quiet_minutes_with_hooks);
    RUN_TEST(test_screensaver_without_hooks_only_for_unstarted_window);
    RUN_TEST(test_engagement_wraps_safely);
    RUN_TEST(test_long_turn_earns_a_wave);
    RUN_TEST(test_pick_never_repeats);
    RUN_TEST(test_jitter_stays_within_40_percent);
    RUN_TEST(test_face_rhythm_is_irregular_with_some_double_blinks);
    RUN_TEST(test_far_eye_travels_further);
    RUN_TEST(test_show_group_uses_base_rate_floored_at_active);
    RUN_TEST(test_show_variety_is_reachable_and_never_repeats);
    RUN_TEST(test_attention_returns_to_an_unattended_screensaver);
    RUN_TEST(test_attention_ends_home_when_you_showed_up);
    RUN_TEST(test_reaction_waits_for_him_to_hurry_home);
    RUN_TEST(test_corner_seat_leaves_headroom_for_the_jump);
    return UNITY_END();
}
