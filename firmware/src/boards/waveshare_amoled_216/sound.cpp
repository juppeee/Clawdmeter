#include "../../hal/sound_hal.h"
#include "board.h"

#if BOARD_HAS_SOUND

#include <Arduino.h>
#include "ESP_I2S.h"
#include "es8311.h"
#include "bell_pcm.h"   // const uint8_t bell_pcm[] / bell_pcm_len — 16 kHz 16-bit stereo

// Reset chime on the ESP32-S3-Touch-AMOLED-2.16's onboard ES8311 codec + speaker.
//
// The chime PCM (a triple-bell alert, embedded in flash from the user's MP3) is
// streamed to the codec over I2S inside a one-shot FreeRTOS task so the LVGL
// render loop never blocks on the ~0.9 s write. The external power amp
// (SND_PA_PIN) is enabled only while the chime plays — kept off otherwise to
// avoid idle hiss. Codec config mirrors Waveshare's factory 07_ES8311 example.

static I2SClass      i2s;
static bool          ready   = false;
static volatile bool playing = false;

static bool es8311_setup(void) {
    es8311_handle_t es = es8311_create(0, SND_ES8311_ADDR);   // I2C port 0 (shared Wire bus)
    if (!es) return false;
    // mclk_inverted, sclk_inverted, mclk_from_mclk_pin, mclk_frequency, sample_frequency
    const es8311_clock_config_t clk = {
        false, false, true, SND_SAMPLE_RATE * 256, SND_SAMPLE_RATE
    };
    if (es8311_init(es, &clk, ES8311_RESOLUTION_16, ES8311_RESOLUTION_16) != ESP_OK) return false;
    es8311_sample_frequency_config(es, clk.mclk_frequency, clk.sample_frequency);
    es8311_microphone_config(es, false);
    es8311_voice_volume_set(es, 65, NULL);   // 0..100
    return true;
}

static void chime_task(void* arg) {
    digitalWrite(SND_PA_PIN, HIGH);   // enable amp
    delay(8);                         // let it settle (avoids turn-on pop)
    i2s.write((uint8_t*)bell_pcm, bell_pcm_len);
    delay(20);
    digitalWrite(SND_PA_PIN, LOW);    // amp back off
    playing = false;
    vTaskDelete(nullptr);
}

void sound_hal_init(void) {
    pinMode(SND_PA_PIN, OUTPUT);
    digitalWrite(SND_PA_PIN, LOW);

    i2s.setPins(SND_I2S_BCLK, SND_I2S_WS, SND_I2S_DOUT, SND_I2S_DIN, SND_I2S_MCLK);
    if (!i2s.begin(I2S_MODE_STD, SND_SAMPLE_RATE, I2S_DATA_BIT_WIDTH_16BIT,
                   I2S_SLOT_MODE_STEREO, I2S_STD_SLOT_BOTH)) {
        Serial.println("sound: I2S init failed");
        return;
    }
    if (!es8311_setup()) {
        Serial.println("sound: ES8311 init failed");
        return;
    }
    ready = true;
    Serial.println("sound: ES8311 ready");
}

void sound_hal_play_reset(void) {
    if (!ready || playing) return;
    playing = true;
    if (xTaskCreatePinnedToCore(chime_task, "chime", 4096, nullptr, 1, nullptr, 0) != pdPASS)
        playing = false;   // couldn't spawn — stay silent rather than wedge the flag
}

void sound_hal_tick(void) {}   // playback runs in chime_task; nothing to poll

#endif  // BOARD_HAS_SOUND
