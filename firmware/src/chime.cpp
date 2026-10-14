#include "chime.h"
#include <Arduino.h>
#include "ESP_I2S.h"
#include "es8311.h"
#include <string.h>
#include "ding.h"

// Shared ES8311 chime engine. See chime.h. The 2.16, 1.8, LCD-1.54 (and any
// future ES8311 board) share one copy of the codec setup and the
// non-blocking playback task, which synthesizes the ding as it streams.

// The ding at its own level (it peaks just under full scale). The codec
// volume (ChimeConfig.volume) sets the loudness on top of this.
#define DING_GAIN 1.0f
#define CHUNK_FRAMES 256

static I2SClass      i2s;
static ChimeConfig   cfg;
static bool          ready   = false;
static volatile bool playing = false;

static bool es8311_setup(void) {
    es8311_handle_t es = es8311_create(0, cfg.es8311_addr);   // I2C port 0 (shared Wire bus)
    if (!es) return false;
    // mclk_inverted, sclk_inverted, mclk_from_mclk_pin, mclk_frequency, sample_frequency
    const es8311_clock_config_t clk = {
        false, false, true, cfg.sample_rate * 256, cfg.sample_rate
    };
    if (es8311_init(es, &clk, ES8311_RESOLUTION_16, ES8311_RESOLUTION_16) != ESP_OK) return false;
    es8311_sample_frequency_config(es, clk.mclk_frequency, clk.sample_frequency);
    es8311_microphone_config(es, false);
    es8311_voice_volume_set(es, cfg.volume, NULL);
    return true;
}

static int16_t chunk[CHUNK_FRAMES * 2];      // stereo frames, off the task stack

static void chime_task(void* arg) {
    if (cfg.amp_enable) cfg.amp_enable(true);
    delay(8);                                  // let the amp settle (avoids turn-on pop)
    const uint32_t rate = (uint32_t)cfg.sample_rate, frames = ding_frames(rate);
    for (uint32_t n = 0; n < frames;) {
        uint32_t k = 0;
        for (; k < CHUNK_FRAMES && n < frames; k++, n++) {
            const int16_t s = ding_sample(n, rate, DING_GAIN);
            chunk[2 * k] = s;
            chunk[2 * k + 1] = s;
        }
        i2s.write((uint8_t*)chunk, k * 2 * sizeof(int16_t));
    }
    memset(chunk, 0, sizeof(chunk));           // a little silence so the DMA drains
    i2s.write((uint8_t*)chunk, sizeof(chunk));
    delay(20);
    if (cfg.amp_enable) cfg.amp_enable(false);
    playing = false;
    vTaskDelete(nullptr);
}

bool chime_init(const ChimeConfig& c) {
    cfg = c;
    if (cfg.amp_enable) cfg.amp_enable(false);   // amp off until we play

    i2s.setPins(cfg.bclk, cfg.ws, cfg.dout, cfg.din, cfg.mclk);
    if (!i2s.begin(I2S_MODE_STD, cfg.sample_rate, I2S_DATA_BIT_WIDTH_16BIT,
                   I2S_SLOT_MODE_STEREO, I2S_STD_SLOT_BOTH)) {
        Serial.println("chime: I2S init failed");
        return false;
    }
    if (!es8311_setup()) {
        Serial.println("chime: ES8311 init failed");
        return false;
    }
    ready = true;
    Serial.println("chime: ES8311 ready");
    return true;
}

void chime_play(void) {
    if (!ready || playing) return;
    playing = true;
    if (xTaskCreatePinnedToCore(chime_task, "chime", 4096, nullptr, 1, nullptr, 0) != pdPASS)
        playing = false;   // couldn't spawn — stay silent rather than wedge the flag
}

void chime_tick(void) {}   // playback runs in chime_task; nothing to poll
