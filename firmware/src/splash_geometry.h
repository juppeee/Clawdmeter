#pragma once
#include <stdint.h>

// Pure geometry for the 60x60 grid Clawd performs on at full size
// (performer.cpp; mascot.cpp sizes the lurk and attention sprites from it).
// Deliberately free of LVGL/Arduino dependencies so it can be unit-tested on
// the host (see test/test_splash_geometry/). Decides the in-buffer pixel size
// of each grid cell and the LVGL image scale needed to fill the panel.
//
// With PSRAM the buffer is rendered at full panel size (scale = 1.0x).
// Without it, the geometry is a tiny 60x60 buffer (~7 KB) for LVGL to upscale
// with nearest-neighbour; the performer doesn't use that path (it needs
// per-pixel alpha at full size), so PSRAM-less boards play corner acts.

#define SPLASH_GRID         60
#define SPLASH_SCALE_UNITY  256   // LVGL image-scale denominator (256 == 1.0x)

typedef struct {
    int cell;        // px per grid cell inside the canvas buffer
    int canvas_dim;  // SPLASH_GRID * cell (canvas is square)
    int scale;       // LVGL image scale in 1/256 units (SPLASH_SCALE_UNITY == none)
} SplashGeometry;

static inline SplashGeometry splash_compute_geometry(int width, int height,
                                                     bool has_psram) {
    int min_dim = (width < height) ? width : height;
    int target_cell = min_dim / SPLASH_GRID;   // desired on-screen px per cell
    if (target_cell < 1) target_cell = 1;

    SplashGeometry g;
    if (has_psram) {
        g.cell  = target_cell;             // render at full size...
        g.scale = SPLASH_SCALE_UNITY;      // ...and don't scale
    } else {
        g.cell  = 1;                       // tiny 20x20 buffer...
        g.scale = SPLASH_SCALE_UNITY * target_cell;  // ...scaled up to full size
    }
    g.canvas_dim = SPLASH_GRID * g.cell;
    return g;
}
