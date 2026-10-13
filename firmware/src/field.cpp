#include "field.h"

static bool area_empty(const lv_area_t* a) { return a->x2 < a->x1 || a->y2 < a->y1; }

static bool intersect(lv_area_t* r, const lv_area_t* a, const lv_area_t* b) {
    r->x1 = a->x1 > b->x1 ? a->x1 : b->x1;
    r->y1 = a->y1 > b->y1 ? a->y1 : b->y1;
    r->x2 = a->x2 < b->x2 ? a->x2 : b->x2;
    r->y2 = a->y2 < b->y2 ? a->y2 : b->y2;
    return !area_empty(r);
}

static void draw_cb(lv_event_t* e) {
    const Field* f = (const Field*)lv_event_get_user_data(e);
    if (area_empty(&f->rect)) return;
    lv_draw_rect_dsc_t d;
    lv_draw_rect_dsc_init(&d);
    d.bg_color = f->color;
    d.bg_opa = LV_OPA_COVER;
    d.radius = 0;
    lv_draw_rect(lv_event_get_layer(e), &d, &f->rect);
}

static void inv(const Field* f, int32_t x1, int32_t y1, int32_t x2, int32_t y2) {
    if (x2 < x1 || y2 < y1) return;
    lv_area_t a = { x1, y1, x2, y2 };
    lv_obj_invalidate_area(f->obj, &a);      // clipped to the object (the screen)
}

// a minus c, where c lies inside a: up to four bands.
static void inv_minus(const Field* f, const lv_area_t* a, const lv_area_t* c) {
    inv(f, a->x1, a->y1, a->x2, c->y1 - 1);
    inv(f, a->x1, c->y2 + 1, a->x2, a->y2);
    inv(f, a->x1, c->y1, c->x1 - 1, c->y2);
    inv(f, c->x2 + 1, c->y1, a->x2, c->y2);
}

void field_init(Field* f, lv_obj_t* parent, int32_t w, int32_t h, lv_color_t color) {
    f->color = color;
    f->rect.x1 = 0; f->rect.y1 = 0; f->rect.x2 = -1; f->rect.y2 = -1;
    f->obj = lv_obj_create(parent);
    lv_obj_remove_style_all(f->obj);
    lv_obj_set_pos(f->obj, 0, 0);
    lv_obj_set_size(f->obj, w, h);
    lv_obj_clear_flag(f->obj, LV_OBJ_FLAG_CLICKABLE);
    lv_obj_clear_flag(f->obj, LV_OBJ_FLAG_SCROLLABLE);
    lv_obj_add_flag(f->obj, LV_OBJ_FLAG_HIDDEN);
    lv_obj_add_event_cb(f->obj, draw_cb, LV_EVENT_DRAW_MAIN, f);
}

void field_set(Field* f, int32_t x1, int32_t y1, int32_t x2, int32_t y2) {
    lv_area_t n = { x1, y1, x2, y2 };
    const lv_area_t o = f->rect;
    if (o.x1 == n.x1 && o.y1 == n.y1 && o.x2 == n.x2 && o.y2 == n.y2) return;
    f->rect = n;
    // Invalidate old XOR new: what was drawn and no longer is, and what is
    // newly drawn. The overlap keeps its colour.
    lv_area_t c;
    const bool on = !area_empty(&o), nn = !area_empty(&n);
    if (on && nn && intersect(&c, &o, &n)) {
        inv_minus(f, &o, &c);
        inv_minus(f, &n, &c);
    } else {
        if (on) inv(f, o.x1, o.y1, o.x2, o.y2);
        if (nn) inv(f, n.x1, n.y1, n.x2, n.y2);
    }
}

void field_clear(Field* f) { field_set(f, 0, 0, -1, -1); }

bool field_empty(const Field* f) { return area_empty(&f->rect); }

void field_quietly(lv_obj_t* any, void (*fn)(void* ctx), void* ctx) {
    lv_obj_update_layout(any);               // others' pending changes keep their redraw
    lv_display_t* d = lv_obj_get_display(any);
    lv_display_enable_invalidation(d, false);
    fn(ctx);
    lv_obj_update_layout(any);               // realise fn's layout changes while paused
    lv_display_enable_invalidation(d, true);
}

static void do_show(void* o)  { lv_obj_clear_flag((lv_obj_t*)o, LV_OBJ_FLAG_HIDDEN); }
static void do_hide(void* o)  { lv_obj_add_flag((lv_obj_t*)o, LV_OBJ_FLAG_HIDDEN); }
static void do_front(void* o) { lv_obj_move_foreground((lv_obj_t*)o); }

void field_show(Field* f) {
    if (!lv_obj_has_flag(f->obj, LV_OBJ_FLAG_HIDDEN)) return;
    field_quietly(f->obj, do_show, f->obj);
}

void field_hide(Field* f) {
    field_clear(f);
    if (lv_obj_has_flag(f->obj, LV_OBJ_FLAG_HIDDEN)) return;
    field_quietly(f->obj, do_hide, f->obj);
}

void field_to_front(Field* f) {
    lv_obj_t* p = lv_obj_get_parent(f->obj);
    if (lv_obj_get_index(f->obj) == (int32_t)lv_obj_get_child_count(p) - 1) return;
    field_quietly(f->obj, do_front, f->obj);
}
