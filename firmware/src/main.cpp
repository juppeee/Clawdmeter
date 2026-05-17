#include <Arduino.h>
#include <Wire.h>
#include <lvgl.h>
#include <ArduinoJson.h>
#include "display_cfg.h"
#include "data.h"
#include "ui.h"
#include "ble.h"
#include "power.h"
#include "imu.h"
#include "splash.h"
#include "usage_rate.h"
#include "idle.h"
#include "idle_cfg.h"

#ifdef BOARD_AMOLED_18
#include "io_expander.h"
#endif

// Physical buttons (global, screen-independent):
//   BTN_BACK (GPIO 0, BOOT)  — left, send Space (Claude Code voice-mode PTT)
//   BTN_FWD  (GPIO 18)       — AMOLED-2.16 only: Shift+Tab (mode toggle)
//   PWR                       — middle, cycle screens; on splash, cycle animations
//                                AMOLED-2.16: AXP2101 PKEY IRQ
//                                AMOLED-1.8 : XCA9554 EXIO4 (polled over I2C)
#define BTN_BACK 0
#ifndef BOARD_AMOLED_18
#define BTN_FWD  18
#endif

// ---- Hardware objects ----
Arduino_DataBus *bus = new Arduino_ESP32QSPI(
    LCD_CS, LCD_SCLK, LCD_SDIO0, LCD_SDIO1, LCD_SDIO2, LCD_SDIO3);
#ifdef BOARD_AMOLED_18
// SH8601 constructor: (bus, rst, rotation, w, h)
PlatformDisplay *gfx = new PlatformDisplay(
    bus, LCD_RESET /* GFX_NOT_DEFINED — reset via XCA9554 */, 0,
    LCD_WIDTH, LCD_HEIGHT);
#else
// CO5300 constructor: (bus, rst, rotation, w, h, col_offset1..2, row_offset1..2)
PlatformDisplay *gfx = new PlatformDisplay(
    bus, LCD_RESET, 0 /* rotation */,
    LCD_WIDTH, LCD_HEIGHT, 0, 0, 0, 0);
TouchDrvCST92xx touch;
#endif
XPowersPMU pmu;
SensorQMI8658 imu;

static UsageData usage = {};

// ---- Touch interrupt + shared state ----
// Centralized once-per-loop read (CLAUDE.md gotcha #5): calling getPoint() /
// reading FT3168 from multiple sites consumes each other's data.
static volatile bool     touch_pressed = false;
static volatile uint16_t touch_x = 0;
static volatile uint16_t touch_y = 0;
static volatile bool     touch_data_ready = false;

static void IRAM_ATTR touch_isr(void) {
    touch_data_ready = true;
}

#ifdef BOARD_AMOLED_18
// Minimal FT3168 reader (FocalTech standard register layout).
// Avoids vendoring Waveshare's GPLv3 Arduino_DriveBus library.
//   reg 0x02: low nibble = active finger count
//   reg 0x03/0x04: X1 high (low nibble), X1 low
//   reg 0x05/0x06: Y1 high (low nibble), Y1 low
static void ft3168_init(void) {
    // Power-mode register 0xA5 = 0x00: active scanning.
    Wire.beginTransmission(FT3168_ADDR);
    Wire.write(0xA5);
    Wire.write(0x00);
    Wire.endTransmission();
    // Verify device ID register 0xA0 (FT3168 reports 0x03 but Waveshare's
    // panel sometimes returns 0x86 — log but don't fail).
    Wire.beginTransmission(FT3168_ADDR);
    Wire.write(0xA0);
    if (Wire.endTransmission(false) == 0 && Wire.requestFrom(FT3168_ADDR, (uint8_t)1) == 1) {
        Serial.printf("FT3168 ID=0x%02X\n", Wire.read());
    } else {
        Serial.println("FT3168 ID read failed");
    }
}

static void ft3168_read_into_shared_state(void) {
    Wire.beginTransmission(FT3168_ADDR);
    Wire.write(0x02);
    if (Wire.endTransmission(false) != 0) { touch_pressed = false; return; }
    if (Wire.requestFrom(FT3168_ADDR, (uint8_t)5) != 5) { touch_pressed = false; return; }
    uint8_t fingers = Wire.read() & 0x0F;
    uint8_t xH = Wire.read();
    uint8_t xL = Wire.read();
    uint8_t yH = Wire.read();
    uint8_t yL = Wire.read();
    if (fingers == 0 || fingers > 5) {
        touch_pressed = false;
        return;
    }
    touch_x = ((uint16_t)(xH & 0x0F) << 8) | xL;
    touch_y = ((uint16_t)(yH & 0x0F) << 8) | yL;
    touch_pressed = true;
}
#endif

static void touch_read() {
    if (!touch_data_ready) return;
    touch_data_ready = false;

#ifdef BOARD_AMOLED_18
    ft3168_read_into_shared_state();
#else
    int16_t tx[5], ty[5];
    uint8_t n = touch.getPoint(tx, ty, touch.getSupportTouchPoint());
    if (n > 0) {
        touch_pressed = true;
        touch_x = (uint16_t)tx[0];
        touch_y = (uint16_t)ty[0];
    } else {
        touch_pressed = false;
    }
#endif

    // Touch policy is driven by IDLE_WAKE_ON_TOUCH:
    //   true  → a press edge while asleep wakes the device and the first
    //           touch is swallowed (mirrors the button wake-consumption); a
    //           press while awake counts as activity.
    //   false → touch never counts as activity and is fully swallowed while
    //           the panel is dark, so pets/sleeves can't wake it overnight
    //           and LVGL can't quietly toggle splash<->usage on a black panel.
    if (IDLE_WAKE_ON_TOUCH) {
        static bool touch_was = false;
        static bool touch_wake_swallowed = false;
        bool touch_now = touch_pressed;
        if (touch_now && !touch_was) {
            if (idle_consume_wake_press()) {
                touch_wake_swallowed = true;
                touch_pressed = false;  // hide this press from LVGL
            }
        } else if (!touch_now && touch_was) {
            if (touch_wake_swallowed) {
                touch_wake_swallowed = false;
                touch_pressed = false;  // also hide the corresponding release
            }
        } else if (touch_now && touch_wake_swallowed) {
            // Held finger through wake — keep hiding until release.
            touch_pressed = false;
        }
        touch_was = touch_now;
    } else {
        if (idle_is_asleep()) touch_pressed = false;
    }
}

// ---- LVGL draw buffers (PSRAM-backed, partial render) ----
#define BUF_LINES 40
static uint16_t *buf1 = nullptr;
static uint16_t *buf2 = nullptr;
// rot_buf for strip rotation — max size is 480×480 (full invalidation case)
// but typical partial strips are much smaller
static uint16_t *rot_buf = nullptr;

// LVGL tick callback
static uint32_t my_tick(void) {
    return millis();
}

#ifndef BOARD_AMOLED_18
// Rotate a w×h strip and compute destination coordinates on the 480×480 display.
// src pixels are in row-major order for the rectangle (sx, sy, w, h).
// Output goes to rot_buf in row-major order for the destination rectangle.
// AMOLED-1.8 port is fixed at 0° so this code is excluded.
static void rotate_strip(const uint16_t *src, int32_t w, int32_t h,
                         int32_t sx, int32_t sy, uint8_t r,
                         int32_t *dx, int32_t *dy, int32_t *dw, int32_t *dh) {
    const int S = LCD_WIDTH;  // 480

    switch (r) {
    case 1: { // 90° CW: (x,y) -> (S-1-y, x)
        *dw = h; *dh = w;
        *dx = S - sy - h;
        *dy = sx;
        for (int32_t y = 0; y < h; y++) {
            for (int32_t x = 0; x < w; x++) {
                // src(x,y) -> dst(h-1-y, x)
                rot_buf[x * h + (h - 1 - y)] = src[y * w + x];
            }
        }
        break;
    }
    case 2: { // 180°: (x,y) -> (S-1-x, S-1-y)
        *dw = w; *dh = h;
        *dx = S - sx - w;
        *dy = S - sy - h;
        for (int32_t y = 0; y < h; y++) {
            for (int32_t x = 0; x < w; x++) {
                rot_buf[(h - 1 - y) * w + (w - 1 - x)] = src[y * w + x];
            }
        }
        break;
    }
    case 3: { // 270° CW: (x,y) -> (y, S-1-x)
        *dw = h; *dh = w;
        *dx = sy;
        *dy = S - sx - w;
        for (int32_t y = 0; y < h; y++) {
            for (int32_t x = 0; x < w; x++) {
                // src(x,y) -> dst(y, w-1-x)
                rot_buf[(w - 1 - x) * h + y] = src[y * w + x];
            }
        }
        break;
    }
    default:
        *dx = sx; *dy = sy; *dw = w; *dh = h;
        break;
    }
}
#endif  // !BOARD_AMOLED_18

// LVGL flush callback — writes pixels to display.
// AMOLED-2.16: applies CPU strip rotation based on IMU.
// AMOLED-1.8 : fixed orientation, direct pass-through.
static void my_flush_cb(lv_display_t* disp, const lv_area_t* area, uint8_t* px_map) {
    int32_t w = area->x2 - area->x1 + 1;
    int32_t h = area->y2 - area->y1 + 1;
    uint16_t *src = (uint16_t*)px_map;

#ifdef BOARD_AMOLED_18
    gfx->draw16bitRGBBitmap(area->x1, area->y1, src, w, h);
#else
    uint8_t r = imu_get_rotation();
    if (r == 0) {
        gfx->draw16bitRGBBitmap(area->x1, area->y1, src, w, h);
    } else {
        int32_t dx, dy, dw, dh;
        rotate_strip(src, w, h, area->x1, area->y1, r, &dx, &dy, &dw, &dh);
        gfx->draw16bitRGBBitmap(dx, dy, rot_buf, dw, dh);
    }
#endif
    lv_display_flush_ready(disp);
}

// CO5300 requires even-aligned flush regions. SH8601's driver doesn't
// enforce this in source, but keeping the rounder is harmless.
static void rounder_cb(lv_event_t* e) {
    lv_area_t *area = (lv_area_t*)lv_event_get_param(e);
    area->x1 = area->x1 & ~1;
    area->y1 = area->y1 & ~1;
    area->x2 = area->x2 | 1;
    area->y2 = area->y2 | 1;
}

// LVGL touch callback
static void my_touch_cb(lv_indev_t* indev, lv_indev_data_t* data) {
    if (touch_pressed) {
        data->point.x = touch_x;
        data->point.y = touch_y;
        data->state = LV_INDEV_STATE_PRESSED;
    } else {
        data->state = LV_INDEV_STATE_RELEASED;
    }
}

// Parse a JSON line into UsageData
static bool parse_json(const char* json, UsageData* out) {
    JsonDocument doc;
    DeserializationError err = deserializeJson(doc, json);
    if (err) {
        Serial.printf("JSON parse error: %s\n", err.c_str());
        return false;
    }

    out->session_pct = doc["s"] | 0.0f;
    out->session_reset_mins = doc["sr"] | -1;
    out->weekly_pct = doc["w"] | 0.0f;
    out->weekly_reset_mins = doc["wr"] | -1;
    strlcpy(out->status, doc["st"] | "unknown", sizeof(out->status));
    out->ok = doc["ok"] | false;
    out->valid = true;
    return true;
}

// Serial command buffer
#define CMD_BUF_SIZE 64
static char cmd_buf[CMD_BUF_SIZE];
static int cmd_pos = 0;

static void send_screenshot() {
    const uint32_t w = LCD_WIDTH, h = LCD_HEIGHT;
    const uint32_t row_bytes = w * 2;
    const uint32_t buf_size = row_bytes * h;
    uint8_t* sbuf = (uint8_t*)heap_caps_malloc(buf_size, MALLOC_CAP_SPIRAM);
    if (!sbuf) {
        Serial.println("SCREENSHOT_ERR");
        return;
    }

    lv_draw_buf_t draw_buf;
    lv_draw_buf_init(&draw_buf, w, h, LV_COLOR_FORMAT_RGB565, row_bytes, sbuf, buf_size);

    lv_result_t res = lv_snapshot_take_to_draw_buf(lv_screen_active(), LV_COLOR_FORMAT_RGB565, &draw_buf);
    if (res != LV_RESULT_OK) {
        heap_caps_free(sbuf);
        Serial.println("SCREENSHOT_ERR");
        return;
    }

    Serial.printf("SCREENSHOT_START %lu %lu %lu\n", (unsigned long)w, (unsigned long)h, (unsigned long)buf_size);
    Serial.flush();
    Serial.write(sbuf, buf_size);
    Serial.flush();
    Serial.println();
    Serial.println("SCREENSHOT_END");

    heap_caps_free(sbuf);
}

static void check_serial_cmd() {
    while (Serial.available()) {
        char c = Serial.read();
        if (c == '\n' || c == '\r') {
            cmd_buf[cmd_pos] = '\0';
            if (strcmp(cmd_buf, "screenshot") == 0) {
                send_screenshot();
            }
            cmd_pos = 0;
        } else if (cmd_pos < CMD_BUF_SIZE - 1) {
            cmd_buf[cmd_pos++] = c;
        }
    }
}

void setup() {
    Serial.begin(115200);
    delay(300);
    Serial.println("{\"ready\":true}");

    // Init I2C (shared by touch + PMU + IMU + IO expander)
    Wire.begin(IIC_SDA, IIC_SCL);

#ifdef BOARD_AMOLED_18
    // XCA9554 must come up FIRST — display + touch are held in reset until
    // EXIO0..2 go HIGH (see io_expander_init()).
    io_expander_init();
#endif

    // Init display
    gfx->begin();
    gfx->fillScreen(0x0000);
    idle_init();  // sets brightness to DISPLAY_DEFAULT_BRIGHTNESS and starts idle timer

    // Init PMU
    power_init();

    // Init IMU (accelerometer for auto-rotation; on AMOLED-1.8 we keep init
    // for I2C bus health but ignore rotation — see imu.cpp).
    imu_init();

    // Init touch
#ifdef BOARD_AMOLED_18
    ft3168_init();
    pinMode(TP_INT, INPUT_PULLUP);
    attachInterrupt(TP_INT, touch_isr, FALLING);
    Serial.println("FT3168 attached on INT pin");
#else
    touch.setPins(TP_RST, TP_INT);
    if (!touch.begin(Wire, CST9220_ADDR, IIC_SDA, IIC_SCL)) {
        Serial.println("Touch init failed");
    } else {
        touch.setMaxCoordinates(LCD_WIDTH, LCD_HEIGHT);
        touch.setSwapXY(true);
        touch.setMirrorXY(true, false);
        attachInterrupt(TP_INT, touch_isr, FALLING);
        Serial.println("Touch init OK");
    }
#endif

    // Init LVGL
    lv_init();
    lv_tick_set_cb(my_tick);

    // Allocate PSRAM-backed partial render buffers
    buf1 = (uint16_t*)heap_caps_malloc(LCD_WIDTH * BUF_LINES * 2, MALLOC_CAP_SPIRAM);
    buf2 = (uint16_t*)heap_caps_malloc(LCD_WIDTH * BUF_LINES * 2, MALLOC_CAP_SPIRAM);
#ifndef BOARD_AMOLED_18
    // rot_buf only needed for AMOLED-2.16 (CPU strip rotation).
    // Holds the largest possible strip after rotation (same pixel count as src).
    rot_buf = (uint16_t*)heap_caps_malloc(LCD_WIDTH * BUF_LINES * 2, MALLOC_CAP_SPIRAM);
#endif

    lv_display_t* disp = lv_display_create(LCD_WIDTH, LCD_HEIGHT);
    lv_display_set_color_format(disp, LV_COLOR_FORMAT_RGB565);
    lv_display_set_flush_cb(disp, my_flush_cb);
    lv_display_set_buffers(disp, buf1, buf2, LCD_WIDTH * BUF_LINES * 2,
                           LV_DISPLAY_RENDER_MODE_PARTIAL);

    // CO5300 even-alignment rounder
    lv_display_add_event_cb(disp, rounder_cb, LV_EVENT_INVALIDATE_AREA, NULL);

    lv_indev_t* indev = lv_indev_create();
    lv_indev_set_type(indev, LV_INDEV_TYPE_POINTER);
    lv_indev_set_read_cb(indev, my_touch_cb);

    // Init BLE data channel
    ble_init();

    // Physical buttons
    pinMode(BTN_BACK, INPUT_PULLUP);
#ifndef BOARD_AMOLED_18
    pinMode(BTN_FWD,  INPUT_PULLUP);
#endif

    // Build dashboard
    ui_init();

    // Show initial BLE status on Bluetooth screen
    ui_update_ble_status(ble_get_state(), ble_get_device_name(), ble_get_mac_address());

    // Show initial battery status
    ui_update_battery(power_battery_pct(), power_is_charging());

    ui_show_screen(SCREEN_SPLASH);

    Serial.println("Dashboard ready, waiting for data on BLE...");
}

static ble_state_t last_ble_state = BLE_STATE_INIT;

#ifndef BOARD_AMOLED_18
// Brightness ramp state for rotation transition.
// AMOLED-2.16 only — the 1.8" port is fixed at 0° (no IMU rotation).
// On rotation change we blank the panel, force a full LVGL redraw at the
// new orientation, then ramp brightness back up over ~125ms so the
// transition reads as deliberate instead of as a glitch.
static void handle_rotation_change(void) {
    static uint8_t last_rotation = 0;
    static uint8_t  ramp_step = 0;  // 0=idle, 1-4=ramping
    static uint32_t ramp_last = 0;

    // While asleep the rotation visual transition (blank + ramp) would fight
    // the idle fade. Defer: a rotation that happens during sleep will be
    // detected after wake and ramped in then.
    if (idle_is_asleep()) return;

    uint8_t rot = imu_get_rotation();
    if (rot != last_rotation) {
        gfx->setBrightness(0);
        last_rotation = rot;
        lv_obj_invalidate(lv_screen_active());
        ramp_step = 1;
        return;
    }

    if (ramp_step == 0) return;
    uint32_t now = millis();
    if (now - ramp_last < 25) return;
    ramp_last = now;

    static const uint8_t levels[] = {60, 120, 170, DISPLAY_DEFAULT_BRIGHTNESS};
    gfx->setBrightness(levels[ramp_step - 1]);
    if (ramp_step >= 4) ramp_step = 0;
    else                ramp_step++;
}
#endif  // !BOARD_AMOLED_18

void loop() {
    touch_read();
    idle_tick();
    lv_timer_handler();
    ui_tick_anim();
    ble_tick();
    power_tick();
    imu_tick();
    splash_tick();

    // Physical button input (global, screen-independent):
    //   LEFT (GPIO 0 / BOOT)  → Space (voice-mode push-to-talk)
    //   RIGHT (GPIO 18)       → Shift+Tab (Claude Code mode toggle). AMOLED-2.16 only.
    //   PWR                   → cycle screens; on splash, cycle animations.
    //                           AMOLED-2.16: AXP2101 PKEY IRQ; AMOLED-1.8: XCA9554 EXIO4.
    // First press from sleep is consumed for wake only (idle_consume_wake_press
    // returns true) — the normal action only fires from the second press.
    // Activity bookkeeping happens inside idle_consume_wake_press, so no
    // separate idle_note_activity() call is needed here.
    {
        static bool back_was = false;
        static bool back_wake_swallowed = false;
#ifndef BOARD_AMOLED_18
        static bool fwd_was = false;
        static bool fwd_wake_swallowed = false;
#endif
        bool back_now = (digitalRead(BTN_BACK) == LOW);
        if (back_now != back_was) {
            if (back_now) {
                if (idle_consume_wake_press()) {
                    back_wake_swallowed = true;
                } else {
                    ble_keyboard_press(0x2C, 0);  // HID Space, no mods
                }
            } else {
                if (back_wake_swallowed) back_wake_swallowed = false;
                else                     ble_keyboard_release();
            }
            back_was = back_now;
        }

#ifndef BOARD_AMOLED_18
        bool fwd_now = (digitalRead(BTN_FWD) == LOW);
        if (fwd_now != fwd_was) {
            if (fwd_now) {
                if (idle_consume_wake_press()) {
                    fwd_wake_swallowed = true;
                } else {
                    ble_keyboard_press(0x2B, 0x02);  // HID Tab + LEFT_SHIFT
                }
            } else {
                if (fwd_wake_swallowed) fwd_wake_swallowed = false;
                else                    ble_keyboard_release();
            }
            fwd_was = fwd_now;
        }
#endif

        if (power_pwr_pressed()) {
            if (!idle_consume_wake_press()) {
                if (ui_get_current_screen() == SCREEN_SPLASH) splash_next();
                else                                          ui_cycle_screen();
            }
        }
    }

#ifndef BOARD_AMOLED_18
    handle_rotation_change();
#endif

    // Update BLE status on screen when state changes
    ble_state_t bs = ble_get_state();
    if (bs != last_ble_state) {
        last_ble_state = bs;
        ui_update_ble_status(bs, ble_get_device_name(), ble_get_mac_address());
    }

    // Update battery indicator
    static int last_pct = -2;
    static bool last_charging = false;
    int pct = power_battery_pct();
    bool charging = power_is_charging();
    if (pct != last_pct || charging != last_charging) {
        last_pct = pct;
        last_charging = charging;
        ui_update_battery(pct, charging);
    }

    // Check for serial commands (screenshot, etc.)
    check_serial_cmd();

    // Process incoming BLE data
    if (ble_has_data()) {
        if (parse_json(ble_get_data(), &usage)) {
            int g_before = usage_rate_group();
            usage_rate_sample(usage.session_pct);
            int g_after = usage_rate_group();
            if (g_after != g_before) {
                Serial.printf("usage rate: group %d -> %d (s=%.2f%%)\n",
                    g_before, g_after, usage.session_pct);
                if (splash_is_active()) splash_pick_for_current_rate();
            }
            ui_update(&usage);
            ble_send_ack();
        } else {
            ble_send_nack();
        }
    }

    delay(5);
}
