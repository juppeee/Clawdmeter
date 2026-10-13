# Clawd's director: rooms, beats and the illusion of life

This is the firmware side of design system v0.4, sections 05–10: one Clawd who
lives on the meters, performs on the stage, and comes up to the glass to check
on you. The behaviour depends on the situation (hook events, usage moving, a
session reset, attention), not on gestures. A tap means one thing everywhere
else: show me the meters. On the meters themselves a tap is tapping the
aquarium: he reacts (see "A tap on the meters" below).

## Modules

| File | Role |
|---|---|
| `firmware/src/clawd_director.{h,cpp}` | Owns the rooms and runs the beats. It is the only module that decides what Clawd does next. |
| `firmware/src/clawd_policy.{h,cpp}` | Pure decision rules, unit-tested in `test/test_clawd_policy`: show gate and show group, screensaver gate, where attention returns to, how long a reaction waits for him, the corner seat's headroom, turn tracker, no-repeat picker, jitter, the resting face's rhythm, what a tap on the meters earns (peek gate, small reactions, where the face looks), and all tunables (`ClawdTuning`). |
| `firmware/src/mascot.{h,cpp}` | Clawd on the meters: the corner Clawd in his seat, plus the full-size sprite used for the lurk trip and the attention walk-on. Does in-room secondary action (idle acts, lurk trips) when the director allows it. |
| `firmware/src/splash.{h,cpp}` | The stage room: a full-screen 60×60-cell stage with one performer who stands, walks, or performs a scene. |
| `firmware/src/face.{h,cpp}` | The #198 face: LVGL rectangles on `lv_layer_top()`. It is both the screensaver and the curtain that rooms change behind. |
| `firmware/src/streak.{h,cpp}` | The orange streak: an LVGL rectangle that sweeps right to left and clips the stage so the meters show behind it. |
| `firmware/src/clawd_anim.{h,cpp}` | Read-only view of the official animations. It is the only translation unit that includes `splash_animations.h` (about 400 KB of static data). |
| `firmware/src/ui.{h,cpp}` | Draws the meters only: cards, clock, status line, battery, the seat. Stale data dims the meters (see below); it never adds a second creature. |

`main.cpp` feeds the director per payload (`director_on_payload` after
`signals_ingest`, then `director_alert` / `director_alert_clear`), on buttons
(`director_button`, or `director_stage_next` for PWR while the stage is up),
and once per loop (`director_tick`). A prompt or back-after-gap event (`e:"p"`
/ `e:"b"`) also calls `idle_note_activity()` first, so a dimmed or dark panel
fades back in before he notices you; an alert already did. A Stop or tool
activity does not wake the panel: Claude working alone isn't you. Taps come straight
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
- Stale data keeps the one-Clawd rule. When the link is up but no fresh usage
  has landed for `DATA_FRESH_MS` (90 s), or the daemon sent `{"ok":false}`,
  the meters stay where they are at 40 % opacity (LVGL `opa`, which scales the
  draws and needs no layer buffer, so it's free on the C6) and the status line
  alternates "Stale…" (or "No data…" before the first payload) with
  "Listening…". The corner Clawd stays in his seat. Shows and the screensaver
  need live meters, so they wait.

## Beats

The director runs one beat at a time. A beat is a list of steps, and each step
issues commands. The next step starts once every actor has settled (face and
streak not moving, stage and mascot not busy) and any explicit pause has
passed. Beat changes always print; every step prints too after
`director_set_trace(true)` (the sim turns it on with `SIM_TRACE=1`).

| Beat | Trigger | Steps |
|---|---|---|
| `home` | default | Corner autonomy on: jittered idle gaps (±40 % of 10 / 7 / 5 / 3.5 s by mood), mood-picked acts, never the same act twice running, and lurk trips (PSRAM). Also checks the gates below. |
| `corner` | notice, wave, joy, or after a transition | Waits for the commanded corner act or walk to finish, then `home`. |
| `show` | `a == 3` held for 5 min; at most one per 15 min; hooks seen; meters live | Points at the bar (or, if he's out on a lurk trip, heads straight out), walks off at 1.5×, face rises, blinks, stage is set behind it, face drops, walks on from the right at 1.5×, one scene (loop held 16 s, so about 20 s), waves, walks off right, face rises, blinks, meters are set behind it, face drops, corner walks back to his seat. |
| `screensaver` | 10 min with no engagement; needs hooks seen or a window that hasn't started; meters live | Walks off through the nearest edge (a lurk trip finishes its peek first), face rises and rests (blinks every 2–6 s, about one in five a double, glances where the far eye travels 1.5×). Every 3–5 min: the stage is set behind the face with Clawd at home, the face drops, one calm scene plays (magnifier or walking, alternating), the face rises, the meters are set behind it, and he rests again. |
| `face-home` | tap, prompt, button or attention while the face is up | If woken by a prompt or tapped during a peek, he blinks first (noticing), then the stage closes behind the face, the face drops into the meters, and the corner hurries home at 1.5×. |
| `streak-home` | tap, prompt or attention while on the stage | The scene releases to the idle pose on the spot, waits 150 ms, runs off right at 3×. The streak starts as soon as his last cell leaves the stage: 380 ms, ease in-out, the stage clipped to its left. Then the stage closes and the corner walks home at 1.5×. |
| `peek` | a tap on the meters while home, at most one per 60 s (below) | Startled hop (`jumping` from its crouch, 1.5×), ducks out through the nearest edge at 3×, face rises, eyes glance toward the tap's x and hold 1.6–2.4 s, a blink (30 % double), the look comes back to you, 350 ms of eye contact, face drops, corner walks back to his seat at 1.5×. |
| `attention` | `m` (alert) in a payload | The corner walks off at 2×, the big Clawd (12 px cells) walks on at 2×, then waves and points alternately. A tap, `x` (clear) or 2 min sends the big one off left. Interrupts a show or the screensaver through `face-home` / `streak-home`, and then returns to where it left off (below). |

**Which scene a show plays.** The trigger (activity 3) lifts the live mood
group to Heavy, so the show reads the usage-only base group instead, floored
at Active (`clawd_show_group`): idle-to-active usage plays the Active set
(laptop, dancing, skateboard, soccer), real burn plays the Heavy set (racing
car, cloud, sailing, jumping happy). Shows never repeat the previous show's
scene, and calm dips never repeat the previous calm scene (tracked separately,
so a calm dip doesn't reset the show's no-repeat memory).

**Where attention returns to** (`attention_resumes_screensaver`):

- It interrupted the **screensaver** and nobody showed up (the alert timed out,
  or was cleared with no tap, button, prompt or tool activity since it
  started): the big Clawd walks off, the corner Clawd stays out of sight, and
  the face rises again. The screensaver carries on as if never interrupted.
  A refreshed alert (a second `m` while one is up) keeps any sign of you seen
  so far, and a new alert that lands while the last one is still leaving for
  the screensaver inherits that origin.
- It interrupted a **show**: the show ends gracefully home. It was a 20 s
  treat; replaying it after an alert of up to 2 minutes would be stale, and the
  show gate already counted it, so the next one is earned as usual.
- A **tap** dismissing it, or any sign of you during it, always ends on the
  meters ("show me the meters"), with the corner Clawd walking home.

Small reactions on the meters, from `home` or `corner`:

- **Noticing**: `e:"p"` or `e:"b"`. He points once (anticipation). From the
  screensaver the same event wakes him (`face-home` with the blink).
- **Long-turn wave**: `e:"s"` when the turn (first prompt to stop) ran at
  least 2 min. He waves once.
- **If he's out** (a lurk trip, or walking back from one): the reaction isn't
  dropped. He cuts the trip short (the rest of the peek plays at 3×), hurries
  back to his seat at 2.5× (or turns around if he was still heading out), and
  then reacts. The reaction waits up to 15 s (`react_pending_ms`) for him. A
  reaction arriving while a commanded act plays queues behind it.
- **Fresh window**: `session_reset` from signals (real samples only, so never
  on a replay). He jumps happy twice, in the same loop iteration as the chime.
  If he's out on a trip he hurries back for it; if he's in another room it
  waits up to 5 min for him to sit down. It outranks the wave.

### A tap on the meters (the aquarium)

While the director is home on the meters, a tap gets a reaction on the next
frame (`clawd_tap_react`):

- **The peek**, when the last one was at least `peek_cooldown_ms` (60 s) ago
  and the meters are live: the `peek` beat above. One Clawd: the corner Clawd
  ducks out through the nearest edge before the face starts to rise. His seat
  is a few steps from the left edge, so that's under a second at 3×, and it
  keeps the "enter and leave through an edge" rule. Letting the rising face
  cover his seat instead would put the face and the corner sprite on screen
  together for most of the rise, since the seat is at the top. The cooldown
  starts when a peek starts, so small reactions never push the next one back.
- **A small corner reaction** otherwise (in the cooldown, or stale / pairing
  meters, where the face would hide the hint): `jumping`, `waving` or
  `dancing`, never the same twice running, with the peek's hop counting as
  `jumping`. Every tap act starts on frame 1 (its crouch), not on the idle
  frame that every act opens with (held 330–580 ms), so the reaction shows
  on the next frame.
- **He's out** on a lurk trip: he hurries back to his seat (the same path a
  prompt takes), and nothing else.
- **Nothing** while a reaction is already playing, he's walking home, or the
  peek's hop or exit is under way. Taps are absorbed, never queued.
- **A tap while the peek's face is up**: he blinks and the face drops
  (`face-home` with the noticing blink), then he walks back to his seat. Once
  the face is already dropping, the tap is absorbed.

Unchanged: a tap during attention dismisses it, and a tap on the stage or the
screensaver face still means "show me the meters". In minimal mode the tap
does nothing (he stays on the still frame). On the C6 it all works: the face
is LVGL rectangles and the corner sprite has its internal-RAM buffer. A tap
that wakes a dark panel is swallowed by `idle_consume_wake_press` (it never
reaches LVGL), so the waking tap only wakes the panel. Every tap counts as
presence: the press edge resets the idle timer, and `director_tap` counts it
as engagement for the screensaver clock.

**The seat leaves headroom for the jump.** Jumping happy rises 12 cells above
the idle pose (30 cells tall). `ui.cpp` lowers the seat's feet line just enough
for it to clear the top edge by 2 px without touching the first panel
(`corner_seat_feet`): on 480×480 the feet sit at y 92, and the 368×448
compact layout starts its panels at y 96 to make room. A 240 px panel has no
such room (the seat would land in the first panel), so there `mascot_act`
swaps any act taller than the headroom for dancing (17 cells) rather than clip
him.

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
| Appeal | The face appears only for the screensaver, as the show's curtain, and for a tap on the meters at most once a minute. |

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
- PWR short press: on the stage it swaps to the next scene and stays in the
  room. During a show that's the next scene in the show's group; during a
  screensaver calm dip it's the other calm scene, never a lively one, and the
  screensaver carries on. Everywhere else it cycles brightness and counts as
  engagement (waking the screensaver face). Hold to pair is unchanged.
- The tap toggle between splash and usage is gone: the stage is earned. On
  the LCD-4, which has only BOOT, a tap no longer opens the splash.

## Tunables

`ClawdTuning` in `clawd_policy.h` holds all the timings: show after 5 min,
cooldown 15 min, show scene loop 16 s, quiet 10 min, calm dip every 3–5 min,
calm scene loop 10 s, alert timeout 2 min, long turn 2 min, joy wait 5 min,
reaction wait 15 s, blinks every 2–6 s, peek cooldown 60 s, peek look
1.6–2.4 s, double blink on a peek 30 %. `clawd_tuning_fast()` compresses them for the sim
(`SIM_FAST=1`; the peek cooldown becomes 20 s).

## QA

- Native: `pio test -d firmware -e native -f test_clawd_policy`.
- Sim: `SIM_FAST=1 SIM_SCENARIO=sim/director.jsonl`, plus `SIM_SHOTS` /
  `SIM_TAPS` for headless capture (see `SIM-USAGE.md`) and `SIM_TRACE=1` for
  the per-step trace. The interruptions (lurk cut short, attention resuming
  the screensaver, PWR on a calm dip, wake) have their own scenario, driven by
  `SIM_CMDS` (scripted serial commands; `pwr`, `prompt`, `stop` and `alert`
  stand for the PWR button, the `e` / `t` keys and the `m` key):

  ```bash
  SIM_FAST=1 SIM_SCENARIO=sim/director-resume.jsonl \
  SIM_CMDS="2500:clawd lurk;5200:prompt;34000:alert;67500:pwr;80000:prompt" \
  .pio/build/sim/program
  ```

  A reference set of captures is in `screenshots/director/` (01–25 the main
  walkthrough, 26 stale data, 27–29 a lurk cut short, 30–31 attention handing
  back to the screensaver, 32 PWR on a calm dip, 33–34 the jump at 368×448 and
  its 240×240 fallback, 35-peek-* a tap on the meters: crouch, hop, ducks out,
  face rises, looks toward the tap, blinks, drops, back in his seat, a small
  reaction in the cooldown, and a tap on the peeking face).

  The tap captures come from a single live payload (any one-line scenario)
  with scripted taps:

  ```bash
  SIM_FAST=1 SIM_TRACE=1 SIM_SCENARIO=<one live payload> \
  SIM_TAPS="3000@420,300;12000@240,300;12150@240,300;12300@240,300;25000@60,300;27500@240,300" \
  .pio/build/sim/program
  ```

  3 s: the peek, looking right. 12 s: a small reaction, and the rapid taps
  after it are absorbed. 25 s: the cooldown is over, so the next peek looks
  left, and the tap at 27.5 s blinks and drops it. A random lurk trip can
  land on one of those taps (`tap at x … -> hurry home` in the log).
- Hardware: send `clawd show`, `clawd screensaver`, `clawd attention`,
  `clawd joy`, `clawd lurk`, `clawd peek [x]` (the peek now, ignoring the
  cooldown, looking toward x) or `clawd home` on the serial console, then `screenshot`
  (`./screenshot.sh`). This replaces swapping the boot screen in `main.cpp`.
  The device boots to the meters.
