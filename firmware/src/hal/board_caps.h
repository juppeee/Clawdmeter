#pragma once
#include <stdint.h>

// Runtime board description consumed by board-agnostic code (UI, main loop).
// Each board provides a single BoardCaps instance via board_caps().
//
// Compile-time-only facts (pin numbers, library choice) belong in
// boards/<name>/board.h and never leak into shared code. Anything the UI or
// main loop needs at runtime — display size, optional-feature presence —
// goes here so shared code stays free of #ifdef BOARD_*.
struct BoardCaps {
    const char* name;        // human-readable, e.g. "Waveshare AMOLED 2.16"

    int16_t width;           // active display width in pixels
    int16_t height;          // active display height in pixels

    uint8_t button_count;    // 1 = primary (BOOT) only; 2 = primary + secondary
    bool    has_rotation;    // IMU-driven CPU rotation in the flush callback
    bool    has_battery;     // AXP2101 battery measurement is meaningful
    bool    has_imu;         // QMI8658 (or compatible) is populated

    // Fields below default to false for boards that don't set them.
    bool    has_encoder;     // rotary ring/knob: input_hal_encoder_steps() replaces the PWR short press
    bool    is_round;        // circular panel: only the inscribed circle is visible
    bool    touch_keys;      // no reachable keys: touch hold = Space (PTT), double tap = Shift+Tab,
                             // 3 s hold + release while disconnected = pair
    const char* pair_key;    // hold-to-pair key as the pairing hint names it; nullptr = "the power button"
};

const BoardCaps& board_caps(void);
