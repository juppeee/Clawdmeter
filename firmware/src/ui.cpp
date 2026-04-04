#include "ui.h"
#include <lvgl.h>
#include "logo.h"
#include "icons.h"

// Custom fonts
LV_FONT_DECLARE(font_tiempos_34);
LV_FONT_DECLARE(font_styrene_28);
LV_FONT_DECLARE(font_styrene_16);
LV_FONT_DECLARE(font_styrene_14);
LV_FONT_DECLARE(font_styrene_12);
LV_FONT_DECLARE(font_mono_18);

// Anthropic brand palette
#define COL_BG        lv_color_hex(0x141413)
#define COL_PANEL     lv_color_hex(0x1f1f1e)
#define COL_TEXT      lv_color_hex(0xfaf9f5)
#define COL_DIM       lv_color_hex(0xb0aea5)
#define COL_ACCENT    lv_color_hex(0xd97757)
#define COL_GREEN     lv_color_hex(0x788c5d)
#define COL_AMBER     lv_color_hex(0xd97757)
#define COL_RED       lv_color_hex(0xc0392b)
#define COL_BAR_BG    lv_color_hex(0x2a2a28)
#define COL_ZONE_BG   lv_color_hex(0x252524)
#define COL_ZONE_BRD  lv_color_hex(0x3a3a38)

// ---- Usage screen widgets ----
static lv_obj_t* usage_container;
static lv_obj_t* lbl_title;
static lv_obj_t* bar_session;
static lv_obj_t* lbl_session_pct;
static lv_obj_t* lbl_session_label;
static lv_obj_t* lbl_session_reset;
static lv_obj_t* bar_weekly;
static lv_obj_t* lbl_weekly_pct;
static lv_obj_t* lbl_weekly_label;
static lv_obj_t* lbl_weekly_reset;
static lv_obj_t* lbl_anim;

// ---- Controller screen widgets ----
static lv_obj_t* ctrl_container;
static lv_obj_t* ctrl_session_bar;
static lv_obj_t* ctrl_session_lbl;
static lv_obj_t* ctrl_reset_lbl;

// ---- Bluetooth screen widgets ----
static lv_obj_t* ble_container;
static lv_obj_t* lbl_ble_status;
static lv_obj_t* lbl_ble_device;
static lv_obj_t* lbl_ble_mac;

// ---- Shared ----
static lv_image_dsc_t logo_dsc;
static screen_t current_screen = SCREEN_USAGE;

// Animation state
static uint32_t anim_last_ms = 0;
static uint8_t anim_spinner_idx = 0;
static uint8_t anim_msg_idx = 0;
static uint32_t anim_msg_start = 0;
#define ANIM_SPIN_MS    120
#define ANIM_MSG_MS     4000

static const char* const spinner_frames[] = {
    "\xC2\xB7", "\xE2\x9C\xBB", "\xE2\x9C\xBD",
    "\xE2\x9C\xB6", "\xE2\x9C\xB3", "\xE2\x9C\xA2",
};
#define SPINNER_COUNT 6

static const char* const anim_messages[] = {
    "Accomplishing", "Elucidating", "Perusing",
    "Actioning", "Enchanting", "Philosophising",
    "Actualizing", "Envisioning", "Pondering",
    "Baking", "Finagling", "Pontificating",
    "Booping", "Flibbertigibbeting", "Processing",
    "Brewing", "Forging", "Puttering",
    "Calculating", "Forming", "Puzzling",
    "Cerebrating", "Frolicking", "Reticulating",
    "Channelling", "Generating", "Ruminating",
    "Churning", "Germinating", "Scheming",
    "Clauding", "Hatching", "Schlepping",
    "Coalescing", "Herding", "Shimmying",
    "Cogitating", "Honking", "Shucking",
    "Combobulating", "Hustling", "Simmering",
    "Computing", "Ideating", "Smooshing",
    "Concocting", "Imagining", "Spelunking",
    "Conjuring", "Incubating", "Spinning",
    "Considering", "Inferring", "Stewing",
    "Contemplating", "Jiving", "Sussing",
    "Cooking", "Manifesting", "Synthesizing",
    "Crafting", "Marinating", "Thinking",
    "Creating", "Meandering", "Tinkering",
    "Crunching", "Moseying", "Transmuting",
    "Deciphering", "Mulling", "Unfurling",
    "Deliberating", "Mustering", "Unravelling",
    "Determining", "Musing", "Vibing",
    "Discombobulating", "Noodling", "Wandering",
    "Divining", "Percolating", "Whirring",
    "Doing", "Wibbling",
    "Effecting", "Wizarding",
    "Working", "Wrangling",
};
#define ANIM_MSG_COUNT (sizeof(anim_messages) / sizeof(anim_messages[0]))

static lv_color_t pct_color(float pct) {
    if (pct >= 80.0f) return COL_RED;
    if (pct >= 50.0f) return COL_AMBER;
    return COL_GREEN;
}

static void format_reset_time(int mins, char* buf, size_t len) {
    if (mins < 0) {
        snprintf(buf, len, "---");
    } else if (mins < 60) {
        snprintf(buf, len, "Resets in %dm", mins);
    } else if (mins < 1440) {
        snprintf(buf, len, "Resets in %dh %dm", mins / 60, mins % 60);
    } else {
        snprintf(buf, len, "Resets in %dd %dh", mins / 1440, (mins % 1440) / 60);
    }
}

static lv_obj_t* make_panel(lv_obj_t* parent, int x, int y, int w, int h) {
    lv_obj_t* panel = lv_obj_create(parent);
    lv_obj_set_pos(panel, x, y);
    lv_obj_set_size(panel, w, h);
    lv_obj_set_style_bg_color(panel, COL_PANEL, 0);
    lv_obj_set_style_bg_opa(panel, LV_OPA_COVER, 0);
    lv_obj_set_style_radius(panel, 8, 0);
    lv_obj_set_style_border_width(panel, 0, 0);
    lv_obj_set_style_pad_left(panel, 12, 0);
    lv_obj_set_style_pad_right(panel, 12, 0);
    lv_obj_set_style_pad_top(panel, 10, 0);
    lv_obj_set_style_pad_bottom(panel, 16, 0);
    lv_obj_clear_flag(panel, LV_OBJ_FLAG_SCROLLABLE);
    return panel;
}

static lv_obj_t* make_bar(lv_obj_t* parent, int x, int y, int w, int h) {
    lv_obj_t* bar = lv_bar_create(parent);
    lv_obj_set_pos(bar, x, y);
    lv_obj_set_size(bar, w, h);
    lv_bar_set_range(bar, 0, 100);
    lv_bar_set_value(bar, 0, LV_ANIM_OFF);
    lv_obj_set_style_bg_color(bar, COL_BAR_BG, LV_PART_MAIN);
    lv_obj_set_style_bg_opa(bar, LV_OPA_COVER, LV_PART_MAIN);
    lv_obj_set_style_radius(bar, 6, LV_PART_MAIN);
    lv_obj_set_style_bg_color(bar, COL_GREEN, LV_PART_INDICATOR);
    lv_obj_set_style_bg_opa(bar, LV_OPA_COVER, LV_PART_INDICATOR);
    lv_obj_set_style_radius(bar, 6, LV_PART_INDICATOR);
    return bar;
}

// Create a touch zone button on the controller screen
static lv_obj_t* make_zone(lv_obj_t* parent, int x, int y, int w, int h, const char* text) {
    lv_obj_t* zone = lv_obj_create(parent);
    lv_obj_set_pos(zone, x, y);
    lv_obj_set_size(zone, w, h);
    lv_obj_set_style_bg_color(zone, COL_PANEL, 0);
    lv_obj_set_style_bg_opa(zone, LV_OPA_COVER, 0);
    lv_obj_set_style_radius(zone, 8, 0);
    lv_obj_set_style_border_width(zone, 0, 0);
    lv_obj_clear_flag(zone, LV_OBJ_FLAG_SCROLLABLE);

    lv_obj_t* lbl = lv_label_create(zone);
    lv_label_set_text(lbl, text);
    lv_obj_set_style_text_font(lbl, &font_styrene_14, 0);
    lv_obj_set_style_text_color(lbl, COL_DIM, 0);
    lv_obj_center(lbl);

    return zone;
}

// Icon image descriptors (initialized in init_controller_screen)
static lv_image_dsc_t icon_escape_dsc;
static lv_image_dsc_t icon_delete_dsc;
static lv_image_dsc_t icon_arrow_left_dsc;
static lv_image_dsc_t icon_arrow_right_dsc;

static void init_icon_dsc(lv_image_dsc_t* dsc, int w, int h, const uint16_t* data) {
    dsc->header.w = w;
    dsc->header.h = h;
    dsc->header.cf = LV_COLOR_FORMAT_RGB565;
    dsc->header.stride = w * 2;
    dsc->data = (const uint8_t*)data;
    dsc->data_size = w * h * 2;
}

// Create a touch zone with an icon centered in it
static lv_obj_t* make_zone_icon(lv_obj_t* parent, int x, int y, int w, int h, lv_image_dsc_t* icon) {
    lv_obj_t* zone = lv_obj_create(parent);
    lv_obj_set_pos(zone, x, y);
    lv_obj_set_size(zone, w, h);
    lv_obj_set_style_bg_color(zone, COL_PANEL, 0);
    lv_obj_set_style_bg_opa(zone, LV_OPA_COVER, 0);
    lv_obj_set_style_radius(zone, 8, 0);
    lv_obj_set_style_border_width(zone, 0, 0);
    lv_obj_clear_flag(zone, LV_OBJ_FLAG_SCROLLABLE);

    lv_obj_t* img = lv_image_create(zone);
    lv_image_set_src(img, icon);
    lv_obj_center(img);

    return zone;
}

// Create a dim hint label for swipe directions
static lv_obj_t* make_hint(lv_obj_t* parent, int x, int y, const char* text) {
    lv_obj_t* lbl = lv_label_create(parent);
    lv_label_set_text(lbl, text);
    lv_obj_set_style_text_font(lbl, &font_styrene_14, 0);
    lv_obj_set_style_text_color(lbl, COL_DIM, 0);
    lv_obj_set_pos(lbl, x, y);
    return lbl;
}

static void init_usage_screen(lv_obj_t* scr) {
    usage_container = lv_obj_create(scr);
    lv_obj_set_size(usage_container, 480, 320);
    lv_obj_set_pos(usage_container, 0, 0);
    lv_obj_set_style_bg_opa(usage_container, LV_OPA_TRANSP, 0);
    lv_obj_set_style_border_width(usage_container, 0, 0);
    lv_obj_set_style_pad_all(usage_container, 0, 0);
    lv_obj_clear_flag(usage_container, LV_OBJ_FLAG_SCROLLABLE);

    // Title
    lbl_title = lv_label_create(usage_container);
    lv_label_set_text(lbl_title, "Claude Usage");
    lv_obj_set_style_text_font(lbl_title, &font_tiempos_34, 0);
    lv_obj_set_style_text_color(lbl_title, COL_TEXT, 0);
    lv_obj_set_pos(lbl_title, 66, 12);

    // Session panel
    lv_obj_t* p_session = make_panel(usage_container, 8, 56, 464, 100);

    lbl_session_pct = lv_label_create(p_session);
    lv_label_set_text(lbl_session_pct, "---%");
    lv_obj_set_style_text_font(lbl_session_pct, &font_styrene_28, 0);
    lv_obj_set_style_text_color(lbl_session_pct, COL_TEXT, 0);
    lv_obj_set_pos(lbl_session_pct, 0, 0);

    bar_session = make_bar(p_session, 0, 36, 440, 20);

    lbl_session_label = lv_label_create(p_session);
    lv_label_set_text(lbl_session_label, "Current Session");
    lv_obj_set_style_text_font(lbl_session_label, &font_styrene_16, 0);
    lv_obj_set_style_text_color(lbl_session_label, COL_DIM, 0);
    lv_obj_set_pos(lbl_session_label, 0, 62);

    lbl_session_reset = lv_label_create(p_session);
    lv_label_set_text(lbl_session_reset, "---");
    lv_obj_set_style_text_font(lbl_session_reset, &font_styrene_16, 0);
    lv_obj_set_style_text_color(lbl_session_reset, COL_DIM, 0);
    lv_obj_align(lbl_session_reset, LV_ALIGN_TOP_RIGHT, 0, 62);

    // Weekly panel
    lv_obj_t* p_weekly = make_panel(usage_container, 8, 166, 464, 100);

    lbl_weekly_pct = lv_label_create(p_weekly);
    lv_label_set_text(lbl_weekly_pct, "---%");
    lv_obj_set_style_text_font(lbl_weekly_pct, &font_styrene_28, 0);
    lv_obj_set_style_text_color(lbl_weekly_pct, COL_TEXT, 0);
    lv_obj_set_pos(lbl_weekly_pct, 0, 0);

    bar_weekly = make_bar(p_weekly, 0, 36, 440, 20);

    lbl_weekly_label = lv_label_create(p_weekly);
    lv_label_set_text(lbl_weekly_label, "Current Week");
    lv_obj_set_style_text_font(lbl_weekly_label, &font_styrene_16, 0);
    lv_obj_set_style_text_color(lbl_weekly_label, COL_DIM, 0);
    lv_obj_set_pos(lbl_weekly_label, 0, 62);

    lbl_weekly_reset = lv_label_create(p_weekly);
    lv_label_set_text(lbl_weekly_reset, "---");
    lv_obj_set_style_text_font(lbl_weekly_reset, &font_styrene_16, 0);
    lv_obj_set_style_text_color(lbl_weekly_reset, COL_DIM, 0);
    lv_obj_align(lbl_weekly_reset, LV_ALIGN_TOP_RIGHT, 0, 62);

    // Animation
    lbl_anim = lv_label_create(usage_container);
    lv_label_set_text(lbl_anim, "");
    lv_obj_set_style_text_font(lbl_anim, &font_mono_18, 0);
    lv_obj_set_style_text_color(lbl_anim, COL_ACCENT, 0);
    lv_obj_align(lbl_anim, LV_ALIGN_BOTTOM_MID, 0, -16);
}

static void init_controller_screen(lv_obj_t* scr) {
    ctrl_container = lv_obj_create(scr);
    lv_obj_set_size(ctrl_container, 480, 320);
    lv_obj_set_pos(ctrl_container, 0, 0);
    lv_obj_set_style_bg_opa(ctrl_container, LV_OPA_TRANSP, 0);
    lv_obj_set_style_border_width(ctrl_container, 0, 0);
    lv_obj_set_style_pad_all(ctrl_container, 0, 0);
    lv_obj_clear_flag(ctrl_container, LV_OBJ_FLAG_SCROLLABLE);

    // Title
    lv_obj_t* lbl_ctrl_title = lv_label_create(ctrl_container);
    lv_label_set_text(lbl_ctrl_title, "Claude Controller");
    lv_obj_set_style_text_font(lbl_ctrl_title, &font_tiempos_34, 0);
    lv_obj_set_style_text_color(lbl_ctrl_title, COL_TEXT, 0);
    lv_obj_set_pos(lbl_ctrl_title, 66, 12);

    // Shared layout math — matches usage screen geometry
    #define MARGIN      8
    #define CONTENT_Y   56
    #define PANEL_H     100
    #define GAP         10
    #define CONTENT_W   (480 - 2 * MARGIN)                    // 464
    #define CONTENT_BOT (CONTENT_Y + 2 * PANEL_H + GAP)      // 266
    #define CONTENT_H   (CONTENT_BOT - CONTENT_Y)             // 210
    #define SIDE_H      ((CONTENT_H - GAP) / 2)               // 100 (square)
    #define SIDE_W      SIDE_H

    // Initialize icon descriptors
    init_icon_dsc(&icon_escape_dsc, ICON_ESCAPE_W, ICON_ESCAPE_H, icon_escape_data);
    init_icon_dsc(&icon_delete_dsc, ICON_DELETE_W, ICON_DELETE_H, icon_delete_data);
    init_icon_dsc(&icon_arrow_left_dsc, ICON_ARROW_LEFT_W, ICON_ARROW_LEFT_H, icon_arrow_left_data);
    init_icon_dsc(&icon_arrow_right_dsc, ICON_ARROW_RIGHT_W, ICON_ARROW_RIGHT_H, icon_arrow_right_data);

    // Left column: ESC (top), < (bottom)
    make_zone_icon(ctrl_container, MARGIN, CONTENT_Y, SIDE_W, SIDE_H, &icon_escape_dsc);
    make_zone_icon(ctrl_container, MARGIN, CONTENT_Y + SIDE_H + GAP, SIDE_W, SIDE_H, &icon_arrow_left_dsc);

    // Right column: DEL (top), > (bottom)
    make_zone_icon(ctrl_container, 480 - MARGIN - SIDE_W, CONTENT_Y, SIDE_W, SIDE_H, &icon_delete_dsc);
    make_zone_icon(ctrl_container, 480 - MARGIN - SIDE_W, CONTENT_Y + SIDE_H + GAP, SIDE_W, SIDE_H, &icon_arrow_right_dsc);

    // Center hints box (full content height)
    int cx = MARGIN + SIDE_W + GAP;
    int cw = CONTENT_W - 2 * (SIDE_W + GAP);
    lv_obj_t* center = make_zone(ctrl_container, cx, CONTENT_Y, cw, CONTENT_H, "");

    // Vertically centered hint list
    lv_obj_t* hint_list = lv_obj_create(center);
    lv_obj_set_size(hint_list, cw - 24, LV_SIZE_CONTENT);
    lv_obj_center(hint_list);
    lv_obj_set_style_bg_opa(hint_list, LV_OPA_TRANSP, 0);
    lv_obj_set_style_border_width(hint_list, 0, 0);
    lv_obj_set_style_pad_all(hint_list, 0, 0);
    lv_obj_set_style_pad_row(hint_list, 6, 0);
    lv_obj_set_flex_flow(hint_list, LV_FLEX_FLOW_COLUMN);
    lv_obj_set_flex_align(hint_list, LV_FLEX_ALIGN_CENTER, LV_FLEX_ALIGN_CENTER, LV_FLEX_ALIGN_CENTER);
    lv_obj_clear_flag(hint_list, LV_OBJ_FLAG_SCROLLABLE);

    // Hand icon + "Gestures" header row
    static lv_image_dsc_t icon_hand_dsc;
    init_icon_dsc(&icon_hand_dsc, ICON_HAND_W, ICON_HAND_H, icon_hand_data);

    lv_obj_t* header_row = lv_obj_create(hint_list);
    lv_obj_set_width(header_row, lv_pct(100));
    lv_obj_set_height(header_row, LV_SIZE_CONTENT);
    lv_obj_set_style_bg_opa(header_row, LV_OPA_TRANSP, 0);
    lv_obj_set_style_border_width(header_row, 0, 0);
    lv_obj_set_style_pad_all(header_row, 0, 0);
    lv_obj_set_style_pad_column(header_row, 8, 0);
    lv_obj_set_flex_flow(header_row, LV_FLEX_FLOW_ROW);
    lv_obj_set_flex_align(header_row, LV_FLEX_ALIGN_CENTER, LV_FLEX_ALIGN_CENTER, LV_FLEX_ALIGN_CENTER);
    lv_obj_set_style_pad_bottom(header_row, 8, 0);
    lv_obj_clear_flag(header_row, LV_OBJ_FLAG_SCROLLABLE);

    lv_obj_t* hand_img = lv_image_create(header_row);
    lv_image_set_src(hand_img, &icon_hand_dsc);

    lv_obj_t* gestures_lbl = lv_label_create(header_row);
    lv_label_set_text(gestures_lbl, "Gestures");
    lv_obj_set_style_text_font(gestures_lbl, &font_styrene_16, 0);
    lv_obj_set_style_text_color(gestures_lbl, COL_TEXT, 0);

    // Hint entries
    make_hint(hint_list, 0, 0, "Swipe up/down: Arrow keys");
    make_hint(hint_list, 0, 0, "Swipe left: Toggle mode");
    make_hint(hint_list, 0, 0, "Swipe right: Enter key");
    make_hint(hint_list, 0, 0, "Hold: Voice dictation");

    // Usage info below content area: labels then bar
    int info_y = CONTENT_BOT + GAP;

    ctrl_session_lbl = lv_label_create(ctrl_container);
    lv_label_set_text(ctrl_session_lbl, "---% of current session");
    lv_obj_set_style_text_font(ctrl_session_lbl, &font_styrene_14, 0);
    lv_obj_set_style_text_color(ctrl_session_lbl, COL_DIM, 0);
    lv_obj_set_pos(ctrl_session_lbl, MARGIN, info_y);

    ctrl_reset_lbl = lv_label_create(ctrl_container);
    lv_label_set_text(ctrl_reset_lbl, "---");
    lv_obj_set_style_text_font(ctrl_reset_lbl, &font_styrene_14, 0);
    lv_obj_set_style_text_color(ctrl_reset_lbl, COL_DIM, 0);
    lv_obj_align(ctrl_reset_lbl, LV_ALIGN_TOP_RIGHT, -MARGIN, info_y);

    int bar_y = info_y + 18;
    ctrl_session_bar = make_bar(ctrl_container, MARGIN, bar_y, CONTENT_W, 16);

    // Start hidden
    lv_obj_add_flag(ctrl_container, LV_OBJ_FLAG_HIDDEN);
}

static void init_bluetooth_screen(lv_obj_t* scr) {
    ble_container = lv_obj_create(scr);
    lv_obj_set_size(ble_container, 480, 320);
    lv_obj_set_pos(ble_container, 0, 0);
    lv_obj_set_style_bg_opa(ble_container, LV_OPA_TRANSP, 0);
    lv_obj_set_style_border_width(ble_container, 0, 0);
    lv_obj_set_style_pad_all(ble_container, 0, 0);
    lv_obj_clear_flag(ble_container, LV_OBJ_FLAG_SCROLLABLE);

    // Title
    lv_obj_t* lbl_ble_title = lv_label_create(ble_container);
    lv_label_set_text(lbl_ble_title, "Bluetooth");
    lv_obj_set_style_text_font(lbl_ble_title, &font_tiempos_34, 0);
    lv_obj_set_style_text_color(lbl_ble_title, COL_TEXT, 0);
    lv_obj_set_pos(lbl_ble_title, 66, 12);

    // Info panel
    lv_obj_t* p_info = make_panel(ble_container, 8, 56, 464, 120);

    // Bluetooth icon + status row
    static lv_image_dsc_t icon_bt_dsc;
    init_icon_dsc(&icon_bt_dsc, ICON_BLUETOOTH_W, ICON_BLUETOOTH_H, icon_bluetooth_data);

    lv_obj_t* bt_img = lv_image_create(p_info);
    lv_image_set_src(bt_img, &icon_bt_dsc);
    lv_obj_set_pos(bt_img, 0, 0);

    lbl_ble_status = lv_label_create(p_info);
    lv_label_set_text(lbl_ble_status, "Initializing...");
    lv_obj_set_style_text_font(lbl_ble_status, &font_styrene_28, 0);
    lv_obj_set_style_text_color(lbl_ble_status, COL_DIM, 0);
    lv_obj_set_pos(lbl_ble_status, 40, 2);

    lbl_ble_device = lv_label_create(p_info);
    lv_label_set_text(lbl_ble_device, "Device: ---");
    lv_obj_set_style_text_font(lbl_ble_device, &font_styrene_16, 0);
    lv_obj_set_style_text_color(lbl_ble_device, COL_DIM, 0);
    lv_obj_set_pos(lbl_ble_device, 0, 50);

    lbl_ble_mac = lv_label_create(p_info);
    lv_label_set_text(lbl_ble_mac, "Address: ---");
    lv_obj_set_style_text_font(lbl_ble_mac, &font_styrene_16, 0);
    lv_obj_set_style_text_color(lbl_ble_mac, COL_DIM, 0);
    lv_obj_set_pos(lbl_ble_mac, 0, 74);

    // Reset Bluetooth tap zone with trash icon
    lv_obj_t* reset_zone = lv_obj_create(ble_container);
    lv_obj_set_pos(reset_zone, 8, 190);
    lv_obj_set_size(reset_zone, 464, 60);
    lv_obj_set_style_bg_color(reset_zone, COL_PANEL, 0);
    lv_obj_set_style_bg_opa(reset_zone, LV_OPA_COVER, 0);
    lv_obj_set_style_radius(reset_zone, 8, 0);
    lv_obj_set_style_border_width(reset_zone, 0, 0);
    lv_obj_set_style_pad_column(reset_zone, 10, 0);
    lv_obj_set_flex_flow(reset_zone, LV_FLEX_FLOW_ROW);
    lv_obj_set_flex_align(reset_zone, LV_FLEX_ALIGN_CENTER, LV_FLEX_ALIGN_CENTER, LV_FLEX_ALIGN_CENTER);
    lv_obj_clear_flag(reset_zone, LV_OBJ_FLAG_SCROLLABLE);

    static lv_image_dsc_t icon_trash_dsc;
    init_icon_dsc(&icon_trash_dsc, ICON_TRASH2_W, ICON_TRASH2_H, icon_trash2_data);
    lv_obj_t* trash_img = lv_image_create(reset_zone);
    lv_image_set_src(trash_img, &icon_trash_dsc);

    lv_obj_t* reset_lbl = lv_label_create(reset_zone);
    lv_label_set_text(reset_lbl, "Reset Bluetooth");
    lv_obj_set_style_text_font(reset_lbl, &font_styrene_16, 0);
    lv_obj_set_style_text_color(reset_lbl, COL_DIM, 0);

    // Footer
    lv_obj_t* lbl_footer = lv_label_create(ble_container);
    lv_label_set_text(lbl_footer, "Bluetooth Low Energy");
    lv_obj_set_style_text_font(lbl_footer, &font_styrene_14, 0);
    lv_obj_set_style_text_color(lbl_footer, COL_DIM, 0);
    lv_obj_align(lbl_footer, LV_ALIGN_BOTTOM_MID, 0, -16);

    // Start hidden
    lv_obj_add_flag(ble_container, LV_OBJ_FLAG_HIDDEN);
}

void ui_init(void) {
    lv_obj_t* scr = lv_screen_active();
    lv_obj_set_style_bg_color(scr, COL_BG, 0);
    lv_obj_set_style_bg_opa(scr, LV_OPA_COVER, 0);

    // Logo (shared, always visible, on top of both containers)
    logo_dsc.header.w = LOGO_WIDTH;
    logo_dsc.header.h = LOGO_HEIGHT;
    logo_dsc.header.cf = LV_COLOR_FORMAT_RGB565;
    logo_dsc.header.stride = LOGO_WIDTH * 2;
    logo_dsc.data = (const uint8_t*)logo_data;
    logo_dsc.data_size = LOGO_WIDTH * LOGO_HEIGHT * 2;

    init_usage_screen(scr);
    init_controller_screen(scr);
    init_bluetooth_screen(scr);

    // Logo on top of all containers
    lv_obj_t* img = lv_image_create(scr);
    lv_image_set_src(img, &logo_dsc);
    lv_obj_set_pos(img, 10, 4);
}

void ui_update(const UsageData* data) {
    if (!data->valid) return;

    int s_pct = (int)(data->session_pct + 0.5f);

    // Usage screen
    lv_label_set_text_fmt(lbl_session_pct, "%d%%", s_pct);
    lv_bar_set_value(bar_session, s_pct, LV_ANIM_ON);
    lv_obj_set_style_bg_color(bar_session, pct_color(data->session_pct), LV_PART_INDICATOR);

    char buf[48];
    format_reset_time(data->session_reset_mins, buf, sizeof(buf));
    lv_label_set_text(lbl_session_reset, buf);

    int w_pct = (int)(data->weekly_pct + 0.5f);
    lv_label_set_text_fmt(lbl_weekly_pct, "%d%%", w_pct);
    lv_bar_set_value(bar_weekly, w_pct, LV_ANIM_ON);
    lv_obj_set_style_bg_color(bar_weekly, pct_color(data->weekly_pct), LV_PART_INDICATOR);

    format_reset_time(data->weekly_reset_mins, buf, sizeof(buf));
    lv_label_set_text(lbl_weekly_reset, buf);

    // Controller screen session info
    lv_bar_set_value(ctrl_session_bar, s_pct, LV_ANIM_ON);
    lv_obj_set_style_bg_color(ctrl_session_bar, pct_color(data->session_pct), LV_PART_INDICATOR);
    lv_label_set_text_fmt(ctrl_session_lbl, "%d%% of current session", s_pct);

    char rbuf[48];
    format_reset_time(data->session_reset_mins, rbuf, sizeof(rbuf));
    lv_label_set_text(ctrl_reset_lbl, rbuf);
}

void ui_tick_anim(void) {
    if (current_screen != SCREEN_USAGE) return;  // animation only on usage screen

    uint32_t now = lv_tick_get();

    if (now - anim_msg_start >= ANIM_MSG_MS) {
        anim_msg_idx = (anim_msg_idx + 1) % ANIM_MSG_COUNT;
        anim_msg_start = now;
    }

    if (now - anim_last_ms >= ANIM_SPIN_MS) {
        anim_last_ms = now;
        anim_spinner_idx = (anim_spinner_idx + 1) % SPINNER_COUNT;

        static char buf[80];
        snprintf(buf, sizeof(buf), "%s %s\xE2\x80\xA6",
                 spinner_frames[anim_spinner_idx],
                 anim_messages[anim_msg_idx]);
        lv_label_set_text(lbl_anim, buf);
    }
}

void ui_show_screen(screen_t screen) {
    lv_obj_add_flag(usage_container, LV_OBJ_FLAG_HIDDEN);
    lv_obj_add_flag(ctrl_container, LV_OBJ_FLAG_HIDDEN);
    lv_obj_add_flag(ble_container, LV_OBJ_FLAG_HIDDEN);

    switch (screen) {
    case SCREEN_USAGE:      lv_obj_clear_flag(usage_container, LV_OBJ_FLAG_HIDDEN); break;
    case SCREEN_CONTROLLER: lv_obj_clear_flag(ctrl_container, LV_OBJ_FLAG_HIDDEN); break;
    case SCREEN_BLUETOOTH:  lv_obj_clear_flag(ble_container, LV_OBJ_FLAG_HIDDEN); break;
    default: break;
    }
    current_screen = screen;
}

void ui_cycle_screen(void) {
    ui_show_screen((screen_t)((current_screen + 1) % SCREEN_COUNT));
}

screen_t ui_get_current_screen(void) {
    return current_screen;
}

void ui_update_ble_status(ble_state_t state, const char* name, const char* mac) {
    switch (state) {
    case BLE_STATE_CONNECTED:
        lv_label_set_text(lbl_ble_status, "Connected");
        lv_obj_set_style_text_color(lbl_ble_status, COL_GREEN, 0);
        break;
    case BLE_STATE_ADVERTISING:
        lv_label_set_text(lbl_ble_status, "Advertising...");
        lv_obj_set_style_text_color(lbl_ble_status, COL_AMBER, 0);
        break;
    case BLE_STATE_DISCONNECTED:
        lv_label_set_text(lbl_ble_status, "Disconnected");
        lv_obj_set_style_text_color(lbl_ble_status, COL_RED, 0);
        break;
    default:
        lv_label_set_text(lbl_ble_status, "Initializing...");
        lv_obj_set_style_text_color(lbl_ble_status, COL_DIM, 0);
        break;
    }

    if (name) {
        static char nbuf[48];
        snprintf(nbuf, sizeof(nbuf), "Device: %s", name);
        lv_label_set_text(lbl_ble_device, nbuf);
    }
    if (mac) {
        static char mbuf[48];
        snprintf(mbuf, sizeof(mbuf), "Address: %s", mac);
        lv_label_set_text(lbl_ble_mac, mbuf);
    }
}
