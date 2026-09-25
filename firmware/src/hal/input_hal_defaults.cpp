#include "input_hal.h"

// Boards without a rotary ring don't implement input_hal_encoder_steps(); this
// weak default keeps them building without touching every board folder. A
// board that defines the function overrides it at link time.
__attribute__((weak)) int input_hal_encoder_steps(void) { return 0; }
