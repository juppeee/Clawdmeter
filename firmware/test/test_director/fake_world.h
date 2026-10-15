#pragma once
// The director test's stand-ins for everything around the director: the
// actors (corner and full-size sprite, stage performer, face, streak) as
// small timed state machines with the real modules' contracts, and the
// situation the director reads (meters live, activity). Hooks come from
// the real signals module.
#include <stdint.h>

struct FakeWorld {
    bool meters_live;
    int  activity;
    bool asleep;
    bool has_big;          // PSRAM boards: lurk trips and the attention walk-on
};
extern FakeWorld world;

// Advance the actors that main.cpp ticks (stage, corner, streak) to now.
void fake_actors_tick(void);

// Who is on screen right now. The one-Clawd rule: at most one at a time.
struct OnScreen {
    bool corner;           // the corner sprite (seated, acting or walking)
    bool big;              // the full-size sprite (lurk trip, attention)
    bool performer;        // the stage performer
    bool face;             // the face, any part of it
};
OnScreen fake_on_screen(void);
int      fake_clawds(const OnScreen& s);
