#pragma once
#include <stdint.h>

// The one sound Clawdmeter makes (attention alert and session-reset chime):
// an ding. A single sine strike at C#5 with an instant attack
// and an exponential decay, synthesized here so no audio file ships. The
// desktop app plays the same ding (clawdmeter-app/src/chime.rs); keep the
// constants in step.
//
// Pure math, no Arduino: native-tested in test/test_ding.

#define DING_FREQ_HZ    554.37f   // C#5
#define DING_PARTIAL3   0.02f     // 3rd partial (1663 Hz), 2 % of the strike
#define DING_ATTACK_S   0.003f    // ramp up, so the strike doesn't click
#define DING_TAU_S      0.24f     // decay time constant
#define DING_LENGTH_S   1.4f      // cut (the decay is ~-50 dB by then)
#define DING_FADE_S     0.02f     // fade to zero before the cut

// Frames in one ding at `rate` Hz.
uint32_t ding_frames(uint32_t rate);

// Frame `n` of the ding at `rate` Hz, in -1..1 (0 past the end).
float ding_value(uint32_t n, uint32_t rate);

// Frame `n` as a 16-bit sample at `gain` (0..1 of full scale).
int16_t ding_sample(uint32_t n, uint32_t rate, float gain);
