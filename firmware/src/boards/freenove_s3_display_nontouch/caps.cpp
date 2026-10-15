#include "../../hal/board_caps.h"
#include "board.h"

static const BoardCaps caps = {
    .name = BOARD_NAME,
    .width = LCD_WIDTH,
    .height = LCD_HEIGHT,
    .button_count = (uint8_t)(1 + BOARD_HAS_SECONDARY_BUTTON),
    .has_rotation = (bool)BOARD_HAS_ROTATION,
    .has_battery  = (bool)BOARD_HAS_BATTERY,
    .has_imu      = (bool)BOARD_HAS_IMU,
    // Meters and a still corner Clawd only: with no touch nothing could
    // dismiss the stage or face, and the SPI TFT is slow.
    .minimal_ui   = true,
    .has_pwr_button = true,   // BOOT (GPIO0): brightness, hold-to-pair (power.cpp)
};

const BoardCaps& board_caps(void) { return caps; }
