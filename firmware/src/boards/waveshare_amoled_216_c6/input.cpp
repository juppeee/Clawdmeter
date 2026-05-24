#include "../../hal/input_hal.h"
#include "board.h"
#include <Arduino.h>

// Only BOOT is wired to a MCU GPIO. The "PWR" side button is the AXP2101
// PKEY, handled in power.cpp.

void input_hal_init(void) {
    pinMode(BTN_BACK_GPIO, INPUT_PULLUP);
}

bool input_hal_is_held(InputButton btn) {
    if (btn == INPUT_BTN_PRIMARY) {
        return digitalRead(BTN_BACK_GPIO) == LOW;
    }
    return false;
}
