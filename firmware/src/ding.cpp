#include "ding.h"
#include <math.h>

static const float TWO_PI_F = 6.28318530718f;

uint32_t ding_frames(uint32_t rate) {
    return (uint32_t)(DING_LENGTH_S * (float)rate);
}

float ding_value(uint32_t n, uint32_t rate) {
    const uint32_t len = ding_frames(rate);
    if (rate == 0 || n >= len) return 0.0f;
    const float t = (float)n / (float)rate;
    float env = expf(-t / DING_TAU_S);
    if (t < DING_ATTACK_S) env *= t / DING_ATTACK_S;
    const float left = DING_LENGTH_S - t;
    if (left < DING_FADE_S) env *= left / DING_FADE_S;
    const float ph = TWO_PI_F * DING_FREQ_HZ * t;
    const float wave = (sinf(ph) + DING_PARTIAL3 * sinf(3.0f * ph)) / (1.0f + DING_PARTIAL3);
    return env * wave;
}

int16_t ding_sample(uint32_t n, uint32_t rate, float gain) {
    if (gain < 0.0f) gain = 0.0f;
    if (gain > 1.0f) gain = 1.0f;
    return (int16_t)lrintf(ding_value(n, rate) * gain * 32767.0f);
}
