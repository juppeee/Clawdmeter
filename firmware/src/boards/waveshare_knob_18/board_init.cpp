#include "board.h"
#include <Arduino.h>
#include <Wire.h>

// Bring up the shared I2C bus (touch + haptic). No IO expander and no power
// latch on this board; the LCD and touch resets are direct GPIOs driven by
// their own HAL inits.
extern "C" void board_init(void) {
    Wire.begin(IIC_SDA, IIC_SCL);
}
