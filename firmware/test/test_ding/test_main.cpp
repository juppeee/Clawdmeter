// ding: the synthesized chime (pitch, attack, decay, length).
//   pio test -d firmware -e native -f test_ding
#include <unity.h>
#include <math.h>
#include "ding.h"

static const uint32_t RATE = 44100;

void setUp(void) {}
void tearDown(void) {}

// Peak |value| over frames [a, b).
static float peak(uint32_t a, uint32_t b) {
    float p = 0;
    for (uint32_t n = a; n < b; n++) p = fmaxf(p, fabsf(ding_value(n, RATE)));
    return p;
}

static void test_length_is_1_4_seconds(void) {
    TEST_ASSERT_EQUAL_UINT32(61740, ding_frames(RATE));
    TEST_ASSERT_EQUAL_FLOAT(0.0f, ding_value(ding_frames(RATE), RATE));
    TEST_ASSERT_EQUAL_FLOAT(0.0f, ding_value(0, 0));
}

static void test_starts_silent_and_ramps_without_a_click(void) {
    TEST_ASSERT_EQUAL_FLOAT(0.0f, ding_value(0, RATE));
    // The first millisecond stays well under the strike.
    TEST_ASSERT_TRUE(peak(0, RATE / 1000) < 0.4f);
    // By 3 ms it's at full strike, below full scale.
    float p = peak(RATE * 3 / 1000, RATE * 6 / 1000);
    TEST_ASSERT_TRUE(p > 0.9f && p <= 1.0f);
}

static void test_decays_with_a_quarter_second_time_constant(void) {
    // One period around t and around t + tau: the ratio is 1/e.
    const uint32_t period = (uint32_t)(RATE / 554.37f) + 1;
    const uint32_t t = RATE / 10, tau = (uint32_t)(0.24f * RATE);
    float ratio = peak(t + tau, t + tau + period) / peak(t, t + period);
    TEST_ASSERT_FLOAT_WITHIN(0.03f, 1.0f / 2.71828f, ratio);
    // About -50 dB near the end, and silent at the cut.
    TEST_ASSERT_TRUE(peak(ding_frames(RATE) - RATE / 10, ding_frames(RATE)) < 0.006f);
    TEST_ASSERT_TRUE(fabsf(ding_value(ding_frames(RATE) - 1, RATE)) < 0.0005f);
}

static void test_rings_at_c_sharp_5(void) {
    // Count rising zero crossings over 0.5 s: 554.37 Hz.
    int rising = 0;
    for (uint32_t n = 1; n < RATE / 2; n++)
        if (ding_value(n - 1, RATE) < 0 && ding_value(n, RATE) >= 0) rising++;
    TEST_ASSERT_INT_WITHIN(2, 277, rising);
}

static void test_samples_scale_and_clamp(void) {
    const uint32_t n = RATE * 4 / 1000;
    TEST_ASSERT_EQUAL_INT16(0, ding_sample(n, RATE, 0.0f));
    int16_t full = ding_sample(n, RATE, 1.0f);
    TEST_ASSERT_EQUAL_INT16(full, ding_sample(n, RATE, 2.0f));
    TEST_ASSERT_INT_WITHIN(1, full / 2, ding_sample(n, RATE, 0.5f));
}

int main(int, char**) {
    UNITY_BEGIN();
    RUN_TEST(test_length_is_1_4_seconds);
    RUN_TEST(test_starts_silent_and_ramps_without_a_click);
    RUN_TEST(test_decays_with_a_quarter_second_time_constant);
    RUN_TEST(test_rings_at_c_sharp_5);
    RUN_TEST(test_samples_scale_and_clamp);
    return UNITY_END();
}
