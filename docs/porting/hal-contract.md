# HAL contract

Each header under `firmware/src/hal/` defines functions that a board port
must provide. The shared code (`main.cpp`, `ui.cpp`, `splash.cpp`) calls
these and nothing else. Where a function has non-functional requirements
(latency, ordering), they're listed here — silently violating them tends
to produce subtle bugs (dropped frames, missed events) rather than
crashes.

## `board_caps.h`

Runtime description of the board. Provided by your `caps.cpp` as a
single `const BoardCaps` instance returned from `board_caps()`. The UI
queries this at startup and gates optional features (battery indicator,
secondary-button HID mapping) by what's true here. Keep the struct in
sync with the compile-time `BOARD_HAS_*` flags in `board.h`.

## `display_hal.h`

| Function                    | Responsibility |
|-----------------------------|----------------|
| `display_hal_init`          | Construct the QSPI bus + driver. Must run AFTER `board_init()` so any IO expander has released the LCD reset line. |
| `display_hal_begin`         | `gfx->begin()`, clear screen, set default brightness. Allocate any rotation buffers needed by `display_hal_draw_bitmap`. |
| `display_hal_set_brightness`| Pass-through to the driver. Driver-defined scale (typically 0..255). |
| `display_hal_fill_screen`   | Used by tests / boot screen — `gfx->fillScreen(color)`. |
| `display_hal_draw_bitmap`   | Push a w×h RGB565 strip at (x, y). If the panel can't rotate natively, apply CPU rotation here before pushing — `imu_hal_rotation_quadrant()` returns the current orientation. **Must complete inside LVGL's render budget** (a few ms at typical strip sizes). |
| `display_hal_tick`          | Per-loop housekeeping — used by rotation-aware boards to blank the panel + ramp brightness during a rotation transition. No-op on boards without rotation. |
| `display_hal_round_area`    | LVGL invalidate-area hook. Most QSPI AMOLED drivers expect even-aligned flush regions; apply `& ~1` / `| 1` to coordinates. |

## `touch_hal.h`

| Function          | Responsibility |
|-------------------|----------------|
| `touch_hal_init`  | Initialize the controller + attach a touch interrupt. Configure axis swap / mirror so coordinates returned in `touch_hal_read` match the panel's pixel coordinates after any rotation. |
| `touch_hal_read`  | Return the latest sample. **Hard requirement: complete in well under 5 ms** — LVGL polls this every screen refresh and any I2C burst longer than a screen tick will visibly stutter. |

Avoid GPL-licensed drivers — vendor a minimal reader instead. The
existing AMOLED-1.8 port has a ~40-line FT3168 reader you can model on.

## `input_hal.h`

| Function          | Responsibility |
|-------------------|----------------|
| `input_hal_init`  | `pinMode()` for the physical button GPIOs. |
| `input_hal_is_held` | Return true while the button is held. Active-low pull-up GPIOs are typical. Boards lacking a secondary button must return `false` for `INPUT_BTN_SECONDARY`. |

The PWR button is **not** here — it belongs to `power_hal` because on
several boards (including all current reference ports) it's tied to the
PMU or an IO expander, not a GPIO.

## `power_hal.h`

| Function                | Responsibility |
|-------------------------|----------------|
| `power_hal_init`        | Bring up the PMU (if any). Configure battery measurement. Subscribe to the PWR button source (PMU IRQ or IO expander polling). |
| `power_hal_tick`        | Refresh battery % and charging state at sensible intervals (the reference ports use 2s / 500 ms). Poll the PWR button if it's not interrupt-driven. |
| `power_hal_battery_pct` | 0..100, or `-1` when battery info isn't available. |
| `power_hal_is_charging` | Bool, false on no-battery boards. |
| `power_hal_pwr_pressed` | **Edge-triggered**: returns true once per short-press, then clears. Shared code calls this every loop and expects one true per press. |

Boards with no PMU and no PWR button can return zero/false from all five
— set `BOARD_HAS_BATTERY=0` and the UI hides the battery indicator.

## `imu_hal.h`

| Function                     | Responsibility |
|------------------------------|----------------|
| `imu_hal_init`               | Bring up the accelerometer. |
| `imu_hal_tick`               | Sample the accelerometer at a low rate (~10 Hz) and update the rotation state with hysteresis. |
| `imu_hal_rotation_quadrant`  | Current rotation, 0..3 (quarter turns CW). Used by `display_hal_draw_bitmap` on rotation-capable boards. Boards without rotation always return 0. |

## `board_info_hal.h` (optional)

Weak defaults live in `firmware/src/board_info_default.cpp`; implement a
function in your board folder only when it has something to report.

| Function         | Responsibility |
|------------------|----------------|
| `board_hal_rev`  | Short, stable hardware-revision token for the `info` serial command (`{"env","ver","rev","link"}`), which the desktop flasher uses to identify a board before re-flashing. Default `""`. Valid after `board_init()`; return a static string. AMOLED-1.8 returns `sh8601_ft3168` or `co5300_cst816` from its boot-time probe. |

## Serial console commands

`check_serial_cmd()` in `main.cpp` (115200 baud, newline-terminated):

| Command                   | Reply |
|---------------------------|-------|
| `screenshot`              | `SCREENSHOT_START w h bytes`, raw RGB565, `SCREENSHOT_END` (`SCREENSHOT_UNSUPPORTED` without PSRAM) |
| `buzz`                    | plays the reset chime (no reply) |
| `info`                    | one JSON line: `{"env":"waveshare_amoled_18","ver":"v1.9.0","rev":"co5300_cst816","link":"ble"}`. `env` comes from `-DCLAWD_ENV` (set per env in `platformio.ini` from `${this.__env__}`), `ver` from `-DCLAWD_VERSION` (defaults to `dev`; CI sets `PLATFORMIO_BUILD_FLAGS='-DCLAWD_VERSION=\"vX.Y.Z\"'`), `rev` from `board_hal_rev()`. |
| `minimal [on\|off\|auto]` | sets (or with no argument, reports) the minimal-mode override; replies `{"minimal":true,"override":"on"}` |
| `clawd <beat>`            | starts a Clawd beat now, for `screenshot` QA without waiting for the situation: `show`, `screensaver`, `attention`, `joy`, `home` (a tap). Replies `{"clawd":"show","ok":true}`. See `docs/clawd-director.md`. |

## Responsive UI breakpoints

`ui.cpp::compute_layout()` picks layout values from `board_caps().width`
and `.height`. The current breakpoints are:

- **`height >= 460`** → "large" layout, tuned for 480×480.
- **otherwise** → "compact" layout, tuned for 368×448.

A new screen size lands on the closer breakpoint and renders correctly
without pixel-perfect alignment. If you want polish, add another branch
to `compute_layout()` (please open a PR — others with that size benefit).

The splash screen is fully responsive — `CELL` is computed as
`min(width, height) / 20` so the 20×20 pixel-art creature fills the
smaller display dimension and centers in the larger one.
