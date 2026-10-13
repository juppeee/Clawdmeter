# Clawd's director: rooms, beats and the illusion of life

This is the firmware side of design system v0.4, sections 05–10: one Clawd who
lives on the meters, performs on the stage, and comes up to the glass to check
on you. The behaviour depends on the situation (hook events, usage moving, a
session reset, attention), not on gestures. A tap means one thing everywhere:
show me the meters.

## Modules

| File | Role |
|---|---|
| `firmware/src/clawd_director.{h,cpp}` | Owns the rooms and runs the beats. It is the only module that decides what Clawd does next. |
| `firmware/src/clawd_policy.{h,cpp}` | Pure decision rules, unit-tested in `test/test_clawd_policy`: show gate, screensaver gate, turn tracker, no-repeat picker, jitter, the resting face's rhythm, and all tunables (`ClawdTuning`). |
| `firmware/src/mascot.{h,cpp}` | Clawd on the meters: the corner Clawd in his seat, plus the full-size sprite used for the lurk trip and the attention walk-on. Does in-room secondary action (idle acts, lurk trips) when the director allows it. |
| `firmware/src/splash.{h,cpp}` | The stage room: a full-screen 60×60-cell stage with one performer who stands, walks, or performs a scene. Also the mini creature on the stale-data view. |
| `firmware/src/face.{h,cpp}` | The #198 face: LVGL rectangles on `lv_layer_top()`. It is both the screensaver and the curtain that rooms change behind. |
| `firmware/src/streak.{h,cpp}` | The orange streak: an LVGL rectangle that sweeps right to left and clips the stage so the meters show behind it. |
| `firmware/src/clawd_anim.{h,cpp}` | Read-only view of the official animations. It is the only translation unit that includes `splash_animations.h` (about 400 KB of static data). |
| `firmware/src/ui.{h,cpp}` | Draws the meters only: cards, clock, status line, battery, the seat. |

`main.cpp` feeds the director per payload (`director_on_payload` after
`signals_ingest`, then `director_alert` / `director_alert_clear`), on buttons
(`director_button`), and once per loop (`director_tick`). Taps come straight
from LVGL: an indev `LV_EVENT_PRESSED` callback, so the reaction starts on the
next frame, not on release.

## Rooms

- **Meters**: home, the default, and where every path ends. The corner Clawd
  sits in his seat (3 px cells on a 480 panel).
- **Stage**: the splash container (8 px cells). It opens only behind the face,
  and closes behind the face or the streak.
- **Face**: an overlay on `lv_layer_top()`. It rises from behind the bottom
  bezel with ease-out (600 ms) and drops with ease-in (480 ms).

Continuity rules, as the code enforces them:

- There is only ever one Clawd. The corner sprite, the full-size sprite and
  the stage performer are never on screen together; each beat waits for one to
  be off screen (`mascot_away()`, `splash_performer_visible()`) before the next
  appears.
- He enters and leaves through an edge. Walks use the official walking gait
  with foot-locked translation (`clawd_gait_cells`) and mirror when heading
  left. When the corner leaves, he takes the nearest edge.
- The screen never cuts. `splash_open()` is only called while the face covers
  the screen. `splash_close()` happens behind the face or once the streak has
  passed.

## Beats

The director runs one beat at a time. A beat is a list of steps, and each step
issues commands. The next step starts once every actor has settled (face and
streak not moving, stage and mascot not busy) and any explicit pause has
passed. Every step is printed when built with `-DDIRECTOR_TRACE=1` (the sim
env sets it).

| Beat | Trigger | Steps |
|---|---|---|
| `home` | default | Corner autonomy on: jittered idle gaps (±40 % of 10 / 7 / 5 / 3.5 s by mood), mood-picked acts, never the same act twice running, and lurk trips (PSRAM). Also checks the gates below. |
| `corner` | notice, wave, joy, or after a transition | Waits for the commanded corner act or walk to finish, then `home`. |
| `show` | `a == 3` held for 5 min; at most one per 15 min; hooks seen; meters live | Points at the bar (or, if he's out on a lurk trip, heads straight out), walks off at 1.5×, face rises, blinks, stage is set behind it, face drops, walks on from the right at 1.5×, one scene from the current mood group (loop held 16 s, so about 20 s), waves, walks off right, face rises, blinks, meters are set behind it, face drops, corner walks back to his seat. |
| `screensaver` | 10 min with no engagement; needs hooks seen or a window that hasn't started; meters live | Walks off through the nearest edge (a lurk trip finishes its peek first), face rises and rests (blinks every 2–6 s, about one in five a double, glances where the far eye travels 1.5×). Every 3–5 min: the stage is set behind the face with Clawd at home, the face drops, one calm scene plays (magnifier or walking, alternating), the face rises, the meters are set behind it, and he rests again. |
| `face-home` | tap, prompt, button or attention while the face is up | If woken by a prompt, he blinks first (noticing), then the stage closes behind the face, the face drops into the meters, and the corner hurries home at 1.5×. |
| `streak-home` | tap, prompt or attention while on the stage | The scene releases to the idle pose on the spot, waits 150 ms, runs off right at 3×. The streak starts as soon as his last cell leaves the stage: 380 ms, ease in-out, the stage clipped to its left. Then the stage closes and the corner walks home at 1.5×. |
| `attention` | `m` (alert) in a payload | The corner walks off at 2×, the big Clawd (12 px cells) walks on at 2×, then waves and points alternately. A tap, `x` (clear) or 2 min sends the big one off left and the small one back home. Interrupts a show or the screensaver through `face-home` / `streak-home`. |

Small reactions on the meters, only from `home`:

- **Noticing**: `e:"p"` or `e:"b"`. He points once (anticipation). From the
  screensaver the same event wakes him (`face-home` with the blink).
- **Long-turn wave**: `e:"s"` when the turn (first prompt to stop) ran at
  least 2 min. He waves once.
- **Fresh window**: `session_reset` from signals (real samples only, so never
  on a replay). He jumps happy twice, in the same loop iteration as the chime.
  If he isn't home, it waits up to 5 min for him to sit down. It outranks the
  wave.

**Engagement** resets the screensaver clock. Engagement is any fired hook
event, any activity level above 0 (tool calls are hook events too), a tap, a
button, an alert, or a real sample where session % went up. Usage going up
also wakes the screensaver, which covers the slow-clock-only case.

## Illusion-of-life principles, where they live

| Principle | Implementation |
|---|---|
| Anticipation | Pointing before a show and on a prompt, a blink before the face drops on a wake, a turn toward the edge (walking intro frames) before every exit. |
| Timing | Taps act on `LV_EVENT_PRESSED`. `splash_release()` snaps to the idle pose in the same frame. |
| Secondary action | `clawd_jitter` for idle gaps, `clawd_pick_no_repeat` for acts and scenes, `face_plan_next` for irregular and double blinks and glances. |
| Staging | The meters never animate while he performs: the stage and face cover them, and there's one actor at a time. |
| Slow in, slow out | `lv_anim_path_ease_out` for the rising face, `ease_in` for the drop, `ease_in_out` for the streak. Walks begin on intro frames and end on the outro. |
| Exaggeration | Wave for a long turn, show for a busy stretch, jumping happy for a reset, the big Clawd for attention. |
| Appeal | The face appears only for the screensaver and as the show's curtain. |

## Minimal mode

When `minimal_mode()` is on (BoardCaps default, user override, `minimal` serial
command, `z` in the sim):

- One room. No stage, face, streak, lurk trip, idle acts, noticing, wave or
  joy.
- The corner Clawd stays on the official still frame (walking frame 0).
- Attention: a held pointing frame (pointing frame 16) in the seat, and the
  message in the status line, truncated rather than scrolled. A tap dismisses
  it, as do `x` and the 2 min timeout.
- The status line shows a fixed glyph and one word ("Connected", "No data",
  "Waiting") and redraws only when the state changes.
- Switching at runtime resets Clawd to his seat with no transition. This is
  the one hard cut, and it only happens because the user asked for it.

## Boards without PSRAM (C6)

- Corner Clawd: animated. Its buffer, sized for the largest corner act
  (jumping happy, 28×30 cells × 3 px, about 23 KB), falls back to internal RAM.
- Face and streak: plain LVGL objects, the same as on S3.
- Stage: the direct-to-panel draw path. While the face or streak covers it
  (`splash_set_occluded`), panel writes pause. When it's uncovered, LVGL is
  flushed (`lv_refr_now`) and the stage repaints in full.
- No full-size sprite (about 250 KB), so there's no lurk trip, and attention
  is message-only: the corner stays in his seat.

## Buttons

- BOOT (Space) and SECONDARY (Shift+Tab): unchanged. They now also count as
  engagement and wake the screensaver.
- PWR short press: on the stage it swaps to the next scene in the group,
  otherwise it cycles brightness. It also counts as engagement. Hold to pair
  is unchanged.
- The tap toggle between splash and usage is gone: the stage is earned. On
  the LCD-4, which has only BOOT, a tap no longer opens the splash.

## Tunables

`ClawdTuning` in `clawd_policy.h` holds all the timings: show after 5 min,
cooldown 15 min, show scene loop 16 s, quiet 10 min, calm dip every 3–5 min,
calm scene loop 10 s, alert timeout 2 min, long turn 2 min, joy wait 5 min,
blinks every 2–6 s. `clawd_tuning_fast()` compresses them for the sim
(`SIM_FAST=1`).

## QA

- Native: `pio test -d firmware -e native -f test_clawd_policy`.
- Sim: `SIM_FAST=1 SIM_SCENARIO=sim/director.jsonl`, plus `SIM_SHOTS` /
  `SIM_TAPS` for headless capture (see `SIM-USAGE.md`). A reference set of
  captures is in `screenshots/director/`.
- Hardware: send `clawd show`, `clawd screensaver`, `clawd attention`,
  `clawd joy` or `clawd home` on the serial console, then `screenshot`
  (`./screenshot.sh`). This replaces swapping the boot screen in `main.cpp`.
  The device boots to the meters.
