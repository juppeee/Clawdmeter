#include "board.h"
#include <Arduino.h>

// SPI is brought up inside display_hal_init().
// No IO expander, no I2C peripherals are used by this port.
extern "C" void board_init(void) {
    // Intentionally empty.
}
