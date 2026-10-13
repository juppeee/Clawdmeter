#include "streak.h"
#include "theme.h"
#include "hal/board_caps.h"

#define STREAK_MS 380

static lv_obj_t* band = NULL;
static lv_obj_t* cover_obj = NULL;
static int W = 480, H = 480, band_w = 150;
static bool running = false;

void streak_init(void) {
    const BoardCaps& c = board_caps();
    W = c.width; H = c.height;
    const int mind = W < H ? W : H;
    band_w = 150 * mind / 480;
    band = lv_obj_create(lv_layer_top());
    lv_obj_remove_style_all(band);
    lv_obj_set_size(band, band_w, H);
    lv_obj_set_pos(band, W, 0);
    lv_obj_set_style_bg_color(band, THEME_ACCENT, 0);
    lv_obj_set_style_bg_opa(band, LV_OPA_COVER, 0);
    lv_obj_clear_flag(band, LV_OBJ_FLAG_CLICKABLE);
    lv_obj_clear_flag(band, LV_OBJ_FLAG_SCROLLABLE);
    lv_obj_add_flag(band, LV_OBJ_FLAG_HIDDEN);
}

static void x_exec(void* var, int32_t x) {
    lv_obj_set_x((lv_obj_t*)var, x);
    // Stage stays left of the band's middle; the meters are revealed right
    // of it. The band covers the seam.
    if (cover_obj) {
        int w = x + band_w / 2;
        if (w < 0) w = 0;
        if (w > W) w = W;
        lv_obj_set_width(cover_obj, w);
    }
}

static void done(lv_anim_t* a) {
    (void)a;
    running = false;
    lv_obj_add_flag(band, LV_OBJ_FLAG_HIDDEN);
}

void streak_run(lv_obj_t* cover) {
    if (!band) return;
    cover_obj = cover;
    lv_anim_delete(band, x_exec);
    lv_obj_clear_flag(band, LV_OBJ_FLAG_HIDDEN);
    lv_obj_move_foreground(band);
    running = true;
    lv_anim_t a;
    lv_anim_init(&a);
    lv_anim_set_var(&a, band);
    lv_anim_set_exec_cb(&a, x_exec);
    lv_anim_set_values(&a, W, -band_w);
    lv_anim_set_duration(&a, STREAK_MS);
    lv_anim_set_path_cb(&a, lv_anim_path_ease_in_out);
    lv_anim_set_completed_cb(&a, done);
    lv_anim_start(&a);
}

bool streak_running(void) { return running; }

void streak_cancel(void) {
    if (!band) return;
    lv_anim_delete(band, x_exec);
    running = false;
    lv_obj_add_flag(band, LV_OBJ_FLAG_HIDDEN);
    if (cover_obj) lv_obj_set_width(cover_obj, W);
    cover_obj = NULL;
}
