#pragma once
#include "data.h"

void ui_init(void);
void ui_update(const UsageData* data);
void ui_tick_anim(void);
void ui_set_status(const char* text);
void ui_show_controller(void);
void ui_show_usage(void);
bool ui_is_controller_shown(void);
