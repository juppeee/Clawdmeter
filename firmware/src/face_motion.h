#pragma once
#include <stdint.h>

// The face's rise and drop as a pure function of elapsed time
// (docs/clawd-director.md, "The face's motion"). No LVGL, no Arduino: the
// same (move, t, geometry) always gives the same pose, so it's unit-tested on
// the host and the durations never depend on frame rate.
//
// Hand-keyed, not simulated: an ease-out rise past rest, then an underdamped
// spring settle; a lean (drawn as a stepped shear of the top edge) on its own
// slower spring; eyes and arms lagging the body; squash and stretch on the
// eyes. The drop is an anticipation hop, then an ease-in fall leaning into it.

enum FaceMove { FACE_MOVE_RISE, FACE_MOVE_DROP };

struct FaceGeom {
    int w, h;          // panel
    int mind;          // shorter side: the design scale (authored on 480)
    int cell;          // art cell (min(W,H)/60): the lean's step
    int start_y;       // where the body's top edge starts (h = from below the bezel)
    int dir;           // which way he leans: -1 left side low, +1 right side low
};

struct FacePose {
    int y;             // the body's top edge at the panel's centre column
    int lean;          // right edge minus left edge of the top edge, px (a multiple of cell)
    int x;             // body drift, px
    int margin;        // px the body is narrower than the panel on each side (arms show there)
    int eye_dy;        // eyes' follow-through, px, on top of riding the body
    int arm_dy;        // arms' follow-through, px
    int stretch;       // eyes, permille: +200 = 20 % taller and narrower, -200 squashed
};

uint32_t face_motion_duration(FaceMove m);
// When the rise first reaches rest (the end of `face-rise` in perf; the
// settle runs from there to the end).
uint32_t face_motion_rest_ms(void);
FacePose face_motion_pose(FaceMove m, uint32_t t_ms, const FaceGeom* g);

// Arm nub size and the body's narrowing while moving, px.
int face_arm_w(const FaceGeom* g);
int face_arm_h(const FaceGeom* g);
