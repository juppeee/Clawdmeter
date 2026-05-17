#pragma once

#include <Arduino_GFX_Library.h>
#include <XPowersLib.h>
#include <SensorQMI8658.hpp>
#include <Wire.h>

// ============================================================================
// Board variant selection — driven by build flag in platformio.ini
//   -DBOARD_AMOLED_216  → original Waveshare ESP32-S3-Touch-AMOLED-2.16 (CO5300, CST9220, 480x480)
//   -DBOARD_AMOLED_18   → newer Waveshare ESP32-S3-Touch-AMOLED-1.8     (SH8601, FT3168,  368x448)
// ============================================================================

#if defined(BOARD_AMOLED_18)

// ---- Display resolution (portrait) ----
#define LCD_WIDTH   368
#define LCD_HEIGHT  448

// ---- QSPI display pins (SH8601) ----
#define LCD_CS      12
#define LCD_SCLK    11        // NOTE: different from AMOLED-2.16 (was GPIO 38)
#define LCD_SDIO0   4
#define LCD_SDIO1   5
#define LCD_SDIO2   6
#define LCD_SDIO3   7
#define LCD_RESET   GFX_NOT_DEFINED   // routed via XCA9554 EXIO1

// ---- I2C bus (touch + PMU + IMU + IO expander all share one bus) ----
#define IIC_SDA     15
#define IIC_SCL     14

// ---- Touch (FT3168 via I2C) ----
#define TP_INT      21
#define FT3168_ADDR 0x38

// ---- PMU (AXP2101 via I2C) ----
#define AXP2101_ADDR 0x34

// ---- IO expander (XCA9554/PCA9554-compatible via I2C) ----
// Gates LCD_RST, TP_RST, audio amp reset, and PWR button readback.
#define XCA9554_ADDR 0x20
#define IOX_PIN_TP_RST  0   // EXIO0 → touch reset (active LOW)
#define IOX_PIN_LCD_RST 1   // EXIO1 → display reset (active LOW)
#define IOX_PIN_PA_EN   2   // EXIO2 → audio amp enable (we keep HIGH to release)
#define IOX_PIN_PWR_BTN 4   // EXIO4 → PWR button input, active HIGH

// ---- Display class typedef ----
typedef Arduino_SH8601 PlatformDisplay;

// ---- Global hardware objects (defined in main.cpp) ----
extern Arduino_DataBus *bus;
extern PlatformDisplay *gfx;
extern XPowersPMU pmu;
extern SensorQMI8658 imu;

#else  // BOARD_AMOLED_216 (default / original board)

#include <TouchDrvCSTXXX.hpp>

// ---- Display resolution ----
#define LCD_WIDTH   480
#define LCD_HEIGHT  480

// ---- QSPI display pins (CO5300) ----
#define LCD_CS      12
#define LCD_SCLK    38
#define LCD_SDIO0   4
#define LCD_SDIO1   5
#define LCD_SDIO2   6
#define LCD_SDIO3   7
#define LCD_RESET   2

// ---- Touch pins (CST9220 via I2C) ----
#define IIC_SDA     15
#define IIC_SCL     14
#define TP_INT      11
#define TP_RST      2    // shared with LCD_RESET
#define CST9220_ADDR 0x5A

// ---- PMU (AXP2101 via same I2C) ----
#define AXP2101_ADDR 0x34

// ---- Display class typedef ----
typedef Arduino_CO5300 PlatformDisplay;

// ---- Global hardware objects (defined in main.cpp) ----
extern Arduino_DataBus *bus;
extern PlatformDisplay *gfx;
extern TouchDrvCST92xx touch;
extern XPowersPMU pmu;
extern SensorQMI8658 imu;

#endif  // BOARD_AMOLED_*
