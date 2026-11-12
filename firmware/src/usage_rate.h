#pragma once

// Tracks short-term rate of change in the limiting usage percentage so the UI
// can react to *how heavily* Claude is being used right now, not just the
// current bucket level. Returns one of 4 group indices for the splash to pick
// animations from.
//
// The rate is judged relative to the window it belongs to: "heavy" means the
// limit is filling at least as fast as the window itself runs out. That keeps
// the groups meaningful for both the 5-hour window and the weekly one, which
// on weekly-only accounts is the only number there is.

#define USAGE_WINDOW_SESSION_MIN  300     // 5 hours
#define USAGE_WINDOW_WEEKLY_MIN   10080   // 7 days

// Feed in the latest percentage every time fresh BLE data arrives, together
// with the length of the window it measures. Returns true when this sample is
// a limit reset (pct dropped substantially vs the previous sample) — the
// caller uses this to chime the buzzer. Never true on the first sample after
// boot, nor when the tracked window changes (nothing comparable to drop from).
bool usage_rate_sample(float pct, int window_mins = USAGE_WINDOW_SESSION_MIN);

// 0 = idle, 1 = normal, 2 = active, 3 = heavy.
// Defaults to 0 when the buffer doesn't have enough samples yet.
int usage_rate_group(void);
