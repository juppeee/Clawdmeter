#include "charge_anim.h"
#include "splash.h"
#include "theme.h"
#include "icons.h"
#include "hal/board_caps.h"

// ---------------------------------------------------------------------------
// Geometry
//
// The drawing is described in the units of the original sketch: a 60x32 battery
// body with a 2-unit outline, a fill inset 3 units all round, and the bolt
// centred on top. Everything below scales from body_w, so one set of numbers
// serves every panel size.
//
// The pop at the start is a size change, not a transform. LVGL would have to
// composite a whole layer to scale or rotate a widget, which costs real memory
// on the PSRAM-free boards — growing the box and re-centring it is free and
// reads the same from a desk away. The small tilt in the sketch is dropped for
// the same reason; there is no cheap way to rotate a rounded rectangle here.
// ---------------------------------------------------------------------------

#define UNIT_W        60
#define UNIT_H        32
#define UNIT_BORDER    2
#define UNIT_RADIUS    7
#define UNIT_INSET     3
#define UNIT_FILL_W   54
#define UNIT_FILL_H   26
#define UNIT_FILL_R    4

#define SCALE_ONE   1000   // fixed-point 1.0 for the pop
#define FILL_FULL   1000   // fixed-point 1.0 for the fill sweep

static const lv_color_t COL_SPENT = LV_COLOR_MAKE(0x55, 0x55, 0x55);

static lv_obj_t      *root  = nullptr;
static lv_obj_t      *body  = nullptr;
static lv_obj_t      *fill  = nullptr;
static lv_obj_t      *bolt  = nullptr;
static lv_image_dsc_t bolt_dsc;
static lv_timer_t    *seq_timer = nullptr;

static int16_t base_w = 0, base_h = 0;   // body size at scale 1

// Animated state. Every animation writes one of these and re-runs relayout(),
// so overlapping steps compose instead of fighting over widget properties.
static int32_t s_scale = SCALE_ONE;
static int32_t s_fill  = 0;
static int32_t s_shift = 0;   // px, the unplug wobble
static int32_t s_spent = 0;   // 0..255, how far the colours have gone grey

static bool s_active = false;

// ---------------------------------------------------------------------------

static void relayout(void) {
    if (!body) return;

    const int w = (int)((int32_t)base_w * s_scale / SCALE_ONE);
    const int h = (int)((int32_t)base_h * s_scale / SCALE_ONE);

    lv_obj_set_size(body, w, h);
    lv_obj_align(body, LV_ALIGN_CENTER, (int16_t)s_shift, 0);
    lv_obj_set_style_border_width(body, LV_MAX(2, w * UNIT_BORDER / UNIT_W), 0);
    lv_obj_set_style_radius(body, w * UNIT_RADIUS / UNIT_W, 0);
    lv_obj_set_style_border_color(body, lv_color_mix(COL_SPENT, THEME_TEXT,
                                                     (uint8_t)s_spent), 0);

    const int fh     = h * UNIT_FILL_H / UNIT_H;
    const int fw_max = w * UNIT_FILL_W / UNIT_W;
    const int fw     = (int)((int32_t)fw_max * s_fill / FILL_FULL);

    if (fw > 0) {
        lv_obj_clear_flag(fill, LV_OBJ_FLAG_HIDDEN);
        lv_obj_set_size(fill, fw, fh);
        // The inset is measured from the outer edge, and LVGL already places
        // children inside the border, so only the remainder is ours to add.
        lv_obj_align(fill, LV_ALIGN_LEFT_MID,
                     w * (UNIT_INSET - UNIT_BORDER) / UNIT_W, 0);
        lv_obj_set_style_radius(fill, fh * UNIT_FILL_R / UNIT_FILL_H, 0);
        lv_obj_set_style_bg_color(fill, lv_color_mix(COL_SPENT, THEME_ACCENT,
                                                     (uint8_t)s_spent), 0);
    } else {
        lv_obj_add_flag(fill, LV_OBJ_FLAG_HIDDEN);
    }

    lv_obj_align_to(bolt, body, LV_ALIGN_CENTER, 0, 0);
}

// ---------------------------------------------------------------------------
// Animation plumbing
// ---------------------------------------------------------------------------

static void exec_state(void *var, int32_t v) {
    *(int32_t *)var = v;
    relayout();
}

static void exec_bolt_opa(void *, int32_t v) {
    lv_obj_set_style_opa(bolt, (lv_opa_t)v, 0);
}

static void exec_root_opa(void *, int32_t v) {
    lv_obj_set_style_opa(root, (lv_opa_t)v, 0);
}

static void on_faded_out(lv_anim_t *) {
    lv_obj_add_flag(root, LV_OBJ_FLAG_HIDDEN);
    s_active = false;
    // The splash only repaints cells that changed, so whatever this overlay
    // covered would stay black until the creature happens to move there.
    splash_request_full_redraw();
}

// Animate a state field from wherever it is now to `to`. Reading the live
// value matters: steps overlap by design (the fill starts before the pop has
// settled), and LVGL drops an earlier animation on the same field when a new
// one starts, so a hard-coded start value would snap.
static void tween(int32_t *field, int32_t to, uint32_t ms,
                  lv_anim_path_cb_t path) {
    lv_anim_t a;
    lv_anim_init(&a);
    lv_anim_set_var(&a, field);
    lv_anim_set_exec_cb(&a, exec_state);
    lv_anim_set_values(&a, *field, to);
    lv_anim_set_duration(&a, ms);
    lv_anim_set_path_cb(&a, path);
    lv_anim_start(&a);
}

static void tween_bolt(int32_t to, uint32_t ms) {
    lv_anim_t a;
    lv_anim_init(&a);
    lv_anim_set_var(&a, bolt);
    lv_anim_set_exec_cb(&a, exec_bolt_opa);
    lv_anim_set_values(&a, lv_obj_get_style_opa(bolt, LV_PART_MAIN), to);
    lv_anim_set_duration(&a, ms);
    lv_anim_set_path_cb(&a, lv_anim_path_ease_in_out);
    lv_anim_start(&a);
}

static void fade_out_and_hide(uint32_t ms) {
    lv_anim_t a;
    lv_anim_init(&a);
    lv_anim_set_var(&a, root);
    lv_anim_set_exec_cb(&a, exec_root_opa);
    lv_anim_set_values(&a, lv_obj_get_style_opa(root, LV_PART_MAIN), LV_OPA_TRANSP);
    lv_anim_set_duration(&a, ms);
    lv_anim_set_completed_cb(&a, on_faded_out);
    lv_anim_start(&a);
}

// ---------------------------------------------------------------------------
// The two sequences
//
// Times are milliseconds from the start of the sequence and come straight from
// the design sketch. A step table rather than nested callbacks, so the timing
// stays readable as a column of numbers.
// ---------------------------------------------------------------------------

struct Step {
    uint32_t at_ms;
    void   (*run)(void);
};

static void plug_pop_out(void)   { tween(&s_scale, 1180, 350, lv_anim_path_overshoot); }
static void plug_bolt_in(void)   { tween_bolt(LV_OPA_COVER, 150); }
static void plug_pop_back(void)  { tween(&s_scale, SCALE_ONE, 350, lv_anim_path_ease_out); }
static void plug_fill(void)      { tween(&s_fill, FILL_FULL, 700, lv_anim_path_ease_out); }
static void plug_bolt_dip(void)  { tween_bolt(38, 400); }   // 0.15 opacity
static void plug_bolt_back(void) { tween_bolt(LV_OPA_COVER, 300); }
static void plug_bolt_off(void)  { tween_bolt(LV_OPA_TRANSP, 500); }
static void plug_done(void)      { fade_out_and_hide(300); }

static const Step PLUG_IN[] = {
    {  250, plug_pop_out   },
    {  250, plug_bolt_in   },
    {  480, plug_pop_back  },
    {  520, plug_fill      },
    { 1300, plug_bolt_dip  },
    { 1550, plug_bolt_back },
    { 1900, plug_bolt_off  },
    { 2450, plug_done      },
};

static void unplug_nudge_r(void)  { tween(&s_shift,  4, 130, lv_anim_path_ease_out); }
static void unplug_nudge_l(void)  { tween(&s_shift, -3, 120, lv_anim_path_ease_out); }
static void unplug_settle(void)   { tween(&s_shift,  0, 120, lv_anim_path_ease_out); }
static void unplug_bolt_off(void) { tween_bolt(LV_OPA_TRANSP, 250); }
static void unplug_grey(void)     { tween(&s_spent, 255, 500, lv_anim_path_linear); }
static void unplug_dim(void)      {
    lv_anim_t a;
    lv_anim_init(&a);
    lv_anim_set_var(&a, root);
    lv_anim_set_exec_cb(&a, exec_root_opa);
    lv_anim_set_values(&a, LV_OPA_COVER, 90);   // the sketch's 0.35
    lv_anim_set_duration(&a, 600);
    lv_anim_start(&a);
}
static void unplug_done(void)     { fade_out_and_hide(400); }

static const Step UNPLUG[] = {
    {  150, unplug_nudge_r  },
    {  280, unplug_nudge_l  },
    {  400, unplug_settle   },
    {  400, unplug_bolt_off },
    {  550, unplug_grey     },
    { 1200, unplug_dim      },
    { 1900, unplug_done     },
};

static const Step *seq       = nullptr;
static uint8_t     seq_len   = 0;
static uint8_t     seq_next  = 0;
static uint32_t    seq_start = 0;

static void seq_tick(lv_timer_t *) {
    const uint32_t t = lv_tick_elaps(seq_start);
    while (seq_next < seq_len && seq[seq_next].at_ms <= t) {
        seq[seq_next].run();
        seq_next++;
    }
    if (seq_next >= seq_len) lv_timer_pause(seq_timer);
}

// ---------------------------------------------------------------------------
// Public
// ---------------------------------------------------------------------------

void charge_anim_init(lv_obj_t *parent) {
    const BoardCaps &caps = board_caps();
    const int16_t shortest = LV_MIN(caps.width, caps.height);

    base_w = (int16_t)(shortest * 62 / 100);
    base_h = (int16_t)((int32_t)base_w * UNIT_H / UNIT_W);

    root = lv_obj_create(parent);
    lv_obj_set_size(root, caps.width, caps.height);
    lv_obj_set_pos(root, 0, 0);
    lv_obj_set_style_bg_color(root, THEME_BG, 0);
    lv_obj_set_style_bg_opa(root, LV_OPA_COVER, 0);
    lv_obj_set_style_border_width(root, 0, 0);
    lv_obj_set_style_pad_all(root, 0, 0);
    lv_obj_clear_flag(root, LV_OBJ_FLAG_SCROLLABLE);
    // Decorative only — taps belong to whatever is underneath.
    lv_obj_clear_flag(root, LV_OBJ_FLAG_CLICKABLE);
    lv_obj_add_flag(root, LV_OBJ_FLAG_HIDDEN);

    body = lv_obj_create(root);
    lv_obj_set_style_bg_opa(body, LV_OPA_TRANSP, 0);
    lv_obj_set_style_pad_all(body, 0, 0);
    lv_obj_clear_flag(body, LV_OBJ_FLAG_SCROLLABLE);
    lv_obj_clear_flag(body, LV_OBJ_FLAG_CLICKABLE);

    fill = lv_obj_create(body);
    lv_obj_set_style_bg_opa(fill, LV_OPA_COVER, 0);
    lv_obj_set_style_border_width(fill, 0, 0);
    lv_obj_set_style_pad_all(fill, 0, 0);
    lv_obj_clear_flag(fill, LV_OBJ_FLAG_SCROLLABLE);
    lv_obj_clear_flag(fill, LV_OBJ_FLAG_CLICKABLE);

    if (shortest >= 460) {
        bolt_dsc.header.w = ICON_BOLT_W;
        bolt_dsc.header.h = ICON_BOLT_H;
        bolt_dsc.data     = icon_bolt_data;
    } else {
        bolt_dsc.header.w = ICON_BOLT_SMALL_W;
        bolt_dsc.header.h = ICON_BOLT_SMALL_H;
        bolt_dsc.data     = icon_bolt_small_data;
    }
    bolt_dsc.header.cf     = LV_COLOR_FORMAT_RGB565A8;
    bolt_dsc.header.stride = bolt_dsc.header.w * 2;
    bolt_dsc.data_size     = bolt_dsc.header.w * bolt_dsc.header.h * 3;

    bolt = lv_image_create(root);
    lv_image_set_src(bolt, &bolt_dsc);
    lv_obj_set_style_opa(bolt, LV_OPA_TRANSP, 0);

    relayout();
}

void charge_anim_play(bool plugged_in) {
    if (!root) return;

    // Restart from a known state — a cable that bounces must not stack runs.
    lv_anim_delete(&s_scale, nullptr);
    lv_anim_delete(&s_fill,  nullptr);
    lv_anim_delete(&s_shift, nullptr);
    lv_anim_delete(&s_spent, nullptr);
    lv_anim_delete(bolt,     nullptr);
    lv_anim_delete(root,     nullptr);

    s_scale = SCALE_ONE;
    s_shift = 0;
    s_spent = 0;
    // Plugging in fills from empty; pulling the cable starts from a full
    // battery, because that is what the picture has to say goodbye to.
    s_fill  = plugged_in ? 0 : FILL_FULL;

    lv_obj_set_style_opa(bolt, plugged_in ? LV_OPA_TRANSP : LV_OPA_COVER, 0);
    lv_obj_set_style_opa(root, LV_OPA_COVER, 0);
    relayout();

    lv_obj_clear_flag(root, LV_OBJ_FLAG_HIDDEN);
    lv_obj_move_foreground(root);
    s_active = true;

    seq       = plugged_in ? PLUG_IN : UNPLUG;
    seq_len   = plugged_in ? (uint8_t)(sizeof(PLUG_IN) / sizeof(PLUG_IN[0]))
                           : (uint8_t)(sizeof(UNPLUG) / sizeof(UNPLUG[0]));
    seq_next  = 0;
    seq_start = lv_tick_get();

    if (!seq_timer) seq_timer = lv_timer_create(seq_tick, 20, nullptr);
    lv_timer_resume(seq_timer);
}

bool charge_anim_is_active(void) {
    return s_active;
}
