#include <Arduino.h>
#include <Wire.h>
#include <lvgl.h>
#include <ArduinoJson.h>
#include <esp_heap_caps.h>

#include "data.h"
#include "ui.h"
#include "ble.h"
#include "splash.h"
#include "charge_anim.h"
#include "usage_rate.h"
#include "idle.h"
#include "idle_cfg.h"
#include "brightness.h"

#include "hal/board_caps.h"
#include "hal/display_hal.h"
#include "hal/touch_hal.h"
#include "hal/input_hal.h"
#include "hal/power_hal.h"
#include "hal/imu_hal.h"
#include "hal/sound_hal.h"

static UsageData usage = {};

// ---- LVGL draw buffers (partial render mode) ----
// PSRAM-equipped boards (S3) can comfortably hold larger strips. PSRAM-free
// boards (e.g. ESP32-C6) allocate from internal SRAM, so we shrink the strip
// — 480×20 RGB565 = 19 KB × 2 buffers = 38 KB, fits beside everything else.
#ifdef BOARD_HAS_PSRAM
#define BUF_LINES 40
#define LV_BUF_CAPS (MALLOC_CAP_SPIRAM)
#else
#define BUF_LINES 20
#define LV_BUF_CAPS (MALLOC_CAP_INTERNAL | MALLOC_CAP_8BIT)
#endif
static uint16_t* buf1 = nullptr;
static uint16_t* buf2 = nullptr;

// Abstecher vom Splash auf die Auslastungszahlen: alle EVERY fuer SHOW lang.
#define USAGE_PEEK_EVERY_MS  (5UL * 60UL * 1000UL)
#define USAGE_PEEK_SHOW_MS   (60UL * 1000UL)

static uint32_t my_tick(void) { return millis(); }

#ifndef BOARD_HAS_PSRAM
// Screenshot on PSRAM-free boards: a whole frame doesn't fit in RAM, so the
// `screenshot` command forces a full redraw and the flush callback streams
// each strip to serial as LVGL renders it. A full-screen invalidation renders
// as full-width strips, top to bottom, so the bytes arrive already in raster
// order — same wire format as the snapshot path on PSRAM boards.
static bool    shot_active = false;
static bool    shot_bad    = false;   // a strip arrived that wasn't the next rows
static int32_t shot_next_y = 0;
#endif

static void my_flush_cb(lv_display_t* disp, const lv_area_t* area, uint8_t* px_map) {
    int32_t w = area->x2 - area->x1 + 1;
    int32_t h = area->y2 - area->y1 + 1;
#ifndef BOARD_HAS_PSRAM
    if (shot_active && !shot_bad) {
        // Before the panel write: the board may rotate/byte-swap in place.
        if (area->x1 != 0 || w != board_caps().width || area->y1 != shot_next_y) {
            shot_bad = true;
        } else {
            // The USB serial drops what doesn't fit its buffer in time, and one
            // lost byte shears the rest of the image. Push in small pieces
            // and keep at it until each is accepted.
            const uint8_t* p = px_map;
            size_t left = (size_t)w * h * 2;
            uint32_t last_progress = millis();
            while (left && !shot_bad) {
                size_t k = Serial.write(p, left > 512 ? 512 : left);
                if (k) { p += k; left -= k; last_progress = millis(); }
                else if (millis() - last_progress > 2000) shot_bad = true;  // nobody reading
                else delay(1);
            }
            shot_next_y += h;
        }
    }
#endif
    display_hal_draw_bitmap(area->x1, area->y1, w, h, (uint16_t*)px_map);
    // Letzter Streifen eines Durchlaufs: der Splash wartet darauf, bevor er
    // nach einem Screenwechsel wieder direkt auf den Panel malt.
    if (lv_display_flush_is_last(disp)) splash_note_refresh_done();
    lv_display_flush_ready(disp);
}

static void rounder_cb(lv_event_t* e) {
    lv_area_t* area = (lv_area_t*)lv_event_get_param(e);
    display_hal_round_area(&area->x1, &area->y1, &area->x2, &area->y2);
}

// Touch policy is driven by IDLE_WAKE_ON_TOUCH:
//   true  → a press edge while asleep wakes the device and the first touch is
//           swallowed (mirrors the button wake-consumption); a press while
//           awake counts as activity.
//   false → touch never counts as activity and is fully swallowed while the
//           panel is dark, so pets/sleeves can't wake it overnight and LVGL
//           can't quietly toggle splash<->usage on a black panel.
static void my_touch_cb(lv_indev_t* indev, lv_indev_data_t* data) {
    uint16_t x, y;
    bool pressed;
    touch_hal_read(&x, &y, &pressed);
    const bool raw_pressed = pressed;

    if (IDLE_WAKE_ON_TOUCH) {
        static bool touch_was = false;
        static bool touch_wake_swallowed = false;
        if (raw_pressed && !touch_was) {
            // Press edge — consume as wake if asleep.
            if (idle_consume_wake_press()) {
                touch_wake_swallowed = true;
                pressed = false;
            }
        } else if (!raw_pressed && touch_was) {
            // Release edge.
            if (touch_wake_swallowed) {
                touch_wake_swallowed = false;
                pressed = false;
            }
        } else if (raw_pressed && touch_wake_swallowed) {
            // Held finger through wake — keep hiding until release.
            pressed = false;
        }
        touch_was = raw_pressed;
    } else if (idle_is_asleep()) {
        pressed = false;
    }

    if (pressed) {
        data->point.x = x;
        data->point.y = y;
        data->state = LV_INDEV_STATE_PRESSED;
    } else {
        data->state = LV_INDEV_STATE_RELEASED;
    }
}

// Parse a JSON line into UsageData.
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
    // Which limit windows the account has. Anthropic dropped the 5-hour window
    // for some plans (weekly only), so the host names what it actually got:
    // "sw" both, "w" weekly only, "s" 5-hour only. Absent (older daemons) or
    // unrecognised → both, which is the layout they were written against.
    if (doc["lim"].is<const char*>()) {
        const char* lim = doc["lim"];
        out->has_session = strchr(lim, 's') != nullptr;
        out->has_weekly  = strchr(lim, 'w') != nullptr;
    } else {
        // No "lim": a host that knows a window is missing but predates the
        // field sends its reset time as -1 (daemons with both windows always
        // send >= 0), so a negative reset means "no such window" too.
        out->has_session = out->session_reset_mins >= 0;
        out->has_weekly  = out->weekly_reset_mins >= 0;
    }
    if (!out->has_session && !out->has_weekly) {
        out->has_session = out->has_weekly = true;
    }
    strlcpy(out->status, doc["st"] | "unknown", sizeof(out->status));
    out->chime = doc["c"] | false;   // absent (old daemon / chime off) → stay silent
    const char* acct = doc["acct"] | "pro";
    out->enterprise = (strcmp(acct, "ent") == 0);
    out->time_pct = doc["tp"] | 0;
    out->period_days = doc["pd"] | 30;
    strlcpy(out->reset_date, doc["rd"] | "", sizeof(out->reset_date));
    strlcpy(out->anim, doc["a"] | "", sizeof(out->anim));
    out->clock_epoch = doc["t"] | 0L;
    out->clock_fmt = doc["tf"] | 24;
    out->ok = doc["ok"] | false;
    out->valid = true;
    return true;
}

// Take one usage payload (from BLE, or a canned one from the `demo` serial
// command) through the whole pipeline: parse, usage-rate tracking, chime,
// host-driven animation, UI. Returns false if the JSON didn't parse.
static bool handle_payload(const char* json) {
    if (!parse_json(json, &usage)) return false;

    // Track the window that actually limits this account: the 5-hour
    // one when it exists (and for Enterprise, whose spending % rides
    // in session_pct), otherwise the weekly one.
    const bool weekly_only = !usage.enterprise && !usage.has_session;
    const float rate_pct   = weekly_only ? usage.weekly_pct : usage.session_pct;
    int g_before = usage_rate_group();
    bool limit_reset = usage_rate_sample(
        rate_pct, weekly_only ? USAGE_WINDOW_WEEKLY_MIN : USAGE_WINDOW_SESSION_MIN);
    int g_after = usage_rate_group();
    // Limit refilled → chime so the user knows they can use Claude
    // again (no-op on boards without a buzzer). Gated on the daemon's
    // opt-in `chime` config; the `buzz` serial cmd ignores it.
    if (limit_reset && usage.chime) {
        Serial.println("limit reset detected — chime");
        sound_hal_play_reset();
    }
    // Host-driven animation. Sent only when the host is configured to
    // mirror its desktop buddy; absent → "" → device keeps deciding.
    splash_set_anim(usage.anim);
    if (g_after != g_before) {
        Serial.printf("usage rate: group %d -> %d (%s=%.2f%%)\n",
            g_before, g_after, weekly_only ? "w" : "s", rate_pct);
        if (splash_is_active()) splash_pick_for_current_rate();
    }
    ui_update(&usage);
    return true;
}

// `demo <name>` — feed a canned payload so every usage layout can be looked at
// (and screenshotted) without a host. The next real BLE payload replaces it.
static void run_demo(const char* name) {
    static const struct { const char* name; const char* json; } demos[] = {
        // 5-hour + weekly (classic Pro/Max)
        {"sw",   "{\"s\":42,\"sr\":187,\"w\":18,\"wr\":6200,\"st\":\"allowed\",\"lim\":\"sw\",\"ok\":true}"},
        // weekly only (plans without the 5-hour window)
        {"w",    "{\"s\":0,\"sr\":-1,\"w\":36,\"wr\":5935,\"st\":\"allowed\",\"lim\":\"w\",\"ok\":true}"},
        {"wover","{\"s\":0,\"sr\":-1,\"w\":83,\"wr\":5935,\"st\":\"allowed_warning\",\"lim\":\"w\",\"ok\":true}"},
        {"wlim", "{\"s\":0,\"sr\":-1,\"w\":100,\"wr\":1439,\"st\":\"rejected\",\"lim\":\"w\",\"ok\":true}"},
        // 5-hour only
        {"s",    "{\"s\":64,\"sr\":95,\"w\":0,\"wr\":-1,\"st\":\"allowed\",\"lim\":\"s\",\"ok\":true}"},
        // Enterprise spending limit
        {"ent",  "{\"s\":31,\"sr\":20000,\"w\":0,\"wr\":0,\"st\":\"allowed\",\"acct\":\"ent\",\"tp\":54,\"pd\":30,\"rd\":\"Dec 1\",\"ok\":true}"},
        // What a daemon from before "lim" sends
        {"old",  "{\"s\":42,\"sr\":187,\"w\":18,\"wr\":6200,\"st\":\"allowed\",\"ok\":true}"},
        // Weekly-only from a host that predates "lim": only sr = -1 gives it away
        {"oldw", "{\"s\":0,\"sr\":-1,\"w\":11,\"wr\":4300,\"st\":\"allowed\",\"acct\":\"pro\",\"ok\":true,\"sx\":\"Allowed\",\"sxd\":\"Weekly limit applies\"}"},
    };
    for (const auto& d : demos) {
        if (strcmp(name, d.name) == 0) {
            handle_payload(d.json);
            ui_show_screen(SCREEN_USAGE);
            ui_demo_hold();
            Serial.printf("DEMO %s\n", d.name);
            return;
        }
    }
    Serial.println("DEMO_UNKNOWN (sw, w, wover, wlim, s, ent, old, oldw)");
}

// ---- Serial command buffer ----
#define CMD_BUF_SIZE 64
static char cmd_buf[CMD_BUF_SIZE];
static int cmd_pos = 0;

static void send_screenshot() {
#ifndef BOARD_HAS_PSRAM
    // A full RGB565 framebuffer doesn't fit in internal SRAM on PSRAM-free
    // boards (e.g. 480×480×2 = 460 KB), so stream it strip by strip out of
    // the flush callback instead (see shot_active above). Captures what LVGL
    // draws; the splash paints the panel directly and is not included.
    const uint32_t w = board_caps().width;
    const uint32_t h = board_caps().height;
    Serial.printf("SCREENSHOT_START %lu %lu %lu\n",
        (unsigned long)w, (unsigned long)h, (unsigned long)(w * h * 2));
    Serial.flush();
    shot_next_y = 0;
    shot_bad    = false;
    shot_active = true;
    lv_obj_invalidate(lv_screen_active());
    lv_refr_now(NULL);
    shot_active = false;
    Serial.flush();
    Serial.println();
    // A short or out-of-order capture leaves the host waiting for bytes; it
    // times out, and this line tells whoever reads the log why.
    Serial.println((shot_bad || shot_next_y != (int32_t)h) ? "SCREENSHOT_ERR"
                                                           : "SCREENSHOT_END");
    return;
#else
    const uint32_t w = board_caps().width;
    const uint32_t h = board_caps().height;
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

    Serial.printf("SCREENSHOT_START %lu %lu %lu\n",
        (unsigned long)w, (unsigned long)h, (unsigned long)buf_size);
    Serial.flush();
    Serial.write(sbuf, buf_size);
    Serial.flush();
    Serial.println();
    Serial.println("SCREENSHOT_END");
    heap_caps_free(sbuf);
#endif
}

static void check_serial_cmd() {
    while (Serial.available()) {
        char c = Serial.read();
        if (c == '\n' || c == '\r') {
            cmd_buf[cmd_pos] = '\0';
            if (strcmp(cmd_buf, "screenshot") == 0) send_screenshot();
            else if (strcmp(cmd_buf, "buzz") == 0)  sound_hal_play_reset();
            // Play the charge overlay without touching the cable — the real
            // trigger needs a USB transition, which is awkward to produce on a
            // device that is being flashed over that same cable.
            else if (strcmp(cmd_buf, "charge") == 0)   charge_anim_play(true);
            else if (strcmp(cmd_buf, "uncharge") == 0) charge_anim_play(false);
            else if (strncmp(cmd_buf, "demo ", 5) == 0) run_demo(cmd_buf + 5);
            cmd_pos = 0;
        } else if (cmd_pos < CMD_BUF_SIZE - 1) {
            cmd_buf[cmd_pos++] = c;
        }
    }
}

// Each board provides this. Must bring up the shared I2C bus (Wire.begin
// with the board's SDA/SCL pins) and any board-private hardware that has
// to settle before display/touch (e.g. an IO expander gating the LCD
// reset line). Called exactly once at the start of setup().
extern "C" void board_init(void);

void setup() {
    Serial.begin(115200);
    delay(300);
    Serial.println("{\"ready\":true}");

    board_init();

    display_hal_init();
    display_hal_begin();
    idle_init();        // takes over panel brightness and starts the idle timer
    brightness_init();  // load the user's saved brightness level and apply via idle

    power_hal_init();
    imu_hal_init();
    sound_hal_init();
    touch_hal_init();

    // ---- LVGL ----
    const int W = board_caps().width;
    const int H = board_caps().height;

    lv_init();
    lv_tick_set_cb(my_tick);

    buf1 = (uint16_t*)heap_caps_malloc(W * BUF_LINES * 2, LV_BUF_CAPS);
    buf2 = (uint16_t*)heap_caps_malloc(W * BUF_LINES * 2, LV_BUF_CAPS);

    lv_display_t* disp = lv_display_create(W, H);
    lv_display_set_color_format(disp, LV_COLOR_FORMAT_RGB565);
    lv_display_set_flush_cb(disp, my_flush_cb);
    lv_display_set_buffers(disp, buf1, buf2, W * BUF_LINES * 2,
                           LV_DISPLAY_RENDER_MODE_PARTIAL);
    lv_display_add_event_cb(disp, rounder_cb, LV_EVENT_INVALIDATE_AREA, NULL);

    lv_indev_t* indev = lv_indev_create();
    lv_indev_set_type(indev, LV_INDEV_TYPE_POINTER);
    lv_indev_set_read_cb(indev, my_touch_cb);

    ble_init();
    input_hal_init();

    ui_init();
    ui_update_ble_status(ble_get_state(), ble_get_device_name(), ble_get_mac_address());
    ui_update_battery(power_hal_battery_pct(), power_hal_is_charging());
    ui_show_screen(SCREEN_SPLASH);

    Serial.printf("Dashboard ready (%s, %dx%d), waiting for data on BLE...\n",
        board_caps().name, W, H);
}

static ble_state_t last_ble_state = BLE_STATE_INIT;

// Hold-to-pair gesture: hold the PWR button ~3s, then RELEASE → clear all BLE
// bonds and re-advertise. Clearing on *release* (not while held) is deliberate:
// holding to power the device OFF (AXP hardware shutdown at 8s) must not wipe
// the bond — a power-off hold never releases before shutdown. To stop a
// "chicken-out" release just before 8s from pairing, the gesture disarms at 6s.
//
//   ~1.5s long-press edge → PENDING
//   3.0s (+1500)          → ARMED   (release from here clears bonds)
//   6.0s (+4500)          → DISARMED (no clear; AXP powers off at 8s)
#define PAIR_ARM_AFTER_LONG_MS    1500   // 3.0s total
#define PAIR_DISARM_AFTER_LONG_MS 4500   // 6.0s total
enum pair_state_t { PAIR_IDLE, PAIR_PENDING, PAIR_ARMED };
static pair_state_t pair_state        = PAIR_IDLE;
static uint32_t     pair_long_seen_ms = 0;

static void pair_tick(void) {
    if (pair_state == PAIR_IDLE && power_hal_pwr_long_pressed()) {
        pair_state = PAIR_PENDING;
        pair_long_seen_ms = millis();
        (void)power_hal_pwr_released();  // drain any stale release edge
        Serial.println("PWR long-press: hold to ~3s then release to pair");
        return;
    }
    if (pair_state == PAIR_IDLE) return;

    if (power_hal_pwr_released()) {
        if (pair_state == PAIR_ARMED) {
            Serial.println("Pair: released in window — clearing bonds, advertising");
            ble_clear_bonds();
        } else {
            Serial.println("Pair: released too early — cancelled");
        }
        pair_state = PAIR_IDLE;
        return;
    }

    uint32_t held = millis() - pair_long_seen_ms;
    if (pair_state == PAIR_PENDING && held >= PAIR_ARM_AFTER_LONG_MS) {
        pair_state = PAIR_ARMED;
        Serial.println("Pair: armed — release to pair");
    } else if (pair_state == PAIR_ARMED && held >= PAIR_DISARM_AFTER_LONG_MS) {
        pair_state = PAIR_IDLE;  // power-off territory; don't pair
        Serial.println("Pair: disarmed (holding toward power-off)");
    }
}

void loop() {
    idle_tick();
    lv_timer_handler();
    ui_tick_anim();
    ble_tick();
    power_hal_tick();
    imu_hal_tick();
    sound_hal_tick();
    splash_tick();
    // Rotation transition (blank + ramp) would fight the idle fade — skip
    // ticks while the panel is dark. A rotation that happens during sleep
    // is detected by the next tick after wake and ramped in then.
    if (!idle_is_asleep()) display_hal_tick();

    // ---- Physical buttons ----
    //   PRIMARY   → HID Space  (Claude Code voice-mode PTT)
    //   SECONDARY → HID Shift+Tab  (mode toggle; only if the board has one)
    //   PWR       → on splash: cycle animations; on usage: cycle brightness;
    //               hold ~3s + release: pairing mode
    // First press from sleep is consumed as a wake-only event by
    // idle_consume_wake_press(); the normal action fires from the second
    // press. Activity bookkeeping happens inside idle_consume_wake_press
    // so no separate idle_note_activity() call is needed here.
    {
        static bool primary_was = false;
        static bool primary_wake_swallowed = false;
        bool primary_now = input_hal_is_held(INPUT_BTN_PRIMARY);
        if (primary_now != primary_was) {
            if (primary_now) {
                if (idle_consume_wake_press()) primary_wake_swallowed = true;
                else                            ble_keyboard_press(0x2C, 0);  // HID Space, no mods
            } else {
                if (primary_wake_swallowed) primary_wake_swallowed = false;
                else                        ble_keyboard_release();
            }
            primary_was = primary_now;
        }

        if (board_caps().button_count >= 2) {
            static bool secondary_was = false;
            static bool secondary_wake_swallowed = false;
            bool secondary_now = input_hal_is_held(INPUT_BTN_SECONDARY);
            if (secondary_now != secondary_was) {
                if (secondary_now) {
                    if (idle_consume_wake_press()) secondary_wake_swallowed = true;
                    else                            ble_keyboard_press(0x2B, 0x02);  // HID Tab + LEFT_SHIFT
                } else {
                    if (secondary_wake_swallowed) secondary_wake_swallowed = false;
                    else                          ble_keyboard_release();
                }
                secondary_was = secondary_now;
            }
        }

        if (power_hal_pwr_pressed()) {
            if (!idle_consume_wake_press()) {
                // On splash: cycle animations. On the usage view: cycle
                // screen brightness (single non-splash view, no more screens).
                if (ui_get_current_screen() == SCREEN_SPLASH) splash_next();
                else                                          brightness_cycle();
            }
        }

        pair_tick();
    }

    // ---- Zwischendurch die Zahlen zeigen ----
    // Wer den Clawd als Animation laufen laesst, will trotzdem ab und zu
    // sehen wie es um die Auslastung steht. Alle USAGE_PEEK_EVERY_MS also
    // fuer USAGE_PEEK_SHOW_MS auf den Usage-Screen und wieder zurueck.
    //
    // Bewusst nur aus dem Splash heraus und nur solange der Nutzer nicht
    // selbst umschaltet: ein Tastendruck waehrend des Abstechers beendet ihn,
    // und wer laenger freiwillig auf den Zahlen steht, faengt danach mit
    // vollem Abstand wieder an - sonst wechselt das Ding vor der Nase hin
    // und her.
    {
        static uint32_t peek_ref_ms = 0;
        static bool     peeking     = false;
        const uint32_t now_ms = millis();
        const screen_t cur    = ui_get_current_screen();

        if (idle_is_asleep()) {
            peek_ref_ms = now_ms;          // im Schlaf laeuft die Uhr nicht
            peeking = false;
        } else if (peeking) {
            if (cur != SCREEN_USAGE) {     // Nutzer hat selbst umgeschaltet
                peeking = false;
                peek_ref_ms = now_ms;
            } else if (now_ms - peek_ref_ms >= USAGE_PEEK_SHOW_MS) {
                peeking = false;
                peek_ref_ms = now_ms;
                ui_show_screen(SCREEN_SPLASH);
                Serial.println("peek: zurueck zu den Animationen");
            }
        } else if (cur != SCREEN_SPLASH) {
            peek_ref_ms = now_ms;          // steht ohnehin auf den Zahlen
        } else if (now_ms - peek_ref_ms >= USAGE_PEEK_EVERY_MS) {
            peeking = true;
            peek_ref_ms = now_ms;
            ui_show_screen(SCREEN_USAGE);
            Serial.println("peek: zeige kurz die Auslastung");
        }
    }

    ble_state_t bs = ble_get_state();
    if (bs != last_ble_state) {
        last_ble_state = bs;
        ui_update_ble_status(bs, ble_get_device_name(), ble_get_mac_address());
    }

    static int  last_pct      = -2;
    static bool last_charging = false;
    int  pct      = power_hal_battery_pct();
    bool charging = power_hal_is_charging();
    if (pct != last_pct || charging != last_charging) {
        if (pct != last_pct) ble_set_battery_level(pct);
        last_pct = pct;
        last_charging = charging;
        ui_update_battery(pct, charging);
    }

    // Cable in / out gets a short animation. Driven by VBUS rather than the
    // charging flag, which also drops when the battery reaches full with the
    // cable still in — that would play the unplug sequence for nothing.
    // The first reading only records the state: booting on USB is not an event.
    static bool vbus_known = false;
    static bool last_vbus  = false;
    bool vbus = power_hal_is_vbus_in();
    if (!vbus_known) {
        vbus_known = true;
        last_vbus  = vbus;
    } else if (vbus != last_vbus) {
        last_vbus = vbus;
        Serial.printf("USB %s\n", vbus ? "in" : "out");
        charge_anim_play(vbus);
    }

    check_serial_cmd();

    if (ble_has_data()) {
        if (handle_payload(ble_get_data())) ble_send_ack();
        else                                ble_send_nack();
    }

    delay(5);
}
