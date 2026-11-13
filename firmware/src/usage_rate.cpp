#include "usage_rate.h"
#include <Arduino.h>

// Thresholds as a multiple of "pace" — 1.0 means the limit fills exactly as
// fast as its window runs out (100% over the whole window). For the 5-hour
// window that is 0.33 %/min, and the tiers below work out to the original
// %/min numbers:
//   < 0.30  →  Idle    (5h: <0.10 %/min, 17h+ to fill, basically dormant)
//   < 0.60  →  Normal  (5h: <0.20 %/min, slow steady use)
//   < 1.00  →  Active  (5h: <0.33 %/min, heavy but not yet pace-matching)
//   >=1.00  →  Heavy   (matching or beating the window's own reset)
#define PACE_THRESH_NORMAL  0.30f
#define PACE_THRESH_ACTIVE  0.60f
#define PACE_THRESH_HEAVY   1.00f

// Minimum span between oldest and newest sample before we trust the computed
// rate. The whole point of the ring buffer is to smooth out single-sample
// jitter — at 60s daemon polling, a 1% bump between two consecutive samples
// looks like 1 %/min (Heavy) but really just means you grew 1% in the last
// minute. We require ~4 min of accumulated history so the rate reflects a
// real trend, not one noisy delta. Side-effect: ~4 min warm-up after boot
// during which we report Idle.
#define FAST_MIN_SPAN_MS    240000UL

// The weekly window moves ~34x slower and the API reports it in whole
// percent, so a few minutes of history can't show anything but 0 or a spike.
// Keep one sample per half hour instead (6 slots ≈ 2.5 h of history) and
// wait for an hour of it. One percent of a week over that history is roughly
// 0.6 pace (Normal/Active); two or more is Heavy.
#define SLOW_KEEP_EVERY_MS  1800000UL
#define SLOW_MIN_SPAN_MS    3600000UL

#define RING_SIZE 6

struct Sample { uint32_t ms; float pct; };

static Sample ring[RING_SIZE];
static uint8_t count = 0;
static uint8_t head  = 0;  // index of next write slot
static Sample  latest = {};       // most recent sample, whether or not kept in the ring
static bool    have_latest = false;
static int     cur_window = 0;    // window length (min) the history belongs to

static inline bool slow_mode(void) { return cur_window >= 1440; }

static inline uint8_t oldest_idx(void) {
    return (head + RING_SIZE - count) % RING_SIZE;
}

static inline uint8_t newest_idx(void) {
    return (head + RING_SIZE - 1) % RING_SIZE;
}

static void usage_rate_reset(void) {
    count = 0;
    head  = 0;
    have_latest = false;
}

bool usage_rate_sample(float pct, int window_mins) {
    uint32_t now = millis();
    bool was_reset = false;

    if (window_mins != cur_window) {
        // Different limit than the history describes (plan changed, or the
        // host started/stopped reporting a window): start over, silently.
        usage_rate_reset();
        cur_window = window_mins;
    } else if (have_latest && pct + 5.0f < latest.pct) {
        // Limit reset: pct dropped substantially. Restart tracking.
        usage_rate_reset();
        was_reset = true;
    }

    latest = { now, pct };
    have_latest = true;

    // Fast window: keep every sample. Slow window: thin them out so the ring
    // spans hours rather than minutes.
    const uint32_t keep_every = slow_mode() ? SLOW_KEEP_EVERY_MS : 0;
    if (count == 0 || now - ring[newest_idx()].ms >= keep_every) {
        ring[head] = latest;
        head = (head + 1) % RING_SIZE;
        if (count < RING_SIZE) count++;
    }

    return was_reset;
}

int usage_rate_group(void) {
    if (count == 0 || !have_latest || cur_window <= 0) return 0;

    const Sample& o = ring[oldest_idx()];
    uint32_t dt = latest.ms - o.ms;
    if (dt < (slow_mode() ? SLOW_MIN_SPAN_MS : FAST_MIN_SPAN_MS)) return 0;

    float dp = latest.pct - o.pct;
    if (dp < 0.0f) dp = 0.0f;
    float rate = dp * 60000.0f / (float)dt;            // %/min
    float pace = rate * (float)cur_window / 100.0f;    // 1.0 = fills in one window

    if (pace < PACE_THRESH_NORMAL) return 0;
    if (pace < PACE_THRESH_ACTIVE) return 1;
    if (pace < PACE_THRESH_HEAVY)  return 2;
    return 3;
}
