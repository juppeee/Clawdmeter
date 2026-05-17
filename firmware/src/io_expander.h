#pragma once

#include <stdint.h>

// XCA9554 / PCA9554-compatible 8-bit I2C IO expander @ 0x20.
// Only compiled for BOARD_AMOLED_18 (the AMOLED-1.8 board routes LCD_RST,
// TP_RST, audio amp enable, and the PWR button through this expander).
//
// Must be initialized BEFORE the display or touch — skipping the reset
// release leaves the SH8601 and FT3168 in reset and they will fail to probe.

bool io_expander_init(void);

// Drive an EXIO pin (one of IOX_PIN_* in display_cfg.h that is configured
// as output). Updates the cached output register.
void io_expander_set(uint8_t pin, bool high);

// Read an EXIO pin configured as input (e.g. PWR button on EXIO4).
bool io_expander_get(uint8_t pin);
