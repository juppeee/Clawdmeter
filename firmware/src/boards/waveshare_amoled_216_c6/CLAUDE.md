# AMOLED-2.16 (C6) — `waveshare_amoled_216_c6`

Hardware notes for this board. Shared firmware rules are in `firmware/CLAUDE.md`.

Waveshare ESP32-C6-Touch-AMOLED-2.16 (CO5300, 480×480, CST9217 touch). Build env: `waveshare_amoled_216_c6`. ESP32-C6 SoC: single-core RISC-V, **no PSRAM**, BLE 5 only.

## Audio

- ES8311 codec on the shared I2C bus (0x18), I2S MCLK 19, BCLK 20, WS 22, DOUT 23, DIN 21 (from Waveshare's `07_Audio_Test`).
- No amp-enable line: Waveshare's BSP sets `BSP_POWER_AMP_IO` to `GPIO_NUM_NC`, so `sound.cpp` passes a null `amp_enable`. Don't hunt for a PA pin on the TCA9554.
- `SND_SAMPLE_RATE` is 16 kHz so the ding buffer (~53 KB) fits in internal RAM; the boot log should show `room for 26720 of 26720 frames`.
- The `i2cWrite ... ESP_ERR_INVALID_STATE` line during `imu_hal_init` is SensorLib polling the QMI8658 mid-reset (lewisxhe/SensorLib#46). It's harmless and unrelated to audio.
