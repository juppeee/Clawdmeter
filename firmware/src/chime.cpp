#include "chime.h"
#include <Arduino.h>
#include <math.h>
#include "ESP_I2S.h"
#include "es8311.h"
#include "bell_pcm.h"   // const uint8_t bell_pcm[] / bell_pcm_len — 44.1 kHz 16-bit stereo

// Shared ES8311 chime engine. See chime.h. Adapted from the original 2.16
// sound.cpp so the 2.16, 1.8 (and any future ES8311 board) share one copy of
// the codec setup, the embedded PCM, and the non-blocking playback task.

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

static void chime_task(void* arg) {
    if (cfg.amp_enable) cfg.amp_enable(true);
    delay(8);                                  // let the amp settle (avoids turn-on pop)
    i2s.write((uint8_t*)bell_pcm, bell_pcm_len);
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

// ---- Synthesized UI cues (see chime.h) ----

struct cue_note { uint16_t freq_hz; uint16_t ms; };

static const cue_note cue_armed[]  = { {880, 90} };
static const cue_note cue_paired[] = { {660, 70}, {988, 130} };

static const cue_note* cue_seq = nullptr;
static uint8_t         cue_len = 0;

#define CUE_AMPLITUDE 9000    // ~28% of full scale — audible, no amp strain
#define CUE_EDGE_MS   4       // attack/release ramp; without it the amp clicks

static void cue_task(void* arg) {
    if (cfg.amp_enable) cfg.amp_enable(true);
    delay(8);                                  // amp settle, same as the bell

    static int16_t frames[256 * 2];            // stereo scratch, one cue at a time
    const uint32_t edge = (uint32_t)cfg.sample_rate * CUE_EDGE_MS / 1000;

    for (uint8_t n = 0; n < cue_len; n++) {
        const uint32_t total = (uint32_t)cfg.sample_rate * cue_seq[n].ms / 1000;
        const float    step  = 2.0f * (float)M_PI * cue_seq[n].freq_hz / cfg.sample_rate;
        float          phase = 0.0f;
        for (uint32_t done = 0; done < total; ) {
            uint32_t chunk = total - done;
            if (chunk > 256) chunk = 256;
            for (uint32_t i = 0; i < chunk; i++) {
                const uint32_t pos = done + i;
                float env = 1.0f;
                if (pos < edge)                env = (float)pos / (float)edge;
                else if (total - pos < edge)   env = (float)(total - pos) / (float)edge;
                const int16_t s = (int16_t)(sinf(phase) * CUE_AMPLITUDE * env);
                phase += step;
                frames[i * 2]     = s;
                frames[i * 2 + 1] = s;
            }
            i2s.write((uint8_t*)frames, chunk * 4);   // 2 ch x 16 bit
            done += chunk;
        }
    }

    delay(20);
    if (cfg.amp_enable) cfg.amp_enable(false);
    playing = false;
    vTaskDelete(nullptr);
}

void chime_play_cue(chime_cue_t cue) {
    if (!ready || playing) return;
    if (cue == CHIME_CUE_PAIRED) { cue_seq = cue_paired; cue_len = 2; }
    else                         { cue_seq = cue_armed;  cue_len = 1; }
    playing = true;
    if (xTaskCreatePinnedToCore(cue_task, "chime_cue", 4096, nullptr, 1, nullptr, 0) != pdPASS)
        playing = false;   // couldn't spawn — stay silent rather than wedge the flag
}

void chime_tick(void) {}   // playback runs in chime_task; nothing to poll
