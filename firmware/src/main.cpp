#include <Arduino.h>
#include <lvgl.h>
#include <ArduinoJson.h>
#include "display_cfg.h"
#include "data.h"
#include "ui.h"
#include "hid.h"
#include "touch.h"
#include "ble.h"

LGFX lcd;  // global - used by touch.cpp
static UsageData usage = {};

// LVGL draw buffers
#define BUF_LINES 40
static uint16_t buf1[480 * BUF_LINES];
static uint16_t buf2[480 * BUF_LINES];

// LVGL tick callback
static uint32_t my_tick(void) {
    return millis();
}

// LVGL flush callback
static void my_flush_cb(lv_display_t* disp, const lv_area_t* area, uint8_t* px_map) {
    uint32_t w = (area->x2 - area->x1 + 1);
    uint32_t h = (area->y2 - area->y1 + 1);
    lcd.startWrite();
    lcd.setAddrWindow(area->x1, area->y1, w, h);
    lcd.writePixels((uint16_t*)px_map, w * h, true);
    lcd.endWrite();
    lv_display_flush_ready(disp);
}

// LVGL touch callback
static void my_touch_cb(lv_indev_t* indev, lv_indev_data_t* data) {
    uint16_t x, y;
    if (lcd.getTouch(&x, &y)) {
        data->point.x = x;
        data->point.y = y;
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
    // Allocate buffer in PSRAM (internal RAM is too small for 307KB)
    const uint32_t w = 480, h = 320;
    const uint32_t row_bytes = w * 2;
    const uint32_t buf_size = row_bytes * h;
    uint8_t* buf = (uint8_t*)heap_caps_malloc(buf_size, MALLOC_CAP_SPIRAM);
    if (!buf) {
        Serial.println("SCREENSHOT_ERR");
        return;
    }

    // Use LVGL snapshot with caller-provided buffer
    lv_draw_buf_t draw_buf;
    lv_draw_buf_init(&draw_buf, w, h, LV_COLOR_FORMAT_RGB565, row_bytes, buf, buf_size);

    lv_result_t res = lv_snapshot_take_to_draw_buf(lv_screen_active(), LV_COLOR_FORMAT_RGB565, &draw_buf);
    if (res != LV_RESULT_OK) {
        heap_caps_free(buf);
        Serial.println("SCREENSHOT_ERR");
        return;
    }

    Serial.printf("SCREENSHOT_START %lu %lu %lu\n", (unsigned long)w, (unsigned long)h, (unsigned long)buf_size);
    Serial.flush();
    Serial.write(buf, buf_size);
    Serial.flush();
    Serial.println();
    Serial.println("SCREENSHOT_END");

    heap_caps_free(buf);
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

    // Init display
    lcd.init();
    lcd.setRotation(1);
    lcd.setBrightness(200);
    lcd.fillScreen(TFT_BLACK);

    // Init LVGL
    lv_init();
    lv_tick_set_cb(my_tick);

    lv_display_t* disp = lv_display_create(480, 320);
    lv_display_set_color_format(disp, LV_COLOR_FORMAT_RGB565);
    lv_display_set_flush_cb(disp, my_flush_cb);
    lv_display_set_buffers(disp, buf1, buf2, sizeof(buf1), LV_DISPLAY_RENDER_MODE_PARTIAL);

    lv_indev_t* indev = lv_indev_create();
    lv_indev_set_type(indev, LV_INDEV_TYPE_POINTER);
    lv_indev_set_read_cb(indev, my_touch_cb);

    // Init USB HID keyboard
    hid_init();

    // Init BLE data channel
    ble_init();

    // Init touch gesture detection
    touch_init();

    // Build dashboard
    ui_init();

    // Show initial BLE status on Bluetooth screen
    ui_update_ble_status(ble_get_state(), ble_get_device_name(), ble_get_mac_address());

    Serial.println("Dashboard ready, waiting for data on BLE...");
}

static ble_state_t last_ble_state = BLE_STATE_INIT;

void loop() {
    lv_timer_handler();
    ui_tick_anim();
    touch_tick();
    ble_tick();

    // Update BLE status on screen when state changes
    ble_state_t bs = ble_get_state();
    if (bs != last_ble_state) {
        last_ble_state = bs;
        ui_update_ble_status(bs, ble_get_device_name(), ble_get_mac_address());
    }

    // Check for serial commands (screenshot, etc.)
    check_serial_cmd();

    // Process incoming BLE data
    if (ble_has_data()) {
        if (parse_json(ble_get_data(), &usage)) {
            ui_update(&usage);
            ble_send_ack();
        } else {
            ble_send_nack();
        }
    }

    delay(5);
}
