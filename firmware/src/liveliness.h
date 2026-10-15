#pragma once

// How lively Clawd is (1 No-nonsense .. 5 All-nonsense; clawd_policy.h has
// what each level does). The app sends it in every data payload (`lv`); the
// last value is kept in NVS so it applies from boot, before the app connects.

void liveliness_init(void);       // after director_init(): load and apply the saved level
void liveliness_set(int level);   // apply; saved only when it changes
int  liveliness_get(void);
