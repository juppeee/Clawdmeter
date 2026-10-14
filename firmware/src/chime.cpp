#include "chime.h"
#include <Arduino.h>
#include "ESP_I2S.h"
#include "es8311.h"
#include <string.h>
#include <esp_heap_caps.h>
#include "ding.h"

// Shared ES8311 chime engine. See chime.h. The 2.16, 1.8, LCD-1.54 (and any
// future ES8311 board) share one copy of the codec setup and the
// non-blocking playback task. The ding is rendered once into a buffer
// (PSRAM where there is some) and streamed with one write, so nothing is
// computed while the DMA waits: computing it live starved the DMA (ESP_I2S
// keeps ~33 ms of buffers and doesn't let us grow them) and crackled.

// Share of the ding's own level (it peaks just under full scale): headroom
// for the codec and the small amp. ChimeConfig.volume sets the loudness.
#define DING_GAIN 0.7f
#define CHUNK_FRAMES 256
// Core 1, above the Arduino loop (LVGL); the NimBLE host and radio run on core 0.
#define CHIME_TASK_PRIORITY 10

static I2SClass      i2s;
static ChimeConfig   cfg;
static bool          ready   = false;
static volatile bool playing = false;
static es8311_handle_t es    = nullptr;

static float    gain       = DING_GAIN;
static int16_t* pcm        = nullptr;   // stereo frames of the rendered ding
static uint32_t pcm_frames = 0;
static float    pcm_gain   = -1.0f;     // gain `pcm` was rendered at

static bool es8311_setup(void) {
    es = es8311_create(0, cfg.es8311_addr);   // I2C port 0 (shared Wire bus)
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

// Render the ding at the current gain into `pcm` (allocated on first use).
// False when there's no room: playback then computes it in chunks.
static bool render(void) {
    const uint32_t rate = (uint32_t)cfg.sample_rate, frames = ding_frames(rate);
    const size_t bytes = (size_t)frames * 2 * sizeof(int16_t);
    if (!pcm) {
        pcm = (int16_t*)heap_caps_malloc(bytes, MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT);
        if (!pcm) pcm = (int16_t*)heap_caps_malloc(bytes, MALLOC_CAP_8BIT);
        if (!pcm) return false;
    }
    const uint32_t t0 = millis();
    for (uint32_t n = 0; n < frames; n++) {
        const int16_t s = ding_sample(n, rate, gain);
        pcm[2 * n] = s;
        pcm[2 * n + 1] = s;
    }
    pcm_frames = frames;
    pcm_gain = gain;
    Serial.printf("chime: rendered %u frames at gain %.2f in %lu ms\n", (unsigned)frames, gain,
                  (unsigned long)(millis() - t0));
    return true;
}

static void chime_task(void* arg) {
    const bool rendered = pcm_gain == gain || render();
    if (cfg.amp_enable) cfg.amp_enable(true);
    delay(8);                                  // let the amp settle (avoids turn-on pop)
    if (rendered) {
        i2s.write((uint8_t*)pcm, (size_t)pcm_frames * 2 * sizeof(int16_t));
    } else {
        const uint32_t rate = (uint32_t)cfg.sample_rate, frames = ding_frames(rate);
        for (uint32_t n = 0; n < frames;) {
            uint32_t k = 0;
            for (; k < CHUNK_FRAMES && n < frames; k++, n++) {
                const int16_t s = ding_sample(n, rate, gain);
                chunk[2 * k] = s;
                chunk[2 * k + 1] = s;
            }
            i2s.write((uint8_t*)chunk, k * 2 * sizeof(int16_t));
        }
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
    if (!render()) Serial.println("chime: no room to pre-render; computing while playing");
    return true;
}

void chime_play(void) {
    if (!ready || playing) return;
    playing = true;
    if (xTaskCreatePinnedToCore(chime_task, "chime", 4096, nullptr, CHIME_TASK_PRIORITY, nullptr, 1) != pdPASS)
        playing = false;   // couldn't spawn — stay silent rather than wedge the flag
}

void chime_tick(void) {}   // playback runs in chime_task; nothing to poll

void chime_tune(float g, int volume) {
    if (!ready || playing) return;
    gain = g < 0.0f ? 0.0f : (g > 1.0f ? 1.0f : g);   // re-rendered on the next play
    if (volume >= 0 && es) es8311_voice_volume_set(es, volume > 100 ? 100 : volume, NULL);
    chime_play();
}
