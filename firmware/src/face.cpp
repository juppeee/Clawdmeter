#include "face.h"
#include "clawd_policy.h"
#include "theme.h"
#include "hal/board_caps.h"
#include <lvgl.h>
#include <Arduino.h>

// Geometry is authored on a 480 px square (design system storyboards) and
// scaled by the panel's shorter side; the eye block is centred vertically.
//   eyes: 44 x 120 px, centres 160 px apart, top at y 150
//   squint: u = 12 px cells, a 5-row chevron per eye (> <)
//   glance: 20 px, far eye 1.5x
#define RISE_MS    600
#define DROP_MS    480
#define BLINK_MS   180
#define BLINK_GAP  140
#define GLANCE_MS  280

#define COL_FACE   THEME_ACCENT               // Clawd orange
#define COL_EYE    lv_color_hex(0x141413)

static lv_obj_t* root = NULL;
static lv_obj_t* eye_box[2];
static lv_obj_t* eye_open[2];
static lv_obj_t* squint[2][9];

static int W = 480, H = 480, mind = 480;
static int eye_cx[2];        // eye centre x, px
static int eye_top = 150;    // eye block top, px (face-local)
static int glance_px = 20;

static bool moving = false;
static bool rising = false;
static bool visible = false;
static bool resting = false;

// blink: 0 none, 1 closed, 2 open gap (double), 3 closed again
static uint8_t  blink_phase = 0;
static uint32_t blink_until = 0;
static bool     blink_twice = false;
// glance: 0 none, 1 out (anim), 2 hold, 3 back (anim)
static uint8_t  glance_phase = 0;
static uint32_t glance_until = 0;
static int      glance_target = 0;
static int      glance_cur = 0;
static uint32_t next_act_at = 0;
static ClawdRng rng;

static int S(int v) { return v * mind / 480; }

static void place_eyes(int g) {
    int dl, dr;
    face_eye_offsets(g, &dl, &dr);
    lv_obj_set_x(eye_box[0], eye_cx[0] - S(30) + dl);
    lv_obj_set_x(eye_box[1], eye_cx[1] - S(30) + dr);
}

static void set_squint(bool on) {
    for (int e = 0; e < 2; e++) {
        if (on) lv_obj_add_flag(eye_open[e], LV_OBJ_FLAG_HIDDEN);
        else    lv_obj_clear_flag(eye_open[e], LV_OBJ_FLAG_HIDDEN);
        for (int i = 0; i < 9; i++) {
            if (on) lv_obj_clear_flag(squint[e][i], LV_OBJ_FLAG_HIDDEN);
            else    lv_obj_add_flag(squint[e][i], LV_OBJ_FLAG_HIDDEN);
        }
    }
}

static lv_obj_t* rect(lv_obj_t* parent, int x, int y, int w, int h, lv_color_t c) {
    lv_obj_t* o = lv_obj_create(parent);
    lv_obj_remove_style_all(o);
    lv_obj_set_pos(o, x, y);
    lv_obj_set_size(o, w, h);
    lv_obj_set_style_bg_color(o, c, 0);
    lv_obj_set_style_bg_opa(o, LV_OPA_COVER, 0);
    lv_obj_clear_flag(o, LV_OBJ_FLAG_CLICKABLE);
    lv_obj_clear_flag(o, LV_OBJ_FLAG_SCROLLABLE);
    return o;
}

void face_init(void) {
    const BoardCaps& c = board_caps();
    W = c.width; H = c.height;
    mind = W < H ? W : H;
    eye_cx[0] = W / 2 - S(80);
    eye_cx[1] = W / 2 + S(80);
    eye_top = (H - mind) / 2 + S(150);
    glance_px = S(20);

    root = rect(lv_layer_top(), 0, H, W, H, COL_FACE);
    const int u = S(12) > 0 ? S(12) : 1;
    for (int e = 0; e < 2; e++) {
        // The box spans the open eye and the squint chevron (which reaches
        // 2u past the eye's right edge); its children are box-local.
        eye_box[e] = lv_obj_create(root);
        lv_obj_remove_style_all(eye_box[e]);
        lv_obj_set_size(eye_box[e], S(30) + 6 * u, S(30) + 10 * u);
        lv_obj_set_y(eye_box[e], eye_top);
        lv_obj_clear_flag(eye_box[e], LV_OBJ_FLAG_CLICKABLE);
        lv_obj_clear_flag(eye_box[e], LV_OBJ_FLAG_SCROLLABLE);
        eye_open[e] = rect(eye_box[e], S(8), 0, S(44), S(120), COL_EYE);
        const int dir = e == 0 ? +1 : -1;      // left eye ">", right eye "<"
        for (int i = 0; i < 5; i++) {
            const int dx = (dir > 0 ? i : 4 - i) * u;
            squint[e][i] = rect(eye_box[e], dx, S(20) + i * u, 2 * u, u, COL_EYE);
            if (i < 4)
                squint[e][5 + i] = rect(eye_box[e], dx, S(20) + (8 - i) * u, 2 * u, u, COL_EYE);
        }
    }
    place_eyes(0);
    set_squint(false);
    lv_obj_add_flag(root, LV_OBJ_FLAG_HIDDEN);
    clawd_rng_seed(&rng, millis() ^ 0xFACE);
}

static void y_exec(void* var, int32_t v) { lv_obj_set_y((lv_obj_t*)var, v); }

static void y_done(lv_anim_t* a) {
    (void)a;
    moving = false;
    if (!rising) {
        visible = false;
        lv_obj_add_flag(root, LV_OBJ_FLAG_HIDDEN);
    }
}

static void move_to(int to, uint32_t ms, lv_anim_path_cb_t path) {
    if (!root) return;
    lv_anim_delete(root, y_exec);
    lv_obj_clear_flag(root, LV_OBJ_FLAG_HIDDEN);
    lv_obj_move_foreground(root);
    visible = true;
    moving = true;
    lv_anim_t a;
    lv_anim_init(&a);
    lv_anim_set_var(&a, root);
    lv_anim_set_exec_cb(&a, y_exec);
    lv_anim_set_values(&a, lv_obj_get_y(root), to);
    lv_anim_set_duration(&a, ms);
    lv_anim_set_path_cb(&a, path);
    lv_anim_set_completed_cb(&a, y_done);
    lv_anim_start(&a);
}

void face_rise(void) {
    if (!root) return;
    rising = true;
    blink_phase = 0; glance_phase = 0;
    set_squint(false);
    glance_cur = 0; place_eyes(0);
    // Not yet on screen: start from just below the bezel.
    if (!visible) lv_obj_set_y(root, H);
    move_to(0, RISE_MS, lv_anim_path_ease_out);
    next_act_at = millis() + RISE_MS + 1200;
}

void face_drop(void) {
    if (!root || !visible) return;
    rising = false;
    resting = false;
    // Eyes open and front-facing as it goes: he's looking at you, then away.
    blink_phase = 0;
    set_squint(false);
    lv_anim_delete(&glance_cur, NULL);
    glance_phase = 0; glance_cur = 0; place_eyes(0);
    move_to(H, DROP_MS, lv_anim_path_ease_in);
}

void face_hide_now(void) {
    if (!root) return;
    lv_anim_delete(root, y_exec);
    lv_anim_delete(&glance_cur, NULL);
    moving = false; rising = false; visible = false; resting = false;
    blink_phase = 0; glance_phase = 0; glance_cur = 0;
    set_squint(false);
    place_eyes(0);
    lv_obj_set_y(root, H);
    lv_obj_add_flag(root, LV_OBJ_FLAG_HIDDEN);
}

bool face_visible(void) { return visible; }
bool face_moving(void)  { return moving; }
bool face_up(void)      { return visible && !moving && rising; }
bool face_busy(void)    { return moving || blink_phase != 0 || glance_phase != 0; }

void face_blink(bool twice) {
    if (!visible) return;
    blink_twice = twice;
    blink_phase = 1;
    blink_until = millis() + BLINK_MS;
    set_squint(true);
}

void face_set_resting(bool on) {
    resting = on;
    if (on) next_act_at = millis() + 800;
}

static void glance_exec(void* var, int32_t v) {
    (void)var;
    glance_cur = v;
    place_eyes(v);
}

static void glance_done(lv_anim_t* a) {
    (void)a;
    if (glance_phase == 1) {
        glance_phase = 2;
        glance_until = millis() + clawd_rng_range(&rng, 700, 1600);
    } else {
        glance_phase = 0;
    }
}

static void glance_to(int target) {
    lv_anim_delete(&glance_cur, NULL);
    lv_anim_t a;
    lv_anim_init(&a);
    lv_anim_set_var(&a, &glance_cur);
    lv_anim_set_exec_cb(&a, glance_exec);
    lv_anim_set_values(&a, glance_cur, target);
    lv_anim_set_duration(&a, GLANCE_MS);
    lv_anim_set_path_cb(&a, lv_anim_path_ease_out);
    lv_anim_set_completed_cb(&a, glance_done);
    lv_anim_start(&a);
}

void face_tick(void) {
    if (!root || !visible) return;
    const uint32_t now = millis();

    if (blink_phase && (int32_t)(now - blink_until) >= 0) {
        if (blink_phase == 1 && blink_twice) {
            blink_phase = 2; blink_until = now + BLINK_GAP; set_squint(false);
        } else if (blink_phase == 2) {
            blink_phase = 3; blink_until = now + BLINK_MS; set_squint(true);
        } else {
            blink_phase = 0; set_squint(false);
        }
    }
    if (glance_phase == 2 && (int32_t)(now - glance_until) >= 0) {
        glance_phase = 3;
        glance_to(0);
    }

    if (!resting || moving || blink_phase || glance_phase) return;
    if ((int32_t)(now - next_act_at) < 0) return;
    const FacePlan p = face_plan_next(&rng, &clawd_tuning());
    switch (p.act) {
    case FACE_BLINK:         face_blink(false); break;
    case FACE_DOUBLE_BLINK:  face_blink(true); break;
    case FACE_GLANCE_LEFT:   glance_phase = 1; glance_target = -glance_px; glance_to(glance_target); break;
    case FACE_GLANCE_RIGHT:  glance_phase = 1; glance_target = glance_px;  glance_to(glance_target); break;
    }
    next_act_at = now + p.gap_ms;
}
