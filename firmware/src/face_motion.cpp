#include "face_motion.h"
#include <math.h>

// All distances are authored on a 480 px panel and scaled by the shorter side;
// times are absolute ms. See docs/clawd-director.md, "The face's motion".

// Rise: ease-out to OVER past rest, then a spring settle.
#define RISE_UP_MS      500     // ease-out leg, ends at the overshoot apex
#define RISE_END_MS     860     // settle done: dead still at rest
#define RISE_OVER       44      // overshoot past rest, px@480
#define SPRING_MS       300.0f  // settle period
#define SPRING_TAU      110.0f  // settle decay
#define LEAN_CELLS      9       // lean on the way up, cells
#define LEAN_MS         340.0f  // the lean's own (slower) spring
#define LEAN_TAU        140.0f
#define DRIFT           28      // arc: starts this far toward his lean side, px@480
#define WIDEN_FROM_MS   420     // presses to the glass: body widens to full bleed
#define WIDEN_MS        180

// Drop: crouch, hop, ease-in fall.
#define DROP_CROUCH_MS  60
#define DROP_HOP_MS     90
#define DROP_ANT_MS     (DROP_CROUCH_MS + DROP_HOP_MS)
#define DROP_FALL_MS    360
#define DROP_END_MS     (DROP_ANT_MS + DROP_FALL_MS)
#define DROP_CROUCH     6       // px@480: the crouch before the hop
#define DROP_HOP        28      // px@480
#define DROP_LEAN_CELLS 8
#define DROP_COUNTER    2       // cells of counter-lean while he gathers
#define DROP_DRIFT      40      // px@480 toward the low side by the end

// Follow-through: eyes and arms trail the body.
#define EYE_LAG_MS      40
#define ARM_LAG_MS      70
#define EYE_LAG_MAX     40      // px@480
#define ARM_LAG_MAX     56

#define ARM_W           48      // px@480: 6 cells wide, 7 tall
#define ARM_H           56

static const float PI_F = 3.14159265f;

static int sc(const FaceGeom* g, int v) { return v * g->mind / 480; }
static float scf(const FaceGeom* g, float v) { return v * (float)g->mind / 480.0f; }

static int clampi(int v, int lo, int hi) { return v < lo ? lo : (v > hi ? hi : v); }

static int iround(float v) { return (int)(v < 0 ? v - 0.5f : v + 0.5f); }

static int cell_of(const FaceGeom* g) { return g->cell > 0 ? g->cell : 1; }

int face_arm_w(const FaceGeom* g) {
    const int c = cell_of(g);
    const int w = sc(g, ARM_W) / c * c;
    return w > c ? w : c;
}

int face_arm_h(const FaceGeom* g) {
    const int c = cell_of(g);
    const int h = sc(g, ARM_H) / c * c;
    return h > c ? h : c;
}

uint32_t face_motion_duration(FaceMove m) {
    return m == FACE_MOVE_RISE ? RISE_END_MS : DROP_END_MS;
}

// An underdamped spring released at rest-offset 1 with zero velocity:
// 1 at t = 0, decaying oscillation about 0.
static float spring(float t, float period, float tau) {
    const float w = 2.0f * PI_F / period;
    return expf(-t / tau) * (cosf(w * t) + sinf(w * t) / (w * tau));
}

uint32_t face_motion_rest_ms(void) {
    // First zero of spring(): tan(wt) = -w*tau.
    const float w = 2.0f * PI_F / SPRING_MS;
    const float t = (PI_F - atanf(w * SPRING_TAU)) / w;
    return RISE_UP_MS + (uint32_t)(t + 0.5f);
}

static int drop_end_y(const FaceGeom* g) {
    // Everything past the bezel, even the high side of the lean.
    return g->h + DROP_LEAN_CELLS * cell_of(g) / 2 + cell_of(g);
}

static float body_y(FaceMove m, float t, const FaceGeom* g) {
    if (t < 0) t = 0;
    if (m == FACE_MOVE_RISE) {
        const float over = scf(g, RISE_OVER);
        if (t < RISE_UP_MS) {
            const float u = t / RISE_UP_MS;
            const float e = 1.0f - (1.0f - u) * (1.0f - u);
            return g->start_y - (g->start_y + over) * e;
        }
        if (t >= RISE_END_MS) return 0;
        return -over * spring(t - RISE_UP_MS, SPRING_MS, SPRING_TAU);
    }
    if (t < DROP_CROUCH_MS)
        return g->start_y + scf(g, DROP_CROUCH) * sinf(PI_F * t / DROP_CROUCH_MS);
    const float top = g->start_y - scf(g, DROP_HOP);
    if (t < DROP_ANT_MS) {
        const float v = (t - DROP_CROUCH_MS) / DROP_HOP_MS;
        return g->start_y - scf(g, DROP_HOP) * (1.0f - (1.0f - v) * (1.0f - v));
    }
    if (t >= DROP_END_MS) return drop_end_y(g);
    const float w = (t - DROP_ANT_MS) / DROP_FALL_MS;
    return top + (drop_end_y(g) - top) * powf(w, 2.0f);
}

static int quant(float v, int c) { return iround(v / c) * c; }

FacePose face_motion_pose(FaceMove m, uint32_t t_ms, const FaceGeom* g) {
    FacePose p = {};
    const int c = cell_of(g);
    const int dir = g->dir < 0 ? -1 : 1;
    const int margin = face_arm_w(g);
    const float t = (float)t_ms;
    const float y = body_y(m, t, g);
    p.y = iround(y);
    p.eye_dy = clampi(iround(body_y(m, t - EYE_LAG_MS, g) - y), -sc(g, EYE_LAG_MAX), sc(g, EYE_LAG_MAX));
    p.arm_dy = clampi(iround(body_y(m, t - ARM_LAG_MS, g) - y), -sc(g, ARM_LAG_MAX), sc(g, ARM_LAG_MAX));

    if (m == FACE_MOVE_RISE) {
        if (t_ms >= RISE_END_MS) { p.y = 0; p.eye_dy = 0; p.arm_dy = 0; return p; }
        float lean;
        if (t < RISE_UP_MS) {
            const float u = t / RISE_UP_MS;
            lean = LEAN_CELLS * c * (1.0f - 0.4f * u * u);
            p.x = iround(dir * scf(g, DRIFT) * (1.0f - u) * (1.0f - u));
            // Fast and stretched, relaxing as he slows into the apex.
            p.stretch = iround(220.0f * powf(1.0f - u, 0.8f));
        } else {
            const float ts = t - RISE_UP_MS;
            lean = 0.6f * LEAN_CELLS * c * spring(ts, LEAN_MS, LEAN_TAU);
            // Lands in the dip: squash, then a small rebound stretch.
            if (ts >= 40 && ts < 240)       p.stretch = -iround(240.0f * sinf(PI_F * (ts - 40) / 200.0f));
            else if (ts >= 240 && ts < 360) p.stretch = iround(70.0f * sinf(PI_F * (ts - 240) / 120.0f));
        }
        p.lean = dir * quant(lean, c);
        if (t_ms < WIDEN_FROM_MS) p.margin = margin;
        else if (t_ms < WIDEN_FROM_MS + WIDEN_MS)
            p.margin = (margin * (int)(WIDEN_FROM_MS + WIDEN_MS - t_ms) / WIDEN_MS + c - 1) / c * c;
        if (p.margin > margin) p.margin = margin;
        return p;
    }

    // Drop.
    if (t_ms < DROP_CROUCH_MS) {
        p.stretch = -iround(140.0f * sinf(PI_F * t / DROP_CROUCH_MS));
    } else if (t_ms < DROP_ANT_MS) {
        const float v = (t - DROP_CROUCH_MS) / DROP_HOP_MS;
        p.stretch = iround(140.0f * sinf(PI_F / 2 * v));
    } else {
        const float w = t_ms >= DROP_END_MS ? 1.0f : (t - DROP_ANT_MS) / DROP_FALL_MS;
        p.stretch = iround(140.0f + 80.0f * w);
        p.x = iround(dir * scf(g, DROP_DRIFT) * w * w);
    }
    float lean;
    if (t_ms < DROP_ANT_MS) lean = -DROP_COUNTER * c * sinf(PI_F * t / DROP_ANT_MS);
    else {
        const float w = t_ms >= DROP_END_MS ? 1.0f : (t - DROP_ANT_MS) / DROP_FALL_MS;
        lean = DROP_LEAN_CELLS * c * powf(w, 1.6f);
    }
    p.lean = dir * quant(lean, c);
    const uint32_t pull = 120;      // the body pulls in, the arms pop in
    p.margin = t_ms >= pull ? margin : (margin * (int)t_ms / (int)pull + c - 1) / c * c;
    if (p.margin > margin) p.margin = margin;
    return p;
}
