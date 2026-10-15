#pragma once
// Host stand-in for the few LVGL types and calls the director touches, for
// the director test (test/test_director).
#include <stdint.h>

typedef struct _lv_obj_t   lv_obj_t;
typedef struct _lv_indev_t lv_indev_t;
typedef struct _lv_event_t lv_event_t;
typedef struct _lv_display_t lv_display_t;
typedef struct { int32_t x, y; } lv_point_t;
typedef void (*lv_event_cb_t)(lv_event_t*);
enum { LV_EVENT_PRESSED = 1 };

static inline lv_indev_t* lv_indev_get_next(lv_indev_t*) { return (lv_indev_t*)0; }
static inline void lv_indev_add_event_cb(lv_indev_t*, lv_event_cb_t, int, void*) {}
static inline void lv_indev_get_point(lv_indev_t*, lv_point_t* p) { p->x = 0; p->y = 0; }
static inline int32_t lv_display_get_horizontal_resolution(lv_display_t*) { return 480; }
