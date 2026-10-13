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
    return UNITY_END();
}
