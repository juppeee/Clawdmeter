#include "../../hal/input_hal.h"
#include "board.h"
#include <Arduino.h>
#include <Wire.h>
#include <esp_timer.h>

// BOOT is the primary button (HID Space). The ring is reported through
// input_hal_encoder_steps(); every call that returns a turn also fires a
// short click on the haptic driver, so each detent is felt.
//
// The ring is a bidirectional detent switch, not a quadrature encoder: A
// pulses LOW once per detent one way, B once per detent the other way.
// Decoding follows Waveshare's hardware-tested bidi_switch_knob.c — sample
// both lines every 3 ms on an esp_timer and count a detent when a line
// returns HIGH after staying LOW for at least one further sample. Polling
// from loop() would miss pulses whenever an LVGL render runs long.

#define ENC_POLL_US  3000

struct EncChannel {
    uint8_t prev;
    uint8_t low_ticks;
};

static EncChannel       ch_a = {1, 0};
static EncChannel       ch_b = {1, 0};
static volatile int32_t enc_steps = 0;
static portMUX_TYPE     enc_mux = portMUX_INITIALIZER_UNLOCKED;
static esp_timer_handle_t enc_timer = nullptr;
static bool             haptic_ok = false;

static void poll_channel(EncChannel& c, uint8_t level, int dir) {
    if (level == 0) {
        c.low_ticks = (c.prev == 0 && c.low_ticks < 255) ? c.low_ticks + 1 : 0;
    } else if (c.prev == 0 && c.low_ticks >= 1) {
        portENTER_CRITICAL(&enc_mux);
        enc_steps += dir;
        portEXIT_CRITICAL(&enc_mux);
    }
    c.prev = level;
}

static void enc_poll_cb(void*) {
    poll_channel(ch_a, digitalRead(ENC_A_GPIO), +1);
    poll_channel(ch_b, digitalRead(ENC_B_GPIO), -1);
}

// ---- DRV2605: ROM-library click, internal trigger ----
// Same setup as Waveshare's 03_DRV2605_Test (SensorLib defaults: ERM open
// loop, library 1, internal trigger), written out as plain register writes.

static void drv_write(uint8_t reg, uint8_t val) {
    Wire.beginTransmission(DRV2605_ADDR);
    Wire.write(reg);
    Wire.write(val);
    Wire.endTransmission();
}

static bool drv_read(uint8_t reg, uint8_t* val) {
    Wire.beginTransmission(DRV2605_ADDR);
    Wire.write(reg);
    if (Wire.endTransmission(false) != 0) return false;
    if (Wire.requestFrom((uint8_t)DRV2605_ADDR, (uint8_t)1) != 1) return false;
    *val = Wire.read();
    return true;
}

static void haptic_init(void) {
    uint8_t status;
    if (!drv_read(0x00, &status)) {
        Serial.println("DRV2605 not found — ring clicks disabled");
        return;
    }
    drv_write(0x01, 0x00);   // MODE: out of standby, internal trigger
    drv_write(0x02, 0x00);   // RTP input off
    drv_write(0x0D, 0x00);   // no overdrive
    drv_write(0x0E, 0x00);   // sustain +
    drv_write(0x0F, 0x00);   // sustain -
    drv_write(0x10, 0x00);   // brake
    drv_write(0x13, 0x64);   // audio-to-vibe max
    uint8_t v;
    if (drv_read(0x1A, &v)) drv_write(0x1A, v & 0x7F);   // FEEDBACK: ERM
    if (drv_read(0x1D, &v)) drv_write(0x1D, v | 0x20);   // CONTROL3: ERM open loop
    drv_write(0x03, 0x01);   // LIBRARY 1
    haptic_ok = true;
    Serial.printf("DRV2605 ready (status 0x%02X)\n", status);
}

static void haptic_click(void) {
    if (!haptic_ok) return;
    drv_write(0x04, HAPTIC_DETENT_EFFECT);   // waveform slot 1
    drv_write(0x05, 0x00);                   // end of sequence
    drv_write(0x0C, 0x01);                   // GO
}

// Exposed to this board's sound.cpp, which answers the pairing cues with
// haptics — there is no speaker here. The DRV2605 sequencer plays slots 1..n
// back-to-back on a single GO, so a two-pulse pattern costs the main loop
// nothing but the I2C writes. Declared at the call site rather than in a
// header: one function, one caller, same board folder.
void knob_haptic_effects(uint8_t first, uint8_t second) {
    if (!haptic_ok) return;
    drv_write(0x04, first);
    drv_write(0x05, second);     // 0x00 here ends the sequence after slot 1
    drv_write(0x06, 0x00);
    drv_write(0x0C, 0x01);       // GO
}

void input_hal_init(void) {
    pinMode(BTN_BACK_GPIO, INPUT_PULLUP);
    pinMode(ENC_A_GPIO, INPUT_PULLUP);
    pinMode(ENC_B_GPIO, INPUT_PULLUP);
    ch_a.prev = digitalRead(ENC_A_GPIO);
    ch_b.prev = digitalRead(ENC_B_GPIO);

    esp_timer_create_args_t args = {};
    args.callback = enc_poll_cb;
    args.dispatch_method = ESP_TIMER_TASK;
    args.name = "ring";
    if (esp_timer_create(&args, &enc_timer) == ESP_OK) {
        esp_timer_start_periodic(enc_timer, ENC_POLL_US);
    }

    haptic_init();
}

bool input_hal_is_held(InputButton btn) {
    switch (btn) {
    case INPUT_BTN_PRIMARY:
        return digitalRead(BTN_BACK_GPIO) == LOW;
    case INPUT_BTN_SECONDARY:
        return false;
    }
    return false;
}

int input_hal_encoder_steps(void) {
    portENTER_CRITICAL(&enc_mux);
    int32_t steps = enc_steps;
    enc_steps = 0;
    portEXIT_CRITICAL(&enc_mux);
    // Called from loop(), the same context as touch_hal_read(), so the I2C
    // write can't interleave with a touch transaction.
    if (steps != 0) haptic_click();
    return (int)steps;
}
