#pragma once

// Waveshare ESP32-S3-Knob-Touch-LCD-1.8 — round 1.8" TFT set inside a rotary
// ring. 360x360 ST77916 over QSPI + CST816 touch + a bidirectional detent
// switch as the ring + DRV2605 haptic driver. A second MCU (ESP32-U4WDH) on the
// board owns classic-BT audio and the second ring encoder; this port never
// talks to it.
//
// Pin map from Waveshare's demo package (08_LVGL_Test/lcd_config.h,
// 04_Encoder_Test, 03_DRV2605_Test) cross-checked against the schematic.

#define BOARD_NAME           "Waveshare Knob 1.8"

// ---- Display geometry ----
// The panel is round: the full 360x360 frame is addressable, but only the
// inscribed circle is visible (BoardCaps.is_round tells the UI).
#define LCD_WIDTH            360
#define LCD_HEIGHT           360
// Fixed 180° so the USB-C socket sits at the top. Done in the panel (MADCTL
// MX|MY via the GFX rotation), so it costs nothing per frame; touch.cpp
// mirrors its coordinates to match. 0 = Waveshare's orientation, USB at the
// bottom.
#define LCD_ROTATION_180     1

// ---- QSPI display pins (ST77916) ----
#define LCD_CS               14
#define LCD_SCLK             13
#define LCD_SDIO0            15
#define LCD_SDIO1            16
#define LCD_SDIO2            17
#define LCD_SDIO3            18
#define LCD_RST              21
#define LCD_BL               47    // backlight MOSFET gate, LEDC PWM (HIGH = on)

// ---- I2C bus (touch + haptic share one bus) ----
#define IIC_SDA              11
#define IIC_SCL              12

// ---- Touch (CST816, minimal inline I2C reader) ----
#define TP_INT               9
#define TP_RST               10
#define CST816_ADDR          0x15

// ---- Ring (bidirectional detent switch, not a quadrature encoder) ----
// Each channel is its own switch: A pulses LOW once per detent turned one
// way, B once per detent the other way. 10k pull-ups on the board.
#define ENC_A_GPIO           8     // +1 (Waveshare's KNOB_RIGHT)
#define ENC_B_GPIO           7     // -1 (KNOB_LEFT)

// ---- Haptic (DRV2605 + vibration motor on the touch I2C bus) ----
#define DRV2605_ADDR         0x5A
#define HAPTIC_DETENT_EFFECT 5     // ROM library effect: 5 = Sharp Click 60%

// ---- Buttons ----
// BOOT is the only key the S3 can read, and it sits inside the closed case —
// in normal use the touchscreen stands in for it (BoardCaps.touch_keys). Still
// wired up for anyone with the case open. GPIO0 doubles as the CH445P I2S
// source select, which is harmless while audio is unused.
#define BTN_BACK_GPIO        0     // BOOT — primary, Space (PTT); hold = pair

// ---- Capability flags ----
#define BOARD_HAS_SECONDARY_BUTTON 0
#define BOARD_HAS_ROTATION         0
#define BOARD_HAS_IMU              0
// BATT_ADC (GPIO1) sits on a divider from the 5 V rail, not the cell, so it
// can't give a charge level.
#define BOARD_HAS_BATTERY          0
#define BOARD_HAS_IO_EXPANDER      0
// The PCM5100A DAC only has a line-out on the expansion connector, and its
// mute pin (XSMT) is driven by the second MCU.
#define BOARD_HAS_SOUND            0
