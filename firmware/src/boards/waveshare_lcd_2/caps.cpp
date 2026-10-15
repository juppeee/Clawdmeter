#include "../../hal/board_caps.h"
#include "board.h"

static const BoardCaps caps = {
    .name = BOARD_NAME,
    .width = LCD_WIDTH,
    .height = LCD_HEIGHT,
    // No user-reachable buttons (KEY1 = reset, KEY2 = BOOT/LCD reset); the
    // count of 1 is the minimum shared code accepts, input.cpp never reports it.
    .button_count = (uint8_t)(1 + BOARD_HAS_SECONDARY_BUTTON),
    .has_rotation = (bool)BOARD_HAS_ROTATION,
    .has_battery  = (bool)BOARD_HAS_BATTERY,
    .has_imu      = (bool)BOARD_HAS_IMU,
    .minimal_ui   = false,   // full animated UI; true for e-ink / slow panels
    .has_pwr_button = false, // pair with the 5 s touch hold on the hint
};

const BoardCaps& board_caps(void) { return caps; }
