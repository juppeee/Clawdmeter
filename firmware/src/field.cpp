#include "field.h"
#include <stdlib.h>

static bool area_empty(const lv_area_t* a) { return a->x2 < a->x1 || a->y2 < a->y1; }

static bool intersect(lv_area_t* r, const lv_area_t* a, const lv_area_t* b) {
    r->x1 = a->x1 > b->x1 ? a->x1 : b->x1;
    r->y1 = a->y1 > b->y1 ? a->y1 : b->y1;
    r->x2 = a->x2 < b->x2 ? a->x2 : b->x2;
    r->y2 = a->y2 < b->y2 ? a->y2 : b->y2;
    return !area_empty(r);
}

static int32_t bands_of(const FieldShape* s) {
    if (!s->lean || s->step <= 0) return 1;
    const int32_t n = (s->lean < 0 ? -s->lean : s->lean) / s->step;
    const int32_t bw = s->body.x2 - s->body.x1 + 1;
    return n + 1 < bw ? n + 1 : (bw > 0 ? bw : 1);
}

// Band i: columns [body.x1 + bw*i/k, body.x1 + bw*(i+1)/k - 1].
static int32_t band_x1(const FieldShape* s, int32_t k, int32_t i) {
    const int32_t bw = s->body.x2 - s->body.x1 + 1;
    return s->body.x1 + bw * i / k;
}

static int32_t band_top(const FieldShape* s, int32_t k, int32_t i) {
    if (k <= 1) return s->body.y1;
    const int32_t n = k - 1;
    const int32_t off = (2 * i - n) * s->step / 2;
    return s->body.y1 + (s->lean > 0 ? off : -off);
}

int32_t field_top_at(const FieldShape* s, int32_t x) {
    if (area_empty(&s->body) || x < s->body.x1 || x > s->body.x2) return FIELD_NO_TOP;
    const int32_t k = bands_of(s);
    const int32_t bw = s->body.x2 - s->body.x1 + 1;
    int32_t i = ((x - s->body.x1 + 1) * k - 1) / bw;
    if (i > k - 1) i = k - 1;
    return band_top(s, k, i);
}

void field_shape_init(FieldShape* s) {
    s->body.x1 = 0; s->body.y1 = 0; s->body.x2 = -1; s->body.y2 = -1;
    s->lean = 0;
    s->step = 1;
    for (int i = 0; i < FIELD_EXTRAS; i++) s->extra[i] = s->body;
}

static void draw_cb(lv_event_t* e) {
    const Field* f = (const Field*)lv_event_get_user_data(e);
    const FieldShape* s = &f->shape;
    lv_draw_rect_dsc_t d;
    lv_draw_rect_dsc_init(&d);
    d.bg_color = f->color;
    d.bg_opa = LV_OPA_COVER;
    d.radius = 0;
    lv_layer_t* layer = lv_event_get_layer(e);
    if (!area_empty(&s->body)) {
        const int32_t k = bands_of(s);
        for (int32_t i = 0; i < k; i++) {
            lv_area_t a = { band_x1(s, k, i), band_top(s, k, i),
                            band_x1(s, k, i + 1) - 1, s->body.y2 };
            if (!area_empty(&a)) lv_draw_rect(layer, &d, &a);
        }
    }
    for (int i = 0; i < FIELD_EXTRAS; i++)
        if (!area_empty(&s->extra[i])) lv_draw_rect(layer, &d, &s->extra[i]);
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

// Old XOR new of two rectangles: what was drawn and no longer is, and what
// is newly drawn. The overlap keeps its colour.
static void inv_xor(const Field* f, const lv_area_t* o, const lv_area_t* n) {
    if (o->x1 == n->x1 && o->y1 == n->y1 && o->x2 == n->x2 && o->y2 == n->y2) return;
    lv_area_t c;
    const bool on = !area_empty(o), nn = !area_empty(n);
    if (on && nn && intersect(&c, o, n)) {
        inv_minus(f, o, &c);
        inv_minus(f, n, &c);
    } else {
        if (on) inv(f, o->x1, o->y1, o->x2, o->y2);
        if (nn) inv(f, n->x1, n->y1, n->x2, n->y2);
    }
}

#define MAX_BODY_AREAS 8
#define MAX_STRIPS     64

static int64_t area_of(const lv_area_t* a) {
    return (int64_t)(a->x2 - a->x1 + 1) * (a->y2 - a->y1 + 1);
}

static lv_area_t join(const lv_area_t* a, const lv_area_t* b) {
    lv_area_t u = { LV_MIN(a->x1, b->x1), LV_MIN(a->y1, b->y1),
                    LV_MAX(a->x2, b->x2), LV_MAX(a->y2, b->y2) };
    return u;
}

static int cmp_i32(const void* a, const void* b) {
    const int32_t x = *(const int32_t*)a, y = *(const int32_t*)b;
    return x < y ? -1 : (x > y ? 1 : 0);
}

// The body's old XOR new, column interval by column interval: within an
// interval where neither staircase has a band edge, both tops are constant.
static void inv_body(const Field* f, const FieldShape* o, const FieldShape* n) {
    const int32_t W = lv_obj_get_width(f->obj);
    const int32_t H = lv_obj_get_height(f->obj);
    int32_t xs[2 * 64 + 8];
    int nx = 0;
    const FieldShape* sh[2] = { o, n };
    for (int s = 0; s < 2; s++) {
        if (area_empty(&sh[s]->body)) continue;
        const int32_t k = bands_of(sh[s]);
        for (int32_t i = 0; i <= k && nx < (int)(sizeof(xs) / sizeof(xs[0])) - 2; i++)
            xs[nx++] = band_x1(sh[s], k, i);
    }
    if (!nx) return;
    xs[nx++] = 0;
    xs[nx++] = W;
    qsort(xs, nx, sizeof(xs[0]), cmp_i32);

    lv_area_t r[MAX_STRIPS];
    int nr = 0;
    for (int i = 0; i + 1 < nx && nr < MAX_STRIPS - 2; i++) {
        const int32_t a = xs[i] < 0 ? 0 : xs[i];
        const int32_t b = (xs[i + 1] > W ? W : xs[i + 1]) - 1;
        if (b < a) continue;
        const int32_t to = field_top_at(o, a), tn = field_top_at(n, a);
        const int32_t bo = o->body.y2, bn = n->body.y2;
        int32_t y[4]; int ny = 0;           // up to two row ranges
        if (to == FIELD_NO_TOP && tn == FIELD_NO_TOP) continue;
        if (to == FIELD_NO_TOP)       { y[0] = tn; y[1] = bn; ny = 2; }
        else if (tn == FIELD_NO_TOP)  { y[0] = to; y[1] = bo; ny = 2; }
        else if (tn > bo || to > bn)  { y[0] = to; y[1] = bo; y[2] = tn; y[3] = bn; ny = 4; }
        else {
            if (to != tn) { y[ny++] = LV_MIN(to, tn); y[ny++] = LV_MAX(to, tn) - 1; }
            if (bo != bn) { y[ny++] = LV_MIN(bo, bn) + 1; y[ny++] = LV_MAX(bo, bn); }
        }
        for (int j = 0; j < ny; j += 2) {
            const int32_t y1 = y[j] < 0 ? 0 : y[j];
            const int32_t y2 = y[j + 1] > H - 1 ? H - 1 : y[j + 1];
            if (y2 < y1) continue;
            r[nr].x1 = a; r[nr].x2 = b; r[nr].y1 = y1; r[nr].y2 = y2;
            nr++;
        }
    }
    // Cheap joins first (neighbours whose box wastes little next to the
    // pixels that really changed), then the cheapest pairs until it fits the
    // budget.
    int64_t exact[MAX_STRIPS];
    for (int i = 0; i < nr; i++) exact[i] = area_of(&r[i]);
    for (int i = 0; i + 1 < nr;) {
        const lv_area_t u = join(&r[i], &r[i + 1]);
        const int64_t e = exact[i] + exact[i + 1];
        if (area_of(&u) * 4 <= e * 5 + 2048) {
            r[i] = u;
            exact[i] = e;
            for (int j = i + 1; j + 1 < nr; j++) { r[j] = r[j + 1]; exact[j] = exact[j + 1]; }
            nr--;
        } else {
            i++;
        }
    }
    while (nr > MAX_BODY_AREAS) {
        int best = 0;
        int64_t best_waste = INT64_MAX;
        for (int i = 0; i + 1 < nr; i++) {
            const lv_area_t u = join(&r[i], &r[i + 1]);
            const int64_t w = area_of(&u) - exact[i] - exact[i + 1];
            if (w < best_waste) { best_waste = w; best = i; }
        }
        r[best] = join(&r[best], &r[best + 1]);
        exact[best] += exact[best + 1];
        for (int j = best + 1; j + 1 < nr; j++) { r[j] = r[j + 1]; exact[j] = exact[j + 1]; }
        nr--;
    }
    for (int i = 0; i < nr; i++) inv(f, r[i].x1, r[i].y1, r[i].x2, r[i].y2);
}

void field_init(Field* f, lv_obj_t* parent, int32_t w, int32_t h, lv_color_t color) {
    f->color = color;
    field_shape_init(&f->shape);
    f->obj = lv_obj_create(parent);
    lv_obj_remove_style_all(f->obj);
    lv_obj_set_pos(f->obj, 0, 0);
    lv_obj_set_size(f->obj, w, h);
    lv_obj_clear_flag(f->obj, LV_OBJ_FLAG_CLICKABLE);
    lv_obj_clear_flag(f->obj, LV_OBJ_FLAG_SCROLLABLE);
    lv_obj_add_flag(f->obj, LV_OBJ_FLAG_HIDDEN);
    lv_obj_add_event_cb(f->obj, draw_cb, LV_EVENT_DRAW_MAIN, f);
}

static bool same_area(const lv_area_t* a, const lv_area_t* b) {
    if (area_empty(a) && area_empty(b)) return true;
    return a->x1 == b->x1 && a->y1 == b->y1 && a->x2 == b->x2 && a->y2 == b->y2;
}

void field_set_shape(Field* f, const FieldShape* s) {
    const FieldShape o = f->shape;
    FieldShape n = *s;
    if (n.step <= 0) n.step = 1;
    if (area_empty(&n.body)) n.lean = 0;
    f->shape = n;
    if (!same_area(&o.body, &n.body) || o.lean != n.lean || o.step != n.step) {
        if (!o.lean && !n.lean) inv_xor(f, &o.body, &n.body);   // flat: the exact XOR
        else                    inv_body(f, &o, &n);
    }
    for (int i = 0; i < FIELD_EXTRAS; i++)
        if (!same_area(&o.extra[i], &n.extra[i])) inv_xor(f, &o.extra[i], &n.extra[i]);
}

void field_set(Field* f, int32_t x1, int32_t y1, int32_t x2, int32_t y2) {
    FieldShape s;
    field_shape_init(&s);
    s.body.x1 = x1; s.body.y1 = y1; s.body.x2 = x2; s.body.y2 = y2;
    field_set_shape(f, &s);
}

void field_clear(Field* f) {
    FieldShape s;
    field_shape_init(&s);
    field_set_shape(f, &s);
}

bool field_empty(const Field* f) {
    if (!area_empty(&f->shape.body)) return false;
    for (int i = 0; i < FIELD_EXTRAS; i++)
        if (!area_empty(&f->shape.extra[i])) return false;
    return true;
}

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
