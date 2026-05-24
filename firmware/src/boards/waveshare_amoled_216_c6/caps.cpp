#include "../../hal/board_caps.h"
#include "board.h"

static const BoardCaps caps = {
    .name = BOARD_NAME,
    .width = LCD_WIDTH,
    .height = LCD_HEIGHT,
    .button_count = 1,    // BOOT only; PWR is via AXP PKEY IRQ
    .has_rotation = (bool)BOARD_HAS_ROTATION,
    .has_battery = (bool)BOARD_HAS_BATTERY,
    .has_imu = (bool)BOARD_HAS_IMU,
};

const BoardCaps& board_caps(void) { return caps; }
