#include "../../hal/sound_hal.h"

// The PCM5100A DAC only drives a line-out on the expansion connector (no amp
// or speaker on the board), and its soft-mute pin is owned by the second MCU.
// No chime here.
//
// The DRV2605 was tried as a substitute for the pairing cues and dropped: on
// this kit the vibration is too weak to notice, whatever it is fed. The driver
// is not the problem — it reports DEVICE_ID 7, auto-calibration passes
// (DIAG_RESULT clear) and measures a real back-EMF, so the motor does turn.
// Every ROM effect from Strong Click to Long Buzz went unnoticed, and so did
// constant full-amplitude drive (RTP) both at the stock ~3 V clamp and at the
// 5.4 V maximum. The motor is simply too small for the mass of the knob it has
// to shake. Pairing feedback on this board is the on-screen overlay alone; the
// ring's detent click stays, since there the fingertips are already on the
// ring that moves.

void sound_hal_init(void) {}
void sound_hal_tick(void) {}
void sound_hal_play_reset(void) {}
void sound_hal_play_pair_armed(void) {}
void sound_hal_play_paired(void) {}
