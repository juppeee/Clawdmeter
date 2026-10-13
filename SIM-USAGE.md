# Desktop simulator (`-e sim`) — usage

The native desktop simulator runs the full firmware loop in an SDL2 window
standing in for the 480×480 AMOLED — `main.cpp`, `ui.cpp`, `splash.cpp`, idle
fade, pair gesture, JSON parsing, and usage-rate/chime logic all run
unmodified. Only `ble.cpp`/`chime.cpp` are swapped for stubs. Sources live in
`firmware/src/boards/sim/` (HAL against SDL2 + Arduino shims in `shim/`), with
scenario data in `firmware/sim/`.

## Build & run

```bash
sudo apt install libsdl2-dev        # one-time (macOS: brew install sdl2)
pio run -d firmware -e sim
cd firmware && .pio/build/sim/program
```

Launch from the `firmware/` directory — the default scenario path
(`sim/scenario.jsonl`) is resolved relative to it.

## Controls

| Key | Action |
|---|---|
| mouse / left-drag | touch: on the meters (live, stale or pairing), the face comes up curious and stays until you tap it or its curiosity runs out (5 min; 12 s with `SIM_FAST`); from any other room, "show me the meters" |
| `space` | play/pause scenario playback |
| `←` / `→` | step one scenario state (pauses playback) |
| `1`–`9` | jump to scenario state N (pauses playback) |
| `d` | toggle BLE connected/disconnected |
| `b` (hold) | PRIMARY button (BOOT — HID Space PTT on hardware) |
| `n` (hold) | SECONDARY button (HID Shift+Tab on hardware) |
| `p` | PWR button (short press; hold ~3s + release = pair gesture) |
| `c` | toggle charging |
| `u` | toggle no battery / USB power only (shows the plug-zap icon) |
| `-` / `=` | battery down / up 5% |
| `e` / `t` | inject a hook event: prompt (`e:"p"`) / stop (`e:"s"`) |
| `a` | inject the activity level, cycling 0 → 1 → 2 → 3 |
| `m` | inject an attention alert / clear it |
| `z` | toggle minimal mode |
| `s` | save screenshot BMP to the current directory |
| `esc` / window close | quit |

Full, authoritative map: `firmware/src/boards/sim/board.h`.

## Scenarios

`firmware/sim/scenario.jsonl` plays in a loop — one JSON object per line, the
daemon payload plus two optional keys:

- `"name"` — shown in the window title
- `"hold_ms"` — time on this state (default 3000)

Lines starting with `#` are comments. Lines containing an `"ss"` array are
**session payloads** (issue #135 wire format) and go out on the session
characteristic path; everything else is a quota payload.

Session row format:

```
[sid, label, state, ctx%, elapsed_s, model, tool, ntools, nagents, tdone, ttotal, tok]
```

States: 0 starting · 1 idle · 2 thinking · 3 responding · 4 running-tool ·
5 compacting · 6 needs-permission · 7 asking-you · 8 needs-input · 9 error.
`tok` is context tokens in 1k units (190 = 190k); `-1`/absent = unknown.

Fast-clock fields from the hook-aware daemon work in scenarios too, since
they go through the same `payload_parse()` → `signals_ingest()` path as on
hardware: `a` (activity 0-3, lifts the mood group), `e` + `es` (latest hook
event `p`/`s`/`b` and its sequence number; the first `es` seen is only a
baseline, each later change fires once), `n` (active sessions) and `rp: 1`
(heartbeat replay, skipped by the rate math). The console logs
`hook event 'p' (#2)` and `usage rate: group 0 -> 3` as they land. Note the
scenario loop wraps `es` back to its first value, which counts as a change.

The injection keys overlay those fields on the current state's payload and
send it through the same path, so you can poke Clawd by hand: `a` three
times then wait for a show, `e` to make him notice, `m` for the big Clawd.

Override the scenario file with `SIM_SCENARIO=<path>`. If the file is missing,
a small built-in state list is used.

## Headless screenshots (CI-friendly)

```bash
SDL_VIDEODRIVER=dummy SIM_AUTOSHOT_MS=6000 .pio/build/sim/program
```

Saves `sim-autoshot.bmp` (override with `SIM_AUTOSHOT_PATH`) after the given
delay and exits. Combine with `SIM_SCENARIO` pointing at a single-state file
to capture any specific screen.

## Clawd walkthrough (rooms and beats)

`firmware/sim/director.jsonl` walks through every beat of the director
(`docs/clawd-director.md`): notice → show → long-turn wave → screensaver
(face, calm scene) → prompt wakes it → attention → clear → fresh window. Real
timings are minutes (a show needs 5 minutes of activity level 3, the
screensaver 10 quiet minutes), so run it with `SIM_FAST=1`, which compresses
them to seconds:

```bash
cd firmware
SIM_FAST=1 SIM_SCENARIO=sim/director.jsonl .pio/build/sim/program
```

With `SIM_FAST` the show starts at ~14 s, the screensaver at ~53 s, the wake
at ~90 s, attention at ~97 s and the reset jump at ~115 s. With `SIM_TRACE=1`
the sim prints a timestamped trace of every beat and step, and
`SIM_CMDS="ms:cmd;…"` types serial commands at set times (`pwr`, `prompt`,
`stop`, `alert`, `link` (the `d` key: BLE link off / on), `clawd <beat>`,
including `clawd peek <x>` for the tap-on-the-meters peek at x):

```
[14014] director: home -> show
[18655] director:   show step 3
...
[53011] director: home -> screensaver
```

Headless capture of several moments, plus scripted taps:

```bash
SDL_VIDEODRIVER=dummy SIM_FAST=1 SIM_SCENARIO=sim/director.jsonl \
  SIM_SHOTS="19500:blink.bmp,26000:scene.bmp,57000:face.bmp" \
  SIM_TAPS="25000@300,250" .pio/build/sim/program
```

- `SIM_SHOTS="ms:path,..."` saves a screenshot at each time (ascending) and
  exits after the last.
- `SIM_TAPS="ms@x,y;..."` holds a 120 ms press at each time. A tap at 25 s
  lands on the show's scene and runs the streak back to the meters. On the
  meters each tap logs how the peek starts (`tap at x 420 -> peek (hop)`
  from his seat, `-> peek (heads out)` when he's already moving or out); on
  the peek's face it logs `-> peek over`, and a face whose curiosity ran out
  logs `peek over (curiosity)`.
- `SIM_MINIMAL=1` boots with the minimal-mode override on.
- `SIM_CMDS="…;9000:perf"` prints the redraw cost of the last face rise,
  face drop and streak (frames, fps, longest gap, pixels per frame), the same
  `perf` serial command as on hardware. The sim's timings are desktop
  timings; pixels per frame carries over to the panel.
- `SIM_EDGE_WATCH=1` logs, with the director beat, every change to how many
  non-black pixels the bottom 16 rows and right 16 columns show. A sliver or
  a stray scrollbar at an edge shows up as a row count that isn't 0 (meters)
  or the full width (face up).
- `SIM_ROUND_EVEN=1` applies the CO5300's even-aligned flush rounding (2.16,
  1.8, 2.06) to the sim's flushes, to reproduce what those panels redraw.

Scenes are picked at random from the mood group, so step times after the
show's scene shift a little between runs; read them from the trace. A
captured set lives in `screenshots/director/`.

## Caveat

The sim mirrors the S3 2.16 geometry but renders with desktop LVGL and fake
data. It's ideal for iterating UI layouts, but panel-level behavior — column
offsets, rotation, flush rounding — lives in the hardware board folders, so
always do a final check on real hardware before merging panel-related changes.
