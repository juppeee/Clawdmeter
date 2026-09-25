#include "../../hal/board_caps.h"
#include "board.h"

static const BoardCaps caps = {
    .name = BOARD_NAME,
    .width = LCD_WIDTH,
    .height = LCD_HEIGHT,
    // BOOT sits on the PCB but can't be reached with the case closed, so the
    // keys move elsewhere: the ring takes the PWR short press (has_encoder),
    // the touchscreen takes Space, Shift+Tab and pairing (touch_keys).
    .button_count = 1,
    .has_rotation = (bool)BOARD_HAS_ROTATION,
    .has_battery  = (bool)BOARD_HAS_BATTERY,
    .has_imu      = (bool)BOARD_HAS_IMU,
    .has_encoder  = true,
    .is_round     = true,
    .touch_keys   = true,
    .pair_key     = "the screen",
};

const BoardCaps& board_caps(void) { return caps; }
