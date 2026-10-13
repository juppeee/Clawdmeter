// Host unit test for face_motion — the face's rise and drop as a pure
// function of time (docs/clawd-director.md, "The face's motion"):
//
//   pio test -d firmware -e native -f test_face_motion

#include "face_motion.h"
#include <unity.h>
#include <stdlib.h>

#define CHECK(cond) TEST_ASSERT_TRUE(cond)

void setUp(void) {}
void tearDown(void) {}

static FaceGeom geom(int w, int h, int dir, int start_y) {
    FaceGeom g;
    g.w = w; g.h = h;
    g.mind = w < h ? w : h;
    g.cell = g.mind / 60;
    g.start_y = start_y;
    g.dir = dir;
    return g;
}

static const int SIZES[][2] = { { 480, 480 }, { 368, 448 }, { 410, 502 }, { 240, 240 } };

static void rise_starts_below_and_ends_dead_still_at_rest() {
    for (const auto& s : SIZES) {
        FaceGeom g = geom(s[0], s[1], 1, s[1]);
        const FacePose p0 = face_motion_pose(FACE_MOVE_RISE, 0, &g);
        CHECK(p0.y == s[1]);
        const FacePose e = face_motion_pose(FACE_MOVE_RISE, face_motion_duration(FACE_MOVE_RISE), &g);
        CHECK(e.y == 0 && e.lean == 0 && e.x == 0 && e.margin == 0);
        CHECK(e.eye_dy == 0 && e.arm_dy == 0 && e.stretch == 0);
    }
}

static void rise_overshoots_then_dips_then_settles() {
    FaceGeom g = geom(480, 480, 1, 480);
    int min_y = 0, max_after = 0;
    for (uint32_t t = 0; t <= face_motion_duration(FACE_MOVE_RISE); t += 4) {
        const FacePose p = face_motion_pose(FACE_MOVE_RISE, t, &g);
        if (p.y < min_y) min_y = p.y;
        if (t > 520 && p.y > max_after) max_after = p.y;
    }
    CHECK(min_y <= -30 && min_y >= -60);     // past rest, not wildly
    CHECK(max_after >= 6 && max_after <= 20); // a small dip back
    // First reaches rest at about the old 600 ms rise.
    CHECK(face_motion_rest_ms() >= 560 && face_motion_rest_ms() <= 620);
    CHECK(face_motion_pose(FACE_MOVE_RISE, face_motion_rest_ms() - 20, &g).y < 0);
    CHECK(face_motion_pose(FACE_MOVE_RISE, face_motion_rest_ms() + 20, &g).y > 0);
}

static void lean_is_whole_cells_toward_dir_and_wobbles_back() {
    for (int dir = -1; dir <= 1; dir += 2) {
        FaceGeom g = geom(480, 480, dir, 480);
        bool counter = false;
        for (uint32_t t = 0; t <= face_motion_duration(FACE_MOVE_RISE); t += 4) {
            const FacePose p = face_motion_pose(FACE_MOVE_RISE, t, &g);
            CHECK(p.lean % g.cell == 0);
            if (t < 300) CHECK(p.lean * dir > 0);
            if (p.lean * dir < 0) counter = true;
        }
        CHECK(counter);                      // one wobble past level
    }
}

static void arms_show_while_moving_and_leave_at_rest() {
    FaceGeom g = geom(480, 480, 1, 480);
    CHECK(face_motion_pose(FACE_MOVE_RISE, 200, &g).margin == face_arm_w(&g));
    CHECK(face_motion_pose(FACE_MOVE_RISE, 700, &g).margin == 0);
    CHECK(face_motion_pose(FACE_MOVE_DROP, 300, &g).margin == face_arm_w(&g));
    CHECK(face_arm_w(&g) % g.cell == 0 && face_arm_h(&g) % g.cell == 0);
}

static void eyes_stretch_rising_and_squash_on_landing() {
    FaceGeom g = geom(480, 480, 1, 480);
    CHECK(face_motion_pose(FACE_MOVE_RISE, 50, &g).stretch > 100);
    int squash = 0;
    for (uint32_t t = 500; t < 700; t += 4) {
        const int s = face_motion_pose(FACE_MOVE_RISE, t, &g).stretch;
        if (s < squash) squash = s;
    }
    CHECK(squash <= -150);
}

static void drop_anticipates_then_ends_fully_below_the_bezel() {
    for (const auto& s : SIZES) {
        for (int dir = -1; dir <= 1; dir += 2) {
            FaceGeom g = geom(s[0], s[1], dir, 0);
            // A hop up before the fall.
            int min_y = 0;
            for (uint32_t t = 0; t < 200; t += 4) {
                const int y = face_motion_pose(FACE_MOVE_DROP, t, &g).y;
                if (y < min_y) min_y = y;
            }
            CHECK(min_y < 0);
            const FacePose e = face_motion_pose(FACE_MOVE_DROP, face_motion_duration(FACE_MOVE_DROP), &g);
            CHECK(e.y - abs(e.lean) / 2 >= s[1]);     // the high side too
            CHECK(e.lean * dir > 0);                  // leaning into the fall
            CHECK(e.x * dir > 0);                     // drifting toward the low side
        }
    }
}

static void deterministic() {
    FaceGeom g = geom(368, 448, -1, 448);
    for (uint32_t t = 0; t < 900; t += 7) {
        const FacePose a = face_motion_pose(FACE_MOVE_RISE, t, &g);
        const FacePose b = face_motion_pose(FACE_MOVE_RISE, t, &g);
        CHECK(a.y == b.y && a.lean == b.lean && a.stretch == b.stretch && a.eye_dy == b.eye_dy);
    }
}

int main() {
    UNITY_BEGIN();
    RUN_TEST(rise_starts_below_and_ends_dead_still_at_rest);
    RUN_TEST(rise_overshoots_then_dips_then_settles);
    RUN_TEST(lean_is_whole_cells_toward_dir_and_wobbles_back);
    RUN_TEST(arms_show_while_moving_and_leave_at_rest);
    RUN_TEST(eyes_stretch_rising_and_squash_on_landing);
    RUN_TEST(drop_anticipates_then_ends_fully_below_the_bezel);
    RUN_TEST(deterministic);
    return UNITY_END();
}
